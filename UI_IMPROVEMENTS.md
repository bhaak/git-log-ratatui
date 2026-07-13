# UI/UX Improvements

**Date:** 2026-07-13
**Scope:** Terminal-UI (ratatui) / git log browser
**Method:** Full codebase review of all rendering, input handling, state, and layout modules.

---

## Priorities

- **P0 (Critical):** Safety, discoverability blockers, core workflow gaps.
- **P1 (High):** Efficiency and clarity, affects daily usage.
- **P2 (Medium):** Polish and completeness.
- **P3 (Low):** Nice-to-have, lower ROI.

---

## P0 – Critical

### 1. Help modal (`?` key)
**Problem:** Only a handful of shortcuts fit into the bottom help bar, and those are only the focused panel's keys. New users have no way to discover available bindings.
**Fix:** Pressing `?` opens a modal overlay listing *all* global and context-sensitive keybindings. Dismiss with `Esc` or `?`.

### 2. Loading indicators
**Problem:** Branch fetches, commit fetches, and diff computation are all backgrounded but the UI gives zero feedback. Panels show blank content; the app appears frozen.
**Fix:** Show a spinner / "Loading…" placeholder in every panel whose worker is active. The `loading_more` flag exists but is only used for lazy-load status — extend it to all three workers. A simple Unicode spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) in the panel title would suffice.

### 3. Visual resize handles
**Problem:** Panel borders are draggable but there is no visual indication. The user must discover this by accident.
**Fix:** Render the border intersection with a distinct character or color (e.g., `┼` in accent color) when the mouse hovers near it. Alternatively, draw a subtle grip indicator (`⁞`) on draggable borders.

### 4. Exit confirmation
**Problem:** `q` or `Ctrl+C` immediately exits without confirmation, discarding navigation state (scroll position, selected branch, search query).
**Fix:** Show a short confirmation prompt ("Press q again to quit") if any state is non-trivial (non-empty search, loaded commits, etc.). Or make quit require `Ctrl+C` / `Ctrl+D` and reserve `q` for search-clearing.

### 5. Copy-to-clipboard feedback
**Problem:** Pressing `y` / `Y` to copy a commit hash gives no visual confirmation. The user doesn't know if the operation succeeded.
**Fix:** Set `status_message` to "Copied abc1234 to clipboard" for a few seconds. Clearing on next render or after a timer.

---

## P1 – High

### 6. Branch panel filter/search
**Problem:** Repositories with hundreds of branches require scrolling through all of them. No fuzzy-filter or type-to-search exists in the branch panel.
**Fix:** When the branch panel is focused, typing starts an inline filter (similar to the search panel). Or `f` enters a filter mode. A second `/`-prefixed mini-search input at the top of the branch panel is another option.

### 7. Keyboard panel resize
**Problem:** Resizing panels is mouse-only. There is no keyboard shortcut to increase/decrease branch width or diff height.
**Fix:** `Ctrl+Left` / `Ctrl+Right` adjusts branch width by ±2%. `Ctrl+Up` / `Ctrl+Down` adjusts diff height by ±2%. Respect `MIN`/`MAX` layout constants.

### 8. Universal "back" navigation
**Problem:** Navigating back from Diff → Commits → Branches requires the user to memorize the Tab order. There's no single "go back to previous panel" key.
**Fix:** `Esc` / `Backspace` in the diff panel returns focus to the commit table. `Esc` in the commit table returns to the branch panel, if a branch is selected. Implement as a LIFO focus stack.

### 9. Explicit refresh command
**Problem:** The app polls workers continuously but there is no way to trigger a full reload. After an external `git fetch`, the user must restart the app.
**Fix:** Bind `Ctrl+R` / `F5` to issue new `RequestBranches` + `RequestCommits` commands, clearing caches (CommitCache). The continuous polling already provides some refresh but only for the current viewport, not for stale branch lists.

### 10. Search bar discoverability
**Problem:** The search panel is a single-line input that is easy to overlook when not focused. `/` is not bound to "focus search and clear" as users expect from less/vim.
**Fix:** Bind `/` to `Command::SetFocus(Search) + Command::ClearSearch` globally. Show a subtle "[Type to search]" placeholder in the search input when empty and unfocused.

### 11. Empty / error states in diff panel
**Problem:** When no diff is loaded, the diff panel shows an empty bordered rectangle. The user has no hint whether a diff is loading, is empty, or failed. Worker errors are swallowed or only appear in the status bar.
**Fix:** Show "Select a commit to view diff" when no diff is loaded. Show the error message inline if the worker returned an error. The `status_message` passthrough already exists — surface it more prominently inside the panel.

