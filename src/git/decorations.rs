use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::error::AppError;
use crate::models::*;
use crate::theme::Theme;

use super::GitRepository;

impl GitRepository {
    /// Build a mapping from branch tip OID to lane color index.
    /// Each unique branch gets a color from theme.graph_colors based on its name hash.
    pub fn build_branch_tip_colors(
        &self,
        scope: BranchScope,
        theme: &Theme,
    ) -> Result<HashMap<git2::Oid, u8>, AppError> {
        let mut tips: HashMap<git2::Oid, u8> = HashMap::new();
        let refs = self.repo.references()?;

        for r in refs {
            let r = r?;
            if !r.is_branch() {
                continue;
            }
            if scope == BranchScope::Local && r.is_remote() {
                continue;
            }
            if scope == BranchScope::Remote && !r.is_remote() {
                continue;
            }

            let target_oid = match r.target() {
                Some(oid) => oid,
                None => continue,
            };

            let name = r.shorthand().unwrap_or("");
            if name.is_empty() {
                continue;
            }

            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            name.hash(&mut hasher);
            let color_idx = (hasher.finish() % theme.graph_colors.len() as u64) as u8;

            tips.entry(target_oid).or_insert(color_idx);
        }

        Ok(tips)
    }

    /// Build a map from commit Oid to decorations by iterating all references.
    pub fn build_decoration_map(&self) -> Result<HashMap<git2::Oid, Vec<Decoration>>, AppError> {
        let mut map: HashMap<git2::Oid, Vec<Decoration>> = HashMap::new();
        let refs = self.repo.references()?;

        for r in refs {
            let r = match r {
                Ok(r) => r,
                Err(_) => continue,
            };
            let target_oid = match r
                .target()
                .or_else(|| r.resolve().ok().and_then(|resolved| resolved.target()))
            {
                Some(oid) => oid,
                None => continue,
            };
            let shorthand = r.shorthand().unwrap_or("").to_string();
            if shorthand.is_empty() {
                continue;
            }

            let kind = if r.is_tag() {
                DecorationKind::Tag
            } else if r.is_remote() {
                DecorationKind::RemoteBranch
            } else if r.is_branch() {
                DecorationKind::LocalBranch
            } else {
                DecorationKind::Head
            };

            map.entry(target_oid).or_default().push(Decoration {
                label: shorthand,
                kind,
            });
        }

        for decos in map.values_mut() {
            decos.sort_unstable_by_key(|d| d.kind.priority());
        }

        Ok(map)
    }
}
