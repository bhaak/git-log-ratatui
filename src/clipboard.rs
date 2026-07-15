//! Clipboard operations using the arboard crate.
//! Handles copying commit hashes and pasting text into the search field.
//!
//! Supports both system clipboard (via arboard) and terminal escape sequences
//! (OSC 52) for copying over SSH or in terminals without clipboard access.

use arboard::Clipboard;
use base64::Engine;
use std::io::{self, Write};

fn write_osc52(clipboard: &str, text: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    // OSC 52 escape sequence: ESC ] 5 2 ; <clipboard> ; <base64> ST
    let _ = io::stdout().write_all(format!("\x1b]52;{};{}\x1b\\", clipboard, encoded).as_bytes());
    let _ = io::stdout().flush();
}

/// Copy text to the system clipboard using a stored instance.
/// Also emits an OSC 52 escape sequence for terminals that support it
/// (e.g. SSH sessions, tmux with set-clipboard, etc.).
#[cfg(target_os = "linux")]
pub fn copy_to_clipboard(clipboard: &mut Option<Clipboard>, text: &str) {
    use arboard::{LinuxClipboardKind, SetExtLinux};
    if let Some(cb) = clipboard {
        let _ = cb.set().clipboard(LinuxClipboardKind::Primary).text(text);
        let _ = cb.set().clipboard(LinuxClipboardKind::Clipboard).text(text);
    }
    write_osc52("p", text);
    write_osc52("c", text);
}

#[cfg(not(target_os = "linux"))]
pub fn copy_to_clipboard(clipboard: &mut Option<Clipboard>, text: &str) {
    if let Some(cb) = clipboard {
        let _ = cb.set_text(text);
    }
    write_osc52("c", text);
}

/// Get text from the system clipboard.
pub fn get_clipboard_text() -> Option<String> {
    Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok())
        .filter(|t| !t.is_empty())
}