### 12. Commit count label clarity
**Problem:** The commit table title shows "N/M" or "N/M (T filtered)". New users don't know what N, M, or T represent.
**Fix:** Change to "Commits: 42 of 1,234 (89 filtered)". Add tooltip-level hints in the help modal.

---

## P2 – Medium

### 13. Scrollbar width
**Problem:** Scrollbars are 1 column wide, making them hard to hit with a mouse on high-DPI or wide terminals.
**Fix:** Increase to 2 columns or make configurable. Add a track/thumb contrast: a darker background for the track, a lighter/colored thumb.

### 14. Diff word wrap
**Problem:** Long diff lines require horizontal scrolling, which doesn't exist. Content past the terminal width is invisible. (Already listed in TODO.md.)
**Fix:** Wrap diff lines at the panel width, inserting `↪` continuation markers. Use ratatui's `Paragraph::wrap()` or manual splitting.

### 15. Diff panel file list: +/- line counts
**Problem:** The changed-files list shows file names and status characters but not how many lines were added/removed per file.
**Fix:** Append " (+42/−7)" after each file entry. The data already exists in `FileEntry` — extract counts during diff building.

### 16. Commit subject tooltip / expand
**Problem:** Long commit subjects are truncated with `…`. The full subject cannot be viewed without a wider terminal.
**Fix:** Hovering a commit row (mouse) or pressing a key (`i` for "info") could show the full subject in the status bar or a popup. Alternatively reserve a line below the table for the selected commit's full subject.

### 17. HEAD / current branch emphasis
**Problem:** The branch tree shows `HEAD ->` at the top of the local branches section but the visual distinction is subtle (same color as other local branches).
**Fix:** Render the HEAD branch in a distinct color (e.g., the focused border color) and prefix with a bold indicator like `★`.

### 18. Inline diff error messages
**Problem:** When a diff worker fails (e.g., binary file, huge diff, memory error), the error is set in `status_message` in the help bar — far from the diff panel. The user may not notice.
**Fix:** Display worker errors directly inside the diff panel body, using the error/warning theme color.

### 19. Stale-data indication
**Problem:** If the repository changes while the app is running, the displayed data goes stale silently. The commit table still shows old commits; the branch tree still shows old branches.
**Fix:** Track the last-refresh timestamp per panel. After a configurable interval (30s default), show a subtle "[Stale — press F5]" hint in the panel title.

### 20. Config validation feedback
**Problem:** Parse errors in `config.toml` are silently swallowed via `unwrap_or_default()`. The user gets the default config with no indication something is wrong.
**Fix:** Log parse errors to the status message on startup, or show a one-time warning bar. Report the specific line and field that failed.

---

## P3 – Low

### 21. Diff syntax highlighting
**Problem:** Diff content is raw text with +/- prefixes. Language-aware syntax highlighting (à la `delta` / `diff-so-fancy`) would dramatically improve readability.
**Fix:** Integrate `syntect` or tree-sitter for syntax-aware coloring within diff hunks. This is non-trivial due to partial-context highlighting but high visual impact.

### 22. Column resize in commit table
**Problem:** Commit table columns (Graph, Hash, Subject, Author, Date) have fixed widths. A long author name or date format crowds out the subject.
**Fix:** Allow dragging column separators with the mouse, or cycling through layout presets with a key (`Ctrl+T`). Store in `LayoutConfig`.

### 23. Multi-branch selection
**Problem:** Only one branch can be selected at a time. For workflows comparing branches, viewing all commits, or showing the graph including multiple branches, this is limiting.
**Fix:** Allow `Space` to toggle branch selection (multi-select) in addition to `Enter` for single-select. Commit graph treats all selected branches as heads.

### 24. Git status / working tree panel
**Problem:** The app only shows committed history. Unstaged/staged changes and working tree status are invisible. (Mentioned in TODO.md.)
**Fix:** Add an optional working tree section above or below the diff panel showing `git status` output (modified, staged, untracked files). Reuse the diff rendering infrastructure.

### 25. Blame view
**Problem:** No way to see who last touched each line in a file. (Mentioned in TODO.md.)
**Fix:** Add a blame panel accessible from the diff view (e.g., `b` on a file entry opens blame for that file at the selected commit).

### 26. File tree view
**Problem:** Changed files in the diff panel are a flat list. For commits touching many files in deep directory structures, this is hard to navigate. (Mentioned in TODO.md.)
**Fix:** Render changed files as a collapsible tree by directory. Reuse `build_branch_tree` / `TreeItem` infrastructure.

