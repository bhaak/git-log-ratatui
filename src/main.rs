mod app;
mod clipboard;
mod diff_format;
mod diff_pairing;
mod error;
mod git;
mod graph;
mod lcs;
mod models;
mod text_utils;
mod time_format;
mod tree;
mod ui;
mod workers;

use std::io;

use clap::Parser;
use crossterm::{
    event::DisableMouseCapture,
    event::EnableMouseCapture,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

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
        let mut app = app::App::new(repo_path, cli.simplified_graph, cli.debug)?;
        return app.run_profile(cli.profile_iterations);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let result = {
        let mut app = app::App::new(repo_path, cli.simplified_graph, cli.debug)?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.run(&mut terminal)))
    };

    // Cleanup terminal
    disable_raw_mode()?;
    let _ = io::stdout().execute(LeaveAlternateScreen);
    let _ = io::stdout().execute(DisableMouseCapture);

    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
        Err(panic) => {
            let msg = if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic".to_string()
            };
            eprintln!("Panic: {}", msg);
            std::process::exit(1);
        }
    }

    Ok(())
}
