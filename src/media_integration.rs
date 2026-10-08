use std::sync::mpsc::SyncSender;
use std::time::Duration;

#[cfg(target_os = "macos")]
use core_foundation::runloop::CFRunLoop;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use tracing::warn;

use crate::app::AppEvent;
use crate::app::state::NowPlaying;
use crate::plex::PlexClient;

pub struct MediaIntegration {
    controls: MediaControls,
}

/// blocks the main thread on the run loop macOS uses for media key and Control Center commands
#[cfg(target_os = "macos")]
pub fn run_main_loop() {
    CFRunLoop::run_current();
}

#[cfg(not(target_os = "macos"))]
pub fn run_main_loop() {}

/// stops `run_main_loop` on drop, so a failing or panicking UI thread can't leave the main thread blocked
pub struct StopMainLoop;

impl Drop for StopMainLoop {
    #[cfg(target_os = "macos")]
    fn drop(&mut self) {
        CFRunLoop::get_main().stop();
    }

    #[cfg(not(target_os = "macos"))]
    fn drop(&mut self) {}
}

impl MediaIntegration {
    /// registers on the session bus; returns `None` (after logging once) when D-Bus isn't usable
    pub fn start(tx: SyncSender<AppEvent>) -> Option<Self> {
        let config = PlatformConfig {
            dbus_name: "rex",
            display_name: "rex",
            hwnd: None,
        };
        let mut controls = match MediaControls::new(config) {
            Ok(c) => c,
            Err(e) => {
                warn!("Media Integration unavailable: {e:?}");
                return None;
            }
        };
        let attached = controls.attach(move |ev: MediaControlEvent| {
            let _ = tx.send(AppEvent::MediaIntegration(ev));
        });
        if let Err(e) = attached {
            warn!("Media Integration unavailable: {e:?}");
            return None;
        }
        Some(Self { controls })
    }

    pub fn update(&mut self, now: &NowPlaying, client: &PlexClient) {
        let Some(t) = &now.track else {
            return self.apply(MediaMetadata::default(), MediaPlayback::Stopped);
        };
        let art = (!t.thumb.is_empty()).then(|| client.url_with_token(&t.thumb));
        let metadata = MediaMetadata {
            title: Some(&t.title),
            artist: Some(&t.grandparent_title),
            album: Some(&t.parent_title),
            duration: t.duration_ms.map(Duration::from_millis),
            cover_url: art.as_deref(),
        };
        let progress = Some(MediaPosition(Duration::from_millis(now.position_ms)));
        let playback = if now.paused {
            MediaPlayback::Paused { progress }
        } else {
            MediaPlayback::Playing { progress }
        };
        self.apply(metadata, playback);
    }

    fn apply(&mut self, metadata: MediaMetadata, playback: MediaPlayback) {
        if let Err(e) = self.controls.set_metadata(metadata) {
            warn!("Media Integration metadata: {e:?}");
        }
        if let Err(e) = self.controls.set_playback(playback) {
            warn!("Media Integration playback: {e:?}");
        }
    }
}
