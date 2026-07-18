use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::app::commands::Command;
use crate::color_depth::ColorDepth;
use crate::domain::DecorationKind;
use crate::state::commit::CommitTableState;
use crate::text_utils::{format_commit_count_info, truncate};
use crate::theme::Theme;
use crate::ui::layout::{self, TABLE_OVERHEAD};
use crate::ui::panel::{KeyBinding, Panel as PanelTrait};
use crate::ui::render_ctx::RenderCtx;
use crate::ui::scrollbar_view::ScrollbarView;
use crate::view::{CommitRow as Commit, Panel};

/// Maximum width in columns for the git graph visualization.
const COL_GRAPH_MAX: u16 = 12;
/// Width of the abbreviated commit hash column.
const COL_HASH: u16 = 8;
/// Minimum width for the commit subject column.
const COL_SUBJECT_MIN: u16 = 20;
/// Width of the author name column.
const COL_AUTHOR: u16 = 15;
/// Width of the commit date column.
const COL_DATE: u16 = 18;
/// Total separator spacing between columns.
const COL_SEPARATORS: u16 = 4;
/// Minimum graph column width when no graph data is present.
const MIN_GRAPH_WIDTH: u16 = 4;
/// Number of hex characters shown for abbreviated commit hashes.
pub(crate) const SHORT_HASH_LEN: usize = 7;

/// Render context for the commit table panel.
pub struct CommitTableCtx<'a> {
    /// All commits available for display (filtered or full list).
    pub commits: &'a [Commit],
    /// Index of the currently highlighted row within the visible subset.
    pub visible_index: usize,
    /// Whether this panel currently has keyboard focus.
    pub is_focused: bool,
    /// Mapping from visible row indices to absolute commit indices.
    pub visible_to_commit: &'a [usize],
    /// Total number of commits loaded (may exceed `commits.len()` when filtered).
    pub total_loaded: usize,
    /// Whether a search filter is currently active.
    pub search_active: bool,
    /// When true, show simple colored bullets instead of full box-drawing graph.
    pub simplified_graph: bool,
    /// Optional debug frame timing label shown in the panel title.
    pub debug_label: Option<&'a str>,
    /// When true, derive hash column color from commit hash hex digits.
    pub hash_color_enabled: bool,
    /// Color theme.
    pub theme: &'a Theme,
    /// Terminal color depth for color adaptation.
    pub color_depth: ColorDepth,
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

    if render_empty_state(frame, area, ctx, border_style) {
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

    // Map the visible selection to an absolute index into `ctx.commits`,
    // compute the scroll offset, and slice the visible window.
    let viewport = compute_viewport(area, state, ctx);
    let mapped_index = viewport.mapped_index;
    let offset = viewport.offset;
    let end = viewport.end;
    let window = viewport.window;
    let (min_days, max_days) = compute_epoch_range(ctx.commits);

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

            let subject_span = build_subject_span(commit, subject_width as usize, ctx.theme);

            let author_span = Line::from(Span::styled(
                truncate(&commit.author, COL_AUTHOR as usize),
                Style::default().fg(ctx.theme.commit_default),
            ));
            let date_span = Line::from(Span::styled(
                &commit.date,
                Style::default().fg(age_color(
                    commit.epoch_days,
                    min_days,
                    max_days,
                    ctx.color_depth,
                )),
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

    let table = build_commit_table(rows, widths, header, ctx, border_style, highlight_style);

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

/// Render an informational message when there are no commits to display.
/// Returns true if an empty state was rendered (caller should return early).
fn render_empty_state(
    frame: &mut Frame,
    area: Rect,
    ctx: &CommitTableCtx,
    border_style: Style,
) -> bool {
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
        true
    } else {
        false
    }
}

/// The visible slice of commits to render in the current frame.
struct Viewport<'a> {
    /// Absolute index of the selected commit within the full commit list.
    mapped_index: usize,
    /// Row offset into the full commit list where the window starts.
    offset: usize,
    /// Exclusive end index of the rendered window.
    end: usize,
    /// Slice of commits that are actually visible on screen.
    window: &'a [Commit],
}

/// Compute the scroll offset, viewport window, and mapped selection index.
/// Mutates `state.offset()` to persist scroll-follow-selection.
fn compute_viewport<'a>(
    area: Rect,
    state: &mut TableState,
    ctx: &CommitTableCtx<'a>,
) -> Viewport<'a> {
    // Map the visible selection to an absolute index into `ctx.commits`.
    let mapped_index = ctx
        .visible_to_commit
        .get(ctx.visible_index)
        .copied()
        .unwrap_or(0);

    // Determine the viewport window and only build rows for the rows actually
    // on screen. Building a Row for every commit (potentially tens of thousands)
    // on every frame made navigation sluggish on large repos.
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

    // Only build rows for the visible window.
    let window = if offset < end {
        &ctx.commits[offset..end]
    } else {
        &[][..]
    };

    Viewport {
        mapped_index,
        offset,
        end,
        window,
    }
}

/// Compute the min and max epoch days from the visible commits for age-based
/// date coloring. Max is always today to ensure newest commits are brightest.
fn compute_epoch_range(commits: &[Commit]) -> (i64, i64) {
    let epochs: Vec<i64> = commits
        .iter()
        .map(|c| c.epoch_days)
        .filter(|&d| d > 0)
        .collect();
    let min_days = epochs.iter().min().copied().unwrap_or(0);
    let max_days = current_epoch_days();
    (min_days, max_days)
}

