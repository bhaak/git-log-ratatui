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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_search_applies_filter_and_sets_dirty() {
        let mut state = SearchState::new();
        let effects = state.handle_command(&Command::SetSearch("fix bug".into(), 7));
        assert_eq!(state.search_query, "fix bug");
        assert_eq!(state.cursor_pos, 7);
        assert_eq!(effects.len(), 2);
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::ApplySearchFilter)));
        assert!(effects.iter().any(|e| matches!(e, Effect::SetDirty)));
    }

    #[test]
    fn test_clear_search_empties_and_reapplies_filter() {
        let mut state = SearchState::new();
        state.search_query = "old".into();
        state.cursor_pos = 3;
        let effects = state.handle_command(&Command::ClearSearch);
        assert!(state.search_query.is_empty());
        assert_eq!(state.cursor_pos, 0);
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::ApplySearchFilter)));
    }

    #[test]
    fn test_paste_search_sets_text_and_cursor_at_end() {
        let mut state = SearchState::new();
        let effects = state.handle_command(&Command::PasteSearch("abc123".into()));
        assert_eq!(state.search_query, "abc123");
        assert_eq!(state.cursor_pos, 6);
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::ApplySearchFilter)));
    }

    #[test]
    fn test_unknown_command_returns_empty_effects() {
        let mut state = SearchState::new();
        let effects = state.handle_command(&Command::MoveUp);
        assert!(effects.is_empty());
    }
}
