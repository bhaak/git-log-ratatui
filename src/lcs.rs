/// Word-level diff highlighting using the `similar` crate (Myers diff algorithm).
/// Replaces the previous custom LCS implementation per AGENTS.md.
use similar::TextDiff;

/// A tagged token span from a word-level diff.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenSpan {
    pub text: String,
    pub changed: bool,
}

/// Compute token spans for an added line (green background on changed words).
/// `prev_line` is the corresponding line from the old version (if available).
pub fn diff_tokens_added(line: &str, prev_line: Option<&str>) -> Vec<TokenSpan> {
    match prev_line {
        Some(prev) => {
            let diff = TextDiff::from_words(prev, line);
            diff.iter_all_changes()
                .map(|change| TokenSpan {
                    text: change.value().to_string(),
                    changed: change.tag() != similar::ChangeTag::Equal,
                })
                .collect()
        }
        None => vec![TokenSpan {
            text: line.to_string(),
            changed: true,
        }],
    }
}

/// Compute token spans for a deleted line (red background on changed words).
/// `next_line` is the corresponding line from the new version (if available).
pub fn diff_tokens_removed(line: &str, next_line: Option<&str>) -> Vec<TokenSpan> {
    match next_line {
        Some(next) => {
            let diff = TextDiff::from_words(line, next);
            diff.iter_all_changes()
                .map(|change| TokenSpan {
                    text: change.value().to_string(),
                    changed: change.tag() != similar::ChangeTag::Equal,
                })
                .collect()
        }
        None => vec![TokenSpan {
            text: line.to_string(),
            changed: true,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_tokens_added_no_prev() {
        let spans = diff_tokens_added("new line", None);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "new line");
        assert!(spans[0].changed);
    }

    #[test]
    fn test_diff_tokens_added_with_prev() {
        let spans = diff_tokens_added("new foo baz", Some("old foo bar"));
        // similar's from_words tokenization may differ from the old hand-written one
        let changed_words: Vec<&str> = spans
            .iter()
            .filter(|t| t.changed)
            .map(|t| t.text.as_str())
            .collect();
        let unchanged_words: Vec<&str> = spans
            .iter()
            .filter(|t| !t.changed)
            .map(|t| t.text.as_str())
            .collect();
        // "foo" should be unchanged
        assert!(
            unchanged_words.contains(&"foo"),
            "expected 'foo' unchanged, got unchanged: {:?}",
            unchanged_words
        );
        // "new" and "baz" should be changed
        assert!(changed_words.contains(&"new"));
        assert!(changed_words.contains(&"baz"));
    }

    #[test]
    fn test_diff_tokens_removed_no_next() {
        let spans = diff_tokens_removed("old line", None);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "old line");
        assert!(spans[0].changed);
    }

    #[test]
    fn test_diff_tokens_removed_with_next() {
        let spans = diff_tokens_removed("old foo bar", Some("new foo baz"));
        let changed_words: Vec<&str> = spans
            .iter()
            .filter(|t| t.changed)
            .map(|t| t.text.as_str())
            .collect();
        let unchanged_words: Vec<&str> = spans
            .iter()
            .filter(|t| !t.changed)
            .map(|t| t.text.as_str())
            .collect();
        assert!(unchanged_words.contains(&"foo"));
        assert!(changed_words.contains(&"old"));
        assert!(changed_words.contains(&"bar"));
    }

    #[test]
    fn test_identical_lines() {
        let spans = diff_tokens_added("same text", Some("same text"));
        assert!(
            spans.iter().all(|t| !t.changed),
            "all tokens should be unchanged"
        );
    }

    #[test]
    fn test_completely_different() {
        let spans = diff_tokens_added("hello", Some("world"));
        assert!(
            spans.iter().all(|t| t.changed),
            "all tokens should be changed"
        );
    }
}
