# git-log-ratatui

A TUI for browsing git log with ratatui.

## Features

- 5-panel layout: Branch Tree, Search, Scope, Commit Table, Diff
- Multi-threaded: per-window git worker threads using git2 for parallel data streaming
- Hierarchical branch tree with expand/collapse (arrow keys, Space, Enter)
- Incremental search with cursor-based text editing
- Branch scope cycling (all/local/remote) via Ctrl+S
- Commit table with Unicode git graph via git-graph crate, decorations (branches/tags/HEAD), merge highlighting
- Diff panel with commit metadata, changed files list, word-level diff highlighting
- Context-sensitive help bar with keyboard shortcuts
- Clipboard: copy short/full commit hash with y/Y
- System clipboard paste into search with Ctrl+V
- Scrollbars on branch, commit, and diff panels
- Mouse support: click to focus/select, scroll to navigate, drag to resize panels
- Suspend with Ctrl+Z (restores UI on resume)
- Resizable panels: drag borders to adjust branch width and diff height

## Installation

```sh
cargo build --release
```

## Usage

```sh
git-log-ratatui [--path <repo-path>]
```

If no path is given, the current directory is used.

## Keyboard shortcuts

### Global

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit |
| `Tab` / `l` | Cycle focus forward |
| `Shift+Tab` / `h` | Cycle focus backward |
| `j` / `k` | Move down/up in focused panel |
| `Ctrl+S` | Cycle branch scope |
| `y` | Copy short hash (7 chars) |
| `Y` | Copy full hash |
| `Ctrl+V` | Paste clipboard into search |
| `Ctrl+Z` | Suspend (fg to resume) |

### Branches

| Key | Action |
|-----|--------|
| `↑↓` / `j`/`k` | Navigate |
| `→` / `Enter` | Expand or load branch |
| `←` | Collapse |
| `Space` | Toggle expand/collapse |

### Search

| Key | Action |
|-----|--------|
| `Esc` | Clear search |
| `Ctrl+A` | Move cursor to start |
| `Ctrl+E` | Move cursor to end |
| `Ctrl+←/→` | Jump by word |

### Scope

| Key | Action |
|-----|--------|
| `Space` / `Enter` | Cycle branch scope |

### Commits

| Key | Action |
|-----|--------|
| `↑↓` / `j`/`k` | Navigate |
| `Enter` | Focus diff panel |
| `PageUp` / `PageDown` | Jump 10 commits |

### Diff

| Key | Action |
|-----|--------|
| `↑↓` / `j`/`k` | Navigate files or scroll |
| `Enter` | Jump to selected file's diff |
| `n` / `p` | Next/previous file |
| `Home` / `End` | Scroll to top/bottom |

## Architecture

```
┌──────────────┬─────────────────────────────────┐
│              │  Search Bar + Scope             │
│   Branch     ├─────────────────────────────────┤
│   Tree       │                                 │
│              │  Commit Table                   │
│              │  (Graph|Hash|Subject|Author|Date)
│              ├─────────────────────────────────┤
│              │  Diff Panel                     │
│              │  (metadata + changed files      │
│              │   + word-level diff)            │
├──────────────┴─────────────────────────────────┤
│  Help Bar                                      │
└────────────────────────────────────────────────┘
```

Each panel has its own background worker thread (BranchWorker, CommitWorker, DiffWorker)
using git2 (libgit2 bindings) and mpsc channels. The main event loop polls all channels
non-blocking and requests data on demand.

## Testing

```sh
cargo test
```

## Dependencies

- [ratatui](https://crates.io/crates/ratatui) — TUI framework
- [crossterm](https://crates.io/crates/crossterm) — Terminal backend
- [clap](https://crates.io/crates/clap) — CLI argument parsing
- [arboard](https://crates.io/crates/arboard) — System clipboard
- [git2](https://crates.io/crates/git2) — Git operations (libgit2 bindings)
- [git-graph](https://crates.io/crates/git-graph) — Git commit graph data structure and rendering
- [similar](https://crates.io/crates/similar) — Diff engine for word-level highlighting
- [unicode-width](https://crates.io/crates/unicode-width) — Unicode character width
- [libc](https://crates.io/crates/libc) — POSIX syscalls (suspend)
