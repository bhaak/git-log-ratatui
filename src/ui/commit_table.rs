use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
    Frame,
};

use std::cell::RefCell;

use crate::app::commands::Command;
use crate::domain::DecorationKind;
use crate::state::commit::CommitTableState;
use crate::text_utils::{format_commit_count_info, truncate};
use crate::theme::Theme;
use crate::ui::layout::{self, TABLE_OVERHEAD};
use crate::ui::panel::{KeyBinding, Panel as PanelTrait};
use crate::ui::render_ctx::RenderCtx;
use crate::ui::scrollbar_view::ScrollbarView;
use crate::view::{CommitRow as Commit, Panel};

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
    /// When true, derive hash column color from commit hash hex digits.
    pub hash_color_enabled: bool,
    /// Color theme.
    pub theme: &'a Theme,
}

/// Render the commit table with git graph, decorations, and merge highlighting.
/// Column order matches Ruby: Graph | Hash | Subject | Author | Date
pub fn render(frame: &mut Frame, area: Rect, ctx: &CommitTableCtx, state: &mut TableState) {
    let border_style = if ctx.is_focused {
        Style::default().fg(ctx.theme.focused_border)
    } else {
        Style::default().fg(ctx.theme.unfocused_border)
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
            let p = Paragraph::new(Span::styled(
                msg,
                Style::default().fg(ctx.theme.commit_secondary),
            ))
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
        .fg(ctx.theme.commit_merge)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(vec!["Graph", "Hash", "Subject", "Author", "Date"])
        .style(header_style)
        .height(1);

    let highlight_style = Style::default()
        .bg(if ctx.is_focused {
            ctx.theme.selected_bg
        } else {
            ctx.theme.unselected_bg
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
    // Compute date range for staleness stretching: today = brightest, oldest = darkest.
    // Uses pre-computed epoch_days from CommitRow to avoid string parsing in the render path.
    let epoches: Vec<i64> = ctx
        .commits
        .iter()
        .map(|c| c.epoch_days)
        .filter(|&d| d > 0)
        .collect();
    let min_days = epoches.iter().min().copied().unwrap_or(0);
    let max_days = current_epoch_days();

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
            let graph_span =
                build_graph_span(commit, col_graph as usize, ctx.simplified_graph, ctx.theme);
            let hash_span = build_hash_span(commit, ctx);

            let subject_span = if commit.graph_only {
                Line::from(Span::styled(
                    graph_only_decorations(commit),
                    Style::default().fg(ctx.theme.commit_secondary),
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
                            decoration_style(&deco.kind, ctx.theme),
                        ));
                    }
                    spans.push(Span::raw(") "));
                }
                spans.push(Span::styled(
                    truncate(&commit.subject, subject_width as usize),
                    if commit.merge {
                        Style::default().fg(ctx.theme.commit_merge)
                    } else {
                        Style::default().fg(ctx.theme.commit_default)
                    },
                ));
                Line::from(spans)
            };

            let author_span = Line::from(Span::styled(
                truncate(&commit.author, COL_AUTHOR as usize),
                Style::default().fg(ctx.theme.commit_default),
            ));
            let date_span = Line::from(Span::styled(
                &commit.date,
                Style::default().fg(age_color(commit.epoch_days, min_days, max_days)),
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

fn is_branch_head(commit: &Commit) -> bool {
    commit.decorations.iter().any(|d| {
        matches!(
            d.kind,
            DecorationKind::LocalBranch | DecorationKind::RemoteBranch
        )
    })
}

fn build_graph_span(
    commit: &Commit,
    graph_width: usize,
    simplified: bool,
    theme: &Theme,
) -> Line<'static> {
    if simplified {
        return build_simplified_graph(commit, theme);
    }

    if commit.graph.is_empty() || commit.graph_colors.len() != commit.graph.chars().count() {
        let padded = format!("{:width$}", commit.graph, width = graph_width);
        return Line::from(Span::styled(
            padded,
            if commit.merge {
                Style::default().fg(theme.commit_merge)
            } else {
                Style::default().fg(theme.commit_secondary)
            },
        ));
    }

    let chars: Vec<char> = commit.graph.chars().collect();
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(chars.len());

    for (i, &ch) in chars.iter().enumerate() {
        let display_ch = if is_branch_head(commit) && (ch == '\u{25CF}' || ch == '\u{25CB}') {
            '\u{29BF}'
        } else {
            ch
        };
        let color = if i < commit.graph_colors.len() && commit.graph_colors[i] != 255 {
            theme.graph_colors[(commit.graph_colors[i] as usize) % theme.graph_colors.len()]
        } else {
            theme.commit_secondary
        };
        spans.push(Span::styled(
            display_ch.to_string(),
            Style::default().fg(color),
        ));
    }

    let current_width = spans.len();
    for _ in current_width..graph_width {
        spans.push(Span::raw(" "));
    }

    Line::from(spans)
}

fn build_simplified_graph(commit: &Commit, theme: &Theme) -> Line<'static> {
    let lane = commit.graph_colors.iter().find(|&&c| c != 255).copied();

    let ch = if is_branch_head(commit) {
        '\u{29BF}'
    } else if commit.merge {
        '○'
    } else {
        '●'
    };
    let color = if commit.merge {
        theme.commit_merge
    } else if let Some(l) = lane {
        theme.graph_colors[(l as usize) % theme.graph_colors.len()]
    } else {
        theme.commit_secondary
    };

    Line::from(Span::styled(ch.to_string(), Style::default().fg(color)))
}

fn build_hash_span(commit: &Commit, ctx: &CommitTableCtx) -> Line<'static> {
    let short_hash = if commit.hash.len() > SHORT_HASH_LEN {
        &commit.hash[..SHORT_HASH_LEN]
    } else {
        &commit.hash
    };

    Line::from(Span::styled(
        short_hash.to_string(),
        if commit.merge {
            Style::default()
                .fg(ctx.theme.commit_merge)
                .add_modifier(Modifier::BOLD)
        } else if ctx.hash_color_enabled {
            Style::default().fg(hash_color(&commit.hash))
        } else {
            Style::default().fg(ctx.theme.scrollbar_thumb)
        },
    ))
}

/// Derive a unique, readable color from a git commit hash (first 6 hex digits).
/// Maps 0-255 per channel to 50-250 to prevent too-dark colors.
fn hash_color(hash: &str) -> Color {
    if hash.len() < 6 {
        return Color::Rgb(128, 128, 128);
    }
    let r = u8::from_str_radix(&hash[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&hash[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&hash[4..6], 16).unwrap_or(128);

    Color::Rgb(
        ((r as u32 * 200 / 255) + 50) as u8,
        ((g as u32 * 200 / 255) + 50) as u8,
        ((b as u32 * 200 / 255) + 50) as u8,
    )
}

/// Derive a greyscale color from pre-computed epoch days, stretched between
/// the newest and oldest visible commits. Newest = bright white (255),
/// oldest = dark grey (75). When all commits have the same date, returns white.
pub(crate) fn age_color(epoch_days: i64, min_days: i64, max_days: i64) -> Color {
    if max_days <= min_days {
        return Color::Rgb(255, 255, 255);
    }
    let range = (max_days - min_days) as f64;
    if range <= 0.0 {
        return Color::Rgb(255, 255, 255);
    }
    let t = (epoch_days - min_days) as f64 / range;
    let staleness = (75.0 + t * 180.0) as u8;
    Color::Rgb(staleness, staleness, staleness)
}

pub(crate) fn current_epoch_days() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    (secs / 86400) as i64
}

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

fn decoration_style(kind: &DecorationKind, theme: &Theme) -> Style {
    match kind {
        DecorationKind::Tag => Style::default().fg(theme.decoration_tag),
        DecorationKind::LocalBranch => Style::default().fg(theme.decoration_local),
        DecorationKind::RemoteBranch => Style::default().fg(theme.decoration_remote),
        DecorationKind::Head => Style::default().fg(theme.decoration_head),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Decoration, DecorationKind};
    use crate::theme::Theme;
    use crate::time_format::ymd_to_days;

    fn make_theme() -> Theme {
        Theme::default()
    }

    fn make_ctx<'a>(theme: &'a Theme, commits: &'a [Commit]) -> CommitTableCtx<'a> {
        CommitTableCtx {
            commits,
            visible_index: 0,
            is_focused: true,
            visible_to_commit: &[],
            total_loaded: 0,
            search_active: false,
            simplified_graph: false,
            hash_color_enabled: true,
            debug_label: None,
            theme,
        }
    }

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
            epoch_days: 0,
        }
    }

    #[test]
    fn test_build_hash_span_long_hash() {
        let theme = make_theme();
        let c = make_commit("abc1234567890abcdef", "", false, vec![]);
        let commits = [c.clone()];
        let ctx = make_ctx(&theme, &commits);
        let span = build_hash_span(&c, &ctx);
        let expected = Span::styled(
            "abc1234".to_string(),
            Style::default().fg(hash_color("abc1234567890abcdef")),
        );
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_hash_span_short_hash() {
        let theme = make_theme();
        let c = make_commit("abc123", "", false, vec![]);
        let commits = [c.clone()];
        let ctx = make_ctx(&theme, &commits);
        let span = build_hash_span(&c, &ctx);
        let expected = Span::styled(
            "abc123".to_string(),
            Style::default().fg(hash_color("abc123")),
        );
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_hash_span_disabled_uses_scrollbar_thumb() {
        let theme = make_theme();
        let c = make_commit("abc1234567890abcdef", "", false, vec![]);
        let commits = [c.clone()];
        let mut ctx = make_ctx(&theme, &commits);
        ctx.hash_color_enabled = false;
        let span = build_hash_span(&c, &ctx);
        let expected = Span::styled(
            "abc1234".to_string(),
            Style::default().fg(theme.scrollbar_thumb),
        );
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_hash_color_derives_unique_values() {
        let c1 = hash_color("aa00000000000000000000000000000000000000");
        let c2 = hash_color("bb00000000000000000000000000000000000000");
        let c3 = hash_color("0000aa0000000000000000000000000000000000");
        assert_ne!(c1, c2);
        assert_ne!(c1, c3);
        assert_ne!(c2, c3);
    }

    #[test]
    fn test_hash_color_respects_minimum_brightness() {
        let c = hash_color("0000000000000000000000000000000000000000");
        match c {
            Color::Rgb(r, g, b) => {
                assert!(r >= 50, "r={r} should be >= 50");
                assert!(g >= 50, "g={g} should be >= 50");
                assert!(b >= 50, "b={b} should be >= 50");
            }
            _ => panic!("expected Rgb"),
        }
    }

    #[test]
    fn test_hash_color_short_hash_falls_back_to_grey() {
        let c = hash_color("abc");
        assert_eq!(c, Color::Rgb(128, 128, 128));
    }

    #[test]
    fn test_hash_color_empty_hash_falls_back_to_grey() {
        let c = hash_color("");
        assert_eq!(c, Color::Rgb(128, 128, 128));
    }

    #[test]
    fn test_age_color_newest_is_bright() {
        let min_d = ymd_to_days(2024, 1, 1).unwrap();
        let max_d = ymd_to_days(2024, 12, 31).unwrap();
        let c = age_color(max_d, min_d, max_d);
        match c {
            Color::Rgb(r, g, b) => {
                assert_eq!(r, 255, "newest should be 255");
                assert_eq!(r, g);
                assert_eq!(r, b);
            }
            _ => panic!("expected Rgb"),
        }
    }

    #[test]
    fn test_age_color_oldest_is_75() {
        let min_d = ymd_to_days(2024, 1, 1).unwrap();
        let max_d = ymd_to_days(2024, 12, 31).unwrap();
        let c = age_color(min_d, min_d, max_d);
        match c {
            Color::Rgb(r, g, b) => {
                assert_eq!(r, 75, "oldest should be 75");
                assert_eq!(r, g);
                assert_eq!(r, b);
            }
            _ => panic!("expected Rgb"),
        }
    }

    #[test]
    fn test_age_color_mid_range() {
        let min_d = ymd_to_days(2024, 1, 1).unwrap();
        let max_d = ymd_to_days(2024, 12, 31).unwrap();
        let mid_d = ymd_to_days(2024, 7, 1).unwrap();
        let c1 = age_color(mid_d, min_d, max_d);
        let c2 = age_color(min_d, min_d, max_d);
        match (c1, c2) {
            (Color::Rgb(r1, _, _), Color::Rgb(r2, _, _)) => {
                assert!(
                    r1 > r2,
                    "older date (Jan) should be darker than newer (Jul): r1={r1} r2={r2}"
                );
            }
            _ => panic!("expected Rgb"),
        }
    }

    #[test]
    fn test_age_color_single_date_returns_white() {
        let d = ymd_to_days(2024, 7, 1).unwrap();
        let c = age_color(d, d, d);
        assert_eq!(c, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn test_age_color_zero_range_returns_white() {
        let c = age_color(1000, 1000, 1000);
        assert_eq!(c, Color::Rgb(255, 255, 255));
    }

    #[test]
    fn test_build_hash_span_merge() {
        let theme = make_theme();
        let c = make_commit("abc1234567890abcdef", "", true, vec![]);
        let commits = [c.clone()];
        let ctx = make_ctx(&theme, &commits);
        let span = build_hash_span(&c, &ctx);
        let expected = Span::styled(
            "abc1234".to_string(),
            Style::default()
                .fg(theme.commit_merge)
                .add_modifier(Modifier::BOLD),
        );
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_graph_span_normal() {
        let theme = make_theme();
        let c = make_commit("", "●", false, vec![]);
        let span = build_graph_span(&c, 2, false, &theme);
        let expected = Span::styled(
            "● ".to_string(),
            Style::default().fg(theme.commit_secondary),
        );
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_graph_span_merge() {
        let theme = make_theme();
        let c = make_commit("", "○", true, vec![]);
        let span = build_graph_span(&c, 1, false, &theme);
        let expected = Span::styled("○".to_string(), Style::default().fg(theme.commit_merge));
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_simplified_graph_regular() {
        let theme = make_theme();
        let mut c = make_commit("", "●", false, vec![]);
        c.graph_colors = vec![2];
        let span = build_simplified_graph(&c, &theme);
        let expected = Span::styled("●".to_string(), Style::default().fg(theme.graph_colors[2]));
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_simplified_graph_merge() {
        let theme = make_theme();
        let mut c = make_commit("", "○", true, vec![]);
        c.graph_colors = vec![0];
        let span = build_simplified_graph(&c, &theme);
        let expected = Span::styled("○".to_string(), Style::default().fg(theme.commit_merge));
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_build_simplified_graph_branch_head() {
        let theme = make_theme();
        let mut c = make_commit(
            "",
            "●",
            false,
            vec![Decoration {
                label: "main".into(),
                kind: DecorationKind::LocalBranch,
            }],
        );
        c.graph_colors = vec![2];
        let span = build_simplified_graph(&c, &theme);
        let expected = Span::styled(
            "\u{29BF}".to_string(),
            Style::default().fg(theme.graph_colors[2]),
        );
        assert_eq!(span, Line::from(expected));
    }

    #[test]
    fn test_is_branch_head_true_for_local_branch() {
        let c = make_commit(
            "",
            "",
            false,
            vec![Decoration {
                label: "main".into(),
                kind: DecorationKind::LocalBranch,
            }],
        );
        assert!(is_branch_head(&c));
    }

    #[test]
    fn test_is_branch_head_false_for_tag_only() {
        let c = make_commit(
            "",
            "",
            false,
            vec![Decoration {
                label: "v1.0".into(),
                kind: DecorationKind::Tag,
            }],
        );
        assert!(!is_branch_head(&c));
    }

    #[test]
    fn test_is_branch_head_false_for_no_decorations() {
        let c = make_commit("", "", false, vec![]);
        assert!(!is_branch_head(&c));
    }

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

    #[test]
    fn test_decoration_style_tag() {
        let theme = make_theme();
        let style = decoration_style(&DecorationKind::Tag, &theme);
        assert_eq!(style, Style::default().fg(theme.decoration_tag));
    }

    #[test]
    fn test_decoration_style_local_branch() {
        let theme = make_theme();
        let style = decoration_style(&DecorationKind::LocalBranch, &theme);
        assert_eq!(style, Style::default().fg(theme.decoration_local));
    }

    #[test]
    fn test_decoration_style_remote_branch() {
        let theme = make_theme();
        let style = decoration_style(&DecorationKind::RemoteBranch, &theme);
        assert_eq!(style, Style::default().fg(theme.decoration_remote));
    }

    #[test]
    fn test_decoration_style_head() {
        let theme = make_theme();
        let style = decoration_style(&DecorationKind::Head, &theme);
        assert_eq!(style, Style::default().fg(theme.decoration_head));
    }

    fn render_with(commit_count: usize, visible_index: usize, height: u16) -> TableState {
        use ratatui::{backend::TestBackend, Terminal};

        let theme = make_theme();
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
            hash_color_enabled: true,
            debug_label: None,
            theme: &theme,
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
        let state = render_with(1000, 999, 13);
        let viewport = 10;
        assert_eq!(state.offset(), 1000 - viewport);
    }
}

/// Wrapper struct implementing the Panel trait for the commit table.
#[allow(clippy::items_after_test_module)]
pub struct CommitPanel {
    #[allow(dead_code)]
    table_state: RefCell<TableState>,
}

impl CommitPanel {
    pub fn new() -> Self {
        CommitPanel {
            table_state: RefCell::new(TableState::default()),
        }
    }
}

impl Default for CommitPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelTrait for CommitPanel {
    type State = CommitTableState;

    fn render(&self, area: Rect, frame: &mut Frame, state: &mut Self::State, ctx: &RenderCtx) {
        let (content_area, scrollbar_area) = ScrollbarView::split(area);

        let commits = state
            .filtered_commits
            .as_deref()
            .unwrap_or(&state.all_commits);
        let table_ctx = CommitTableCtx {
            commits,
            visible_index: state.selected_index,
            is_focused: ctx.is_focused(Panel::Commits),
            visible_to_commit: &state.visible_to_commit,
            total_loaded: state.all_commits.len(),
            search_active: ctx.search_active,
            simplified_graph: state.simplified_graph,
            hash_color_enabled: state.hash_color_enabled,
            debug_label: ctx.debug_label,
            theme: ctx.theme,
        };
        let mut ts = state.table_state.clone();
        render(frame, content_area, &table_ctx, &mut ts);
        state.table_state = ts;

        let focus_style = if ctx.is_focused(Panel::Commits) {
            Style::default().fg(ctx.theme.focused_border)
        } else {
            Style::default().fg(ctx.theme.unfocused_border)
        };
        let visible = (content_area.height.saturating_sub(layout::TABLE_OVERHEAD)) as usize;
        let item_count = commits.len();
        state.scrollbar.render(
            frame,
            scrollbar_area,
            item_count,
            visible,
            state.table_state.offset(),
            focus_style,
        );
    }

    fn handle_event(&mut self, key: &KeyEvent, _state: &mut Self::State) -> Vec<Command> {
        match key.code {
            KeyCode::Up => vec![Command::MoveUp],
            KeyCode::Down => vec![Command::MoveDown],
            KeyCode::Enter => vec![Command::ShowCommitDiff],
            KeyCode::Char(' ') => vec![Command::PreviewDiff],
            KeyCode::PageUp => vec![Command::PageUp],
            KeyCode::PageDown => vec![Command::PageDown],
            KeyCode::Home => vec![Command::JumpToTop],
            KeyCode::End => vec![Command::JumpToBottom],
            _ => Vec::new(),
        }
    }

    fn help_keys(&self) -> &'static [KeyBinding] {
        static KEYS: &[KeyBinding] = &[
            KeyBinding::new("↑↓/j,k", "navigate", "Navigate up / down"),
            KeyBinding::new("Enter", "show diff", "Show diff (focus Diff panel)"),
            KeyBinding::new("Space", "preview diff", "Preview diff (keep focus)"),
            KeyBinding::new("y/Y", "copy hash", "Copy short / full hash"),
            KeyBinding::new("g", "toggle graph", "Toggle full / simplified graph"),
            KeyBinding::new("PgUp/PgDn", "page", "Page up / down"),
            KeyBinding::new("Home/End", "top/bottom", "Jump to top / bottom"),
        ];
        KEYS
    }

    fn label(&self) -> &str {
        "Commits"
    }
}
