use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::domain::BranchScope;
use crate::theme::Theme;

/// Render the scope panel showing the current branch scope.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    scope: BranchScope,
    is_focused: bool,
    debug_label: Option<&str>,
    theme: &Theme,
) {
    if area.width < 3 || area.height < 2 {
        return;
    }

    let border_style = if is_focused {
        Style::default().fg(theme.focused_border)
    } else {
        Style::default().fg(theme.unfocused_border)
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
                .fg(theme.scope_text)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(paragraph, area);
}

use crate::app::commands::Command;
use crate::ui::panel::Panel;
use crate::ui::render_ctx::RenderCtx;

/// Wrapper struct implementing the Panel trait for the scope indicator.
pub struct ScopePanel;

impl Panel for ScopePanel {
    type State = BranchScope;

    fn render(&self, area: Rect, frame: &mut Frame, state: &mut Self::State, ctx: &RenderCtx) {
        render(
            frame,
            area,
            *state,
            ctx.is_focused(crate::view::Panel::Scope),
            ctx.debug_label,
            ctx.theme,
        );
    }

    fn handle_event(&mut self, key: &KeyEvent, _state: &mut Self::State) -> Vec<Command> {
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            return vec![Command::CycleScope];
        }
        Vec::new()
    }

    fn help_keys(&self) -> &[(&str, &str)] {
        &[("Enter/Space", "cycle scope")]
    }

    fn label(&self) -> &str {
        "Scope"
    }
}
