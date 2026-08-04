use std::time::Instant;

use crate::app::commands::{Command, Effect};
use crate::ui;
use crate::view::Panel;

/// Global UI state — focus, layout, drag tracking, and status.
pub struct UiState {
    pub focus: Panel,
    /// LIFO stack for GoBack / Esc navigation (pushed when Enter navigates away).
    pub focus_stack: Vec<Panel>,
    pub branch_width_pct: u16,
    pub diff_height_pct: u16,
    pub dragging: Option<ui::layout::DragDirection>,
    pub scrollbar_drag: Option<Panel>,
    pub last_size: Option<(u16, u16)>,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub status_message: Option<String>,
    pub status_expiry: Option<Instant>,
    pub poll_interval_ms: u8,
    pub dirty: bool,
    pub needs_terminal_reset: bool,
    /// Whether the help modal is currently visible (toggled by `?`).
    pub help_visible: bool,
    /// When Some, the given panel is displayed fullscreen (toggled by `f`).
    pub fullscreen: Option<Panel>,
}

impl UiState {
    pub fn new(branch_width_pct: u16, diff_height_pct: u16, poll_min_ms: u8) -> Self {
        UiState {
            focus: Panel::Commits,
            focus_stack: Vec::new(),
            branch_width_pct,
            diff_height_pct,
            dragging: None,
            scrollbar_drag: None,
            last_size: None,
            last_mouse_pos: None,
            status_message: None,
            status_expiry: None,
            poll_interval_ms: poll_min_ms,
            dirty: true,
            needs_terminal_reset: false,
            help_visible: false,
            fullscreen: None,
        }
    }

    /// Push the given panel onto the focus stack before navigating away.
    pub(crate) fn push_focus(&mut self, panel: Panel) {
        self.focus_stack.push(panel);
    }

