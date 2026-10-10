use ratatui::Frame;
use ratatui::layout::VerticalAlignment::Center;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::app::queue::Repeat;
use crate::app::state::{AppState, NowPlaying};
use crate::ui::string_formatting::fit;

pub fn draw_minimized_player(f: &mut Frame, s: &AppState, area: Rect) {
    let block = Block::bordered();
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;
    let now = &s.now;

    let first = match &now.track {
        None => fit("■ not playing", "", width),
        Some(t) => {
            let icon = if now.paused { "‖" } else { "▶" };
            let mut text = format!("{icon} {}", t.title);
            for part in [&t.grandparent_title, &t.parent_title] {
                if !part.is_empty() {
                    text.push_str(if text.contains(" — ") {
                        " · "
                    } else {
                        " — "
                    });
                    text.push_str(part);
                }
            }
            fit(&text, if now.buffering { "buffering…" } else { "" }, width)
        }
    };

    let mut right = String::new();
    let duration = now.track.as_ref().and_then(|t| t.duration_ms);
    if let Some(d) = duration {
        right.push_str(&format!(" {}", crate::fmt_ms(d)));
    }
    right.push_str(&format!("  vol {:.0}%", now.volume * 100.0));
    if s.queue.shuffle {
        right.push_str(" ⇄");
    }
    match s.queue.repeat {
        Repeat::Off => {}
        Repeat::All => right.push_str(" ↻"),
        Repeat::One => right.push_str(" ↻1"),
    }
    if s.debug_line {
        right.push_str(&format!("  rss {:.1} MB", crate::rss_kb() as f64 / 1024.0));
    }
    let left = match now.track {
        Some(_) => format!("{} ", crate::fmt_ms(now.position_ms)),
        None => String::new(),
    };
    let bar_width = width.saturating_sub(left.width() + right.width());
    let bar = match duration {
        Some(d) if d > 0 && now.track.is_some() && bar_width >= 5 => {
            let filled = ((now.position_ms.min(d) as f64 / d as f64) * bar_width as f64) as usize;
            format!("{}{}", "━".repeat(filled), "─".repeat(bar_width - filled))
        }
        _ => String::new(),
    };
    let second = fit(&format!("{left}{bar}"), right.trim_start(), width);
    f.render_widget(
        Paragraph::new(vec![Line::raw(first), Line::raw(second)]),
        inner,
    );
}

pub fn draw_maximized_player(f: &mut Frame, s: &AppState, area: Rect) {
    let block = Block::bordered().title(" Player ");
    f.render_widget(&block, area);

    match &s.now.track {
        Some(_) => draw_non_empty_state(f, block.inner(area), s),
        None => draw_empty_state(f, block.inner(area)),
    }
}

fn draw_empty_state(f: &mut Frame, area: Rect) {
    let text = "quiet for now";
    let centered_area = area.centered(
        Constraint::Length(text.width() as u16),
        Constraint::Length(1),
    );
    f.render_widget(Paragraph::new(text).centered().bold(), centered_area)
}

fn draw_non_empty_state(f: &mut Frame, area: Rect, s: &AppState) {
    let now = &s.now;
    let width = area.width as usize;
    let area_centered = area.centered_horizontally(Constraint::Length(width as u16));

    let [cover_art, artist_info, progress_bar] = Layout::vertical([
        Constraint::Min(20),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .areas(area_centered);

    draw_cover_art(f, cover_art);
    draw_info(f, artist_info, now);
    draw_progress_bar(f, progress_bar, now, s.accent);
}

fn draw_cover_art(f: &mut Frame, area: Rect) {
    let centered_art = area.centered(Constraint::Fill(1), Constraint::Length(1));
    f.render_widget(
        Paragraph::new("No cover art available").centered(),
        centered_art,
    );
}

fn draw_info(f: &mut Frame, area: Rect, now: &NowPlaying) {
    let [track_and_album, artist] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

    let (track_text, album_and_artist_text) = match &now.track {
        None => (String::new(), String::new()),
        Some(t) => {
            if now.buffering {
                (String::from(" buffering…"), String::new())
            } else {
                (
                    t.title.to_owned(),
                    format!("{} · {}", t.parent_title, t.grandparent_title.clone()),
                )
            }
        }
    };

    f.render_widget(
        Paragraph::new(track_text).centered().bold(),
        track_and_album,
    );
    f.render_widget(Paragraph::new(album_and_artist_text).centered(), artist);
}

fn draw_progress_bar(f: &mut Frame, area: Rect, now: &NowPlaying, accent: Color) {
    let padded_area = area.inner(Margin::new(10, 0));
    let [time_stamp, _, bar] = Layout::horizontal([
        Constraint::Length(11),
        Constraint::Length(1),
        Constraint::Min(10),
    ])
    .areas(padded_area);

    let width = bar.width as usize;
    let duration = now.track.as_ref().and_then(|t| t.duration_ms);

    let bar_spans = match duration {
        Some(d) if d > 0 && now.track.is_some() && width >= 5 => {
            let filled = ((now.position_ms.min(d) as f64 / d as f64) * width as f64) as usize;
            vec![
                Span::styled("━".repeat(filled), Style::new().fg(accent)),
                Span::raw("━".repeat(width - filled)),
            ]
        }
        _ => vec![],
    };
    let time = format!(
        "{} / {}",
        crate::fmt_ms(now.position_ms),
        now.track
            .as_ref()
            .and_then(|t| t.duration_ms)
            .map_or(crate::fmt_ms(0), crate::fmt_ms)
    );
    f.render_widget(time, time_stamp);
    f.render_widget(Line::from(bar_spans), bar);
}
