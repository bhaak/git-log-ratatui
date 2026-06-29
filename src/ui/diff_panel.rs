use std::collections::HashMap;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::lcs;
use crate::models::*;
use crate::ui::layout::PANEL_BORDER_H;

/// Render context for the diff panel.
pub struct DiffPanelCtx<'a> {
    pub commit_info: Option<&'a CommitInfo>,
    pub diff_lines: &'a [String],
    pub file_entries: &'a [FileEntry],
    pub selected_file_index: usize,
    pub diff_scroll: usize,
    pub is_focused: bool,
    pub short_hash: Option<&'a str>,
}

/// Render the diff panel with commit metadata, changed files, and colored diff.
/// Returns the total number of display lines.
pub fn render(frame: &mut Frame, area: Rect, ctx: &DiffPanelCtx) -> usize {
    if area.width < 4 || area.height < 2 {
        return 0;
    }

    let border_style = if ctx.is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    let all_lines = build_all_lines(
        ctx.commit_info,
        ctx.diff_lines,
        ctx.file_entries,
        ctx.selected_file_index,
    );
    let total = all_lines.len();
    let visible = area.height.saturating_sub(PANEL_BORDER_H) as usize;
    let start = (ctx.diff_scroll + 1).min(total);
    let end = (ctx.diff_scroll + visible).min(total);

    let diff_title = if let Some(hash) = ctx.short_hash {
        format!(" Diff - {}", hash)
    } else {
        " Diff".to_string()
    };

    let scroll_info = if total > 0 {
        format!(" lines {}-{}/{} ", start, end, total)
    } else {
        String::new()
    };

    let paragraph = Paragraph::new(all_lines)
        .block(
            Block::default()
                .title(format!("{}{}", diff_title, scroll_info))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .scroll((ctx.diff_scroll as u16, 0));

    frame.render_widget(paragraph, area);
    total
}

/// Build the complete display: metadata + file list header + file entries + a gap + diff content.
fn build_all_lines<'a>(
    commit_info: Option<&'a CommitInfo>,
    diff_lines: &'a [String],
    file_entries: &'a [FileEntry],
    selected_file_index: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    if diff_lines.is_empty() {
        if commit_info.is_some() {
            lines.push(Line::from(Span::styled(
                "No changes in this commit.",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "Select a commit to view diff.",
                Style::default().fg(Color::DarkGray),
            )));
        }
        return lines;
    }

    // Commit metadata
    if let Some(info) = commit_info {
        lines.extend(build_metadata_lines(info));
    }

    let metadata_len = lines.len();

    // Changed files header + entries
    if !file_entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "Changed files:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));

        for (i, entry) in file_entries.iter().enumerate() {
            let selected = i == selected_file_index;
            let status_color = match entry.status {
                '+' => Color::Green,
                '-' => Color::Red,
                '~' => Color::Yellow,
                '→' => Color::Blue,
                _ => Color::Gray,
            };
            let name_style = if selected {
                Style::default().bg(Color::White).fg(Color::Black)
            } else {
                Style::default().fg(Color::Rgb(100, 150, 255))
            };
            let status_style = Style::default().fg(status_color);
            let display_name = if let Some(ref old) = entry.old_name {
                diff_paths(old, &entry.name)
            } else {
                entry.name.clone()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", entry.status), status_style),
                Span::styled(display_name, name_style),
            ]));
        }

        lines.push(Line::from(""));
    }

    let _file_header_len = lines.len() - metadata_len;

    // Diff content with word-level highlighting
    // Build pair maps once — O(n) scan, then O(1) lookup per line
    let pair_maps = build_pair_maps(diff_lines);

    for (line_idx, line) in diff_lines.iter().enumerate() {
        if line.starts_with("@@") {
            lines.push(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(Color::Cyan),
            )));
        } else if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
        {
            lines.push(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(Color::Yellow),
            )));
        } else if let Some(content) = line.strip_prefix('+') {
            let prev_removed = find_prev_removed_line(diff_lines, line_idx, &pair_maps);
            let tokens = lcs::diff_tokens_added(content, prev_removed);
            let spans: Vec<Span> = tokens
                .into_iter()
                .map(|t| {
                    if t.changed {
                        Span::styled(
                            t.text,
                            Style::default().fg(Color::Green).bg(Color::Rgb(0, 50, 0)),
                        )
                    } else {
                        Span::styled(t.text, Style::default().fg(Color::Green))
                    }
                })
                .collect();
            if spans.is_empty() {
                lines.push(Line::from(Span::styled(
                    "+",
                    Style::default().fg(Color::Green),
                )));
            } else {
                let mut combined = vec![Span::styled("+", Style::default().fg(Color::Green))];
                combined.extend(spans);
                lines.push(Line::from(combined));
            }
        } else if let Some(content) = line.strip_prefix('-') {
            let next_added = find_next_added_line(diff_lines, line_idx, &pair_maps);
            let tokens = lcs::diff_tokens_removed(content, next_added);
            let spans: Vec<Span> = tokens
                .into_iter()
                .map(|t| {
                    if t.changed {
                        Span::styled(
                            t.text,
                            Style::default().fg(Color::Red).bg(Color::Rgb(50, 0, 0)),
                        )
                    } else {
                        Span::styled(t.text, Style::default().fg(Color::Red))
                    }
                })
                .collect();
            if spans.is_empty() {
                lines.push(Line::from(Span::styled(
                    "-",
                    Style::default().fg(Color::Red),
                )));
            } else {
                let mut combined = vec![Span::styled("-", Style::default().fg(Color::Red))];
                combined.extend(spans);
                lines.push(Line::from(combined));
            }
        } else {
            lines.push(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(Color::Gray),
            )));
        }
    }

    lines
}

