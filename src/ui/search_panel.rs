use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the search panel with search input and cursor.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    search_query: &str,
    cursor_pos: usize,
    branch_label: &str,
    title: &str,
    is_focused: bool,
) {
    if area.width < 4 || area.height < 2 {
        return;
    }

    let border_style = if is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(border_style);

    // Build the inner content
    let inner_area = block.inner(area);

    // Build display text with cursor
    let display: Vec<Span> = if is_focused && inner_area.width > 2 {
        let max_visible = (inner_area.width as usize).saturating_sub(2);
        let start = if search_query.len() > max_visible {
            search_query.len() - max_visible
        } else {
            0
        };
        let visible: String = search_query.chars().skip(start).take(max_visible).collect();
        let cursor_rel = cursor_pos.saturating_sub(start).min(visible.len());

        let mut spans = Vec::new();
        for (i, ch) in visible.chars().enumerate() {
            if i == cursor_rel {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(180, 140, 255)),
                ));
            } else {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::White),
                ));
            }
        }
        // Show cursor at end if it's past the last character
        if cursor_rel >= visible.len() {
            spans.push(Span::styled(
                " ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(180, 140, 255)),
            ));
        }
        spans
    } else {
        // Not focused: show search text or placeholder
        if search_query.is_empty() {
            vec![Span::styled(
                format!("<{}>", branch_label),
                Style::default().fg(Color::DarkGray),
            )]
        } else {
            vec![Span::styled(
                search_query.to_string(),
                Style::default().fg(Color::White),
            )]
        }
    };

    let paragraph = Paragraph::new(Line::from(display)).block(block);

    frame.render_widget(paragraph, area);
}
