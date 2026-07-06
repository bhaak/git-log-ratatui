use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::theme::Theme;

/// Render the search panel with search input and cursor.
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    search_query: &str,
    cursor_pos: usize,
    branch_label: &str,
    title: &str,
    is_focused: bool,
    debug_label: Option<&str>,
    theme: &Theme,
) {
    if area.width < 4 || area.height < 2 {
        return;
    }

    let border_style = if is_focused {
        Style::default().fg(theme.focused_border)
    } else {
        Style::default().fg(theme.unfocused_border)
    };

    let full_title = if let Some(label) = debug_label {
        format!(" {} [{}] ", title, label)
    } else {
        format!(" {} ", title)
    };

    let block = Block::default()
        .title(full_title)
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

        let mut spans = vec![Span::styled(
            prefix,
            Style::default().fg(theme.unfocused_border),
        )];
        for (i, ch) in visible.chars().enumerate() {
            if i == cursor_rel {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default()
                        .fg(theme.search_cursor_fg)
                        .bg(theme.search_cursor_bg),
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
                    .fg(theme.search_cursor_fg)
                    .bg(theme.search_cursor_bg),
            ));
        }
        spans
    } else {
        if search_query.is_empty() {
            vec![Span::styled(
                format!("{}<{}>", prefix, branch_label),
                Style::default().fg(theme.unfocused_border),
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

use crate::app::search;
use crate::app::state::AppState;
use crate::text_utils;
use crate::ui::panel::{EventOutcome, Panel};

/// Wrapper struct implementing the Panel trait for the search input.
pub struct SearchPanel;

impl Panel for SearchPanel {
    fn render(&self, area: Rect, frame: &mut Frame, state: &AppState, is_focused: bool) {
        let branch_label = state.selected_branch.as_deref().unwrap_or("all branches");
        let title = format!("Git Log - {} [{}]", state.repo_path, branch_label);
        render(
            frame,
            area,
            &state.search_query,
            state.cursor_pos,
            branch_label,
            &title,
            is_focused,
            None,
            &state.theme,
        );
    }

    fn handle_event(&mut self, event: &Event, state: &mut AppState) -> EventOutcome {
        let Event::Key(key) = event else {
            return EventOutcome::Continue;
        };
        match key.code {
            KeyCode::Esc => {
                state.search_query.clear();
                state.cursor_pos = 0;
                search::apply_search_filter(state);
            }
            KeyCode::Backspace if state.cursor_pos > 0 => {
                let prev = text_utils::prev_char_boundary(&state.search_query, state.cursor_pos);
                state.search_query.remove(prev);
                state.cursor_pos = prev;
                search::apply_search_filter(state);
            }
            KeyCode::Delete if state.cursor_pos < state.search_query.len() => {
                let pos = state.cursor_pos;
                state.search_query.remove(pos);
                search::apply_search_filter(state);
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    state.cursor_pos =
                        text_utils::prev_word_boundary(&state.search_query, state.cursor_pos);
                } else {
                    state.cursor_pos =
                        text_utils::prev_char_boundary(&state.search_query, state.cursor_pos);
                }
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    state.cursor_pos =
                        text_utils::next_word_boundary(&state.search_query, state.cursor_pos);
                } else {
                    state.cursor_pos =
                        text_utils::next_char_boundary(&state.search_query, state.cursor_pos);
                }
            }
            KeyCode::Home => {
                state.cursor_pos = 0;
            }
            KeyCode::End => {
                state.cursor_pos = state.search_query.len();
            }
            KeyCode::Char(ch) => {
                let pos = state.cursor_pos;
                state.search_query.insert(pos, ch);
                state.cursor_pos += ch.len_utf8();
                search::apply_search_filter(state);
            }
            _ => {}
        }
        EventOutcome::Continue
    }

    fn help_keys(&self) -> &[(&str, &str)] {
        &[
            ("Ctrl+A/E", "start/end"),
            ("Esc", "clear"),
            ("Ctrl+V", "paste"),
        ]
    }

    fn label(&self) -> &str {
        "Search"
    }
}
