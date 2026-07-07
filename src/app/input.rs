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
            let commands = handle_mouse(app, mouse);
            for cmd in commands {
                execute_command(app, cmd);
            }
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
    // --- Always-global keys ---
    match key.code {
        // Quit
        KeyCode::Char('q') => return vec![Command::Quit],
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return vec![Command::Quit]
        }
        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            suspend(app);
            return Vec::new();
        }
        // Focus cycle
        KeyCode::Tab => {
            return vec![if key.modifiers.contains(KeyModifiers::SHIFT) {
                Command::FocusPrev
            } else {
                Command::FocusNext
            }];
        }
        KeyCode::BackTab => return vec![Command::FocusPrev],
        // Ctrl+shortcuts
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return vec![Command::CycleScope];
        }
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
        _ => {}
    }

    // --- Panel dispatch — focused panel gets first chance at the key ---
    let commands = match app.state.ui.focus {
        PanelEnum::Branches => BranchPanel.handle_event(&key, &mut app.state.branch),
        PanelEnum::Search => SearchPanel.handle_event(&key, &mut app.state.search),
        PanelEnum::Scope => ScopePanel.handle_event(&key, &mut app.state.branch.branch_scope),
        PanelEnum::Commits => CommitPanel::new().handle_event(&key, &mut app.state.commit),
        PanelEnum::Diff => DiffPanel.handle_event(&key, &mut app.state.diff),
    };
    if !commands.is_empty() {
        return commands;
    }

    // --- Fallback: single-character shortcuts (only if panel ignored the key) ---
    match key.code {
        KeyCode::Char('h') => vec![Command::FocusPrev],
        KeyCode::Char('l') => vec![Command::FocusNext],
        KeyCode::Char('j') => vec![Command::MoveDown],
        KeyCode::Char('k') => vec![Command::MoveUp],
        KeyCode::Char('y') => vec![Command::CopyHashShort],
        KeyCode::Char('Y') => vec![Command::CopyHashFull],
        KeyCode::Char('g')
            if key.modifiers.is_empty() && app.state.ui.focus == PanelEnum::Commits =>
        {
            vec![Command::ToggleGraph]
        }
        _ => Vec::new(),
    }
}

// --- Command execution ---

pub(crate) fn execute_command(app: &mut App, cmd: Command) {
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
        CopyHashShort => copy_hash(app, true),
        CopyHashFull => copy_hash(app, false),
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
        SelectNextFile => select_next_file(app),
        SelectPrevFile => select_prev_file(app),
        JumpToTop => match app.state.ui.focus {
            PanelEnum::Branches => {
                app.state.branch.branch_index = 0;
            }
            PanelEnum::Commits => {
                if search::visible_count(&app.state) > 0 {
                    app.state.commit.selected_index = 0;
                    search::clamp_selection(&mut app.state);
                }
            }
            PanelEnum::Diff => {
                app.state.diff.diff_scroll = 0;
            }
            _ => {}
        },
        JumpToBottom => match app.state.ui.focus {
            PanelEnum::Branches => {
                app.state.branch.branch_index =
                    app.state.branch.branch_tree.len().saturating_sub(1);
            }
            PanelEnum::Commits => {
                let count = search::visible_count(&app.state);
                if count > 0 {
                    app.state.commit.selected_index = count.saturating_sub(1);
                    search::clamp_selection(&mut app.state);
                }
            }
            PanelEnum::Diff => {
                app.state.diff.diff_scroll = usize::MAX;
            }
            _ => {}
        },
        ShowCommitDiff => {
            app.state.ui.focus = PanelEnum::Diff;
        }
        InitiateDragVertical => {
            app.state.ui.dragging = Some(ui::layout::DragDirection::Vertical);
        }
        InitiateDragHorizontal => {
            app.state.ui.dragging = Some(ui::layout::DragDirection::Horizontal);
        }
        EndDrag => {
            app.state.ui.dragging = None;
        }
        InitiateScrollbarDrag(panel) => {
            app.state.ui.dragging = None;
            app.state.ui.focus = panel;
            app.state.ui.scrollbar_drag = Some(panel);
        }
        EndScrollbarDrag => {
            app.state.ui.scrollbar_drag = None;
        }
        SetBranchWidthPct(pct) => {
            app.state.ui.branch_width_pct = pct;
        }
        SetDiffHeightPct(pct) => {
            app.state.ui.diff_height_pct = pct;
        }
        SelectCommitIndex(idx) => {
            app.state.commit.selected_index = idx;
            search::clamp_selection(&mut app.state);
        }
        MouseClickBranch {
            index,
            full_path,
            is_branch,
            is_expandable,
            is_expanded,
            key,
        } => {
            app.state.branch.branch_index = index;
            if is_branch {
                app.request_commits(Some(full_path));
                app.state.ui.focus = PanelEnum::Commits;
            } else if is_expandable {
                app.state.branch.expanded_nodes.insert(key, !is_expanded);
                branches::rebuild_branch_tree(&mut app.state);
            }
        }
        ScrollToAbsolute(pos) => {
            app.state.diff.diff_scroll = pos;
        }
        Quit => {} // handled outside
    }
}

