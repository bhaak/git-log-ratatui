use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::commands::Command;

use super::render_ctx::RenderCtx;

/// Unified interface for UI panels.
///
/// Each panel renders itself into a given area and produces Commands from events.
/// Each panel defines its own `State` type so it only receives its own data,
/// not the entire AppState.
pub trait Panel {
    /// The state slice this panel owns.
    type State;

    /// Render the panel into the given area.
    fn render(&self, area: Rect, frame: &mut Frame, state: &Self::State, ctx: &RenderCtx);

    /// Handle a keyboard event. Returns a list of Commands to execute on AppState.
    fn handle_event(&mut self, event: &KeyEvent, state: &mut Self::State) -> Vec<Command>;

    /// Keyboard shortcuts shown in the help bar.
    fn help_keys(&self) -> &[(&str, &str)];

    /// Human-readable panel name.
    fn label(&self) -> &str;
}
