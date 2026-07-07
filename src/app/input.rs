use crate::app::commands::Command;
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
            let commands = handle_key(app, key);
            let should_quit = commands.iter().any(|c| matches!(c, Command::Quit));
            for cmd in commands {
                execute_command(app, cmd);
            }
            if should_quit {
                return Ok(EventOutcome::Quit);
            }
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
        }
        Event::Resize(w, h) => {
            app.state.ui.dirty = true;
            app.state.ui.last_size = Some((w, h));
        }
        _ => {}
    }

    Ok(EventOutcome::Continue)
}

pub(crate) fn handle_key(app: &mut App, key: KeyEvent) -> Vec<Command> {
    // --- Global quit keys ---
    match key.code {
        KeyCode::Char('q') => return vec![Command::Quit],
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return vec![Command::Quit]
        }
        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            suspend(app);
            return Vec::new();
        }
        _ => {}
    }

    // --- Global keys — work regardless of focus ---
    match key.code {
        KeyCode::Tab => {
            return vec![if key.modifiers.contains(KeyModifiers::SHIFT) {
                Command::FocusPrev
            } else {
                Command::FocusNext
            }];
        }
        KeyCode::BackTab => return vec![Command::FocusPrev],
        KeyCode::Char('l') => return vec![Command::FocusNext],
        KeyCode::Char('h') => return vec![Command::FocusPrev],
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return vec![Command::CycleScope];
        }
        KeyCode::Char('y') => return vec![Command::CopyHashShort],
        KeyCode::Char('Y') => return vec![Command::CopyHashFull],
        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(text) = clipboard::get_clipboard_text() {
                return vec![Command::PasteSearch(text)];
            }
            return Vec::new();
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.state.ui.focus == PanelEnum::Search {
                return vec![Command::SetSearch(app.state.search.search_query.clone(), 0)];
            } else {
                return vec![
                    Command::SetFocus(PanelEnum::Search),
                    Command::SetSearch(app.state.search.search_query.clone(), 0),
                ];
            }
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let len = app.state.search.search_query.len();
            if app.state.ui.focus == PanelEnum::Search {
                return vec![Command::SetSearch(
                    app.state.search.search_query.clone(),
                    len,
                )];
            } else {
                return vec![
                    Command::SetFocus(PanelEnum::Search),
                    Command::SetSearch(app.state.search.search_query.clone(), len),
                ];
            }
        }
        KeyCode::Esc => {
            if app.state.ui.focus == PanelEnum::Search {
                return vec![Command::ClearSearch];
            }
            return Vec::new();
        }
        KeyCode::Char('g')
            if key.modifiers.is_empty() && app.state.ui.focus == PanelEnum::Commits =>
        {
            return vec![Command::ToggleGraph];
        }
        _ => {}
    }

    // --- Vim navigation keys (global alternative for up/down) ---
    match key.code {
        KeyCode::Char('j') => return vec![Command::MoveDown],
        KeyCode::Char('k') => return vec![Command::MoveUp],
        _ => {}
    }

    // --- Dispatch to the focused panel ---
    match app.state.ui.focus {
        PanelEnum::Branches => BranchPanel.handle_event(&key, &mut app.state.branch),
        PanelEnum::Search => SearchPanel.handle_event(&key, &mut app.state.search),
        PanelEnum::Scope => ScopePanel.handle_event(&key, &mut app.state.branch.branch_scope),
        PanelEnum::Commits => CommitPanel::new().handle_event(&key, &mut app.state.commit),
        PanelEnum::Diff => DiffPanel.handle_event(&key, &mut app.state.diff),
    }
}

// --- Command execution ---

