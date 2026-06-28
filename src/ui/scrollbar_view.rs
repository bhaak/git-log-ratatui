use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

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
            Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(panel_area);
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
}
