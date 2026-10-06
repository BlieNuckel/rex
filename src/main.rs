mod config;

use anyhow::{Result, bail};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::{Block, Paragraph};
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

use config::Config;

struct Args {
    log_level: Level,
    command: Vec<String>,
}

fn parse_args() -> Result<Args> {
    use lexopt::prelude::*;
    let mut args = Args {
        log_level: Level::INFO,
        command: Vec::new(),
    };
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            Long("log-level") => args.log_level = parser.value()?.string()?.parse()?,
            Long("help") | Short('h') => {
                println!("usage: rex [--log-level LEVEL] [login | logout | debug <cmd> ...]");
                std::process::exit(0);
            }
            Value(v) => args.command.push(v.string()?),
            _ => return Err(arg.unexpected().into()),
        }
    }
    Ok(args)
}

fn init_logging(level: Level) -> Result<()> {
    let dir = config::state_dir()?;
    std::fs::create_dir_all(&dir)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("rex")
        .filename_suffix("log")
        .max_log_files(3)
        .build(dir)?;
    tracing_subscriber::fmt()
        .with_writer(appender)
        .with_ansi(false)
        .with_max_level(level)
        .init();
    Ok(())
}

fn main() -> Result<()> {
    let args = parse_args()?;
    init_logging(args.log_level)?;
    let mut cfg = Config::load()?;
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting");

    match args.command.first().map(String::as_str) {
        None => run_tui(&mut cfg),
        Some("logout") => {
            cfg.token = None;
            cfg.server_token = None;
            cfg.save()?;
            println!("Logged out.");
            Ok(())
        }
        Some(cmd) => bail!("unknown command: {cmd}"),
    }
}

fn run_tui(_cfg: &mut Config) -> Result<()> {
    // ratatui::init installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        loop {
            terminal.draw(|f| {
                f.render_widget(
                    Paragraph::new("q to quit").block(Block::bordered().title(" rex ")),
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
