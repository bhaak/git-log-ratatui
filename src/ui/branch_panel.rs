use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListState},
    Frame,
};

use crate::app::commands::Command;
use crate::state::branch::BranchState;
use crate::theme::Theme;
use crate::ui::layout;
use crate::ui::panel::{KeyBinding, Panel};
use crate::ui::render_ctx::RenderCtx;
use crate::ui::scrollbar_view::ScrollbarView;
use crate::view::TreeItem;

/// Wrapper struct implementing the Panel trait for the branch tree view.
pub struct BranchPanel;

impl Panel for BranchPanel {
    type State = BranchState;

    fn render(&self, area: Rect, frame: &mut Frame, state: &mut Self::State, ctx: &RenderCtx) {
        let (content_area, scrollbar_area) = ScrollbarView::split(area);

        let list_state = render_tree(
            frame,
            content_area,
            &state.branch_tree,
            state.branch_index,
            state.branch_staleness_enabled,
            ctx.is_focused(crate::view::Panel::Branches),
            ctx.debug_label,
            ctx.theme,
        );

        state.branch_list_offset = list_state.offset();

        let focus_style = if ctx.is_focused(crate::view::Panel::Branches) {
            Style::default().fg(ctx.theme.focused_border)
        } else {
            Style::default().fg(ctx.theme.unfocused_border)
        };
        let visible = (content_area.height.saturating_sub(layout::PANEL_BORDER_H)) as usize;
        state.scrollbar.render(
            frame,
            scrollbar_area,
            state.branch_tree.len(),
            visible,
            list_state.offset(),
            focus_style,
        );
    }

    fn handle_event(&mut self, key: &KeyEvent, state: &mut Self::State) -> Vec<Command> {
        match key.code {
            KeyCode::Up => vec![Command::MoveUp],
            KeyCode::Down => vec![Command::MoveDown],
            KeyCode::Right => {
                if let Some(item) = state.branch_tree.get(state.branch_index) {
                    if item.expandable && !item.expanded {
                        return vec![Command::ToggleBranchNode {
                            key: item.key.clone(),
                            expanded: true,
                        }];
                    }
                }
                Vec::new()
            }
            KeyCode::Left => {
                if let Some(item) = state.branch_tree.get(state.branch_index) {
                    if item.expandable && item.expanded {
                        return vec![Command::ToggleBranchNode {
                            key: item.key.clone(),
                            expanded: false,
                        }];
                    }
                }
                Vec::new()
            }
            KeyCode::Char(' ') => {
                if let Some(item) = state.branch_tree.get(state.branch_index) {
                    if item.expandable {
                        return vec![Command::ToggleBranchNode {
                            key: item.key.clone(),
                            expanded: !item.expanded,
                        }];
                    }
                }
                Vec::new()
            }
            KeyCode::Char('n') => vec![Command::JumpToSibling(1)],
            KeyCode::Char('p') => vec![Command::JumpToSibling(-1)],
            KeyCode::Enter => {
                if let Some(item) = state.branch_tree.get(state.branch_index) {
                    if item.is_branch {
                        return vec![Command::SelectBranch(item.full_path.clone())];
                    } else if item.expandable {
                        return vec![Command::ToggleBranchNode {
                            key: item.key.clone(),
                            expanded: !item.expanded,
                        }];
                    }
                }
                Vec::new()
            }
            KeyCode::PageUp => vec![Command::PageUp],
            KeyCode::PageDown => vec![Command::PageDown],
            KeyCode::Home => vec![Command::JumpToTop],
            KeyCode::End => vec![Command::JumpToBottom],
            _ => Vec::new(),
        }
    }

    fn help_keys(&self) -> &'static [KeyBinding] {
        static KEYS: &[KeyBinding] = &[
            KeyBinding::new("↑↓/j,k", "navigate", "Navigate up / down"),
            KeyBinding::new("Enter", "select", "Select branch (load commits)"),
            KeyBinding::new("Space", "toggle", "Toggle expand / collapse"),
            KeyBinding::new("n/p", "sibling", "Next / previous sibling"),
            KeyBinding::new("PgUp/PgDn", "page", "Page up / down"),
            KeyBinding::new("Home/End", "top/bottom", "Jump to top / bottom"),
        ];
        KEYS
    }

    fn label(&self) -> &str {
        "Branches"
    }
}

/// Render the branch panel content with a hierarchical tree view.
/// Returns the ListState for offset tracking.
#[allow(clippy::too_many_arguments)]
fn render_tree(
    frame: &mut Frame,
    area: Rect,
    tree_items: &[TreeItem],
    selected_index: usize,
    staleness_enabled: bool,
    is_focused: bool,
    debug_label: Option<&str>,
    theme: &Theme,
) -> ListState {
    if area.width < 4 || area.height < 2 {
        return ListState::default();
    }

    let border_style = if is_focused {
        Style::default().fg(theme.focused_border)
    } else {
        Style::default().fg(theme.unfocused_border)
    };

    let item_dates: Vec<i64> = tree_items
        .iter()
        .filter_map(|item| item.epoch_days)
        .collect();
    let min_days = item_dates.iter().min().copied().unwrap_or(0);
    let max_days = crate::ui::commit_table::current_epoch_days();

    let items: Vec<Line> = tree_items
        .iter()
        .map(|item| {
            let color = if staleness_enabled {
                item.epoch_days
                    .map(|d| crate::ui::commit_table::age_color(d, min_days, max_days))
                    .unwrap_or(theme.commit_secondary)
            } else {
                theme.commit_secondary
            };
            Line::from(Span::styled(item.name.clone(), Style::default().fg(color)))
        })
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
                    theme.selected_bg
                } else {
                    theme.unselected_bg
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
