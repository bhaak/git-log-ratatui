use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::commands::Command;
use crate::app::state::AppState;

use super::render_ctx::RenderCtx;

/// Unified interface for UI panels.
///
/// Each panel renders itself into a given area and produces Commands from events.
#[allow(dead_code)]
pub trait Panel {
    /// Render the panel into the given area.
    fn render(&self, area: Rect, frame: &mut Frame, state: &AppState, ctx: &RenderCtx);

    /// Handle a UI event. Returns a list of Commands to execute on AppState.
    fn handle_event(&mut self, _event: &Event, _state: &AppState) -> Vec<Command> {
        Vec::new()
    }

    /// Keyboard shortcuts shown in the help bar.
    fn help_keys(&self) -> &[(&str, &str)];

    /// Human-readable panel name.
    fn label(&self) -> &str;
}
