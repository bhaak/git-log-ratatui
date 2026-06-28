use std::collections::BTreeMap;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::TableState,
    Frame,
};

use crate::clipboard;
use crate::models::*;
use crate::workers::{BranchWorker, BranchCommand, BranchResult, CommitWorker, CommitCommand, CommitResult, DiffWorker, DiffCommand, DiffResult};
use crate::tree;
use crate::ui;

const DEFAULT_BRANCH_PCT: u16 = 20;
const MIN_BRANCH_PCT: u16 = 10;
const MAX_BRANCH_PCT: u16 = 40;

const DEFAULT_DIFF_PCT: u16 = 35;
const MIN_DIFF_PCT: u16 = 10;
const MAX_DIFF_PCT: u16 = 65;

pub struct App {
    repo_path: String,
    branch_worker: BranchWorker,
    commit_worker: CommitWorker,
    diff_worker: DiffWorker,

    all_branches: Vec<String>,
    branch_tree: Vec<TreeItem>,
    expanded_nodes: BTreeMap<String, bool>,
    branch_index: usize,
    branch_scope: BranchScope,
    selected_branch: Option<String>,

    search_query: String,
    cursor_pos: usize,

    all_commits: Vec<Commit>,
    filtered_commits: Vec<Commit>,
    selected_index: usize,
    /// Maps visible row (skipping graph_only) to filtered_commits index.
    visible_to_commit: Vec<usize>,

    commit_info: Option<CommitInfo>,
    diff_lines: Vec<String>,
    file_entries: Vec<FileEntry>,
    selected_file_index: usize,
    diff_scroll: usize,
    last_selected_hash: Option<String>,

    focus: Panel,
    branch_width_pct: u16,
    diff_height_pct: u16,
    dragging: Option<DragDirection>,
    last_size: Option<(u16, u16)>,
    last_mouse_pos: Option<(u16, u16)>,
    table_state: TableState,

    status_message: Option<String>,
    branches_loaded: bool,
    commits_loaded: bool,
    diff_pending: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragDirection {
    Vertical,
    Horizontal,
}

impl App {
    pub fn new(repo_path: String) -> Result<Self, String> {
        let branch_worker = BranchWorker::new(&repo_path)?;
        let commit_worker = CommitWorker::new(&repo_path)?;
        let diff_worker = DiffWorker::new(&repo_path)?;

        Ok(App {
            repo_path,
            branch_worker,
            commit_worker,
            diff_worker,
            all_branches: Vec::new(),
            branch_tree: Vec::new(),
            expanded_nodes: BTreeMap::new(),
            branch_index: 0,
            branch_scope: BranchScope::All,
            selected_branch: None,
            search_query: String::new(),
            cursor_pos: 0,
            all_commits: Vec::new(),
            filtered_commits: Vec::new(),
            selected_index: 0,
            visible_to_commit: Vec::new(),
            commit_info: None,
            diff_lines: Vec::new(),
            file_entries: Vec::new(),
            selected_file_index: 0,
            diff_scroll: 0,
            last_selected_hash: None,
            focus: Panel::Commits,
            branch_width_pct: DEFAULT_BRANCH_PCT,
            diff_height_pct: DEFAULT_DIFF_PCT,
            dragging: None,
            last_size: None,
            last_mouse_pos: None,
            table_state: TableState::default(),
            status_message: None,
            branches_loaded: false,
            commits_loaded: false,
            diff_pending: false,
        })
    }

    pub fn run(&mut self, terminal: &mut ratatui::Terminal<impl ratatui::backend::Backend>) -> Result<(), String> {
        self.request_branches();
        self.request_commits(None);

        loop {
            self.process_git_results();
            let draw_result = terminal.draw(|frame| self.render(frame));
            if let Err(e) = draw_result {
                return Err(format!("Render error: {}", e));
            }
            if !self.handle_event()? {
                break;
            }
        }

        Ok(())
    }

    // --- Worker communication (per-window threads) ---

    fn request_branches(&mut self) {
        self.branches_loaded = false;
        self.branch_worker.send(BranchCommand::FetchBranches {
            scope: self.branch_scope,
        });
    }

    fn request_commits(&mut self, branch: Option<String>) {
        self.commits_loaded = false;
        self.selected_branch = branch.clone();
        self.commit_worker.send(CommitCommand::FetchCommits {
            branch,
            scope: self.branch_scope,
        });
    }

    fn request_diff(&mut self, hash: &str) {
        self.diff_pending = true;
        self.diff_worker.send(DiffCommand::FetchDiff {
            hash: hash.to_string(),
        });
    }