fn copy_hash(app: &mut App, short: bool) {
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
            let hash = if short && c.hash.len() > ui::commit_table::SHORT_HASH_LEN {
                &c.hash[..ui::commit_table::SHORT_HASH_LEN]
            } else {
                &c.hash
            };
            let _ = clipboard::copy_to_clipboard(hash);
        }
    }
}

fn select_next_file(app: &mut App) {
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

fn select_prev_file(app: &mut App) {
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
            let page = app.state.diff.scrollbar.viewport_length().max(1);
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
            let page = app.state.diff.scrollbar.viewport_length().max(1);
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

// --- Mouse handling (produces Commands, does not mutate state) ---

fn handle_mouse(app: &App, mouse: event::MouseEvent) -> Vec<Command> {
    use crossterm::event::MouseButton;

    let col = mouse.column;
    let row = mouse.row;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let mut commands = mouse_click(app, col, row);
            commands.extend(check_resize_start(app, col, row));
            commands
        }
        MouseEventKind::Drag(MouseButton::Left) => mouse_drag(app, col, row),
        MouseEventKind::Up(MouseButton::Left) => {
            vec![Command::EndDrag, Command::EndScrollbarDrag]
        }
        MouseEventKind::ScrollDown => scroll_at(app, col, row, 1),
        MouseEventKind::ScrollUp => scroll_at(app, col, row, -1),
        _ => Vec::new(),
    }
}

fn compute_mouse_areas(app: &App) -> (Rect, ui::layout::LayoutAreas, Rect) {
    let (tw, th) = app.state.ui.last_size.unwrap_or((80, 24));
    let full = Rect::new(0, 0, tw, th);
    let areas = ui::layout::compute_areas(
        full,
        app.state.ui.branch_width_pct,
        app.state.ui.diff_height_pct,
    );
    let help_h = ui::layout::HELP_BAR_HEIGHT.min(th);
    let branch_visible = Rect::new(
        areas.branch.x,
        areas.branch.y,
        areas.branch.width,
        areas.branch.height.saturating_sub(help_h),
    );
    (full, areas, branch_visible)
}

