use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListState},
    Frame,
};

use crate::app::state::AppState;
use crate::models::*;
use crate::ui::panel::Panel;

/// Wrapper struct implementing the Panel trait for the branch tree view.
pub struct BranchPanel;

impl Panel for BranchPanel {
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
