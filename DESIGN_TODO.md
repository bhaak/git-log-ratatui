# Architecture Refactoring Plan

Based on full codebase analysis (2026-07-06), updated 2026-07-07 with architectural review.
Current: 44 `.rs` files, 17 top-level modules.

Legend:
  [P] = parallelizable (no dependency on other tasks)
  [H] = high priority
  [M] = medium priority
  [L] = low priority
  |->  = depends on
  [DONE] = completed

---

## Phase 1: Data Ownership & Encapsulation [H]

### A1 — Split AppState into focused state structs [H] **[DONE]**
- Problem: `src/app/state.rs` — 43 public fields, zero encapsulation.
  Any module can mutate anything. No invariants guaranteed.
- Create `src/state/` module directory:
  - `src/state/mod.rs` — re-exports all sub-states
  - `src/state/branch.rs` — `BranchState { tree, index, scope, selected_branch, expanded_nodes, all_branches, list_offset }`
  - `src/state/commit.rs` — `CommitTableState { all_commits, filtered_commits, selected_index, visible_to_commit, table_state, simplified_graph, full_cache, simplified_cache }`
  - `src/state/diff.rs` — `DiffState { commit_info, diff_lines, file_entries, selected_file_index, diff_scroll, last_selected_hash }`
  - `src/state/search.rs` — `SearchState { query, cursor_pos }`
  - `src/state/ui.rs` — `UiState { focus, branch_width_pct, diff_height_pct, dragging, scrollbar_drag, last_size, last_mouse_pos, branch_scrollbar, table_scrollbar, diff_scrollbar, dirty }`
  - `src/state/loading.rs` — `LoadingState { branches_loaded, commits_loaded, diff_pending, commit_limit, all_commits_loaded, loading_more, status_message, poll_interval_ms }`
- Each state struct gets methods enforcing invariants (e.g. `CommitTableState::select()` clamps internally).
- `AppState` becomes a thin container of these 6 structs + `repo_path`, `debug`, `theme`.
- `Deref` / `DerefMut` on `App` is **removed**.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: split AppState into 6 focused state structs, remove Deref"

### A2 — Remove Deref anti-pattern from App [H] **[DONE]**
- |-> A1
- Problem: `App` derefs to `AppState`, so every function taking `&App` has unlimited write access.
- Delete `impl Deref for App` and `impl DerefMut for App`.
- Add explicit accessor methods on `App`:
  - `app.state()` / `app.state_mut()` (used only by event loop)
  - `app.request_commits()` etc. remain as direct methods
- All call sites that used `app.some_field` or `app.state.some_field` now use `app.state_mut().some_field`.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: remove Deref/DerefMut from App, use explicit accessors"

---

## Phase 2: Panel Architecture [H]

### B1 — Redesign Panel trait with proper State ownership [H]
- |-> A1
- Problem: `Panel` trait exists but is only used for event dispatch.
  Rendering uses standalone functions that access all of `AppState`.
  Panel trait objects hold no state.
- Redesign:
  ```rust
  trait Panel {
      type State;
      fn render(&self, area: Rect, frame: &mut Frame, state: &Self::State, ctx: &RenderCtx);
      fn handle_event(&mut self, event: &KeyEvent, state: &mut Self::State) -> Vec<Command>;
      fn help_keys(&self) -> &[(&str, &str)];
      fn label(&self) -> &str;
  }
  ```
- `RenderCtx` holds: `focus: PanelId`, `debug_label: Option<&str>`, `theme: &Theme`.
- Each panel only receives its own `Self::State`, not the entire `AppState`.
- `Command` enum: `SelectBranch(String)`, `Search(String)`, `ToggleScope`, `ToggleGraph`, `CopyHash`, `PasteSearch(String)`, `ScrollDiff(i32)`, `SelectFile(usize)`, `Quit`.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: redesign Panel trait with typed State and Command pattern"

### B2 — Move rendering logic into panel modules [M]
- |-> B1
- Problem: `src/app/render.rs` (236 lines) directly accesses all `AppState` fields.
- `render.rs` becomes a thin orchestrator: compute layout, then call each panel's `render()`.
- Panel-specific rendering moves into the panel modules:
  - Branch rendering → `ui/branch_panel.rs`
  - Commit table rendering → `ui/commit_table.rs`
  - Diff rendering → `ui/diff_panel.rs`
  - Search rendering → `ui/search_panel.rs`
  - Scope rendering → `ui/scope_panel.rs`