fn execute_command(app: &mut App, cmd: Command) {
    use Command::*;

    match cmd {
        SetSearch(query, cursor) => {
            app.state.search.search_query = query;
            app.state.search.cursor_pos = cursor;
            search::apply_search_filter(&mut app.state);
        }
        ClearSearch => {
            app.state.search.search_query.clear();
            app.state.search.cursor_pos = 0;
            search::apply_search_filter(&mut app.state);
        }
        MoveUp => move_up(app),
        MoveDown => move_down(app),
        PageUp => page_up(app),
        PageDown => page_down(app),
        SetFocus(panel) => {
            app.state.ui.focus = panel;
        }
        FocusNext => {
            app.state.ui.focus = app.state.ui.focus.next();
        }
        FocusPrev => {
            app.state.ui.focus = app.state.ui.focus.prev();
        }
        SelectBranch(name) => {
            app.request_commits(Some(name));
            app.state.ui.focus = PanelEnum::Commits;
        }
        ToggleBranchNode { key, expanded } => {
            app.state.branch.expanded_nodes.insert(key, expanded);
            branches::rebuild_branch_tree(&mut app.state);
        }
        CycleScope => {
            app.state.branch.branch_scope = app.state.branch.branch_scope.next();
            app.state.branch.branch_index = 0;
            app.state.branch.expanded_nodes.clear();
            app.state.search.search_query.clear();
            app.state.search.cursor_pos = 0;
            app.state.branch.selected_branch = None;
            app.request_branches();
            app.request_commits(None);
        }
        ToggleGraph => {
            app.toggle_simplified_graph();
        }
        CopyHashShort => {
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
        }
        CopyHashFull => {
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
        }
        PasteSearch(text) => {
            app.state.search.search_query = text;
            app.state.search.cursor_pos = app.state.search.search_query.len();
            search::apply_search_filter(&mut app.state);
            app.state.ui.focus = PanelEnum::Search;
        }
        ScrollDiff(delta) => {
            if delta > 0 {
                app.state.diff.diff_scroll += delta as usize;
            } else {
                app.state.diff.diff_scroll =
                    app.state.diff.diff_scroll.saturating_sub((-delta) as usize);
            }
        }
        JumpToDiffFile(index) => {
            let offset = ui::diff_panel::diff_line_offset(
                app.state.diff.commit_info.as_ref(),
                &app.state.diff.file_entries,
            );
            if let Some(entry) = app.state.diff.file_entries.get(index) {
                app.state.diff.diff_scroll = entry.diff_line + offset;
            }
        }
        SelectNextFile => {
            if !app.state.diff.file_entries.is_empty() {
                app.state.diff.selected_file_index =
                    (app.state.diff.selected_file_index + 1) % app.state.diff.file_entries.len();
                let offset = ui::diff_panel::diff_line_offset(
                    app.state.diff.commit_info.as_ref(),
                    &app.state.diff.file_entries,
                );
                if let Some(entry) = app
                    .state
                    .diff
                    .file_entries
                    .get(app.state.diff.selected_file_index)
                {
                    app.state.diff.diff_scroll = entry.diff_line + offset;
                }
            }
        }
        SelectPrevFile => {
            if !app.state.diff.file_entries.is_empty() {
                if app.state.diff.selected_file_index > 0 {
                    app.state.diff.selected_file_index -= 1;
                    let offset = ui::diff_panel::diff_line_offset(
                        app.state.diff.commit_info.as_ref(),
                        &app.state.diff.file_entries,
                    );
                    if let Some(entry) = app
                        .state
                        .diff
                        .file_entries
                        .get(app.state.diff.selected_file_index)
                    {
                        app.state.diff.diff_scroll = entry.diff_line + offset;
                    }
                } else {
                    app.state.diff.diff_scroll = 0;
                    app.state.diff.selected_file_index = 0;
                }
            }
        }
        JumpToTop => {
            app.state.diff.diff_scroll = 0;
        }
        JumpToBottom => {
            app.state.diff.diff_scroll = usize::MAX;
        }
        ShowCommitDiff => {
            app.state.ui.focus = PanelEnum::Diff;
        }
        Quit => {} // handled outside
    }
}

fn move_down(app: &mut App) {
    match app.state.ui.focus {
        PanelEnum::Branches => {
            app.state.branch.branch_index = cycle_forward(
                app.state.branch.branch_index,
                app.state.branch.branch_tree.len(),
            );
        }
        PanelEnum::Commits => {
            app.state.commit.selected_index = cycle_forward(
                app.state.commit.selected_index,
                search::visible_count(&app.state),
            );
        }
        PanelEnum::Diff => {
            app.state.diff.selected_file_index = cycle_forward(
                app.state.diff.selected_file_index,
                app.state.diff.file_entries.len(),
            );
        }
        _ => {}
    }
}

