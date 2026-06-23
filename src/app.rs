use std::collections::BTreeMap;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    Frame,
};

use crate::clipboard;
use crate::models::*;
use crate::git::GitWorker;
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
    git_worker: GitWorker,

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
    file_lines: Vec<String>,
    selected_file_index: usize,
    diff_scroll: usize,
    last_selected_hash: Option<String>,

    focus: Panel,
    branch_width_pct: u16,
    diff_height_pct: u16,
    dragging: Option<DragDirection>,
    last_size: Option<(u16, u16)>,
    last_mouse_pos: Option<(u16, u16)>,

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
    pub fn new(repo_path: String) -> Self {
        let git_worker = GitWorker::new();

        App {
            repo_path,
            git_worker,
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
            file_lines: Vec::new(),
            selected_file_index: 0,
            diff_scroll: 0,
            last_selected_hash: None,
            focus: Panel::Commits,
            branch_width_pct: DEFAULT_BRANCH_PCT,
            diff_height_pct: DEFAULT_DIFF_PCT,
            dragging: None,
            last_size: None,
            last_mouse_pos: None,
            status_message: None,
            branches_loaded: false,
            commits_loaded: false,
            diff_pending: false,
        }
    }

    pub fn run(&mut self, terminal: &mut ratatui::Terminal<impl ratatui::backend::Backend>) -> Result<(), String> {
        self.request_branches();
        self.request_commits(None);

        loop {
            self.process_git_results();
            terminal
                .draw(|frame| self.render(frame))
                .map_err(|e| format!("Render error: {}", e))?;
            if !self.handle_event()? {
                break;
            }
        }

        Ok(())
    }

    // --- Git worker communication ---

    fn request_branches(&mut self) {
        self.branches_loaded = false;
        self.git_worker.send(GitCommand::FetchBranches {
            repo_path: self.repo_path.clone(),
            scope: self.branch_scope,
        });
    }

    fn request_commits(&mut self, branch: Option<String>) {
        self.commits_loaded = false;
        self.selected_branch = branch.clone();
        self.git_worker.send(GitCommand::FetchCommits {
            repo_path: self.repo_path.clone(),
            branch,
            scope: self.branch_scope,
        });
    }

    fn request_diff(&mut self, hash: &str) {
        self.diff_pending = true;
        self.git_worker.send(GitCommand::FetchDiff {
            repo_path: self.repo_path.clone(),
            hash: hash.to_string(),
        });
    }

    fn process_git_results(&mut self) {
        while let Some(result) = self.git_worker.try_recv() {
            match result {
                GitResult::Branches(branches) => {
                    self.all_branches = branches;
                    self.rebuild_branch_tree();
                    self.branches_loaded = true;
                }
                GitResult::Commits(commits) => {
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
                GitResult::Diff {
                    commit_info,
                    diff_lines,
                    file_entries,
                } => {
                    self.commit_info = Some(commit_info);
                    self.diff_lines = diff_lines;
                    self.file_entries = file_entries;
                    self.file_lines = ui::diff_panel::build_file_lines(&self.file_entries);
                    self.diff_scroll = 0;
                    self.selected_file_index = 0;
                    self.diff_pending = false;
                }
                GitResult::Error(err) => {
                    self.status_message = Some(err);
                }
            }
        }
    }

    // --- Branch tree ---

    fn rebuild_branch_tree(&mut self) {
        // Build tree, inserting "All Branches" as a virtual root item
        let all_item = TreeItem {
            name: "  All Branches".to_string(),
            depth: 1,
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

        // Flatten with depth starting at 1 so items have depth >= 2
        let branch_items = tree::flatten_tree(&root, 1, &self.expanded_nodes);
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

    #[allow(dead_code)]
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
        if !event::poll(std::time::Duration::from_millis(16)).map_err(|e| format!("Poll error: {}", e))? {
            return Ok(true);
        }

        let ev = event::read().map_err(|e| format!("Event error: {}", e))?;

        match ev {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                self.handle_key(key)
            }
            Event::Mouse(mouse) => {
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
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen,);
        disable_raw_mode().ok();

        #[cfg(unix)]
        unsafe {
            libc::kill(libc::getpid(), libc::SIGTSTP);
        }

        enable_raw_mode().ok();
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
                    self.cursor_pos -= 1;
                    self.search_query.remove(self.cursor_pos);
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
                    self.cursor_pos = self.cursor_pos.saturating_sub(1);
                }
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.cursor_pos = next_word_boundary(&self.search_query, self.cursor_pos);
                } else {
                    self.cursor_pos = (self.cursor_pos + 1).min(self.search_query.len());
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
                self.cursor_pos += 1;
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
            if rel_row < self.visible_count() {
                self.selected_index = rel_row;
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

        let main_split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Percentage(self.diff_height_pct),
            Constraint::Length(3),
        ])
        .split(right_area);

        let search_scope_area = main_split[0];
        let table_area = main_split[1];
        let diff_area = main_split[2];

        let search_split = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(10),
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
        let title = format!("Git Log — {} [{}]", self.repo_path, branch_label);
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
        );

        ui::diff_panel::render(
            frame,
            diff_area,
            self.commit_info.as_ref(),
            &self.diff_lines,
            &self.file_entries,
            &self.file_lines,
            self.selected_file_index,
            self.diff_scroll,
            self.focus == Panel::Diff,
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

fn prev_word_boundary(s: &str, mut pos: usize) -> usize {
    while pos > 0 {
        pos -= 1;
        if s.as_bytes()
            .get(pos)
            .map(|&b| b.is_ascii_alphanumeric() || b == b'_')
            .unwrap_or(false)
        {
            while pos > 0 {
                let prev = s.as_bytes()[pos - 1];
                if !prev.is_ascii_alphanumeric() && prev != b'_' {
                    break;
                }
                pos -= 1;
            }
            break;
        }
    }
    pos
}

fn next_word_boundary(s: &str, mut pos: usize) -> usize {
    let len = s.len();
    while pos < len {
        let ch = s.as_bytes()[pos];
        if !ch.is_ascii_alphanumeric() && ch != b'_' {
            break;
        }
        pos += 1;
    }
    while pos < len {
        let ch = s.as_bytes()[pos];
        if ch.is_ascii_alphanumeric() || ch == b'_' {
            break;
        }
        pos += 1;
    }
    pos
}
