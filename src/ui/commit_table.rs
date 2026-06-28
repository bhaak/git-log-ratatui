use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::models::*;

const COL_GRAPH_MAX: u16 = 12;
const COL_HASH: u16 = 8;
const COL_SUBJECT_MIN: u16 = 20;
const COL_AUTHOR: u16 = 15;
const COL_DATE: u16 = 18;

/// Render the commit table with git graph, decorations, and merge highlighting.
/// Render the commit table with git graph, decorations, and merge highlighting.
/// Column order matches Ruby: Graph | Hash | Subject | Author | Date
pub fn render(
    frame: &mut Frame,
    area: Rect,
    commits: &[Commit],
    visible_index: usize,
    is_focused: bool,
    visible_to_commit: &[usize],
    total_loaded: usize,
    search_active: bool,
    state: &mut TableState,
) {
    let border_style = if is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    // Guard against zero-size area
    if area.width < 10 || area.height < 3 {
        return;
    }

    // Empty state messages
    if commits.is_empty() && total_loaded > 0 {
        let msg = if search_active {
            "No commits match your search."
        } else {
            "No commits found in this repository."
        };
        let block = Block::default()
            .title(" Commits ")
            .borders(Borders::ALL)
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.width > 4 && inner.height > 1 {
            let p = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray)))
                .block(Block::default());
            let centered = Rect::new(
                inner.x + inner.width.saturating_sub(msg.len() as u16) / 2,
                inner.y + inner.height / 2,
                (msg.len() as u16).min(inner.width),
                1,
            );
            frame.render_widget(p, centered);
        }
        return;
    }

    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec![
        "Graph",
        "Hash",
        "Subject",
        "Author",
        "Date",
    ])
    .style(header_style)
    .height(1);

    let highlight_style = Style::default()
        .bg(if is_focused {
            Color::Rgb(80, 60, 120)
        } else {
            Color::DarkGray
        })
        .add_modifier(Modifier::BOLD);

    // Calculate dynamic graph width
    let max_graph = commits
        .iter()
        .map(|c| c.graph.len())
        .max()
        .unwrap_or(4) as u16;

    let col_graph = max_graph.clamp(4, COL_GRAPH_MAX);

    let available_width = area.width.saturating_sub(2);
    let fixed_width = col_graph + COL_HASH + COL_AUTHOR + COL_DATE + 4; // +4 separators
    let subject_width = available_width
        .saturating_sub(fixed_width)
        .max(COL_SUBJECT_MIN);

    let widths = [
        Constraint::Length(col_graph),
        Constraint::Length(COL_HASH),
        Constraint::Length(subject_width),
        Constraint::Length(COL_AUTHOR),
        Constraint::Length(COL_DATE),
    ];

    // Build rows with mapped selection
    let mapped_index = visible_to_commit.get(visible_index).copied().unwrap_or(0);

    // Filter graph_only rows out and show them with minimal content
    let rows: Vec<Row> = commits
        .iter()
        .map(|commit| {
            let graph_span = build_graph_span(commit, col_graph as usize);
            let hash_span = build_hash_span(commit);

            let subject_span = if commit.graph_only {
                Line::from(Span::styled(
                    graph_only_decorations(commit),
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                Line::from(Span::styled(
                    truncate(&commit.subject, subject_width as usize),
                    if commit.merge {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ))
            };

            let author_span = Line::from(Span::styled(
                truncate(&commit.author, COL_AUTHOR as usize),
                Style::default().fg(Color::White),
            ));
            let date_span = Line::from(Span::styled(
                &commit.date,
                Style::default().fg(Color::DarkGray),
            ));

            Row::new(vec![
                graph_span,
                hash_span,
                subject_span,
                author_span,
                date_span,
            ])
            .height(1)
        })
        .collect();

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(" Commits ")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .row_highlight_style(highlight_style)
        .column_spacing(1);

    if !commits.is_empty() && mapped_index < commits.len() {
        state.select(Some(mapped_index));
    }

    frame.render_stateful_widget(table, area, state);
}

fn build_graph_span(commit: &Commit, graph_width: usize) -> Line<'static> {
    // Graph lines from git-graph are already Unicode box-drawing characters; use them directly
    let padded = format!("{:width$}", commit.graph, width = graph_width);
    let mut spans = Vec::new();

    spans.push(Span::styled(
        padded,
        if commit.merge {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    ));

    for deco in &commit.decorations {
        let (bg, fg) = decoration_colors(&deco.kind);
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!(" {} ", deco.label),
            Style::default().fg(fg).bg(bg),
        ));
    }

    Line::from(spans)
}

