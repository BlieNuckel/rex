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

#[derive(Default)]
struct State {
    downloaded: u64,
    len: Option<u64>,
    started: bool,
    done: bool,
    error: Option<String>,
    cancelled: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    cv: Condvar,
}

pub struct CachedHttpSource {
    shared: Arc<Shared>,
    file: File,
    path: PathBuf,
    pos: u64,
    len: Option<u64>,
    on_buffering: Arc<dyn Fn(bool) + Send + Sync>,
}

impl CachedHttpSource {
    /// Starts downloading `part_key` into `path` and returns once response headers arrived
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
            let result = download(&client, &key, &mut writer, &sh);
            let mut st = sh.state.lock().unwrap_or_else(|e| e.into_inner());
            st.started = true;
            st.done = true;
            if let Err(e) = result {
                warn!("download {key}: {e:#}");
                st.error = Some(format!("{e:#}"));
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

fn download(client: &PlexClient, key: &str, out: &mut File, sh: &Shared) -> Result<()> {
    let (len, mut body) = client.stream(key)?;
    {
        let mut st = lock(sh);
        st.len = len;
        st.started = true;
        sh.cv.notify_all();
    }
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if lock(sh).cancelled {
            debug!("download {key} cancelled");
            return Ok(());
        }
        let n = body.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        out.write_all(&buf[..n])?;
        lock(sh).downloaded += n as u64;
        sh.cv.notify_all();
    }
}

impl Read for CachedHttpSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut st = lock(&self.shared);
        let mut buffering = false;
        while st.downloaded <= self.pos && !st.done {
            let (guard, timeout) = self
                .shared
                .cv
                .wait_timeout(st, BUFFERING_AFTER)
                .unwrap_or_else(|e| e.into_inner());
            st = guard;
            if timeout.timed_out() && !buffering && st.downloaded <= self.pos && !st.done {
                buffering = true;
                (self.on_buffering)(true);
            }
        }
        if buffering {
            (self.on_buffering)(false);
        }
        if st.downloaded <= self.pos {
            return match &st.error {
                Some(e) => Err(io::Error::other(e.clone())),
                None => Ok(0),
            };
        }
        let avail = (st.downloaded - self.pos).min(buf.len() as u64) as usize;
        drop(st);
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
                    st.done.then_some(st.downloaded)
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
