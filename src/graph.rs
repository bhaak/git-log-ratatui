use std::sync::LazyLock;

use git_graph::print::format::CommitFormat;
use git_graph::settings::{
    BranchOrder, BranchSettings, BranchSettingsDef, Characters, MergePatterns, Settings,
};

use crate::domain::BranchScope;

fn base_settings(include_remote: bool) -> Settings {
    Settings {
        reverse_commit_order: false,
        debug: false,
        compact: false,
        colored: false,
        include_remote,
        format: CommitFormat::OneLine,
        wrapping: None,
        characters: Characters::thin(),
        branch_order: BranchOrder::ShortestFirst(true),
        branches: BranchSettings::from(BranchSettingsDef::simple())
            .expect("simple branching model is valid"),
        merge_patterns: MergePatterns::default(),
    }
}

static SETTINGS_WITH_REMOTE: LazyLock<Settings> = LazyLock::new(|| base_settings(true));
static SETTINGS_LOCAL_ONLY: LazyLock<Settings> = LazyLock::new(|| base_settings(false));

pub fn create_graph_settings(scope: BranchScope) -> &'static Settings {
    match scope {
        BranchScope::All | BranchScope::Remote | BranchScope::Tags | BranchScope::Stash => {
            &SETTINGS_WITH_REMOTE
        }
        BranchScope::Local => &SETTINGS_LOCAL_ONLY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_graph_settings_all_scope() {
        let settings = create_graph_settings(BranchScope::All);
        assert!(settings.include_remote);
        assert!(!settings.reverse_commit_order);
        assert!(!settings.debug);
        assert!(!settings.compact);
        assert!(!settings.colored);
    }

    #[test]
    fn test_create_graph_settings_local_scope() {
        let settings = create_graph_settings(BranchScope::Local);
        assert!(!settings.include_remote);
    }

    #[test]
    fn test_create_graph_settings_remote_scope() {
        let settings = create_graph_settings(BranchScope::Remote);
        assert!(settings.include_remote);
    }

    #[test]
    fn test_create_graph_settings_tags_scope() {
        let settings = create_graph_settings(BranchScope::Tags);
        assert!(settings.include_remote);
    }

    #[test]
    fn test_create_graph_settings_stash_scope() {
        let settings = create_graph_settings(BranchScope::Stash);
        assert!(settings.include_remote);
    }
}
