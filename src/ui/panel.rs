use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::commands::Command;

use super::render_ctx::RenderCtx;

/// A single keybinding with compact and verbose descriptions.
/// `short_desc` is used in the status bar, `long_desc` in the help modal.
pub struct KeyBinding {
    pub key: &'static str,
    pub short_desc: &'static str,
    pub long_desc: &'static str,
}

impl KeyBinding {
    pub const fn new(key: &'static str, short_desc: &'static str, long_desc: &'static str) -> Self {
        KeyBinding {
            key,
            short_desc,
            long_desc,
        }
    }
}

/// Unified interface for UI panels.
///
/// Each panel renders itself into a given area and produces Commands from events.
/// Each panel defines its own `State` type so it only receives its own data,
/// not the entire AppState.
pub trait Panel {
    /// The state slice this panel owns.
    type State;

    /// Render the panel into the given area.
    fn render(&self, area: Rect, frame: &mut Frame, state: &mut Self::State, ctx: &RenderCtx);

    /// Handle a keyboard event. Returns a list of Commands to execute on AppState.
    fn handle_event(&mut self, event: &KeyEvent, state: &mut Self::State) -> Vec<Command>;

    /// Keyboard shortcuts. `short_desc` is shown in the help bar,
    /// `long_desc` in the help modal.
    fn help_keys(&self) -> &'static [KeyBinding];

    /// Human-readable panel name.
    fn label(&self) -> &str;
}
