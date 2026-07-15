//! Clipboard operations using the arboard crate.
//! Handles copying commit hashes and pasting text into the search field.

use arboard::Clipboard;

/// Copy text to the system clipboard using a stored instance.
/// The clipboard instance should be kept alive in the app's context
/// to allow other applications and clipboard managers to request the contents later.
///
/// On Linux, writes to both PRIMARY (middle-click) and CLIPBOARD (Ctrl+V) selections.
/// On other platforms, writes to the default clipboard.
#[cfg(target_os = "linux")]
pub fn copy_to_clipboard(clipboard: &mut Option<Clipboard>, text: &str) {
    use arboard::{LinuxClipboardKind, SetExtLinux};
    if let Some(cb) = clipboard {
        let _ = cb.set().clipboard(LinuxClipboardKind::Primary).text(text);
        let _ = cb.set().clipboard(LinuxClipboardKind::Clipboard).text(text);
    }
}

#[cfg(not(target_os = "linux"))]
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
