use crate::domain::{Decoration, DecorationKind};
use crate::theme::Theme;
use crate::time_format::ymd_to_days;
use crate::ui::commit_table::{
    age_color, build_graph_span, build_hash_span, build_simplified_graph, decoration_style,
    graph_only_decorations, hash_color, is_branch_head, render, CommitTableCtx,
};
use crate::view::CommitRow as Commit;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::TableState;

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
