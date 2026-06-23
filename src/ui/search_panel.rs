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

    // Build display text with cursor and "Search: " prefix
    let prefix = " Search: ";
    let display: Vec<Span> = if is_focused && inner_area.width > 2 {
        let prefix_len = prefix.chars().count();
        let max_visible = (inner_area.width as usize).saturating_sub(2).saturating_sub(prefix_len);
        let char_count = search_query.chars().count();
        let char_start = if char_count > max_visible && max_visible > 0 {
            char_count - max_visible
        } else {
            0
        };
        let visible: String = search_query
            .chars()
            .skip(char_start)
            .take(max_visible.max(1))
            .collect();
        let visible_char_count = visible.chars().count();
        let cursor_char_pos = search_query[..cursor_pos.min(search_query.len())]
            .chars()
            .count();
        let cursor_rel = cursor_char_pos.saturating_sub(char_start);

        let mut spans = vec![Span::styled(
            prefix,
            Style::default().fg(Color::DarkGray),
        )];
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
        if cursor_rel >= visible_char_count {
            spans.push(Span::styled(
                " ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(180, 140, 255)),
            ));
        }
        spans
    } else {
        if search_query.is_empty() {
            vec![Span::styled(
                format!("{}<{}>", prefix, branch_label),
                Style::default().fg(Color::DarkGray),
            )]
        } else {
            vec![Span::styled(
                format!("{}{}", prefix, search_query),
                Style::default().fg(Color::White),
            )]
        }
    };

    let paragraph = Paragraph::new(Line::from(display)).block(block);

    frame.render_widget(paragraph, area);
}
