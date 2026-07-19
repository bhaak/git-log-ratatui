use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::time::Instant;

use crate::app::commands::Command;
use crate::diff_pairing::{
    build_pair_maps, diff_paths, find_next_added_line, find_prev_removed_line,
};
use crate::domain::{CommitInfo, FileEntry};
use crate::lcs;
use crate::state::diff::DiffState;
use crate::theme::Theme;
use crate::ui::layout::PANEL_BORDER_H;
use crate::ui::panel::KeyBinding;
use crate::ui::panel::{self as panel_mod};
use crate::ui::render_ctx::RenderCtx;
use crate::ui::scrollbar_view::ScrollbarView;
use crate::view::Panel;

/// Maximum characters for proportional visual indicators in the file list.
/// Cap avoids extreme rendering cost with pathological +999999 line diffs.
const MAX_VISUAL_INDICATOR_CHARS: u16 = 200;

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
    /// Color theme.
    pub theme: &'a Theme,
}

/// Render the diff panel with commit metadata, changed files, and colored diff.
/// Returns the total number of display lines.
pub fn render(frame: &mut Frame, area: Rect, ctx: &DiffPanelCtx) -> usize {
    if area.width < 4 || area.height < 2 {
        return 0;
    }

    let border_style = if ctx.is_focused {
        Style::default().fg(ctx.theme.focused_border)
    } else {
        Style::default().fg(ctx.theme.unfocused_border)
    };

    let content_width = area.width.saturating_sub(PANEL_BORDER_H);
    let all_lines = build_all_lines(
        ctx.commit_info,
        ctx.diff_lines,
        ctx.file_entries,
        ctx.selected_file_index,
        ctx.theme,
        content_width,
    );
    let total = all_lines.len();
    let visible = area.height.saturating_sub(PANEL_BORDER_H) as usize;
    let start = (ctx.diff_scroll.saturating_add(1)).min(total);
    let end = (ctx.diff_scroll.saturating_add(visible)).min(total);

    // Slice the content to the visible window instead of using Paragraph scroll.
    // This avoids ratatui's internal u16 arithmetic with potentially large scroll values.
    let visible_lines: Vec<Line> = all_lines
        .into_iter()
        .skip(ctx.diff_scroll.min(total))
        .take(visible)
        .collect();

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

    let paragraph = Paragraph::new(visible_lines).block(
        Block::default()
            .title(format!("{}{}{}", diff_title, debug_part, scroll_info))
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    frame.render_widget(paragraph, area);
    total
}

/// The kind of a diff line: added (+) or removed (-).
enum DiffLineKind {
    Added,
    Removed,
}

/// Build a `Line` with word-level LCS highlighting for one diff side.
/// `context` is the paired line from the other side (previous removed for added,
/// next added for removed), used by the LCS tokenizer to pinpoint changes.
fn build_diff_line_spans<'a>(
    content: &'a str,
    context: Option<&'a str>,
    kind: DiffLineKind,
    theme: &Theme,
) -> Line<'a> {
    match kind {
        DiffLineKind::Added => {
            let tokens = lcs::diff_tokens_added(content, context);
            let spans: Vec<Span> = tokens
                .into_iter()
                .map(|t| {
                    if t.changed {
                        Span::styled(
                            t.text,
                            Style::default()
                                .fg(theme.diff_added)
                                .bg(theme.diff_added_bg),
                        )
                    } else {
                        Span::styled(t.text, Style::default().fg(theme.diff_added))
                    }
                })
                .collect();
            finish_line(spans, '+', theme.diff_added)
        }
        DiffLineKind::Removed => {
            let tokens = lcs::diff_tokens_removed(content, context);
            let spans: Vec<Span> = tokens
                .into_iter()
                .map(|t| {
                    if t.changed {
                        Span::styled(
                            t.text,
                            Style::default()
                                .fg(theme.diff_removed)
                                .bg(theme.diff_removed_bg),
                        )
                    } else {
                        Span::styled(t.text, Style::default().fg(theme.diff_removed))
                    }
                })
                .collect();
            finish_line(spans, '-', theme.diff_removed)
        }
    }
}