- Each panel module becomes self-contained: rendering + event handling + help keys.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: move rendering logic into panel modules"

### B3 — Command-based input handling [M]
- |-> B2
- Problem: `src/app/input.rs` (627 lines) directly mutates `AppState` while interpreting events.
- Two-phase dispatch:
  1. `handle_event()` maps `Event` → `Vec<Command>` (pure, no mutation)
  2. `execute_commands()` applies `Vec<Command>` to state (all mutation happens here)
- `input.rs` splits:
  - `src/app/input.rs` — `handle_event()`, `handle_key()`, `handle_mouse()` → produce `Command`s
  - `src/app/commands.rs` — `execute(app, commands)` → apply to state
- Global keys (quit, focus cycle, scope cycle, clipboard) produce commands like any other key.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: introduce Command pattern for input-to-state decoupling"

---

## Phase 3: Worker & Event Loop [M]

### C1 — Generic WorkerPool abstraction [M]
- |-> A1
- Problem: `process_git_results()` has hard-coded match arms for all 3 worker types.
  Adding a 4th worker requires touching `App`.
- Create `trait Worker: Send`:
  ```rust
  trait Worker {
      type Result;
      fn try_recv(&self) -> Option<Self::Result>;
      fn handle_result(&self, result: Self::Result, state: &mut AppState) -> bool; // true = dirty
  }
  ```
- `WorkerPool` holds `Vec<Box<dyn AnyWorker>>` and polls all workers uniformly.
- `process_git_results()` becomes:
  ```rust
  for worker in &self.workers {
      while let Some(result) = worker.try_recv() {
          changed |= worker.handle_result(result, &mut self.state);
      }
  }
  ```
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: introduce generic Worker trait and WorkerPool"

### C2 — Swap mpsc for crossbeam or flume [L]
- |-> C1
- Problem: `std::sync::mpsc::Receiver::try_recv()` is called 3x per frame sequentially.
  No true multiplexing.
- Consider `crossbeam-channel::Select` to wait on all 3 channels simultaneously.
- Or use `flume` for better ergonomics with async-compatible receivers.
- Evaluate: is the current polling-with-backoff actually a problem? Only change if beneficial.
- Commit: "perf: use crossbeam select for worker multiplexing" OR skip if not needed.

---

## Phase 4: Theme & Config [M]

### D1 — Eliminate Theme clone in render hot path [M] **[DONE]**
- |-> A1
- Problem: `src/app/render.rs:21` clones a 40-field `Theme` struct every frame.
- Change `AppState.theme: Theme` to `AppState.theme: Arc<Theme>`.
- `RenderCtx` (from B1) takes `&Theme`.
- When theme is reloaded from config, swap the `Arc` atomically.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "perf: use Arc<Theme> to avoid per-frame clone"

### D2 — Make theme fully configurable via TOML [L] **[DONE]**
- |-> D1
- Problem: Theme is hardcoded in `src/theme.rs:69-112`. Config supports layout/behavior but not theme.
- Add `[theme]` section to config TOML with all color fields.
- Derive `Deserialize` for `Theme`.
- `Config::load()` merges TOML theme values over defaults.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "feat: make theme colors configurable via config.toml"

### D3 — Config hot-reload [L] **[NEW]**
- |-> D2
- Problem: Config is loaded once at startup. Theme/layout changes require restart.
- Watch config file with `notify` crate or inotify for changes.
- On change: re-read TOML, update `Arc<Theme>` atomically, trigger re-render.
- Evaluate: is the dependency on `notify` worth it? Could also bind a key (e.g. F5) to reload.
- Commit: "feat: hot-reload config on file change"

---

## Phase 5: Code Reorganization [M]

### E1 — Extract CommitCache from App [L] **[DONE]**
- |-> A1
- Problem: `toggle_simplified_graph()` (40 lines in `App`) manages cache directly.
- Create `src/app/cache.rs` with `struct CommitCache { full: Option<Vec<Commit>>, simplified: Option<Vec<Commit>> }`.
- Methods: `get(mode) -> Option<Vec<Commit>>`, `set(mode, commits)`, `invalidate()`.
- Move cache logic out of `App` and into `CommitTableState` (from A1).
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: extract CommitCache from App"

