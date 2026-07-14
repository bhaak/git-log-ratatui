use ratatui::layout::{Constraint, Layout, Rect};

pub const DEFAULT_BRANCH_PCT: u16 = 20;
pub const MIN_BRANCH_PCT: u16 = 10;
pub const MAX_BRANCH_PCT: u16 = 40;

pub const DEFAULT_DIFF_PCT: u16 = 60;
pub const MIN_DIFF_PCT: u16 = 10;
pub const MAX_DIFF_PCT: u16 = 65;
pub const DIFF_MAX_PCT: u16 = 90;

pub const HELP_BAR_HEIGHT: u16 = 3;
pub const MIN_SEARCH_HEIGHT: u16 = 3;
pub const MIN_SCOPE_WIDTH: u16 = 10;
pub const SCROLLBAR_WIDTH: u16 = 1;

pub const BORDER_OVERHEAD: u16 = 1;
pub const PANEL_BORDER_H: u16 = 2;
pub const TABLE_OVERHEAD: u16 = 3;

pub const MIN_TERM_WIDTH: u16 = 20;
pub const MIN_TERM_HEIGHT: u16 = 8;

pub const RESIZE_GRAB_RANGE: i32 = 2;

/// Direction of a resize drag operation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
pub fn horizontal_resize_pct(row: u16, total_height: u16) -> u16 {
    let pct = (((total_height
        .saturating_sub(row)
        .saturating_sub(HELP_BAR_HEIGHT)) as f32)
        / (total_height as f32)
        * 100.0) as u16;
    pct.clamp(MIN_DIFF_PCT, MAX_DIFF_PCT)
}

/// Check if a position is near a vertical resize border (branch panel right edge).
pub fn is_on_vertical_border(col: u16, row: u16, branch_area: Rect) -> bool {
    let border_x = branch_area.x + branch_area.width;
    (col as i32 - border_x as i32).abs() <= RESIZE_GRAB_RANGE
        && row >= branch_area.y
        && row < branch_area.y + branch_area.height
}

/// Check if a position is near a horizontal resize border (below commit table).
pub fn is_on_horizontal_border(col: u16, row: u16, right_area: Rect, table_area: Rect) -> bool {
    let border_y = table_area.y + table_area.height;
    (row as i32 - border_y as i32).abs() <= RESIZE_GRAB_RANGE
        && col >= right_area.x
        && col < right_area.x + right_area.width
}

/// Computed screen areas for all application panels.
pub struct LayoutAreas {
    pub branch: Rect,
    pub right: Rect,
    pub search: Rect,
    pub scope: Rect,
    pub table: Rect,
    pub diff: Rect,
}

/// Compute panel layout areas from terminal dimensions and resizable split percentages.
pub fn compute_areas(full: Rect, branch_width_pct: u16, diff_height_pct: u16) -> LayoutAreas {
    let branch_w = Constraint::Percentage(branch_width_pct);
    let right_w = Constraint::Percentage(100 - branch_width_pct);

    let horizontal = Layout::horizontal([branch_w, right_w]).split(full);
    let branch_area = horizontal[0];
    let right_area = horizontal[1];

    let search_h = Constraint::Length(MIN_SEARCH_HEIGHT.min(right_area.height / 3));
    let diff_h = Constraint::Percentage(diff_height_pct.min(DIFF_MAX_PCT));
    let help_h = Constraint::Length(HELP_BAR_HEIGHT.min(right_area.height.saturating_sub(6) / 2));

    let main_split =
        Layout::vertical([search_h, Constraint::Min(0), diff_h, help_h]).split(right_area);

    let search_scope_area = main_split[0];
    let table_area = main_split[1];
    let diff_area = main_split[2];

    let search_split = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(
            MIN_SCOPE_WIDTH.min(search_scope_area.width.saturating_sub(PANEL_BORDER_H)),
        ),
    ])
    .split(search_scope_area);
    let search_area = search_split[0];
    let scope_area = search_split[1];

    LayoutAreas {
        branch: branch_area,
        right: right_area,
        search: search_area,
        scope: scope_area,
        table: table_area,
        diff: diff_area,
    }
}

