use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use git_log_ratatui::git::GitRepository;
use git_log_ratatui::models::BranchScope;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("git-log-ratatui-{name}-{n}"))
}

fn run_git(dir: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git command failed");
    if !out.status.success() {
        panic!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

struct TempRepo {
    dir: std::path::PathBuf,
}

impl TempRepo {
    fn new(name: &str) -> Self {
        let dir = tmp_dir(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run_git(&dir, &["init", "-b", "main"]);
        run_git(&dir, &["config", "user.name", "Tester"]);
        run_git(&dir, &["config", "user.email", "test@test.com"]);
        TempRepo { dir }
    }

    fn write_file(&self, name: &str, content: &str) {
        std::fs::write(self.dir.join(name), content).unwrap();
    }

    fn commit(&self, message: &str) {
        run_git(&self.dir, &["add", "."]);
        run_git(&self.dir, &["commit", "--allow-empty", "-m", message]);
    }

    fn checkout(&self, branch: &str, create: bool) {
        let mut args = vec!["checkout"];
        if create {
            args.push("-b");
        }
        args.push(branch);
        run_git(&self.dir, &args);
    }

    fn tag(&self, name: &str) {
        run_git(&self.dir, &["tag", name]);
    }

    fn merge(&self, branch: &str) {
        run_git(
            &self.dir,
            &["merge", "--no-ff", branch, "-m", &format!("Merge {branch}")],
        );
    }

    fn rev_parse(&self, refname: &str) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(["rev-parse", "--short", refname])
            .output()
            .expect("git rev-parse failed");
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// Add a remote-tracking ref so `fetch_branches(BranchScope::Remote)` returns it.
    fn add_remote_branch(&self, name: &str, local_branch: &str) {
        let hash = self.rev_parse(local_branch);
        let refname = format!("refs/remotes/origin/{}", name);
        run_git(&self.dir, &["update-ref", &refname, &hash]);
    }

    /// Create a remote HEAD symbolic ref so git2 recognizes remote branches.
    fn set_remote_head(&self, target_branch: &str) {
        run_git(
            &self.dir,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                &format!("refs/remotes/origin/{}", target_branch),
            ],
        );
    }

    fn repo(&self) -> GitRepository {
        GitRepository::open(&self.dir.to_string_lossy()).unwrap()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

// ────────────────────────────────────────────────────────────
// Test 1: fetch_branches returns local and remote branches
// ────────────────────────────────────────────────────────────
#[test]
fn test_fetch_branches_local_and_remote() {
    let tr = TempRepo::new("branches");
    tr.write_file("f.txt", "hello");
    tr.commit("root");
    tr.checkout("feature/login", true);
    tr.commit("feature commit");
    tr.checkout("main", false);
    tr.add_remote_branch("main", "main");
    tr.add_remote_branch("feature/login", "feature/login");
    tr.set_remote_head("main");

    let repo = tr.repo();

    let local = repo.fetch_branches(BranchScope::Local).unwrap();
    let local_names: Vec<_> = local.entries.iter().map(|e| e.name.clone()).collect();
    assert!(local_names.contains(&"main".into()));
    assert!(local_names.contains(&"feature/login".into()));
    assert!(local.entries.iter().all(|e| !e.is_remote));

    let remote = repo.fetch_branches(BranchScope::Remote).unwrap();
    let remote_names: Vec<_> = remote.entries.iter().map(|e| e.name.clone()).collect();
    assert!(remote_names.contains(&"origin/main".into()));
    assert!(remote_names.contains(&"origin/feature/login".into()));
    assert!(remote.entries.iter().all(|e| e.is_remote));

    let all = repo.fetch_branches(BranchScope::All).unwrap();
    assert!(all.entries.len() >= 4);
}

// ────────────────────────────────────────────────────────────
// Test 2: fetch_commits with shorthand branch name
// ────────────────────────────────────────────────────────────
#[test]
fn test_fetch_commits_with_shorthand_branch() {
    let tr = TempRepo::new("shorthand");
    tr.write_file("a.txt", "a");
    tr.commit("commit on main");
    tr.checkout("dev", true);
    tr.write_file("b.txt", "b");
    tr.commit("commit on dev");

    let repo = tr.repo();

    let commits = repo
        .fetch_commits(Some("dev"), BranchScope::Local, None)
        .unwrap();
    assert!(!commits.is_empty());
    let first = commits.iter().find(|c| !c.hash.is_empty()).unwrap();
    let short = &first.hash[..first.hash.len().min(7)];
    assert_eq!(short, tr.rev_parse("dev"));
}

// ────────────────────────────────────────────────────────────
// Test 3: fetch_commits_simplified returns same number of commits
// ────────────────────────────────────────────────────────────
#[test]
fn test_simplified_equals_full_commit_count() {
    let tr = TempRepo::new("simplified");
    tr.write_file("x.txt", "x");
    tr.commit("c1");
    tr.commit("c2");
    tr.commit("c3");

    let repo = tr.repo();

    let full = repo.fetch_commits(None, BranchScope::Local, None).unwrap();
    let simpl = repo
        .fetch_commits_simplified(None, BranchScope::Local, None)
        .unwrap();

    let full_non_graph: Vec<_> = full.iter().filter(|c| !c.graph_only).collect();
    let simpl_non_graph: Vec<_> = simpl.iter().filter(|c| !c.graph_only).collect();
    assert_eq!(full_non_graph.len(), simpl_non_graph.len());
}

// ────────────────────────────────────────────────────────────
// Test 4: fetch_diff with existing hash returns correct lines
// ────────────────────────────────────────────────────────────
#[test]
fn test_fetch_diff_returns_lines() {
    let tr = TempRepo::new("diff");
    tr.write_file("hello.txt", "hello world");
    tr.commit("initial");

    let repo = tr.repo();
    let commits = repo
        .fetch_commits_simplified(None, BranchScope::Local, None)
        .unwrap();
    let hash = &commits.iter().find(|c| !c.hash.is_empty()).unwrap().hash;

    let (lines, files) = repo.fetch_diff(hash).unwrap();
    assert!(!lines.is_empty(), "diff should have lines");
    assert!(!files.is_empty(), "should have at least one file entry");

    let has_hello = lines.iter().any(|l| l.contains("hello world"));
    assert!(has_hello, "diff should contain 'hello world'");
}

// ────────────────────────────────────────────────────────────
// Test 5: fetch_diff with nonexistent hash returns error
// ────────────────────────────────────────────────────────────
#[test]
fn test_fetch_diff_nonexistent_hash_fails() {
    let tr = TempRepo::new("badhash");
    tr.write_file("f", "x");
    tr.commit("c");

    let repo = tr.repo();
    let result = repo.fetch_diff("0000000000000000000000000000000000000000");
    assert!(result.is_err());
}

// ────────────────────────────────────────────────────────────
// Test 6: fetch_commit_info returns correct author/committer
// ────────────────────────────────────────────────────────────
#[test]
fn test_fetch_commit_info_author_committer() {
    let tr = TempRepo::new("info");
    tr.write_file("z.txt", "z");
    tr.commit("single commit");

    let repo = tr.repo();
    let commits = repo.fetch_commits(None, BranchScope::Local, None).unwrap();
    let hash = &commits.iter().find(|c| !c.hash.is_empty()).unwrap().hash;

    let info = repo.fetch_commit_info(hash).unwrap();
    assert_eq!(info.author_name, "Tester");
    assert_eq!(info.author_email, "test@test.com");
    assert_eq!(info.subject, "single commit");
    assert!(!info.hash.is_empty());
}

// ────────────────────────────────────────────────────────────
// Test 7: Graph with merge commit produces lane colors
// ────────────────────────────────────────────────────────────
#[test]
fn test_merge_commit_produces_graph_lanes() {
    let tr = TempRepo::new("merge");
    tr.write_file("m.txt", "base");
    tr.commit("root");

    tr.checkout("feature/x", true);
    tr.write_file("m.txt", "feature");
    tr.commit("feature work");

    tr.checkout("main", false);
    tr.merge("feature/x");

    let repo = tr.repo();
    let commits = repo.fetch_commits(None, BranchScope::Local, None).unwrap();

    // Find a commit with a merge marker ○
    let has_merge_marker = commits
        .iter()
        .any(|c| c.graph.contains('\u{25CB}') || c.graph.contains('\u{25CF}'));
    assert!(has_merge_marker, "graph should contain commit markers");

    // At least one commit should have colored lanes
    let has_colors = commits
        .iter()
        .any(|c| c.graph_colors.iter().any(|&lane| lane != 255));
    assert!(has_colors, "at least one commit should have lane colors");
}

// ────────────────────────────────────────────────────────────
// Test 8: Decorations (HEAD, tags, local/remote branches)
// ────────────────────────────────────────────────────────────
#[test]
fn test_decorations_on_commits() {
    let tr = TempRepo::new("decos");
    tr.write_file("d.txt", "d");
    tr.commit("tagged commit");
    tr.tag("v1.0");

    tr.checkout("release/1.x", true);
    tr.write_file("d2.txt", "release");
    tr.commit("release commit");

    tr.checkout("main", false);
    tr.add_remote_branch("main", "main");
    tr.set_remote_head("main");

    let repo = tr.repo();
    // decorations test
    let deco_map = repo.build_decoration_map().unwrap();

    assert!(!deco_map.is_empty(), "decoration map should not be empty");

    let has_tag = deco_map.values().any(|decos| {
        decos.iter().any(|d| {
            d.label == "v1.0" && matches!(d.kind, git_log_ratatui::models::DecorationKind::Tag)
        })
    });
    assert!(has_tag, "should have a 'v1.0' tag decoration");

    let has_local_branch = deco_map.values().any(|decos| {
        decos
            .iter()
            .any(|d| matches!(d.kind, git_log_ratatui::models::DecorationKind::LocalBranch))
    });
    assert!(
        has_local_branch,
        "should have at least one local branch decoration"
    );

    let has_remote_branch = deco_map.values().any(|decos| {
        decos.iter().any(|d| {
            matches!(
                d.kind,
                git_log_ratatui::models::DecorationKind::RemoteBranch
            )
        })
    });
    assert!(
        has_remote_branch,
        "should have at least one remote branch decoration"
    );
}
