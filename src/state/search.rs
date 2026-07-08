use crate::app::commands::{Command, Effect};

/// Search panel state — query text and cursor position.
pub struct SearchState {
    pub search_query: String,
    pub cursor_pos: usize,
}

impl SearchState {
    pub fn new() -> Self {
        SearchState {
            search_query: String::new(),
            cursor_pos: 0,
        }
    }

    /// Handle search-related commands. Returns effects for cross-state operations.
    pub(crate) fn handle_command(&mut self, cmd: &Command) -> Vec<Effect> {
        match cmd {
            Command::SetSearch(query, cursor) => {
                self.search_query = query.clone();
                self.cursor_pos = *cursor;
                vec![Effect::ApplySearchFilter, Effect::SetDirty]
            }
            Command::ClearSearch => {
                self.search_query.clear();
                self.cursor_pos = 0;
                vec![Effect::ApplySearchFilter, Effect::SetDirty]
            }
            Command::PasteSearch(text) => {
                self.search_query = text.clone();
                self.cursor_pos = self.search_query.len();
                vec![Effect::ApplySearchFilter, Effect::SetDirty]
            }
            _ => vec![],
        }
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}
