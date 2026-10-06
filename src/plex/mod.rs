#[expect(dead_code, reason = "search results are consumed by the TUI in M6")]
pub mod api;
pub mod auth;
pub mod client;
#[expect(dead_code, reason = "fields are read by the browse UI in M4")]
pub mod models;

pub use client::PlexClient;