### E2 — Unify duplicate scroll/move helpers in input.rs [L] **[DONE]**
- |-> A1
- Problem: `move_down`, `move_up`, `handle_scroll_down`, `handle_scroll_up`, `handle_scroll_at` all duplicate the same index-wrapping logic.
- Extract `fn cycle_index(current: usize, delta: i32, len: usize) -> usize`.
- Use in all 5 functions.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: deduplicate scroll/move index logic"

---

## Phase 6: Event Loop & State Architecture [NEW — 2026-07-07 Architectural Review]

### F1 — Separate Update from Render (Effect System) [H]
- |-> B3
- Problem: `render()` triggers side effects — lazy loading and auto-diff-request.
  The render phase should be read-only. Side effects during rendering violate the
  intent/effect separation and make the event loop untestable.
- Introduce an `Effect` enum and clean event loop phases:
  ```rust
  enum Effect {
      RequestBranches(BranchScope),
      RequestCommits { branch: String, scope: BranchScope, limit: usize, simplified: bool },
      RequestDiff(String),
      LoadMoreCommits,
      Quit,
  }

  // Clean event loop:
  // 1. poll_events()      -> Vec<Event>
  // 2. handle_events()    -> Vec<Command>
  // 3. execute_commands() -> Vec<Effect>    // <-- NEW: all side effects go here
  // 4. process_effects()  -> triggers workers
  // 5. poll_workers()     -> updates state
  // 6. render()           -> read-only, NO side effects
  ```
- `execute_commands()` returns `Vec<Effect>` instead of directly calling `request_*()`.
- Auto-diff-request moves from `render()` into the command execution phase.
- Lazy-load trigger moves from `render()` into `Effect::LoadMoreCommits` processing.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: separate update from render with Effect system"

### F2 — Diff LRU Cache [M]
- |-> A1
- Problem: Every up/down in the commit list triggers a full `git2 diff_tree_to_tree()`.
  Navigating back to a recently viewed commit re-fetches everything. Wasteful I/O.
- Introduce a bounded cache:
  ```rust
  struct DiffCache {
      entries: LinkedHashMap<String, Arc<DiffResult>>,
      max_entries: usize,  // default: 50
      hits: u64,
      misses: u64,
  }
  impl DiffCache {
      fn get(&mut self, hash: &str) -> Option<Arc<DiffResult>>;
      fn insert(&mut self, hash: String, result: DiffResult);
      fn invalidate(&mut self);
      fn stats(&self) -> (u64, u64);  // hits, misses
  }
  ```
- Integrate into `DiffState`. Before sending `DiffCommand`, check cache.
- Invalidate cache on branch switch or graph toggle.
- Add `lru` or `linked-hash-map` crate, or use a simple `VecDeque<(String, Arc<DiffResult>)>`.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "perf: add LRU cache for diff results"

### F3 — Resize Event Debouncing [M]
- |-> P (no dependencies)
- Problem: Terminal resize produces multiple `Event::Resize` per physical resize.
  Each triggers `compute_areas()` and a full render. Causes visual flicker and wasted CPU.
- Debounce resize events:
  ```rust
  struct ResizeState {
      last_resize: Instant,
      pending_size: Option<Size>,
      debounce_ms: u64,   // default: 50ms
      flush_deadline: Option<Instant>,
  }
  ```
- On first resize: apply immediately.
- On subsequent resizes within `debounce_ms`: store pending, skip render.
- On idle timeout (e.g. 150ms since last resize): flush pending size if any.
- This is a pure event-loop concern — no new dependencies required.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "perf: debounce terminal resize events"

### F4 — Unified State Trait (StateComponent) [M]
- |-> B3, F1
- Problem: Each sub-state has ad-hoc methods. No common interface for init, reset,
  or status queries. Adding state components requires boilerplate.
- Introduce a common trait:
  ```rust
  trait StateComponent {
      fn init(&mut self);
      fn reset(&mut self);
      fn is_loading(&self) -> bool;
      fn update(&mut self, cmd: &Command) -> Vec<Effect>;
  }
  ```
