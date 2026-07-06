//! Clipboard operations using the arboard crate.
//! Handles copying commit hashes and pasting text into the search field.

use arboard::Clipboard;

use crate::error::AppError;

/// Copy text to the system clipboard.
pub fn copy_to_clipboard(text: &str) -> Result<(), AppError> {
    let mut clipboard = Clipboard::new()
        .map_err(|e| AppError::Clipboard(format!("Failed to open clipboard: {}", e)))?;
    clipboard
        .set_text(text)
        .map_err(|e| AppError::Clipboard(format!("Failed to copy to clipboard: {}", e)))?;
    Ok(())
}

/// Get text from the system clipboard.
pub fn get_clipboard_text() -> Option<String> {
    Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok())
        .filter(|t| !t.is_empty())
}
