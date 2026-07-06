use ratatui::{
    layout::Rect,
    style::{Color, Style},
    Frame,
};

use crate::app::PAGE_SIZE;
use crate::models::*;
use crate::ui;

use super::search;
use super::viewport;
use super::App;

pub(crate) fn render(app: &mut App, frame: &mut Frame) {
    let full = frame.area();
    app.last_size = Some((full.width, full.height));

    // Guard against zero-size terminal (can happen during resize)
    if full.width < ui::layout::MIN_TERM_WIDTH || full.height < ui::layout::MIN_TERM_HEIGHT {
        return;
    }

    let debug_label = if app.debug {
        Some(format!("{}ms", app.last_frame_time_ms))
    } else {
        None
    };
    let debug_label = debug_label.as_deref();

    let areas = ui::layout::compute_areas(full, app.branch_width_pct, app.diff_height_pct);

    let help_area = Rect::new(
        full.x,
        full.y + full.height.saturating_sub(ui::layout::HELP_BAR_HEIGHT),
        full.width,
        ui::layout::HELP_BAR_HEIGHT.min(full.height),
    );

    // Clamp cursor
    app.cursor_pos = app.cursor_pos.min(app.search_query.len());
    search::clamp_selection(&mut app.state);

    // Incrementally load older commits when the selection nears the end of
    // what's currently loaded. Skipped while a search filter is active (the
    // filtered view isn't a reliable proxy for the loaded window) and while a
    // load is already in flight (commits_loaded == false).
    if !app.all_commits_loaded && app.commits_loaded && app.filtered_commits.is_none() {
        let loaded = search::visible_count(&app.state);
        if loaded > 0 && app.selected_index + PAGE_SIZE >= loaded {
            viewport::request_more_commits(&mut app.state, &app.commit_worker);
        }
    }

    // --- Branch panel (content + scrollbar) ---
    // Trim bottom so the help bar does not overwrite the panel border
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
        &app.branch_tree,
        app.branch_index,
        app.focus == Panel::Branches,
        debug_label,
    );

    app.branch_list_offset = branch_list_state.offset();

    let branch_focus_style = if app.focus == Panel::Branches {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };
    let branch_visible = (branch_content_area
        .height
        .saturating_sub(ui::layout::PANEL_BORDER_H)) as usize;
    let branch_tree_len = app.branch_tree.len();
    let branch_offset = branch_list_state.offset();
    app.branch_scrollbar.render(
        frame,
        branch_scrollbar_area,
        branch_tree_len,
        branch_visible,
        branch_offset,
        branch_focus_style,
    );

    // --- Search panel ---

    let branch_label = app.selected_branch.as_deref().unwrap_or("all branches");
    let title = format!("Git Log - {} [{}]", app.repo_path, branch_label);
    ui::search_panel::render(
        frame,
        areas.search,
        &app.search_query,
        app.cursor_pos,
        branch_label,
        &title,
        app.focus == Panel::Search,
        debug_label,
    );

    ui::scope_panel::render(
        frame,
        areas.scope,
        app.branch_scope,
        app.focus == Panel::Scope,
        debug_label,
    );

    // --- Commit table (content + scrollbar) ---
    let (table_content_area, table_scrollbar_area) =
        ui::scrollbar_view::ScrollbarView::split(areas.table);

    let state = &mut app.state;
    let table_ctx = ui::commit_table::CommitTableCtx {
        commits: state
            .filtered_commits
            .as_deref()
            .unwrap_or(&state.all_commits),
        visible_index: state.selected_index,
        is_focused: state.focus == Panel::Commits,
        visible_to_commit: &state.visible_to_commit,
        total_loaded: state.all_commits.len(),
        search_active: !state.search_query.is_empty(),
        simplified_graph: state.simplified_graph,
        debug_label,
    };
    ui::commit_table::render(
        frame,
        table_content_area,
        &table_ctx,
        &mut state.table_state,
    );

    let table_focus_style = if app.focus == Panel::Commits {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };
    let table_visible = (table_content_area
        .height
        .saturating_sub(ui::layout::TABLE_OVERHEAD)) as usize;
    let table_item_count = app
        .filtered_commits
        .as_deref()
        .unwrap_or(&app.all_commits)
        .len();
    let table_offset = app.table_state.offset();
    app.table_scrollbar.render(
        frame,
        table_scrollbar_area,
        table_item_count,
        table_visible,
        table_offset,
        table_focus_style,
    );

    let short_hash = app
        .commit_info
        .as_ref()
        .map(|info| &info.hash[..std::cmp::min(ui::commit_table::SHORT_HASH_LEN, info.hash.len())]);

    // --- Diff panel (content + scrollbar) ---
    let (diff_content_area, diff_scrollbar_area) =
        ui::scrollbar_view::ScrollbarView::split(areas.diff);

    let diff_ctx = ui::diff_panel::DiffPanelCtx {
        commit_info: app.commit_info.as_ref(),
        diff_lines: &app.diff_lines,
        file_entries: &app.file_entries,
        selected_file_index: app.selected_file_index,
        diff_scroll: app.diff_scroll,
        is_focused: app.focus == Panel::Diff,
        short_hash,
        debug_label,
    };
    let diff_total_lines = ui::diff_panel::render(frame, diff_content_area, &diff_ctx);

    let diff_focus_style = if app.focus == Panel::Diff {
        Style::default().fg(Color::Rgb(180, 140, 255))
    } else {
        Style::default().fg(Color::Gray)
    };
    let diff_visible = (diff_content_area
        .height
        .saturating_sub(ui::layout::PANEL_BORDER_H)) as usize;
    let diff_scroll_val = app.diff_scroll;
    app.diff_scrollbar.render(
        frame,
        diff_scrollbar_area,
        diff_total_lines,
        diff_visible,
        diff_scroll_val,
        diff_focus_style,
    );

    ui::help_bar::render(frame, help_area, app.focus, app.status_message.as_deref());

    // Trigger diff load on selection change
    let current_hash = if search::visible_count(&app.state) > 0 {
        let ci = search::visible_to_filtered(&app.state, app.selected_index);
        app.filtered_commits
            .as_deref()
            .unwrap_or(&app.all_commits)
            .get(ci)
            .map(|c| c.hash.clone())
    } else {
        None
    };

    if current_hash != app.last_selected_hash {
        app.last_selected_hash = current_hash.clone();
        if let Some(hash) = current_hash {
            if !hash.is_empty() {
                app.request_diff(&hash);
            }
        }
    }
}
