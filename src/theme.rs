use ratatui::style::Color;
use serde::Deserialize;

use crate::color_depth::ColorDepth;

/// Customizable color theme for the application.
/// Stores colors adapted to the detected terminal color depth.
#[derive(Clone)]
pub struct Theme {
    pub focused_border: Color,
    pub unfocused_border: Color,
    pub selected_bg: Color,
    pub unselected_bg: Color,
    pub graph_colors: [Color; 8],
    pub commit_merge: Color,
    pub commit_default: Color,
    pub commit_secondary: Color,
    pub text_primary: Color,
    pub decoration_tag: Color,
    pub decoration_local: Color,
    pub decoration_remote: Color,
    pub decoration_head: Color,
    pub diff_added: Color,
    pub diff_added_bg: Color,
    pub diff_removed: Color,
    pub diff_removed_bg: Color,
    pub diff_modified: Color,
    pub diff_renamed: Color,
    pub diff_hunk_header: Color,
    pub diff_file_header: Color,
    pub diff_context: Color,
    pub diff_selected_file_bg: Color,
    pub diff_selected_file_fg: Color,
    pub diff_selected_file_border: Color,
    pub search_cursor_fg: Color,
    pub search_cursor_bg: Color,
    pub help_title: Color,
    pub scrollbar_thumb: Color,
    #[allow(dead_code)]
    pub scrollbar_track: Color,
    pub scope_text: Color,
}

/// Deserializable theme configuration with hex color strings.
/// Each field defaults to the built-in palette when omitted.
#[derive(Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub focused_border: String,
    pub unfocused_border: String,
    pub selected_bg: String,
    pub unselected_bg: String,
    pub graph_color_0: String,
    pub graph_color_1: String,
    pub graph_color_2: String,
    pub graph_color_3: String,
    pub graph_color_4: String,
    pub graph_color_5: String,
    pub graph_color_6: String,
    pub graph_color_7: String,
    pub commit_merge: String,
    pub commit_default: String,
    pub commit_secondary: String,
    pub text_primary: String,
    pub decoration_tag: String,
    pub decoration_local: String,
    pub decoration_remote: String,
    pub decoration_head: String,
    pub diff_added: String,
    pub diff_added_bg: String,
    pub diff_removed: String,
    pub diff_removed_bg: String,
    pub diff_modified: String,
    pub diff_renamed: String,
    pub diff_hunk_header: String,
    pub diff_file_header: String,
    pub diff_context: String,
    pub diff_selected_file_bg: String,
    pub diff_selected_file_fg: String,
    pub diff_selected_file_border: String,
    pub search_cursor_fg: String,
    pub search_cursor_bg: String,
    pub help_title: String,
    pub scrollbar_thumb: String,
    pub scrollbar_track: String,
    pub scope_text: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        ThemeConfig {
            focused_border: "#B48CFF".into(),
            unfocused_border: "#808080".into(),
            selected_bg: "#503C78".into(),
            unselected_bg: "#404040".into(),
            graph_color_0: "#CC664D".into(),
            graph_color_1: "#CC994D".into(),
            graph_color_2: "#CCCC4D".into(),
            graph_color_3: "#4DCC66".into(),
            graph_color_4: "#4DCCCC".into(),
            graph_color_5: "#4D99CC".into(),
            graph_color_6: "#994DCC".into(),
            graph_color_7: "#CC4D99".into(),
            commit_merge: "#FFFF00".into(),
            commit_default: "#FFFFFF".into(),
            commit_secondary: "#808080".into(),
            text_primary: "#FFFFFF".into(),
            decoration_tag: "#FFFF00".into(),
            decoration_local: "#008000".into(),
            decoration_remote: "#FF0000".into(),
            decoration_head: "#90EE90".into(),
            diff_added: "#008000".into(),
            diff_added_bg: "#003200".into(),
            diff_removed: "#FF0000".into(),
            diff_removed_bg: "#320000".into(),
            diff_modified: "#FFFF00".into(),
            diff_renamed: "#0000FF".into(),
            diff_hunk_header: "#00FFFF".into(),
            diff_file_header: "#FFFF00".into(),
            diff_context: "#808080".into(),
            diff_selected_file_bg: "#FFFFFF".into(),
            diff_selected_file_fg: "#000000".into(),
            diff_selected_file_border: "#6496FF".into(),
            search_cursor_fg: "#000000".into(),
            search_cursor_bg: "#B48CFF".into(),
            help_title: "#FFFF00".into(),
            scrollbar_thumb: "#C89664".into(),
            scrollbar_track: "#404040".into(),
            scope_text: "#FFFF00".into(),
        }
    }
}

