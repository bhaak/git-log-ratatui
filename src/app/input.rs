use crate::clipboard;
use crate::error::AppError;
use crate::models::Panel as PanelEnum;
use crate::ui;
use crate::ui::branch_panel::BranchPanel;
use crate::ui::commit_table::CommitPanel;
use crate::ui::diff_panel::DiffPanel;
use crate::ui::panel::Panel;
use crate::ui::scope_panel::ScopePanel;
use crate::ui::search_panel::SearchPanel;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::Rect;

use super::branches;
use super::search;
use super::App;
use super::{POLL_BACKOFF_STEP, POLL_INTERVAL_DEFAULT, POLL_INTERVAL_MAX};

pub enum EventOutcome {
    Continue,
    Quit,
}

// --- Events ---

pub(crate) fn handle_event(app: &mut App) -> Result<EventOutcome, AppError> {
    let interval = std::time::Duration::from_millis(app.state.ui.poll_interval_ms as u64);
    if !event::poll(interval)? {
        app.state.ui.poll_interval_ms =
            (app.state.ui.poll_interval_ms + POLL_BACKOFF_STEP).min(POLL_INTERVAL_MAX);
        return Ok(EventOutcome::Continue);
    }

    app.state.ui.poll_interval_ms = POLL_INTERVAL_DEFAULT;
    let ev = event::read()?;

    match ev {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            app.state.ui.dirty = true;
            handle_key(app, key)
        }
        Event::Mouse(mouse) => {
            if let MouseEventKind::Moved = mouse.kind {
                return Ok(EventOutcome::Continue);
            }
            app.state.ui.dirty = true;
            if let MouseEventKind::Down(_) | MouseEventKind::Drag(_) = mouse.kind {
                app.state.ui.last_mouse_pos = Some((mouse.column, mouse.row));
            }
            if let MouseEventKind::Up(_) = mouse.kind {
                app.state.ui.dragging = None;
            }
            handle_mouse(app, mouse);
            Ok(EventOutcome::Continue)
        }
        Event::Resize(w, h) => {
            app.state.ui.dirty = true;
            app.state.ui.last_size = Some((w, h));
            Ok(EventOutcome::Continue)
        }
        _ => Ok(EventOutcome::Continue),
    }
}

