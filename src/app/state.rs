use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};

use super::keys::Action;
use super::{ApiRequest, PageData};
use crate::plex::models::{Album, Artist, Playlist, Track};

const PREFETCH_MARGIN: usize = 50;
const MESSAGE_TTL: Duration = Duration::from_secs(5);

pub const SECTIONS: [&str; 5] = ["Artists", "Albums", "Playlists", "Search", "Queue"];

#[derive(Debug, Clone)]
pub enum ListKind {
    Artists,
    Albums,
    Playlists,
    ArtistAlbums(String),
    AlbumTracks(String),
    PlaylistTracks(String),
    Search,
    Queue,
}

pub enum Items {
    Artists(Vec<Artist>),
    Albums(Vec<Album>),
    Tracks(Vec<Track>),
    Playlists(Vec<Playlist>),
}

impl Items {
    pub fn len(&self) -> usize {
        match self {
            Items::Artists(v) => v.len(),
            Items::Albums(v) => v.len(),
            Items::Tracks(v) => v.len(),
            Items::Playlists(v) => v.len(),
        }
    }
}

pub struct View {
    pub id: u64,
    pub kind: ListKind,
    pub title: String,
    pub items: Items,
    pub total: Option<u32>,
    pub loading: bool,
    pub selected: usize,
    pub offset: usize,
}

impl View {
    fn fetchable(&self) -> bool {
        !matches!(self.kind, ListKind::Search | ListKind::Queue)
    }

    fn complete(&self) -> bool {
        !self.fetchable() || self.total.is_some_and(|t| self.items.len() >= t as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    List,
}

pub struct AppState {
    api: SyncSender<ApiRequest>,
    next_id: u64,
    pub focus: Focus,
    pub section: usize,
    pub stack: Vec<View>,
    pub help: bool,
    pub debug_line: bool,
    pub message: Option<(String, Instant)>,
    pub list_height: usize,
    pub quit: bool,
}

impl AppState {
    pub fn new(api: SyncSender<ApiRequest>) -> Self {
        let mut s = Self {
            api,
            next_id: 0,
            focus: Focus::List,
            section: 0,
            stack: Vec::new(),
            help: false,
            debug_line: false,
            message: None,
            list_height: 10,
            quit: false,
        };
        s.set_section(0);
        s
    }

    pub fn view(&self) -> Option<&View> {
        self.stack.last()
    }

    fn view_mut(&mut self) -> Option<&mut View> {
        self.stack.last_mut()
    }

    pub fn error(&mut self, msg: String) {
        tracing::warn!("{msg}");
        self.message = Some((msg, Instant::now()));
    }

    /// how long until the transient message should disappear
    pub fn next_deadline(&self) -> Option<Duration> {
        self.message
            .as_ref()
            .map(|(_, at)| MESSAGE_TTL.saturating_sub(at.elapsed()))
    }

    pub fn expire_message(&mut self) {
        if self.next_deadline().is_some_and(|d| d.is_zero()) {
            self.message = None;
        }
    }

    fn new_view(&mut self, kind: ListKind, title: String) -> View {
        self.next_id += 1;
        let items = match kind {
            ListKind::Artists => Items::Artists(Vec::new()),
            ListKind::Albums | ListKind::ArtistAlbums(_) => Items::Albums(Vec::new()),
            ListKind::Playlists => Items::Playlists(Vec::new()),
            ListKind::AlbumTracks(_)
            | ListKind::PlaylistTracks(_)
            | ListKind::Search
            | ListKind::Queue => Items::Tracks(Vec::new()),
        };
        View {
            id: self.next_id,
            kind,
            title,
            items,
            total: None,
            loading: false,
            selected: 0,
            offset: 0,
        }
    }

    fn set_section(&mut self, i: usize) {
        self.section = i;
        let kind = match i {
            0 => ListKind::Artists,
            1 => ListKind::Albums,
            2 => ListKind::Playlists,
            3 => ListKind::Search,
            _ => ListKind::Queue,
        };
        let view = self.new_view(kind, SECTIONS[i].to_owned());
        self.stack = vec![view];
        self.ensure_loaded();
    }

    fn push(&mut self, kind: ListKind, title: String) {
        let view = self.new_view(kind, title);
        self.stack.push(view);
        self.ensure_loaded();
    }

    /// requests the next page when the selection nears the end of what is loaded
    fn ensure_loaded(&mut self) {
        let api = self.api.clone();
        let Some(v) = self.view_mut() else { return };
        if v.loading || v.complete() {
            return;
        }
        let len = v.items.len();
        if len > 0 && v.selected + PREFETCH_MARGIN < len {
            return;
        }
        let req = ApiRequest::Page {
            view: v.id,
            kind: v.kind.clone(),
            start: len as u32,
        };
        if api.try_send(req).is_ok() {
            v.loading = true;
        }
    }

    pub fn on_page(&mut self, view: u64, start: u32, result: Result<PageData, String>) {
        let Some(v) = self.stack.iter_mut().find(|v| v.id == view) else {
            return;
        };
        v.loading = false;
        let page = match result {
            Ok(p) => p,
            Err(e) => return self.error(e),
        };
        if start as usize != v.items.len() {
            return;
        }
        match (&mut v.items, page) {
            (Items::Artists(dst), PageData::Artists(p)) => {
                v.total = Some(p.total);
                dst.extend(p.items);
            }
            (Items::Albums(dst), PageData::Albums(p)) => {
                v.total = Some(p.total);
                dst.extend(p.items);
            }
            (Items::Tracks(dst), PageData::Tracks(p)) => {
                v.total = Some(p.total);
                dst.extend(p.items);
            }
            (Items::Playlists(dst), PageData::Playlists(p)) => {
                v.total = Some(p.total);
                dst.extend(p.items);
            }
            _ => return,
        }
        // an empty page means the server's totalSize overstated the list
        if v.items.len() == start as usize {
            v.total = Some(start);
        }
        self.ensure_loaded();
    }

    pub fn on_action(&mut self, action: Action) {
        if self.help {
            if matches!(action, Action::Help | Action::Quit | Action::Back) {
                self.help = false;
            }
            return;
        }
        let half = (self.list_height / 2).max(1) as isize;
        match action {
            Action::Down => self.move_by(1),
            Action::Up => self.move_by(-1),
            Action::Top => self.move_by(isize::MIN / 2),
            Action::Bottom => self.move_by(isize::MAX / 2),
            Action::HalfDown => self.move_by(half),
            Action::HalfUp => self.move_by(-half),
            Action::Open => self.open(),
            Action::Back => self.back(),
            Action::ToggleFocus => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::List,
                    Focus::List => Focus::Sidebar,
                }
            }
            Action::Jump(i) => {
                self.set_section(i);
                self.focus = Focus::List;
            }
            Action::Help => self.help = true,
            Action::DebugLine => self.debug_line = !self.debug_line,
            Action::Quit => self.quit = true,
        }
    }