/// Build the final Line from token spans plus the diff prefix character.
fn finish_line(spans: Vec<Span<'_>>, prefix: char, fg: ratatui::style::Color) -> Line<'_> {
    let prefix_style = Style::default().fg(fg);
    if spans.is_empty() {
        Line::from(Span::styled(prefix.to_string(), prefix_style))
    } else {
        let mut combined = vec![Span::styled(prefix.to_string(), prefix_style)];
        combined.extend(spans);
        Line::from(combined)
    }
}

/// Build the complete display: metadata + file list header + file entries + a gap + diff content.
/// `content_width` is the available text area width (panel width minus borders).
fn build_all_lines<'a>(
    commit_info: Option<&'a CommitInfo>,
    diff_lines: &'a [String],
    file_entries: &'a [FileEntry],
    selected_file_index: usize,
    theme: &Theme,
    content_width: u16,
) -> Vec<Line<'a>> {
    let t0 = Instant::now();
    let mut lines = Vec::new();

    if diff_lines.is_empty() {
        if commit_info.is_some() {
            lines.push(Line::from(Span::styled(
                "No changes in this commit.",
                Style::default().fg(theme.commit_secondary),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "Select a commit to view diff.",
                Style::default().fg(theme.commit_secondary),
            )));
        }
        return lines;
    }

    // Commit metadata
    if let Some(info) = commit_info {
        lines.extend(build_metadata_lines(info, theme));
    }

    // Changed files header + entries
    if !file_entries.is_empty() {
        lines.extend(build_file_entries_lines(
            file_entries,
            selected_file_index,
            content_width,
            theme,
        ));
    }

    // Diff content with word-level highlighting
    lines.extend(build_diff_content_lines(diff_lines, theme));

    tracing::debug!(
        "build_all_lines: {} lines in {}ms",
        diff_lines.len(),
        t0.elapsed().as_millis()
    );

    lines
}

/// Build the "Changed files:" section with per-file status, name, line-count
/// summary, and proportional visual indicators.
fn build_file_entries_lines<'a>(
    file_entries: &'a [FileEntry],
    selected_file_index: usize,
    content_width: u16,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "Changed files:",
        Style::default()
            .fg(theme.diff_hunk_header)
            .add_modifier(Modifier::BOLD),
    )));

    // Compute display names and max width for alignment.
    let display_names: Vec<String> = file_entries
        .iter()
        .map(|entry| {
            if let Some(ref old) = entry.old_name {
                diff_paths(old, &entry.name)
            } else {
                entry.name.clone()
            }
        })
        .collect();
    let max_name_width = display_names.iter().map(|n| n.len()).max().unwrap_or(0);
    // Compute max field width for sign+number alignment, per column.
    let added_strs: Vec<String> = file_entries
        .iter()
        .map(|e| format!("+{}", e.lines_added))
        .collect();
    let removed_strs: Vec<String> = file_entries
        .iter()
        .map(|e| format!("-{}", e.lines_removed))
        .collect();
    let added_width = added_strs.iter().map(|s| s.len()).max().unwrap_or(1);
    let removed_width = removed_strs.iter().map(|s| s.len()).max().unwrap_or(1);
    // Minimum column width of 2 ("+0", "-0") so the sign always has room.
    let added_width = added_width.max(2);
    let removed_width = removed_width.max(2);

    // Compute max visual indicator characters based on available content width.
    // Line format: "<status> <padded_name> <(+N/-M)><space><indicators>  "
    // Fixed width: 1(status) + 1(space) + name + 1(space) + count + 1(space) = 4 + name + count
    // Indicators get the rest, minus 2 chars for right margin.
    let count_width = added_width + removed_width + 3; // "(+N/-M)"
    let fixed_prefix = max_name_width + count_width + 4;
    let max_vis = content_width
        .saturating_sub(fixed_prefix as u16)
        .min(MAX_VISUAL_INDICATOR_CHARS) as usize;

    for (i, entry) in file_entries.iter().enumerate() {
        lines.push(build_file_entry_line(
            entry,
            i,
            selected_file_index,
            &display_names[i],
            &added_strs[i],
            &removed_strs[i],
            max_name_width,
            added_width,
            removed_width,
            max_vis,
            theme,
        ));
    }

    lines.push(Line::from(""));
    lines
}

