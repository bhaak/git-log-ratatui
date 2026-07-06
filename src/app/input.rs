use crate::clipboard;
use crate::error::AppError;
use crate::models::*;
use crate::text_utils;
use crate::ui;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::Rect;

use super::App;
use super::{PAGE_SIZE, POLL_BACKOFF_STEP, POLL_INTERVAL_DEFAULT, POLL_INTERVAL_MAX};

/// Outcome of processing an event.
pub enum EventOutcome {
    Continue,
    Quit,
}

// --- Events ---

pub(crate) fn handle_event(app: &mut App) -> Result<EventOutcome, AppError> {
    let interval = std::time::Duration::from_millis(app.poll_interval_ms as u64);
    if !event::poll(interval)? {
        // No event received, back off slowly.
        app.poll_interval_ms = (app.poll_interval_ms + POLL_BACKOFF_STEP).min(POLL_INTERVAL_MAX);
        return Ok(EventOutcome::Continue);
    }

    // Event received, reset polling interval.
    app.poll_interval_ms = POLL_INTERVAL_DEFAULT;
    let ev = event::read()?;

    match ev {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            app.dirty = true;
            handle_key(app, key)
        }
        Event::Mouse(mouse) => {
            // Ignore move events — they don't change anything visible.
            if let MouseEventKind::Moved = mouse.kind {
                return Ok(EventOutcome::Continue);
            }
            app.dirty = true;
            if let MouseEventKind::Down(_) | MouseEventKind::Drag(_) = mouse.kind {
                app.last_mouse_pos = Some((mouse.column, mouse.row));
            }
            if let MouseEventKind::Up(_) = mouse.kind {
                app.dragging = None;
            }
            handle_mouse(app, mouse);
            Ok(EventOutcome::Continue)
        }
        Event::Resize(w, h) => {
            app.dirty = true;
            app.last_size = Some((w, h));
            Ok(EventOutcome::Continue)
        }
        _ => Ok(EventOutcome::Continue),
    }
}

pub(crate) fn handle_key(app: &mut App, key: KeyEvent) -> Result<EventOutcome, AppError> {
    // Quit
    match key.code {
        KeyCode::Char('q') => return Ok(EventOutcome::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(EventOutcome::Quit)
        }
        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            suspend(app);
            return Ok(EventOutcome::Continue);
        }
        _ => {}
    }

    // Global keys — work regardless of focus
    match key.code {
        // Focus cycling
        KeyCode::Tab => {
            app.focus = if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.focus.prev()
            } else {
                app.focus.next()
            };
            return Ok(EventOutcome::Continue);
        }
        // BackTab: terminals that send ESC [ Z for Shift+Tab
        KeyCode::BackTab => {
            app.focus = app.focus.prev();
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('l') => {
            app.focus = app.focus.next();
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('h') => {
            app.focus = app.focus.prev();
            return Ok(EventOutcome::Continue);
        }
        // Scope cycling — global
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            cycle_scope(app);
            return Ok(EventOutcome::Continue);
        }
        // Clipboard — global
        KeyCode::Char('y') => {
            if app.visible_count() > 0 {
                let ci = app.visible_to_filtered(app.selected_index);
                if let Some(c) = app
                    .filtered_commits
                    .as_deref()
                    .unwrap_or(&app.all_commits)
                    .get(ci)
                {
                    let short = if c.hash.len() > ui::commit_table::SHORT_HASH_LEN {
                        &c.hash[..ui::commit_table::SHORT_HASH_LEN]
                    } else {
                        &c.hash
                    };
                    let _ = clipboard::copy_to_clipboard(short);
                }
            }
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('Y') => {
            if app.visible_count() > 0 {
                let ci = app.visible_to_filtered(app.selected_index);
                if let Some(c) = app
                    .filtered_commits
                    .as_deref()
                    .unwrap_or(&app.all_commits)
                    .get(ci)
                {
                    let _ = clipboard::copy_to_clipboard(&c.hash);
                }
            }
            return Ok(EventOutcome::Continue);
        }
        // Paste — global
        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_paste(app);
            return Ok(EventOutcome::Continue);
        }
        // Search editing — global (when not handled by focused panel)
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.focus == Panel::Search {
                app.cursor_pos = 0;
            } else {
                app.focus = Panel::Search;
                app.cursor_pos = 0;
            }
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.focus == Panel::Search {
                app.cursor_pos = app.search_query.len();
            } else {
                app.focus = Panel::Search;
                app.cursor_pos = app.search_query.len();
            }
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Esc => {
            if app.focus == Panel::Search {
                app.search_query.clear();
                app.cursor_pos = 0;
                app.apply_search_filter();
            }
            return Ok(EventOutcome::Continue);
        }
        // Toggle simplified graph (colored bullets, no box-drawing lines)
        KeyCode::Char('g') if key.modifiers.is_empty() && app.focus == Panel::Commits => {
            app.toggle_simplified_graph();
            return Ok(EventOutcome::Continue);
        }
        _ => {}
    }

    // Vim navigation keys (global alternative for up/down)
    match key.code {
        KeyCode::Char('j') => {
            move_down(app);
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('k') => {
            move_up(app);
            return Ok(EventOutcome::Continue);
        }
        _ => {}
    }

    // Panel-specific keys
    match app.focus {
        Panel::Branches => handle_branch_keys(app, key),
        Panel::Search => handle_search_keys(app, key),
        Panel::Scope => handle_scope_keys(app, key),
        Panel::Commits => handle_commit_keys(app, key),
        Panel::Diff => handle_diff_keys(app, key),
    }

    Ok(EventOutcome::Continue)
}

