use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::models::BranchScope;

/// Render the scope panel showing the current branch scope.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    scope: BranchScope,
    is_focused: bool,
    debug_label: Option<&str>,
) {
    if area.width < 3 || area.height < 2 {
        return;
    }

    let border_style = if is_focused {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };

    let text = format!(" {}", scope.label());

    let title = if let Some(label) = debug_label {
        format!(" Scope [{}] ", label)
    } else {
        " Scope ".to_string()
    };

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(paragraph, area);
}

use crate::app::state::AppState;
use crate::ui::panel::Panel;

/// Wrapper struct implementing the Panel trait for the scope indicator.
pub struct ScopePanel;

impl Panel for ScopePanel {
    fn render(&self, area: Rect, frame: &mut Frame, state: &AppState, is_focused: bool) {
        render(frame, area, state.branch_scope, is_focused, None);
    }

    fn help_keys(&self) -> &[(&str, &str)] {
        &[("Enter/Space", "cycle scope")]
    }

    fn label(&self) -> &str {
        "Scope"
    }
}