/// Build a single file entry line with status, padded name, line-count summary,
/// and proportionally scaled visual indicators.
#[allow(clippy::too_many_arguments)]
fn build_file_entry_line<'a>(
    entry: &FileEntry,
    i: usize,
    selected_file_index: usize,
    display_name: &str,
    added_str: &str,
    removed_str: &str,
    max_name_width: usize,
    added_width: usize,
    removed_width: usize,
    max_vis: usize,
    theme: &Theme,
) -> Line<'a> {
    let selected = i == selected_file_index;
    let status_color = match entry.status {
        '+' => theme.diff_added,
        '-' => theme.diff_removed,
        '~' => theme.diff_modified,
        '→' => theme.diff_renamed,
        _ => theme.diff_context,
    };
    let name_style = if selected {
        Style::default()
            .bg(theme.diff_selected_file_bg)
            .fg(theme.diff_selected_file_fg)
    } else {
        Style::default().fg(theme.diff_selected_file_border)
    };
    let status_style = Style::default().fg(status_color);
    let padded_name = format!("{:<width$}", display_name, width = max_name_width);
    let count_style = Style::default().fg(theme.diff_context);
    let added_style = Style::default().fg(theme.diff_added);
    let removed_style = Style::default().fg(theme.diff_removed);
    let available = max_vis.saturating_sub(2);
    let total = entry.lines_added + entry.lines_removed;
    let (pluses, minuses) = if total == 0 {
        (0, 0)
    } else if total <= available {
        (entry.lines_added, entry.lines_removed)
    } else {
        let p = (available * entry.lines_added) / total;
        (p, available - p)
    };
    let mut spans = vec![
        Span::styled(format!("{} ", entry.status), status_style),
        Span::styled(padded_name, name_style),
        Span::styled(" ", count_style),
        Span::styled(
            format!(
                "({:>aw$}/{:>rw$})",
                added_str,
                removed_str,
                aw = added_width,
                rw = removed_width
            ),
            count_style,
        ),
    ];
    if pluses + minuses > 0 {
        spans.push(Span::styled(" ", count_style));
        spans.push(Span::styled("+".repeat(pluses), added_style));
        spans.push(Span::styled("-".repeat(minuses), removed_style));
    }
    Line::from(spans)
}

/// Build the diff content section with per-line coloring based on git diff syntax.
/// Uses pair-maps and LCS for word-level intra-line change highlighting.
fn build_diff_content_lines<'a>(diff_lines: &'a [String], theme: &Theme) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    // Build pair maps once — O(n) scan, then O(1) lookup per line
    let pair_maps = build_pair_maps(diff_lines);

    for (line_idx, line) in diff_lines.iter().enumerate() {
        if line.starts_with("@@") {
            lines.push(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(theme.diff_hunk_header),
            )));
        } else if line.starts_with("diff --git")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
        {
            lines.push(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(theme.diff_file_header),
            )));
        } else if let Some(content) = line.strip_prefix('+') {
            let prev_removed = find_prev_removed_line(diff_lines, line_idx, &pair_maps);
            lines.push(build_diff_line_spans(
                content,
                prev_removed,
                DiffLineKind::Added,
                theme,
            ));
        } else if let Some(content) = line.strip_prefix('-') {
            let next_added = find_next_added_line(diff_lines, line_idx, &pair_maps);
            lines.push(build_diff_line_spans(
                content,
                next_added,
                DiffLineKind::Removed,
                theme,
            ));
        } else {
            lines.push(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(theme.diff_context),
            )));
        }
    }
    lines
}

/// Returns true when the committer differs from the author.
fn has_different_committer(info: &CommitInfo) -> bool {
    info.committer_name != info.author_name || info.committer_email != info.author_email
}

/// Returns true when the committer date differs from the author date and is non-empty.
fn has_different_committer_date(info: &CommitInfo) -> bool {
    info.committer_date != info.author_date && !info.committer_date.is_empty()
}

/// Build the metadata display lines for a commit.
pub fn build_metadata_lines<'a>(commit_info: &'a CommitInfo, theme: &Theme) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let label_style = Style::default()
        .fg(theme.commit_secondary)
        .add_modifier(Modifier::BOLD);
    let subject_style = Style::default()
        .fg(theme.commit_default)
        .add_modifier(Modifier::BOLD);
    let value_style = Style::default();

    lines.push(Line::from(vec![
        Span::styled("Subject:       ", label_style),
        Span::styled(commit_info.subject.clone(), subject_style),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Hash:          ", label_style),
        Span::styled(&commit_info.hash, value_style),
    ]));

    let parents_text = if commit_info.parents.is_empty() {
        "\u{2014}".to_string()
    } else {
        commit_info.parents.join(" ")
    };
    lines.push(Line::from(vec![
        Span::styled("Parents:       ", label_style),
        Span::styled(parents_text, value_style),
    ]));

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

    if has_different_committer(commit_info) {
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
    if has_different_committer_date(commit_info) {
        lines.push(Line::from(vec![
            Span::styled("Committer date:", label_style),
            Span::styled(&commit_info.committer_date, value_style),
        ]));
    }

    lines.push(Line::from(""));
    lines
}

