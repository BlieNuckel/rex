use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::{Block, Paragraph};

use crate::config::Config;
use crate::plex::{PlexClient, auth};

#[derive(Parser, Debug, Default)]
pub(super) struct Options {}

impl Options {
    pub fn run(self, cfg: &mut Config) -> Result<ExitCode> {
        if cfg.token.is_none() && cfg.server_url.is_none() {
            cfg.token = Some(auth::login(cfg)?);
            cfg.save()?;
        }
        println!("Connecting…");
        let client = auth::connect(cfg)?;
        run_tui(&client)?;
        Ok(ExitCode::SUCCESS)
    }
}

fn run_tui(client: &PlexClient) -> Result<()> {
    // ratatui::init installs a panic hook that restores the terminal
    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|f| {
                f.render_widget(
                    Paragraph::new(format!("{}\n\nq to quit", client.base))
                        .block(Block::bordered().title(" rex ")),
                    f.area(),
                )
            })?;
            if let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press
                && k.code == KeyCode::Char('q')
            {
                return Ok(());
            }
        }
    })();
    ratatui::restore();
    result
}
