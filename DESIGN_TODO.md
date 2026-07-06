# Architecture Refactoring Plan

Based on full codebase analysis (2026-07-06).
Current: 44 `.rs` files, 17 top-level modules, ~3000 lines.

Legend:
  [P] = parallelizable (no dependency on other tasks)
  [H] = high priority
  [M] = medium priority
  [L] = low priority
  |->  = depends on

---

## Phase 1: Data Ownership & Encapsulation [H]

### A1 — Split AppState into focused state structs [H]
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

### A2 — Remove Deref anti-pattern from App [H]
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

### D1 — Eliminate Theme clone in render hot path [M]
- |-> A1
- Problem: `src/app/render.rs:21` clones a 40-field `Theme` struct every frame.
- Change `AppState.theme: Theme` to `AppState.theme: Arc<Theme>`.
- `RenderCtx` (from B1) takes `&Theme`.
- When theme is reloaded from config, swap the `Arc` atomically.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "perf: use Arc<Theme> to avoid per-frame clone"

### D2 — Make theme fully configurable via TOML [L]
- |-> D1
- Problem: Theme is hardcoded in `src/theme.rs:69-112`. Config supports layout/behavior but not theme.
- Add `[theme]` section to config TOML with all color fields.
- Derive `Deserialize` for `Theme`.
- `Config::load()` merges TOML theme values over defaults.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "feat: make theme colors configurable via config.toml"

---

## Phase 5: Code Reorganization [M]

### E1 — Extract CommitCache from App [L]
- |-> A1
- Problem: `toggle_simplified_graph()` (40 lines in `App`) manages cache directly.
- Create `src/app/cache.rs` with `struct CommitCache { full: Option<Vec<Commit>>, simplified: Option<Vec<Commit>> }`.
- Methods: `get(mode) -> Option<Vec<Commit>>`, `set(mode, commits)`, `invalidate()`.
- Move cache logic out of `App` and into `CommitTableState` (from A1).
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: extract CommitCache from App"

### E2 — Unify duplicate scroll/move helpers in input.rs [L]
- |-> A1
- Problem: `move_down`, `move_up`, `handle_scroll_down`, `handle_scroll_up`, `handle_scroll_at` all duplicate the same index-wrapping logic.
- Extract `fn cycle_index(current: usize, delta: i32, len: usize) -> usize`.
- Use in all 5 functions.
- Run `cargo fmt && cargo clippy && cargo test`.
- Commit: "refactor: deduplicate scroll/move index logic"

---

## Dependency Graph

```
Phase 1:  [A1] ──► [A2]

Phase 2:  [A1] ──► [B1] ──► [B2] ──► [B3]

Phase 3:  [A1] ──► [C1] ──► [C2]

Phase 4:  [A1] ──► [D1] ──► [D2]

Phase 5:  [A1] ──► [E1]
                  └──► [E2]
```

Phases 2-5 can run in parallel after Phase 1 completes.

---

## Target Module Structure After Refactoring

```
src/
  ├── main.rs
  ├── lib.rs
  ├── app/
  │   ├── mod.rs           -- App struct (state + workers + run loop)
  │   ├── input.rs          -- handle_event, handle_key (produces Commands)
  │   ├── commands.rs       -- execute(commands, state) (applies mutations)
  │   ├── render.rs         -- thin orchestrator: layout + panel.render()
  │   ├── branches.rs       -- rebuild_branch_tree, toggle_node
  │   ├── search.rs         -- apply_search_filter, visible mapping
  │   ├── viewport.rs       -- lazy loading, clamp_selection
  │   └── cache.rs          -- CommitCache
  │
  ├── state/                -- NEW: focused state structs
  │   ├── mod.rs
  │   ├── branch.rs         -- BranchState
  │   ├── commit.rs         -- CommitTableState (+ CommitCache)
  │   ├── diff.rs           -- DiffState
  │   ├── search.rs         -- SearchState
  │   ├── ui.rs             -- UiState
  │   └── loading.rs        -- LoadingState
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
  │   ├── panel.rs          -- Panel trait (with typed State + Command pattern)
  │   ├── render_ctx.rs     -- RenderCtx (focus, debug_label, theme ref)
  │   ├── layout.rs         -- compute_areas, resize logic
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
  ├── workers.rs            -- Command/Result enums, worker spawn functions
  ├── worker.rs             -- BackgroundWorker<C,R>, Worker trait, WorkerPool
  ├── error.rs              -- AppError (thiserror)
  ├── theme.rs              -- Theme struct
  ├── config.rs             -- Config (TOML loading, now includes [theme])
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
