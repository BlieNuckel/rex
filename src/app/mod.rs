pub mod keys;
pub mod state;

use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::plex::PlexClient;
use crate::plex::models::{Album, Artist, Page, Playlist, Track};
use state::{AppState, ListKind};

const API_WORKERS: usize = 2;

pub enum ApiRequest {
    Page {
        view: u64,
        kind: ListKind,
        start: u32,
    },
}

pub enum PageData {
    Artists(Page<Artist>),
    Albums(Page<Album>),
    Tracks(Page<Track>),
    Playlists(Page<Playlist>),
}

pub enum AppEvent {
    Input(Event),
    Page {
        view: u64,
        start: u32,
        result: Result<PageData, String>,
    },
}

pub fn run(client: Arc<PlexClient>, section: String) -> Result<()> {
    let (tx, rx) = sync_channel(64);
    let (api_tx, api_rx) = sync_channel(16);
    spawn_api_workers(client, section, api_rx, tx.clone())?;
    spawn_input(tx)?;

    let mut state = AppState::new(api_tx);
    // ratatui::init installs a panic hook that restores the terminal
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut state, &rx);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut AppState,
    rx: &Receiver<AppEvent>,
) -> Result<()> {
    loop {
        terminal.draw(|f| crate::ui::draw(f, state))?;
        let first = match state.next_deadline() {
            Some(d) => match rx.recv_timeout(d) {
                Ok(ev) => Some(ev),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            },
            None => Some(rx.recv().context("event channel closed")?),
        };
        for ev in first.into_iter().chain(rx.try_iter()) {
            handle(state, ev);
        }
        state.expire_message();
        if state.quit {
            return Ok(());
        }
    }
}

fn handle(state: &mut AppState, ev: AppEvent) {
    match ev {
        AppEvent::Input(Event::Key(k)) if k.kind == KeyEventKind::Press => {
            if let Some(action) = keys::lookup(k) {
                state.on_action(action);
            }
        }
        AppEvent::Input(_) => {}
        AppEvent::Page {
            view,
            start,
            result,
        } => state.on_page(view, start, result),
    }
}

fn spawn_input(tx: SyncSender<AppEvent>) -> Result<()> {
    thread::Builder::new().name("input".into()).spawn(move || {
        while let Ok(ev) = event::read() {
            if tx.send(AppEvent::Input(ev)).is_err() {
                break;
            }
        }
    })?;
    Ok(())
}

fn spawn_api_workers(
    client: Arc<PlexClient>,
    section: String,
    rx: Receiver<ApiRequest>,
    tx: SyncSender<AppEvent>,
) -> Result<()> {
    let rx = Arc::new(Mutex::new(rx));
    for i in 0..API_WORKERS {
        let (client, section, rx, tx) = (client.clone(), section.clone(), rx.clone(), tx.clone());
        thread::Builder::new()
            .name(format!("api-{i}"))
            .spawn(move || {
                loop {
                    let req = match rx.lock() {
                        Ok(rx) => rx.recv(),
                        Err(_) => break,
                    };
                    let Ok(req) = req else { break };
                    let ev = match req {
                        ApiRequest::Page { view, kind, start } => AppEvent::Page {
                            view,
                            start,
                            result: fetch_page(&client, &section, &kind, start)
                                .map_err(|e| format!("{e:#}")),
                        },
                    };
                    if tx.send(ev).is_err() {
                        break;
                    }
                }
            })?;
    }
    Ok(())
}

fn fetch_page(c: &PlexClient, section: &str, kind: &ListKind, start: u32) -> Result<PageData> {
    Ok(match kind {
        ListKind::Artists => PageData::Artists(c.artists(section, start)?),
        ListKind::Albums => PageData::Albums(c.albums(section, start)?),
        ListKind::Playlists => PageData::Playlists(c.playlists(start)?),
        ListKind::ArtistAlbums(rk) => PageData::Albums(c.artist_albums(rk, start)?),
        ListKind::AlbumTracks(rk) => PageData::Tracks(c.album_tracks(rk, start)?),
        ListKind::PlaylistTracks(rk) => PageData::Tracks(c.playlist_tracks(rk, start)?),
        ListKind::Search | ListKind::Queue => anyhow::bail!("{kind:?} is not paginated"),
    })
}
