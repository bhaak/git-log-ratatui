use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use super::layout::SCROLLBAR_WIDTH;

/// Encapsulates a scrollbar widget with rendering, state tracking, and mouse click handling.
pub struct ScrollbarView {
    /// Ratatui scrollbar state for rendering.
    state: ScrollbarState,
    /// Total number of content items (e.g. lines, rows).
    content_length: usize,
    /// Number of items visible in the viewport.
    viewport_length: usize,
}

impl ScrollbarView {
    pub fn new() -> Self {
        Self {
            state: ScrollbarState::default(),
            content_length: 0,
            viewport_length: 0,
        }
    }

    /// Return the content length from the last render.
    pub fn content_length(&self) -> usize {
        self.content_length
    }

    /// Return the viewport length from the last render.
    pub fn viewport_length(&self) -> usize {
        self.viewport_length
    }

    /// Split a panel area horizontally into (content_area, scrollbar_area).
    /// The scrollbar gets exactly 1 column on the right.
    pub fn split(panel_area: Rect) -> (Rect, Rect) {
        let [content_area, scrollbar_area] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(SCROLLBAR_WIDTH)])
                .areas(panel_area);
        (content_area, scrollbar_area)
    }

    /// Update internal state and render the scrollbar if content overflows the viewport.
    pub fn render(
        &mut self,
        frame: &mut Frame,
        scrollbar_area: Rect,
        content_length: usize,
        viewport_length: usize,
        position: usize,
        thumb_style: Style,
    ) {
        self.content_length = content_length;
        self.viewport_length = viewport_length;

        if content_length <= viewport_length {
            return;
        }

        self.state = ScrollbarState::new(content_length)
            .viewport_content_length(viewport_length)
            .position(position);

        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).thumb_style(thumb_style),
            scrollbar_area,
            &mut self.state,
        );
    }

    /// Handle a mouse click on the scrollbar area.
    ///
    /// Maps the click Y position within `scrollbar_area` to a proportional scroll position.
    /// Returns `None` if the content fits entirely (no scrolling needed).
    ///
    /// The formula is the exact inverse of how ratatui renders the scrollbar thumb:
    ///   thumb_start = position * track_length / (content_length - 1 + viewport_length)
    pub fn map_click_to_position(
        scrollbar_area: Rect,
        row: u16,
        content_length: usize,
        viewport_length: usize,
    ) -> Option<usize> {
        if content_length <= viewport_length || scrollbar_area.height == 0 {
            return None;
        }

        let track_height = scrollbar_area.height as f64;
        let rel = (row.saturating_sub(scrollbar_area.y)) as f64;
        let ratio = (rel / track_height).clamp(0.0, 1.0);
        // ratatui uses (content_length - 1 + viewport_length) as the full range denominator
        let range = (content_length.saturating_sub(1) + viewport_length) as f64;
        let max_position = content_length.saturating_sub(1);

        Some(((ratio * range).round() as usize).min(max_position))
    }

    /// Convenience: split `panel_area` and map a mouse click to a scroll position.
    ///
    /// Combines `split()`, positional containment check and `map_click_to_position()`.
    /// Returns `None` if content fits entirely or click is outside the scrollbar area.
    pub fn click_to_index(&self, panel_area: Rect, click_pos: (u16, u16)) -> Option<usize> {
        let (_, sb) = Self::split(panel_area);
        if click_pos.0 < sb.x
            || click_pos.0 >= sb.x + sb.width
            || click_pos.1 < sb.y
            || click_pos.1 >= sb.y + sb.height
        {
            return None;
        }
        Self::map_click_to_position(
            sb,
            click_pos.1,
            self.content_length(),
            self.viewport_length(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrollbar_view_new_is_empty() {
        let sv = ScrollbarView::new();
        assert_eq!(sv.content_length(), 0);
        assert_eq!(sv.viewport_length(), 0);
    }

    #[test]
    fn test_split_produces_two_areas() {
        let panel = Rect::new(0, 0, 100, 30);
        let (content, scrollbar) = ScrollbarView::split(panel);
        assert_eq!(content.width + scrollbar.width, panel.width);
        assert_eq!(scrollbar.width, SCROLLBAR_WIDTH);
        assert_eq!(content.height, panel.height);
        assert_eq!(scrollbar.height, panel.height);
    }

    #[test]
    fn test_map_click_to_position_top() {
        let sb = Rect::new(95, 0, 1, 30);
        let pos = ScrollbarView::map_click_to_position(sb, 0, 100, 10);
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn test_map_click_to_position_bottom() {
        let sb = Rect::new(95, 0, 1, 30);
        let pos = ScrollbarView::map_click_to_position(sb, 29, 100, 10);
        assert!(pos.is_some());
        assert!(pos.unwrap() > 0);
    }

    #[test]
    fn test_map_click_to_position_no_scroll_needed() {
        let sb = Rect::new(95, 0, 1, 30);
        // Content fits in viewport — no scrollbar needed
        assert_eq!(ScrollbarView::map_click_to_position(sb, 15, 5, 10), None);
    }

    #[test]
    fn test_map_click_to_position_zero_height() {
        let sb = Rect::new(95, 0, 1, 0);
        assert_eq!(ScrollbarView::map_click_to_position(sb, 0, 100, 10), None);
    }

    #[test]
    fn test_click_to_index_inside_scrollbar() {
        let sv = ScrollbarView::new();
        let panel = Rect::new(0, 0, 100, 30);
        // Click on the scrollbar area (x=99, which is in the scrollbar column)
        let result = sv.click_to_index(panel, (99, 5));
        // content_length=0, viewport_length=0 → no scroll needed → None
        assert_eq!(result, None);
    }

    #[test]
    fn test_click_to_index_outside_scrollbar() {
        let sv = ScrollbarView::new();
        let panel = Rect::new(0, 0, 100, 30);
        // Click on the content area, not the scrollbar
        let result = sv.click_to_index(panel, (50, 5));
        assert_eq!(result, None);
    }
}