- `AppState::update(cmd)` delegates to the relevant sub-state and collects `Vec<Effect>`.
- Enables:
  - Consistent reset semantics across all states
  - Single entry point for state mutation
  - Future property-based testing of state transitions
  - Clean undo/redo stack (by recording `(Command, StateSnapshot)`)
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: introduce StateComponent trait for uniform state interface"

### F5 — Worker Error Recovery (Resilient Workers) [M]
- |-> C1
- Problem: If a worker thread panics or encounters a persistent git2 error, the
  channel closes. The app silently ignores the dead worker. No retry or user feedback.
- Implement resilient worker pattern:
  ```rust
  struct ResilientWorker<C, R> {
      inner: Option<BackgroundWorker<C, R>>,
      status: WorkerStatus,  // Running | Error(AppError) | Stopped
      retry_count: u32,
      max_retries: u32,
      last_command: Option<C>,
  }
  ```
- When `try_recv()` returns `RecvError`: set status to `Stopped`, show "Worker stopped — press r to restart".
- On restart key: spawn new thread, resend last_command.
- Worker errors show as status messages in the help bar, not crashes.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "feat: resilient workers with restart on failure"

### F6 — Bounded Commit Memory (Sliding Window) [L]
- |-> A1
- Problem: `CommitTableState.all_commits: Vec<Commit>` stores all fetched commits.
  For repositories with 500k+ commits, this can consume 500+ MB. No eviction strategy.
- Implement a sliding window or ring buffer:
  ```rust
  struct CommitStore {
      window: VecDeque<Commit>,
      window_start: usize,      // offset into the full logical list
      total_loaded: usize,
      estimated_total: Option<usize>,
      max_window_size: usize,   // default: 50_000
  }
  ```
- Keep only `[window_start .. window_start + max_window_size]` in memory.
- Evict commits that fall outside the window.
- Lazy-load supports both forward and backward direction.
- Visible-to-commit mapping must account for the window offset.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "perf: bounded commit memory with sliding window eviction"

### F7 — Extensible Layout System [L]
- |-> B2
- Problem: Panels are hardcoded as 5 fixed regions. No way to hide, swap, or rearrange panels.
- Define layouts declaratively:
  ```rust
  struct LayoutDefinition {
      panels: Vec<PanelSlot>,
      splits: Vec<Split>,
  }
  struct PanelSlot {
      panel_id: PanelId,
      visible: bool,
      constraints: Constraint,
  }
  ```
- TOML config declares panel visibility and proportions:
  ```toml
  [layout]
  panels = ["branches", "search", "scope", "commits", "diff"]
  show_search = true
  show_scope = true
  branches.width_pct = 20
  diff.height_pct = 35
  ```
- `compute_areas()` reads from `LayoutDefinition` instead of hardcoded dimensions.
- Allows future panel types (stash panel, file tree, blame) to be added without refactoring layout.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "feat: declarative layout system with configurable panel visibility"

---

## Dependency Graph

```
Phase 1:  [A1] ──► [A2]
          DONE    DONE

Phase 2:  [A1] ──► [B1] ──► [B2] ──► [B3]

Phase 3:  [A1] ──► [C1] ──► [C2]

Phase 4:  [A1] ──► [D1] ──► [D2] ──► [D3]
                    DONE    DONE

Phase 5:  [A1] ──► [E1]
          │        DONE
          └──► [E2]
                DONE

Phase 6:  [B3] ──► [F1] ──► [F4] (StateComponent trait)
                    │
          [A1] ──► [F2] (Diff cache — independent)
          [P]  ──► [F3] (Resize debounce — independent)
          [C1] ──► [F5] (Resilient workers)
          [A1] ──► [F6] (Sliding window — independent)
          [B2] ──► [F7] (Layout system)
```

Phases 2-6 can run largely in parallel after their respective prerequisites.

---

## Recommended Execution Order

1. **Complete Phase 2** (B1 → B2 → B3) — prerequisite for F1, F4, F7.
2. **Implement F1** (Effect system) — highest architectural impact, enables testing.
3. **Implement F2** (Diff cache) — highest user-perceived performance gain.
4. **Implement F3** (Resize debounce) — quick win, smooth UX.
5. **Complete Phase 3** (C1) — prerequisite for F5.
6. **Implement F4** (StateComponent trait) — enables future undo.
7. **Implement F5** (Resilient workers) — robustness.
8. **F6, F7, D3** — deferred; evaluate based on user feedback and repository size needs.

