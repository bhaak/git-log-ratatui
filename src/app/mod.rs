use std::ops::{Deref, DerefMut};
use std::time::Instant;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::TableState,
    Frame,
};

use crate::clipboard;
use crate::error::AppError;
use crate::models::*;
use crate::text_utils;
use crate::tree;
use crate::ui;
use crate::workers::{
    self, BranchCommand, BranchResult, BranchWorker, CommitCommand, CommitResult, CommitWorker,
    DiffCommand, DiffResult, DiffWorker,
};

use state::AppState;

mod state;

const PAGE_SIZE: usize = 10;
/// Number of commits loaded at startup. Keeps first paint fast on large repos.
const INITIAL_COMMIT_LIMIT: usize = 5000;
/// How many additional commits to load when scrolling near the end.
const COMMIT_LIMIT_INCREMENT: usize = 5000;
const POLL_INTERVAL_DEFAULT: u8 = 10;
const POLL_INTERVAL_MAX: u8 = 200;
const POLL_BACKOFF_STEP: u8 = 10;

pub struct App {
    pub state: AppState,
    branch_worker: BranchWorker,
    commit_worker: CommitWorker,
    diff_worker: DiffWorker,
}

impl Deref for App {
    type Target = AppState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for App {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl App {
    pub fn new(repo_path: String, simplified_graph: bool, debug: bool) -> Result<Self, AppError> {
        let branch_worker = workers::new_branch_worker(&repo_path)?;
        let commit_worker = workers::new_commit_worker(&repo_path)?;
        let diff_worker = workers::new_diff_worker(&repo_path)?;

        Ok(App {
            state: AppState::new(repo_path, simplified_graph, debug),
            branch_worker,
            commit_worker,
            diff_worker,
        })
    }

    pub fn run(
        &mut self,
        terminal: &mut ratatui::Terminal<impl ratatui::backend::Backend>,
    ) -> Result<(), AppError> {
        self.request_branches();
        self.request_commits(None);

        loop {
            let frame_start = if self.debug {
                Some(Instant::now())
            } else {
                None
            };
            if self.process_git_results() {
                self.dirty = true;
            }
            if self.dirty {
                let draw_result = terminal.draw(|frame| self.render(frame));
                if let Err(e) = draw_result {
                    return Err(format!("Render error: {}", e).into());
                }
                if let Some(start) = frame_start {
                    self.last_frame_time_ms = start.elapsed().as_millis() as u64;
                }
                self.dirty = false;
            }
            if !self.handle_event()? {
                break;
            }
        }

        Ok(())
    }

    /// Non-interactive profiling mode: load branches, commits, and diff repeatedly, print timings.
    pub fn run_profile(&mut self, iterations: u32) -> Result<(), AppError> {
        let total = Instant::now();
        let mut branch_total = 0u128;
        let mut commit_total = 0u128;
        let mut diff_total = 0u128;

        for i in 0..iterations {
            // Phase 1: Load branches
            let t0 = Instant::now();
            self.request_branches();
            match self.branch_worker.recv() {
                Some(BranchResult::Branches(branches)) => {
                    self.all_branches = branches;
                }
                Some(BranchResult::Error(e)) => return Err(e),
                None => return Err("Branch worker disconnected".into()),
            }
            branch_total += t0.elapsed().as_millis();

            // Phase 2: Load commits with graph
            let t0 = Instant::now();
            self.request_commits(None);
            match self.commit_worker.recv() {
                Some(CommitResult::Commits(commits)) => {
                    self.all_commits = commits;
                }
                Some(CommitResult::Error(e)) => return Err(e),
                None => return Err("Commit worker disconnected".into()),
            }
            commit_total += t0.elapsed().as_millis();

            // Phase 3: Load diff for first commit (if any)
            let first_hash = self.all_commits.first().map(|c| c.hash.clone());
            if let Some(hash) = first_hash {
                let t0 = Instant::now();
                self.request_diff(&hash);
                match self.diff_worker.recv() {
                    Some(DiffResult::Diff {
                        commit_info,
                        diff_lines,
                        file_entries,
                    }) => {
                        self.commit_info = Some(*commit_info);
                        self.diff_lines = diff_lines;
                        self.file_entries = file_entries;
                    }
                    Some(DiffResult::Error(e)) => return Err(e),
                    None => return Err("Diff worker disconnected".into()),
                }
                diff_total += t0.elapsed().as_millis();
            }

            if i == 0 {
                println!("Total branches: {}", self.all_branches.entries.len());
                println!("Total commits:  {}", self.all_commits.len());
            }
        }

        let total_ms = total.elapsed().as_millis();
        let n = iterations as u128;

        println!(
            "Iterations: {iterations} | avg (ms): branches={} commits={} diff={} total={total_ms}",
            branch_total / n,
            commit_total / n,
            diff_total / n
        );

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
        // Clear stale commit data immediately so the old branch's commits are
        // not shown while the new branch's data is loading. Also reset
        // selection to the first commit (tip of the branch) and clear diff
        // data so the old branch's metadata isn't shown.
        self.all_commits.clear();
        self.filtered_commits = None;
        self.visible_to_commit.clear();
        self.selected_index = 0;
        self.commit_info = None;
        self.diff_lines = Vec::new();
        self.file_entries = Vec::new();
        self.last_selected_hash = None;
        self.table_state = TableState::default();
        // Fresh load (startup, branch change, scope change): reset the window.
        self.commit_limit = INITIAL_COMMIT_LIMIT;
        self.all_commits_loaded = false;
        self.loading_more = false;
        self.selected_branch = branch.clone();
        self.status_message = Some("Loading commits...".to_string());
        // Branch / scope changed: the other mode's cache is now stale.
        self.full_commits_cache = None;
        self.simplified_commits_cache = None;
        if !self.commit_worker.send(CommitCommand::FetchCommits {
            branch,
            scope: self.branch_scope,
            limit: Some(self.commit_limit),
            simplified: self.simplified_graph,
        }) {
            self.status_message = Some("Commit worker disconnected — restart required".to_string());
        }
    }

    /// Toggle between full (git-graph) and simplified (git2 revwalk) graphs.
    /// Saves the current commit list to a cache so re-toggling is instant.
    fn toggle_simplified_graph(&mut self) {
        // Save current commits into the cache for the old mode.
        if self.simplified_graph {
            self.simplified_commits_cache = Some(self.all_commits.clone());
        } else {
            self.full_commits_cache = Some(self.all_commits.clone());
        }

        self.simplified_graph = !self.simplified_graph;

        // Restore from cache if the target mode was previously loaded.
        if self.simplified_graph {
            if let Some(cached) = self.simplified_commits_cache.take() {
                self.all_commits = cached;
                self.commits_loaded = true;
                self.apply_search_filter();
                return;
            }
        } else if let Some(cached) = self.full_commits_cache.take() {
            self.all_commits = cached;
            self.commits_loaded = true;
            self.apply_search_filter();
            return;
        }

        // Cache miss: fetch from the worker thread.
        self.request_commits(self.selected_branch.clone());
    }

    /// Raise the commit limit and reload, appending older commits. Triggered when
    /// the selection nears the end of the currently loaded commits.
    fn request_more_commits(&mut self) {
        self.commits_loaded = false;
        self.loading_more = true;
        self.commit_limit = self.commit_limit.saturating_add(COMMIT_LIMIT_INCREMENT);
        if !self.commit_worker.send(CommitCommand::FetchCommits {
            branch: self.selected_branch.clone(),
            scope: self.branch_scope,
            limit: Some(self.commit_limit),
            simplified: self.simplified_graph,
        }) {
            self.status_message = Some("Commit worker disconnected — restart required".to_string());
        }
    }

    /// Re-run the search filter after an incremental load while keeping the
    /// current selection (unlike `apply_search_filter`, which resets it to 0).
    fn reapply_filter_preserving_selection(&mut self) {
        let prev = self.selected_index;
        self.apply_search_filter();
        self.selected_index = prev;
        self.clamp_selection();
    }

    fn request_diff(&mut self, hash: &str) {
        self.diff_pending = true;
        self.diff_worker.send(DiffCommand::FetchDiff {
            hash: hash.to_string(),
        });
    }

    /// Poll all three worker channels for results (non-blocking, parallel streams).
    /// Returns true if any worker produced data that requires a redraw.
    fn process_git_results(&mut self) -> bool {
        let mut changed = false;

        // Poll branch worker
        while let Some(result) = self.branch_worker.try_recv() {
            changed = true;
            match result {
                BranchResult::Branches(branches) => {
                    self.all_branches = branches;
                    self.rebuild_branch_tree();
                    self.branches_loaded = true;
                }
                BranchResult::Error(err) => {
                    self.status_message = Some(err.to_string());
                }
            }
        }

        // Poll commit worker
        while let Some(result) = self.commit_worker.try_recv() {
            changed = true;
            match result {
                CommitResult::Commits(commits) => {
                    let prev_len = self.all_commits.len();
                    self.all_commits = commits;
                    if self.loading_more {
                        self.loading_more = false;
                        if self.all_commits.len() <= prev_len {
                            self.all_commits_loaded = true;
                        }
                        self.reapply_filter_preserving_selection();
                    } else {
                        self.apply_search_filter();
                    }
                    self.commits_loaded = true;
                    self.status_message = None;
                    if self.simplified_graph {
                        self.simplified_commits_cache = Some(self.all_commits.clone());
                    } else {
                        self.full_commits_cache = Some(self.all_commits.clone());
                    }
                    if !self
                        .filtered_commits
                        .as_deref()
                        .unwrap_or(&self.all_commits)
                        .is_empty()
                        && self.commit_info.is_none()
                    {
                        let hash = self
                            .filtered_commits
                            .as_deref()
                            .unwrap_or(&self.all_commits)[0]
                            .hash
                            .clone();
                        if !hash.is_empty() {
                            self.request_diff(&hash);
                        }
                    }
                }
                CommitResult::Error(err) => {
                    self.status_message = Some(err.to_string());
                }
            }
        }

        // Poll diff worker
        while let Some(result) = self.diff_worker.try_recv() {
            changed = true;
            match result {
                DiffResult::Diff {
                    commit_info,
                    diff_lines,
                    file_entries,
                } => {
                    self.commit_info = Some(*commit_info);
                    self.diff_lines = diff_lines;
                    self.file_entries = file_entries;
                    self.diff_scroll = 0;
                    self.selected_file_index = 0;
                    self.diff_pending = false;
                }
                DiffResult::Error(err) => {
                    self.status_message = Some(err.to_string());
                }
            }
        }

        changed
    }

    // --- Branch tree ---

    fn rebuild_branch_tree(&mut self) {
        let local_section_key = "__local__";
        let remote_section_key = "__remote__";

        let local_expanded = self
            .expanded_nodes
            .get(local_section_key)
            .copied()
            .unwrap_or(true);
        let remote_expanded = self
            .expanded_nodes
            .get(remote_section_key)
            .copied()
            .unwrap_or(true);

        let local_item = TreeItem {
            name: "Local Branches".to_string(),
            depth: 0,
            expandable: true,
            expanded: local_expanded,
            is_branch: false,
            full_path: String::new(),
            tree_prefix: String::new(),
            key: local_section_key.to_string(),
        };

        let remote_item = TreeItem {
            name: "Remote Branches".to_string(),
            depth: 0,
            expandable: true,
            expanded: remote_expanded,
            is_branch: false,
            full_path: String::new(),
            tree_prefix: String::new(),
            key: remote_section_key.to_string(),
        };

        // Separate local and remote branch names
        let local_names: Vec<String> = self
            .all_branches
            .entries
            .iter()
            .filter(|e| !e.is_remote)
            .map(|e| e.name.clone())
            .collect();

        let remote_names: Vec<String> = self
            .all_branches
            .entries
            .iter()
            .filter(|e| e.is_remote)
            .map(|e| e.name.clone())
            .collect();

        let mut items = vec![local_item];

        // Build and sort local tree
        let mut local_root = tree::build_branch_tree(&local_names);
        tree::sort_tree(&mut local_root);

        // Move default branch to the front of local children
        if let Some(ref default) = self.all_branches.default_branch {
            if let Some(pos) = local_root.children.iter().position(|c| c.name == *default) {
                let default_child = local_root.children.remove(pos);
                local_root.children.insert(0, default_child);
            }
        }

        if local_expanded {
            let branch_items = tree::flatten_tree(&local_root, 0, &self.expanded_nodes);
            items.extend(branch_items);
        }

        items.push(remote_item);

        // Build and sort remote tree
        let mut remote_root = tree::build_branch_tree(&remote_names);
        tree::sort_tree(&mut remote_root);

        if remote_expanded {
            let branch_items = tree::flatten_tree(&remote_root, 0, &self.expanded_nodes);
            items.extend(branch_items);
        }

        self.branch_tree = items;
    }

    // --- Search ---

    fn apply_search_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_commits = None;
        } else {
            let q = self.search_query.to_lowercase();
            self.filtered_commits = Some(
                self.all_commits
                    .iter()
                    .filter(|c| {
                        c.graph_only
                            || c.hash.to_lowercase().contains(&q)
                            || c.author.to_lowercase().contains(&q)
                            || c.date.to_lowercase().contains(&q)
                            || c.subject.to_lowercase().contains(&q)
                    })
                    .cloned()
                    .collect(),
            );
        }
        self.build_visible_mapping();
        self.selected_index = 0;
        self.clamp_selection();
    }