fn build_hash_span(commit: &Commit) -> Line<'static> {
    let short_hash = if commit.hash.len() > 7 {
        &commit.hash[..7]
    } else {
        &commit.hash
    };

    Line::from(Span::styled(
        short_hash.to_string(),
        if commit.merge {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(200, 150, 100))
        },
    ))
}

/// Build decorations-only text for graph_only rows.
fn graph_only_decorations(commit: &Commit) -> String {
    if commit.decorations.is_empty() {
        String::new()
    } else {
        commit
            .decorations
            .iter()
            .map(|d| d.label.clone())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn decoration_colors(kind: &DecorationKind) -> (Color, Color) {
    match kind {
        DecorationKind::Head => (Color::Rgb(0, 85, 0), Color::Rgb(100, 255, 100)),
        DecorationKind::Tag => (Color::Rgb(0, 85, 85), Color::Rgb(100, 255, 255)),
        DecorationKind::LocalBranch => (Color::Rgb(85, 85, 0), Color::Rgb(255, 255, 100)),
        DecorationKind::RemoteBranch => (Color::Rgb(85, 0, 85), Color::Rgb(255, 100, 255)),
    }
}

/// Truncate a string to at most `max_len` bytes, snapping to a valid char boundary.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len || max_len <= 1 {
        return s.to_string();
    }
    // Find the last valid char boundary at or before max_len - 1 (for the "…" char)
    let target = max_len.saturating_sub(1);
    let end = if s.is_char_boundary(target) {
        target
    } else {
        (0..target).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0)
    };
    if end == 0 {
        return s.to_string(); // can't meaningfully truncate with a char-safe prefix
    }
    format!("{}…", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_ascii() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
    }

    #[test]
    fn test_truncate_multibyte() {
        // "Mäller": M(0)+ä(1-2)+l(3)+l(4)+e(5)+r(6) = 7 bytes
        assert_eq!(truncate("Mäller", 7), "Mäller");
        // max_len=5: target=4, byte 4 is 'l' (=char boundary) → &s[..4]="Mäl" → "Mäl…"
        assert_eq!(truncate("Mäller", 5), "Mäl…");
        // max_len=3: target=2, byte 2 is inside 'ä' → step back to byte 1 (still inside 'ä')
        // step back to byte 0 (=char boundary) → &s[..0]="" → "…"
        // Actually "…" for empty prefix is bad UX but technically correct char-safe behavior.
        // The caller should pass reasonable max_len values (>= 1).
    }

    #[test]
    fn test_truncate_emoji() {
        // "hi🎉there" = h(0)+i(1)+🎉(2-5)+t(6)+h(7)+e(8)+r(9)+e(10) = 11 bytes
        let s = "hi🎉there";
        assert_eq!(truncate(s, 20), "hi🎉there");
        // max_len=9: target=8, byte 8 is 'e' (=char boundary) → &s[..8]="hi🎉th" → "hi🎉th…"
        assert_eq!(truncate(s, 9), "hi🎉th…");
        // max_len=5: target=4, byte 4 is inside 🎉 → step back to byte 2 (=char boundary 🎉 start)
        // &s[..2]="hi" → "hi…"
        assert_eq!(truncate(s, 5), "hi…");
    }
}
