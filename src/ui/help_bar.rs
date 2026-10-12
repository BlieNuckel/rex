use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::keys::{BINDINGS, key_name};
use crate::app::state::AppState;

pub fn draw_help_bar(f: &mut Frame, s: &AppState, area: Rect) {
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
