use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Result, bail};
use symphonia::core::io::MediaSource;
use tracing::{debug, warn};

use crate::plex::PlexClient;

const BUFFERING_AFTER: Duration = Duration::from_millis(200);
/// reads this close ahead of the active download wait for it instead of opening a new range
const JUMP_AHEAD: u64 = 256 * 1024;

/// sorted, non-overlapping, non-adjacent `[start, end)` byte ranges present in the cache file
#[derive(Default)]
struct Ranges(Vec<(u64, u64)>);

impl Ranges {
    fn insert(&mut self, start: u64, end: u64) {
        self.0.push((start, end));
        self.0.sort_unstable();
        let mut merged: Vec<(u64, u64)> = Vec::with_capacity(self.0.len());
        for &(s, e) in &self.0 {
            match merged.last_mut() {
                Some(last) if s <= last.1 => last.1 = last.1.max(e),
                _ => merged.push((s, e)),
            }
        }
        self.0 = merged;
    }

    /// end of the range containing `pos`
    fn end_of(&self, pos: u64) -> Option<u64> {
        self.0
            .iter()
            .find(|&&(s, e)| s <= pos && pos < e)
            .map(|&(_, e)| e)
    }

    /// first missing byte at or after `from`
    fn first_gap(&self, from: u64, len: u64) -> Option<u64> {
        let mut pos = from;
        while let Some(end) = self.end_of(pos) {
            pos = end;
        }
        (pos < len).then_some(pos)
    }
}

#[derive(Default)]
struct State {
    ranges: Ranges,
    len: Option<u64>,
    started: bool,
    /// every byte is present, or the length is unknown and the download hit EOF
    finished: bool,
    error: Option<String>,
    cancelled: bool,
    /// where the active request writes next
    cursor: u64,
    /// a position a reader is blocked on that the active request won't reach soon
    want: Option<u64>,
    /// the server ignored a range request, so only a single linear download works
    linear: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    cv: Condvar,
}

/// a seekable `MediaSource` over a sparse cache file that a fetch thread fills,
/// jumping with HTTP range requests to wherever the decoder needs data
pub struct CachedHttpSource {
    shared: Arc<Shared>,
    file: File,
    path: PathBuf,
    pos: u64,
    len: Option<u64>,
    on_buffering: Arc<dyn Fn(bool) + Send + Sync>,
}

impl CachedHttpSource {
    /// starts downloading `part_key` into `path` and returns once response headers arrived
    pub fn open(
        client: &Arc<PlexClient>,
        part_key: &str,
        path: PathBuf,
        on_buffering: Arc<dyn Fn(bool) + Send + Sync>,
    ) -> Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut writer = File::create(&path)?;
        let file = File::open(&path)?;
        let shared = Arc::new(Shared::default());

        let (client, key, sh) = (client.clone(), part_key.to_owned(), shared.clone());
        thread::Builder::new().name("fetch".into()).spawn(move || {
            let result = fetch(&client, &key, &mut writer, &sh);
            let mut st = lock(&sh);
            st.started = true;
            match result {
                Ok(()) => st.finished = true,
                Err(e) => {
                    warn!("download {key}: {e:#}");
                    st.error = Some(format!("{e:#}"));
                }
            }
            sh.cv.notify_all();
        })?;

        let st = shared
            .cv
            .wait_while(lock(&shared), |s| !s.started)
            .unwrap_or_else(|e| e.into_inner());
        if let Some(e) = &st.error {
            bail!("{e}");
        }
        let len = st.len;
        drop(st);
        Ok(Self {
            shared,
            file,
            path,
            pos: 0,
            len,
            on_buffering,
        })
    }
}

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, State> {
    shared.state.lock().unwrap_or_else(|e| e.into_inner())
}

/// downloads until every byte is present, restarting at a wanted offset when a reader jumps
fn fetch(client: &PlexClient, key: &str, out: &mut File, sh: &Shared) -> Result<()> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut from = 0;
    loop {
        let mut stream = client.stream(key, from)?;
        let mut pos = stream.start;
        {
            let mut st = lock(sh);
            if stream.start != from {
                debug!("download {key}: server ignored range request, falling back to linear");
                st.linear = true;
                st.ranges = Ranges::default();
            }
            if st.len.is_none() {
                st.len = stream.len;
                if let Some(len) = stream.len {
                    // sparse on Linux, so unfetched ranges cost no disk
                    out.set_len(len)?;
                }
            }
            st.started = true;
            st.cursor = pos;
            sh.cv.notify_all();
        }
        out.seek(SeekFrom::Start(pos))?;

        let jump = loop {
            if lock(sh).cancelled {
                debug!("download {key} cancelled");
                return Ok(());
            }
            let n = stream.body.read(&mut buf)?;
            if n == 0 {
                break None;
            }
            out.write_all(&buf[..n])?;
            let mut st = lock(sh);
            st.ranges.insert(pos, pos + n as u64);
            pos += n as u64;
            st.cursor = pos;
            sh.cv.notify_all();
            if st.linear {
                continue;
            }
            if let Some(w) = st.want.take() {
                break Some(w);
            }
            // ran into bytes fetched earlier; move on to the next gap
            if st.ranges.end_of(pos).is_some() {
                break Some(pos);
            }
        };

        let st = lock(sh);
        let Some(len) = st.len.filter(|_| !st.linear) else {
            return Ok(());
        };
        let next = jump
            .and_then(|w| st.ranges.first_gap(w, len))
            .or_else(|| st.ranges.first_gap(0, len));
        match next {
            Some(n) => {
                debug!("download {key}: continuing at byte {n}");
                from = n;
            }
            None => return Ok(()),
        }
    }
}

impl Read for CachedHttpSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.len.is_some_and(|len| self.pos >= len) {
            return Ok(0);
        }
        let mut st = lock(&self.shared);
        let mut buffering = false;
        let mut asked = false;
        let end = loop {
            if let Some(end) = st.ranges.end_of(self.pos) {
                break end;
            }
            if let Some(e) = &st.error {
                return Err(io::Error::other(e.clone()));
            }
            if st.finished {
                return Ok(0);
            }
            let near = st.cursor <= self.pos && self.pos < st.cursor + JUMP_AHEAD;
            if !asked && !near && st.len.is_some() && !st.linear {
                st.want = Some(self.pos);
                asked = true;
            }
            let (guard, timeout) = self
                .shared
                .cv
                .wait_timeout(st, BUFFERING_AFTER)
                .unwrap_or_else(|e| e.into_inner());
            st = guard;
            if timeout.timed_out() && !buffering {
                buffering = true;
                (self.on_buffering)(true);
            }
        };
        drop(st);
        if buffering {
            (self.on_buffering)(false);
        }
        let avail = (end - self.pos).min(buf.len() as u64) as usize;
        self.file.seek(SeekFrom::Start(self.pos))?;
        let n = self.file.read(&mut buf[..avail])?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for CachedHttpSource {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let pos = match to {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            SeekFrom::End(d) => {
                let len = self.len.or_else(|| {
                    let st = lock(&self.shared);
                    st.finished.then(|| st.ranges.0.last().map_or(0, |r| r.1))
                });
                match len {
                    Some(len) => len.checked_add_signed(d),
                    None => return Err(io::Error::other("seek from end with unknown length")),
                }
            }
        };
        self.pos = pos.ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok(self.pos)
    }
}

impl MediaSource for CachedHttpSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

impl Drop for CachedHttpSource {
    fn drop(&mut self) {
        lock(&self.shared).cancelled = true;
        if let Err(e) = fs::remove_file(&self.path) {
            debug!("removing {}: {e}", self.path.display());
        }
    }
}
