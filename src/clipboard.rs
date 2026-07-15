//! Clipboard operations using the arboard crate.
//! Handles copying commit hashes and pasting text into the search field.

use arboard::Clipboard;
use std::thread;

/// Copy text to the system clipboard.
/// Spawns a background thread to keep the clipboard instance alive,
/// allowing arboard's background thread and clipboard managers sufficient
/// time to retrieve the contents. The Clipboard object is kept in scope
/// for the lifetime of the spawned thread.
pub fn copy_to_clipboard(text: &str) {
    let text = text.to_string();

    // Spawn background thread to keep clipboard instance alive
    thread::spawn(move || {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(text);
            // Keep the clipboard instance alive in this thread's scope.
            // The background thread arboard uses for serving clipboard contents
            // has time to run and let other apps (clipboard managers) make requests.
            // The Clipboard object will be dropped when this thread exits.
        }
    });
}

/// Get text from the system clipboard.
pub fn get_clipboard_text() -> Option<String> {
    Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok())
        .filter(|t| !t.is_empty())
}
