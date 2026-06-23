# git-log-ratatui

A TUI for browsing git log with ratatui.

## Features

- 5-panel layout: Branch Tree, Search, Scope, Commit Table, Diff
- Multi-threaded: per-window git worker threads for parallel data streaming
- Hierarchical branch tree with expand/collapse (arrow keys, Space, Enter)
- Incremental search with cursor-based text editing
- Branch scope cycling (all/local/remote) via Ctrl+S
- Commit table with git graph column, decorations (branches/tags/HEAD), merge highlighting
- Diff panel with commit metadata, changed files list, word-level diff highlighting
- Context-sensitive help bar with keyboard shortcuts
- Clipboard: copy short/full commit hash with y/Y
- System clipboard paste into search with Ctrl+V

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
| `q` | Quit |
| `Tab` | Cycle focus forward |

### Branches

| Key | Action |
|-----|--------|
| `↑↓` | Navigate |
| `→←` | Expand/collapse |
| `Space` | Toggle expand |
| `Enter` | Load selected branch commits |

### Search

| Key | Action |
|-----|--------|
| `Esc` | Clear search |
| `Ctrl+V` | Paste from clipboard |

### Scope

| Key | Action |
|-----|--------|
| `Ctrl+S` | Cycle branch scope |

### Commits

| Key | Action |
|-----|--------|
| `↑↓` | Navigate |
| `y` | Copy short hash (7 chars) |
| `Y` | Copy full hash |

### Diff

| Key | Action |
|-----|--------|
| `↑↓` | Navigate files |
| `Enter` | Jump to selected file's diff |
| `n/p` | Next/previous file |
| `Home/End` | Scroll to top/bottom |

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

Each window has its own background git worker thread using mpsc channels.
The main event loop polls all channels non-blocking and requests data on demand.

## Dependencies

- [ratatui](https://crates.io/crates/ratatui) — TUI framework
- [crossterm](https://crates.io/crates/crossterm) — Terminal backend
- [clap](https://crates.io/crates/clap) — CLI argument parsing
- [arboard](https://crates.io/crates/arboard) — System clipboard
- [unicode-width](https://crates.io/crates/unicode-width) — Unicode character width
- [libc](https://crates.io/crates/libc) — POSIX syscalls (suspend)