fn move_up(app: &mut App) {
    match app.state.ui.focus {
        PanelEnum::Branches => {
            app.state.branch.branch_index = cycle_backward(
                app.state.branch.branch_index,
                app.state.branch.branch_tree.len(),
            );
        }
        PanelEnum::Commits => {
            app.state.commit.selected_index = cycle_backward(
                app.state.commit.selected_index,
                search::visible_count(&app.state),
            );
        }
        PanelEnum::Diff => {
            app.state.diff.selected_file_index = cycle_backward(
                app.state.diff.selected_file_index,
                app.state.diff.file_entries.len(),
            );
        }
        _ => {}
    }
}

fn page_up(app: &mut App) {
    match app.state.ui.focus {
        PanelEnum::Branches => {
            app.state.branch.branch_index = app
                .state
                .branch
                .branch_index
                .saturating_sub(super::PAGE_SIZE);
        }
        PanelEnum::Commits if search::visible_count(&app.state) > 0 => {
            app.state.commit.selected_index = app
                .state
                .commit
                .selected_index
                .saturating_sub(super::PAGE_SIZE);
            search::clamp_selection(&mut app.state);
        }
        PanelEnum::Diff => {
            let page = app.state.ui.diff_scrollbar.viewport_length().max(1);
            app.state.diff.diff_scroll = app.state.diff.diff_scroll.saturating_sub(page);
        }
        _ => {}
    }
}

fn page_down(app: &mut App) {
    match app.state.ui.focus {
        PanelEnum::Branches => {
            app.state.branch.branch_index = (app.state.branch.branch_index + super::PAGE_SIZE)
                .min(app.state.branch.branch_tree.len().saturating_sub(1));
        }
        PanelEnum::Commits if search::visible_count(&app.state) > 0 => {
            app.state.commit.selected_index = (app.state.commit.selected_index + super::PAGE_SIZE)
                .min(search::visible_count(&app.state).saturating_sub(1));
        }
        PanelEnum::Diff => {
            let page = app.state.ui.diff_scrollbar.viewport_length().max(1);
            app.state.diff.diff_scroll = app.state.diff.diff_scroll.saturating_add(page);
        }
        _ => {}
    }
}

/// Cycle index forward with wrapping. Returns 0 if len is 0.
fn cycle_forward(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current + 1) % len
    }
}

/// Cycle index backward with wrapping. Returns 0 if len is 0.
fn cycle_backward(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if current > 0 {
        current - 1
    } else {
        len - 1
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
        app.state.branch.branch_scope = app.state.branch.branch_scope.next();
        app.state.branch.branch_index = 0;
        app.state.branch.expanded_nodes.clear();
        app.state.search.search_query.clear();
        app.state.search.cursor_pos = 0;
        app.state.branch.selected_branch = None;
        app.request_branches();
        app.request_commits(None);
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
            ui::diff_panel::count_metadata_lines(info)
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
        app.state.branch.branch_index = if direction > 0 {
            cycle_forward(
                app.state.branch.branch_index,
                app.state.branch.branch_tree.len(),
            )
        } else {
            cycle_backward(
                app.state.branch.branch_index,
                app.state.branch.branch_tree.len(),
            )
        };
    } else if ui::layout::rect_contains(&areas.table, pos) {
        let count = search::visible_count(&app.state);
        if count > 0 {
            app.state.commit.selected_index = if direction > 0 {
                cycle_forward(app.state.commit.selected_index, count)
            } else {
                cycle_backward(app.state.commit.selected_index, count)
            };
        }
    } else if ui::layout::rect_contains(&areas.diff, pos) {
        if direction > 0 {
            app.state.diff.diff_scroll += 1;
        } else {
            app.state.diff.diff_scroll = app.state.diff.diff_scroll.saturating_sub(1);
        }
    }
}
