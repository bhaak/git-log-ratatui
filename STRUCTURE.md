git-log-ratatui/
│
├── Cargo.toml           # deps: ratatui, crossterm, clap, arboard, unicode-width, libc, git2, git-graph, similar, rayon
├── AGENTS.md            # coding conventions
│
└── src/
    │
    ├── main.rs (101L)    # CLI args, terminal setup/teardown, panic handler
    │
    ├── models.rs (228L) # data types: Commit, Decoration, DecorationKind, CommitInfo,
    │   │                 #   BranchNode, TreeItem, BranchScope, Panel, FileEntry
    │   └── [tests: 8]
    │
    ├── git_repository.rs (334L)   # git2 + git-graph: branches, commit info, diff, git log --graph
    │   │                           # + parsing: extract_graph, parse_decorations, append_diff_line
    │   └── [tests: 1]
    │
    ├── workers.rs (210L)          # per-window threads
    │   ├── BranchWorker  ──► mpsc channels ──► BranchCommand / BranchResult
    │   ├── CommitWorker  ──► mpsc channels ──► CommitCommand / CommitResult
    │   └── DiffWorker    ──► mpsc channels ──► DiffCommand  / DiffResult
    │
    ├── app.rs (1603L)             # App state, event loop, keyboard/mouse, layout, search
    │   └── [tests: 7]             #   text navigation, search filter, visible mapping, selection
    │
    ├── tree.rs (248L)             # branch tree: build, sort, flatten (tree prefix rendering)
    │   └── [tests: 5]
    │
    ├── lcs.rs (132L)              # word-level diff via `similar` crate
    │   └── [tests: 6]             #   diff_tokens_added / diff_tokens_removed
    │
    ├── clipboard.rs (21L)         # system clipboard (arboard)
    │
    └── ui/
        ├── mod.rs (8L)
        ├── commit_table.rs (412L) # git graph, hash, subject, author, date columns
        │   └── [tests: 11]        #   truncate (ascii, multibyte, emoji), decoration styles
        ├── branch_panel.rs (58L)  # hierarchical branch tree list
        ├── diff_panel.rs (462L)   # metadata, changed files, colored diff with word highlight
        │   └── [tests: 7]         #   build_diff_pairs, find_prev/next
        ├── search_panel.rs (103L) # search input with cursor
        ├── scope_panel.rs (38L)   # branch scope (all/local/remote)
        └── help_bar.rs (71L)      # context-sensitive keyboard shortcuts


ARCHITECTURE OVERVIEW
═══════════════════════

  ┌───────────────────────────────────────────────┐
  │ MAIN THREAD (App::run)                        │
  │                                               │
  │  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
  │  │  Branch  │  │  Commit  │  │   Diff   │     │
  │  │  Worker  │  │  Worker  │  │  Worker  │     │
  │  │ ───────  │  │ ──────── │  │ ──────── │     │
  │  │ thread 1 │  │ thread 2 │  │ thread 3 │     │
  │  └────┬─────┘  └────┬─────┘  └────┬─────┘     │
  │       │try_recv()   │try_recv()   │try_recv() │
  │       ▼             ▼             ▼           │
  │  ┌──────────────────────────────────────┐     │
  │  │  process_git_results()               │     │
  │  │  Update App state (branch_tree,      │     │
  │  │  filtered_commits, diff_lines ...)   │     │
  │  └──────────────────────────────────────┘     │
  │       │                                       │
  │       ▼                                       │
  │  ┌──────────────────────────────────────┐     │
  │  │  handle_event() → keyboard / mouse   │     │
  │  └──────────────────────────────────────┘     │
  │       │                                       │
  │       ▼                                       │
  │  ┌──────────────────────────────────────┐     │
  │  │  render() → 5 panels                 │     │
  │  └──────────────────────────────────────┘     │
  └───────────────────────────────────────────────┘


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
  │              │   + colored diff)               │  ▼
  ├──────────────┴─────────────────────────────────┤
  │  Help Bar                                      │  3 rows
  └────────────────────────────────────────────────┘


DATA FLOW
═════════

  User Input ──► handle_event() ──► state mutation ──► request_*()
      ▲                                                    │
      │                                                    ▼
      │                                            send(Command) → Worker Thread
      │                                                    │
      │                                           git2::Repository op
      │                                                    │
      │                                                    ▼
      └─────────────────── render() ◄── process_git_results()
                                          try_recv() ← Worker Thread


TEST COVERAGE: 111 tests (110 passed)
══════════════════════

  models.rs            8  (BranchScope, Panel, CommitInfo)
  ui/commit_table.rs  11  (truncation, decoration styles)
  app.rs               7  (text nav, search, visible mapping, selection)
  ui/diff_panel.rs     7  (diff pairs)
  lcs.rs               6  (word diff tokens)
  tree.rs              5  (branch tree build, sort, flatten)
  git_repository.rs    1  (graph parsing)
