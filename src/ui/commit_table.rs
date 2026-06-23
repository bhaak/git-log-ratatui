use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

use crate::models::*;

/// Widths for the commit table columns.
const COL_GRAPH: u16 = 12;
const COL_HASH: u16 = 8;
const COL_AUTHOR: u16 = 15;
const COL_DATE: u16 = 18;

/// Render the commit table with git graph, decorations, and merge highlighting.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    commits: &[Commit],
    selected_index: usize,
    is_focused: bool,
) -> TableState {
    let border_style = if is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec![
        "Graph",
        "Hash",
        "Author",
        "Date",
        "Subject",
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

    let available_width = area.width.saturating_sub(2); // borders
    let subject_width = available_width
        .saturating_sub(COL_GRAPH + COL_HASH + COL_AUTHOR + COL_DATE + 4) // 4 for separators
        .max(10);

    let widths = [
        Constraint::Length(COL_GRAPH),
        Constraint::Length(COL_HASH),
        Constraint::Length(COL_AUTHOR),
        Constraint::Length(COL_DATE),
        Constraint::Length(subject_width),
    ];

    let rows: Vec<Row> = commits.iter().map(|commit| {
        let graph_span = build_graph_span(commit);
        let hash_span = build_hash_span(commit);
        let author_span = Span::styled(
            truncate(&commit.author, COL_AUTHOR as usize),
            Style::default().fg(Color::White),
        );
        let date_span = Span::styled(
            &commit.date,
            Style::default().fg(Color::DarkGray),
        );
        let subject_span = build_subject_span(commit);

        Row::new(vec![
            graph_span,
            Line::from(hash_span),
            Line::from(author_span),
            Line::from(date_span),
            Line::from(subject_span),
        ])
        .height(1)
    }).collect();

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

    let mut state = TableState::default();
    if !commits.is_empty() {
        state.select(Some(selected_index.min(commits.len() - 1)));
    }

    frame.render_stateful_widget(table, area, &mut state);

    state
}

/// Build the graph span with Unicode box-drawing characters and decorations.
fn build_graph_span(commit: &Commit) -> Line<'static> {
    let graph_unicode = convert_graph_chars(&commit.graph);
    let mut spans = Vec::new();

    // Add graph text
    spans.push(Span::styled(
        graph_unicode,
        if commit.merge {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        },
    ));

    // Add decorations inline
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

/// Convert ASCII graph characters to Unicode box-drawing characters.
fn convert_graph_chars(graph: &str) -> String {
    graph
        .replace('|', "\u{2502}") // │
        .replace('/', "\u{2571}") // ╱
        .replace('\\', "\u{2572}") // ╲
        .replace('_', "\u{2500}") // ─
        .replace('*', "\u{2502}") // │ (commit marker)
}

/// Build the hash span, with yellow color for merge commits.
fn build_hash_span(commit: &Commit) -> Span<'static> {
    let short_hash = if commit.hash.len() > 7 {
        &commit.hash[..7]
    } else {
        &commit.hash
    };

    Span::styled(
        short_hash.to_string(),
        if commit.merge {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Rgb(200, 150, 100))
        },
    )
}

/// Build the subject span with decoration-related styling.
fn build_subject_span(commit: &Commit) -> Span<'static> {
    Span::styled(
        commit.subject.clone(),
        if commit.merge {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        },
    )
}

/// Get colors for a decoration kind.
fn decoration_colors(kind: &DecorationKind) -> (Color, Color) {
    match kind {
        DecorationKind::Head => (Color::Rgb(0, 85, 0), Color::Rgb(100, 255, 100)),
        DecorationKind::Tag => (Color::Rgb(0, 85, 85), Color::Rgb(100, 255, 255)),
        DecorationKind::LocalBranch => (Color::Rgb(85, 85, 0), Color::Rgb(255, 255, 100)),
        DecorationKind::RemoteBranch => (Color::Rgb(85, 0, 85), Color::Rgb(255, 100, 255)),
    }
}

/// Truncate a string to the given max character count, adding "..." if needed.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}