/// Check if a point (col, row) is on the 1-character border of a rectangle,
/// i.e. not in the interior content area.
pub fn is_on_border_of(col: u16, row: u16, rect: Rect) -> bool {
    col == rect.x
        || col >= rect.x + rect.width.saturating_sub(1)
        || row == rect.y
        || row >= rect.y + rect.height.saturating_sub(1)
}

/// Check if a point (col, row) is inside a rectangle AND not on its border
/// AND not in the rightmost scrollbar column.
pub fn rect_contains_interior(rect: &Rect, pos: (u16, u16)) -> bool {
    rect_contains(rect, pos)
        && !is_on_border_of(pos.0, pos.1, *rect)
        && !is_on_scrollbar_col(pos.0, *rect)
}

/// True when `col` is in the rightmost column of `rect` (the scrollbar).
pub fn is_on_scrollbar_col(col: u16, rect: Rect) -> bool {
    col >= rect.x + rect.width.saturating_sub(SCROLLBAR_WIDTH)
}

/// Check if a point (col, row) is inside a rectangle.
pub fn rect_contains(rect: &Rect, pos: (u16, u16)) -> bool {
    pos.0 >= rect.x
        && pos.0 < rect.x + rect.width
        && pos.1 >= rect.y
        && pos.1 < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- rect_contains ---

    #[test]
    fn test_rect_contains_inside() {
        let r = Rect::new(10, 5, 20, 10);
        assert!(rect_contains(&r, (15, 8)));
    }

    #[test]
    fn test_rect_contains_top_left_corner() {
        let r = Rect::new(10, 5, 20, 10);
        assert!(rect_contains(&r, (10, 5)));
    }

    #[test]
    fn test_rect_contains_outside_left() {
        let r = Rect::new(10, 5, 20, 10);
        assert!(!rect_contains(&r, (9, 8)));
    }

    #[test]
    fn test_rect_contains_outside_right() {
        let r = Rect::new(10, 5, 20, 10);
        assert!(!rect_contains(&r, (30, 8)));
    }

    #[test]
    fn test_rect_contains_outside_top() {
        let r = Rect::new(10, 5, 20, 10);
        assert!(!rect_contains(&r, (15, 4)));
    }

    #[test]
    fn test_rect_contains_outside_bottom() {
        let r = Rect::new(10, 5, 20, 10);
        assert!(!rect_contains(&r, (15, 15)));
    }

    // --- vertical_resize_pct ---

    #[test]
    fn test_vertical_resize_pct_normal() {
        assert_eq!(vertical_resize_pct(30, 100), 30);
    }

    #[test]
    fn test_vertical_resize_pct_clamp_min() {
        assert_eq!(vertical_resize_pct(5, 100), MIN_BRANCH_PCT);
    }

    #[test]
    fn test_vertical_resize_pct_clamp_max() {
        assert_eq!(vertical_resize_pct(80, 100), MAX_BRANCH_PCT);
    }

    // --- horizontal_resize_pct ---

    #[test]
    fn test_horizontal_resize_pct_normal() {
        // row=30, total=100: diff = 100-30-3=67 → 67/100*100 = 67%
        let pct = horizontal_resize_pct(30, 100);
        assert!(pct >= 10);
    }

    #[test]
    fn test_horizontal_resize_pct_clamp_min() {
        // row=90, total=100: diff=100-90-3=7 → 7% → clamped to min
        assert_eq!(horizontal_resize_pct(90, 100), MIN_DIFF_PCT);
    }

    #[test]
    fn test_horizontal_resize_pct_clamp_max() {
        // row=0, total=50: diff=50-0-3=47 → 47/50*100=94 → clamped to max
        assert_eq!(horizontal_resize_pct(0, 50), MAX_DIFF_PCT);
    }

    // --- is_on_vertical_border ---

    #[test]
    fn test_is_on_vertical_border_exact() {
        let branch = Rect::new(0, 0, 20, 30);
        assert!(is_on_vertical_border(20, 10, branch));
    }

    #[test]
    fn test_is_on_vertical_border_near() {
        let branch = Rect::new(0, 0, 20, 30);
        assert!(is_on_vertical_border(22, 10, branch));
        assert!(is_on_vertical_border(18, 10, branch));
    }

    #[test]
    fn test_is_on_vertical_border_far() {
        let branch = Rect::new(0, 0, 20, 30);
        assert!(!is_on_vertical_border(25, 10, branch));
        assert!(!is_on_vertical_border(15, 10, branch));
    }

    #[test]
    fn test_is_on_vertical_border_outside_row() {
        let branch = Rect::new(0, 5, 20, 10);
        assert!(!is_on_vertical_border(20, 4, branch));
        assert!(!is_on_vertical_border(20, 15, branch));
    }

    // --- is_on_horizontal_border ---

    #[test]
    fn test_is_on_horizontal_border_near() {
        let right = Rect::new(20, 0, 80, 50);
        let table = Rect::new(20, 3, 80, 20);
        // border_y = 3+20 = 23, near at rows 21-25
        assert!(is_on_horizontal_border(30, 23, right, table));
        assert!(is_on_horizontal_border(30, 21, right, table));
    }

    #[test]
    fn test_is_on_horizontal_border_far() {
        let right = Rect::new(20, 0, 80, 50);
        let table = Rect::new(20, 3, 80, 20);
        assert!(!is_on_horizontal_border(30, 30, right, table));
        assert!(!is_on_horizontal_border(30, 15, right, table));
    }

    #[test]
    fn test_is_on_horizontal_border_outside_column() {
        let right = Rect::new(20, 0, 80, 50);
        let table = Rect::new(20, 3, 80, 20);
        assert!(!is_on_horizontal_border(10, 23, right, table));
        assert!(!is_on_horizontal_border(110, 23, right, table));
    }

    // --- compute_areas ---

    #[test]
    fn test_compute_areas_returns_valid_layout() {
        let full = Rect::new(0, 0, 120, 40);
        let areas = compute_areas(full, 20, 35);

        // Branch panel takes ~20%
        assert!(areas.branch.width > 0 && areas.branch.width < full.width);
        assert_eq!(areas.branch.x, 0);

        // Right area fills the rest
        assert_eq!(areas.branch.width + areas.right.width, full.width);
        assert_eq!(areas.right.x, areas.branch.width);

        // Search + scope are in the top of right area
        assert!(areas.search.height > 0);
        assert!(areas.scope.height > 0);
        assert_eq!(areas.search.y, areas.right.y);

        // Table is in the middle
        assert!(areas.table.height > 0);
        assert!(areas.table.y > areas.search.y);

        // Diff is below table
        assert!(areas.diff.y >= areas.table.y + areas.table.height);
    }

    // --- is_on_border_of ---

    #[test]
    fn test_is_on_border_of_top() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(is_on_border_of(5, 0, rect));
    }

    #[test]
    fn test_is_on_border_of_bottom() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(is_on_border_of(5, 9, rect));
    }

    #[test]
    fn test_is_on_border_of_left() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(is_on_border_of(0, 5, rect));
    }

    #[test]
    fn test_is_on_border_of_right() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(is_on_border_of(9, 5, rect));
    }

    #[test]
    fn test_is_on_border_of_interior() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(!is_on_border_of(5, 5, rect));
    }

    #[test]
    fn test_is_on_border_of_corner() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(is_on_border_of(0, 0, rect));
        assert!(is_on_border_of(9, 0, rect));
        assert!(is_on_border_of(0, 9, rect));
        assert!(is_on_border_of(9, 9, rect));
    }

    // --- rect_contains_interior ---

    #[test]
    fn test_rect_contains_interior_inside() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(rect_contains_interior(&rect, (5, 5)));
    }

    #[test]
    fn test_rect_contains_interior_on_border() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(!rect_contains_interior(&rect, (0, 5)));
        assert!(!rect_contains_interior(&rect, (9, 5)));
        assert!(!rect_contains_interior(&rect, (5, 0)));
        assert!(!rect_contains_interior(&rect, (5, 9)));
    }

    #[test]
    fn test_rect_contains_interior_outside() {
        let rect = Rect::new(0, 0, 10, 10);
        assert!(!rect_contains_interior(&rect, (10, 5)));
        assert!(!rect_contains_interior(&rect, (5, 10)));
    }

    #[test]
    fn test_rect_contains_interior_excludes_scrollbar_column() {
        let rect = Rect::new(0, 0, 20, 10);
        // Scrollbar column is at x=19 (rightmost 1 column)
        assert!(!rect_contains_interior(&rect, (19, 5)));
        // Content column just left of scrollbar is valid
        assert!(rect_contains_interior(&rect, (18, 5)));
    }
}
