git-log-ratatui/
│
├── Cargo.toml              # deps: ratatui, crossterm, clap, arboard, unicode-width, libc, git2, git-graph, similar, rayon
├── AGENTS.md               # coding conventions
│
├── src/
│   │
│   ├── main.rs (135L)      # CLI args (clap), terminal setup/teardown, panic handler, tracing init
│   ├── lib.rs (19L)        # public API: app, config, domain, error, git, view
│   │
│   ├── domain/             # pure git data types (zero ratatui dependency)
│   │   ├── mod.rs (5L)
│   │   ├── commit.rs (70L) # Commit, Decoration, DecorationKind, CommitInfo
│   │   ├── branch.rs (91L) # BranchNode, BranchScope, BranchEntry, BranchData
│   │   └── diff.rs (10L)   # FileEntry
│   │   └── [tests: 9]      # CommitInfo, BranchScope
│   │
│   ├── view/               # UI presentation types
│   │   ├── mod.rs (5L)
│   │   ├── commit.rs (49L) # CommitRow (domain::Commit + graph rendering data)
│   │   ├── branch.rs (13L) # TreeItem (flattened branch tree display)
│   │   └── panel.rs (73L)  # Panel enum (Branches, Search, Scope, Commits, Diff)
│   │   └── [tests: 3]      # Panel cycle, display
│   │
│   ├── git/                # git2 + git-graph repository operations
│   │   ├── mod.rs (164L)   # GitRepository struct, detect_default_branch
│   │   ├── branches.rs (107L) # fetch_branches, list_branches, resolve_branch_ref_name
│   │   ├── commits.rs (184L)  # fetch_commits (git-graph), fetch_commits_simplified (revwalk)
│   │   ├── decorations.rs (97L) # build_decoration_map, build_branch_tip_colors
│   │   ├── diff.rs (104L)     # fetch_commit_info, fetch_diff (unified diff)
│   │   └── graph_conv.rs (387L) # build_commits_from_graph (Unicode box-drawing)
│   │   └── [tests: 7]      # linear history, merge commits, decorations
│   │
│   ├── state/              # panel-specific UI state
│   │   ├── mod.rs (9L)     # re-exports
│   │   ├── branch.rs (112L) # BranchState + handle_command
│   │   ├── commit.rs (77L)  # CommitTableState + handle_command
│   │   ├── diff.rs (155L)   # DiffState + handle_command
│   │   ├── search.rs (66L)  # SearchState + handle_command
│   │   └── ui.rs (128L)     # UiState + handle_command
│   │   └── [tests: 46]     # per-state command handler tests
│   │
│   ├── app/                # application orchestration
│   │   ├── mod.rs (58L)    # App struct, App::new(), constants, submodule declarations
│   │   ├── state.rs (41L)  # AppState (all panel states + theme)
│   │   ├── runner.rs (131L)  # run() event loop, run_profile() benchmark
│   │   ├── worker_mgr.rs (193L) # request_*(), process_git_results(), poll_*(), toggle_simplified_graph()
│   │   ├── commands.rs (92L)   # Command enum, Effect enum
│   │   ├── input.rs (667L)     # handle_event(), handle_key(), mouse, dispatch(), process_effects()
│   │   ├── render.rs (148L)    # render() -- layout, panel dispatch, help bar
│   │   ├── viewport.rs (22L)   # request_more_commits() lazy loading
│   │   ├── search.rs (237L)    # apply_search_filter, visible_count, clamp_selection
│   │   ├── branches.rs (139L)  # rebuild_branch_tree (local/remote/tags sections)
│   │   ├── cache.rs (52L)      # CommitCache (full/simplified graph mode caching)
│   │   ├── tests/
│   │   │   ├── mod.rs (3L)
│   │   │   ├── cache_test.rs (96L)    # CommitCache tests
│   │   │   ├── search_test.rs (160L)  # search helpers tests
│   │   │   └── main_test.rs (318L)    # App integration tests
│   │   └── [total tests: 32]
│   │
│   ├── ui/                 # ratatui rendering panels
│   │   ├── mod.rs (10L)
│   │   ├── panel.rs (29L)       # Panel trait (render, handle_event, help_keys, label)
│   │   ├── render_ctx.rs (21L)  # RenderCtx (focus, theme, repo_path, selected_branch)
│   │   ├── layout.rs (384L)     # compute_areas, resize handles, geometry helpers
│   │   ├── commit_table.rs (673L) # CommitPanel: graph, hash, subject, author, date columns
│   │   ├── diff_panel.rs (612L) # DiffPanel: metadata, changed files, word-level diff
│   │   ├── branch_panel.rs (186L) # BranchPanel: hierarchical tree with Unicode connectors
│   │   ├── search_panel.rs (206L) # SearchPanel: search input with cursor, word navigation
│   │   ├── scope_panel.rs (90L)  # ScopePanel: branch scope indicator (all/local/remote)
│   │   ├── help_bar.rs (57L)     # context-sensitive keyboard shortcut bar
│   │   └── scrollbar_view.rs (281L) # ScrollbarView: encapsulated scrollbar widget
│   │   └── [tests: 53]      # commit_table, diff_panel, layout, scrollbar_view
│   │
│   ├── config.rs (146L)     # Config, RawConfig, LayoutConfig, BehaviorConfig, TOML loading
│   ├── error.rs (28L)       # AppError enum (Git, Io, Clipboard, General)
│   ├── theme.rs (246L)      # Theme, ThemeConfig, parse_hex()
│   ├── graph.rs (62L)       # git-graph Settings factory (all/local/remote scopes)
│   ├── tree.rs (270L)       # hierarchical branch tree: build, sort, flatten
│   ├── lcs.rs (132L)       # word-level diff via `similar` crate
│   ├── diff_pairing.rs (169L) # DiffPairMaps for O(1) added/removed line lookups
│   ├── diff_format.rs (83L)   # append_diff_line (git2 diff output parsing)
│   ├── text_utils.rs (196L)   # Unicode-aware text nav, truncate, format_commit_count_info
│   ├── time_format.rs (108L)  # git2::Time → "YYYY-MM-DD HH:MM"
│   ├── clipboard.rs (24L)     # system clipboard (arboard)
│   ├── worker.rs (50L)        # BackgroundWorker<C, R> generic channel wrapper
│   └── workers.rs (160L)      # BranchWorker, CommitWorker, DiffWorker + spawn
│
├── tests/
│   ├── integration_test.rs (231L)  # 9 tests: initial state, search, scope, focus, selection
│   └── git_integration_test.rs (335L) # 8 tests: branches, commits, diff, decorations
│
├── STRUCTURE.md            # this file
├── README.md               # user documentation
├── TODO.md                 # open tasks
└── DESIGN_TODO.md          # architectural refactoring plan