pub(crate) fn mouse_click(app: &App, col: u16, row: u16) -> Vec<Command> {
    let (_full, areas, branch_visible) = compute_mouse_areas(app);
    let click_pos = (col, row);

    // Scrollbar clicks
    if app
        .state
        .branch
        .scrollbar
        .click_to_index(branch_visible, click_pos)
        .or_else(|| {
            app.state
                .branch
                .scrollbar
                .is_click_in_scrollbar_area(branch_visible, click_pos)
                .then_some(0)
        })
        .is_some()
    {
        return vec![
            Command::SetFocus(PanelEnum::Branches),
            Command::InitiateScrollbarDrag(PanelEnum::Branches),
        ];
    }
    if app
        .state
        .commit
        .scrollbar
        .click_to_index(areas.table, click_pos)
        .or_else(|| {
            app.state
                .commit
                .scrollbar
                .is_click_in_scrollbar_area(areas.table, click_pos)
                .then_some(0)
        })
        .is_some()
    {
        return vec![
            Command::SetFocus(PanelEnum::Commits),
            Command::InitiateScrollbarDrag(PanelEnum::Commits),
        ];
    }
    if let Some(pos) = app
        .state
        .diff
        .scrollbar
        .click_to_index(areas.diff, click_pos)
        .or_else(|| {
            app.state
                .diff
                .scrollbar
                .is_click_in_scrollbar_area(areas.diff, click_pos)
                .then_some(app.state.diff.diff_scroll)
        })
    {
        return vec![
            Command::SetFocus(PanelEnum::Diff),
            Command::InitiateScrollbarDrag(PanelEnum::Diff),
            Command::ScrollToAbsolute(pos),
        ];
    }

    // Panel content clicks
    if ui::layout::rect_contains_interior(&branch_visible, click_pos) {
        let rel_row = (row
            .saturating_sub(areas.branch.y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.state.branch.branch_list_offset;
        if actual_index < app.state.branch.branch_tree.len() {
            let item = &app.state.branch.branch_tree[actual_index];
            return vec![Command::MouseClickBranch {
                index: actual_index,
                full_path: item.full_path.clone(),
                is_branch: item.is_branch,
                is_expandable: item.expandable,
                is_expanded: item.expanded,
                key: item.key.clone(),
            }];
        }
        return vec![Command::SetFocus(PanelEnum::Branches)];
    }

    if ui::layout::rect_contains(&branch_visible, click_pos) {
        return vec![Command::SetFocus(PanelEnum::Branches)];
    }

    if ui::layout::rect_contains(&areas.scope, click_pos) {
        return vec![Command::SetFocus(PanelEnum::Scope), Command::CycleScope];
    }

    if ui::layout::rect_contains(&areas.search, click_pos) {
        return vec![Command::SetFocus(PanelEnum::Search)];
    }

    if ui::layout::rect_contains_interior(&areas.table, click_pos) {
        let rel_row = (row
            .saturating_sub(areas.table.y)
            .saturating_sub(ui::layout::TABLE_OVERHEAD.saturating_sub(ui::layout::BORDER_OVERHEAD)))
            as usize;
        let filtered_idx = rel_row + app.state.commit.table_state.offset();
        if let Some(vis_idx) = search::filtered_to_visible(&app.state, filtered_idx) {
            return vec![
                Command::SetFocus(PanelEnum::Commits),
                Command::SelectCommitIndex(vis_idx),
            ];
        }
        return vec![Command::SetFocus(PanelEnum::Commits)];
    }

    if ui::layout::rect_contains(&areas.table, click_pos) {
        return vec![Command::SetFocus(PanelEnum::Commits)];
    }

    if ui::layout::rect_contains(&areas.diff, click_pos) {
        let meta_offset = if let Some(ref info) = app.state.diff.commit_info {
            ui::diff_panel::count_metadata_lines(info)
        } else {
            0
        };
        let rel_row = (row
            .saturating_sub(areas.diff.y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        if rel_row > meta_offset && rel_row <= meta_offset + app.state.diff.file_entries.len() + 2 {
            let file_idx = rel_row - meta_offset - 1;
            if file_idx < app.state.diff.file_entries.len() {
                let offset = ui::diff_panel::diff_line_offset(
                    app.state.diff.commit_info.as_ref(),
                    &app.state.diff.file_entries,
                );
                if let Some(entry) = app.state.diff.file_entries.get(file_idx) {
                    return vec![
                        Command::SetFocus(PanelEnum::Diff),
                        Command::JumpToDiffFile(file_idx),
                        Command::ScrollToAbsolute(entry.diff_line + offset),
                    ];
                }
            }
        }
        return vec![Command::SetFocus(PanelEnum::Diff)];
    }

    Vec::new()
}

fn check_resize_start(app: &App, col: u16, row: u16) -> Vec<Command> {
    let (_full, areas, _branch_visible) = compute_mouse_areas(app);

    if ui::layout::is_on_vertical_border(col, row, areas.branch) {
        return vec![Command::InitiateDragVertical];
    }
    if ui::layout::is_on_horizontal_border(col, row, areas.right, areas.table) {
        return vec![Command::InitiateDragHorizontal];
    }
    Vec::new()
}

fn mouse_drag(app: &App, col: u16, row: u16) -> Vec<Command> {
    let mut commands = Vec::new();

    if let Some(panel) = app.state.ui.scrollbar_drag {
        let (_full, areas, branch_visible) = compute_mouse_areas(app);
        match panel {
            PanelEnum::Branches => {
                // scrollbar drag on branch: position is tracked via state alone
                let _ = app
                    .state
                    .branch
                    .scrollbar
                    .click_to_index(branch_visible, (col, row));
            }
            PanelEnum::Commits => {
                let _ = app
                    .state
                    .commit
                    .scrollbar
                    .click_to_index(areas.table, (col, row));
            }
            PanelEnum::Diff => {
                if let Some(new_pos) = app
                    .state
                    .diff
                    .scrollbar
                    .click_to_index(areas.diff, (col, row))
                {
                    commands.push(Command::ScrollToAbsolute(new_pos));
                }
            }
            _ => {}
        }
        return commands;
    }

    match app.state.ui.dragging {
        Some(ui::layout::DragDirection::Vertical) => {
            if let Some((tw, _)) = app.state.ui.last_size {
                commands.push(Command::SetBranchWidthPct(ui::layout::vertical_resize_pct(
                    col, tw,
                )));
            }
        }
        Some(ui::layout::DragDirection::Horizontal) => {
            if let Some((_, th)) = app.state.ui.last_size {
                commands.push(Command::SetDiffHeightPct(
                    ui::layout::horizontal_resize_pct(row, th),
                ));
            }
        }
        None => {}
    }

    commands
}

fn scroll_at(app: &App, col: u16, row: u16, direction: i32) -> Vec<Command> {
    let (_full, areas, _branch_visible) = compute_mouse_areas(app);
    let pos = (col, row);

    if ui::layout::rect_contains(&areas.branch, pos) {
        if direction > 0 {
            vec![Command::MoveDown]
        } else {
            vec![Command::MoveUp]
        }
    } else if ui::layout::rect_contains(&areas.table, pos) {
        if direction > 0 {
            vec![Command::MoveDown]
        } else {
            vec![Command::MoveUp]
        }
    } else if ui::layout::rect_contains(&areas.diff, pos) {
        vec![Command::ScrollDiff(direction)]
    } else {
        Vec::new()
    }
}
