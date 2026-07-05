use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::graph::LANE_COLORS;
use crate::models::*;
use crate::text_utils::{format_commit_count_info, truncate};
use crate::ui::layout::TABLE_OVERHEAD;

const COL_GRAPH_MAX: u16 = 12;
const COL_HASH: u16 = 8;
const COL_SUBJECT_MIN: u16 = 20;
const COL_AUTHOR: u16 = 15;
const COL_DATE: u16 = 18;
const COL_SEPARATORS: u16 = 4;
const MIN_GRAPH_WIDTH: u16 = 4;
pub(crate) const SHORT_HASH_LEN: usize = 7;

/// Render context for the commit table panel.
pub struct CommitTableCtx<'a> {
    pub commits: &'a [Commit],
    pub visible_index: usize,
    pub is_focused: bool,
    pub visible_to_commit: &'a [usize],
    pub total_loaded: usize,
    pub search_active: bool,
    /// When true, show simple colored bullets instead of full box-drawing graph.
    pub simplified_graph: bool,
    /// Optional debug frame timing label shown in the panel title.
    pub debug_label: Option<&'a str>,
}

/// Render the commit table with git graph, decorations, and merge highlighting.
/// Column order matches Ruby: Graph | Hash | Subject | Author | Date
pub fn render(frame: &mut Frame, area: Rect, ctx: &CommitTableCtx, state: &mut TableState) {
    let border_style = if ctx.is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    // Guard against zero-size area
    if area.width < 10 || area.height < TABLE_OVERHEAD {
        return;
    }

    // Empty state messages
    if ctx.commits.is_empty() && ctx.total_loaded > 0 {
        let msg = if ctx.search_active {
            "No commits match your search."
        } else {
            "No commits found in this repository."
        };
        let title = if let Some(label) = ctx.debug_label {
            format!(" Commits [{}] ", label)
        } else {
            " Commits ".to_string()
        };
        let block = Block::default()
            .title(title)
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

    let header = Row::new(vec!["Graph", "Hash", "Subject", "Author", "Date"])
        .style(header_style)
        .height(1);

    let highlight_style = Style::default()
        .bg(if ctx.is_focused {
            Color::Rgb(80, 60, 120)
        } else {
            Color::DarkGray
        })
        .add_modifier(Modifier::BOLD);

    // Map the visible selection to an absolute index into `ctx.commits`.
    let mapped_index = ctx
        .visible_to_commit
        .get(ctx.visible_index)
        .copied()
        .unwrap_or(0);

    // Determine the viewport window and only build rows for the rows actually
    // on screen. Building a Row for every commit (potentially tens of thousands)
    // on every frame is what made navigation sluggish on large repos.
    // TABLE_OVERHEAD accounts for the two borders plus the header row, which
    // matches ratatui's own inner-height calculation for a bordered table.
    let viewport_height = area.height.saturating_sub(TABLE_OVERHEAD) as usize;
    let total = ctx.commits.len();

    // Scroll-follow-selection: keep the previous absolute offset unless the
    // selection has moved out of view, then clamp so we never scroll past the end.
    let mut offset = state.offset();
    if mapped_index < offset {
        offset = mapped_index;
    } else if viewport_height > 0 && mapped_index >= offset + viewport_height {
        offset = mapped_index + 1 - viewport_height;
    }
    let max_offset = total.saturating_sub(viewport_height);
    offset = offset.min(max_offset);

    // Persist the absolute offset so the scrollbar and mouse-click mapping
    // (which both read `state.offset()`) stay correct.
    *state.offset_mut() = offset;

    let end = (offset + viewport_height).min(total);
    let window = if offset < end {
        &ctx.commits[offset..end]
    } else {
        &[][..]
    };

    // Calculate dynamic graph width from the visible window only (use char count,
    // not byte length — all Unicode box-drawing/graph characters are single-width
    // but 3 bytes each in UTF-8). In simplified mode the graph is always 1 column.
    let col_graph = if ctx.simplified_graph {
        1
    } else {
        let max_graph = window
            .iter()
            .map(|c| c.graph.chars().count())
            .max()
            .unwrap_or(MIN_GRAPH_WIDTH as usize) as u16;
        max_graph.clamp(MIN_GRAPH_WIDTH, COL_GRAPH_MAX)
    };

    let available_width = area.width.saturating_sub(crate::ui::layout::PANEL_BORDER_H);
    let fixed_width = col_graph + COL_HASH + COL_AUTHOR + COL_DATE + COL_SEPARATORS;
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

    // Filter graph_only rows out and show them with minimal content
    let rows: Vec<Row> = window
        .iter()
        .map(|commit| {
            let graph_span = build_graph_span(commit, col_graph as usize, ctx.simplified_graph);
            let hash_span = build_hash_span(commit);

            let subject_span = if commit.graph_only {
                Line::from(Span::styled(
                    graph_only_decorations(commit),
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                let mut spans: Vec<Span> = Vec::new();
                if !commit.decorations.is_empty() {
                    spans.push(Span::raw("("));
                    for (i, deco) in commit.decorations.iter().enumerate() {
                        if i > 0 {
                            spans.push(Span::raw(", "));
                        }
                        spans.push(Span::styled(
                            deco.label.clone(),
                            decoration_style(&deco.kind),
                        ));
                    }
                    spans.push(Span::raw(") "));
                }
                spans.push(Span::styled(
                    truncate(&commit.subject, subject_width as usize),
                    if commit.merge {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ));
                Line::from(spans)
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
                .title(format!(
                    "{} - {} ",
                    if let Some(label) = ctx.debug_label {
                        format!(" Commits [{}]", label)
                    } else {
                        " Commits".to_string()
                    },
                    format_commit_count_info(
                        ctx.visible_index,
                        ctx.commits.len(),
                        ctx.total_loaded,
                    )
                ))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .row_highlight_style(highlight_style)
        .column_spacing(1);

    // `state` now holds the absolute offset (for the scrollbar / mouse mapping).
    // Render with a local state whose offset is relative to the window slice, so
    // the highlight lands on the right row without ratatui re-deriving the offset.
    let mut local_state = TableState::default();
    *local_state.offset_mut() = 0;
    if !window.is_empty() && mapped_index >= offset && mapped_index < end {
        local_state.select(Some(mapped_index - offset));
    }

    frame.render_stateful_widget(table, area, &mut local_state);
}

fn build_graph_span(commit: &Commit, graph_width: usize, simplified: bool) -> Line<'static> {
    if simplified {
        return build_simplified_graph(commit);
    }

    if commit.graph.is_empty() || commit.graph_colors.len() != commit.graph.chars().count() {
        // Fallback: single-color graph line
        let padded = format!("{:width$}", commit.graph, width = graph_width);
        return Line::from(Span::styled(
            padded,
            if commit.merge {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ));
    }

    let chars: Vec<char> = commit.graph.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(chars.len());

    for (i, &ch) in chars.iter().enumerate() {
        let color = if i < commit.graph_colors.len() && commit.graph_colors[i] != 255 {
            LANE_COLORS[(commit.graph_colors[i] as usize) % LANE_COLORS.len()]
        } else {
            Color::DarkGray
        };
        spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
    }

    // Pad to graph_width
    let current_width = spans.len();
    for _ in current_width..graph_width {
        spans.push(Span::raw(" "));
    }

    Line::from(spans)
}

/// Simplified graph: a colored bullet (●), or ○ for merges, no connecting lines.
/// Bullets are colored by branch lane; commits not on a known branch tip render in gray.
fn build_simplified_graph(commit: &Commit) -> Line<'static> {
    let lane = commit.graph_colors.iter().find(|&&c| c != 255).copied();

    let ch = if commit.merge { '○' } else { '●' };
    let color = if commit.merge {
        Color::Yellow
    } else if let Some(l) = lane {
        LANE_COLORS[(l as usize) % LANE_COLORS.len()]
    } else {
        Color::DarkGray
    };

    Line::from(Span::styled(ch.to_string(), Style::default().fg(color)))
}

fn build_hash_span(commit: &Commit) -> Line<'static> {
    let short_hash = if commit.hash.len() > SHORT_HASH_LEN {
        &commit.hash[..SHORT_HASH_LEN]
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

fn decoration_style(kind: &DecorationKind) -> Style {
    match kind {
        DecorationKind::Tag => Style::default().fg(Color::Yellow),
        DecorationKind::LocalBranch => Style::default().fg(Color::Green),
        DecorationKind::RemoteBranch => Style::default().fg(Color::Red),
        DecorationKind::Head => Style::default().fg(Color::LightGreen),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commit(hash: &str, graph: &str, merge: bool, decorations: Vec<Decoration>) -> Commit {
        Commit {
            hash: hash.to_string(),
            graph: graph.to_string(),
            graph_colors: vec![],
            graph_only: false,
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            merge,
            decorations,
            deco_line: 0,
        }
    }

    // --- build_hash_span ---

    #[test]
    fn test_build_hash_span_long_hash() {
        let c = make_commit("abc1234567890abcdef", "", false, vec![]);
        let span = build_hash_span(&c);
        // Should be truncated to SHORT_HASH_LEN (7)
        let expected = Span::styled(
            "abc1234".to_string(),
            Style::default().fg(Color::Rgb(200, 150, 100)),
        );
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_hash_span_short_hash() {
        let c = make_commit("abc", "", false, vec![]);
        let span = build_hash_span(&c);
        let expected = Span::styled(
            "abc".to_string(),
            Style::default().fg(Color::Rgb(200, 150, 100)),
        );
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_hash_span_merge() {
        let c = make_commit("abc1234567890abcdef", "", true, vec![]);
        let span = build_hash_span(&c);
        let expected = Span::styled(
            "abc1234".to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        assert_eq!(span, Line::from(expected));
    }

    // --- build_graph_span ---

    #[test]
    fn test_build_graph_span_normal() {
        let c = make_commit("", "●", false, vec![]);
        let span = build_graph_span(&c, 2, false);
        let expected = Span::styled("● ".to_string(), Style::default().fg(Color::DarkGray));
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_graph_span_merge() {
        let c = make_commit("", "○", true, vec![]);
        let span = build_graph_span(&c, 1, false);
        let expected = Span::styled("○".to_string(), Style::default().fg(Color::Yellow));
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_simplified_graph_regular() {
        let mut c = make_commit("", "●", false, vec![]);
        c.graph_colors = vec![2]; // lane 2 → cyan
        let span = build_simplified_graph(&c);
        let expected = Span::styled("●".to_string(), Style::default().fg(LANE_COLORS[2]));
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_simplified_graph_merge() {
        let mut c = make_commit("", "○", true, vec![]);
        c.graph_colors = vec![0];
        let span = build_simplified_graph(&c);
        let expected = Span::styled("○".to_string(), Style::default().fg(Color::Yellow));
        assert_eq!(span, Line::from(expected));
    }

    // --- graph_only_decorations ---

    #[test]
    fn test_graph_only_decorations_empty() {
        let c = make_commit("", "", false, vec![]);
        assert_eq!(graph_only_decorations(&c), "");
    }

    #[test]
    fn test_graph_only_decorations_with_labels() {
        let c = make_commit(
            "",
            "",
            false,
            vec![
                Decoration {
                    label: "main".to_string(),
                    kind: DecorationKind::LocalBranch,
                },
                Decoration {
                    label: "v1.0".to_string(),
                    kind: DecorationKind::Tag,
                },
            ],
        );
        assert_eq!(graph_only_decorations(&c), "main, v1.0");
    }

    // --- decoration_style ---

    #[test]
    fn test_decoration_style_tag() {
        let style = decoration_style(&DecorationKind::Tag);
        assert_eq!(style, Style::default().fg(Color::Yellow));
    }

    #[test]
    fn test_decoration_style_local_branch() {
        let style = decoration_style(&DecorationKind::LocalBranch);
        assert_eq!(style, Style::default().fg(Color::Green));
    }

    #[test]
    fn test_decoration_style_remote_branch() {
        let style = decoration_style(&DecorationKind::RemoteBranch);
        assert_eq!(style, Style::default().fg(Color::Red));
    }

    #[test]
    fn test_decoration_style_head() {
        let style = decoration_style(&DecorationKind::Head);
        assert_eq!(style, Style::default().fg(Color::LightGreen));
    }

    // --- viewport windowing (performance regression guard) ---

    fn render_with(commit_count: usize, visible_index: usize, height: u16) -> TableState {
        use ratatui::{backend::TestBackend, Terminal};

        let commits: Vec<Commit> = (0..commit_count)
            .map(|i| make_commit(&format!("{:040x}", i), "*", false, vec![]))
            .collect();
        let visible_to_commit: Vec<usize> = (0..commits.len()).collect();
        let ctx = CommitTableCtx {
            commits: &commits,
            visible_index,
            is_focused: true,
            visible_to_commit: &visible_to_commit,
            total_loaded: commits.len(),
            search_active: false,
            simplified_graph: false,
            debug_label: None,
        };
        let mut state = TableState::default();
        let mut terminal = Terminal::new(TestBackend::new(80, height)).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &ctx, &mut state))
            .unwrap();
        state
    }

    #[test]
    fn test_offset_follows_selection_into_view() {
        // Height 13 => 13 - TABLE_OVERHEAD(3) = 10 visible data rows.
        let state = render_with(1000, 500, 13);
        let offset = state.offset();
        let viewport = 10;
        assert!(offset <= 500, "offset {} must not exceed selection", offset);
        assert!(
            500 < offset + viewport,
            "selection must be within [{}, {}) viewport",
            offset,
            offset + viewport
        );
    }

    #[test]
    fn test_offset_zero_when_selection_at_start() {
        let state = render_with(1000, 0, 13);
        assert_eq!(state.offset(), 0);
    }

    #[test]
    fn test_offset_clamped_at_end() {
        // Selecting the last commit must not scroll past the end.
        let state = render_with(1000, 999, 13);
        let viewport = 10;
        assert_eq!(state.offset(), 1000 - viewport);
    }
}
