use crate::app::commands::{Command, Effect};
use crate::clipboard;
use crate::error::AppError;
use crate::ui;
use crate::ui::branch_panel::BranchPanel;
use crate::ui::commit_table::CommitPanel;
use crate::ui::diff_panel::DiffPanel;
use crate::ui::panel::Panel;
use crate::ui::scope_panel::ScopePanel;
use crate::ui::search_panel::SearchPanel;
use crate::view::Panel as PanelEnum;

use crossterm::cursor::Hide;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::Rect;

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

// --- Command dispatching ---
//
// Commands are routed to state handlers which return `Effect` values.
// Cross-state commands (navigation, scope cycling, branch selection) are
// orchestrated here since they need access to multiple state fields.
// Effects are then processed by the app-level dispatcher.

pub(crate) fn execute_command(app: &mut App, cmd: Command) {
    let effects = dispatch(&mut app.state, &cmd);
    process_effects(effects, app);
}

fn dispatch(state: &mut super::state::AppState, cmd: &Command) -> Vec<Effect> {
    use crate::view::Panel;

    match cmd {
        // Search commands → SearchState (PasteSearch also sets focus)
        Command::SetSearch(..) | Command::ClearSearch => state.search.handle_command(cmd),
        Command::PasteSearch(_) => {
            let effects = state.search.handle_command(cmd);
            state.ui.focus = crate::view::Panel::Search;
            effects
        }
        // UI commands → UiState
        Command::SetFocus(_)
        | Command::FocusNext
        | Command::FocusPrev
        | Command::InitiateDragVertical
        | Command::InitiateDragHorizontal
        | Command::EndDrag
        | Command::InitiateScrollbarDrag(_)
        | Command::EndScrollbarDrag
        | Command::SetBranchWidthPct(_)
        | Command::SetDiffHeightPct(_) => state.ui.handle_command(cmd),
        // Diff panel commands → DiffState
        Command::ScrollDiff(_)
        | Command::JumpToDiffFile(_)
        | Command::SelectNextFile
        | Command::SelectPrevFile
        | Command::ScrollToAbsolute(_) => state.diff.handle_command(cmd),
        // Commit table commands → CommitTableState (ShowCommitDiff also sets focus)
        Command::CopyHashShort | Command::CopyHashFull | Command::SelectCommitIndex(_) => {
            state.commit.handle_command(cmd)
        }
        Command::ShowCommitDiff => {
            let effects = state.commit.handle_command(cmd);
            state.ui.focus = crate::view::Panel::Diff;
            effects
        }
        // Branch panel commands → BranchState
        Command::ToggleBranchNode { .. } => state.branch.handle_command(cmd),
        // Focus-dependent navigation
        Command::MoveUp
        | Command::MoveDown
        | Command::PageUp
        | Command::PageDown
        | Command::JumpToTop
        | Command::JumpToBottom => match state.ui.focus {
            Panel::Branches => state.branch.handle_command(cmd),
            Panel::Diff => state.diff.handle_command(cmd),
            Panel::Commits => dispatch_commit_navigation(state, cmd),
            _ => vec![],
        },
        // Multi-state orchestration
        Command::CycleScope => {
            state.branch.branch_scope = state.branch.branch_scope.next();
            state.branch.branch_index = 0;
            state.branch.expanded_nodes.clear();
            state.search.search_query.clear();
            state.search.cursor_pos = 0;
            state.branch.selected_branch = None;
            vec![
                Effect::RequestBranches,
                Effect::RequestCommits(None),
                Effect::ApplySearchFilter,
                Effect::SetDirty,
            ]
        }
        Command::SelectBranch(name) => {
            if state.branch.selected_branch.as_deref() == Some(name.as_str()) {
                state.ui.focus = Panel::Commits;
                return vec![Effect::SetDirty];
            }
            state.branch.selected_branch = Some(name.clone());
            state.ui.focus = Panel::Commits;
            vec![Effect::RequestCommits(Some(name.clone())), Effect::SetDirty]
        }
        Command::ToggleGraph => vec![Effect::ToggleGraph, Effect::SetDirty],
        Command::MouseClickBranch {
            index,
            full_path,
            is_branch,
            is_expandable,
            is_expanded,
            key,
        } => {
            state.branch.branch_index = *index;
            if *is_branch {
                if state.branch.selected_branch.as_deref() == Some(full_path.as_str()) {
                    return vec![Effect::SetDirty];
                }
                state.branch.selected_branch = Some(full_path.clone());
                vec![
                    Effect::RequestCommits(Some(full_path.clone())),
                    Effect::SetDirty,
                ]
            } else if *is_expandable {
                state
                    .branch
                    .expanded_nodes
                    .insert(key.clone(), !is_expanded);
                vec![Effect::RebuildBranchTree, Effect::SetDirty]
            } else {
                vec![Effect::SetDirty]
            }
        }
        Command::Quit => vec![],
    }
}

