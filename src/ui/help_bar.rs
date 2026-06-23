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
    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let bracket_style = Style::default().fg(Color::DarkGray);

    let common = vec![
        ("q", "Quit"),
        ("Tab", "Focus"),
    ];

    let context: Vec<(&str, &str)> = match focus {
        Panel::Branches => vec![
            ("\u{2191}\u{2193}", "Nav"),
            ("\u{2192}\u{2190}", "Expand"),
            ("Space", "Toggle"),
            ("Enter", "Load"),
        ],
        Panel::Search => vec![
            ("Esc", "Clear"),
            ("Ctrl+V", "Paste"),
        ],
        Panel::Scope => vec![
            ("Ctrl+S", "Cycle"),
        ],
        Panel::Commits => vec![
            ("\u{2191}\u{2193}", "Nav"),
            ("y", "Copy hash 7"),
            ("Y", "Copy full"),
        ],
        Panel::Diff => vec![
            ("\u{2191}\u{2193}", "File"),
            ("Enter", "Jump"),
            ("n/p", "Next/prev"),
            ("Home/End", "Top/bottom"),
        ],
    };

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(commit_count_info, desc_style));
    spans.push(Span::raw("  "));

    for (key, desc) in common.iter().chain(context.iter()) {
        spans.push(Span::styled("<", bracket_style));
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(": ", bracket_style));
        spans.push(Span::styled(*desc, desc_style));
        spans.push(Span::styled(">", bracket_style));
        spans.push(Span::raw("  "));
    }

    let paragraph = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(paragraph, area);
}

/// Format commit count information string.
pub fn format_commit_count(selected: usize, filtered: usize, total: usize) -> String {
    format!("{}/{} (filtered from {})", selected + 1, filtered, total)
}
