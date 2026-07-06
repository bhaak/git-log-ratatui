use std::sync::mpsc;
use std::thread;

use crate::error::AppError;
use crate::git_repository::GitRepository;
use crate::models::*;

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

/// Worker thread for branch panel data.
/// Each window gets its own thread as required by AGENTS.md.
pub struct BranchWorker {
    cmd_tx: mpsc::Sender<BranchCommand>,
    result_rx: mpsc::Receiver<BranchResult>,
}

impl BranchWorker {
    /// Spawn a new branch worker thread with its own git repository.
    pub fn new(repo_path: &str) -> Result<Self, AppError> {
        let repo = GitRepository::open(repo_path)?;
        let (cmd_tx, cmd_rx) = mpsc::channel::<BranchCommand>();
        let (result_tx, result_rx) = mpsc::channel::<BranchResult>();

        thread::spawn(move || {
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
        });

        Ok(BranchWorker { cmd_tx, result_rx })
    }

    /// Send a command to the branch worker.
    pub fn send(&self, cmd: BranchCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// Try to receive a result (non-blocking).
    pub fn try_recv(&self) -> Option<BranchResult> {
        self.result_rx.try_recv().ok()
    }

    /// Receive a result (blocking).
    pub fn recv(&self) -> Option<BranchResult> {
        self.result_rx.recv().ok()
    }
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

/// Worker thread for commit table data.
/// Each window gets its own thread as required by AGENTS.md.
pub struct CommitWorker {
    cmd_tx: mpsc::Sender<CommitCommand>,
    result_rx: mpsc::Receiver<CommitResult>,
}

impl CommitWorker {
    /// Spawn a new commit worker thread with its own git repository.
    pub fn new(repo_path: &str) -> Result<Self, AppError> {
        let repo = GitRepository::open(repo_path)?;
        let (cmd_tx, cmd_rx) = mpsc::channel::<CommitCommand>();
        let (result_tx, result_rx) = mpsc::channel::<CommitResult>();

        thread::spawn(move || {
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
        });

        Ok(CommitWorker { cmd_tx, result_rx })
    }

    /// Send a command to the commit worker. Returns false if the channel is closed.
    pub fn send(&self, cmd: CommitCommand) -> bool {
        self.cmd_tx.send(cmd).is_ok()
    }

    /// Try to receive a result (non-blocking).
    pub fn try_recv(&self) -> Option<CommitResult> {
        self.result_rx.try_recv().ok()
    }

    /// Receive a result (blocking).
    pub fn recv(&self) -> Option<CommitResult> {
        self.result_rx.recv().ok()
    }
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

/// Worker thread for diff panel data.
/// Each window gets its own thread as required by AGENTS.md.
pub struct DiffWorker {
    cmd_tx: mpsc::Sender<DiffCommand>,
    result_rx: mpsc::Receiver<DiffResult>,
}

impl DiffWorker {
    /// Spawn a new diff worker thread with its own git repository.
    pub fn new(repo_path: &str) -> Result<Self, AppError> {
        let repo = GitRepository::open(repo_path)?;
        let (cmd_tx, cmd_rx) = mpsc::channel::<DiffCommand>();
        let (result_tx, result_rx) = mpsc::channel::<DiffResult>();

        thread::spawn(move || {
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
        });

        Ok(DiffWorker { cmd_tx, result_rx })
    }

    /// Send a command to the diff worker.
    pub fn send(&self, cmd: DiffCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// Try to receive a result (non-blocking).
    pub fn try_recv(&self) -> Option<DiffResult> {
        self.result_rx.try_recv().ok()
    }

    /// Receive a result (blocking).
    pub fn recv(&self) -> Option<DiffResult> {
        self.result_rx.recv().ok()
    }
}
