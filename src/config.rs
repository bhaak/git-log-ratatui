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
