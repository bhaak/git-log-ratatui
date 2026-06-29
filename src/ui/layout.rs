use ratatui::layout::Rect;

pub const DEFAULT_BRANCH_PCT: u16 = 20;
pub const MIN_BRANCH_PCT: u16 = 10;
pub const MAX_BRANCH_PCT: u16 = 40;

pub const DEFAULT_DIFF_PCT: u16 = 35;
pub const MIN_DIFF_PCT: u16 = 10;
pub const MAX_DIFF_PCT: u16 = 65;

/// Direction of a resize drag operation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DragDirection {
    /// Dragging the vertical split between branch panel and main area
    Vertical,
    /// Dragging the horizontal split between commit table and diff panel
    Horizontal,
}

/// Compute new branch panel width percentage from a vertical drag at column `col`.
pub fn vertical_resize_pct(col: u16, total_width: u16) -> u16 {
    let pct = ((col as f32) / (total_width as f32) * 100.0) as u16;
    pct.clamp(MIN_BRANCH_PCT, MAX_BRANCH_PCT)
}

/// Compute new diff panel height percentage from a horizontal drag at row `row`.
/// `total_height` is terminal height, `help_bar_height` accounts for the help bar at bottom.
pub fn horizontal_resize_pct(row: u16, total_height: u16, help_bar_height: u16) -> u16 {
    let pct = (((total_height
        .saturating_sub(row)
        .saturating_sub(help_bar_height)) as f32)
        / (total_height as f32)
        * 100.0) as u16;
    pct.clamp(MIN_DIFF_PCT, MAX_DIFF_PCT)
}

/// Check if a position is near a vertical resize border (branch panel right edge).
pub fn is_on_vertical_border(col: u16, row: u16, branch_area: Rect) -> bool {
    let border_x = branch_area.x + branch_area.width;
    (col as i32 - border_x as i32).abs() <= 2
        && row >= branch_area.y
        && row < branch_area.y + branch_area.height
}

/// Check if a position is near a horizontal resize border (below commit table).
pub fn is_on_horizontal_border(col: u16, row: u16, right_area: Rect, table_area: Rect) -> bool {
    let border_y = table_area.y + table_area.height;
    (row as i32 - border_y as i32).abs() <= 2
        && col >= right_area.x
        && col < right_area.x + right_area.width
}
