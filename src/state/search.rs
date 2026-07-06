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
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}
