/// Move to the previous char boundary (for single-step left).
pub fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    for (i, _) in s.char_indices() {
        if i >= pos {
            break;
        }
    }
    for i in (0..pos).rev() {
        if s.is_char_boundary(i) {
            return i;
        }
    }
    0
}

/// Move to the next char boundary (for single-step right).
pub fn next_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    for (i, _) in s.char_indices().skip(1) {
        if i > pos {
            return i;
        }
    }
    s.len()
}

pub fn prev_word_boundary(s: &str, pos: usize) -> usize {
    let pos = prev_char_boundary(s, pos);
    if pos == 0 {
        return 0;
    }
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let char_pos = chars
        .iter()
        .position(|&(i, _)| i == pos)
        .unwrap_or(chars.len());
    if char_pos == 0 {
        return 0;
    }
    let mut idx = char_pos - 1;
    loop {
        let (_bi, ch) = chars.get(idx).copied().unwrap_or((0, '\0'));
        if ch.is_alphanumeric() || ch == '_' {
            if idx == 0 {
                return 0;
            }
            idx = idx.saturating_sub(1);
        } else {
            return chars.get(idx + 1).map(|&(i, _)| i).unwrap_or(0);
        }
    }
}

pub fn next_word_boundary(s: &str, pos: usize) -> usize {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let char_pos = chars
        .iter()
        .position(|&(i, _)| i >= pos)
        .unwrap_or(chars.len());
    let mut idx = char_pos;
    while idx < chars.len() {
        let (_, ch) = chars[idx];
        if ch.is_alphanumeric() || ch == '_' {
            idx += 1;
        } else {
            break;
        }
    }
    while idx < chars.len() {
        let (_, ch) = chars[idx];
        if !ch.is_alphanumeric() && ch != '_' {
            idx += 1;
        } else {
            break;
        }
    }
    chars.get(idx).map(|&(i, _)| i).unwrap_or(s.len())
}

/// Truncate a string to at most `max_len` bytes, snapping to a valid char boundary.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len || max_len <= 1 {
        return s.to_string();
    }
    let target = max_len.saturating_sub(1);
    let end = if s.is_char_boundary(target) {
        target
    } else {
        (0..target)
            .rev()
            .find(|&i| s.is_char_boundary(i))
            .unwrap_or(0)
    };
    if end == 0 {
        return s.to_string();
    }
    format!("{}…", &s[..end])
}

/// Format commit count info like "3/10" or "1/5 (20 filtered)".
pub fn format_commit_count_info(selected: usize, visible: usize, total: usize) -> String {
    if visible == 0 {
        "-".to_string()
    } else if visible == total {
        format!("{}/{}", selected + 1, visible)
    } else {
        format!("{}/{} ({} filtered)", selected + 1, visible, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prev_char_boundary_ascii() {
        assert_eq!(prev_char_boundary("hello", 3), 2);
        assert_eq!(prev_char_boundary("hello", 0), 0);
    }

    #[test]
    fn test_prev_char_boundary_utf8() {
        assert_eq!(prev_char_boundary("Mäller", 3), 1);
        assert_eq!(prev_char_boundary("Mäller", 2), 1);
    }

    #[test]
    fn test_next_char_boundary_ascii() {
        assert_eq!(next_char_boundary("hello", 2), 3);
        assert_eq!(next_char_boundary("hello", 5), 5);
    }

    #[test]
    fn test_next_char_boundary_utf8() {
        assert_eq!(next_char_boundary("Mäller", 1), 3);
        assert_eq!(next_char_boundary("Mäller", 3), 4);
    }

    #[test]
    fn test_prev_word_boundary() {
        assert_eq!(prev_word_boundary("hello world", 6), 0);
        assert_eq!(prev_word_boundary("foo bar", 6), 4);
    }

    #[test]
    fn test_next_word_boundary() {
        assert_eq!(next_word_boundary("hello world", 0), 6);
        assert_eq!(next_word_boundary("hello world", 5), 6);
        assert_eq!(next_word_boundary("hello", 0), 5);
    }

    #[test]
    fn test_truncate_ascii() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
    }

    #[test]
    fn test_truncate_multibyte() {
        assert_eq!(truncate("Mäller", 7), "Mäller");
        assert_eq!(truncate("Mäller", 5), "Mäl…");
    }

    #[test]
    fn test_truncate_emoji() {
        let s = "hi🎉there";
        assert_eq!(truncate(s, 20), "hi🎉there");
        assert_eq!(truncate(s, 9), "hi🎉th…");
        assert_eq!(truncate(s, 5), "hi…");
    }

    #[test]
    fn test_format_commit_count_info_no_commits() {
        assert_eq!(format_commit_count_info(0, 0, 0), "-");
    }

    #[test]
    fn test_format_commit_count_info_all_visible() {
        assert_eq!(format_commit_count_info(2, 10, 10), "3/10");
    }

    #[test]
    fn test_format_commit_count_info_filtered() {
        assert_eq!(format_commit_count_info(0, 5, 20), "1/5 (20 filtered)");
    }

    #[test]
    fn test_format_commit_count_info_first_item() {
        assert_eq!(format_commit_count_info(0, 1, 1), "1/1");
    }
}
