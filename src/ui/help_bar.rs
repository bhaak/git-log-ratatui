use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::models::Panel;
use crate::theme::Theme;

/// Render the context-sensitive help bar at the bottom of the screen.
pub fn render(frame: &mut Frame, area: Rect, focus: Panel, status: Option<&str>, theme: &Theme) {
    if area.width < 10 || area.height < 2 {
        return;
    }

    let key_style = Style::default()
        .fg(theme.help_title)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let bracket_style = Style::default().fg(theme.unfocused_border);

    let common = [("q", "Quit"), ("Tab", "Focus"), ("l/h", "Next/Prev")];

    let context: Vec<(&str, &str)> = match focus {
        Panel::Branches => vec![
            ("j/k/↑↓", "Nav"),
            ("→←", "Expand"),
            ("Space", "Toggle"),
            ("Enter", "Load"),
        ],
        Panel::Search => vec![
            ("Esc", "Clear"),
            ("Ctrl+V", "Paste"),
            ("Ctrl+A/E", "Home/End"),
        ],
        Panel::Scope => vec![("Space/Enter", "Cycle"), ("Ctrl+S", "Cycle")],
        Panel::Commits => vec![
            ("j/k/↑↓", "Nav"),
            ("y", "Copy hash 7"),
            ("Y", "Copy full"),
            ("Enter", "→ Diff"),
            ("g", "Graph toggle"),
        ],
        Panel::Diff => vec![
            ("j/k/↑↓", "File"),
            ("Enter", "Jump"),
            ("n/p", "Next/prev"),
            ("Home/End", "Top/bottom"),
        ],
    };

    let mut spans: Vec<Span> = Vec::new();

    for (key, desc) in common.iter().chain(context.iter()) {
        spans.push(Span::styled("<", bracket_style));
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(": ", bracket_style));
        spans.push(Span::styled(*desc, desc_style));
        spans.push(Span::styled(">", bracket_style));
        spans.push(Span::raw("  "));
    }

    let title = if let Some(msg) = status {
        format!(" Help | {} ", msg)
    } else {
        " Help ".to_string()
    };

    let paragraph = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.unfocused_border)),
    );

    frame.render_widget(paragraph, area);
}
