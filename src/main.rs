mod app;
mod clipboard;
mod config;
mod diff_format;
mod diff_pairing;
mod error;
mod git;
mod graph;
mod lcs;
mod models;
mod state;
mod text_utils;
mod theme;
mod time_format;
mod tree;
mod ui;
mod worker;
mod workers;

use std::io;

use clap::Parser;
use crossterm::{
    event::DisableMouseCapture,
    event::EnableMouseCapture,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use tracing_subscriber::EnvFilter;

use crate::error::AppError;

/// A TUI for browsing git log with ratatui.
#[derive(Parser)]
#[command(name = "git-log-ratatui", version, about)]
struct Cli {
    /// Path to the git repository (defaults to current directory).
    path: Option<String>,
    /// Run in profiling mode: load all data once and print timing.
    #[arg(long)]
    profile: bool,
    /// Number of iterations for profile mode (default: 1).
    #[arg(long, default_value = "1")]
    profile_iterations: u32,
    /// Start with simplified graph (colored bullets, no connecting lines).
    #[arg(long, short = 's')]
    simplified_graph: bool,
    /// Show frame timing in panel titles.
    #[arg(long, short = 'd')]
    debug: bool,
}

fn main() -> Result<(), AppError> {
    std::env::set_var("RUST_BACKTRACE", "full");

    // Write panic backtraces to a file so they survive terminal cleanup.
    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        let output = format!(
            "PANIC: {}\nLocation: {:?}\n\nBacktrace:\n{}",
            info,
            info.location(),
            bt
        );
        eprintln!("{}", output);
        let _ = std::fs::write("/tmp/git-log-ratatui-crash.log", output);
    }));

    // Initialize tracing: logs to /tmp/git-log-ratatui.log.
    // Set RUST_LOG to control verbosity (default: info).
    let file = std::fs::File::create("/tmp/git-log-ratatui.log");
    if let Ok(f) = file {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
            .with_writer(f)
            .try_init();
    }

    let cli = Cli::parse();
    let path = cli.path.unwrap_or_else(|| ".".to_string());

    // Resolve to absolute path
    let abs_path = std::path::Path::new(&path)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(&path));
    let repo_path = abs_path.to_string_lossy().to_string();

    // Check that the path is a git repository (handles worktrees via git2).
    if git2::Repository::discover(&repo_path).is_err() {
        eprintln!("Error: '{}' is not a git repository.", repo_path);
        std::process::exit(1);
    }

    if cli.profile {
        let config = config::Config::load();
        let mut app = app::App::new(repo_path, &config, cli.simplified_graph, cli.debug)?;
        return app.run_profile(cli.profile_iterations);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let config = config::Config::load();
    let mut app = app::App::new(repo_path, &config, cli.simplified_graph, cli.debug)?;

    // Run without catch_unwind so default panic handler prints full backtrace.
    // Terminal cleanup happens below; if a panic occurs above, the terminal
    // state is restored in the Drop guard.
    struct TerminalGuard;

    impl Drop for TerminalGuard {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = io::stdout().execute(LeaveAlternateScreen);
            let _ = io::stdout().execute(DisableMouseCapture);
        }
    }

    let _guard = TerminalGuard;
    app.run(&mut terminal)
}
