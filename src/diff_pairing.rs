use std::collections::HashMap;

/// Pre-computed O(1) lookup maps for diff line pairing.
/// Maps each `+` line index to its paired `-` line index, and vice versa.
#[derive(Debug, Default)]
pub struct DiffPairMaps {
    pub added_to_removed: HashMap<usize, usize>,
    pub removed_to_added: HashMap<usize, usize>,
}

/// Build diff line pairing maps for LCS matching.
/// Called once per diff render -- O(n) scan over all lines.
pub fn build_pair_maps(diff_lines: &[String]) -> DiffPairMaps {
    let mut maps = DiffPairMaps::default();
    let mut removed_stack: Vec<usize> = Vec::new();
    let mut added_stack: Vec<usize> = Vec::new();

    for (i, line) in diff_lines.iter().enumerate() {
        if line.starts_with('-') {
            if let Some(&added_idx) = added_stack.first() {
                maps.added_to_removed.insert(added_idx, i);
                maps.removed_to_added.insert(i, added_idx);
                added_stack.remove(0);
            } else {
                removed_stack.push(i);
            }
        } else if line.starts_with('+') {
            if let Some(&removed_idx) = removed_stack.first() {
                maps.removed_to_added.insert(removed_idx, i);
                maps.added_to_removed.insert(i, removed_idx);
                removed_stack.remove(0);
            } else {
                added_stack.push(i);
            }
        } else if !line.starts_with("@@")
            && !line.starts_with("diff ")
            && !line.starts_with("index ")
            && !line.starts_with("--- ")
            && !line.starts_with("+++ ")
        {
            removed_stack.clear();
            added_stack.clear();
        }
    }

    maps
}

/// Find the previous removed line content that pairs with an added line.
pub fn find_prev_removed_line<'a>(
    diff_lines: &'a [String],
    added_idx: usize,
    maps: &DiffPairMaps,
) -> Option<&'a str> {
    maps.added_to_removed
        .get(&added_idx)
        .and_then(|&removed_idx| diff_lines.get(removed_idx))
        .map(|line| &line[1..])
}

/// Find the next added line content that pairs with a removed line.
pub fn find_next_added_line<'a>(
    diff_lines: &'a [String],
    removed_idx: usize,
    maps: &DiffPairMaps,
) -> Option<&'a str> {
    maps.removed_to_added
        .get(&removed_idx)
        .and_then(|&added_idx| diff_lines.get(added_idx))
        .map(|line| &line[1..])
}

/// Format a renamed path diff like `app/commands/{belege → kreditor}/test_command.rb`
pub fn diff_paths(old: &str, new: &str) -> String {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();

    let prefix_len = old_chars
        .iter()
        .zip(new_chars.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let old_rest = &old_chars[prefix_len..];
    let new_rest = &new_chars[prefix_len..];

    let suffix_len = old_rest
        .iter()
        .rev()
        .zip(new_rest.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();

    let old_mid: String = old_rest[..old_rest.len() - suffix_len].iter().collect();
    let new_mid: String = new_rest[..new_rest.len() - suffix_len].iter().collect();
    let suffix: String = old_rest[old_rest.len() - suffix_len..].iter().collect();
    let prefix: String = old_chars[..prefix_len].iter().collect();

    format!("{}{{{} → {}}}{}", prefix, old_mid, new_mid, suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_pair_maps_simple() {
        let lines = vec!["-old".to_string(), "+new".to_string()];
        let maps = build_pair_maps(&lines);
        assert_eq!(maps.removed_to_added.get(&0), Some(&1));
        assert_eq!(maps.added_to_removed.get(&1), Some(&0));
    }

    #[test]
    fn test_build_pair_maps_with_context() {
        let lines = vec![
            "-old1".to_string(),
            " context".to_string(),
            "+new1".to_string(),
        ];
        let maps = build_pair_maps(&lines);
        assert!(maps.removed_to_added.is_empty());
        assert!(maps.added_to_removed.is_empty());
    }

    #[test]
    fn test_build_pair_maps_multiple() {
        let lines = vec![
            "-old1".to_string(),
            "+new1".to_string(),
            "-old2".to_string(),
            "+new2".to_string(),
        ];
        let maps = build_pair_maps(&lines);
        assert_eq!(maps.removed_to_added.len(), 2);
        assert_eq!(maps.added_to_removed.len(), 2);
        assert_eq!(maps.removed_to_added.get(&0), Some(&1));
        assert_eq!(maps.removed_to_added.get(&2), Some(&3));
    }

    #[test]
    fn test_find_prev_removed() {
        let lines = vec!["-removed content".to_string(), "+added content".to_string()];
        let maps = build_pair_maps(&lines);
        assert_eq!(
            find_prev_removed_line(&lines, 1, &maps),
            Some("removed content")
        );
    }

    #[test]
    fn test_find_next_added() {
        let lines = vec!["-removed content".to_string(), "+added content".to_string()];
        let maps = build_pair_maps(&lines);
        assert_eq!(
            find_next_added_line(&lines, 0, &maps),
            Some("added content")
        );
    }

    #[test]
    fn test_diff_paths() {
        assert_eq!(
            diff_paths("src/old/module.rs", "src/new/module.rs"),
            "src/{old → new}/module.rs"
        );
        assert_eq!(diff_paths("old.rs", "new.rs"), "{old → new}.rs");
    }
}