/// Count metadata lines without theme dependency (for offset calculations).
pub fn count_metadata_lines(commit_info: &CommitInfo) -> usize {
    let mut count = 5;
    if has_different_committer(commit_info) {
        count += 1;
    }
    if has_different_committer_date(commit_info) {
        count += 1;
    }
    count + 1
}

/// Calculate the offset of the first diff line in the rendered output
/// (metadata lines + file header lines + separator).
/// Uses `count_metadata_lines` which does not depend on theme.
pub fn diff_line_offset(commit_info: Option<&CommitInfo>, file_entries: &[FileEntry]) -> usize {
    let mut offset = 0;
    if let Some(info) = commit_info {
        offset += count_metadata_lines(info);
    }
    if !file_entries.is_empty() {
        offset += 1;
        offset += file_entries.len();
        offset += 1;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_theme() -> Theme {
        Theme::default()
    }

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

    #[test]
    fn test_build_metadata_lines_basic() {
        let theme = make_theme();
        let info = make_info("Hello world", "abc123");
        let lines = build_metadata_lines(&info, &theme);

        assert_eq!(lines.len(), 6);

        let subject_span = &lines[0].spans[1];
        assert_eq!(subject_span.content, "Hello world");

        let hash_span = &lines[1].spans[1];
        assert_eq!(hash_span.content, "abc123");

        let parents_span = &lines[2].spans[1];
        assert_eq!(parents_span.content, "\u{2014}");

        let author_span = &lines[3].spans[1];
        assert_eq!(author_span.content, "Author <a@b.com>");
    }

    #[test]
    fn test_build_metadata_lines_with_committer() {
        let theme = make_theme();
        let info = make_info_with_committer();
        let lines = build_metadata_lines(&info, &theme);

        assert!(lines.len() >= 8);

        let parents_span = &lines[2].spans[1];
        assert_eq!(parents_span.content, "parent1 parent2");

        let has_committer = lines
            .iter()
            .any(|l| !l.spans.is_empty() && l.spans[1].content.contains("Committer <c@d.com>"));
        assert!(has_committer, "should have committer line");

        let has_committer_date = lines
            .iter()
            .any(|l| !l.spans.is_empty() && l.spans[1].content.contains("2024-06-15"));
        assert!(has_committer_date, "should have committer date line");
    }

    #[test]
    fn test_build_metadata_lines_no_committer_when_same() {
        let theme = make_theme();
        let info = make_info("test", "hash");
        let lines = build_metadata_lines(&info, &theme);
        let has_committer = lines
            .iter()
            .any(|l| !l.spans.is_empty() && l.spans[0].content.contains("Committer"));
        assert!(
            !has_committer,
            "should not show committer when same as author"
        );
    }

    #[test]
    fn test_count_metadata_lines_basic() {
        let info = make_info("test", "hash");
        assert_eq!(count_metadata_lines(&info), 6);
    }

    #[test]
    fn test_count_metadata_lines_with_committer() {
        let info = make_info_with_committer();
        assert_eq!(count_metadata_lines(&info), 8);
    }

    #[test]
    fn test_diff_line_offset_no_info_no_files() {
        assert_eq!(diff_line_offset(None, &[]), 0);
    }

    #[test]
    fn test_diff_line_offset_with_info_only() {
        let info = make_info("test", "hash");
        assert_eq!(diff_line_offset(Some(&info), &[]), 6);
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
                lines_added: 5,
                lines_removed: 2,
            },
            FileEntry {
                name: "b.rs".to_string(),
                diff_line: 5,
                status: '+',
                old_name: None,
                lines_added: 10,
                lines_removed: 0,
            },
        ];
        assert_eq!(diff_line_offset(Some(&info), files), 10);
    }

    #[test]
    fn test_diff_line_offset_files_only() {
        let files = &[FileEntry {
            name: "a.rs".to_string(),
            diff_line: 0,
            status: '~',
            old_name: None,
            lines_added: 3,
            lines_removed: 1,
        }];
        assert_eq!(diff_line_offset(None, files), 3);
    }

    #[test]
    fn test_file_list_shows_line_counts_aligned() {
        let theme = make_theme();
        let diff_lines: Vec<String> = vec![
            "diff --git a/a.rs b/a.rs".into(),
            "@@ -1,1 +1,1 @@".into(),
            "-old".into(),
            "+new".into(),
        ];
        let files = &[
            FileEntry {
                name: "short.rs".to_string(),
                diff_line: 0,
                status: '~',
                old_name: None,
                lines_added: 95,
                lines_removed: 72,
            },
            FileEntry {
                name: "longer_name.rs".to_string(),
                diff_line: 2,
                status: '+',
                old_name: None,
                lines_added: 1,
                lines_removed: 0,
            },
        ];
        let lines = build_all_lines(None, &diff_lines, files, 0, &theme, 80);

        // lines[0] = "Changed files:", lines[1,2] = file entries, lines[3] = "", lines[4..] = diff
        let file_0 = &lines[1];
        let text_0: String = file_0.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text_0.contains("short.rs"),
            "should contain file name, got: {text_0}"
        );
        assert!(
            text_0.contains("(+95/-72)"),
            "should contain count for +95/-72, got: {text_0}"
        );
        // With content_width=80: max_vis=53, available=51. +95/-72 scaled = ~29/22.
        assert!(
            text_0.contains("+++++++++++++++++++++++++++++"),
            "should contain scaled plus visual indicator, got: {text_0}"
        );
        assert!(
            text_0.contains("----------------------"),
            "should contain scaled minus visual indicator, got: {text_0}"
        );

        let file_1 = &lines[2];
        let text_1: String = file_1.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text_1.contains("longer_name.rs"),
            "should contain file name, got: {text_1}"
        );
        assert!(
            text_1.contains("( +1/ -0)"),
            "should contain right-aligned count for 1/0, got: {text_1}"
        );
        // Verify alignment: "short.rs" is 8 chars, "longer_name.rs" is 14 chars.
        // Both should be padded to 14 so parens start at same column.
        let pos0 = text_0.find("(").unwrap();
        let pos1 = text_1.find("(").unwrap();
        assert_eq!(
            pos0, pos1,
            "parentheses should align at column {pos0} vs {pos1}"
        );
    }

    #[test]
    fn test_visual_indicators_truncated_by_content_width() {
        let theme = make_theme();
        let diff_lines: Vec<String> = vec![
            "diff --git a/x b/x".into(),
            "@@ -1 +1 @@".into(),
            "-a".into(),
            "+b".into(),
        ];
        let files = &[FileEntry {
            name: "lib.rs".to_string(),
            diff_line: 0,
            status: '~',
            old_name: None,
            lines_added: 42,
            lines_removed: 7,
        }];

        // With content_width smaller than fixed prefix: max_vis = 0, no visual indicators.
        let narrow = build_all_lines(None, &diff_lines, files, 0, &theme, 17);
        let text_narrow: String = narrow[1].spans.iter().map(|s| s.content.as_ref()).collect();
        // Verify no content after the closing paren of the count.
        let close_paren = text_narrow.rfind(')').unwrap();
        assert_eq!(
            &text_narrow[close_paren + 1..],
            "",
            "no visual indicators after count"
        );

        // With content_width=30: fixed_prefix=18, max_vis=12, available=10 (after 2-char margin).
        // +42/-7 → proportion: 10*42/49=8 pluses, 10-8=2 minuses.
        let wide = build_all_lines(None, &diff_lines, files, 0, &theme, 30);
        let text_wide: String = wide[1].spans.iter().map(|s| s.content.as_ref()).collect();
        let close_paren = text_wide.rfind(')').unwrap();
        let after_count = &text_wide[close_paren + 1..];
        assert_eq!(
            after_count, " ++++++++--",
            "should show proportionally scaled indicators"
        );

        // With plenty of space: all indicators shown, respecting margin.
        let roomy = build_all_lines(None, &diff_lines, files, 0, &theme, 120);
        let text_roomy: String = roomy[1].spans.iter().map(|s| s.content.as_ref()).collect();
        let cp = text_roomy.rfind(')').unwrap();
        assert!(
            text_roomy[cp..].len() <= 118, // content_width=120 minus ~2 margin
            "should leave at least 2 chars margin"
        );
        assert!(
            text_roomy[cp + 1..].starts_with(" +"),
            "should start with space and pluses"
        );
    }
}

