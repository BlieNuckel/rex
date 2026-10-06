use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::keys::{BINDINGS, key_name};
use crate::app::state::{AppState, Focus, Items, ListKind, SECTIONS, View};

const SIDEBAR_WIDTH: u16 = 19;

pub fn draw(f: &mut Frame, s: &mut AppState) {
    let msg_height = u16::from(s.message.is_some());
    let [main, msg, bar] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(msg_height),
        Constraint::Length(4),
    ])
    .areas(f.area());
    let [side, list] =
        Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(10)]).areas(main);

    draw_sidebar(f, s, side);
    draw_list(f, s, list);
    if let Some((m, _)) = &s.message {
        let text = truncate(m, msg.width as usize);
        f.render_widget(Paragraph::new(text).style(Style::new().fg(Color::Red)), msg);
    }
    draw_now_playing(f, s, bar);
    if s.help {
        draw_help(f);
    }
}

fn selected_style(focused: bool) -> Style {
    if focused {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        Style::new().add_modifier(Modifier::BOLD)
    }
}

fn draw_sidebar(f: &mut Frame, s: &AppState, area: Rect) {
    let block = Block::bordered().title(" Library ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;
    let lines: Vec<Line> = SECTIONS
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let selected = i == s.section;
            let text = format!("{}{name}", if selected { "> " } else { "  " });
            let line = Line::raw(fit(&text, "", width));
            if selected {
                line.style(selected_style(s.focus == Focus::Sidebar))
            } else {
                line
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn breadcrumb(stack: &[View]) -> String {
    stack
        .iter()
        .map(|v| v.title.as_str())
        .collect::<Vec<_>>()
        .join(" › ")
}

fn draw_list(f: &mut Frame, s: &mut AppState, area: Rect) {
    let title_width = (area.width as usize).saturating_sub(16);
    let crumb = truncate_left(&breadcrumb(&s.stack), title_width);
    let focused = s.focus == Focus::List;
    let Some(v) = s.stack.last_mut() else { return };

    let count = match (v.total, v.loading) {
        (Some(t), _) if v.items.len() < t as usize => format!(" {}/{t} ", v.items.len()),
        (Some(t), _) => format!(" {t} "),
        (None, true) => " … ".to_owned(),
        (None, false) => String::new(),
    };
    let block = Block::bordered()
        .title(format!(" {crumb} "))
        .title_top(Line::raw(count).right_aligned());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    let width = inner.width as usize;
    s.list_height = height;
    let len = v.items.len();
    if len == 0 {
        let hint = if v.loading {
            "loading…"
        } else {
            match v.kind {
                ListKind::Search => "search arrives in a later milestone",
                ListKind::Queue => "queue is empty",
                _ => "nothing here",
            }
        };
        f.render_widget(Paragraph::new(format!("  {hint}")), inner);
        return;
    }

    if v.selected < v.offset {
        v.offset = v.selected;
    } else if v.selected >= v.offset + height {
        v.offset = v.selected + 1 - height;
    }
    let end = (v.offset + height).min(len);
    let lines: Vec<Line> = (v.offset..end)
        .map(|i| {
            let (left, right) = row(v, i);
            let marker = if i == v.selected { "> " } else { "  " };
            let line = Line::raw(fit(&format!("{marker}{left}"), &right, width));
            if i == v.selected {
                line.style(selected_style(focused))
            } else {
                line
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn row(v: &View, i: usize) -> (String, String) {
    match &v.items {
        Items::Artists(a) => (a[i].title.clone(), String::new()),
        Items::Albums(a) => {
            let a = &a[i];
            let year = a.year.map_or("    ".to_owned(), |y| y.to_string());
            let left = match v.kind {
                ListKind::ArtistAlbums(_) => format!("{year}  {}", a.title),
                _ => format!("{year}  {} — {}", a.title, a.parent_title),
            };
            let right = a
                .leaf_count
                .map_or(String::new(), |n| format!("{n} tracks"));
            (left, right)
        }
        Items::Tracks(t) => {
            let t = &t[i];
            let left = match v.kind {
                ListKind::AlbumTracks(_) => {
                    let n = t.index.map_or(String::new(), |n| n.to_string());
                    format!("{n:>2}  {}", t.title)
                }
                _ => format!("{} — {}", t.title, t.grandparent_title),
            };
            (left, t.duration_ms.map_or(String::new(), crate::fmt_ms))
        }
        Items::Playlists(p) => {
            let p = &p[i];
            let count = p.leaf_count.map(|n| format!("{n} tracks"));
            let dur = p.duration_ms.map(crate::fmt_ms);
            let right = [count, dur].into_iter().flatten().collect::<Vec<_>>();
            (p.title.clone(), right.join(" · "))
        }
    }
}

fn draw_now_playing(f: &mut Frame, s: &AppState, area: Rect) {
    let block = Block::bordered();
    let inner = block.inner(area);
    f.render_widget(block, area);
    let width = inner.width as usize;
    let debug = if s.debug_line {
        format!("rss {:.1} MB", crate::rss_kb() as f64 / 1024.0)
    } else {
        String::new()
    };
    let lines = vec![
        Line::raw(fit("■ not playing", "", width)),
        Line::raw(fit("", &debug, width)),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_help(f: &mut Frame) {
    let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
    for b in BINDINGS {
        let desc = b.action.describe();
        match groups.iter_mut().find(|(d, _)| *d == desc) {
            Some((_, keys)) => keys.push(key_name(b)),
            None => groups.push((desc, vec![key_name(b)])),
        }
    }
    let keys: Vec<String> = groups.iter().map(|(_, k)| k.join(" ")).collect();
    let key_width = keys.iter().map(|k| k.width()).max().unwrap_or(0);

    let area = f.area();
    let width = 64.min(area.width);
    let height = (groups.len() as u16 + 2).min(area.height);
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let block = Block::bordered().title(" Keys — ? to close ");
    let inner = block.inner(popup);
    let lines: Vec<Line> = groups
        .iter()
        .zip(&keys)
        .map(|((desc, _), k)| {
            let pad = " ".repeat(key_width - k.width() + 2);
            Line::raw(truncate(&format!("{k}{pad}{desc}"), inner.width as usize))
        })
        .collect();
    f.render_widget(Clear, popup);
    f.render_widget(block, popup);
    f.render_widget(Paragraph::new(lines), inner);
}

/// `left` truncated with an ellipsis, then `right` flush against the right edge
fn fit(left: &str, right: &str, width: usize) -> String {
    let rw = right.width();
    if rw + 2 > width {
        return truncate(left, width);
    }
    let l = truncate(left, width - rw - 1);
    let pad = width - l.width() - rw;
    format!("{l}{}{right}", " ".repeat(pad))
}

fn truncate(s: &str, width: usize) -> String {
    if s.width() <= width {
        return s.to_owned();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > width {
            break;
        }
        out.push(c);
        w += cw;
    }
    if width > 0 {
        out.push('…');
    }
    out
}

fn truncate_left(s: &str, width: usize) -> String {
    if s.width() <= width {
        return s.to_owned();
    }
    let mut out: Vec<char> = Vec::new();
    let mut w = 0;
    for c in s.chars().rev() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > width {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out.iter().rev().collect()
}