fn move_down(app: &mut App) {
    match app.focus {
        Panel::Branches => {
            if app.branch_index + 1 < app.branch_tree.len() {
                app.branch_index += 1;
            } else {
                app.branch_index = 0;
            }
        }
        Panel::Commits if app.visible_count() > 0 => {
            app.selected_index = (app.selected_index + 1) % app.visible_count();
        }
        Panel::Diff if !app.file_entries.is_empty() => {
            app.selected_file_index = (app.selected_file_index + 1) % app.file_entries.len();
        }
        _ => {}
    }
}

fn move_up(app: &mut App) {
    match app.focus {
        Panel::Branches => {
            if app.branch_index > 0 {
                app.branch_index -= 1;
            } else if !app.branch_tree.is_empty() {
                app.branch_index = app.branch_tree.len() - 1;
            }
        }
        Panel::Commits if app.visible_count() > 0 => {
            if app.selected_index > 0 {
                app.selected_index -= 1;
            } else {
                app.selected_index = app.visible_count() - 1;
            }
        }
        Panel::Diff if !app.file_entries.is_empty() => {
            if app.selected_file_index > 0 {
                app.selected_file_index -= 1;
            } else {
                app.selected_file_index = app.file_entries.len() - 1;
            }
        }
        _ => {}
    }
}

fn cycle_scope(app: &mut App) {
    app.branch_scope = app.branch_scope.next();
    app.branch_index = 0;
    app.expanded_nodes.clear();
    app.search_query.clear();
    app.cursor_pos = 0;
    app.selected_branch = None;
    app.request_branches();
    app.request_commits(None);
}

fn handle_paste(app: &mut App) {
    if let Some(text) = clipboard::get_clipboard_text() {
        app.search_query = text;
        app.cursor_pos = app.search_query.len();
        app.apply_search_filter();
        app.focus = Panel::Search;
    }
}

fn suspend(_app: &mut App) {
    let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen,);
    disable_raw_mode().ok();

    #[cfg(unix)]
    unsafe {
        libc::kill(libc::getpid(), libc::SIGTSTP);
    }

    enable_raw_mode().ok();
    let _ = execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture,);
}

// --- Panel key handlers ---

