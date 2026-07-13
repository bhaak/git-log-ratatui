use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

    let inner_area = block.inner(area);

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

use crate::app::commands::Command;
use crate::state::search::SearchState;
use crate::text_utils;
use crate::ui::panel::Panel;
use crate::ui::render_ctx::RenderCtx;

/// Wrapper struct implementing the Panel trait for the search input.
pub struct SearchPanel;

impl Panel for SearchPanel {
    type State = SearchState;

    fn render(&self, area: Rect, frame: &mut Frame, state: &mut Self::State, ctx: &RenderCtx) {
        let branch_label = ctx.selected_branch.unwrap_or("all branches");
        let title = format!("Git Log - {} [{}]", ctx.repo_path, branch_label);
        render(
            frame,
            area,
            &state.search_query,
            state.cursor_pos,
            branch_label,
            &title,
            ctx.is_focused(crate::view::Panel::Search),
            ctx.debug_label,
            ctx.theme,
        );
    }

    fn handle_event(&mut self, key: &KeyEvent, state: &mut Self::State) -> Vec<Command> {
        match key.code {
            KeyCode::Esc => vec![Command::ClearSearch],
            KeyCode::Backspace if state.cursor_pos > 0 => {
                let pos = state.cursor_pos;
                let prev = text_utils::prev_char_boundary(&state.search_query, pos);
                let mut q = state.search_query.clone();
                q.remove(prev);
                vec![Command::SetSearch(q, prev)]
            }
            KeyCode::Delete if state.cursor_pos < state.search_query.len() => {
                let pos = state.cursor_pos;
                let mut q = state.search_query.clone();
                q.remove(pos);
                vec![Command::SetSearch(q, pos)]
            }
            KeyCode::Left => {
                let new_pos = if key.modifiers.contains(KeyModifiers::CONTROL) {
                    text_utils::prev_word_boundary(&state.search_query, state.cursor_pos)
                } else {
                    text_utils::prev_char_boundary(&state.search_query, state.cursor_pos)
                };
                vec![Command::SetSearch(state.search_query.clone(), new_pos)]
            }
            KeyCode::Right => {
                let new_pos = if key.modifiers.contains(KeyModifiers::CONTROL) {
                    text_utils::next_word_boundary(&state.search_query, state.cursor_pos)
                } else {
                    text_utils::next_char_boundary(&state.search_query, state.cursor_pos)
                };
                vec![Command::SetSearch(state.search_query.clone(), new_pos)]
            }
            KeyCode::Home => {
                vec![Command::SetSearch(state.search_query.clone(), 0)]
            }
            KeyCode::End => {
                let len = state.search_query.len();
                vec![Command::SetSearch(state.search_query.clone(), len)]
            }
            KeyCode::Char(ch) => {
                let pos = state.cursor_pos;
                let mut q = state.search_query.clone();
                q.insert(pos, ch);
                vec![Command::SetSearch(q, pos + ch.len_utf8())]
            }
            _ => Vec::new(),
        }
    }

    fn help_keys(&self) -> &[(&str, &str)] {
        &[
            ("Esc", "clear search"),
            ("Ctrl+A/E", "start/end"),
            ("Ctrl+V", "paste"),
            ("Tab", "next panel"),
        ]
    }

    fn label(&self) -> &str {
        "Search"
    }
}