    fn build_visible_mapping(&mut self) {
        let commits = self
            .filtered_commits
            .as_deref()
            .unwrap_or(&self.all_commits);
        self.visible_to_commit = (0..commits.len())
            .filter(|&i| !commits[i].graph_only)
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
            self.selected_index = self
                .selected_index
                .min(self.visible_count().saturating_sub(1));
        } else {
            self.selected_index = 0;
        }
    }

    // --- Events ---

    fn handle_event(&mut self) -> Result<bool, AppError> {
        let interval = std::time::Duration::from_millis(self.poll_interval_ms as u64);
        if !event::poll(interval)? {
            // No event received, back off slowly.
            self.poll_interval_ms =
                (self.poll_interval_ms + POLL_BACKOFF_STEP).min(POLL_INTERVAL_MAX);
            return Ok(true);
        }

        // Event received, reset polling interval.
        self.poll_interval_ms = POLL_INTERVAL_DEFAULT;
        let ev = event::read()?;

        match ev {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                self.dirty = true;
                self.handle_key(key)
            }
            Event::Mouse(mouse) => {
                // Ignore move events — they don't change anything visible.
                if let MouseEventKind::Moved = mouse.kind {
                    return Ok(true);
                }
                self.dirty = true;
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
                self.dirty = true;
                self.last_size = Some((w, h));
                Ok(true)
            }
            _ => Ok(true),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool, AppError> {
        // Quit
        match key.code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(false)
            }
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
                    if let Some(c) = self
                        .filtered_commits
                        .as_deref()
                        .unwrap_or(&self.all_commits)
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
                return Ok(true);
            }
            KeyCode::Char('Y') => {
                if self.visible_count() > 0 {
                    let ci = self.visible_to_filtered(self.selected_index);
                    if let Some(c) = self
                        .filtered_commits
                        .as_deref()
                        .unwrap_or(&self.all_commits)
                        .get(ci)
                    {
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
            // Toggle simplified graph (colored bullets, no box-drawing lines)
            KeyCode::Char('g') if key.modifiers.is_empty() && self.focus == Panel::Commits => {
                self.toggle_simplified_graph();
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
            Panel::Commits if self.visible_count() > 0 => {
                self.selected_index = (self.selected_index + 1) % self.visible_count();
            }
            Panel::Diff if !self.file_entries.is_empty() => {
                self.selected_file_index = (self.selected_file_index + 1) % self.file_entries.len();
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
            Panel::Commits if self.visible_count() > 0 => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                } else {
                    self.selected_index = self.visible_count() - 1;
                }
            }
            Panel::Diff if !self.file_entries.is_empty() => {
                if self.selected_file_index > 0 {
                    self.selected_file_index -= 1;
                } else {
                    self.selected_file_index = self.file_entries.len() - 1;
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

    fn handle_branch_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                self.branch_index = self.branch_index.saturating_sub(1);
            }
            KeyCode::Down if self.branch_index + 1 < self.branch_tree.len() => {
                self.branch_index += 1;
            }
            KeyCode::Right => {
                let action = self
                    .branch_tree
                    .get(self.branch_index)
                    .filter(|item| item.expandable && !item.expanded)
                    .map(|item| item.key.clone());
                if let Some(key) = action {
                    self.expanded_nodes.insert(key, true);
                    self.rebuild_branch_tree();
                }
            }
            KeyCode::Left => {
                let action = self
                    .branch_tree
                    .get(self.branch_index)
                    .filter(|item| item.expandable && item.expanded)
                    .map(|item| item.key.clone());
                if let Some(key) = action {
                    self.expanded_nodes.insert(key, false);
                    self.rebuild_branch_tree();
                }
            }
            KeyCode::Char(' ') => {
                let action = self
                    .branch_tree
                    .get(self.branch_index)
                    .filter(|item| item.expandable)
                    .map(|item| (item.key.clone(), !item.expanded));
                if let Some((key, new_state)) = action {
                    self.expanded_nodes.insert(key, new_state);
                    self.rebuild_branch_tree();
                }
            }
            KeyCode::Enter => {
                let action = self.branch_tree.get(self.branch_index).map(|item| {
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
                        self.request_commits(Some(path));
                        self.focus = Panel::Commits;
                    } else if let Some((key, new_state)) = toggle {
                        self.expanded_nodes.insert(key, new_state);
                        self.rebuild_branch_tree();
                    }
                }
            }
            KeyCode::PageUp => {
                self.branch_index = self.branch_index.saturating_sub(PAGE_SIZE);
            }
            KeyCode::PageDown => {
                self.branch_index =
                    (self.branch_index + PAGE_SIZE).min(self.branch_tree.len().saturating_sub(1));
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
            KeyCode::Backspace if self.cursor_pos > 0 => {
                let prev = text_utils::prev_char_boundary(&self.search_query, self.cursor_pos);
                self.search_query.remove(prev);
                self.cursor_pos = prev;
                self.apply_search_filter();
            }
            KeyCode::Delete if self.cursor_pos < self.search_query.len() => {
                let pos = self.cursor_pos;
                self.search_query.remove(pos);
                self.apply_search_filter();
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.cursor_pos =
                        text_utils::prev_word_boundary(&self.search_query, self.cursor_pos);
                } else {
                    self.cursor_pos =
                        text_utils::prev_char_boundary(&self.search_query, self.cursor_pos);
                }
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.cursor_pos =
                        text_utils::next_word_boundary(&self.search_query, self.cursor_pos);
                } else {
                    self.cursor_pos =
                        text_utils::next_char_boundary(&self.search_query, self.cursor_pos);
                }
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
            }
            KeyCode::End => {
                self.cursor_pos = self.search_query.len();
            }
            KeyCode::Char(ch) => {
                let pos = self.cursor_pos;
                self.search_query.insert(pos, ch);
                self.cursor_pos += ch.len_utf8();
                self.apply_search_filter();
            }
            _ => {}
        }
    }

    fn handle_commit_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up if self.visible_count() > 0 => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                } else {
                    self.selected_index = self.visible_count() - 1;
                }
            }
            KeyCode::Down if self.visible_count() > 0 => {
                self.selected_index = (self.selected_index + 1) % self.visible_count();
            }
            KeyCode::Enter => {
                self.focus = Panel::Diff;
            }
            KeyCode::PageUp => {
                self.selected_index = self.selected_index.saturating_sub(PAGE_SIZE);
                self.clamp_selection();
            }
            KeyCode::PageDown if self.visible_count() > 0 => {
                self.selected_index =
                    (self.selected_index + PAGE_SIZE).min(self.visible_count().saturating_sub(1));
            }
            _ => {}
        }
    }

    fn handle_diff_keys(&mut self, key: KeyEvent) {
        // Determine if we're scrolled past the metadata+file section
        let file_section_end =
            ui::diff_panel::diff_line_offset(self.commit_info.as_ref(), &self.file_entries);
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
                    self.selected_file_index =
                        (self.selected_file_index + 1) % self.file_entries.len();
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
            KeyCode::Char('n') if !self.file_entries.is_empty() => {
                self.selected_file_index = (self.selected_file_index + 1) % self.file_entries.len();
                let offset =
                    ui::diff_panel::diff_line_offset(self.commit_info.as_ref(), &self.file_entries);
                if let Some(entry) = self.file_entries.get(self.selected_file_index) {
                    self.diff_scroll = entry.diff_line + offset;
                }
            }
            KeyCode::Char('p') if !self.file_entries.is_empty() => {
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
            KeyCode::Home => {
                self.diff_scroll = 0;
            }
            KeyCode::End => {
                // scroll to end
                self.diff_scroll = usize::MAX;
            }
            KeyCode::PageUp => {
                let page = self.diff_scrollbar.viewport_length().max(1);
                self.diff_scroll = self.diff_scroll.saturating_sub(page);
            }
            KeyCode::PageDown => {
                let page = self.diff_scrollbar.viewport_length().max(1);
                self.diff_scroll = self.diff_scroll.saturating_add(page);
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
                self.scrollbar_drag = None;
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
        let Some((tw, th)) = self.last_size else {
            return;
        };
        let full = Rect::new(0, 0, tw, th);
        let areas = ui::layout::compute_areas(full, self.branch_width_pct, self.diff_height_pct);
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
        if let Some(_new_pos) = self
            .branch_scrollbar
            .click_to_index(branch_visible_area, click_pos)
        {
            self.dragging = None;
            self.focus = Panel::Branches;
            self.scrollbar_drag = Some(Panel::Branches);
            return;
        }
        if let Some(_new_pos) = self.table_scrollbar.click_to_index(areas.table, click_pos) {
            self.dragging = None;
            self.focus = Panel::Commits;
            self.scrollbar_drag = Some(Panel::Commits);
            return;
        }
        if let Some(new_pos) = self.diff_scrollbar.click_to_index(areas.diff, click_pos) {
            self.dragging = None;
            self.focus = Panel::Diff;
            self.scrollbar_drag = Some(Panel::Diff);
            self.diff_scroll = new_pos;
            return;
        }

        if ui::layout::rect_contains_interior(&branch_visible_area, click_pos) {
            self.focus = Panel::Branches;
            let rel_row = (row
                .saturating_sub(areas.branch.y)
                .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
            let actual_index = rel_row + self.branch_list_offset;
            if actual_index < self.branch_tree.len() {
                self.branch_index = actual_index;
                let action = self.branch_tree.get(actual_index).map(|item| {
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
                        self.request_commits(Some(path));
                        self.focus = Panel::Commits;
                    } else if let Some((key, new_state)) = toggle {
                        self.expanded_nodes.insert(key, new_state);
                        self.rebuild_branch_tree();
                    }
                }
            }
        } else if ui::layout::rect_contains(&areas.scope, click_pos) {
            self.focus = Panel::Scope;
            self.cycle_scope();
        } else if ui::layout::rect_contains(&areas.search, click_pos) {
            self.focus = Panel::Search;
        } else if ui::layout::rect_contains_interior(&areas.table, click_pos) {
            self.focus = Panel::Commits;
            let rel_row = (row.saturating_sub(areas.table.y).saturating_sub(
                ui::layout::TABLE_OVERHEAD.saturating_sub(ui::layout::BORDER_OVERHEAD),
            )) as usize;
            let filtered_idx = rel_row + self.table_state.offset();
            if let Some(vis_idx) = self.filtered_to_visible(filtered_idx) {
                self.selected_index = vis_idx;
            }
        } else if ui::layout::rect_contains(&areas.diff, click_pos) {
            self.focus = Panel::Diff;
            let rel_row = (row
                .saturating_sub(areas.diff.y)
                .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
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
        let Some((tw, th)) = self.last_size else {
            return;
        };
        let full = Rect::new(0, 0, tw, th);
        let areas = ui::layout::compute_areas(full, self.branch_width_pct, self.diff_height_pct);

        if ui::layout::is_on_vertical_border(col, row, areas.branch) {
            self.dragging = Some(ui::layout::DragDirection::Vertical);
            return;
        }

        if ui::layout::is_on_horizontal_border(col, row, areas.right, areas.table) {
            self.dragging = Some(ui::layout::DragDirection::Horizontal);
        }
    }

    fn handle_mouse_drag(&mut self, col: u16, row: u16) {
        // Scrollbar dragging — update scroll position proportionally
        if let Some(panel) = self.scrollbar_drag {
            let Some((tw, th)) = self.last_size else {
                return;
            };
            let full = Rect::new(0, 0, tw, th);
            let areas =
                ui::layout::compute_areas(full, self.branch_width_pct, self.diff_height_pct);
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
                    let _ = self
                        .branch_scrollbar
                        .click_to_index(branch_visible_area, (col, row));
                }
                Panel::Commits => {
                    // Scrollbar drag scrolls but does not change the selected item.
                    let _ = self.table_scrollbar.click_to_index(areas.table, (col, row));
                }
                Panel::Diff => {
                    if let Some(new_pos) =
                        self.diff_scrollbar.click_to_index(areas.diff, (col, row))
                    {
                        self.diff_scroll = new_pos;
                    }
                }
                _ => {}
            }
            return;
        }

        match self.dragging {
            Some(ui::layout::DragDirection::Vertical) => {
                if let Some((tw, _)) = self.last_size {
                    self.branch_width_pct = ui::layout::vertical_resize_pct(col, tw);
                }
            }
            Some(ui::layout::DragDirection::Horizontal) => {
                if let Some((_, th)) = self.last_size {
                    self.diff_height_pct = ui::layout::horizontal_resize_pct(row, th);
                }
            }
            None => {}
        }
    }

    /// Scroll the panel under the mouse cursor (position-aware).
    fn handle_scroll_at(&mut self, col: u16, row: u16, direction: i32) {
        let Some((tw, th)) = self.last_size else {
            return;
        };
        let full = Rect::new(0, 0, tw, th);
        let areas = ui::layout::compute_areas(full, self.branch_width_pct, self.diff_height_pct);
        let pos = (col, row);

        if ui::layout::rect_contains(&areas.branch, pos) {
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
        } else if ui::layout::rect_contains(&areas.table, pos) {
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
        } else if ui::layout::rect_contains(&areas.diff, pos) {
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
            Panel::Commits if self.visible_count() > 0 => {
                self.selected_index = (self.selected_index + 1) % self.visible_count();
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
            Panel::Commits if self.visible_count() > 0 => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                } else {
                    self.selected_index = self.visible_count() - 1;
                }
            }
            Panel::Diff => {
                self.diff_scroll = self.diff_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    // --- Rendering ---

    fn render(&mut self, frame: &mut Frame) {
        let full = frame.area();
        self.last_size = Some((full.width, full.height));

        // Guard against zero-size terminal (can happen during resize)
        if full.width < ui::layout::MIN_TERM_WIDTH || full.height < ui::layout::MIN_TERM_HEIGHT {
            return;
        }

        let debug_label = if self.debug {
            Some(format!("{}ms", self.last_frame_time_ms))
        } else {
            None
        };
        let debug_label = debug_label.as_deref();

        let areas = ui::layout::compute_areas(full, self.branch_width_pct, self.diff_height_pct);

        let help_area = Rect::new(
            full.x,
            full.y + full.height.saturating_sub(ui::layout::HELP_BAR_HEIGHT),
            full.width,
            ui::layout::HELP_BAR_HEIGHT.min(full.height),
        );

        // Clamp cursor
        self.cursor_pos = self.cursor_pos.min(self.search_query.len());
        self.clamp_selection();

        // Incrementally load older commits when the selection nears the end of
        // what's currently loaded. Skipped while a search filter is active (the
        // filtered view isn't a reliable proxy for the loaded window) and while a
        // load is already in flight (commits_loaded == false).
        if !self.all_commits_loaded && self.commits_loaded && self.filtered_commits.is_none() {
            let loaded = self.visible_count();
            if loaded > 0 && self.selected_index + PAGE_SIZE >= loaded {
                self.request_more_commits();
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
            &self.branch_tree,
            self.branch_index,
            self.focus == Panel::Branches,
            debug_label,
        );

        self.branch_list_offset = branch_list_state.offset();

        let branch_focus_style = if self.focus == Panel::Branches {
            Style::default().fg(Color::Rgb(180, 140, 255))
        } else {
            Style::default().fg(Color::Gray)
        };
        let branch_visible = (branch_content_area
            .height
            .saturating_sub(ui::layout::PANEL_BORDER_H)) as usize;
        let branch_tree_len = self.branch_tree.len();
        let branch_offset = branch_list_state.offset();
        self.branch_scrollbar.render(
            frame,
            branch_scrollbar_area,
            branch_tree_len,
            branch_visible,
            branch_offset,
            branch_focus_style,
        );

        // --- Search panel ---

        let branch_label = self.selected_branch.as_deref().unwrap_or("all branches");
        let title = format!("Git Log - {} [{}]", self.repo_path, branch_label);
        ui::search_panel::render(
            frame,
            areas.search,
            &self.search_query,
            self.cursor_pos,
            branch_label,
            &title,
            self.focus == Panel::Search,
            debug_label,
        );

        ui::scope_panel::render(
            frame,
            areas.scope,
            self.branch_scope,
            self.focus == Panel::Scope,
            debug_label,
        );

        // --- Commit table (content + scrollbar) ---
        let (table_content_area, table_scrollbar_area) =
            ui::scrollbar_view::ScrollbarView::split(areas.table);

        let state = &mut self.state;
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

        let table_focus_style = if self.focus == Panel::Commits {
            Style::default().fg(Color::Rgb(180, 140, 255))
        } else {
            Style::default().fg(Color::Gray)
        };
        let table_visible = (table_content_area
            .height
            .saturating_sub(ui::layout::TABLE_OVERHEAD)) as usize;
        let table_item_count = self
            .filtered_commits
            .as_deref()
            .unwrap_or(&self.all_commits)
            .len();
        let table_offset = self.table_state.offset();
        self.table_scrollbar.render(
            frame,
            table_scrollbar_area,
            table_item_count,
            table_visible,
            table_offset,
            table_focus_style,
        );

        let short_hash = self.commit_info.as_ref().map(|info| {
            &info.hash[..std::cmp::min(ui::commit_table::SHORT_HASH_LEN, info.hash.len())]
        });

        // --- Diff panel (content + scrollbar) ---
        let (diff_content_area, diff_scrollbar_area) =
            ui::scrollbar_view::ScrollbarView::split(areas.diff);

        let diff_ctx = ui::diff_panel::DiffPanelCtx {
            commit_info: self.commit_info.as_ref(),
            diff_lines: &self.diff_lines,
            file_entries: &self.file_entries,
            selected_file_index: self.selected_file_index,
            diff_scroll: self.diff_scroll,
            is_focused: self.focus == Panel::Diff,
            short_hash,
            debug_label,
        };
        let diff_total_lines = ui::diff_panel::render(frame, diff_content_area, &diff_ctx);

        let diff_focus_style = if self.focus == Panel::Diff {
            Style::default().fg(Color::Rgb(180, 140, 255))
        } else {
            Style::default().fg(Color::Gray)
        };
        let diff_visible = (diff_content_area
            .height
            .saturating_sub(ui::layout::PANEL_BORDER_H)) as usize;
        let diff_scroll_val = self.diff_scroll;
        self.diff_scrollbar.render(
            frame,
            diff_scrollbar_area,
            diff_total_lines,
            diff_visible,
            diff_scroll_val,
            diff_focus_style,
        );

        ui::help_bar::render(frame, help_area, self.focus, self.status_message.as_deref());

        // Trigger diff load on selection change
        let current_hash = if self.visible_count() > 0 {
            let ci = self.visible_to_filtered(self.selected_index);
            self.filtered_commits
                .as_deref()
                .unwrap_or(&self.all_commits)
                .get(ci)
                .map(|c| c.hash.clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a minimal App for testing pure logic functions.
    fn test_app() -> App {
        App {
            state: AppState::new(".".to_string(), false, false),
            branch_worker: workers::new_branch_worker(".").unwrap(),
            commit_worker: workers::new_commit_worker(".").unwrap(),
            diff_worker: workers::new_diff_worker(".").unwrap(),
        }
    }

    #[test]
    fn test_search_filter_empty_query() {
        let mut app = test_app();
        app.all_commits = vec![Commit {
            hash: "abc".into(),
            author: "alice".into(),
            date: "2024-01-01".into(),
            subject: "fix bug".into(),
            graph: "*".into(),
            graph_colors: vec![],
            merge: false,
            graph_only: false,
            decorations: vec![],
            deco_line: 0,
        }];
        app.search_query.clear();
        app.apply_search_filter();
        assert_eq!(
            app.filtered_commits
                .as_deref()
                .unwrap_or(&app.all_commits)
                .len(),
            1
        );
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
                graph_colors: vec![],
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
                graph_colors: vec![],
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ];
        app.search_query = "bug".into();
        app.apply_search_filter();
        assert_eq!(app.filtered_commits.as_deref().unwrap().len(), 1);
        assert_eq!(app.filtered_commits.as_deref().unwrap()[0].hash, "abc");
    }

    #[test]
    fn test_search_filter_case_insensitive() {
        let mut app = test_app();
        app.all_commits = vec![Commit {
            hash: "abc".into(),
            author: "ALICE".into(),
            date: "2024-01-01".into(),
            subject: "Fix Bug".into(),
            graph: "*".into(),
            graph_colors: vec![],
            merge: false,
            graph_only: false,
            decorations: vec![],
            deco_line: 0,
        }];
        app.search_query = "bug".into();
        app.apply_search_filter();
        assert_eq!(app.filtered_commits.as_deref().unwrap().len(), 1);
    }

    #[test]
    fn test_visible_mapping_skips_graph_only() {
        let mut app = test_app();
        app.filtered_commits = Some(vec![
            Commit {
                hash: "abc".into(),
                author: "a".into(),
                date: "d".into(),
                subject: "s".into(),
                graph: "*".into(),
                graph_colors: vec![],
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
                graph_colors: vec![],
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
                graph_colors: vec![],
                merge: false,
                graph_only: false,
                decorations: vec![],
                deco_line: 0,
            },
        ]);
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
        app.filtered_commits = Some(vec![Commit {
            hash: "abc".into(),
            author: "a".into(),
            date: "d".into(),
            subject: "s".into(),
            graph: "*".into(),
            graph_colors: vec![],
            merge: false,
            graph_only: false,
            decorations: vec![],
            deco_line: 0,
        }]);
        app.build_visible_mapping();
        app.selected_index = 0;
        app.clamp_selection();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_branch_click_with_scroll_offset() {
        let mut app = test_app();

        // Simulate a scrolled branch tree — the list widget offset is 20,
        // meaning 20 items are scrolled off the top.
        app.branch_tree = (0..50)
            .map(|i| TreeItem {
                name: format!("branch_{}", i),
                key: format!("branch_{}", i),
                depth: 0,
                is_branch: true,
                full_path: format!("branch_{}", i),
                expanded: false,
                expandable: false,
                tree_prefix: String::new(),
            })
            .collect();
        app.branch_list_offset = 20;

        // Simulate a click at the 5th visible row (terminal row = panel_y + BORDER_OVERHEAD + 5).
        // Panel is at y=0, BORDER_OVERHEAD=1, so clicking terminal row 6 should give
        // rel_row=5, and actual_index = 5 + 20 = 25.
        let panel_y = 0u16;
        let click_row = 6u16; // panel_y=0, BORDER_OVERHEAD=1 → rel_row = 6-0-1 = 5
        let rel_row = (click_row
            .saturating_sub(panel_y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.branch_list_offset;

        assert_eq!(rel_row, 5);
        assert_eq!(actual_index, 25);
        assert!(actual_index < app.branch_tree.len());
        assert_eq!(app.branch_tree[actual_index].full_path, "branch_25");
    }

    #[test]
    fn test_branch_click_without_scroll() {
        let mut app = test_app();

        app.branch_tree = (0..10)
            .map(|i| TreeItem {
                name: format!("branch_{}", i),
                key: format!("branch_{}", i),
                depth: 0,
                is_branch: true,
                full_path: format!("branch_{}", i),
                expanded: false,
                expandable: false,
                tree_prefix: String::new(),
            })
            .collect();
        app.branch_list_offset = 0;

        let panel_y = 0u16;
        let click_row = 3u16; // panel_y=0, BORDER_OVERHEAD=1 → rel_row = 3-0-1 = 2
        let rel_row = (click_row
            .saturating_sub(panel_y)
            .saturating_sub(ui::layout::BORDER_OVERHEAD)) as usize;
        let actual_index = rel_row + app.branch_list_offset;

        assert_eq!(rel_row, 2);
        assert_eq!(actual_index, 2);
        assert_eq!(app.branch_tree[actual_index].full_path, "branch_2");
    }

    #[test]
    fn test_app_new_returns_ok() {
        let result = App::new(".".to_string(), false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_scrollbar_click_does_not_change_selection() {
        let mut app = test_app();

        // Use a 100-wide terminal. With DEFAULT_BRANCH_PCT=20, branch gets 20 cols.
        // Scrollbar is at col=19 (rightmost column of branch area).
        app.last_size = Some((100, 30));
        app.branch_tree = (0..50)
            .map(|i| TreeItem {
                name: format!("branch_{}", i),
                key: format!("branch_{}", i),
                depth: 0,
                is_branch: true,
                full_path: format!("branch_{}", i),
                expanded: false,
                expandable: false,
                tree_prefix: String::new(),
            })
            .collect();

        // Simulate a prior render that set up the scrollbar state so
        // click_to_index returns Some for scrollbar clicks.
        {
            use ratatui::{backend::TestBackend, Terminal};
            let mut terminal = Terminal::new(TestBackend::new(2, 10)).unwrap();
            terminal
                .draw(|f| {
                    app.branch_scrollbar.render(
                        f,
                        Rect::new(1, 0, 1, 10),
                        50,
                        5,
                        0,
                        Style::default(),
                    );
                })
                .unwrap();
        }

        let prev_branch_index = app.branch_index;

        // Click on the branch panel scrollbar column (rightmost of 20-col area = col 19)
        app.handle_mouse_click(19, 5);

        assert_eq!(
            app.branch_index, prev_branch_index,
            "branch_index should NOT change on scrollbar click"
        );

        // Click on content area (col=5, row=3) should change branch_index
        app.handle_mouse_click(5, 3);
        // BORDER_OVERHEAD=1, branch area y=0 → rel_row = 3 - 0 - 1 = 2
        assert_eq!(
            app.branch_index, 2,
            "branch_index should change on content click"
        );
    }
}