    /// Poll all three worker channels for results (non-blocking, parallel streams).
    fn process_git_results(&mut self) {
        // Poll branch worker
        while let Some(result) = self.branch_worker.try_recv() {
            match result {
                BranchResult::Branches(branches) => {
                    self.all_branches = branches;
                    self.rebuild_branch_tree();
                    self.branches_loaded = true;
                }
                BranchResult::Error(err) => {
                    self.status_message = Some(err);
                }
            }
        }

        // Poll commit worker
        while let Some(result) = self.commit_worker.try_recv() {
            match result {
                CommitResult::Commits(commits) => {
                    self.all_commits = commits;
                    self.apply_search_filter();
                    self.commits_loaded = true;
                    if !self.filtered_commits.is_empty() && self.commit_info.is_none() {
                        let hash = self.filtered_commits[0].hash.clone();
                        if !hash.is_empty() {
                            self.request_diff(&hash);
                        }
                    }
                }
                CommitResult::Error(err) => {
                    self.status_message = Some(err);
                }
            }
        }

        // Poll diff worker
        while let Some(result) = self.diff_worker.try_recv() {
            match result {
                DiffResult::Diff {
                    commit_info,
                    diff_lines,
                    file_entries,
                } => {
                    self.commit_info = Some(commit_info);
                    self.diff_lines = diff_lines;
                    self.file_entries = file_entries;
                    self.diff_scroll = 0;
                    self.selected_file_index = 0;
                    self.diff_pending = false;
                }
                DiffResult::Error(err) => {
                    self.status_message = Some(err);
                }
            }
        }
    }

    // --- Branch tree ---

    fn rebuild_branch_tree(&mut self) {
        // Build tree, inserting "All Branches" as a virtual root item
        let all_item = TreeItem {
            name: "All Branches".to_string(),
            depth: 0,
            expandable: false,
            expanded: false,
            is_branch: true,
            full_path: "__all__".to_string(),
            tree_prefix: String::new(),
            key: "__all__".to_string(),
        };

        let mut items = vec![all_item];

        let mut root = tree::build_branch_tree(&self.all_branches);
        tree::sort_tree(&mut root);

        // Flatten with depth starting at 0 so items have depth >= 1
        let branch_items = tree::flatten_tree(&root, 0, &self.expanded_nodes);
        items.extend(branch_items);

        self.branch_tree = items;
    }

    // --- Search ---

