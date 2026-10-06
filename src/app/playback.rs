use std::time::{Duration, Instant};

use tracing::warn;

use super::state::{AppState, ListKind};
use crate::audio::player::{PlayerCmd, PlayerEvent};

const ENQUEUE_BEFORE_END_MS: u64 = 10_000;
const RESTART_AFTER_MS: u64 = 3_000;
const SEEK_STEP_MS: i64 = 10_000;
const VOLUME_STEP: f32 = 0.05;
const QUIT_CONFIRM: Duration = Duration::from_secs(3);

impl AppState {
    fn send(&self, cmd: PlayerCmd) {
        if self.player.send(cmd).is_err() {
            warn!("player thread is gone");
        }
    }

    pub fn play_index(&mut self, i: usize) {
        let Some(e) = self.queue.entries.get(i) else {
            return;
        };
        let track = e.track.clone();
        self.loading = Some((e.id, track.rating_key.clone()));
        self.queue.current = Some(i);
        self.enqueued = None;
        self.now.track = Some(track.clone());
        self.now.position_ms = 0;
        self.now.paused = false;
        self.now.buffering = false;
        self.send(PlayerCmd::Load(track));
    }

    pub fn stop(&mut self) {
        self.send(PlayerCmd::Stop);
        self.now.track = None;
        self.now.position_ms = 0;
        self.now.paused = false;
        self.now.buffering = false;
        self.loading = None;
        self.enqueued = None;
    }

    pub fn toggle_pause(&mut self) {
        if self.now.track.is_none() {
            if let Some(i) = self.queue.current.or_else(|| self.queue.next_index(false)) {
                self.play_index(i);
            }
            return;
        }
        self.now.paused = !self.now.paused;
        self.send(if self.now.paused {
            PlayerCmd::Pause
        } else {
            PlayerCmd::Play
        });
    }

    pub fn skip(&mut self) {
        match self.queue.next_index(false) {
            Some(i) => self.play_index(i),
            None => self.stop(),
        }
    }

    pub fn previous(&mut self) {
        if self.now.track.is_some() && self.now.position_ms > RESTART_AFTER_MS {
            self.send(PlayerCmd::Seek(0));
            self.now.position_ms = 0;
        } else if let Some(i) = self.queue.prev_index() {
            self.play_index(i);
        }
    }

    pub fn seek_by(&mut self, forward: bool) {
        let Some(t) = &self.now.track else { return };
        let delta = if forward { SEEK_STEP_MS } else { -SEEK_STEP_MS };
        let mut target = self.now.position_ms.saturating_add_signed(delta);
        if let Some(d) = t.duration_ms {
            target = target.min(d.saturating_sub(1_000));
        }
        self.now.position_ms = target;
        self.send(PlayerCmd::Seek(target));
    }

    pub fn change_volume(&mut self, up: bool) {
        let step = if up { VOLUME_STEP } else { -VOLUME_STEP };
        self.now.volume = ((self.now.volume + step) * 20.0).round().clamp(0.0, 20.0) / 20.0;
        self.send(PlayerCmd::SetVolume(self.now.volume));
    }

    pub fn toggle_shuffle(&mut self) {
        self.queue.set_shuffle(!self.queue.shuffle);
        self.sync_next();
    }

    pub fn cycle_repeat(&mut self) {
        self.queue.repeat = self.queue.repeat.cycle();
        self.sync_next();
    }

    /// keeps the player's pre-opened next track in line with the queue
    pub fn sync_next(&mut self) {
        let due = self.now.track.as_ref().is_some_and(|t| {
            t.duration_ms
                .is_none_or(|d| d.saturating_sub(self.now.position_ms) <= ENQUEUE_BEFORE_END_MS)
        });
        let next = self
            .queue
            .next_index(true)
            .and_then(|i| self.queue.entries.get(i))
            .filter(|_| self.now.track.is_some());
        match (next, &self.enqueued) {
            (Some(e), Some((id, _))) if e.id == *id => {}
            (Some(e), _) if due => {
                self.enqueued = Some((e.id, e.track.rating_key.clone()));
                let track = e.track.clone();
                self.send(PlayerCmd::EnqueueNext(track));
            }
            (_, Some(_)) => {
                self.enqueued = None;
                self.send(PlayerCmd::ClearNext);
            }
            _ => {}
        }
    }

    pub fn on_player(&mut self, ev: PlayerEvent) {
        match ev {
            PlayerEvent::Position(ms) => {
                self.now.position_ms = ms;
                self.sync_next();
            }
            PlayerEvent::TrackStarted(t) => {
                let entry = if self
                    .enqueued
                    .as_ref()
                    .is_some_and(|(_, rk)| *rk == t.rating_key)
                {
                    self.enqueued.take()
                } else if self
                    .loading
                    .as_ref()
                    .is_some_and(|(_, rk)| *rk == t.rating_key)
                {
                    self.loading.take()
                } else {
                    None
                };
                if let Some(i) = entry.and_then(|(id, _)| self.queue.position(id)) {
                    self.queue.current = Some(i);
                }
                self.now.track = Some(t);
                self.now.position_ms = 0;
                self.now.buffering = false;
                self.failures = 0;
                self.sync_next();
            }
            PlayerEvent::TrackEnded(_) => {
                // a pre-opened successor is about to report TrackStarted
                if self.enqueued.is_some() {
                    return;
                }
                match self.queue.next_index(true) {
                    Some(i) => self.play_index(i),
                    None => {
                        self.now.track = None;
                        self.now.position_ms = 0;
                    }
                }
            }
            PlayerEvent::Error {
                rating_key,
                message,
            } => {
                self.error(message);
                if self
                    .loading
                    .as_ref()
                    .is_some_and(|(_, rk)| *rk == rating_key)
                {
                    self.loading = None;
                    self.failures += 1;
                    match self.queue.next_index(false) {
                        Some(i) if self.failures < self.queue.len() => self.play_index(i),
                        _ => self.stop(),
                    }
                } else if self
                    .enqueued
                    .as_ref()
                    .is_some_and(|(_, rk)| *rk == rating_key)
                {
                    self.enqueued = None;
                }
            }
            PlayerEvent::BufferingChanged(b) => self.now.buffering = b,
        }
    }

    fn in_queue_view(&self) -> bool {
        matches!(self.view().map(|v| &v.kind), Some(ListKind::Queue))
    }

    pub fn queue_edit(&mut self, f: impl FnOnce(&mut Self, usize)) {
        if !self.in_queue_view() {
            return;
        }
        let sel = self.view().map_or(0, |v| v.selected);
        f(self, sel);
        let last = self.queue.len().saturating_sub(1);
        if let Some(v) = self.stack.last_mut() {
            v.selected = v.selected.min(last);
        }
        self.sync_next();
    }

    pub fn remove_at(&mut self, i: usize) {
        if self.queue.remove(i) && self.now.track.is_some() {
            if i < self.queue.len() {
                self.play_index(i);
            } else {
                self.stop();
            }
        }
    }

    pub fn move_at(&mut self, i: usize, down: bool) {
        if let Some(j) = self.queue.move_entry(i, down)
            && let Some(v) = self.stack.last_mut()
        {
            v.selected = j;
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.stop();
    }

    /// quits right away unless something is playing, then asks for a second `q`
    pub fn request_quit(&mut self) {
        let playing = self.now.track.is_some() && !self.now.paused;
        if !playing || self.quit_armed.is_some_and(|t| t.elapsed() < QUIT_CONFIRM) {
            self.quit = true;
        } else {
            self.quit_armed = Some(Instant::now());
            self.info("press q again to quit".into());
        }
    }
}
