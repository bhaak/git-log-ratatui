* General
  - write in English
  - write comments
  - give each window its own thread
  - write unit tests for each step
  - commit after each step
  - changes to TODO.md and AGENTS.md always in their own, per-file separate commit
  - never use —, always use -
  - add documentation to README.md
  - update STRUCTURE.md if the architectural design changes

* Code
  - adhere to these principles: KISS, DRY, YAGNI, SRP, SOC, OCP, LSP, ISP, DIP
  - Composition over Inheritance
  - Fail Fast
  - Principle of Least Astonishment
  - avoid long methods
  - run cargo fmt after each step
  - run clippy after each step

* Performance
  - Performance is top priority.
  - use data streams in parallel processes to process data quickly

* UI
  - use ratatui
  - use standard ratatui widgets
  - consistent mouse support
  - app must be fully keyboard-controllable
  - window sizes resizable via mouse click
  - make colors customizable

* Rust
  - use standard crates, avoid custom implementations
  - use git2
  - tests go in separate files
