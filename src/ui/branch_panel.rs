use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListState},
    Frame,
};

use crate::models::*;

/// Render the branch panel with a hierarchical tree view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    tree_items: &[TreeItem],
    selected_index: usize,
    is_focused: bool,
) -> ListState {
    let border_style = if is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    let items: Vec<String> = tree_items
        .iter()
        .map(|item| {
            format!(
                "{}",
                item.name
            )
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Branches ")
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
