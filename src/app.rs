use std::collections::BTreeMap;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton},
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

/// Ratio of branch panel width (percentage).
const DEFAULT_BRANCH_PCT: u16 = 20;
const MIN_BRANCH_PCT: u16 = 10;
const MAX_BRANCH_PCT: u16 = 40;

/// Ratio of diff panel height (percentage).
const DEFAULT_DIFF_PCT: u16 = 35;
const MIN_DIFF_PCT: u16 = 10;
const MAX_DIFF_PCT: u16 = 65;

/// The main application state.
pub struct App {
    /// Path to the git repository.
    repo_path: String,

    // --- Git worker ---
    git_worker: GitWorker,

    // --- Branch state ---
    /// Flat list of all loaded branch names.
    all_branches: Vec<String>,
    /// Hierarchical tree of branches.
    branch_tree: Vec<TreeItem>,
    /// Which tree nodes are expanded (key = full_path).
    expanded_nodes: BTreeMap<String, bool>,
    /// Selected row in the branch list.
    branch_index: usize,
    /// Current branch scope filter.
    branch_scope: BranchScope,
    /// Currently loaded branch name (None = all branches).
    selected_branch: Option<String>,

    // --- Search state ---
    search_query: String,
    cursor_pos: usize,

    // --- Commit state ---
    /// All commits for the currently loaded branch/scope.
    all_commits: Vec<Commit>,
    /// Filtered commits based on search query.
    filtered_commits: Vec<Commit>,
    /// Mapping from filtered index to original commit index.
    commit_map: Vec<usize>,
    /// Selected row in the commit table.
    selected_index: usize,

    // --- Diff state ---
    /// Structured commit metadata for the selected commit.
    commit_info: Option<CommitInfo>,
    /// Raw diff output lines.
    diff_lines: Vec<String>,
    /// Changed files in the diff.
    file_entries: Vec<FileEntry>,
    /// Formatted file list display lines.
    file_lines: Vec<String>,
    /// Selected file index in the file list.
    selected_file_index: usize,
    /// Scroll position in the diff panel.
    diff_scroll: usize,
    /// Last selected commit index (to detect changes).
    last_selected_index: Option<usize>,

    // --- Focus and layout ---
    focus: Panel,
    branch_width_pct: u16,
    diff_height_pct: u16,
    dragging: Option<DragDirection>,
    /// Last terminal size (to detect resize).
    last_size: Option<(u16, u16)>,

    // --- Clipboard ---
    status_message: Option<String>,

