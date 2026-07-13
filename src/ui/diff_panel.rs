use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::time::Instant;

use crate::diff_pairing::{
    build_pair_maps, diff_paths, find_next_added_line, find_prev_removed_line,
};
use crate::domain::{CommitInfo, FileEntry};
use crate::lcs;
use crate::state::diff::DiffState;
use crate::theme::Theme;
use crate::ui::layout::PANEL_BORDER_H;
use crate::ui::scrollbar_view::ScrollbarView;
use crate::view::Panel;

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

    let all_lines = build_all_lines(
        ctx.commit_info,
        ctx.diff_lines,
        ctx.file_entries,
        ctx.selected_file_index,
        ctx.theme,
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

/// Build the complete display: metadata + file list header + file entries + a gap + diff content.
fn build_all_lines<'a>(
    commit_info: Option<&'a CommitInfo>,
    diff_lines: &'a [String],
    file_entries: &'a [FileEntry],
    selected_file_index: usize,
    theme: &Theme,
) -> Vec<Line<'a>> {
    let t0 = Instant::now();
    let total_lines = diff_lines.len();
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

    let metadata_len = lines.len();

    // Changed files header + entries
    if !file_entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "Changed files:",
            Style::default()
                .fg(theme.diff_hunk_header)
                .add_modifier(Modifier::BOLD),
        )));

        for (i, entry) in file_entries.iter().enumerate() {
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
            let tokens = lcs::diff_tokens_added(content, prev_removed);
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
            if spans.is_empty() {
                lines.push(Line::from(Span::styled(
                    "+",
                    Style::default().fg(theme.diff_added),
                )));
            } else {
                let mut combined = vec![Span::styled("+", Style::default().fg(theme.diff_added))];
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
                            Style::default()
                                .fg(theme.diff_removed)
                                .bg(theme.diff_removed_bg),
                        )
                    } else {
                        Span::styled(t.text, Style::default().fg(theme.diff_removed))
                    }
                })
                .collect();
            if spans.is_empty() {
                lines.push(Line::from(Span::styled(
                    "-",
                    Style::default().fg(theme.diff_removed),
                )));
            } else {
                let mut combined = vec![Span::styled("-", Style::default().fg(theme.diff_removed))];
                combined.extend(spans);
                lines.push(Line::from(combined));
            }
        } else {
            lines.push(Line::from(Span::styled(
                line.clone(),
                Style::default().fg(theme.diff_context),
            )));
        }
    }

    tracing::debug!(
        "build_all_lines: {} lines in {}ms",
        total_lines,
        t0.elapsed().as_millis()
    );

    lines
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

/// Count metadata lines without theme dependency (for offset calculations).
pub fn count_metadata_lines(commit_info: &CommitInfo) -> usize {
    let mut count = 5;
    if commit_info.committer_name != commit_info.author_name
        || commit_info.committer_email != commit_info.author_email
    {
        count += 1;
    }
    if commit_info.committer_date != commit_info.author_date
        && !commit_info.committer_date.is_empty()
    {
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
            },
            FileEntry {
                name: "b.rs".to_string(),
                diff_line: 5,
                status: '+',
                old_name: None,
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
        }];
        assert_eq!(diff_line_offset(None, files), 3);
    }
}

use crate::app::commands::Command;
use crate::ui::panel::{self as panel_mod};
use crate::ui::render_ctx::RenderCtx;

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
        let past_meta = state.diff_scroll >= file_section_end || state.file_entries.is_empty();

        match key.code {
            KeyCode::Up => {
                if past_meta {
                    vec![Command::ScrollDiff(-1)]
                } else {
                    vec![Command::MoveUp]
                }
            }
            KeyCode::Down => {
                if past_meta {
                    vec![Command::ScrollDiff(1)]
                } else {
                    vec![Command::MoveDown]
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

    fn help_keys(&self) -> &[(&str, &str)] {
        &[
            ("↑↓/j,k", "scroll/files"),
            ("n/p", "next/prev file"),
            ("Enter", "jump to file"),
            ("Esc", "back to commits"),
            ("Home/End", "top/bottom"),
            ("PgUp/PgDn", "page"),
        ]
    }

    fn label(&self) -> &str {
        "Diff"
    }
}
