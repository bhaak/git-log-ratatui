use crate::error::AppError;
use crate::git::GitRepository;
use crate::models::*;
use crate::worker::BackgroundWorker;

// --- Branch worker ---

/// Commands sent to the branch worker thread.
pub enum BranchCommand {
    /// Fetch branches for the given scope.
    FetchBranches { scope: BranchScope },
}

/// Results returned from the branch worker thread.
pub enum BranchResult {
    Branches(BranchData),
    Error(AppError),
}

/// Branch worker type alias.
pub type BranchWorker = BackgroundWorker<BranchCommand, BranchResult>;

/// Spawn a new branch worker thread with its own git repository.
pub fn new_branch_worker(repo_path: &str) -> Result<BranchWorker, AppError> {
    let repo = GitRepository::open(repo_path)?;
    Ok(BackgroundWorker::spawn(
        move |cmd_rx: std::sync::mpsc::Receiver<BranchCommand>,
              result_tx: std::sync::mpsc::Sender<BranchResult>| {
            for cmd in cmd_rx {
                let result = match cmd {
                    BranchCommand::FetchBranches { scope } => match repo.fetch_branches(scope) {
                        Ok(branches) => BranchResult::Branches(branches),
                        Err(e) => BranchResult::Error(e),
                    },
                };
                if result_tx.send(result).is_err() {
                    break;
                }
            }
        },
    ))
}

// --- Commit worker ---

/// Commands sent to the commit worker thread.
pub enum CommitCommand {
    /// Fetch commits for the given branch (None = all) and scope.
    /// `limit` caps how many commits are walked (None = full history).
    /// `simplified` skips git-graph and uses a fast git2 Revwalk path.
    FetchCommits {
        branch: Option<String>,
        scope: BranchScope,
        limit: Option<usize>,
        simplified: bool,
    },
}

/// Results returned from the commit worker thread.
pub enum CommitResult {
    Commits(Vec<Commit>),
    Error(AppError),
}

/// Commit worker type alias.
pub type CommitWorker = BackgroundWorker<CommitCommand, CommitResult>;

/// Spawn a new commit worker thread with its own git repository.
pub fn new_commit_worker(repo_path: &str) -> Result<CommitWorker, AppError> {
    let repo = GitRepository::open(repo_path)?;
    Ok(BackgroundWorker::spawn(
        move |cmd_rx: std::sync::mpsc::Receiver<CommitCommand>,
              result_tx: std::sync::mpsc::Sender<CommitResult>| {
            for cmd in cmd_rx {
                let result = match cmd {
                    CommitCommand::FetchCommits {
                        branch,
                        scope,
                        limit,
                        simplified,
                    } => {
                        let fetch_result = if simplified {
                            repo.fetch_commits_simplified(branch.as_deref(), scope, limit)
                        } else {
                            repo.fetch_commits(branch.as_deref(), scope, limit)
                        };
                        match fetch_result {
                            Ok(commits) => CommitResult::Commits(commits),
                            Err(e) => CommitResult::Error(e),
                        }
                    }
                };
                if result_tx.send(result).is_err() {
                    break;
                }
            }
        },
    ))
}

// --- Diff worker ---

/// Commands sent to the diff worker thread.
pub enum DiffCommand {
    /// Fetch diff and commit info for a commit hash.
    FetchDiff { hash: String },
}

/// Results returned from the diff worker thread.
pub enum DiffResult {
    Diff {
        commit_info: Box<CommitInfo>,
        diff_lines: Vec<String>,
        file_entries: Vec<FileEntry>,
    },
    Error(AppError),
}

/// Diff worker type alias.
pub type DiffWorker = BackgroundWorker<DiffCommand, DiffResult>;

/// Spawn a new diff worker thread with its own git repository.
/// The diff worker drains all queued commands, keeping only the most recent one,
/// to avoid backing up when rapidly scrolling through commits.
pub fn new_diff_worker(repo_path: &str) -> Result<DiffWorker, AppError> {
    let repo = GitRepository::open(repo_path)?;
    Ok(BackgroundWorker::spawn(
        move |cmd_rx: std::sync::mpsc::Receiver<DiffCommand>,
              result_tx: std::sync::mpsc::Sender<DiffResult>| {
            while let Ok(mut cmd) = cmd_rx.recv() {
                // Drain all queued commands, keeping only the most recent one.
                while let Ok(next) = cmd_rx.try_recv() {
                    cmd = next;
                }
                let result = match cmd {
                    DiffCommand::FetchDiff { hash } => {
                        let commit_info = match repo.fetch_commit_info(&hash) {
                            Ok(info) => info,
                            Err(e) => {
                                let _ = result_tx.send(DiffResult::Error(e));
                                continue;
                            }
                        };
                        match repo.fetch_diff(&hash) {
                            Ok((diff_lines, file_entries)) => DiffResult::Diff {
                                commit_info: Box::new(commit_info),
                                diff_lines,
                                file_entries,
                            },
                            Err(e) => DiffResult::Error(e),
                        }
                    }
                };
                if result_tx.send(result).is_err() {
                    break;
                }
            }
        },
    ))
}