/// Wrapper struct implementing the Panel trait for the diff view.
#[allow(clippy::items_after_test_module)]
pub struct DiffPanel;

impl panel_mod::Panel for DiffPanel {
    type State = DiffState;

    fn render(&self, area: Rect, frame: &mut Frame, state: &mut Self::State, ctx: &RenderCtx) {
        let (content_area, scrollbar_area) = ScrollbarView::split(area);

        // Clamp diff_scroll BEFORE rendering so End/PageDown show correct bounds
        // immediately. Uses the previous frame's total_lines which only changes
        // when a new diff is loaded (in which case diff_scroll is already 0).
        let visible = (content_area.height.saturating_sub(PANEL_BORDER_H)) as usize;
        state.diff_scroll = state
            .diff_scroll
            .min(state.prev_total_lines.saturating_sub(visible));

        let short_hash = state.commit_info.as_ref().map(|info| {
            &info.hash[..std::cmp::min(crate::ui::commit_table::SHORT_HASH_LEN, info.hash.len())]
        });
        let diff_ctx = DiffPanelCtx {
            commit_info: state.commit_info.as_ref(),
            diff_lines: &state.diff_lines,
            file_entries: &state.file_entries,
            selected_file_index: state.selected_file_index,
            diff_scroll: state.diff_scroll,
            is_focused: ctx.is_focused(Panel::Diff),
            short_hash,
            debug_label: ctx.debug_label,
            theme: ctx.theme,
        };
        let total_lines = render(frame, content_area, &diff_ctx);
        state.prev_total_lines = total_lines;

        let focus_style = if ctx.is_focused(Panel::Diff) {
            Style::default().fg(ctx.theme.focused_border)
        } else {
            Style::default().fg(ctx.theme.unfocused_border)
        };
        state.scrollbar.render(
            frame,
            scrollbar_area,
            total_lines,
            visible,
            state.diff_scroll,
            focus_style,
        );
    }

