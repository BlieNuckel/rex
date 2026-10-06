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

#[derive(Deserialize)]
pub struct Section {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "type", default)]
    pub kind: String,
}