ARCHITECTURE OVERVIEW
═══════════════════════

  ┌─────────────────────────────────────────────────┐
  │                    lib.rs                       │
  │  ┌──────────┐ ┌──────────┐ ┌─────────────────┐ │
  │  │  domain/  │ │  view/   │ │     state/      │ │
  │  │  git data │ │ UI types │ │  panel state    │ │
  │  └──────────┘ └──────────┘ └─────────────────┘ │
  │  ┌──────────┐ ┌──────────┐ ┌─────────────────┐ │
  │  │   git/   │ │   ui/    │ │      app/       │ │
  │  │ repo ops │ │ ratatui  │ │  orchestrator   │ │
  │  └──────────┘ └──────────┘ └─────────────────┘ │
  └─────────────────────────────────────────────────┘

  MAIN THREAD (App::run in app/runner.rs)
  ──────────────────────────────────────
  ┌────────────────────────────────────────────────┐
  │  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
  │  │  Branch  │  │  Commit  │  │   Diff   │      │
  │  │  Worker  │  │  Worker  │  │  Worker  │      │
  │  │ ───────  │  │ ──────── │  │ ──────── │      │
  │  │ thread 1 │  │ thread 2 │  │ thread 3 │      │
  │  └────┬─────┘  └────┬─────┘  └────┬─────┘      │
  │       │try_recv()   │try_recv()   │try_recv()  │
  │       ▼             ▼             ▼            │
  │  ┌──────────────────────────────────────┐      │
  │  │  worker_mgr::process_git_results()   │      │
  │  │  Updates App.state via poll_*()      │      │
  │  └──────────────────────────────────────┘      │
  │       │                                        │
  │       ▼                                        │
  │  ┌──────────────────────────────────────┐      │
  │  │  input::handle_event()               │      │
  │  │  keyboard / mouse → Commands         │      │
  │  └─────────────────┬────────────────────┘      │
  │                    │                           │
  │                    ▼                           │
  │  ┌──────────────────────────────────────┐      │
  │  │  input::dispatch()                   │      │
  │  │  Route commands → state handlers     │      │
  │  │  state::*::handle_command()          │      │
  │  └─────────────────┬────────────────────┘      │
  │                    │ returns Vec<Effect>       │
  │                    ▼                           │
  │  ┌──────────────────────────────────────┐      │
  │  │  input::process_effects()            │      │
  │  │  Call workers, rebuild tree, copy    │      │
  │  └──────────────────────────────────────┘      │
  │       │                                        │
  │       ▼                                        │
  │  ┌──────────────────────────────────────┐      │
  │  │  render::render() → 5 panels         │      │
  │  │  Branch, Search, Scope, Commits, Diff│      │
  │  └──────────────────────────────────────┘      │
  └────────────────────────────────────────────────┘

  Command → Effect Flow
  ═════════════════════
  Panel::handle_event() → Vec<Command>
       │
       ▼
  input::dispatch() → routes to state::*::handle_command()
       │
       ▼ returns Vec<Effect>
  input::process_effects() → worker calls, clipboard, tree rebuild


