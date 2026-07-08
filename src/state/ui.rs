use crate::app::commands::{Command, Effect};
use crate::ui;
use crate::view::Panel;

/// Global UI state — focus, layout, drag tracking, and status.
pub struct UiState {
    pub focus: Panel,
    pub branch_width_pct: u16,
    pub diff_height_pct: u16,
    pub dragging: Option<ui::layout::DragDirection>,
    pub scrollbar_drag: Option<Panel>,
    pub last_size: Option<(u16, u16)>,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub status_message: Option<String>,
    pub poll_interval_ms: u8,
    pub dirty: bool,
}

impl UiState {
    pub fn new(branch_width_pct: u16, diff_height_pct: u16, poll_min_ms: u8) -> Self {
        UiState {
            focus: Panel::Commits,
            branch_width_pct,
            diff_height_pct,
            dragging: None,
            scrollbar_drag: None,
            last_size: None,
            last_mouse_pos: None,
            status_message: None,
            poll_interval_ms: poll_min_ms,
            dirty: true,
        }
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