impl ThemeConfig {
    /// Convert hex string configuration into a runtime Theme using the given color depth.
    pub fn into_theme(self, depth: ColorDepth) -> Theme {
        let c = |hex: &str| {
            let (r, g, b) = parse_hex(hex);
            depth.rgb_to_color(r, g, b)
        };
        Theme {
            focused_border: c(&self.focused_border),
            unfocused_border: c(&self.unfocused_border),
            selected_bg: c(&self.selected_bg),
            unselected_bg: c(&self.unselected_bg),
            graph_colors: [
                c(&self.graph_color_0),
                c(&self.graph_color_1),
                c(&self.graph_color_2),
                c(&self.graph_color_3),
                c(&self.graph_color_4),
                c(&self.graph_color_5),
                c(&self.graph_color_6),
                c(&self.graph_color_7),
            ],
            commit_merge: c(&self.commit_merge),
            commit_default: c(&self.commit_default),
            commit_secondary: c(&self.commit_secondary),
            text_primary: c(&self.text_primary),
            decoration_tag: c(&self.decoration_tag),
            decoration_local: c(&self.decoration_local),
            decoration_remote: c(&self.decoration_remote),
            decoration_head: c(&self.decoration_head),
            diff_added: c(&self.diff_added),
            diff_added_bg: c(&self.diff_added_bg),
            diff_removed: c(&self.diff_removed),
            diff_removed_bg: c(&self.diff_removed_bg),
            diff_modified: c(&self.diff_modified),
            diff_renamed: c(&self.diff_renamed),
            diff_hunk_header: c(&self.diff_hunk_header),
            diff_file_header: c(&self.diff_file_header),
            diff_context: c(&self.diff_context),
            diff_selected_file_bg: c(&self.diff_selected_file_bg),
            diff_selected_file_fg: c(&self.diff_selected_file_fg),
            diff_selected_file_border: c(&self.diff_selected_file_border),
            search_cursor_fg: c(&self.search_cursor_fg),
            search_cursor_bg: c(&self.search_cursor_bg),
            help_title: c(&self.help_title),
            scrollbar_thumb: c(&self.scrollbar_thumb),
            scrollbar_track: c(&self.scrollbar_track),
            scope_text: c(&self.scope_text),
        }
    }
}

fn parse_hex(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return (128, 128, 128);
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
    (r, g, b)
}

impl Default for Theme {
    fn default() -> Self {
        ThemeConfig::default().into_theme(ColorDepth::TrueColor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_depth::ColorDepth;

    #[test]
    fn test_parse_hex_valid_red() {
        assert_eq!(parse_hex("#FF0000"), (255, 0, 0));
    }

    #[test]
    fn test_parse_hex_valid_green() {
        assert_eq!(parse_hex("#00FF00"), (0, 255, 0));
    }

    #[test]
    fn test_parse_hex_valid_blue() {
        assert_eq!(parse_hex("#0000FF"), (0, 0, 255));
    }

    #[test]
    fn test_parse_hex_valid_mixed() {
        assert_eq!(parse_hex("#1A2B3C"), (0x1A, 0x2B, 0x3C));
    }

    #[test]
    fn test_parse_hex_valid_lowercase() {
        assert_eq!(parse_hex("#abcdef"), (0xAB, 0xCD, 0xEF));
    }

    #[test]
    fn test_parse_hex_no_hash_prefix() {
        assert_eq!(parse_hex("FF0000"), (255, 0, 0));
    }

    #[test]
    fn test_parse_hex_invalid_len_too_short_fallback() {
        assert_eq!(parse_hex("#FFF"), (128, 128, 128));
    }

    #[test]
    fn test_parse_hex_invalid_len_too_long_fallback() {
        assert_eq!(parse_hex("#FF0000FF"), (128, 128, 128));
    }

    #[test]
    fn test_parse_hex_empty_string_fallback() {
        assert_eq!(parse_hex(""), (128, 128, 128));
    }

    #[test]
    fn test_parse_hex_invalid_chars_fallback() {
        assert_eq!(parse_hex("#GGGGGG"), (128, 128, 128));
    }

    #[test]
    fn test_parse_hex_partial_invalid_chars() {
        assert_eq!(parse_hex("#FF00ZZ"), (255, 0, 128));
    }

    #[test]
    fn test_theme_config_into_theme_produces_graph_colors() {
        let theme = ThemeConfig::default().into_theme(ColorDepth::TrueColor);
        assert_eq!(theme.graph_colors.len(), 8);
    }

    #[test]
    fn test_theme_default_produces_valid_theme() {
        let theme = Theme::default();
        assert_eq!(theme.graph_colors.len(), 8);
    }
}