    fn apply_search_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_commits = self.all_commits.clone();
        } else {
            let q = self.search_query.to_lowercase();
            self.filtered_commits = self
                .all_commits
                .iter()
                .filter(|c| {
                    c.graph_only
                        || c.hash.to_lowercase().contains(&q)
                        || c.author.to_lowercase().contains(&q)
                        || c.date.to_lowercase().contains(&q)
                        || c.subject.to_lowercase().contains(&q)
                })
                .cloned()
                .collect();
        }
        self.build_visible_mapping();
        self.selected_index = 0;
        self.clamp_selection();
    }

    fn build_visible_mapping(&mut self) {
        self.visible_to_commit = (0..self.filtered_commits.len())
            .filter(|&i| !self.filtered_commits[i].graph_only)
            .collect();
    }

    fn visible_count(&self) -> usize {
        self.visible_to_commit.len()
    }

    fn visible_to_filtered(&self, visible_idx: usize) -> usize {
        self.visible_to_commit
            .get(visible_idx)
            .copied()
            .unwrap_or(0)
    }

    fn filtered_to_visible(&self, filtered_idx: usize) -> Option<usize> {
        self.visible_to_commit
            .iter()
            .position(|&i| i == filtered_idx)
    }

    fn clamp_selection(&mut self) {
        if self.visible_count() > 0 {
            self.selected_index = self.selected_index.min(self.visible_count().saturating_sub(1));
        } else {
            self.selected_index = 0;
        }
    }

    // --- Events ---

    fn handle_event(&mut self) -> Result<bool, String> {
        if !event::poll(std::time::Duration::from_millis(250)).map_err(|e| format!("Poll error: {}", e))? {
            return Ok(true);
        }

        let ev = event::read().map_err(|e| format!("Event error: {}", e))?;

        match ev {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                self.handle_key(key)
            }
            Event::Mouse(mouse) => {
                // Ignore move events to avoid redrawing on every mouse movement.
                if let MouseEventKind::Moved = mouse.kind {
                    return Ok(true);
                }
                // Track mouse position for scroll
                if let MouseEventKind::Down(_) | MouseEventKind::Drag(_) = mouse.kind {
                    self.last_mouse_pos = Some((mouse.column, mouse.row));
                }
                if let MouseEventKind::Up(_) = mouse.kind {
                    self.dragging = None;
                }
                self.handle_mouse(mouse);
                Ok(true)
            }
            Event::Resize(w, h) => {
                self.last_size = Some((w, h));
                Ok(true)
            }
            _ => Ok(true),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool, String> {
        // Quit
        match key.code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(false),
            KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.suspend();
                return Ok(true);
            }
            _ => {}
        }

        // Global keys — work regardless of focus
        match key.code {
            // Focus cycling
            KeyCode::Tab => {
                self.focus = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus.prev()
                } else {
                    self.focus.next()
                };
                return Ok(true);
            }
            // BackTab: terminals that send ESC [ Z for Shift+Tab
            KeyCode::BackTab => {
                self.focus = self.focus.prev();
                return Ok(true);
            }
            KeyCode::Char('l') => {
                self.focus = self.focus.next();
                return Ok(true);
            }
            KeyCode::Char('h') => {
                self.focus = self.focus.prev();
                return Ok(true);
            }
            // Scope cycling — global
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cycle_scope();
                return Ok(true);
            }
            // Clipboard — global
            KeyCode::Char('y') => {
                if self.visible_count() > 0 {
                    let ci = self.visible_to_filtered(self.selected_index);
                    if let Some(c) = self.filtered_commits.get(ci) {
                        let short = if c.hash.len() > 7 {
                            &c.hash[..7]
                        } else {
                            &c.hash
                        };
                        let _ = clipboard::copy_to_clipboard(short);
                    }
                }
                return Ok(true);
            }
            KeyCode::Char('Y') => {
                if self.visible_count() > 0 {
                    let ci = self.visible_to_filtered(self.selected_index);
                    if let Some(c) = self.filtered_commits.get(ci) {
                        let _ = clipboard::copy_to_clipboard(&c.hash);
                    }
                }
                return Ok(true);
            }
            // Paste — global
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_paste();
                return Ok(true);
            }
            // Search editing — global (when not handled by focused panel)
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.focus == Panel::Search {
                    self.cursor_pos = 0;
                } else {
                    self.focus = Panel::Search;
                    self.cursor_pos = 0;
                }
                return Ok(true);
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.focus == Panel::Search {
                    self.cursor_pos = self.search_query.len();
                } else {
                    self.focus = Panel::Search;
                    self.cursor_pos = self.search_query.len();
                }
                return Ok(true);
            }
            KeyCode::Esc => {
                if self.focus == Panel::Search {
                    self.search_query.clear();
                    self.cursor_pos = 0;
                    self.apply_search_filter();
                }
                return Ok(true);
            }
            _ => {}
        }

        // Vim navigation keys (global alternative for up/down)
        match key.code {
            KeyCode::Char('j') => {
                self.move_down();
                return Ok(true);
            }
            KeyCode::Char('k') => {
                self.move_up();
                return Ok(true);
            }
            _ => {}
        }

        // Panel-specific keys
        match self.focus {
            Panel::Branches => self.handle_branch_keys(key),
            Panel::Search => self.handle_search_keys(key),
            Panel::Scope => self.handle_scope_keys(key),
            Panel::Commits => self.handle_commit_keys(key),
            Panel::Diff => self.handle_diff_keys(key),
        }

        Ok(true)
    }

    fn move_down(&mut self) {
        match self.focus {
            Panel::Branches => {
                if self.branch_index + 1 < self.branch_tree.len() {
                    self.branch_index += 1;
                } else {
                    self.branch_index = 0;
                }
            }
            Panel::Commits => {
                if self.visible_count() > 0 {
                    self.selected_index = (self.selected_index + 1) % self.visible_count();
                }
            }
            Panel::Diff => {
                if !self.file_entries.is_empty() {
                    self.selected_file_index = (self.selected_file_index + 1)
                        % self.file_entries.len();
                }
            }
            _ => {}
        }
    }

    fn move_up(&mut self) {
        match self.focus {
            Panel::Branches => {
                if self.branch_index > 0 {
                    self.branch_index -= 1;
                } else if !self.branch_tree.is_empty() {
                    self.branch_index = self.branch_tree.len() - 1;
                }
            }
            Panel::Commits => {
                if self.visible_count() > 0 {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    } else {
                        self.selected_index = self.visible_count() - 1;
                    }
                }
            }
            Panel::Diff => {
                if !self.file_entries.is_empty() {
                    if self.selected_file_index > 0 {
                        self.selected_file_index -= 1;
                    } else {
                        self.selected_file_index = self.file_entries.len() - 1;
                    }
                }
            }
            _ => {}
        }
    }

    fn cycle_scope(&mut self) {
        self.branch_scope = self.branch_scope.next();
        self.branch_index = 0;
        self.expanded_nodes.clear();
        self.search_query.clear();
        self.cursor_pos = 0;
        self.selected_branch = None;
        self.request_branches();
        self.request_commits(None);
    }

    fn handle_paste(&mut self) {
        if let Some(text) = clipboard::get_clipboard_text() {
            self.search_query = text;
            self.cursor_pos = self.search_query.len();
            self.apply_search_filter();
            self.focus = Panel::Search;
        }
    }

    fn suspend(&mut self) {
        let _ = execute!(
            std::io::stdout(),
            DisableMouseCapture,
            LeaveAlternateScreen,
        );
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
        );
    }

    // --- Panel key handlers ---

    fn handle_branch_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                self.branch_index = self.branch_index.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.branch_index + 1 < self.branch_tree.len() {
                    self.branch_index += 1;
                }
            }
            KeyCode::Right => {
                if let Some(item) = self.branch_tree.get(self.branch_index) {
                    if item.expandable && !item.expanded {
                        self.expanded_nodes.insert(item.key.clone(), true);
                        self.rebuild_branch_tree();
                    }
                }
            }
            KeyCode::Left => {
                if let Some(item) = self.branch_tree.get(self.branch_index) {
                    if item.expandable && item.expanded {
                        self.expanded_nodes.insert(item.key.clone(), false);
                        self.rebuild_branch_tree();
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(item) = self.branch_tree.get(self.branch_index) {
                    if item.expandable {
                        let new_state = !item.expanded;
                        self.expanded_nodes.insert(item.key.clone(), new_state);
                        self.rebuild_branch_tree();
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(item) = self.branch_tree.get(self.branch_index) {
                    if item.is_branch {
                        if item.full_path == "__all__" {
                            self.request_commits(None);
                        } else {
                            self.request_commits(Some(item.full_path.clone()));
                        }
                        self.focus = Panel::Commits;
                    } else if item.expandable {
                        // Toggle expandable directory on Enter
                        let new_state = !item.expanded;
                        self.expanded_nodes.insert(item.key.clone(), new_state);
                        self.rebuild_branch_tree();
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_scope_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.cycle_scope();
            }
            _ => {}
        }
    }

    fn handle_search_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_query.clear();
                self.cursor_pos = 0;
                self.apply_search_filter();
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    let prev = prev_char_boundary(&self.search_query, self.cursor_pos);
                    self.search_query.remove(prev);
                    self.cursor_pos = prev;
                    self.apply_search_filter();
                }
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.search_query.len() {
                    self.search_query.remove(self.cursor_pos);
                    self.apply_search_filter();
                }
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.cursor_pos = prev_word_boundary(&self.search_query, self.cursor_pos);
                } else {
                    self.cursor_pos = prev_char_boundary(&self.search_query, self.cursor_pos);
                }
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.cursor_pos = next_word_boundary(&self.search_query, self.cursor_pos);
                } else {
                    self.cursor_pos = next_char_boundary(&self.search_query, self.cursor_pos);
                }
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
            }
            KeyCode::End => {
                self.cursor_pos = self.search_query.len();
            }
            KeyCode::Char(ch) => {
                self.search_query.insert(self.cursor_pos, ch);
                self.cursor_pos += ch.len_utf8();
                self.apply_search_filter();
            }
            _ => {}
        }
    }

    fn handle_commit_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if self.visible_count() > 0 {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    } else {
                        self.selected_index = self.visible_count() - 1;
                    }
                }
            }
            KeyCode::Down => {
                if self.visible_count() > 0 {
                    self.selected_index = (self.selected_index + 1) % self.visible_count();
                }
            }
            KeyCode::Enter => {
                self.focus = Panel::Diff;
            }
            KeyCode::PageUp => {
                self.selected_index = self.selected_index.saturating_sub(10);
                self.clamp_selection();
            }
            KeyCode::PageDown => {
                if self.visible_count() > 0 {
                    self.selected_index = (self.selected_index + 10)
                        .min(self.visible_count().saturating_sub(1));
                }
            }
            _ => {}
        }
    }

    fn handle_diff_keys(&mut self, key: KeyEvent) {
        // Determine if we're scrolled past the metadata+file section
        let file_section_end = ui::diff_panel::diff_line_offset(
            self.commit_info.as_ref(),
            &self.file_entries,
        );
        let past_meta = self.diff_scroll >= file_section_end || self.file_entries.is_empty();

        match key.code {
            KeyCode::Up => {
                if past_meta {
                    self.diff_scroll = self.diff_scroll.saturating_sub(1);
                } else {
                    if self.selected_file_index > 0 {
                        self.selected_file_index -= 1;
                    } else {
                        self.selected_file_index = self.file_entries.len() - 1;
                    }
                }
            }
            KeyCode::Down => {
                if past_meta {
                    self.diff_scroll += 1;
                } else {
                    self.selected_file_index = (self.selected_file_index + 1)
                        % self.file_entries.len();
                }
            }
            KeyCode::Enter => {
                if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                    let offset = ui::diff_panel::diff_line_offset(
                        self.commit_info.as_ref(),
                        &self.file_entries,
                    );
                    self.diff_scroll = entry.diff_line + offset;
                }
            }
            KeyCode::Char('n') => {
                if !self.file_entries.is_empty() {
                    self.selected_file_index = (self.selected_file_index + 1)
                        % self.file_entries.len();
                    let offset = ui::diff_panel::diff_line_offset(
                        self.commit_info.as_ref(),
                        &self.file_entries,
                    );
                    if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                        self.diff_scroll = entry.diff_line + offset;
                    }
                }
            }
            KeyCode::Char('p') => {
                if !self.file_entries.is_empty() {
                    if self.selected_file_index > 0 {
                        self.selected_file_index -= 1;
                        let offset = ui::diff_panel::diff_line_offset(
                            self.commit_info.as_ref(),
                            &self.file_entries,
                        );
                        if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                            self.diff_scroll = entry.diff_line + offset;
                        }
                    } else {
                        // Wrap from first: reset to top
                        self.diff_scroll = 0;
                        self.selected_file_index = 0;
                    }
                }
            }
            KeyCode::Home => {
                self.diff_scroll = 0;
            }
            KeyCode::End => {
                // scroll to end
                self.diff_scroll = usize::MAX;
            }
            _ => {}
        }
    }

    // --- Mouse handling ---

    fn handle_mouse(&mut self, mouse: event::MouseEvent) {
        use crossterm::event::MouseButton;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_mouse_click(mouse.column, mouse.row);
                self.check_resize_start(mouse.column, mouse.row);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.handle_mouse_drag(mouse.column, mouse.row);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.dragging = None;
            }
            MouseEventKind::ScrollDown => {
                self.handle_scroll_at(mouse.column, mouse.row, 1);
            }
            MouseEventKind::ScrollUp => {
                self.handle_scroll_at(mouse.column, mouse.row, -1);
            }
            _ => {}
        }
    }

    fn handle_mouse_click(&mut self, col: u16, row: u16) {
        let Some((tw, th)) = self.last_size else { return };
        let full = Rect::new(0, 0, tw, th);
        let (branch_area, _, search_area, scope_area, table_area, diff_area) = self.compute_areas(full);
        let click_pos = (col, row);

        if rect_contains(&branch_area, click_pos) {
            self.focus = Panel::Branches;
            let rel_row = (row.saturating_sub(branch_area.y).saturating_sub(1)) as usize;
            if rel_row < self.branch_tree.len() {
                self.branch_index = rel_row;
                if let Some(item) = self.branch_tree.get(rel_row) {
                    if item.expandable {
                        let new_state = !item.expanded;
                        self.expanded_nodes.insert(item.key.clone(), new_state);
                        self.rebuild_branch_tree();
                    } else if item.is_branch {
                        if item.full_path == "__all__" {
                            self.request_commits(None);
                        } else {
                            self.request_commits(Some(item.full_path.clone()));
                        }
                        self.focus = Panel::Commits;
                    }
                }
            }
        } else if rect_contains(&scope_area, click_pos) {
            self.focus = Panel::Scope;
            self.cycle_scope();
        } else if rect_contains(&search_area, click_pos) {
            self.focus = Panel::Search;
        } else if rect_contains(&table_area, click_pos) {
            self.focus = Panel::Commits;
            let rel_row = (row.saturating_sub(table_area.y).saturating_sub(2)) as usize;
            let filtered_idx = rel_row + self.table_state.offset() as usize;
            if let Some(vis_idx) = self.filtered_to_visible(filtered_idx) {
                self.selected_index = vis_idx;
            }
        } else if rect_contains(&diff_area, click_pos) {
            self.focus = Panel::Diff;
            let rel_row = (row.saturating_sub(diff_area.y).saturating_sub(1)) as usize;
            // Check if clicking on a file entry after metadata
            let meta_offset = if let Some(ref info) = self.commit_info {
                ui::diff_panel::build_metadata_lines(info).len()
            } else {
                0
            };
            if rel_row > meta_offset && rel_row <= meta_offset + self.file_entries.len() + 2 {
                let file_idx = rel_row - meta_offset - 1;
                if file_idx < self.file_entries.len() {
                    self.selected_file_index = file_idx;
                    // Scroll to the file's diff section
                    let offset = ui::diff_panel::diff_line_offset(
                        self.commit_info.as_ref(),
                        &self.file_entries,
                    );
                    if let Some(entry) = self.file_entries.get(file_idx) {
                        self.diff_scroll = entry.diff_line + offset;
                    }
                }
            }
        }
    }

    fn check_resize_start(&mut self, col: u16, row: u16) {
        let Some((tw, th)) = self.last_size else { return };
        let full = Rect::new(0, 0, tw, th);
        let (branch_area, right_area, _, _, table_area, _) = self.compute_areas(full);

        let border_x = branch_area.x + branch_area.width;
        if (col as i32 - border_x as i32).abs() <= 2
            && row >= branch_area.y
            && row < branch_area.y + branch_area.height
        {
            self.dragging = Some(DragDirection::Vertical);
            return;
        }

        let border_y = table_area.y + table_area.height;
        if (row as i32 - border_y as i32).abs() <= 2
            && col >= right_area.x
            && col < right_area.x + right_area.width
        {
            self.dragging = Some(DragDirection::Horizontal);
        }
    }

    fn handle_mouse_drag(&mut self, col: u16, row: u16) {
        match self.dragging {
            Some(DragDirection::Vertical) => {
                if let Some((tw, _)) = self.last_size {
                    let pct = ((col as f32) / (tw as f32) * 100.0) as u16;
                    self.branch_width_pct = pct.clamp(MIN_BRANCH_PCT, MAX_BRANCH_PCT);
                }
            }
            Some(DragDirection::Horizontal) => {
                if let Some((_, th)) = self.last_size {
                    let pct = (((th.saturating_sub(row).saturating_sub(3)) as f32) / (th as f32) * 100.0) as u16;
                    self.diff_height_pct = pct.clamp(MIN_DIFF_PCT, MAX_DIFF_PCT);
                }
            }
            None => {}
        }
    }

    /// Scroll the panel under the mouse cursor (position-aware).
    fn handle_scroll_at(&mut self, col: u16, row: u16, direction: i32) {
        let Some((tw, th)) = self.last_size else { return };
        let full = Rect::new(0, 0, tw, th);
        let (branch_area, _, _, _, table_area, diff_area) = self.compute_areas(full);
        let pos = (col, row);

        if rect_contains(&branch_area, pos) {
            if direction > 0 {
                // Scroll down
                if self.branch_index + 1 < self.branch_tree.len() {
                    self.branch_index += 1;
                } else {
                    self.branch_index = 0;
                }
            } else {
                // Scroll up
                if self.branch_index > 0 {
                    self.branch_index -= 1;
                } else if !self.branch_tree.is_empty() {
                    self.branch_index = self.branch_tree.len() - 1;
                }
            }
        } else if rect_contains(&table_area, pos) {
            if self.visible_count() > 0 {
                if direction > 0 {
                    self.selected_index = (self.selected_index + 1) % self.visible_count();
                } else {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    } else {
                        self.selected_index = self.visible_count() - 1;
                    }
                }
            }
        } else if rect_contains(&diff_area, pos) {
            if direction > 0 {
                self.diff_scroll += 1;
            } else {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
        }
    }

    #[allow(dead_code)]
    fn handle_scroll_down(&mut self) {
        match self.focus {
            Panel::Branches => {
                if self.branch_index + 1 < self.branch_tree.len() {
                    self.branch_index += 1;
                } else {
                    self.branch_index = 0;
                }
            }
            Panel::Commits => {
                if self.visible_count() > 0 {
                    self.selected_index = (self.selected_index + 1) % self.visible_count();
                }
            }
            Panel::Diff => {
                self.diff_scroll += 1;
            }
            _ => {}
        }
    }

    #[allow(dead_code)]
    fn handle_scroll_up(&mut self) {
        match self.focus {
            Panel::Branches => {
                if self.branch_index > 0 {
                    self.branch_index -= 1;
                } else if !self.branch_tree.is_empty() {
                    self.branch_index = self.branch_tree.len() - 1;
                }
            }
            Panel::Commits => {
                if self.visible_count() > 0 {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    } else {
                        self.selected_index = self.visible_count() - 1;
                    }
                }
            }
            Panel::Diff => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    // --- Layout ---

    fn compute_areas(&self, full: Rect) -> (Rect, Rect, Rect, Rect, Rect, Rect) {
        let branch_w = Constraint::Percentage(self.branch_width_pct);
        let right_w = Constraint::Percentage(100 - self.branch_width_pct);

        let horizontal = Layout::horizontal([branch_w, right_w]).split(full);
        let branch_area = horizontal[0];
        let right_area = horizontal[1];

        // Use flexible constraints that won't overflow small terminals
        let search_h = Constraint::Length(3.min(right_area.height / 3));
        let diff_h = Constraint::Percentage(self.diff_height_pct.min(90));
        let help_h = Constraint::Length(3.min(right_area.height.saturating_sub(6) / 2));

        let main_split = Layout::vertical([
            search_h,
            Constraint::Min(0),
            diff_h,
            help_h,
        ])
        .split(right_area);

        let search_scope_area = main_split[0];
        let table_area = main_split[1];
        let diff_area = main_split[2];

        let search_split = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(10.min(search_scope_area.width.saturating_sub(2))),
        ])
        .split(search_scope_area);
        let search_area = search_split[0];
        let scope_area = search_split[1];

        (branch_area, right_area, search_area, scope_area, table_area, diff_area)
    }

    // --- Rendering ---

    fn render(&mut self, frame: &mut Frame) {
        let full = frame.area();
        self.last_size = Some((full.width, full.height));

        // Guard against zero-size terminal (can happen during resize)
        if full.width < 20 || full.height < 8 {
            return;
        }

        let (branch_area, _right_area, search_area, scope_area, table_area, diff_area) = self.compute_areas(full);

        let help_area = Rect::new(
            full.x,
            full.y + full.height.saturating_sub(3),
            full.width,
            3.min(full.height),
        );

        // Clamp cursor
        self.cursor_pos = self.cursor_pos.min(self.search_query.len());
        self.clamp_selection();

        ui::branch_panel::render(
            frame,
            branch_area,
            &self.branch_tree,
            self.branch_index,
            self.focus == Panel::Branches,
        );

        let branch_label = self.selected_branch.as_deref().unwrap_or("all branches");
        let title = format!("Git Log - {} [{}]", self.repo_path, branch_label);
        ui::search_panel::render(
            frame,
            search_area,
            &self.search_query,
            self.cursor_pos,
            branch_label,
            &title,
            self.focus == Panel::Search,
        );

        ui::scope_panel::render(frame, scope_area, self.branch_scope, self.focus == Panel::Scope);

        ui::commit_table::render(
            frame,
            table_area,
            &self.filtered_commits,
            self.selected_index,
            self.focus == Panel::Commits,
            &self.visible_to_commit,
            self.all_commits.len(),
            !self.search_query.is_empty(),
            &mut self.table_state,
        );

        let short_hash = self.commit_info.as_ref().map(|info| {
            &info.hash[..std::cmp::min(8, info.hash.len())]
        });
        ui::diff_panel::render(
            frame,
            diff_area,
            self.commit_info.as_ref(),
            &self.diff_lines,
            &self.file_entries,
            self.selected_file_index,
            self.diff_scroll,
            self.focus == Panel::Diff,
            short_hash,
        );

        let count_info = ui::help_bar::format_commit_count(
            self.selected_index,
            self.visible_count(),
            self.all_commits.len(),
        );
        ui::help_bar::render(frame, help_area, self.focus, &count_info);

        // Trigger diff load on selection change
        let current_hash = if self.visible_count() > 0 {
            let ci = self.visible_to_filtered(self.selected_index);
            self.filtered_commits.get(ci).map(|c| c.hash.clone())
        } else {
            None
        };

        if current_hash != self.last_selected_hash {
            self.last_selected_hash = current_hash.clone();
            if let Some(hash) = current_hash {
                if !hash.is_empty() {
                    self.request_diff(&hash);
                }
            }
        }
    }
}