    fn handle_event(&mut self, key: &KeyEvent, state: &mut Self::State) -> Vec<Command> {
        let file_section_end = diff_line_offset(state.commit_info.as_ref(), &state.file_entries);
        let viewport = state.scrollbar.viewport_length().max(1);
        // The last file entry sits just before the trailing separator (file_section_end - 2).
        let last_entry_line = file_section_end.saturating_sub(2);
        let last_entry_visible = !state.file_entries.is_empty()
            && state.diff_scroll <= last_entry_line
            && last_entry_line < state.diff_scroll.saturating_add(viewport);

        match key.code {
            // Up / k: scroll until the last file entry is visible, then navigate the file list.
            KeyCode::Up | KeyCode::Char('k') => {
                if last_entry_visible {
                    vec![Command::MoveUp]
                } else {
                    vec![Command::ScrollDiff(-1)]
                }
            }
            // Down / j: navigate the file list while the last entry is visible,
            // scroll diff content otherwise. Also scroll when at the last entry.
            KeyCode::Down | KeyCode::Char('j') => {
                if last_entry_visible
                    && state.selected_file_index < state.file_entries.len().saturating_sub(1)
                {
                    vec![Command::MoveDown]
                } else {
                    vec![Command::ScrollDiff(1)]
                }
            }
            KeyCode::Enter => vec![Command::JumpToDiffFile(state.selected_file_index)],
            KeyCode::Char('n') if !state.file_entries.is_empty() => {
                vec![Command::SelectNextFile]
            }
            KeyCode::Char('p') if !state.file_entries.is_empty() => {
                vec![Command::SelectPrevFile]
            }
            KeyCode::Home => vec![Command::JumpToTop],
            KeyCode::End => vec![Command::JumpToBottom],
            KeyCode::PageUp => {
                vec![Command::PageUp]
            }
            KeyCode::PageDown => {
                vec![Command::PageDown]
            }
            _ => Vec::new(),
        }
    }

    fn help_keys(&self) -> &'static [KeyBinding] {
        static KEYS: &[KeyBinding] = &[
            KeyBinding::new("↑↓/j,k", "scroll/files", "Scroll diff / navigate files"),
            KeyBinding::new("n/p", "next/prev file", "Next / previous changed file"),
            KeyBinding::new("Enter", "jump to file", "Jump to selected file's diff"),
            KeyBinding::new("Esc", "back to commits", "Back to commits panel"),
            KeyBinding::new("Home/End", "top/bottom", "Jump to top / bottom"),
            KeyBinding::new("PgUp/PgDn", "page", "Page up / down"),
        ];
        KEYS
    }

    fn label(&self) -> &str {
        "Diff"
    }
}
