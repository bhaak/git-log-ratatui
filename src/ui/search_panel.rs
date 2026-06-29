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
        let max_visible = (inner_area.width as usize)
            .saturating_sub(2)
            .saturating_sub(prefix_len);
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

        let mut spans = vec![Span::styled(prefix, Style::default().fg(Color::DarkGray))];
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

// --- Cursor navigation helpers ---

/// Move to the previous char boundary (for single-step left).
pub(crate) fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    for (i, _) in s.char_indices() {
        if i >= pos {
            break;
        }
    }
    for i in (0..pos).rev() {
        if s.is_char_boundary(i) {
            return i;
        }
    }
    0
}

/// Move to the next char boundary (for single-step right).
pub(crate) fn next_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    for (i, _) in s.char_indices().skip(1) {
        if i > pos {
            return i;
        }
    }
    s.len()
}

pub(crate) fn prev_word_boundary(s: &str, pos: usize) -> usize {
    let pos = prev_char_boundary(s, pos);
    if pos == 0 {
        return 0;
    }
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let char_pos = chars
        .iter()
        .position(|&(i, _)| i == pos)
        .unwrap_or(chars.len());
    if char_pos == 0 {
        return 0;
    }
    let mut idx = char_pos - 1;
    loop {
        let (_bi, ch) = chars.get(idx).copied().unwrap_or((0, '\0'));
        if ch.is_alphanumeric() || ch == '_' {
            if idx == 0 {
                return 0;
            }
            idx = idx.saturating_sub(1);
        } else {
            return chars.get(idx + 1).map(|&(i, _)| i).unwrap_or(0);
        }
    }
}

pub(crate) fn next_word_boundary(s: &str, pos: usize) -> usize {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let char_pos = chars
        .iter()
        .position(|&(i, _)| i >= pos)
        .unwrap_or(chars.len());
    let mut idx = char_pos;
    while idx < chars.len() {
        let (_, ch) = chars[idx];
        if ch.is_alphanumeric() || ch == '_' {
            idx += 1;
        } else {
            break;
        }
    }
    while idx < chars.len() {
        let (_, ch) = chars[idx];
        if !ch.is_alphanumeric() && ch != '_' {
            idx += 1;
        } else {
            break;
        }
    }
    chars.get(idx).map(|&(i, _)| i).unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prev_char_boundary_ascii() {
        assert_eq!(prev_char_boundary("hello", 3), 2);
        assert_eq!(prev_char_boundary("hello", 0), 0);
    }

    #[test]
    fn test_prev_char_boundary_utf8() {
        assert_eq!(prev_char_boundary("Mäller", 3), 1);
        assert_eq!(prev_char_boundary("Mäller", 2), 1);
    }

    #[test]
    fn test_next_char_boundary_ascii() {
        assert_eq!(next_char_boundary("hello", 2), 3);
        assert_eq!(next_char_boundary("hello", 5), 5);
    }

    #[test]
    fn test_next_char_boundary_utf8() {
        assert_eq!(next_char_boundary("Mäller", 1), 3);
        assert_eq!(next_char_boundary("Mäller", 3), 4);
    }

    #[test]
    fn test_prev_word_boundary() {
        assert_eq!(prev_word_boundary("hello world", 6), 0);
        assert_eq!(prev_word_boundary("foo bar", 6), 4);
    }

    #[test]
    fn test_next_word_boundary() {
        assert_eq!(next_word_boundary("hello world", 0), 6);
        assert_eq!(next_word_boundary("hello world", 5), 6);
        assert_eq!(next_word_boundary("hello", 0), 5);
    }
}
