use crate::models::Panel;
use crate::theme::Theme;

/// Shared rendering context passed to every panel.
#[allow(dead_code)]
pub struct RenderCtx<'a> {
    pub focus: Panel,
    pub debug_label: Option<&'a str>,
    pub theme: &'a Theme,
}

#[allow(dead_code)]
impl<'a> RenderCtx<'a> {
    pub fn is_focused(&self, panel: Panel) -> bool {
        self.focus == panel
    }
}