    // --- Load state ---
    /// Whether branches have been initially loaded.
    branches_loaded: bool,
    /// Whether commits have been initially loaded.
    commits_loaded: bool,
    /// Whether a diff request is pending.
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
            commit_map: Vec::new(),
            selected_index: 0,
            commit_info: None,
            diff_lines: Vec::new(),
            file_entries: Vec::new(),
            file_lines: Vec::new(),
            selected_file_index: 0,
            diff_scroll: 0,
            last_selected_index: None,
            focus: Panel::Commits,
            branch_width_pct: DEFAULT_BRANCH_PCT,
            diff_height_pct: DEFAULT_DIFF_PCT,
            dragging: None,
            last_size: None,
            status_message: None,
            branches_loaded: false,
            commits_loaded: false,
            diff_pending: false,
        }
    }

    /// Run the main event loop.
    pub fn run(&mut self, terminal: &mut ratatui::Terminal<impl ratatui::backend::Backend>) -> Result<(), String> {
        // Initial data load
        self.request_branches();
        self.request_commits(None);

        loop {
            // Check for git worker results
            self.process_git_results();

            // Draw the UI
            terminal
                .draw(|frame| self.render(frame))
                .map_err(|e| format!("Render error: {}", e))?;

            // Handle events
            if !self.handle_event()? {
                break; // quit
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
                    // Load diff for first commit
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

    // --- Branch tree management ---

    fn rebuild_branch_tree(&mut self) {
        let mut root = tree::build_branch_tree(&self.all_branches);
        tree::sort_tree(&mut root);
        self.branch_tree = tree::flatten_tree(
            &root,
            0,
            &self.expanded_nodes,
        );
    }

    // --- Search filtering ---

    fn apply_search_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_commits = self.all_commits.clone();
            self.commit_map = (0..self.all_commits.len()).collect();
        } else {
            let q = self.search_query.to_lowercase();
            let mut filtered = Vec::new();
            let mut map = Vec::new();
            for (i, commit) in self.all_commits.iter().enumerate() {
                if commit.hash.to_lowercase().contains(&q)
                    || commit.author.to_lowercase().contains(&q)
                    || commit.date.to_lowercase().contains(&q)
                    || commit.subject.to_lowercase().contains(&q)
                {
                    filtered.push(commit.clone());
                    map.push(i);
                }
            }
            self.filtered_commits = filtered;
            self.commit_map = map;
        }
        self.selected_index = self
            .selected_index
            .min(self.filtered_commits.len().saturating_sub(1));
    }

    // --- Input handling ---

    /// Handle a single event. Returns Ok(false) to quit.
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
        // Global keys (always handled)
        match key.code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(false),
            KeyCode::Tab => {
                self.focus = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus.prev()
                } else {
                    self.focus.next()
                };
                return Ok(true);
            }
            KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.suspend();
                return Ok(true);
            }
            _ => {}
        }

        // Clipboard global keys
        if key.code == KeyCode::Char('y') {
            self.copy_current_hash();
            return Ok(true);
        }
        if key.code == KeyCode::Char('Y') {
            self.copy_current_full_hash();
            return Ok(true);
        }

        // Paste (Ctrl+V) — handled for search
        if key.code == KeyCode::Char('v') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.handle_paste();
            return Ok(true);
        }

        // Handle based on focused panel
        match self.focus {
            Panel::Branches => self.handle_branch_keys(key),
            Panel::Search => self.handle_search_keys(key),
            Panel::Scope => self.handle_scope_keys(key),
            Panel::Commits => self.handle_commit_keys(key),
            Panel::Diff => self.handle_diff_keys(key),
        }

        Ok(true)
    }

    fn copy_current_hash(&self) {
        if let Some(commit) = self.filtered_commits.get(self.selected_index) {
            let short = if commit.hash.len() > 7 {
                &commit.hash[..7]
            } else {
                &commit.hash
            };
            if let Err(_e) = clipboard::copy_to_clipboard(short) {
                // silently fail
            }
        }
    }

    fn copy_current_full_hash(&self) {
        if let Some(commit) = self.filtered_commits.get(self.selected_index) {
            let _ = clipboard::copy_to_clipboard(&commit.hash);
        }
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
        // Save terminal state and return to cooked mode
        let _ = execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
        );
        disable_raw_mode().ok();

        // Send SIGTSTP
        #[cfg(unix)]
        unsafe {
            libc::kill(libc::getpid(), libc::SIGTSTP);
        }

        // After resume, restore terminal
        enable_raw_mode().ok();
    }

    // --- Panel-specific key handlers ---

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
                // Expand node
                if let Some(item) = self.branch_tree.get(self.branch_index) {
                    if item.expandable && !item.expanded {
                        self.expanded_nodes
                            .insert(item.full_path.clone(), true);
                        self.rebuild_branch_tree();
                    }
                }
            }
            KeyCode::Left => {
                // Collapse node
                if let Some(item) = self.branch_tree.get(self.branch_index) {
                    if item.expandable && item.expanded {
                        self.expanded_nodes
                            .insert(item.full_path.clone(), false);
                        self.rebuild_branch_tree();
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Toggle expand
                if let Some(item) = self.branch_tree.get(self.branch_index) {
                    if item.expandable {
                        let new_state = !item.expanded;
                        self.expanded_nodes
                            .insert(item.full_path.clone(), new_state);
                        self.rebuild_branch_tree();
                    }
                }
            }
            KeyCode::Enter => {
                // Load branch commits
                if let Some(item) = self.branch_tree.get(self.branch_index) {
                    if item.is_branch {
                        self.request_commits(Some(item.full_path.clone()));
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_scope_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.branch_scope = self.branch_scope.next();
                self.request_branches();
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.branch_scope = self.branch_scope.next();
                self.request_branches();
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
                    // Jump to previous word start
                    self.cursor_pos = prev_word_boundary(&self.search_query, self.cursor_pos);
                } else {
                    self.cursor_pos = self.cursor_pos.saturating_sub(1);
                }
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Jump to next word start
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
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor_pos = 0;
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                } else if !self.filtered_commits.is_empty() {
                    self.selected_index = self.filtered_commits.len() - 1;
                }
            }
            KeyCode::Down => {
                if self.selected_index + 1 < self.filtered_commits.len() {
                    self.selected_index += 1;
                } else {
                    self.selected_index = 0;
                }
            }
            KeyCode::Enter => {
                self.focus = Panel::Diff;
            }
            KeyCode::PageUp => {
                self.selected_index = self.selected_index.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.selected_index = (self.selected_index + 10)
                    .min(self.filtered_commits.len().saturating_sub(1));
            }
            _ => {}
        }
    }

    fn handle_diff_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if !self.file_entries.is_empty() {
                    self.selected_file_index = self.selected_file_index.saturating_sub(1);
                } else {
                    self.diff_scroll = self.diff_scroll.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if !self.file_entries.is_empty() {
                    self.selected_file_index = (self.selected_file_index + 1)
                        .min(self.file_entries.len().saturating_sub(1));
                } else {
                    self.diff_scroll += 1;
                }
            }
            KeyCode::Enter => {
                // Jump to selected file's diff section
                if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                    self.diff_scroll = entry.diff_line;
                }
            }
            KeyCode::Char('n') => {
                if self.selected_file_index + 1 < self.file_entries.len() {
                    self.selected_file_index += 1;
                    if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                        self.diff_scroll = entry.diff_line;
                    }
                }
            }
            KeyCode::Char('p') => {
                if self.selected_file_index > 0 {
                    self.selected_file_index -= 1;
                    if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                        self.diff_scroll = entry.diff_line;
                    }
                }
            }
            KeyCode::Home => {
                self.diff_scroll = 0;
            }
            KeyCode::End => {
                self.diff_scroll = self.diff_lines.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    // --- Mouse handling ---

    fn handle_mouse(&mut self, mouse: event::MouseEvent) {
        use crossterm::event::MouseEventKind;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_mouse_click(mouse.column, mouse.row);
                // Check if clicking on a resize border
                self.check_resize_start(mouse.column, mouse.row);
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.handle_mouse_drag(mouse.column, mouse.row);
            }
            MouseEventKind::ScrollDown => {
                self.handle_scroll_down();
            }
            MouseEventKind::ScrollUp => {
                self.handle_scroll_up();
            }
            _ => {}
        }
    }

    fn handle_mouse_click(&mut self, col: u16, row: u16) {
        // Determine which area was clicked based on layout
        // We need to compute layout areas (simplified: use known layout structure)
        // The layout is: [branches | [search/scope, commits, diff, help]]
        let (branch_area, _right_area, search_area, scope_area, table_area, diff_area) = self.compute_areas(Rect::new(0, 0, col + 1, row + 1));

        let click_pos = (col, row);

        if Self::rect_contains(&branch_area, click_pos) {
            self.focus = Panel::Branches;
            let rel_row = (row.saturating_sub(branch_area.y).saturating_sub(1)) as usize;
            if rel_row < self.branch_tree.len() {
                self.branch_index = rel_row;
                // Double-click to load branch
                if let Some(item) = self.branch_tree.get(rel_row) {
                    if item.is_branch {
                        self.request_commits(Some(item.full_path.clone()));
                    }
                }
            }
        } else if Self::rect_contains(&scope_area, click_pos) {
            self.focus = Panel::Scope;
            self.branch_scope = self.branch_scope.next();
            self.request_branches();
        } else if Self::rect_contains(&search_area, click_pos) {
            self.focus = Panel::Search;
        } else if Self::rect_contains(&table_area, click_pos) {
            self.focus = Panel::Commits;
            let rel_row = (row.saturating_sub(table_area.y).saturating_sub(2)) as usize; // header + border
            if rel_row < self.filtered_commits.len() {
                self.selected_index = rel_row;
            }
        } else if Self::rect_contains(&diff_area, click_pos) {
            self.focus = Panel::Diff;
            let rel_row = (row.saturating_sub(diff_area.y).saturating_sub(1)) as usize;
            // Check if clicking on a file entry
            if rel_row < self.file_entries.len() {
                self.selected_file_index = rel_row;
            }
        }
    }

    fn rect_contains(rect: &Rect, pos: (u16, u16)) -> bool {
        pos.0 >= rect.x && pos.0 < rect.x + rect.width && pos.1 >= rect.y && pos.1 < rect.y + rect.height
    }

    fn check_resize_start(&mut self, col: u16, row: u16) {
        if self.last_size.is_none() {
            return;
        }
        let (tw, th) = self.last_size.unwrap();
        let full = Rect::new(0, 0, tw, th);
        let (branch_area, right_area, _search_area, _scope_area, table_area, _diff_area) = self.compute_areas(full);

        // Vertical resize: check if clicking on the border between branches and right panel
        let border_x = branch_area.x + branch_area.width;
        if (col as i32 - border_x as i32).abs() <= 2 && row >= branch_area.y && row < branch_area.y + branch_area.height {
            self.dragging = Some(DragDirection::Vertical);
            return;
        }

        // Horizontal resize: check if clicking on the border between commits and diff
        let border_y = table_area.y + table_area.height;
        if (row as i32 - border_y as i32).abs() <= 2 && col >= right_area.x && col < right_area.x + right_area.width {
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
                    // Calculate diff height percentage from the bottom
                    let pct = (((th - row) as f32) / (th as f32) * 100.0) as u16;
                    self.diff_height_pct = pct.clamp(MIN_DIFF_PCT, MAX_DIFF_PCT);
                }
            }
            None => {}
        }
    }

    fn handle_scroll_down(&mut self) {
        match self.focus {
            Panel::Branches => {
                if self.branch_index + 1 < self.branch_tree.len() {
                    self.branch_index += 1;
                }
            }
            Panel::Commits => {
                if self.selected_index + 1 < self.filtered_commits.len() {
                    self.selected_index += 1;
                }
            }
            Panel::Diff => {
                self.diff_scroll += 1;
            }
            _ => {}
        }
    }

    fn handle_scroll_up(&mut self) {
        match self.focus {
            Panel::Branches => {
                self.branch_index = self.branch_index.saturating_sub(1);
            }
            Panel::Commits => {
                self.selected_index = self.selected_index.saturating_sub(1);
            }
            Panel::Diff => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    // --- Layout computation ---

    fn compute_areas(&self, full: Rect) -> (Rect, Rect, Rect, Rect, Rect, Rect) {
        let branch_w = Constraint::Percentage(self.branch_width_pct);
        let right_w = Constraint::Percentage(100 - self.branch_width_pct);

        let horizontal = Layout::horizontal([branch_w, right_w]).split(full);
        let branch_area = horizontal[0];
        let right_area = horizontal[1];

        // Inside right_area: [search + scope (1 line), commits, diff, help (3 lines)]
        let _search_row = Constraint::Length(3);
        let _diff_h = Constraint::Percentage(self.diff_height_pct);
        let _help_h = Constraint::Length(3);
        let _commits_h = Constraint::Percentage(100 - self.diff_height_pct - 5); // approx

        // Actually, we need more precise layout
        // Top row: search (left) + scope (small right)
        let top_row = Layout::vertical([Constraint::Length(3)]).split(right_area);
        let _top_area = top_row[0];
        let _remaining = top_row[0]; // We'll use a different approach

        // Better approach: split right_area into top (search/scope), middle (commits), bottom (help)
        let main_split = Layout::vertical([
            Constraint::Length(3),                                 // search/scope row
            Constraint::Percentage(100 - self.diff_height_pct - 5), // commits
            Constraint::Percentage(self.diff_height_pct),           // diff
            Constraint::Length(3),                                 // help
        ])
        .split(right_area);

        let search_scope_area = main_split[0]; // 3 lines
        let table_area = main_split[1];
        let diff_area = main_split[2];
        let _help_area = main_split[3];

        // Split search_scope_area into search (left) and scope (right)
        let search_split = Layout::horizontal([
            Constraint::Percentage(85),
            Constraint::Percentage(15),
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

        // Help bar area — fixed at bottom 3 rows
        let help_area = Rect::new(
            full.x,
            full.y + full.height.saturating_sub(3),
            full.width,
            3.min(full.height),
        );

        // Render each panel
        ui::branch_panel::render(
            frame,
            branch_area,
            &self.branch_tree,
            self.branch_index,
            self.focus == Panel::Branches,
        );

        let branch_label = self
            .selected_branch
            .as_deref()
            .unwrap_or("all branches");
        let title = format!("Git Log — {}", self.repo_path);
        ui::search_panel::render(
            frame,
            search_area,
            &self.search_query,
            self.cursor_pos,
            branch_label,
            &title,
            self.focus == Panel::Search,
        );

        ui::scope_panel::render(
            frame,
            scope_area,
            self.branch_scope,
            self.focus == Panel::Scope,
        );

        ui::commit_table::render(
            frame,
            table_area,
            &self.filtered_commits,
            self.selected_index,
            self.focus == Panel::Commits,
        );

        // Build metadata lines for the diff panel
        let _metadata_lines = self
            .commit_info
            .as_ref()
            .map(|info| ui::diff_panel::build_metadata_lines(info))
            .unwrap_or_default();

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

        // Help bar
        let count_info = ui::help_bar::format_commit_count(
            self.selected_index,
            self.filtered_commits.len(),
            self.all_commits.len(),
        );
        ui::help_bar::render(frame, help_area, self.focus, &count_info);

        // Check if we need to load diff for newly selected commit
        if self.selected_index != self.last_selected_index.unwrap_or(0) {
            self.last_selected_index = Some(self.selected_index);
            if let Some(commit) = self.filtered_commits.get(self.selected_index) {
                let hash = commit.hash.clone();
                if !hash.is_empty() {
                    self.request_diff(&hash);
                }
            }
        }
    }
}

// --- Word boundary helpers ---

fn prev_word_boundary(s: &str, mut pos: usize) -> usize {
    while pos > 0 {
        pos -= 1;
        if s.as_bytes().get(pos).map(|&b| b.is_ascii_alphanumeric() || b == b'_').unwrap_or(false) {
            // At start of a word — go to previous word boundary
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
    // Skip current word
    while pos < len {
        let ch = s.as_bytes()[pos];
        if !ch.is_ascii_alphanumeric() && ch != b'_' {
            break;
        }
        pos += 1;
    }
    // Skip non-word chars
    while pos < len {
        let ch = s.as_bytes()[pos];
        if ch.is_ascii_alphanumeric() || ch == b'_' {
            break;
        }
        pos += 1;
    }
    pos
}
