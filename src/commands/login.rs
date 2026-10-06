use std::process::ExitCode;

use clap::Parser;

use crate::config::Config;
use crate::plex::auth;

#[derive(Parser, Debug)]
pub(super) struct Options {}

impl Options {
    pub fn run(self, cfg: &mut Config) -> anyhow::Result<ExitCode> {
        cfg.token = Some(auth::login(cfg)?);
        cfg.server_url = None;
        cfg.server_id = None;
        cfg.server_token = None;
        cfg.music_section = None;
        cfg.save()?;
        let client = auth::connect(cfg)?;
        println!("Signed in. Using {}.", client.base);
        Ok(ExitCode::SUCCESS)
    }
}
