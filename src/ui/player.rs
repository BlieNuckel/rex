use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::app::queue::Repeat;
use crate::app::state::AppState;
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
    let now = &s.now;
    let block = Block::bordered();
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;
    let inner_centered = inner.centered_horizontally(Constraint::Length(width as u16));

    let [cover_art, artist_info, progress_bar] = Layout::vertical([
        Constraint::Min(20),
        Constraint::Length(5),
        Constraint::Length(2),
    ])
    .areas(inner_centered);

    let first = match &s.now.track {
        None => "■ not playing".to_string(),
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
            if now.buffering {
                " buffering…".to_string()
            } else {
                text
            }
        }
    };

    f.render_widget(Paragraph::new(first).centered(), artist_info);
}
