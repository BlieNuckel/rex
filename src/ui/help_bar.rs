use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::keys::{BINDINGS, key_name};
use crate::app::state::AppState;
use crate::ui::string_formatting::truncate;

fn draw_message(f: &mut Frame, s: &AppState, area: Rect) {
    if let Some(m) = &s.message {
        let text = truncate(&m.text, area.width as usize);
        let style = if m.error {
            Style::new().fg(Color::Red)
        } else {
            Style::new()
        };
        f.render_widget(Paragraph::new(text).style(style), area);
    }
}

pub fn draw_help_bar(f: &mut Frame, s: &AppState, area: Rect) {
    if s.message.is_some() {
        return draw_message(f, s, area);
    }

    let key_style = Style::new().fg(s.accent).add_modifier(Modifier::BOLD);
    let spans: Vec<Span> = s
        .get_supported_actions()
        .iter()
        .filter_map(|&action| {
            BINDINGS
                .iter()
                .filter(|b| b.show_in_help_bar)
                .find(|b| b.action == action)
                .map(|b| (b, action))
        })
        .flat_map(|(b, action)| {
            [
                Span::styled(key_name(b), key_style),
                Span::raw(format!(" {}   ", action.describe_short())),
            ]
        })
        .collect();
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
