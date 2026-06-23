use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::models::Panel;

/// Render the context-sensitive help bar at the bottom of the screen.
pub fn render(frame: &mut Frame, area: Rect, focus: Panel, commit_count_info: &str) {
    if area.width < 10 || area.height < 2 {
        return;
    }

    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let bracket_style = Style::default().fg(Color::DarkGray);

    let common = vec![("q", "Quit"), ("Tab", "Focus"), ("l/h", "Next/Prev")];

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

    spans.push(Span::styled(commit_count_info, desc_style));

    let paragraph = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Format commit count information string.
/// Shows dash when no commits, or "selected/total" or "selected/total (filtered from full)".
pub fn format_commit_count(selected: usize, visible: usize, total: usize) -> String {
    if visible == 0 {
        " - ".to_string()
    } else if visible == total {
        format!(" {}/{} ", selected + 1, visible)
    } else {
        format!(" {}/{} (filtered from {}) ", selected + 1, visible, total)
    }
}