    /// Handle UI-level commands (focus, drag, layout).
    pub(crate) fn handle_command(&mut self, cmd: &Command) -> Vec<Effect> {
        match cmd {
            Command::SetFocus(panel) => {
                self.focus = *panel;
                vec![Effect::SetDirty]
            }
            Command::FocusNext => {
                self.focus = self.focus.next();
                vec![Effect::SetDirty]
            }
            Command::FocusPrev => {
                self.focus = self.focus.prev();
                vec![Effect::SetDirty]
            }
            Command::GoBack => {
                if let Some(prev) = self.focus_stack.pop() {
                    self.focus = prev;
                }
                vec![Effect::SetDirty]
            }
            Command::FocusSearchClear => {
                self.focus = Panel::Search;
                vec![Effect::SetDirty]
            }
            Command::ToggleHelp => {
                self.help_visible = !self.help_visible;
                vec![Effect::SetDirty]
            }
            Command::ToggleFullscreen => {
                self.help_visible = false;
                match self.fullscreen {
                    Some(_) => self.fullscreen = None,
                    None => self.fullscreen = Some(self.focus),
                }
                vec![Effect::SetDirty]
            }
            Command::InitiateDragVertical => {
                self.dragging = Some(ui::layout::DragDirection::Vertical);
                vec![]
            }
            Command::InitiateDragHorizontal => {
                self.dragging = Some(ui::layout::DragDirection::Horizontal);
                vec![]
            }
            Command::EndDrag => {
                self.dragging = None;
                vec![]
            }
            Command::InitiateScrollbarDrag(panel) => {
                self.dragging = None;
                self.focus = *panel;
                self.scrollbar_drag = Some(*panel);
                vec![Effect::SetDirty]
            }
            Command::EndScrollbarDrag => {
                self.scrollbar_drag = None;
                vec![]
            }
            Command::SetBranchWidthPct(pct) => {
                self.branch_width_pct = *pct;
                vec![Effect::SetDirty]
            }
            Command::SetDiffHeightPct(pct) => {
                self.diff_height_pct = *pct;
                vec![Effect::SetDirty]
            }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::layout::DragDirection;

    #[test]
    fn test_set_focus_changes_panel_and_sets_dirty() {
        let mut state = UiState::new(20, 35, 10);
        assert_eq!(state.focus, Panel::Commits);
        let effects = state.handle_command(&Command::SetFocus(Panel::Branches));
        assert_eq!(state.focus, Panel::Branches);
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
    }

    #[test]
    fn test_focus_next_cycles_forward() {
        let mut state = UiState::new(20, 35, 10);
        state.focus = Panel::Branches;
        state.handle_command(&Command::FocusNext);
        assert_eq!(state.focus, Panel::Commits);
    }

    #[test]
    fn test_focus_prev_cycles_backward() {
        let mut state = UiState::new(20, 35, 10);
        state.focus = Panel::Branches;
        state.handle_command(&Command::FocusPrev);
        assert_eq!(state.focus, Panel::Scope);
    }

    #[test]
    fn test_go_back_pops_focus_stack() {
        let mut state = UiState::new(20, 35, 10);
        state.focus = Panel::Diff;
        state.push_focus(Panel::Commits);
        state.handle_command(&Command::GoBack);
        assert_eq!(state.focus, Panel::Commits);
    }

    #[test]
    fn test_go_back_empty_stack_keeps_focus() {
        let mut state = UiState::new(20, 35, 10);
        state.focus = Panel::Diff;
        state.handle_command(&Command::GoBack);
        assert_eq!(state.focus, Panel::Diff);
    }

    #[test]
    fn test_focus_search_clear_sets_focus_to_search() {
        let mut state = UiState::new(20, 35, 10);
        state.focus = Panel::Commits;
        state.handle_command(&Command::FocusSearchClear);
        assert_eq!(state.focus, Panel::Search);
    }

    #[test]
    fn test_push_focus_adds_to_stack() {
        let mut state = UiState::new(20, 35, 10);
        state.push_focus(Panel::Branches);
        state.push_focus(Panel::Commits);
        assert_eq!(state.focus_stack, vec![Panel::Branches, Panel::Commits]);
    }

    #[test]
    fn test_drag_vertical_sets_dragging_state() {
        let mut state = UiState::new(20, 35, 10);
        let effects = state.handle_command(&Command::InitiateDragVertical);
        assert_eq!(state.dragging, Some(DragDirection::Vertical));
        assert!(effects.is_empty());
    }

    #[test]
    fn test_drag_horizontal_sets_dragging_state() {
        let mut state = UiState::new(20, 35, 10);
        state.handle_command(&Command::InitiateDragHorizontal);
        assert_eq!(state.dragging, Some(DragDirection::Horizontal));
    }

    #[test]
    fn test_end_drag_clears_dragging() {
        let mut state = UiState::new(20, 35, 10);
        state.dragging = Some(DragDirection::Vertical);
        state.handle_command(&Command::EndDrag);
        assert!(state.dragging.is_none());
    }

    #[test]
    fn test_scrollbar_drag_sets_focus_and_state() {
        let mut state = UiState::new(20, 35, 10);
        state.dragging = Some(DragDirection::Horizontal);
        let effects = state.handle_command(&Command::InitiateScrollbarDrag(Panel::Diff));
        assert!(state.dragging.is_none());
        assert_eq!(state.focus, Panel::Diff);
        assert_eq!(state.scrollbar_drag, Some(Panel::Diff));
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
    }

    #[test]
    fn test_end_scrollbar_drag_clears_state() {
        let mut state = UiState::new(20, 35, 10);
        state.scrollbar_drag = Some(Panel::Commits);
        state.handle_command(&Command::EndScrollbarDrag);
        assert!(state.scrollbar_drag.is_none());
    }

    #[test]
    fn test_set_branch_width_pct() {
        let mut state = UiState::new(20, 35, 10);
        let effects = state.handle_command(&Command::SetBranchWidthPct(30));
        assert_eq!(state.branch_width_pct, 30);
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
    }

    #[test]
    fn test_set_diff_height_pct() {
        let mut state = UiState::new(20, 35, 10);
        let effects = state.handle_command(&Command::SetDiffHeightPct(50));
        assert_eq!(state.diff_height_pct, 50);
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
    }

    #[test]
    fn test_toggle_help_flips_visibility() {
        let mut state = UiState::new(20, 35, 10);
        assert!(!state.help_visible);
        let effects = state.handle_command(&Command::ToggleHelp);
        assert!(state.help_visible);
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
        state.handle_command(&Command::ToggleHelp);
        assert!(!state.help_visible);
    }

    #[test]
    fn test_unknown_command_returns_empty_effects() {
        let mut state = UiState::new(20, 35, 10);
        let effects = state.handle_command(&Command::MoveUp);
        assert!(effects.is_empty());
    }

    #[test]
    fn test_toggle_fullscreen_enters_and_exits() {
        let mut state = UiState::new(20, 35, 10);
        state.focus = Panel::Commits;
        assert!(state.fullscreen.is_none());

        let effects = state.handle_command(&Command::ToggleFullscreen);
        assert_eq!(state.fullscreen, Some(Panel::Commits));
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));

        state.handle_command(&Command::ToggleFullscreen);
        assert!(state.fullscreen.is_none());
    }

    #[test]
    fn test_toggle_fullscreen_hides_help() {
        let mut state = UiState::new(20, 35, 10);
        state.help_visible = true;

        state.handle_command(&Command::ToggleFullscreen);
        assert!(!state.help_visible);
    }
}
