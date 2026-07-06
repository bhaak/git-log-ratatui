use ratatui::{layout::Rect, style::Style, Frame};

use crate::app::PAGE_SIZE;
use crate::models::*;
use crate::ui;

use super::search;
use super::viewport;
use super::App;

pub(crate) fn render(app: &mut App, frame: &mut Frame) {
    let full = frame.area();
    app.state.ui.last_size = Some((full.width, full.height));

    if full.width < ui::layout::MIN_TERM_WIDTH || full.height < ui::layout::MIN_TERM_HEIGHT {
        return;
    }

    let theme = app.state.theme.clone();
    let debug_label = if app.state.debug {
        Some(format!("{}ms", app.state.last_frame_time_ms))
    } else {
        None
    };
    let debug_label = debug_label.as_deref();

    let areas = ui::layout::compute_areas(
        full,
        app.state.ui.branch_width_pct,
        app.state.ui.diff_height_pct,
    );

    let help_area = Rect::new(
        full.x,
        full.y + full.height.saturating_sub(ui::layout::HELP_BAR_HEIGHT),
        full.width,
        ui::layout::HELP_BAR_HEIGHT.min(full.height),
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

    // --- Branch panel (content + scrollbar) ---
    let branch_visible_area = Rect::new(
        areas.branch.x,
        areas.branch.y,
        areas.branch.width,
        areas.branch.height.saturating_sub(help_area.height),
    );
    let (branch_content_area, branch_scrollbar_area) =
        ui::scrollbar_view::ScrollbarView::split(branch_visible_area);

    let branch_list_state = ui::branch_panel::render(
        frame,
        branch_content_area,
        &app.state.branch.branch_tree,
        app.state.branch.branch_index,
        app.state.ui.focus == Panel::Branches,
        debug_label,
        &theme,
    );

    app.state.branch.branch_list_offset = branch_list_state.offset();

    let branch_focus_style = if app.state.ui.focus == Panel::Branches {
        Style::default().fg(theme.focused_border)
    } else {
        Style::default().fg(theme.unfocused_border)
    };
    let branch_visible = (branch_content_area
        .height
        .saturating_sub(ui::layout::PANEL_BORDER_H)) as usize;
    let branch_tree_len = app.state.branch.branch_tree.len();
    let branch_offset = branch_list_state.offset();
    app.state.ui.branch_scrollbar.render(
        frame,
        branch_scrollbar_area,
        branch_tree_len,
        branch_visible,
        branch_offset,
        branch_focus_style,
    );

    // --- Search panel ---

    let branch_label = app
        .state
        .branch
        .selected_branch
        .as_deref()
        .unwrap_or("all branches");
    let title = format!("Git Log - {} [{}]", app.state.repo_path, branch_label);
    ui::search_panel::render(
        frame,
        areas.search,
        &app.state.search.search_query,
        app.state.search.cursor_pos,
        branch_label,
        &title,
        app.state.ui.focus == Panel::Search,
        debug_label,
        &theme,
    );

    ui::scope_panel::render(
        frame,
        areas.scope,
        app.state.branch.branch_scope,
        app.state.ui.focus == Panel::Scope,
        debug_label,
        &theme,
    );

    // --- Commit table (content + scrollbar) ---
    let (table_content_area, table_scrollbar_area) =
        ui::scrollbar_view::ScrollbarView::split(areas.table);

    let table_ctx = ui::commit_table::CommitTableCtx {
        commits: app
            .state
            .commit
            .filtered_commits
            .as_deref()
            .unwrap_or(&app.state.commit.all_commits),
        visible_index: app.state.commit.selected_index,
        is_focused: app.state.ui.focus == Panel::Commits,
        visible_to_commit: &app.state.commit.visible_to_commit,
        total_loaded: app.state.commit.all_commits.len(),
        search_active: !app.state.search.search_query.is_empty(),
        simplified_graph: app.state.commit.simplified_graph,
        debug_label,
        theme: &theme,
    };
    ui::commit_table::render(
        frame,
        table_content_area,
        &table_ctx,
        &mut app.state.commit.table_state,
    );

    let table_focus_style = if app.state.ui.focus == Panel::Commits {
        Style::default().fg(theme.focused_border)
    } else {
        Style::default().fg(theme.unfocused_border)
    };
    let table_visible = (table_content_area
        .height
        .saturating_sub(ui::layout::TABLE_OVERHEAD)) as usize;
    let table_item_count = app
        .state
        .commit
        .filtered_commits
        .as_deref()
        .unwrap_or(&app.state.commit.all_commits)
        .len();
    let table_offset = app.state.commit.table_state.offset();
    app.state.ui.table_scrollbar.render(
        frame,
        table_scrollbar_area,
        table_item_count,
        table_visible,
        table_offset,
        table_focus_style,
    );

    let short_hash =
        app.state.diff.commit_info.as_ref().map(|info| {
            &info.hash[..std::cmp::min(ui::commit_table::SHORT_HASH_LEN, info.hash.len())]
        });

    // --- Diff panel (content + scrollbar) ---
    let (diff_content_area, diff_scrollbar_area) =
        ui::scrollbar_view::ScrollbarView::split(areas.diff);

    let diff_ctx = ui::diff_panel::DiffPanelCtx {
        commit_info: app.state.diff.commit_info.as_ref(),
        diff_lines: &app.state.diff.diff_lines,
        file_entries: &app.state.diff.file_entries,
        selected_file_index: app.state.diff.selected_file_index,
        diff_scroll: app.state.diff.diff_scroll,
        is_focused: app.state.ui.focus == Panel::Diff,
        short_hash,
        debug_label,
        theme: &theme,
    };
    let diff_total_lines = ui::diff_panel::render(frame, diff_content_area, &diff_ctx);

    let diff_focus_style = if app.state.ui.focus == Panel::Diff {
        Style::default().fg(theme.focused_border)
    } else {
        Style::default().fg(theme.unfocused_border)
    };
    let diff_visible = (diff_content_area
        .height
        .saturating_sub(ui::layout::PANEL_BORDER_H)) as usize;
    let diff_scroll_val = app.state.diff.diff_scroll;
    app.state.ui.diff_scrollbar.render(
        frame,
        diff_scrollbar_area,
        diff_total_lines,
        diff_visible,
        diff_scroll_val,
        diff_focus_style,
    );

    ui::help_bar::render(
        frame,
        help_area,
        app.state.ui.focus,
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