/// Check whether a commit is at the tip of a local or remote branch.
/// Used to render branch-head commits with a distinct graph marker (⦿).
pub(super) fn is_branch_head(commit: &Commit) -> bool {
    commit.decorations.iter().any(|d| {
        matches!(
            d.kind,
            DecorationKind::LocalBranch | DecorationKind::RemoteBranch
        )
    })
}

/// Build the graph column span for a commit row.
/// In full mode, each graph character is colored according to `graph_colors`.
/// In simplified mode, a single colored bullet is shown.
/// Pads the result to `graph_width` columns.
pub(super) fn build_graph_span(
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

/// Build a simplified single-character graph bullet for a commit row.
/// Uses ● for regular commits, ○ for merges, and ⦿ for branch heads.
pub(super) fn build_simplified_graph(commit: &Commit, theme: &Theme) -> Line<'static> {
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

/// Build the abbreviated hash column span for a commit row.
/// Merge commits are rendered bold in the merge color.
/// Otherwise the color is derived from the hash hex digits or falls back to the scrollbar thumb color.
pub(super) fn build_hash_span(commit: &Commit, ctx: &CommitTableCtx) -> Line<'static> {
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
            Style::default().fg(hash_color(&commit.hash, ctx.color_depth))
        } else {
            Style::default().fg(ctx.theme.scrollbar_thumb)
        },
    ))
}

/// Build the subject column span including decoration labels for a commit row.
/// Graph-only rows (no real commit) show just the decoration names in secondary color.
/// Normal rows show decorations in parens followed by the truncated subject.
pub(super) fn build_subject_span<'a>(
    commit: &'a Commit,
    subject_width: usize,
    theme: &Theme,
) -> Line<'a> {
    if commit.graph_only {
        Line::from(Span::styled(
            graph_only_decorations(commit),
            Style::default().fg(theme.commit_secondary),
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
                    decoration_style(&deco.kind, theme),
                ));
            }
            spans.push(Span::raw(") "));
        }
        spans.push(Span::styled(
            truncate(&commit.subject, subject_width),
            if commit.merge {
                Style::default().fg(theme.commit_merge)
            } else {
                Style::default().fg(theme.commit_default)
            },
        ));
        Line::from(spans)
    }
}

/// Derive a unique, readable color from a git commit hash (first 6 hex digits).
/// Maps 0-255 per channel to 50-250 to prevent too-dark colors.
/// Color is adapted to the terminal color depth.
pub(super) fn hash_color(hash: &str, depth: ColorDepth) -> Color {
    if hash.len() < 6 {
        return depth.rgb_to_color(128, 128, 128);
    }
    let r = u8::from_str_radix(&hash[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&hash[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&hash[4..6], 16).unwrap_or(128);

    depth.rgb_to_color(
        ((r as u32 * 200 / 255) + 50) as u8,
        ((g as u32 * 200 / 255) + 50) as u8,
        ((b as u32 * 200 / 255) + 50) as u8,
    )
}

/// Build the commit table widget with header, title, and styling.
fn build_commit_table<'a>(
    rows: Vec<Row<'a>>,
    widths: [Constraint; 5],
    header: Row<'a>,
    ctx: &CommitTableCtx,
    border_style: Style,
    highlight_style: Style,
) -> Table<'a> {
    Table::new(rows, widths)
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
        .column_spacing(1)
}

/// Derive a greyscale color from pre-computed epoch days, stretched between
/// the newest and oldest visible commits. Newest = bright white (255),
/// oldest = dark grey (75). When all commits have the same date, returns white.
/// Color is adapted to the terminal color depth.
pub(crate) fn age_color(epoch_days: i64, min_days: i64, max_days: i64, depth: ColorDepth) -> Color {
    if max_days <= min_days {
        return depth.rgb_to_color(255, 255, 255);
    }
    let range = (max_days - min_days) as f64;
    if range <= 0.0 {
        return depth.rgb_to_color(255, 255, 255);
    }
    let t = (epoch_days - min_days) as f64 / range;
    let staleness = (75.0 + t * 180.0) as u8;
    depth.rgb_to_color(staleness, staleness, staleness)
}

/// Return the current date as days since Unix epoch.
/// Used as the upper bound for the date staleness color gradient.
pub(crate) fn current_epoch_days() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    (secs / 86400) as i64
}

/// Format decorations for rows that only show graph information (no real commit).
/// Joins decoration labels with ", " for compact display.
pub(super) fn graph_only_decorations(commit: &Commit) -> String {
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

/// Map a decoration kind to its themed style.
pub(super) fn decoration_style(kind: &DecorationKind, theme: &Theme) -> Style {
    match kind {
        DecorationKind::Tag => Style::default().fg(theme.decoration_tag),
        DecorationKind::LocalBranch => Style::default().fg(theme.decoration_local),
        DecorationKind::RemoteBranch => Style::default().fg(theme.decoration_remote),
        DecorationKind::Head => Style::default().fg(theme.decoration_head),
    }
}

/// Wrapper struct implementing the Panel trait for the commit table.
/// Stateless - all mutable state lives in `CommitTableState`.
pub struct CommitPanel;

impl CommitPanel {
    /// Create a new commit table panel.
    pub fn new() -> Self {
        CommitPanel
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
            color_depth: ctx.color_depth,
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

    /// Map keyboard events to commit table navigation and action commands.
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

    /// Return keyboard shortcuts and descriptions for the help bar.
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

    /// Human-readable panel name used in focus indicators.
    fn label(&self) -> &str {
        "Commits"
    }
}