fn rect_contains(rect: &Rect, pos: (u16, u16)) -> bool {
    pos.0 >= rect.x
        && pos.0 < rect.x + rect.width
        && pos.1 >= rect.y
        && pos.1 < rect.y + rect.height
}

/// Move to the previous char boundary (for single-step left).
fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    for (i, _) in s.char_indices() {
        if i >= pos {
            // i is at or past pos; previous boundary is the one before
            break;
        }
    }
    // Walk backwards from pos-1 to find a char boundary
    for i in (0..pos).rev() {
        if s.is_char_boundary(i) {
            return i;
        }
    }
    0
}

/// Move to the next char boundary (for single-step right).
fn next_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    // Find next char boundary after pos
    for (i, _) in s.char_indices().skip(1) {
        if i > pos {
            return i;
        }
    }
    s.len()
}

fn prev_word_boundary(s: &str, pos: usize) -> usize {
    // Ensure pos is at a char boundary
    let pos = prev_char_boundary(s, pos);
    if pos == 0 {
        return 0;
    }
    // Work with chars from the end backward
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let char_pos = chars.iter().position(|&(i, _)| i == pos).unwrap_or(chars.len());
    if char_pos == 0 {
        return 0;
    }
    let mut idx = char_pos - 1;
    // If at a word char, skip to start of word
    loop {
        let (_bi, ch) = chars.get(idx).copied().unwrap_or((0, '\0'));
        if ch.is_alphanumeric() || ch == '_' {
            if idx == 0 { return 0; }
            idx = idx.saturating_sub(1);
        } else {
            // Found non-word char; the boundary is right after it
            return chars.get(idx + 1).map(|&(i, _)| i).unwrap_or(0);
        }
    }
}