fn handle_branch_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up => {
            app.branch_index = app.branch_index.saturating_sub(1);
        }
        KeyCode::Down if app.branch_index + 1 < app.branch_tree.len() => {
            app.branch_index += 1;
        }
        KeyCode::Right => {
            let action = app
                .branch_tree
                .get(app.branch_index)
                .filter(|item| item.expandable && !item.expanded)
                .map(|item| item.key.clone());
            if let Some(key) = action {
                app.expanded_nodes.insert(key, true);
                app.rebuild_branch_tree();
            }
        }
        KeyCode::Left => {
            let action = app
                .branch_tree
                .get(app.branch_index)
                .filter(|item| item.expandable && item.expanded)
                .map(|item| item.key.clone());
            if let Some(key) = action {
                app.expanded_nodes.insert(key, false);
                app.rebuild_branch_tree();
            }
        }
        KeyCode::Char(' ') => {
            let action = app
                .branch_tree
                .get(app.branch_index)
                .filter(|item| item.expandable)
                .map(|item| (item.key.clone(), !item.expanded));
            if let Some((key, new_state)) = action {
                app.expanded_nodes.insert(key, new_state);
                app.rebuild_branch_tree();
            }
        }
        KeyCode::Enter => {
            let action = app.branch_tree.get(app.branch_index).map(|item| {
                if item.is_branch {
                    (Some(item.full_path.clone()), None)
                } else if item.expandable {
                    (None, Some((item.key.clone(), !item.expanded)))
                } else {
                    (None, None)
                }
            });
            if let Some((branch_path, toggle)) = action {
                if let Some(path) = branch_path {
                    app.request_commits(Some(path));
                    app.focus = Panel::Commits;
                } else if let Some((key, new_state)) = toggle {
                    app.expanded_nodes.insert(key, new_state);
                    app.rebuild_branch_tree();
                }
            }
        }
        KeyCode::PageUp => {
            app.branch_index = app.branch_index.saturating_sub(PAGE_SIZE);
        }
        KeyCode::PageDown => {
            app.branch_index =
                (app.branch_index + PAGE_SIZE).min(app.branch_tree.len().saturating_sub(1));
        }
        _ => {}
    }
}

fn handle_scope_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter | KeyCode::Char(' ') => {
            cycle_scope(app);
        }
        _ => {}
    }
}

fn handle_search_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.search_query.clear();
            app.cursor_pos = 0;
            app.apply_search_filter();
        }
        KeyCode::Backspace if app.cursor_pos > 0 => {
            let prev = text_utils::prev_char_boundary(&app.search_query, app.cursor_pos);
            app.search_query.remove(prev);
            app.cursor_pos = prev;
            app.apply_search_filter();
        }
        KeyCode::Delete if app.cursor_pos < app.search_query.len() => {
            let pos = app.cursor_pos;
            app.search_query.remove(pos);
            app.apply_search_filter();
        }
        KeyCode::Left => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                app.cursor_pos = text_utils::prev_word_boundary(&app.search_query, app.cursor_pos);
            } else {
                app.cursor_pos = text_utils::prev_char_boundary(&app.search_query, app.cursor_pos);
            }
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                app.cursor_pos = text_utils::next_word_boundary(&app.search_query, app.cursor_pos);
            } else {
                app.cursor_pos = text_utils::next_char_boundary(&app.search_query, app.cursor_pos);
            }
        }
        KeyCode::Home => {
            app.cursor_pos = 0;
        }
        KeyCode::End => {
            app.cursor_pos = app.search_query.len();
        }
        KeyCode::Char(ch) => {
            let pos = app.cursor_pos;
            app.search_query.insert(pos, ch);
            app.cursor_pos += ch.len_utf8();
            app.apply_search_filter();
        }
        _ => {}
    }
}

fn handle_commit_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up if app.visible_count() > 0 => {
            if app.selected_index > 0 {
                app.selected_index -= 1;
            } else {
                app.selected_index = app.visible_count() - 1;
            }
        }
        KeyCode::Down if app.visible_count() > 0 => {
            app.selected_index = (app.selected_index + 1) % app.visible_count();
        }
        KeyCode::Enter => {
            app.focus = Panel::Diff;
        }
        KeyCode::PageUp => {
            app.selected_index = app.selected_index.saturating_sub(PAGE_SIZE);
            app.clamp_selection();
        }
        KeyCode::PageDown if app.visible_count() > 0 => {
            app.selected_index =
                (app.selected_index + PAGE_SIZE).min(app.visible_count().saturating_sub(1));
        }
        _ => {}
    }
}

