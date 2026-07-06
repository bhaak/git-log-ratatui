use crossterm::event::{Event, KeyCode};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListState},
    Frame,
};

use crate::app::branches;
use crate::app::state::AppState;
use crate::app::PAGE_SIZE;
use crate::models::*;
use crate::ui::panel::{self as panel_mod, EventOutcome};

/// Wrapper struct implementing the Panel trait for the branch tree view.
pub struct BranchPanel;

impl panel_mod::Panel for BranchPanel {
    fn render(&self, area: Rect, frame: &mut Frame, state: &AppState, is_focused: bool) {
        let _ = render(
            frame,
            area,
            &state.branch_tree,
            state.branch_index,
            is_focused,
            None,
        );
    }

    fn handle_event(&mut self, event: &Event, state: &mut AppState) -> EventOutcome {
        let Event::Key(key) = event else {
            return EventOutcome::Continue;
        };
        match key.code {
            KeyCode::Up => {
                state.branch_index = state.branch_index.saturating_sub(1);
            }
            KeyCode::Down if state.branch_index + 1 < state.branch_tree.len() => {
                state.branch_index += 1;
            }
            KeyCode::Right => {
                let action = state
                    .branch_tree
                    .get(state.branch_index)
                    .filter(|item| item.expandable && !item.expanded)
                    .map(|item| item.key.clone());
                if let Some(k) = action {
                    state.expanded_nodes.insert(k, true);
                    branches::rebuild_branch_tree(state);
                }
            }
            KeyCode::Left => {
                let action = state
                    .branch_tree
                    .get(state.branch_index)
                    .filter(|item| item.expandable && item.expanded)
                    .map(|item| item.key.clone());
                if let Some(k) = action {
                    state.expanded_nodes.insert(k, false);
                    branches::rebuild_branch_tree(state);
                }
            }
            KeyCode::Char(' ') => {
                let action = state
                    .branch_tree
                    .get(state.branch_index)
                    .filter(|item| item.expandable)
                    .map(|item| (item.key.clone(), !item.expanded));
                if let Some((k, new_state)) = action {
                    state.expanded_nodes.insert(k, new_state);
                    branches::rebuild_branch_tree(state);
                }
            }
            // Enter is handled at the App level (requires worker access)
            KeyCode::Enter => {
                let action = state.branch_tree.get(state.branch_index).map(|item| {
                    if item.expandable {
                        Some((item.key.clone(), !item.expanded))
                    } else {
                        None
                    }
                });
                if let Some(Some((k, new_state))) = action {
                    state.expanded_nodes.insert(k, new_state);
                    branches::rebuild_branch_tree(state);
                }
            }
            KeyCode::PageUp => {
                state.branch_index = state.branch_index.saturating_sub(PAGE_SIZE);
            }
            KeyCode::PageDown => {
                state.branch_index =
                    (state.branch_index + PAGE_SIZE).min(state.branch_tree.len().saturating_sub(1));
            }
            _ => {}
        }
        EventOutcome::Continue
    }

    fn help_keys(&self) -> &[(&str, &str)] {
        &[
            ("↑↓", "navigate"),
            ("Enter", "select"),
            ("Space", "toggle"),
            ("PgUp/PgDn", "page"),
        ]
    }

    fn label(&self) -> &str {
        "Branches"
    }
}

/// Render the branch panel with a hierarchical tree view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    tree_items: &[TreeItem],
    selected_index: usize,
    is_focused: bool,
    debug_label: Option<&str>,
) -> ListState {
    if area.width < 4 || area.height < 2 {
        return ListState::default();
    }

    let border_style = if is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    let items: Vec<String> = tree_items
        .iter()
        .map(|item| item.name.to_string())
        .collect();

    let title = if let Some(label) = debug_label {
        format!(" Branches [{}] ", label)
    } else {
        " Branches ".to_string()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(if is_focused {
                    Color::Rgb(80, 60, 120)
                } else {
                    Color::DarkGray
                })
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    if !tree_items.is_empty() {
        state.select(Some(selected_index.min(tree_items.len() - 1)));
    }

    frame.render_stateful_widget(list, area, &mut state);

    state
}