### 27. Live theme reload
**Problem:** Theme is loaded at startup from `config.toml`. Changes require a restart.
**Fix:** Watch the config file for changes (`notify` crate) and reload theme + layout on modification, immediately re-rendering.

### 28. Diff side-by-side view
**Problem:** The diff is always shown in unified format. A side-by-side view is more natural for some users.
**Fix:** Add a toggle (`t`/`s`) between unified and split view in the diff panel. Layout the two halves side-by-side, synchronizing scroll positions.

---

## Summary

| Priority | Count | Key Items |
|----------|-------|-----------|
| P0 | 5 | Help modal, loading indicators, resize handles, exit confirm, copy feedback |
| P1 | 6 | Branch filter, keyboard resize, back nav, refresh, search discoverability, empty states |
| P2 | 8 | Scrollbar width, word wrap, file line counts, subject expand, HEAD emphasis, inline errors, stale data, config validation |
| P3 | 8 | Syntax highlighting, column resize, multi-branch, git status, blame, file tree, live theme, side-by-side diff |

Items already present in TODO.md are cross-referenced. Items already in ARCHITECTURE_REVIEW.md (rendering side effects, diff cache, resize debounce, etc.) are omitted as they are architectural rather than pure UX.

---

## Consistent Key Handling

**Problem:** The current key map has multiple inconsistencies across panels:

| Key | Branches | Commits | Diff | Search | Scope | Global fallback |
|-----|----------|---------|------|--------|-------|-----------------|
| `j`/`k` | via global | via global | via global | **inserts text** ❌ | — | MoveUp/Down |
| `h`/`l` | via global | via global | via global | **inserts text** ❌ | — | FocusPrev/Next |
| `↑↓` | own handler | own handler | own handler | — | — | — |
| `←→` | expand/collapse | — | — | cursor move | — | — |
| `Home`/`End` | JumpTop/Btm | JumpTop/Btm | JumpTop/Btm | **cursor move** | — | — |
| `PgUp`/`PgDn` | page | page | page | **none** | — | — |
| `Enter` | select/toggle | show diff | jump to file | — | cycle | — |
| `Space` | toggle expand | — | — | insert space | cycle | — |
| `Esc` | — | — | — | clear search | — | — |
| `g` | — | toggle graph | — | inserts 'g' ❌ | — | — |
| `n`/`p` | — | — | next/prev file | inserts text ❌ | — | — |
| `/` | — | — | — | inserts '/' ❌ | — | — |
| `Ctrl+F`/`B` | — | — | — | — | — | — |
| `gg` / `G` | — | — | — | — | — | — |

**Specific issues:**

1. **Vim keys (`j`/`k`/`h`/`l`) break in Search.** The search panel's catch-all `KeyCode::Char(ch)` consumes every alpha character before the global fallback runs. Typing `jjjj` to scroll down while search is focused silently inserts `jjjj` into the query.

2. **`Home` means two different things.** In list panels it jumps to item 0; in search it moves the text cursor to position 0. Both are "go to start" but the scope differs (list vs. text line). This is functionally correct but the mental model must be clear from focus.

3. **`/` has no global binding.** Pressing `/` in commits or branches does nothing (or inserts `/` in search). The standard expectation from vim/less is `/` = "focus search and clear".

4. **`Esc` only works in Search.** Clearing search is the only `Esc` action. Diff → back to commits, commits → back to branches, help modal → dismiss — none exist.

5. **`n`/`p` are Diff-only.** TODO.md already notes "n/p working in every panel". Branches could use them for sibling navigation; commits could use them for author-grouped jump (stretch).

6. **`Ctrl+F`/`Ctrl+B` missing.** Standard page-up/page-down alternatives in vim/less/most pagers.

7. **`gg`/`G` missing.** Vim convention for jump-to-top (`gg`) and jump-to-bottom (`G`).

8. **`Space` has 3 different behaviors.** Toggle expand in branches, insert literal space in search, cycle scope. No unified mental model.

---

### Recommended key layer scheme

