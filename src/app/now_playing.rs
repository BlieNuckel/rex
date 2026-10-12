use crate::plex::models::Track;

#[derive(Default)]
pub struct NowPlaying {
    pub track: Option<Track>,
    pub position_ms: u64,
    pub paused: bool,
    pub buffering: bool,
    pub volume: f32,
}

impl NowPlaying {
    pub fn artist_playing_now(&self, artist_title: &String) -> bool {
        self.track
            .as_ref()
            .is_some_and(|t| t.grandparent_title == *artist_title)
    }

    pub fn album_playing_now(&self, album_key: &String) -> bool {
        self.track
            .as_ref()
            .is_some_and(|t| t.parent_rating_key == *album_key)
    }

    pub fn track_playing_now(&self, track_key: &String) -> bool {
        self.track
            .as_ref()
            .is_some_and(|t| t.rating_key == *track_key)
    }
}
