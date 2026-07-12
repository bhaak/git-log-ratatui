use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use git_graph::graph::GitGraph;
use rayon::prelude::*;
use tracing::{debug, debug_span};

use crate::domain::{BranchScope, Decoration};
use crate::error::AppError;
use crate::graph::create_graph_settings;
use crate::theme::Theme;
use crate::time_format::time_to_string;
use crate::view::CommitRow;

use super::graph_conv::build_commits_from_graph;
use super::GitRepository;

impl GitRepository {
    /// Fetch commits using git-graph to generate the graph visualization,
    /// then enriches each commit with author/date/subject/merge/decorations from git2.
    pub fn fetch_commits(
        &self,
        branch: Option<&str>,
        scope: BranchScope,
        limit: Option<usize>,
    ) -> Result<Vec<CommitRow>, AppError> {
        let t0 = Instant::now();
        let _span = debug_span!("fetch_commits", ?branch, ?scope, ?limit).entered();
        let decoration_map = self.build_decoration_map()?;

        let settings = create_graph_settings(scope);

        let start_point = branch.map(|b| b.to_string());

        let repo = git2::Repository::open(Path::new(&self.repo_path))?;

        let graph = GitGraph::new(repo, settings, start_point, limit)?;

        let graph_done = t0.elapsed();
        let mut commits = build_commits_from_graph(&graph);
        let enrich_t0 = Instant::now();
        self.enrich_commits(&mut commits, &decoration_map);
        let enrich_done = enrich_t0.elapsed();
        let total = t0.elapsed();
        debug!(
            "commits(git-graph): {} in {}ms (graph={}ms enrich={}ms)",
            commits.len(),
            total.as_millis(),
            graph_done.as_millis(),
            enrich_done.as_millis()
        );
        Ok(commits)
    }

    /// Fast-path fetch that skips git-graph entirely. Walks commits with git2
    /// Revwalk and assigns lane colors based on which branch tip each commit
    /// belongs to. Much faster for the simplified graph view.
    pub fn fetch_commits_simplified(
        &self,
        branch: Option<&str>,
        scope: BranchScope,
        limit: Option<usize>,
    ) -> Result<Vec<CommitRow>, AppError> {
        let t0 = Instant::now();
        let _span = debug_span!("fetch_commits_simplified", ?branch, ?scope, ?limit).entered();
        let decoration_map = self.build_decoration_map()?;
        let branch_tip_colors = self.build_branch_tip_colors(scope, &Theme::default())?;

        let mut revwalk = self.repo.revwalk()?;

        revwalk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)?;

        if let Some(b) = branch {
            let refname = self
                .resolve_branch_ref_name(b)
                .unwrap_or_else(|| format!("refs/heads/{}", b));
            revwalk.push_ref(&refname)?;
        } else {
            self.push_scope_refs(&mut revwalk, scope)?;
        }

        let mut commits: Vec<CommitRow> = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result?;
            if let Some(max) = limit {
                if commits.len() >= max {
                    break;
                }
            }

            let lane = branch_tip_colors.get(&oid).copied().unwrap_or(255);
            let graph = if lane == 255 {
                let merge = self
                    .repo
                    .find_commit(oid)
                    .map(|c| c.parent_count() > 1)
                    .unwrap_or(false);
                let ch = if merge { '○' } else { '●' };
                ch.to_string()
            } else {
                '●'.to_string()
            };

            commits.push(CommitRow {
                hash: oid.to_string(),
                graph,
                graph_colors: vec![lane],
                graph_only: false,
                author: String::new(),
                date: String::new(),
                subject: String::new(),
                merge: false,
                decorations: Vec::new(),
                deco_line: 0,
                epoch_days: 0,
            });
        }

        let walk_done = t0.elapsed();
        let enrich_t0 = Instant::now();
        self.enrich_commits(&mut commits, &decoration_map);
        let enrich_done = enrich_t0.elapsed();
        let total = t0.elapsed();
        debug!(
            "commits(simplified): {} in {}ms (walk={}ms enrich={}ms)",
            commits.len(),
            total.as_millis(),
            walk_done.as_millis(),
            enrich_done.as_millis()
        );
        Ok(commits)
    }

    fn push_scope_refs(
        &self,
        revwalk: &mut git2::Revwalk,
        scope: BranchScope,
    ) -> Result<(), AppError> {
        let refs = self.repo.references()?;

        for r in refs {
            let r = r?;
            if !r.is_branch() && !r.is_tag() {
                continue;
            }
            if scope == BranchScope::Local && r.is_remote() {
                continue;
            }
            if scope == BranchScope::Remote && !r.is_remote() {
                continue;
            }
            if r.is_tag() {
                continue;
            }
            if let Some(name) = r.name() {
                revwalk.push_ref(name)?;
            }
        }
        Ok(())
    }

    /// Fill in author, date, subject, merge status, and decorations from git2.
    fn enrich_commits(
        &self,
        commits: &mut [CommitRow],
        decoration_map: &HashMap<git2::Oid, Vec<Decoration>>,
    ) {
        let t0 = Instant::now();
        let repo_path = self.repo_path.clone();

        commits.par_iter_mut().for_each_init(
            || {
                git2::Repository::open(Path::new(&repo_path))
                    .expect("Failed to open repository for enrich_commits")
            },
            |repo, commit| {
                if commit.graph_only || commit.hash.is_empty() {
                    return;
                }

                let oid = match git2::Oid::from_str(&commit.hash) {
                    Ok(oid) => oid,
                    Err(_) => return,
                };
                let git_commit = match repo.find_commit(oid) {
                    Ok(c) => c,
                    Err(_) => return,
                };

                commit.author = git_commit.author().name().unwrap_or("").to_string();
                commit.date = time_to_string(git_commit.time());
                commit.epoch_days = git_commit.time().seconds() / 86400;
                commit.subject = git_commit.summary().unwrap_or("").to_string();
                commit.merge = git_commit.parent_count() > 1;

                if let Some(decos) = decoration_map.get(&oid) {
                    commit.decorations = decos.clone();
                }
            },
        );

        let mut expanded_idx = 0;
        for commit in commits.iter_mut() {
            if !commit.decorations.is_empty() {
                expanded_idx += 1;
            }
            commit.deco_line = expanded_idx;
            expanded_idx += 1;
        }
        debug!(
            "enrich_commits: {} in {}ms",
            commits.len(),
            t0.elapsed().as_millis()
        );
    }
}