/// Navigation within the commit table needs `visible_count` from the search state,
/// so it is orchestrated here rather than inside CommitTableState::handle_command.
fn dispatch_commit_navigation(state: &mut super::state::AppState, cmd: &Command) -> Vec<Effect> {
    let visible = super::search::visible_count(state);

    match cmd {
        Command::MoveUp => {
            state.commit.selected_index =
                commit_cycle_backward(state.commit.selected_index, visible);
        }
        Command::MoveDown => {
            state.commit.selected_index =
                commit_cycle_forward(state.commit.selected_index, visible);
        }
        Command::PageUp if visible > 0 => {
            state.commit.selected_index =
                state.commit.selected_index.saturating_sub(super::PAGE_SIZE);
            super::search::clamp_selection(state);
        }
        Command::PageDown if visible > 0 => {
            state.commit.selected_index =
                (state.commit.selected_index + super::PAGE_SIZE).min(visible.saturating_sub(1));
            super::search::clamp_selection(state);
        }
        Command::JumpToTop if visible > 0 => {
            state.commit.selected_index = 0;
            super::search::clamp_selection(state);
        }
        Command::JumpToBottom if visible > 0 => {
            state.commit.selected_index = visible.saturating_sub(1);
            super::search::clamp_selection(state);
        }
        _ => {}
    }
    vec![Effect::SetDirty]
}

fn commit_cycle_forward(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (current + 1) % len
    }
}

fn commit_cycle_backward(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if current > 0 {
        current - 1
    } else {
        len - 1
    }
}

/// Process side effects returned by state command handlers.
fn process_effects(effects: Vec<Effect>, app: &mut App) {
    use crate::clipboard;
    use crate::ui::commit_table::SHORT_HASH_LEN;

    for effect in effects {
        match effect {
            Effect::RequestBranches => app.request_branches(),
            Effect::RequestCommits(branch) => app.request_commits(branch),
            Effect::ToggleGraph => app.toggle_simplified_graph(),
            Effect::CopySelectedHash { short } => {
                if super::search::visible_count(&app.state) == 0 {
                    continue;
                }
                let ci =
                    super::search::visible_to_filtered(&app.state, app.state.commit.selected_index);
                let hash_opt = app
                    .state
                    .commit
                    .filtered_commits
                    .as_deref()
                    .unwrap_or(&app.state.commit.all_commits)
                    .get(ci)
                    .map(|c| {
                        if short && c.hash.len() > SHORT_HASH_LEN {
                            &c.hash[..SHORT_HASH_LEN]
                        } else {
                            c.hash.as_str()
                        }
                    });
                if let Some(hash) = hash_opt {
                    let _ = clipboard::copy_to_clipboard(hash);
                }
            }
            Effect::RebuildBranchTree => super::branches::rebuild_branch_tree(&mut app.state),
            Effect::ApplySearchFilter => {
                super::search::apply_search_filter(&mut app.state);
                super::search::clamp_selection(&mut app.state);
            }
            Effect::SetDirty => app.state.ui.dirty = true,
        }
    }
}

fn suspend(app: &mut App) {
    let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    disable_raw_mode().ok();

    #[cfg(unix)]
    unsafe {
        libc::kill(libc::getpid(), libc::SIGTSTP);
    }

    enable_raw_mode().ok();
    let _ = execute!(
        std::io::stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        Hide
    );

    app.state.ui.needs_terminal_reset = true;
    app.state.ui.dirty = true;
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
            return vec![
                Command::SetFocus(PanelEnum::Branches),
                Command::MouseClickBranch {
                    index: actual_index,
                    full_path: item.full_path.clone(),
                    is_branch: item.is_branch,
                    is_expandable: item.expandable,
                    is_expanded: item.expanded,
                    key: item.key.clone(),
                },
            ];
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

    if ui::layout::rect_contains(&areas.branch, pos) || ui::layout::rect_contains(&areas.table, pos)
    {
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
