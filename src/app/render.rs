use std::sync::Arc;

use ratatui::{layout::Rect, Frame};

use crate::app::PAGE_SIZE;
use crate::ui;
use crate::ui::panel::Panel;

use super::search;
use super::viewport;
use super::App;

pub(crate) fn render(app: &mut App, frame: &mut Frame) {
    let full = frame.area();
    app.state.ui.last_size = Some((full.width, full.height));

    if full.width < ui::layout::MIN_TERM_WIDTH || full.height < ui::layout::MIN_TERM_HEIGHT {
        return;
    }

    let theme = Arc::clone(&app.state.theme);

    let areas = ui::layout::compute_areas(
        full,
        app.state.ui.branch_width_pct,
        app.state.ui.diff_height_pct,
    );

    app.state.search.cursor_pos = app
        .state
        .search
        .cursor_pos
        .min(app.state.search.search_query.len());
    search::clamp_selection(&mut app.state);

    if !app.state.commit.all_commits_loaded
        && app.state.commit.commits_loaded
        && app.state.commit.filtered_commits.is_none()
    {
        let loaded = search::visible_count(&app.state);
        if loaded > 0 && app.state.commit.selected_index + PAGE_SIZE >= loaded {
            viewport::request_more_commits(&mut app.state, &app.commit_worker);
        }
    }

    let selected_branch_owned = app.state.branch.selected_branch.clone();
    let ctx = ui::render_ctx::RenderCtx {
        focus: app.state.ui.focus,
        debug_label: None,
        theme: &theme,
        repo_path: &app.state.repo_path,
        selected_branch: selected_branch_owned.as_deref(),
        search_active: !app.state.search.search_query.is_empty(),
    };

    // Branch panel (shortened to avoid overlap with help bar)
    let branch_area = Rect::new(
        areas.branch.x,
        areas.branch.y,
        areas.branch.width,
        areas
            .branch
            .height
            .saturating_sub(ui::layout::HELP_BAR_HEIGHT),
    );
    ui::branch_panel::BranchPanel.render(branch_area, frame, &mut app.state.branch, &ctx);

    // Search panel
    ui::search_panel::SearchPanel.render(areas.search, frame, &mut app.state.search, &ctx);

    // Scope panel
    ui::scope_panel::ScopePanel.render(
        areas.scope,
        frame,
        &mut app.state.branch.branch_scope,
        &ctx,
    );

    // Commit table panel — only this panel shows debug metrics in the title.
    let debug_label = if app.state.debug {
        Some(app.state.metrics.format_summary())
    } else {
        None
    };
    let commit_ctx = ui::render_ctx::RenderCtx {
        debug_label: debug_label.as_deref(),
        ..ctx
    };
    ui::commit_table::CommitPanel::new().render(
        areas.table,
        frame,
        &mut app.state.commit,
        &commit_ctx,
    );

    // Diff panel
    ui::diff_panel::DiffPanel.render(areas.diff, frame, &mut app.state.diff, &ctx);

    // Help bar (full width at bottom) — dispatch to focused panel's help_keys + label
    let help_area = Rect::new(
        full.x,
        full.y + full.height.saturating_sub(ui::layout::HELP_BAR_HEIGHT),
        full.width,
        ui::layout::HELP_BAR_HEIGHT.min(full.height),
    );
    let commit_panel = ui::commit_table::CommitPanel::new();
    let (help_keys, label) = match app.state.ui.focus {
        crate::view::Panel::Branches => (
            ui::branch_panel::BranchPanel.help_keys(),
            ui::branch_panel::BranchPanel.label(),
        ),
        crate::view::Panel::Search => (
            ui::search_panel::SearchPanel.help_keys(),
            ui::search_panel::SearchPanel.label(),
        ),
        crate::view::Panel::Scope => (
            ui::scope_panel::ScopePanel.help_keys(),
            ui::scope_panel::ScopePanel.label(),
        ),
        crate::view::Panel::Commits => (commit_panel.help_keys(), commit_panel.label()),
        crate::view::Panel::Diff => (
            ui::diff_panel::DiffPanel.help_keys(),
            ui::diff_panel::DiffPanel.label(),
        ),
    };
    ui::help_bar::render(
        frame,
        help_area,
        label,
        help_keys,
        app.state.ui.status_message.as_deref(),
        &theme,
    );

    let current_hash = if search::visible_count(&app.state) > 0 {
        let ci = search::visible_to_filtered(&app.state, app.state.commit.selected_index);
        app.state
            .commit
            .filtered_commits
            .as_deref()
            .unwrap_or(&app.state.commit.all_commits)
            .get(ci)
            .map(|c| c.hash.clone())
    } else {
        None
    };

    if current_hash != app.state.diff.last_selected_hash {
        app.state.diff.last_selected_hash = current_hash.clone();
        if let Some(hash) = current_hash {
            if !hash.is_empty() {
                app.request_diff(&hash);
            }
        }
    }
}
