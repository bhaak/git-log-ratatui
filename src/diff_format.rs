/// Append a single diff line from git2's `diff.print()` callback to `diff_lines`.
/// Handles the origin encoding: '+'/'-'/' ' get their prefix prepended,
/// 'F'/'H' (file/hunk headers) are split on newlines and pushed each, others trimmed.
pub fn append_diff_line(diff_lines: &mut Vec<String>, origin: char, content: &str) {
    match origin {
        '+' | '-' | ' ' => {
            diff_lines.push(format!("{}{}", origin, content.trim_end_matches('\n')));
        }
        'F' | 'H' => {
            for s in content.split('\n') {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    diff_lines.push(trimmed.to_string());
                }
            }
        }
        _ => {
            let trimmed = content.trim_end_matches('\n');
            if !trimmed.is_empty() {
                diff_lines.push(trimmed.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_diff_line_addition() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, '+', "new line\n");
        assert_eq!(lines, vec!["+new line"]);
    }

    #[test]
    fn test_append_diff_line_deletion() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, '-', "old line\n");
        assert_eq!(lines, vec!["-old line"]);
    }

    #[test]
    fn test_append_diff_line_context() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, ' ', "unchanged\n");
        assert_eq!(lines, vec![" unchanged"]);
    }

    #[test]
    fn test_append_diff_line_file_header() {
        let mut lines = Vec::new();
        append_diff_line(
            &mut lines,
            'F',
            "diff --git a/foo.txt b/foo.txt\nindex abc..def\n--- a/foo.txt\n+++ b/foo.txt\n",
        );
        assert_eq!(
            lines,
            vec![
                "diff --git a/foo.txt b/foo.txt",
                "index abc..def",
                "--- a/foo.txt",
                "+++ b/foo.txt",
            ]
        );
    }

    #[test]
    fn test_append_diff_line_hunk_header() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, 'H', "@@ -1,3 +1,4 @@\n");
        assert_eq!(lines, vec!["@@ -1,3 +1,4 @@"]);
    }

    #[test]
    fn test_append_diff_line_empty_skipped() {
        let mut lines = Vec::new();
        append_diff_line(&mut lines, 'F', "\n");
        assert!(lines.is_empty());
    }
}