LAYOUT (5 Panels)
════════════════

  ┌──────────────┬─────────────────────────────────┐
  │              │  Search Bar + Scope             │  ▲ 3 rows
  │   Branch     ├─────────────────────────────────┤
  │   Tree       │                                 │
  │              │  Commit Table                   │  ▼ fill
  │  20% width   │  (Graph|Hash|Subject|Author|Date)
  │  adjustable  │                                 │
  │              ├─────────────────────────────────┤
  │              │  Diff Panel                     │  ▲ 35% height
  │              │  (metadata + changed files      │  │ adjustable
  │              │   + colored word-level diff)    │  ▼
  ├──────────────┴─────────────────────────────────┤
  │  Help Bar (context-sensitive keyboard shortcuts)│  3 rows
  └────────────────────────────────────────────────┘


TEST COVERAGE: 229 tests
══════════════════════

  state/                    46   per-state command handlers (Branch, Commit, Diff, Search, Ui)
  ui/layout.rs              31   resize, borders, geometry
  ui/commit_table.rs        15   graph spans, hashes, decorations, offsets
  app/tests/                32   search filter, visible mapping, selection, CommitCache
  config.rs                 13   defaults, config_path, roundtrip
  theme.rs                  13   parse_hex, Theme::default
  ui/scrollbar_view.rs       9   click mapping, position calculation
  ui/diff_panel.rs          10   metadata, offsets
  domain/                    9   CommitInfo, BranchScope
  tests/integration_test.rs  9   end-to-end: search, scope, focus, selection
  tests/git_integration_test.rs 8  git2 operations
  git/graph_conv.rs          5   linear history, merge commits
  git/mod.rs                 2   current repo, shorthand branch
  ui/commit_table.rs        15   refactored from above
  text_utils.rs             13   Unicode navigation, truncation
  diff_format.rs             6   git2 diff line parsing
  diff_pairing.rs            6   added/removed line pairing
  lcs.rs                     6   word diff tokens
  tree.rs                    7   branch tree build, sort, flatten
  graph.rs                   3   graph settings scopes
  time_format.rs             5   date formatting
  view/panel.rs              3   Panel cycle, display
  worker.rs                  0   (tests use real repo)
  clipboard.rs               0   (I/O dependent)
