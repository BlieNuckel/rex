use serde::Deserialize;

#[derive(Deserialize)]
pub struct Container<T> {
    #[serde(rename = "MediaContainer")]
    pub mc: T,
}

#[derive(Deserialize)]
pub struct Directories<T> {
    #[serde(rename = "Directory", default = "Vec::new")]
    pub items: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct Section {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "type", default)]
    pub kind: String,
}

#[derive(Debug)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u32,
}

#[derive(Debug)]
pub struct Artist {
    pub rating_key: String,
    pub title: String,
}

#[derive(Debug)]
pub struct Album {
    pub rating_key: String,
    pub title: String,
    pub parent_title: String,
    pub year: Option<u32>,
    pub leaf_count: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub rating_key: String,
    pub title: String,
    pub grandparent_title: String,
    #[expect(dead_code, reason = "shown by the now-playing bar in M5")]
    pub parent_title: String,
    pub index: Option<u32>,
    pub duration_ms: Option<u64>,
    pub part_key: String,
    pub container: Option<String>,
    pub audio_codec: Option<String>,
}

#[derive(Debug)]
pub struct Playlist {
    pub rating_key: String,
    pub title: String,
    pub leaf_count: Option<u32>,
    pub duration_ms: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct RawItem {
    rating_key: String,
    title: String,
    parent_title: String,
    grandparent_title: String,
    year: Option<u32>,
    leaf_count: Option<u32>,
    index: Option<u32>,
    duration: Option<u64>,
    #[serde(rename = "Media")]
    media: Vec<RawMedia>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawMedia {
    container: Option<String>,
    audio_codec: Option<String>,
    #[serde(rename = "Part")]
    parts: Vec<RawPart>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawPart {
    key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MetadataList {
    #[serde(rename = "Metadata", default)]
    pub items: Vec<RawItem>,
    #[serde(default)]
    pub total_size: Option<u32>,
}

impl From<RawItem> for Artist {
    fn from(r: RawItem) -> Self {
        Self {
            rating_key: r.rating_key,
            title: r.title,
        }
    }
}

impl From<RawItem> for Album {
    fn from(r: RawItem) -> Self {
        Self {
            rating_key: r.rating_key,
            title: r.title,
            parent_title: r.parent_title,
            year: r.year,
            leaf_count: r.leaf_count,
        }
    }
}

impl From<RawItem> for Track {
    fn from(r: RawItem) -> Self {
        let media = r.media.into_iter().next().unwrap_or_default();
        Self {
            rating_key: r.rating_key,
            title: r.title,
            grandparent_title: r.grandparent_title,
            parent_title: r.parent_title,
            index: r.index,
            duration_ms: r.duration,
            part_key: media
                .parts
                .into_iter()
                .next()
                .map(|p| p.key)
                .unwrap_or_default(),
            container: media.container,
            audio_codec: media.audio_codec,
        }
    }
}

impl From<RawItem> for Playlist {
    fn from(r: RawItem) -> Self {
        Self {
            rating_key: r.rating_key,
            title: r.title,
            leaf_count: r.leaf_count,
            duration_ms: r.duration,
        }
    }
}
