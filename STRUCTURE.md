git-log-ratatui/
│
├── Cargo.toml           # deps: ratatui, crossterm, clap, arboard, git2, similar
├── AGENTS.md            # coding conventions
│
└── src/
    │
    ├── main.rs (99L)    # CLI args, terminal setup/teardown, panic handler
    │
    ├── models.rs (209L) # data types: Commit, CommitInfo, TreeItem, BranchScope, Panel, FileEntry
    │
    ├── git_repository.rs (591L)   # git2 operations: branches, commit info, diff, git log --graph
    │   │                           # + parsing: extract_graph, parse_decorations, append_diff_line
    │   └── [tests: 21]
    │
    ├── workers.rs (195L)          # per-window threads
    │   ├── BranchWorker  ──► mpsc channels ──► BranchCommand / BranchResult
    │   ├── CommitWorker  ──► mpsc channels ──► CommitCommand / CommitResult
    │   └── DiffWorker    ──► mpsc channels ──► DiffCommand  / DiffResult
    │
    ├── app.rs (1476L)             # App state, event loop, keyboard/mouse, layout, search
    │   └── [tests: 14]            #   text navigation, search filter, visible mapping, selection
    │
    ├── tree.rs (246L)             # branch tree: build, sort, flatten (tree prefix rendering)
    │   └── [tests: 5]
    │
    ├── lcs.rs (122L)              # word-level diff via `similar` crate
    │   └── [tests: 6]             #   diff_tokens_added / diff_tokens_removed
    │
    ├── clipboard.rs (22L)         # system clipboard (arboard)
    │
    └── ui/
        ├── mod.rs (6L)
        ├── commit_table.rs (309L) # git graph, hash, subject, author, date columns
        │   └── [tests: 3]         #   truncate (ascii, multibyte, emoji)
        ├── branch_panel.rs (58L)  # hierarchical branch tree list
        ├── diff_panel.rs (433L)   # metadata, changed files, colored diff with word highlight
        │   └── [tests: 5]         #   build_diff_pairs, find_prev/next
        ├── search_panel.rs (104L) # search input with cursor
        ├── scope_panel.rs (38L)   # branch scope (all/local/remote)
        └── help_bar.rs (110L)     # context-sensitive keyboard shortcuts
            └── [tests: 4]         #   format_commit_count


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


TEST COVERAGE: 65 tests
══════════════════════

  git_repository.rs  21  (graph, decorations, time, diff parsing)
  app.rs             14  (text nav, search, visible mapping, selection)
  models.rs           8  (BranchScope, Panel)
  lcs.rs              6  (word diff tokens)
  tree.rs             5  (branch tree build, sort, flatten)
  ui/diff_panel.rs    5  (diff pairs)
  ui/help_bar.rs      4  (commit count formatting)
  ui/commit_table.rs  3  (truncation)
