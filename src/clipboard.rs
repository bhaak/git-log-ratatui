//! Clipboard operations using the arboard crate.
//! Handles copying commit hashes and pasting text into the search field.

use arboard::Clipboard;
use std::thread;
use std::time::Duration;

/// Copy text to the system clipboard.
/// Spawns a background thread to handle the clipboard operation, keeping the
/// clipboard instance alive longer to ensure clipboard managers can retrieve
/// the contents. This runs asynchronously without blocking the UI.
pub fn copy_to_clipboard(text: &str) {
    let text = text.to_string();

    // Spawn background thread to handle clipboard operation
    thread::spawn(move || {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(text);
            // Keep clipboard instance alive to give clipboard managers time to see contents
            thread::sleep(Duration::from_millis(100));
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
