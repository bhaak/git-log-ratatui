use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::theme::Theme;

/// Render the context-sensitive help bar at the bottom of the screen.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    panel_label: &str,
    context_keys: &[(&str, &str)],
    status: Option<&str>,
    theme: &Theme,
) {
    if area.width < 10 || area.height < 2 {
        return;
    }

    let key_style = Style::default()
        .fg(theme.help_title)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let bracket_style = Style::default().fg(theme.unfocused_border);

    let common = [("q", "Quit"), ("Tab", "Focus"), ("l/h", "Next/Prev")];

    let mut spans: Vec<Span> = Vec::new();

    for (key, desc) in common.iter().chain(context_keys.iter()) {
        spans.push(Span::styled("<", bracket_style));
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(": ", bracket_style));
        spans.push(Span::styled(*desc, desc_style));
        spans.push(Span::styled(">", bracket_style));
        spans.push(Span::raw("  "));
    }

    let title = if let Some(msg) = status {
        format!(" {} | {} ", panel_label, msg)
    } else {
        format!(" {} ", panel_label)
    };

    let paragraph = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.unfocused_border)),
    );

    frame.render_widget(paragraph, area);
}
