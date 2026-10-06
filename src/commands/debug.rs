use std::fmt::Debug;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;

use crate::config::Config;
use crate::plex::auth;
use crate::plex::models::Page;

#[derive(Parser, Debug)]
pub(super) struct Options {
    #[command(subcommand)]
    subcommand: Subcommand,
}

#[derive(Parser, Debug)]
enum Subcommand {
    /// List all library sections on the server
    Sections,

    /// List every artist in the music library
    Artists,

    /// List an artist's albums, or every album when no artist is given
    Albums {
        /// Artist rating key
        artist: Option<String>,
    },

    /// List an album's tracks
    Tracks {
        /// Album rating key
        album: String,
    },

    /// List audio playlists, or a playlist's tracks when one is given
    Playlists {
        /// Playlist rating key
        playlist: Option<String>,
    },

    /// Search artists, albums and tracks
    Search {
        #[arg(required = true)]
        query: Vec<String>,
    },
}

impl Options {
    pub fn run(self, cfg: &mut Config) -> Result<ExitCode> {
        use Subcommand as SC;
        let client = auth::connect(cfg)?;
        let section = cfg
            .music_section
            .clone()
            .context("no music section selected")?;
        match self.subcommand {
            SC::Sections => client.sections()?.iter().for_each(|s| println!("{s:?}")),
            SC::Artists => print_all(|start| client.artists(&section, start))?,
            SC::Albums { artist: None } => print_all(|start| client.albums(&section, start))?,
            SC::Albums { artist: Some(rk) } => print_all(|start| client.artist_albums(&rk, start))?,
            SC::Tracks { album } => print_all(|start| client.album_tracks(&album, start))?,
            SC::Playlists { playlist: None } => print_all(|start| client.playlists(start))?,
            SC::Playlists { playlist: Some(rk) } => {
                print_all(|start| client.playlist_tracks(&rk, start))?
            }
            SC::Search { query } => println!("{:#?}", client.search(&section, &query.join(" "))?),
        }
        Ok(ExitCode::SUCCESS)
    }
}

fn print_all<T: Debug>(mut fetch: impl FnMut(u32) -> Result<Page<T>>) -> Result<()> {
    let mut seen = 0;
    loop {
        let page = fetch(seen)?;
        if page.items.is_empty() {
            break;
        }
        seen += page.items.len() as u32;
        page.items.iter().for_each(|i| println!("{i:?}"));
        eprintln!("-- {seen} of {}", page.total);
        if seen >= page.total {
            break;
        }
    }
    Ok(())
}