pub(crate) fn handle_key(app: &mut App, key: KeyEvent) -> Result<EventOutcome, AppError> {
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
        KeyCode::Tab => {
            app.state.ui.focus = if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.state.ui.focus.prev()
            } else {
                app.state.ui.focus.next()
            };
            return Ok(EventOutcome::Continue);
        }
        KeyCode::BackTab => {
            app.state.ui.focus = app.state.ui.focus.prev();
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('l') => {
            app.state.ui.focus = app.state.ui.focus.next();
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('h') => {
            app.state.ui.focus = app.state.ui.focus.prev();
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            cycle_scope(app);
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('y') => {
            if search::visible_count(&app.state) > 0 {
                let ci = search::visible_to_filtered(&app.state, app.state.commit.selected_index);
                if let Some(c) = app
                    .state
                    .commit
                    .filtered_commits
                    .as_deref()
                    .unwrap_or(&app.state.commit.all_commits)
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
            if search::visible_count(&app.state) > 0 {
                let ci = search::visible_to_filtered(&app.state, app.state.commit.selected_index);
                if let Some(c) = app
                    .state
                    .commit
                    .filtered_commits
                    .as_deref()
                    .unwrap_or(&app.state.commit.all_commits)
                    .get(ci)
                {
                    let _ = clipboard::copy_to_clipboard(&c.hash);
                }
            }
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_paste(app);
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.state.ui.focus == PanelEnum::Search {
                app.state.search.cursor_pos = 0;
            } else {
                app.state.ui.focus = PanelEnum::Search;
                app.state.search.cursor_pos = 0;
            }
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.state.ui.focus == PanelEnum::Search {
                app.state.search.cursor_pos = app.state.search.search_query.len();
            } else {
                app.state.ui.focus = PanelEnum::Search;
                app.state.search.cursor_pos = app.state.search.search_query.len();
            }
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Esc => {
            if app.state.ui.focus == PanelEnum::Search {
                app.state.search.search_query.clear();
                app.state.search.cursor_pos = 0;
                search::apply_search_filter(&mut app.state);
            }
            return Ok(EventOutcome::Continue);
        }
        KeyCode::Char('g')
            if key.modifiers.is_empty() && app.state.ui.focus == PanelEnum::Commits =>
        {
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

    // Panel-specific key dispatch via Panel trait.
    match app.state.ui.focus {
        PanelEnum::Branches if key.code == KeyCode::Enter => {
            if let Some(item) = app
                .state
                .branch
                .branch_tree
                .get(app.state.branch.branch_index)
            {
                if item.is_branch {
                    app.request_commits(Some(item.full_path.clone()));
                    app.state.ui.focus = PanelEnum::Commits;
                    return Ok(EventOutcome::Continue);
                }
            }
        }
        PanelEnum::Scope if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) => {
            cycle_scope(app);
            return Ok(EventOutcome::Continue);
        }
        _ => {}
    }

    // Dispatch to the focused panel via the Panel trait.
    match app.state.ui.focus {
        PanelEnum::Branches => {
            let mut panel = BranchPanel;
            panel.handle_event(&Event::Key(key), &mut app.state);
        }
        PanelEnum::Search => {
            let mut panel = SearchPanel;
            panel.handle_event(&Event::Key(key), &mut app.state);
        }
        PanelEnum::Scope => {
            let mut panel = ScopePanel;
            panel.handle_event(&Event::Key(key), &mut app.state);
        }
        PanelEnum::Commits => {
            let mut panel = CommitPanel::new();
            panel.handle_event(&Event::Key(key), &mut app.state);
        }
        PanelEnum::Diff => {
            let mut panel = DiffPanel;
            panel.handle_event(&Event::Key(key), &mut app.state);
        }
    }

    Ok(EventOutcome::Continue)
}

fn move_down(app: &mut App) {
    match app.state.ui.focus {
        PanelEnum::Branches => {
            if app.state.branch.branch_index + 1 < app.state.branch.branch_tree.len() {
                app.state.branch.branch_index += 1;
            } else {
                app.state.branch.branch_index = 0;
            }
        }
        PanelEnum::Commits if search::visible_count(&app.state) > 0 => {
            app.state.commit.selected_index =
                (app.state.commit.selected_index + 1) % search::visible_count(&app.state);
        }
        PanelEnum::Diff if !app.state.diff.file_entries.is_empty() => {
            app.state.diff.selected_file_index =
                (app.state.diff.selected_file_index + 1) % app.state.diff.file_entries.len();
        }
        _ => {}
    }
}

fn move_up(app: &mut App) {
    match app.state.ui.focus {
        PanelEnum::Branches => {
            if app.state.branch.branch_index > 0 {
                app.state.branch.branch_index -= 1;
            } else if !app.state.branch.branch_tree.is_empty() {
                app.state.branch.branch_index = app.state.branch.branch_tree.len() - 1;
            }
        }
        PanelEnum::Commits if search::visible_count(&app.state) > 0 => {
            if app.state.commit.selected_index > 0 {
                app.state.commit.selected_index -= 1;
            } else {
                app.state.commit.selected_index = search::visible_count(&app.state) - 1;
            }
        }
        PanelEnum::Diff if !app.state.diff.file_entries.is_empty() => {
            if app.state.diff.selected_file_index > 0 {
                app.state.diff.selected_file_index -= 1;
            } else {
                app.state.diff.selected_file_index = app.state.diff.file_entries.len() - 1;
            }
        }
        _ => {}
    }
}

fn cycle_scope(app: &mut App) {
    app.state.branch.branch_scope = app.state.branch.branch_scope.next();
    app.state.branch.branch_index = 0;
    app.state.branch.expanded_nodes.clear();
    app.state.search.search_query.clear();
    app.state.search.cursor_pos = 0;
    app.state.branch.selected_branch = None;
    app.request_branches();
    app.request_commits(None);
}

fn handle_paste(app: &mut App) {
    if let Some(text) = clipboard::get_clipboard_text() {
        app.state.search.search_query = text;
        app.state.search.cursor_pos = app.state.search.search_query.len();
        search::apply_search_filter(&mut app.state);
        app.state.ui.focus = PanelEnum::Search;
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
            app.state.ui.dragging = None;
            app.state.ui.scrollbar_drag = None;
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
    let Some((tw, th)) = app.state.ui.last_size else {
        return;
    };
    let full = Rect::new(0, 0, tw, th);
    let areas = ui::layout::compute_areas(
        full,
        app.state.ui.branch_width_pct,
        app.state.ui.diff_height_pct,
    );
    let help_h = ui::layout::HELP_BAR_HEIGHT.min(th);
    let branch_visible_area = Rect::new(
        areas.branch.x,
        areas.branch.y,
        areas.branch.width,
        areas.branch.height.saturating_sub(help_h),
    );
    let click_pos = (col, row);

    if let Some(_new_pos) = app
        .state
        .ui
        .branch_scrollbar
        .click_to_index(branch_visible_area, click_pos)
    {
        app.state.ui.dragging = None;
        app.state.ui.focus = PanelEnum::Branches;
        app.state.ui.scrollbar_drag = Some(PanelEnum::Branches);
        return;
    }
    if let Some(_new_pos) = app
        .state
        .ui
        .table_scrollbar
        .click_to_index(areas.table, click_pos)
    {
        app.state.ui.dragging = None;
        app.state.ui.focus = PanelEnum::Commits;
        app.state.ui.scrollbar_drag = Some(PanelEnum::Commits);
        return;
    }
    if let Some(new_pos) = app
        .state
        .ui
        .diff_scrollbar
        .click_to_index(areas.diff, click_pos)
    {
        app.state.ui.dragging = None;
        app.state.ui.focus = PanelEnum::Diff;
        app.state.ui.scrollbar_drag = Some(PanelEnum::Diff);
        app.state.diff.diff_scroll = new_pos;
        return;
    }

    if ui::layout::rect_contains_interior(&branch_visible_area, click_pos) {
        app.state.ui.focus = PanelEnum::Branches;
        let rel_row = (row
            .saturating_sub(areas.branch.y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.state.branch.branch_list_offset;
        if actual_index < app.state.branch.branch_tree.len() {
            app.state.branch.branch_index = actual_index;
            let action = app.state.branch.branch_tree.get(actual_index).map(|item| {
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
                    app.state.ui.focus = PanelEnum::Commits;
                } else if let Some((key, new_state)) = toggle {
                    app.state.branch.expanded_nodes.insert(key, new_state);
                    branches::rebuild_branch_tree(&mut app.state);
                }
            }
        }
    } else if ui::layout::rect_contains(&areas.scope, click_pos) {
        app.state.ui.focus = PanelEnum::Scope;
        cycle_scope(app);
    } else if ui::layout::rect_contains(&areas.search, click_pos) {
        app.state.ui.focus = PanelEnum::Search;
    } else if ui::layout::rect_contains_interior(&areas.table, click_pos) {
        app.state.ui.focus = PanelEnum::Commits;
        let rel_row = (row
            .saturating_sub(areas.table.y)
            .saturating_sub(ui::layout::TABLE_OVERHEAD.saturating_sub(ui::layout::BORDER_OVERHEAD)))
            as usize;
        let filtered_idx = rel_row + app.state.commit.table_state.offset();
        if let Some(vis_idx) = search::filtered_to_visible(&app.state, filtered_idx) {
            app.state.commit.selected_index = vis_idx;
        }
    } else if ui::layout::rect_contains(&areas.diff, click_pos) {
        app.state.ui.focus = PanelEnum::Diff;
        let rel_row = (row
            .saturating_sub(areas.diff.y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let meta_offset = if let Some(ref info) = app.state.diff.commit_info {
            ui::diff_panel::build_metadata_lines(info, &app.state.theme).len()
        } else {
            0
        };
        if rel_row > meta_offset && rel_row <= meta_offset + app.state.diff.file_entries.len() + 2 {
            let file_idx = rel_row - meta_offset - 1;
            if file_idx < app.state.diff.file_entries.len() {
                app.state.diff.selected_file_index = file_idx;
                let offset = ui::diff_panel::diff_line_offset(
                    app.state.diff.commit_info.as_ref(),
                    &app.state.diff.file_entries,
                    &app.state.theme,
                );
                if let Some(entry) = app.state.diff.file_entries.get(file_idx) {
                    app.state.diff.diff_scroll = entry.diff_line + offset;
                }
            }
        }
    }
}

fn check_resize_start(app: &mut App, col: u16, row: u16) {
    let Some((tw, th)) = app.state.ui.last_size else {
        return;
    };
    let full = Rect::new(0, 0, tw, th);
    let areas = ui::layout::compute_areas(
        full,
        app.state.ui.branch_width_pct,
        app.state.ui.diff_height_pct,
    );

    if ui::layout::is_on_vertical_border(col, row, areas.branch) {
        app.state.ui.dragging = Some(ui::layout::DragDirection::Vertical);
        return;
    }

    if ui::layout::is_on_horizontal_border(col, row, areas.right, areas.table) {
        app.state.ui.dragging = Some(ui::layout::DragDirection::Horizontal);
    }
}

fn handle_mouse_drag(app: &mut App, col: u16, row: u16) {
    if let Some(panel) = app.state.ui.scrollbar_drag {
        let Some((tw, th)) = app.state.ui.last_size else {
            return;
        };
        let full = Rect::new(0, 0, tw, th);
        let areas = ui::layout::compute_areas(
            full,
            app.state.ui.branch_width_pct,
            app.state.ui.diff_height_pct,
        );
        let help_h = ui::layout::HELP_BAR_HEIGHT.min(th);
        let branch_visible_area = Rect::new(
            areas.branch.x,
            areas.branch.y,
            areas.branch.width,
            areas.branch.height.saturating_sub(help_h),
        );
        match panel {
            PanelEnum::Branches => {
                let _ = app
                    .state
                    .ui
                    .branch_scrollbar
                    .click_to_index(branch_visible_area, (col, row));
            }
            PanelEnum::Commits => {
                let _ = app
                    .state
                    .ui
                    .table_scrollbar
                    .click_to_index(areas.table, (col, row));
            }
            PanelEnum::Diff => {
                if let Some(new_pos) = app
                    .state
                    .ui
                    .diff_scrollbar
                    .click_to_index(areas.diff, (col, row))
                {
                    app.state.diff.diff_scroll = new_pos;
                }
            }
            _ => {}
        }
        return;
    }

    match app.state.ui.dragging {
        Some(ui::layout::DragDirection::Vertical) => {
            if let Some((tw, _)) = app.state.ui.last_size {
                app.state.ui.branch_width_pct = ui::layout::vertical_resize_pct(col, tw);
            }
        }
        Some(ui::layout::DragDirection::Horizontal) => {
            if let Some((_, th)) = app.state.ui.last_size {
                app.state.ui.diff_height_pct = ui::layout::horizontal_resize_pct(row, th);
            }
        }
        None => {}
    }
}

fn handle_scroll_at(app: &mut App, col: u16, row: u16, direction: i32) {
    let Some((tw, th)) = app.state.ui.last_size else {
        return;
    };
    let full = Rect::new(0, 0, tw, th);
    let areas = ui::layout::compute_areas(
        full,
        app.state.ui.branch_width_pct,
        app.state.ui.diff_height_pct,
    );
    let pos = (col, row);

    if ui::layout::rect_contains(&areas.branch, pos) {
        if direction > 0 {
            if app.state.branch.branch_index + 1 < app.state.branch.branch_tree.len() {
                app.state.branch.branch_index += 1;
            } else {
                app.state.branch.branch_index = 0;
            }
        } else {
            if app.state.branch.branch_index > 0 {
                app.state.branch.branch_index -= 1;
            } else if !app.state.branch.branch_tree.is_empty() {
                app.state.branch.branch_index = app.state.branch.branch_tree.len() - 1;
            }
        }
    } else if ui::layout::rect_contains(&areas.table, pos) {
        if search::visible_count(&app.state) > 0 {
            if direction > 0 {
                app.state.commit.selected_index =
                    (app.state.commit.selected_index + 1) % search::visible_count(&app.state);
            } else {
                if app.state.commit.selected_index > 0 {
                    app.state.commit.selected_index -= 1;
                } else {
                    app.state.commit.selected_index = search::visible_count(&app.state) - 1;
                }
            }
        }
    } else if ui::layout::rect_contains(&areas.diff, pos) {
        if direction > 0 {
            app.state.diff.diff_scroll += 1;
        } else {
            app.state.diff.diff_scroll = app.state.diff.diff_scroll.saturating_sub(1);
        }
    }
}

#[allow(dead_code)]
fn handle_scroll_down(app: &mut App) {
    match app.state.ui.focus {
        PanelEnum::Branches => {
            if app.state.branch.branch_index + 1 < app.state.branch.branch_tree.len() {
                app.state.branch.branch_index += 1;
            } else {
                app.state.branch.branch_index = 0;
            }
        }
        PanelEnum::Commits if search::visible_count(&app.state) > 0 => {
            app.state.commit.selected_index =
                (app.state.commit.selected_index + 1) % search::visible_count(&app.state);
        }
        PanelEnum::Diff => {
            app.state.diff.diff_scroll += 1;
        }
        _ => {}
    }
}

#[allow(dead_code)]
fn handle_scroll_up(app: &mut App) {
    match app.state.ui.focus {
        PanelEnum::Branches => {
            if app.state.branch.branch_index > 0 {
                app.state.branch.branch_index -= 1;
            } else if !app.state.branch.branch_tree.is_empty() {
                app.state.branch.branch_index = app.state.branch.branch_tree.len() - 1;
            }
        }
        PanelEnum::Commits if search::visible_count(&app.state) > 0 => {
            if app.state.commit.selected_index > 0 {
                app.state.commit.selected_index -= 1;
            } else {
                app.state.commit.selected_index = search::visible_count(&app.state) - 1;
            }
        }
        PanelEnum::Diff => {
            app.state.diff.diff_scroll = app.state.diff.diff_scroll.saturating_sub(1);
        }
        _ => {}
    }
}
