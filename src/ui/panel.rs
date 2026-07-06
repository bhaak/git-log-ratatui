use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::state::AppState;

/// Outcome of a panel event handler.
#[allow(dead_code)]
pub enum EventOutcome {
    Continue,
    Quit,
}

/// Unified interface for UI panels.
///
/// Each panel renders itself into a given area and handles keyboard/mouse events.
#[allow(unused)]
pub trait Panel {
    /// Render the panel into the given area.
    fn render(&self, area: Rect, frame: &mut Frame, state: &AppState, is_focused: bool);

    /// Handle a UI event. Returns EventOutcome::Quit to exit the app.
    fn handle_event(&mut self, _event: &Event, _state: &mut AppState) -> EventOutcome {
        EventOutcome::Continue
    }

    /// Keyboard shortcuts shown in the help bar.
    fn help_keys(&self) -> &[(&str, &str)];

    /// Human-readable panel name.
    fn label(&self) -> &str;
}
