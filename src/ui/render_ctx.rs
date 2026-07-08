use crate::theme::Theme;
use crate::view::Panel;

/// Shared rendering context passed to every panel.
pub struct RenderCtx<'a> {
    pub focus: Panel,
    pub debug_label: Option<&'a str>,
    pub theme: &'a Theme,
    /// Repository path for the title bar.
    pub repo_path: &'a str,
    /// Currently selected branch name (None = all branches).
    pub selected_branch: Option<&'a str>,
    /// Whether a search query is active.
    pub search_active: bool,
}

impl<'a> RenderCtx<'a> {
    pub fn is_focused(&self, panel: Panel) -> bool {
        self.focus == panel
    }
}
