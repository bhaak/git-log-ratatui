mod app;
mod clipboard;
mod git_repository;
mod lcs;
mod models;
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

/// A TUI for browsing git log with ratatui.
#[derive(Parser)]
#[command(name = "git-log-ratatui", version, about)]
struct Cli {
    /// Path to the git repository (defaults to current directory).
    path: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let path = cli.path.unwrap_or_else(|| ".".to_string());

    // Resolve to absolute path
    let abs_path = std::path::Path::new(&path)
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from(&path));
    let repo_path = abs_path.to_string_lossy().to_string();

    // Check that the path is a git repository
    let git_dir = std::path::Path::new(&repo_path).join(".git");
    let is_repo = git_dir.exists() || std::path::Path::new(&repo_path).join("HEAD").exists();
    if !is_repo {
        // Check if we're inside a worktree
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("rev-parse")
            .arg("--git-dir")
            .output();
        match output {
            Ok(o) if o.status.success() => {}
            _ => {
                eprintln!("Error: '{}' is not a git repository.", repo_path);
                std::process::exit(1);
            }
        }
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let result = {
        let mut app = app::App::new(repo_path)?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.run(&mut terminal)
        }))
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
