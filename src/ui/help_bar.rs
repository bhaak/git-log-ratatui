use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::theme::Theme;
use crate::ui::panel::KeyBinding;

/// Render the context-sensitive help bar at the bottom of the screen.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    panel_label: &str,
    context_keys: &[KeyBinding],
    status: Option<&str>,
    theme: &Theme,
) {
    if area.width < 10 || area.height < 2 {
        return;
    }

    let key_style = Style::default()
        .fg(theme.help_title)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(theme.text_primary);
    let bracket_style = Style::default().fg(theme.unfocused_border);

    let common = [
        KeyBinding::new("q", "Quit", "Quit"),
        KeyBinding::new("Tab", "Focus", "Focus"),
        KeyBinding::new("l/h", "Next/Prev", "Next/Prev"),
    ];

    let mut spans: Vec<Span> = Vec::new();

    for kb in common.iter().chain(context_keys.iter()) {
        spans.push(Span::styled("<", bracket_style));
        spans.push(Span::styled(kb.key, key_style));
        spans.push(Span::styled(": ", bracket_style));
        spans.push(Span::styled(kb.short_desc, desc_style));
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
pub fn render_help_modal(
    frame: &mut Frame,
    full_area: Rect,
    panel_label: &str,
    panel_keys: &[KeyBinding],
    theme: &Theme,
) {
    let key_style = Style::default()
        .fg(theme.help_title)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(theme.text_primary);
    let section_style = Style::default()
        .fg(theme.focused_border)
        .add_modifier(Modifier::BOLD);

    let border_style = Style::default().fg(theme.focused_border);

    let global_keys: &[KeyBinding] = &[
        KeyBinding::new("q / Ctrl+C", "Quit", "Quit"),
        KeyBinding::new("? / Esc", "Close help", "Close this help"),
        KeyBinding::new("/", "Search", "Focus search + clear"),
        KeyBinding::new("Tab / Shift+Tab", "Focus", "Focus next / previous panel"),
        KeyBinding::new("Esc", "Go back", "Go back to previous panel"),
        KeyBinding::new(
            "t / T",
            "Theme",
            "Cycle forward/backward through theme presets",
        ),
        KeyBinding::new("Ctrl+S", "Scope", "Cycle branch scope"),
        KeyBinding::new("Ctrl+Z", "Suspend", "Suspend (background)"),
    ];

    let nav_keys: &[KeyBinding] = &[
        KeyBinding::new("h / l", "Focus prev/next", "Focus previous / next panel"),
        KeyBinding::new(
            "b / c / d",
            "Jump panel",
            "Jump to Branches / Commits / Diff",
        ),
        KeyBinding::new("j / k / ↑ / ↓", "Move up/down", "Move down / up in list"),
        KeyBinding::new("Ctrl+F / Ctrl+B", "Page down/up", "Page down / up"),
        KeyBinding::new("G / Home / End", "Jump", "Jump to bottom / top"),
        KeyBinding::new("PgUp / PgDn", "Page", "Page up / down"),
    ];

    let global_items = global_keys.len();
    let nav_items = nav_keys.len();
    let panel_items = panel_keys.len();
    // 3 section titles + all items + 3 blank separators + 1 hint
    let content_lines = 3 + global_items + 3 + nav_items + panel_items + 1;
    let modal_area = compute_modal_area(full_area, content_lines);

    frame.render_widget(Clear, modal_area);

    let sections: [(&str, &[KeyBinding]); 3] = [
        ("Global", global_keys),
        ("Navigation", nav_keys),
        (panel_label, panel_keys),
    ];

    let mut lines: Vec<Line> = Vec::new();
    for (title, bindings) in &sections {
        lines.push(Line::from(Span::styled(
            format!("  {title}"),
            section_style,
        )));
        for kb in *bindings {
            lines.push(build_key_binding_line(kb, key_style, desc_style));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "  Tab / Shift+Tab to see other panels' keys",
        Style::default().fg(theme.commit_secondary),
    )));

    let title = format!(" Help — {panel_label} (? or Esc to close) ");

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

/// Compute the centered modal area rectangle based on content line count.
fn compute_modal_area(full_area: Rect, content_lines: usize) -> Rect {
    let modal_h = (content_lines + 4).min(full_area.height.saturating_sub(2) as usize);
    let modal_w = (full_area.width * 7 / 10)
        .max(50)
        .min(full_area.width.saturating_sub(2));
    let modal_x = full_area.x + (full_area.width.saturating_sub(modal_w)) / 2;
    let modal_y = full_area.y + (full_area.height.saturating_sub(modal_h as u16)) / 2;
    Rect::new(modal_x, modal_y, modal_w, modal_h as u16)
}

/// Build a single formatted line for one key binding: indentation + key + description.
fn build_key_binding_line<'a>(kb: &'a KeyBinding, key_style: Style, desc_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::raw("    "),
        Span::styled(kb.key, key_style),
        Span::raw("  "),
        Span::styled(kb.long_desc, desc_style),
    ])
}