    fn move_by(&mut self, delta: isize) {
        match self.focus {
            Focus::Sidebar => {
                let i = self
                    .section
                    .saturating_add_signed(delta)
                    .min(SECTIONS.len() - 1);
                if i != self.section {
                    self.set_section(i);
                }
            }
            Focus::List => {
                let Some(v) = self.view_mut() else { return };
                let last = v.items.len().saturating_sub(1);
                v.selected = v.selected.saturating_add_signed(delta).min(last);
                self.ensure_loaded();
            }
        }
    }

    fn open(&mut self) {
        if self.focus == Focus::Sidebar {
            self.focus = Focus::List;
            return;
        }
        let Some(v) = self.view() else { return };
        let i = v.selected;
        let child = match &v.items {
            Items::Artists(a) => a.get(i).map(|a| {
                (
                    ListKind::ArtistAlbums(a.rating_key.clone()),
                    a.title.clone(),
                )
            }),
            Items::Albums(a) => a.get(i).map(|a| {
                let title = if a.parent_title.is_empty() {
                    a.title.clone()
                } else {
                    format!("{} — {}", a.title, a.parent_title)
                };
                (ListKind::AlbumTracks(a.rating_key.clone()), title)
            }),
            Items::Playlists(p) => p.get(i).map(|p| {
                (
                    ListKind::PlaylistTracks(p.rating_key.clone()),
                    p.title.clone(),
                )
            }),
            Items::Tracks(_) => None,
        };
        if let Some((kind, title)) = child {
            self.push(kind, title);
        }
    }

    fn back(&mut self) {
        if self.focus == Focus::List && self.stack.len() > 1 {
            self.stack.pop();
        } else {
            self.focus = Focus::Sidebar;
        }
    }
}