---

## Target Module Structure After Refactoring

```
src/
  ├── main.rs
  ├── lib.rs
  ├── app/
  │   ├── mod.rs           -- App struct (state + workers + run loop)
  │   ├── input.rs          -- handle_event, handle_key (produces Commands)
  │   ├── commands.rs       -- execute(commands, state) -> Vec<Effect>
  │   ├── effects.rs        -- Effect enum + process_effects()
  │   ├── render.rs         -- thin orchestrator: layout + panel.render() [READ-ONLY]
  │   ├── branches.rs       -- rebuild_branch_tree, toggle_node
  │   ├── search.rs         -- apply_search_filter, visible mapping
  │   ├── viewport.rs       -- lazy loading, clamp_selection
  │   └── cache.rs          -- CommitCache
  │
  ├── state/
  │   ├── mod.rs
  │   ├── branch.rs         -- BranchState
  │   ├── commit.rs         -- CommitTableState + CommitStore (sliding window)
  │   ├── diff.rs           -- DiffState + DiffCache (LRU, 50 entries)
  │   ├── search.rs         -- SearchState
  │   ├── ui.rs             -- UiState + ResizeState (debounce)
  │   ├── loading.rs        -- LoadingState
  │   └── traits.rs         -- StateComponent trait
  │
  ├── git/
  │   ├── mod.rs            -- GitRepository struct + open/detect_default_branch
  │   ├── branches.rs       -- fetch_branches, list_branches, resolve_branch_ref_name
  │   ├── commits.rs        -- fetch_commits, fetch_commits_simplified, enrich_commits
  │   ├── diff.rs           -- fetch_diff, fetch_commit_info
  │   ├── decorations.rs    -- build_decoration_map, build_branch_tip_colors
  │   └── graph_conv.rs     -- build_commits_from_graph
  │
  ├── ui/
  │   ├── mod.rs
  │   ├── panel.rs          -- Panel trait (with typed State)
  │   ├── render_ctx.rs     -- RenderCtx (focus, debug_label, theme ref)
  │   ├── layout.rs         -- compute_areas, resize logic, LayoutDefinition
  │   ├── branch_panel.rs   -- BranchPanel (render + handle_event + help_keys)
  │   ├── commit_table.rs   -- CommitPanel (render + handle_event + help_keys)
  │   ├── diff_panel.rs     -- DiffPanel (render + handle_event + help_keys)
  │   ├── search_panel.rs   -- SearchPanel (render + handle_event + help_keys)
  │   ├── scope_panel.rs    -- ScopePanel (render + handle_event + help_keys)
  │   ├── help_bar.rs       -- HelpBar (render only)
  │   └── scrollbar_view.rs -- ScrollbarView widget
  │
  ├── models/
  │   ├── mod.rs
  │   ├── commit.rs         -- Commit, CommitInfo, Decoration, DecorationKind
  │   ├── branch.rs         -- BranchNode, TreeItem, BranchScope, BranchEntry, BranchData
  │   ├── diff.rs           -- FileEntry
  │   └── ui.rs             -- Panel (enum), DragDirection, LayoutAreas
  │
  ├── workers/
  │   ├── mod.rs             -- Command/Result enums, worker spawn functions
  │   ├── background.rs      -- BackgroundWorker<C,R>
  │   ├── pool.rs            -- WorkerPool + Worker trait
  │   └── resilient.rs       -- ResilientWorker<C,R>
  │
  ├── error.rs              -- AppError (thiserror)
  ├── theme.rs              -- Theme struct + ThemeConfig (Deserialize)
  ├── config.rs             -- Config (TOML loading + watching)
  ├── graph.rs              -- create_graph_settings, lane color constants
  ├── tree.rs               -- build/sort/flatten branch tree
  ├── lcs.rs                -- word-level diff via similar
  ├── diff_format.rs        -- append_diff_line formatting
  ├── diff_pairing.rs       -- word-level diff line pairing maps
  ├── text_utils.rs         -- unicode-safe text operations
  ├── time_format.rs        -- epoch-to-YMD formatting
  └── clipboard.rs          -- arboard wrapper

tests/
  ├── integration_test.rs
  └── git_integration_test.rs
```
