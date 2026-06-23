/// Longest Common Subsequence for word-level diff highlighting.
/// Tokenizes lines by word boundaries and special characters,
/// finds unchanged tokens via LCS, and returns changed token ranges.

#[derive(Debug, Clone, PartialEq)]
pub struct TokenSpan {
    pub text: String,
    pub changed: bool,
}

/// Tokenize a line into word tokens, splitting at word boundaries and special characters.
pub fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in line.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            tokens.push(ch.to_string());
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Compute LCS length matrix for two token sequences.
fn lcs_matrix(a: &[String], b: &[String]) -> Vec<Vec<usize>> {
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    dp
}

/// Backtrack LCS to mark which tokens are unchanged.
fn mark_unchanged(a: &[String], b: &[String]) -> (Vec<bool>, Vec<bool>) {
    let dp = lcs_matrix(a, b);
    let mut a_unchanged = vec![false; a.len()];
    let mut b_unchanged = vec![false; b.len()];

    let (mut i, mut j) = (a.len(), b.len());
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            a_unchanged[i - 1] = true;
            b_unchanged[j - 1] = true;
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    (a_unchanged, b_unchanged)
}

/// Compute token spans for an added line (green background on changed words).
/// `prev_line` is the corresponding line from the old version (if available).
pub fn diff_tokens_added(line: &str, prev_line: Option<&str>) -> Vec<TokenSpan> {
    let tokens = tokenize(line);

    match prev_line {
        Some(prev) => {
            let prev_tokens = tokenize(prev);
            let (_, b_unchanged) = mark_unchanged(&prev_tokens, &tokens);
            tokens
                .into_iter()
                .enumerate()
                .map(|(i, text)| TokenSpan {
                    text,
                    changed: !b_unchanged[i],
                })
                .collect()
        }
        None => tokens
            .into_iter()
            .map(|text| TokenSpan {
                text,
                changed: true,
            })
            .collect(),
    }
}

/// Compute token spans for a deleted line (red background on changed words).
/// `next_line` is the corresponding line from the new version (if available).
pub fn diff_tokens_removed(line: &str, next_line: Option<&str>) -> Vec<TokenSpan> {
    let tokens = tokenize(line);

    match next_line {
        Some(next) => {
            let next_tokens = tokenize(next);
            let (a_unchanged, _) = mark_unchanged(&tokens, &next_tokens);
            tokens
                .into_iter()
                .enumerate()
                .map(|(i, text)| TokenSpan {
                    text,
                    changed: !a_unchanged[i],
                })
                .collect()
        }
        None => tokens
            .into_iter()
            .map(|text| TokenSpan {
                text,
                changed: true,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize("hello world");
        assert_eq!(tokens, vec!["hello", " ", "world"]);
    }

    #[test]
    fn test_tokenize_special_chars() {
        let tokens = tokenize("foo = bar");
        assert_eq!(tokens, vec!["foo", " ", "=", " ", "bar"]);
    }

    #[test]
    fn test_tokenize_empty() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_single_word() {
        let tokens = tokenize("hello");
        assert_eq!(tokens, vec!["hello"]);
    }

    #[test]
    fn test_tokenize_underscore() {
        let tokens = tokenize("my_var");
        assert_eq!(tokens, vec!["my_var"]);
    }

    #[test]
    fn test_diff_tokens_added_no_prev() {
        let spans = diff_tokens_added("new line", None);
        assert_eq!(spans[0].changed, true);
        assert_eq!(spans[0].text, "new");
        assert_eq!(spans[2].changed, true);
        assert_eq!(spans[2].text, "line");
    }

    #[test]
    fn test_diff_tokens_added_with_prev() {
        let spans = diff_tokens_added("new foo baz", Some("old foo bar"));
        assert_eq!(spans[0].text, "new");
        assert!(spans[0].changed);
        assert_eq!(spans[2].text, "foo");
        assert!(!spans[2].changed);
        assert_eq!(spans[4].text, "baz");
        assert!(spans[4].changed);
    }

    #[test]
    fn test_diff_tokens_removed_no_next() {
        let spans = diff_tokens_removed("old line", None);
        assert_eq!(spans[0].changed, true);
        assert_eq!(spans[0].text, "old");
        assert_eq!(spans[2].changed, true);
        assert_eq!(spans[2].text, "line");
    }

    #[test]
    fn test_mark_unchanged_identical() {
        let a = vec!["a".to_string(), "b".to_string()];
        let b = vec!["a".to_string(), "b".to_string()];
        let (a_unchanged, b_unchanged) = super::mark_unchanged(&a, &b);
        assert_eq!(a_unchanged, vec![true, true]);
        assert_eq!(b_unchanged, vec![true, true]);
    }

    #[test]
    fn test_mark_unchanged_all_different() {
        let a = vec!["x".to_string()];
        let b = vec!["y".to_string()];
        let (a_unchanged, b_unchanged) = super::mark_unchanged(&a, &b);
        assert_eq!(a_unchanged, vec![false]);
        assert_eq!(b_unchanged, vec![false]);
    }
}