/// Pre-computed O(1) lookup maps for diff line pairing.
/// Maps each `+` line index to its paired `-` line index, and vice versa.
#[derive(Debug, Default)]
struct DiffPairMaps {
    added_to_removed: HashMap<usize, usize>,
    removed_to_added: HashMap<usize, usize>,
}

/// Build diff line pairing maps for LCS matching.
/// Called once per diff render — O(n) scan over all lines.
fn build_pair_maps(diff_lines: &[String]) -> DiffPairMaps {
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
fn find_prev_removed_line<'a>(
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
fn find_next_added_line<'a>(
    diff_lines: &'a [String],
    removed_idx: usize,
    maps: &DiffPairMaps,
) -> Option<&'a str> {
    maps.removed_to_added
        .get(&removed_idx)
        .and_then(|&added_idx| diff_lines.get(added_idx))
        .map(|line| &line[1..])
}

/// Build the metadata display lines for a commit.
pub fn build_metadata_lines<'a>(commit_info: &'a CommitInfo) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let label_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);
    let subject_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let value_style = Style::default();

    // Subject
    lines.push(Line::from(vec![
        Span::styled("Subject:       ", label_style),
        Span::styled(commit_info.subject.clone(), subject_style),
    ]));

    // Hash
    lines.push(Line::from(vec![
        Span::styled("Hash:          ", label_style),
        Span::styled(&commit_info.hash, value_style),
    ]));

    // Parents
    let parents_text = if commit_info.parents.is_empty() {
        "\u{2014}".to_string()
    } else {
        commit_info.parents.join(" ")
    };
    lines.push(Line::from(vec![
        Span::styled("Parents:       ", label_style),
        Span::styled(parents_text, value_style),
    ]));

    // Author
    lines.push(Line::from(vec![
        Span::styled("Author:        ", label_style),
        Span::styled(
            format!("{} <{}>", commit_info.author_name, commit_info.author_email),
            value_style,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Author date:   ", label_style),
        Span::styled(&commit_info.author_date, value_style),
    ]));

    // Committer (only if different from author)
    if commit_info.committer_name != commit_info.author_name
        || commit_info.committer_email != commit_info.author_email
    {
        lines.push(Line::from(vec![
            Span::styled("Committer:     ", label_style),
            Span::styled(
                format!(
                    "{} <{}>",
                    commit_info.committer_name, commit_info.committer_email
                ),
                value_style,
            ),
        ]));
    }
    if commit_info.committer_date != commit_info.author_date
        && !commit_info.committer_date.is_empty()
    {
        lines.push(Line::from(vec![
            Span::styled("Committer date:", label_style),
            Span::styled(&commit_info.committer_date, value_style),
        ]));
    }

    lines.push(Line::from(""));
    lines
}

/// Calculate the offset of the first diff line in the rendered output
/// (metadata lines + file header lines + separator).
pub fn diff_line_offset(commit_info: Option<&CommitInfo>, file_entries: &[FileEntry]) -> usize {
    let mut offset = 0;
    if let Some(info) = commit_info {
        offset += build_metadata_lines(info).len();
    }
    if !file_entries.is_empty() {
        offset += 1; // "Changed files:" header
        offset += file_entries.len(); // file entries
        offset += 1; // separator blank line
    }
    offset
}

/// Format a renamed path diff like `app/commands/{belege → kreditor}/test_command.rb`
fn diff_paths(old: &str, new: &str) -> String {
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
        // Context line clears stacks — no pairs formed
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