fn next_word_boundary(s: &str, pos: usize) -> usize {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let char_pos = chars.iter().position(|&(i, _)| i >= pos).unwrap_or(chars.len());
    let mut idx = char_pos;
    // Skip word characters
    while idx < chars.len() {
        let (_, ch) = chars[idx];
        if ch.is_alphanumeric() || ch == '_' {
            idx += 1;
        } else {
            break;
        }
    }
    // Skip non-word characters
    while idx < chars.len() {
        let (_, ch) = chars[idx];
        if !ch.is_alphanumeric() && ch != '_' {
            idx += 1;
        } else {
            break;
        }
    }
    chars.get(idx).map(|&(i, _)| i).unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prev_char_boundary_ascii() {
        assert_eq!(prev_char_boundary("hello", 3), 2);
        assert_eq!(prev_char_boundary("hello", 0), 0);
    }

    #[test]
    fn test_prev_char_boundary_utf8() {
        // "Mäller": M(0-1)+ä(1-3)+l(3-4)+l(4-5)+e(5-6)+r(6-7)
        assert_eq!(prev_char_boundary("Mäller", 3), 1); // from 'l', prev boundary is start of 'ä'
        assert_eq!(prev_char_boundary("Mäller", 2), 1); // inside 'ä', prev boundary is start of 'ä'
    }

    #[test]
    fn test_next_char_boundary_ascii() {
        assert_eq!(next_char_boundary("hello", 2), 3);
        assert_eq!(next_char_boundary("hello", 5), 5); // at end
    }

    #[test]
    fn test_next_char_boundary_utf8() {
        // "Mäller": M(0)+ä(1-2)+l(3)+l(4)+e(5)+r(6)
        assert_eq!(next_char_boundary("Mäller", 1), 3); // from ä start to next 'l'
        assert_eq!(next_char_boundary("Mäller", 3), 4); // from 'l' to next 'l'
    }

    #[test]
    fn test_prev_word_boundary() {
        // "hello world" at pos 6: snaps to pos 5 (space), then walks to start of "hello"
        assert_eq!(prev_word_boundary("hello world", 6), 0);
        // "foo bar": 'r' at pos 6 (char_indices: 0=f,1=o,2=o,3=' ',4=b,5=a,6=r)
        assert_eq!(prev_word_boundary("foo bar", 6), 4);
    }

    #[test]
    fn test_next_word_boundary() {
        // Skips "hello", then space, then starts "world"
        assert_eq!(next_word_boundary("hello world", 0), 6);
        // From space at pos 5: skips space, lands at start of "world"
        assert_eq!(next_word_boundary("hello world", 5), 6);
        assert_eq!(next_word_boundary("hello", 0), 5); // end of string
    }

    /// Helper to build a minimal App for testing pure logic functions.
    fn test_app() -> App {
        App {
            repo_path: ".".to_string(),
            branch_worker: BranchWorker::new(".").unwrap(),
            commit_worker: CommitWorker::new(".").unwrap(),
            diff_worker: DiffWorker::new(".").unwrap(),
            all_branches: Vec::new(),
            branch_tree: Vec::new(),
            expanded_nodes: BTreeMap::new(),
            branch_index: 0,
            branch_scope: BranchScope::All,
            selected_branch: None,
            search_query: String::new(),
            cursor_pos: 0,
            all_commits: Vec::new(),
            filtered_commits: Vec::new(),
            selected_index: 0,
            visible_to_commit: Vec::new(),
            commit_info: None,
            diff_lines: Vec::new(),
            file_entries: Vec::new(),
            selected_file_index: 0,
            diff_scroll: 0,
            last_selected_hash: None,
            focus: Panel::Commits,
            branch_width_pct: DEFAULT_BRANCH_PCT,
            diff_height_pct: DEFAULT_DIFF_PCT,
            dragging: None,
            last_size: None,
            last_mouse_pos: None,
            table_state: TableState::default(),
            status_message: None,
            branches_loaded: false,
            commits_loaded: false,
            diff_pending: false,
        }
    }

    #[test]
    fn test_search_filter_empty_query() {
        let mut app = test_app();
        app.all_commits = vec![
            Commit {
                hash: "abc".into(),
                author: "alice".into(),
                date: "2024-01-01".into(),
                subject: "fix bug".into(),
                graph: "*".into(),
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ];
        app.search_query.clear();
        app.apply_search_filter();
        assert_eq!(app.filtered_commits.len(), 1);
    }

    #[test]
    fn test_search_filter_by_subject() {
        let mut app = test_app();
        app.all_commits = vec![
            Commit {
                hash: "abc".into(),
                author: "alice".into(),
                date: "2024-01-01".into(),
                subject: "fix bug".into(),
                graph: "*".into(),
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
            Commit {
                hash: "def".into(),
                author: "bob".into(),
                date: "2024-01-02".into(),
                subject: "add feature".into(),
                graph: "*".into(),
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ];
        app.search_query = "bug".into();
        app.apply_search_filter();
        assert_eq!(app.filtered_commits.len(), 1);
        assert_eq!(app.filtered_commits[0].hash, "abc");
    }

    #[test]
    fn test_search_filter_case_insensitive() {
        let mut app = test_app();
        app.all_commits = vec![
            Commit {
                hash: "abc".into(),
                author: "ALICE".into(),
                date: "2024-01-01".into(),
                subject: "Fix Bug".into(),
                graph: "*".into(),
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ];
        app.search_query = "bug".into();
        app.apply_search_filter();
        assert_eq!(app.filtered_commits.len(), 1);
    }

    #[test]
    fn test_visible_mapping_skips_graph_only() {
        let mut app = test_app();
        app.filtered_commits = vec![
            Commit {
                hash: "abc".into(),
                author: "a".into(),
                date: "d".into(),
                subject: "s".into(),
                graph: "*".into(),
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
            Commit {
                hash: "".into(),
                author: "".into(),
                date: "".into(),
                subject: "".into(),
                graph: "|".into(),
                merge: false,
                graph_only: true,
                decorations: vec![],
                deco_line: 0,
            },
            Commit {
                hash: "def".into(),
                author: "b".into(),
                date: "d".into(),
                subject: "s".into(),
                graph: "*".into(),
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ];
        app.build_visible_mapping();
        assert_eq!(app.visible_count(), 2);
        assert_eq!(app.visible_to_filtered(0), 0); // first commit
        assert_eq!(app.visible_to_filtered(1), 2); // third commit
    }

    #[test]
    fn test_clamp_selection_empty() {
        let mut app = test_app();
        app.clamp_selection();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_clamp_selection_in_range() {
        let mut app = test_app();
        app.filtered_commits = vec![
            Commit {
                hash: "abc".into(),
                author: "a".into(),
                date: "d".into(),
                subject: "s".into(),
                graph: "*".into(),
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ];
        app.build_visible_mapping();
        app.selected_index = 0;
        app.clamp_selection();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_app_new_returns_ok() {
        let result = App::new(".".to_string());
        assert!(result.is_ok());
    }
}
