//! Clipboard operations using the arboard crate.
//! Handles copying commit hashes and pasting text into the search field.

use arboard::Clipboard;

/// Copy text to the system clipboard using a stored instance.
/// The clipboard instance should be kept alive in the app's context
/// to allow other applications and clipboard managers to request the contents later.
pub fn copy_to_clipboard(clipboard: &mut Option<Clipboard>, text: &str) {
    if let Some(cb) = clipboard {
        let _ = cb.set_text(text);
    }
}

/// Get text from the system clipboard.
pub fn get_clipboard_text() -> Option<String> {
    Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok())
        .filter(|t| !t.is_empty())
}