fn handle_diff_keys(app: &mut App, key: KeyEvent) {
    // Determine if we're scrolled past the metadata+file section
    let file_section_end =
        ui::diff_panel::diff_line_offset(app.commit_info.as_ref(), &app.file_entries);
    let past_meta = app.diff_scroll >= file_section_end || app.file_entries.is_empty();

    match key.code {
        KeyCode::Up => {
            if past_meta {
                app.diff_scroll = app.diff_scroll.saturating_sub(1);
            } else {
                if app.selected_file_index > 0 {
                    app.selected_file_index -= 1;
                } else {
                    app.selected_file_index = app.file_entries.len() - 1;
                }
            }
        }
        KeyCode::Down => {
            if past_meta {
                app.diff_scroll += 1;
            } else {
                app.selected_file_index = (app.selected_file_index + 1) % app.file_entries.len();
            }
        }
        KeyCode::Enter => {
            if let Some(entry) = app.file_entries.get(app.selected_file_index) {
                let offset =
                    ui::diff_panel::diff_line_offset(app.commit_info.as_ref(), &app.file_entries);
                app.diff_scroll = entry.diff_line + offset;
            }
        }
        KeyCode::Char('n') if !app.file_entries.is_empty() => {
            app.selected_file_index = (app.selected_file_index + 1) % app.file_entries.len();
            let offset =
                ui::diff_panel::diff_line_offset(app.commit_info.as_ref(), &app.file_entries);
            if let Some(entry) = app.file_entries.get(app.selected_file_index) {
                app.diff_scroll = entry.diff_line + offset;
            }
        }
        KeyCode::Char('p') if !app.file_entries.is_empty() => {
            if app.selected_file_index > 0 {
                app.selected_file_index -= 1;
                let offset =
                    ui::diff_panel::diff_line_offset(app.commit_info.as_ref(), &app.file_entries);
                if let Some(entry) = app.file_entries.get(app.selected_file_index) {
                    app.diff_scroll = entry.diff_line + offset;
                }
            } else {
                // Wrap from first: reset to top
                app.diff_scroll = 0;
                app.selected_file_index = 0;
            }
        }
        KeyCode::Home => {
            app.diff_scroll = 0;
        }
        KeyCode::End => {
            // scroll to end
            app.diff_scroll = usize::MAX;
        }
        KeyCode::PageUp => {
            let page = app.diff_scrollbar.viewport_length().max(1);
            app.diff_scroll = app.diff_scroll.saturating_sub(page);
        }
        KeyCode::PageDown => {
            let page = app.diff_scrollbar.viewport_length().max(1);
            app.diff_scroll = app.diff_scroll.saturating_add(page);
        }
        _ => {}
    }
}

// --- Mouse handling ---

pub(crate) fn handle_mouse(app: &mut App, mouse: event::MouseEvent) {
    use crossterm::event::MouseButton;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            handle_mouse_click(app, mouse.column, mouse.row);
            check_resize_start(app, mouse.column, mouse.row);
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            handle_mouse_drag(app, mouse.column, mouse.row);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            app.dragging = None;
            app.scrollbar_drag = None;
        }
        MouseEventKind::ScrollDown => {
            handle_scroll_at(app, mouse.column, mouse.row, 1);
        }
        MouseEventKind::ScrollUp => {
            handle_scroll_at(app, mouse.column, mouse.row, -1);
        }
        _ => {}
    }
}

