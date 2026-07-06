use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::diff_pairing::{
    build_pair_maps, diff_paths, find_next_added_line, find_prev_removed_line,
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
    /// Optional debug frame timing label shown in the panel title.
    pub debug_label: Option<&'a str>,
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

    let debug_part = if let Some(label) = ctx.debug_label {
        format!(" [{}]", label)
    } else {
        String::new()
    };

    let scroll_info = if total > 0 {
        format!(" lines {}-{}/{} ", start, end, total)
    } else {
        String::new()
    };

    let paragraph = Paragraph::new(all_lines)
        .block(
            Block::default()
                .title(format!("{}{}{}", diff_title, debug_part, scroll_info))
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
    // Build pair maps once -- O(n) scan, then O(1) lookup per line
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info(subject: &str, hash: &str) -> CommitInfo {
        CommitInfo {
            subject: subject.to_string(),
            hash: hash.to_string(),
            parents: vec![],
            author_name: "Author".to_string(),
            author_email: "a@b.com".to_string(),
            author_date: "2024-01-01".to_string(),
            committer_name: "Author".to_string(),
            committer_email: "a@b.com".to_string(),
            committer_date: String::new(),
        }
    }

    fn make_info_with_committer() -> CommitInfo {
        CommitInfo {
            subject: "Merge".to_string(),
            hash: "abcdef".to_string(),
            parents: vec!["parent1".to_string(), "parent2".to_string()],
            author_name: "Author".to_string(),
            author_email: "a@b.com".to_string(),
            author_date: "2024-01-01".to_string(),
            committer_name: "Committer".to_string(),
            committer_email: "c@d.com".to_string(),
            committer_date: "2024-06-15".to_string(),
        }
    }

    // --- build_metadata_lines ---

    #[test]
    fn test_build_metadata_lines_basic() {
        let info = make_info("Hello world", "abc123");
        let lines = build_metadata_lines(&info);

        // Should have: subject, hash, parents (—), author, author date, blank line = 6 lines
        assert_eq!(lines.len(), 6);

        // Subject line
        let subject_span = &lines[0].spans[1];
        assert_eq!(subject_span.content, "Hello world");

        // Hash line
        let hash_span = &lines[1].spans[1];
        assert_eq!(hash_span.content, "abc123");

        // Parents (empty → em dash)
        let parents_span = &lines[2].spans[1];
        assert_eq!(parents_span.content, "\u{2014}");

        // Author
        let author_span = &lines[3].spans[1];
        assert_eq!(author_span.content, "Author <a@b.com>");
    }

    #[test]
    fn test_build_metadata_lines_with_committer() {
        let info = make_info_with_committer();
        let lines = build_metadata_lines(&info);

        // Should include committer info (different from author)
        assert!(lines.len() >= 8);

        // Parents should be joined with space
        let parents_span = &lines[2].spans[1];
        assert_eq!(parents_span.content, "parent1 parent2");

        // Find committer line
        let has_committer = lines
            .iter()
            .any(|l| l.spans.len() > 1 && l.spans[1].content.contains("Committer <c@d.com>"));
        assert!(has_committer, "should have committer line");

        let has_committer_date = lines
            .iter()
            .any(|l| l.spans.len() > 1 && l.spans[1].content.contains("2024-06-15"));
        assert!(has_committer_date, "should have committer date line");
    }

    #[test]
    fn test_build_metadata_lines_no_committer_when_same() {
        let info = make_info("test", "hash");
        let lines = build_metadata_lines(&info);
        // Author and committer are the same, committer date is empty — no extra lines
        let has_committer = lines
            .iter()
            .any(|l| l.spans.len() > 0 && l.spans[0].content.contains("Committer"));
        assert!(
            !has_committer,
            "should not show committer when same as author"
        );
    }

    // --- diff_line_offset ---

    #[test]
    fn test_diff_line_offset_no_info_no_files() {
        assert_eq!(diff_line_offset(None, &[]), 0);
    }

    #[test]
    fn test_diff_line_offset_with_info_only() {
        let info = make_info("test", "hash");
        let expected = build_metadata_lines(&info).len();
        assert_eq!(diff_line_offset(Some(&info), &[]), expected);
    }

    #[test]
    fn test_diff_line_offset_with_files() {
        let info = make_info("test", "hash");
        let files = &[
            FileEntry {
                name: "a.rs".to_string(),
                diff_line: 0,
                status: '~',
                old_name: None,
            },
            FileEntry {
                name: "b.rs".to_string(),
                diff_line: 5,
                status: '+',
                old_name: None,
            },
        ];
        let metadata_len = build_metadata_lines(&info).len();
        let expected = metadata_len + 1 + 2 + 1; // header + 2 files + blank
        assert_eq!(diff_line_offset(Some(&info), files), expected);
    }

    #[test]
    fn test_diff_line_offset_files_only() {
        let files = &[FileEntry {
            name: "a.rs".to_string(),
            diff_line: 0,
            status: '~',
            old_name: None,
        }];
        assert_eq!(diff_line_offset(None, files), 3); // header + 1 file + blank
    }
}

use crate::app::state::AppState;
use crate::ui::panel::Panel;

/// Wrapper struct implementing the Panel trait for the diff view.
pub struct DiffPanel;

impl Panel for DiffPanel {
    fn render(&self, area: Rect, frame: &mut Frame, state: &AppState, is_focused: bool) {
        let short_hash = state.commit_info.as_ref().map(|info| {
            &info.hash[..std::cmp::min(crate::ui::commit_table::SHORT_HASH_LEN, info.hash.len())]
        });
        let ctx = DiffPanelCtx {
            commit_info: state.commit_info.as_ref(),
            diff_lines: &state.diff_lines,
            file_entries: &state.file_entries,
            selected_file_index: state.selected_file_index,
            diff_scroll: state.diff_scroll,
            is_focused,
            short_hash,
            debug_label: None,
        };
        let _ = render(frame, area, &ctx);
    }

    fn help_keys(&self) -> &[(&str, &str)] {
        &[
            ("↑↓/j,k", "scroll/navigate files"),
            ("n/p", "next/prev file"),
            ("Enter", "jump to file diff"),
            ("Home/End", "top/bottom"),
            ("PgUp/PgDn", "page"),
        ]
    }

    fn label(&self) -> &str {
        "Diff"
    }
}
