use crate::models::Panel;
use crate::ui;

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
}