pub(crate) fn handle_mouse_click(app: &mut App, col: u16, row: u16) {
    let Some((tw, th)) = app.last_size else {
        return;
    };
    let full = Rect::new(0, 0, tw, th);
    let areas = ui::layout::compute_areas(full, app.branch_width_pct, app.diff_height_pct);
    // Trim bottom rows occupied by the help bar (same as in render)
    let help_h = ui::layout::HELP_BAR_HEIGHT.min(th);
    let branch_visible_area = Rect::new(
        areas.branch.x,
        areas.branch.y,
        areas.branch.width,
        areas.branch.height.saturating_sub(help_h),
    );
    let click_pos = (col, row);

    // Scrollbar click handling — intercept before content click.
    // Scrollbar clicks scroll without changing the selected item.
    if let Some(_new_pos) = app
        .branch_scrollbar
        .click_to_index(branch_visible_area, click_pos)
    {
        app.dragging = None;
        app.focus = Panel::Branches;
        app.scrollbar_drag = Some(Panel::Branches);
        return;
    }
    if let Some(_new_pos) = app.table_scrollbar.click_to_index(areas.table, click_pos) {
        app.dragging = None;
        app.focus = Panel::Commits;
        app.scrollbar_drag = Some(Panel::Commits);
        return;
    }
    if let Some(new_pos) = app.diff_scrollbar.click_to_index(areas.diff, click_pos) {
        app.dragging = None;
        app.focus = Panel::Diff;
        app.scrollbar_drag = Some(Panel::Diff);
        app.diff_scroll = new_pos;
        return;
    }

    if ui::layout::rect_contains_interior(&branch_visible_area, click_pos) {
        app.focus = Panel::Branches;
        let rel_row = (row
            .saturating_sub(areas.branch.y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.branch_list_offset;
        if actual_index < app.branch_tree.len() {
            app.branch_index = actual_index;
            let action = app.branch_tree.get(actual_index).map(|item| {
                if item.is_branch {
                    (Some(item.full_path.clone()), None)
                } else if item.expandable {
                    (None, Some((item.key.clone(), !item.expanded)))
                } else {
                    (None, None)
                }
            });
            if let Some((branch_path, toggle)) = action {
                if let Some(path) = branch_path {
                    app.request_commits(Some(path));
                    app.focus = Panel::Commits;
                } else if let Some((key, new_state)) = toggle {
                    app.expanded_nodes.insert(key, new_state);
                    app.rebuild_branch_tree();
                }
            }
        }
    } else if ui::layout::rect_contains(&areas.scope, click_pos) {
        app.focus = Panel::Scope;
        cycle_scope(app);
    } else if ui::layout::rect_contains(&areas.search, click_pos) {
        app.focus = Panel::Search;
    } else if ui::layout::rect_contains_interior(&areas.table, click_pos) {
        app.focus = Panel::Commits;
        let rel_row = (row
            .saturating_sub(areas.table.y)
            .saturating_sub(ui::layout::TABLE_OVERHEAD.saturating_sub(ui::layout::BORDER_OVERHEAD)))
            as usize;
        let filtered_idx = rel_row + app.table_state.offset();
        if let Some(vis_idx) = app.filtered_to_visible(filtered_idx) {
            app.selected_index = vis_idx;
        }
    } else if ui::layout::rect_contains(&areas.diff, click_pos) {
        app.focus = Panel::Diff;
        let rel_row = (row
            .saturating_sub(areas.diff.y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        // Check if clicking on a file entry after metadata
        let meta_offset = if let Some(ref info) = app.commit_info {
            ui::diff_panel::build_metadata_lines(info).len()
        } else {
            0
        };
        if rel_row > meta_offset && rel_row <= meta_offset + app.file_entries.len() + 2 {
            let file_idx = rel_row - meta_offset - 1;
            if file_idx < app.file_entries.len() {
                app.selected_file_index = file_idx;
                // Scroll to the file's diff section
                let offset =
                    ui::diff_panel::diff_line_offset(app.commit_info.as_ref(), &app.file_entries);
                if let Some(entry) = app.file_entries.get(file_idx) {
                    app.diff_scroll = entry.diff_line + offset;
                }
            }
        }
    }
}

fn check_resize_start(app: &mut App, col: u16, row: u16) {
    let Some((tw, th)) = app.last_size else {
        return;
    };
    let full = Rect::new(0, 0, tw, th);
    let areas = ui::layout::compute_areas(full, app.branch_width_pct, app.diff_height_pct);

    if ui::layout::is_on_vertical_border(col, row, areas.branch) {
        app.dragging = Some(ui::layout::DragDirection::Vertical);
        return;
    }

    if ui::layout::is_on_horizontal_border(col, row, areas.right, areas.table) {
        app.dragging = Some(ui::layout::DragDirection::Horizontal);
    }
}

fn handle_mouse_drag(app: &mut App, col: u16, row: u16) {
    // Scrollbar dragging — update scroll position proportionally
    if let Some(panel) = app.scrollbar_drag {
        let Some((tw, th)) = app.last_size else {
            return;
        };
        let full = Rect::new(0, 0, tw, th);
        let areas = ui::layout::compute_areas(full, app.branch_width_pct, app.diff_height_pct);
        let help_h = ui::layout::HELP_BAR_HEIGHT.min(th);
        let branch_visible_area = Rect::new(
            areas.branch.x,
            areas.branch.y,
            areas.branch.width,
            areas.branch.height.saturating_sub(help_h),
        );
        match panel {
            Panel::Branches => {
                // Scrollbar drag scrolls but does not change the selected item.
                let _ = app
                    .branch_scrollbar
                    .click_to_index(branch_visible_area, (col, row));
            }
            Panel::Commits => {
                // Scrollbar drag scrolls but does not change the selected item.
                let _ = app.table_scrollbar.click_to_index(areas.table, (col, row));
            }
            Panel::Diff => {
                if let Some(new_pos) = app.diff_scrollbar.click_to_index(areas.diff, (col, row)) {
                    app.diff_scroll = new_pos;
                }
            }
            _ => {}
        }
        return;
    }

    match app.dragging {
        Some(ui::layout::DragDirection::Vertical) => {
            if let Some((tw, _)) = app.last_size {
                app.branch_width_pct = ui::layout::vertical_resize_pct(col, tw);
            }
        }
        Some(ui::layout::DragDirection::Horizontal) => {
            if let Some((_, th)) = app.last_size {
                app.diff_height_pct = ui::layout::horizontal_resize_pct(row, th);
            }
        }
        None => {}
    }
}

/// Scroll the panel under the mouse cursor (position-aware).
fn handle_scroll_at(app: &mut App, col: u16, row: u16, direction: i32) {
    let Some((tw, th)) = app.last_size else {
        return;
    };
    let full = Rect::new(0, 0, tw, th);
    let areas = ui::layout::compute_areas(full, app.branch_width_pct, app.diff_height_pct);
    let pos = (col, row);

    if ui::layout::rect_contains(&areas.branch, pos) {
        if direction > 0 {
            // Scroll down
            if app.branch_index + 1 < app.branch_tree.len() {
                app.branch_index += 1;
            } else {
                app.branch_index = 0;
            }
        } else {
            // Scroll up
            if app.branch_index > 0 {
                app.branch_index -= 1;
            } else if !app.branch_tree.is_empty() {
                app.branch_index = app.branch_tree.len() - 1;
            }
        }
    } else if ui::layout::rect_contains(&areas.table, pos) {
        if app.visible_count() > 0 {
            if direction > 0 {
                app.selected_index = (app.selected_index + 1) % app.visible_count();
            } else {
                if app.selected_index > 0 {
                    app.selected_index -= 1;
                } else {
                    app.selected_index = app.visible_count() - 1;
                }
            }
        }
    } else if ui::layout::rect_contains(&areas.diff, pos) {
        if direction > 0 {
            app.diff_scroll += 1;
        } else {
            app.diff_scroll = app.diff_scroll.saturating_sub(1);
        }
    }
}

#[allow(dead_code)]
fn handle_scroll_down(app: &mut App) {
    match app.focus {
        Panel::Branches => {
            if app.branch_index + 1 < app.branch_tree.len() {
                app.branch_index += 1;
            } else {
                app.branch_index = 0;
            }
        }
        Panel::Commits if app.visible_count() > 0 => {
            app.selected_index = (app.selected_index + 1) % app.visible_count();
        }
        Panel::Diff => {
            app.diff_scroll += 1;
        }
        _ => {}
    }
}

#[allow(dead_code)]
fn handle_scroll_up(app: &mut App) {
    match app.focus {
        Panel::Branches => {
            if app.branch_index > 0 {
                app.branch_index -= 1;
            } else if !app.branch_tree.is_empty() {
                app.branch_index = app.branch_tree.len() - 1;
            }
        }
        Panel::Commits if app.visible_count() > 0 => {
            if app.selected_index > 0 {
                app.selected_index -= 1;
            } else {
                app.selected_index = app.visible_count() - 1;
            }
        }
        Panel::Diff => {
            app.diff_scroll = app.diff_scroll.saturating_sub(1);
        }
        _ => {}
    }
}
