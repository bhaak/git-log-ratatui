use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::theme::Theme;
use crate::view::Panel as PanelEnum;

/// Render the context-sensitive help bar at the bottom of the screen.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    panel_label: &str,
    context_keys: &[(&str, &str)],
    status: Option<&str>,
    theme: &Theme,
) {
    if area.width < 10 || area.height < 2 {
        return;
    }

    let key_style = Style::default()
        .fg(theme.help_title)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let bracket_style = Style::default().fg(theme.unfocused_border);

    let common = [("q", "Quit"), ("Tab", "Focus"), ("l/h", "Next/Prev")];

    let mut spans: Vec<Span> = Vec::new();

    for (key, desc) in common.iter().chain(context_keys.iter()) {
        spans.push(Span::styled("<", bracket_style));
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(": ", bracket_style));
        spans.push(Span::styled(*desc, desc_style));
        spans.push(Span::styled(">", bracket_style));
        spans.push(Span::raw("  "));
    }

    let title = if let Some(msg) = status {
        format!(" {} | {} ", panel_label, msg)
    } else {
        format!(" {} ", panel_label)
    };

    let paragraph = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.unfocused_border)),
    );

    frame.render_widget(paragraph, area);
}

/// Render the help modal overlay showing keybindings relevant to the focused panel.
/// Blocks underlying content with a [`Clear`] widget and draws a centered,
/// bordered panel with Global, Navigation, and focused-panel-specific sections.
pub fn render_help_modal(frame: &mut Frame, full_area: Rect, focus: PanelEnum, theme: &Theme) {
    let key_style = Style::default()
        .fg(theme.help_title)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let section_style = Style::default()
        .fg(theme.focused_border)
        .add_modifier(Modifier::BOLD);

    let border_style = Style::default().fg(theme.focused_border);

    let panel_section = panel_bindings(focus);

    // Calculate modal geometry based on content: fewer sections → shorter modal
    let section_count = 3; // Global + Navigation + focused panel
    let modal_h = (3 + section_count * 6) // rough lines per section
        .min(full_area.height.saturating_sub(2))
        .max(8);
    let modal_w = (full_area.width * 7 / 10)
        .max(48)
        .min(full_area.width.saturating_sub(2));
    let modal_x = full_area.x + (full_area.width.saturating_sub(modal_w)) / 2;
    let modal_y = full_area.y + (full_area.height.saturating_sub(modal_h)) / 2;

    let modal_area = Rect::new(modal_x, modal_y, modal_w, modal_h);

    frame.render_widget(Clear, modal_area);

    let sections: &[(&str, &[(&str, &str)])] = &[
        (
            "Global",
            &[
                ("q / Ctrl+C", "Quit"),
                ("? / Esc", "Close this help"),
                ("/", "Focus search + clear"),
                ("Tab / Shift+Tab", "Focus next / previous panel"),
                ("Esc", "Go back to previous panel"),
                ("Ctrl+S", "Cycle branch scope"),
                ("Ctrl+Z", "Suspend (background)"),
            ],
        ),
        (
            "Navigation",
            &[
                ("h / l", "Focus previous / next panel"),
                ("j / k / \u{2191} / \u{2193}", "Move down / up"),
                ("Ctrl+F / Ctrl+B", "Page down / up"),
                ("G / Home / End", "Jump to bottom / top"),
                ("PgUp / PgDn", "Page up / down"),
            ],
        ),
        panel_section,
    ];

    let mut lines: Vec<Line> = Vec::new();
    for (title, bindings) in sections {
        lines.push(Line::from(Span::styled(
            format!("  {title}"),
            section_style,
        )));
        for (key, desc) in *bindings {
            let spans = vec![
                Span::raw("    "),
                Span::styled(*key, key_style),
                Span::raw("  "),
                Span::styled(*desc, desc_style),
            ];
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
    }

    // Hint about Tab to see other panels' help
    lines.push(Line::from(Span::styled(
        "  Tab to another panel, then ? for its keys",
        Style::default().fg(theme.commit_secondary),
    )));

    let label = match focus {
        PanelEnum::Branches => "Branches",
        PanelEnum::Search => "Search",
        PanelEnum::Scope => "Scope",
        PanelEnum::Commits => "Commits",
        PanelEnum::Diff => "Diff",
    };
    let title = format!(" Help — {label} (? or Esc to close) ");

    let inner = Layout::default()
        .vertical_margin(1)
        .horizontal_margin(2)
        .constraints([Constraint::Percentage(100)])
        .split(modal_area)[0];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    frame.render_widget(paragraph, inner);
}

/// Return the keybinding table for a specific panel.
fn panel_bindings(focus: PanelEnum) -> (&'static str, &'static [(&'static str, &'static str)]) {
    match focus {
        PanelEnum::Branches => (
            "Branches",
            &[
                ("Enter", "Select branch (load commits)"),
                ("Space", "Toggle expand / collapse"),
                ("n / p", "Next / previous sibling"),
                ("\u{2190} / \u{2192}", "Collapse / expand node"),
            ],
        ),
        PanelEnum::Commits => (
            "Commits",
            &[
                ("Enter", "Show diff (focus Diff panel)"),
                ("Space", "Preview diff (keep focus)"),
                ("y / Y", "Copy short / full hash"),
                ("g", "Toggle full / simplified graph"),
            ],
        ),
        PanelEnum::Diff => (
            "Diff",
            &[
                ("Enter", "Jump to selected file's diff"),
                ("n / p", "Next / previous changed file"),
                ("j / k", "Scroll / navigate files"),
                ("Esc", "Back to commits"),
            ],
        ),
        PanelEnum::Search => (
            "Search",
            &[
                ("Esc", "Clear search query"),
                ("Ctrl+A / Ctrl+E", "Jump to start / end of line"),
                ("Ctrl+V", "Paste from clipboard"),
                ("Tab", "Move to next panel"),
            ],
        ),
        PanelEnum::Scope => (
            "Scope",
            &[(
                "Enter / Space",
                "Cycle (All \u{2192} Local \u{2192} Remote)",
            )],
        ),
    }
}
