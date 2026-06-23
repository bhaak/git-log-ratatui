use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::lcs;
use crate::models::*;

/// Render the diff panel with commit metadata, changed files, and colored diff.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    _commit_info: Option<&CommitInfo>,
    diff_lines: &[String],
    file_entries: &[FileEntry],
    _file_lines: &[String],
    selected_file_index: usize,
    diff_scroll: usize,
    is_focused: bool,
) {
    let border_style = if is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    let total_lines = build_diff_display(diff_lines, file_entries, diff_scroll, selected_file_index);
    let displayed = total_lines.len();

    let scroll_info = if diff_lines.is_empty() {
        String::new()
    } else {
        let visible = area.height.saturating_sub(2) as usize;
        let total = displayed;
        let start = diff_scroll + 1;
        let end = (diff_scroll + visible).min(total);
        format!(" lines {}-{}/{} ", start, end, total)
    };

    let paragraph = Paragraph::new(total_lines)
        .block(
            Block::default()
                .title(format!(" Diff{}", scroll_info))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .scroll((diff_scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

/// Build the full diff display lines: metadata, files, and diff content.
fn build_diff_display(
    diff_lines: &[String],
    file_entries: &[FileEntry],
    _diff_scroll: usize,
    selected_file_index: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if diff_lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "Select a commit to view diff.",
            Style::default().fg(Color::DarkGray),
        )));
        return lines;
    }

    // Metadata is built separately by the caller
    // Changed files header
    if !file_entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "Changed files:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));

        for (i, entry) in file_entries.iter().enumerate() {
            let style = if i == selected_file_index {
                Style::default()
                    .bg(Color::White)
                    .fg(Color::Black)
            } else {
                Style::default().fg(Color::Rgb(100, 150, 255))
            };
            lines.push(Line::from(Span::styled(
                format!("  {}", entry.name),
                style,
            )));
        }

        lines.push(Line::from(""));
    }

    // Diff content with word-level highlighting
    let mut prev_removed: Option<String> = None;
    for line in diff_lines {
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
        } else if line.starts_with('+') {
            let content = &line[1..];
            let tokens = lcs::diff_tokens_added(content, prev_removed.as_deref());
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
            prev_removed = None;
        } else if line.starts_with('-') {
            let content = &line[1..];
            prev_removed = Some(content.to_string());
            let tokens = lcs::diff_tokens_removed(content, None);
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
            prev_removed = None;
        }
    }

    lines
}

/// Build the metadata display lines for a commit.
pub fn build_metadata_lines(commit_info: &CommitInfo) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let label_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);
    let value_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

    // Subject
    lines.push(Line::from(Span::styled(
        commit_info.subject.clone(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Hash
    lines.push(Line::from(vec![
        Span::styled("Hash:       ", label_style),
        Span::styled(&commit_info.hash, value_style),
    ]));

    // Parents
    let parents_text = if commit_info.parents.is_empty() {
        "—".to_string()
    } else {
        commit_info.parents.join(" ")
    };
    lines.push(Line::from(vec![
        Span::styled("Parents:    ", label_style),
        Span::styled(parents_text, dim_style),
    ]));

    // Author
    lines.push(Line::from(vec![
        Span::styled("Author:     ", label_style),
        Span::styled(
            format!("{} <{}>", commit_info.author_name, commit_info.author_email),
            value_style,
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Date:       ", label_style),
        Span::styled(&commit_info.author_date, value_style),
    ]));

    // Committer (only if different from author)
    if commit_info.committer_name != commit_info.author_name
        || commit_info.committer_email != commit_info.author_email
    {
        lines.push(Line::from(vec![
            Span::styled("Committer:  ", label_style),
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
            Span::styled("Comm. Date: ", label_style),
            Span::styled(&commit_info.committer_date, value_style),
        ]));
    }

    lines.push(Line::from(""));
    lines
}

/// Build formatted file entry lines for the file list.
pub fn build_file_lines(file_entries: &[FileEntry]) -> Vec<String> {
    file_entries
        .iter()
        .map(|e| format!("  {}", e.name))
        .collect()
}
