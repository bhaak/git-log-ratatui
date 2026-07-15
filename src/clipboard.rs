//! Clipboard operations using the arboard crate.
//! Handles copying commit hashes and pasting text into the search field.

use arboard::Clipboard;
use std::thread;
use std::time::Duration;

use crate::error::AppError;

/// Copy text to the system clipboard.
/// Keep the clipboard instance alive longer to ensure clipboard managers
/// can retrieve the contents. Uses a brief delay to work around issues with
/// clipboard managers not seeing the contents if the clipboard is dropped too quickly.
pub fn copy_to_clipboard(text: &str) -> Result<(), AppError> {
    let mut clipboard = Clipboard::new()
        .map_err(|e| AppError::Clipboard(format!("Failed to open clipboard: {}", e)))?;
    clipboard
        .set_text(text)
        .map_err(|e| AppError::Clipboard(format!("Failed to copy to clipboard: {}", e)))?;
    // Keep clipboard instance alive and give clipboard managers time to see the contents
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

/// Get text from the system clipboard.
pub fn get_clipboard_text() -> Option<String> {
    Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok())
        .filter(|t| !t.is_empty())
}
