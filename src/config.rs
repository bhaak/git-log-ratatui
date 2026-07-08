use std::sync::Arc;

use serde::Deserialize;

use crate::theme::{Theme, ThemeConfig};
use crate::ui::layout;

/// Top-level application configuration loaded from a TOML file.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct RawConfig {
    pub layout: LayoutConfig,
    pub behavior: BehaviorConfig,
    pub theme: ThemeConfig,
}

/// Runtime configuration with parsed Theme.
pub struct Config {
    pub layout: LayoutConfig,
    pub behavior: BehaviorConfig,
    pub theme: Arc<Theme>,
}

impl Default for Config {
    fn default() -> Self {
        RawConfig::default().into()
    }
}

impl From<RawConfig> for Config {
    fn from(raw: RawConfig) -> Self {
        Config {
            layout: raw.layout,
            behavior: raw.behavior,
            theme: Arc::new(raw.theme.into_theme()),
        }
    }
}

impl Config {
    /// Load configuration from the default path.
    ///
    /// Searches `$XDG_CONFIG_HOME/git-log-ratatui/config.toml`, falling back to
    /// `$HOME/.config/git-log-ratatui/config.toml`.
    /// Returns `Config::default()` if no config file exists.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let raw: RawConfig = toml::from_str(&content).unwrap_or_default();
                raw.into()
            }
            Err(_) => Config::default(),
        }
    }
}

/// Layout-related configuration (initial panel sizes, limits).
#[derive(Deserialize)]
pub struct LayoutConfig {
    #[serde(default = "default_branch_width")]
    pub branch_width_pct: u16,
    #[serde(default = "default_diff_height")]
    pub diff_height_pct: u16,
}

/// Behavior-related configuration (commit loading, polling, flags).
#[derive(Deserialize)]
pub struct BehaviorConfig {
    #[serde(default = "default_commit_batch_size")]
    pub commit_batch_size: usize,
    #[serde(default = "default_poll_min_ms")]
    pub poll_min_ms: u8,
    #[serde(default = "default_poll_max_ms")]
    #[allow(dead_code)]
    pub poll_max_ms: u8,
    #[serde(default)]
    pub simplified_graph_default: bool,
    #[serde(default)]
    pub debug_default: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        LayoutConfig {
            branch_width_pct: default_branch_width(),
            diff_height_pct: default_diff_height(),
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        BehaviorConfig {
            commit_batch_size: default_commit_batch_size(),
            poll_min_ms: default_poll_min_ms(),
            poll_max_ms: default_poll_max_ms(),
            simplified_graph_default: false,
            debug_default: false,
        }
    }
}

/// Resolve the TOML config file path.
fn config_path() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        return std::path::PathBuf::from(dir).join("git-log-ratatui/config.toml");
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".config/git-log-ratatui/config.toml");
    }
    std::path::PathBuf::from("config.toml")
}

fn default_branch_width() -> u16 {
    layout::DEFAULT_BRANCH_PCT
}

fn default_diff_height() -> u16 {
    layout::DEFAULT_DIFF_PCT
}

fn default_commit_batch_size() -> usize {
    5000
}

fn default_poll_min_ms() -> u8 {
    10
}

fn default_poll_max_ms() -> u8 {
    200
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_branch_width_matches_layout() {
        assert_eq!(default_branch_width(), layout::DEFAULT_BRANCH_PCT);
    }

    #[test]
    fn test_default_diff_height_matches_layout() {
        assert_eq!(default_diff_height(), layout::DEFAULT_DIFF_PCT);
    }

    #[test]
    fn test_default_commit_batch_size() {
        assert_eq!(default_commit_batch_size(), 5000);
    }

    #[test]
    fn test_default_poll_min_ms() {
        assert_eq!(default_poll_min_ms(), 10);
    }

    #[test]
    fn test_default_poll_max_ms() {
        assert_eq!(default_poll_max_ms(), 200);
    }

    #[test]
    fn test_config_default_produces_non_panicking_config() {
        let config = Config::default();
        assert!(config.theme.graph_colors.len() == 8);
    }

    #[test]
    fn test_raw_config_to_config_roundtrip() {
        let raw = RawConfig::default();
        let config = Config::from(raw);
        assert_eq!(config.layout.branch_width_pct, default_branch_width());
        assert_eq!(
            config.behavior.commit_batch_size,
            default_commit_batch_size()
        );
    }

    #[test]
    fn test_layout_config_default() {
        let layout = LayoutConfig::default();
        assert_eq!(layout.branch_width_pct, default_branch_width());
        assert_eq!(layout.diff_height_pct, default_diff_height());
    }

    #[test]
    fn test_behavior_config_default() {
        let behavior = BehaviorConfig::default();
        assert_eq!(behavior.commit_batch_size, 5000);
        assert_eq!(behavior.poll_min_ms, 10);
        assert!(!behavior.simplified_graph_default);
        assert!(!behavior.debug_default);
    }

    #[test]
    fn test_config_path_with_xdg_set() {
        std::env::set_var("XDG_CONFIG_HOME", "/custom/xdg");
        let path = config_path();
        assert!(path
            .to_string_lossy()
            .contains("/custom/xdg/git-log-ratatui"));
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn test_config_path_falls_back_to_home() {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::set_var("HOME", "/home/testuser");
        let path = config_path();
        assert!(path
            .to_string_lossy()
            .contains("/home/testuser/.config/git-log-ratatui"));
        std::env::remove_var("HOME");
    }

    #[test]
    fn test_config_path_no_vars_fallback() {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("HOME");
        let path = config_path();
        assert_eq!(path, std::path::PathBuf::from("config.toml"));
    }

    #[test]
    fn test_config_load_returns_default_when_no_file() {
        // Set a non-existent path to force fallback
        std::env::set_var("XDG_CONFIG_HOME", "/nonexistent/path/that/does/not/exist");
        let config = Config::load();
        assert!(config.theme.graph_colors.len() == 8);
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