```
┌──────────────────────────────────────────────────────────┐
│ Layer 1 — Always-global (no panel may override)          │
│   q          Quit (confirm if state is non-trivial)      │
│   Ctrl+C     Quit                                        │
│   Ctrl+Z     Suspend                                     │
│   Tab        Focus next / Shift+Tab Focus prev           │
│   F1 / ?     Open help modal                             │
│   /          Focus search + clear query                  │
│   Esc        Focus back (LIFO focus stack) / close modal │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Layer 2 — Common list navigation (Branches, Commits,     │
│           Diff, Scope, NOT text-input panels)            │
│   j / ↓      Move down (cycle at bottom)                │
│   k / ↑      Move up (cycle at top)                     │
│   h          Focus previous panel                       │
│   l          Focus next panel                           │
│   g g        Jump to top (double-tap g)                 │
│   G          Jump to bottom                             │
│   Ctrl+F     Page down                                  │
│   Ctrl+B     Page up                                    │
│   Home       Jump to top                                │
│   End        Jump to bottom                             │
│   PgUp       Page up                                    │
│   PgDn       Page down                                  │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Layer 3 — Text-input panels (Search only)                │
│   Char       Insert literal character at cursor         │
│   →/←        Cursor right/left                          │
│   Ctrl+→/←   Cursor by word boundary                    │
│   Home/End   Cursor to line start / line end            │
│   Backspace  Delete char before cursor                  │
│   Delete     Delete char at cursor                      │
│   Ctrl+V     Paste from clipboard                       │
│   Esc        Clear search + return focus (LIFO pop)     │
│   Enter      Confirm search + focus commits             │
│   Tab        Exit text input + focus next               │
│   Shift+Tab  Exit text input + focus prev               │
│                                                          │
│   NOTE: In text input, j/k/h/l become literal chars.    │
│   This matches vim's insert-mode behavior exactly.       │
└──────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────┐
│ Layer 4 — Panel-specific keys                            │
│                                                          │
│   ── Branches ──                                         │
│   Enter       Select branch (load commits)               │
│   Space       Toggle expand/collapse node                │
│   → / e       Expand focused node                       │
│   ← / c       Collapse focused node                     │
│   n / p       Jump to next/prev sibling at same depth   │
│                                                          │
│   ── Commits ──                                          │
│   Enter       Show diff for selected commit              │
│   Space       Preview diff (no focus change) [new]      │
│   y           Copy short hash (7 chars)                 │
│   Y           Copy full hash                            │
│   g           Toggle full / simplified graph             │
│                                                          │
│   ── Diff ──                                             │
│   n / p       Next/prev changed file                    │
│   Enter       Jump to selected file's diff section      │
│   Space       Return focus to commits (same as Esc)      │
│   ← / →       Horizontal scroll (once word-wrap exists)  │
│                                                          │
│   ── Scope ──                                            │
│   Space       Cycle scope (All → Local → Remote)         │
│   Enter       Cycle scope (same as Space)                │
│   Ctrl+S      Cycle scope (global, works anywhere)       │
└──────────────────────────────────────────────────────────┘
```

### Design principles

1. **Vim keys are universal except in text input.** `j`/`k`/`h`/`l` must work identically in all non-text-input panels. Text input breaks them intentionally — exactly how vim's insert mode works.

2. **`Esc` always means "go back / dismiss".** In search: clear + return. In diff: back to commits. In commits: back to branches. In help modal: close. Implement via a `Vec<Panel>` focus stack pushed/popped by relevant actions.

3. **`/` always means "search".** Globally bound. Focuses search input and clears the current query so the user can start typing immediately.

4. **`Home`/`End` vs `gg`/`G` are complementary.** `Home`/`End` work everywhere with context-sensitive meaning (list jump vs. cursor move). `gg`/`G` are list-only vim alternatives for users who prefer them.

5. **`Space` means "interact without moving".** In branches: toggle (don't navigate away). In commits: preview diff (don't move focus to diff panel — new behavior). In scope: cycle. Enter means "commit the action / navigate".

6. **`n`/`p` mean "next/previous within a sub-collection".** In diff: next/prev file. In branches: next/prev sibling at same depth. Pattern scales to future features (e.g., next/prev blame chunk).

### Implementation approach

1. **Refactor `handle_key` into a layer stack.** Evaluate layers in order: always-global → text-input guard → list-navigation → panel-specific. The first match wins. This replaces the current fallback-chain approach.

2. **Move `j`/`k`/`h`/`l` out of global fallback into `handle_key`.** Currently in `input.rs` lines 154-155. Move them into the common list-navigation layer with an explicit `if focus == Search { skip }` guard. The search panel already has `KeyCode::Char(ch)` which will capture them before the fallback — this just makes the intent explicit.

3. **Add LIFO focus stack to `UiState`.** Push current focus when `Enter` navigates (commits→diff, branches→commits). Pop on `Esc`. `Tab`/`Shift+Tab` bypass the stack (linear cycling as today).

4. **Add `gg` double-tap detection.** Store `last_key` and `last_key_time` in `UiState`. If `g` was pressed within 500ms and current key is also `g`, emit `JumpToTop`. Else buffer the first `g`.

5. **Bind `/` globally in `handle_key` layer 1.** Do not delegate to the focused panel.

