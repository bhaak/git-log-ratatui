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
    pub diff_added_word_fg: Color,
    pub diff_added_word_bg: Color,
    pub diff_removed: Color,
    pub diff_removed_word_fg: Color,
    pub diff_removed_word_bg: Color,
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
    pub diff_added_word_fg: Option<String>,
    pub diff_added_word_bg: Option<String>,
    pub diff_removed: String,
    pub diff_removed_bg: String,
    pub diff_removed_word_fg: Option<String>,
    pub diff_removed_word_bg: Option<String>,
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
            diff_added_bg: "#005f00".into(),
            diff_added_word_fg: None,
            diff_added_word_bg: None,
            diff_removed: "#FF0000".into(),
            diff_removed_bg: "#5f0000".into(),
            diff_removed_word_fg: None,
            diff_removed_word_bg: None,
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
    /// Convert hex string configuration into a runtime Theme.
    ///
    /// For Color8 and Color16 terminal depths, returns a static hand-picked
    /// ANSI palette that ignores the config values – on these terminals
    /// nearest-color approximation produces unpredictable results.
    /// TrueColor and Color256 modes handle hex, ANSI index, and named colors.
    pub fn into_theme(self, depth: ColorDepth) -> Theme {
        match depth {
            ColorDepth::NoColor => Theme::no_color(),
            ColorDepth::Color8 => Theme::eight_color(),
            ColorDepth::Color16 => Theme::sixteen_color(),
            ColorDepth::Color256 | ColorDepth::TrueColor => self.into_theme_from_config(depth),
        }
    }

    fn into_theme_from_config(self, depth: ColorDepth) -> Theme {
        let c = |s: &str| {
            let color = parse_color_str(s);
            depth.adapt_color(color)
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
            diff_added_word_fg: c(self
                .diff_added_word_fg
                .as_deref()
                .unwrap_or(&self.diff_added)),
            diff_added_word_bg: c(self
                .diff_added_word_bg
                .as_deref()
                .unwrap_or(&self.diff_added_bg)),
            diff_removed: c(&self.diff_removed),
            diff_removed_word_fg: c(self
                .diff_removed_word_fg
                .as_deref()
                .unwrap_or(&self.diff_removed)),
            diff_removed_word_bg: c(self
                .diff_removed_word_bg
                .as_deref()
                .unwrap_or(&self.diff_removed_bg)),
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

/// Normalize a color name by folding to lowercase and stripping
/// dashes, underscores, and spaces so that "light-green",
/// "light_green", "light green" and "lightgreen" all match.
fn normalize_color_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| *c != '-' && *c != '_' && *c != ' ')
        .collect()
}

/// Parse a color string into a `Color`.
///
/// Supported formats:
/// - `#RRGGBB` hex string
/// - Decimal integer (0-255) → `Color::Indexed` (ANSI 256 palette)
/// - Named colors (case-insensitive, dashes/underscores/spaces optional):
///   `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`,
///   `white`, `gray`/`grey`, `darkgray`/`dark-gray`/`dark gray`,
///   `lightred`/`light-red`/`light red`,
///   `lightgreen`/`light-green`/`light green`,
///   `lightyellow`/`light-yellow`/`light yellow`,
///   `lightblue`/`light-blue`/`light blue`,
///   `lightmagenta`/`light-magenta`/`light magenta`,
///   `lightcyan`/`light-cyan`/`light cyan`, `reset`
/// - Unknown/unparseable values fall back to gray (128, 128, 128).
fn parse_color_str(s: &str) -> Color {
    if s.starts_with('#') {
        let (r, g, b) = parse_hex(s);
        return Color::Rgb(r, g, b);
    }
    if let Ok(idx) = s.parse::<u8>() {
        return Color::Indexed(idx);
    }
    match normalize_color_name(s).as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "reset" => Color::Reset,
        _ => Color::Rgb(128, 128, 128),
    }
}

impl Default for Theme {
    fn default() -> Self {
        ThemeConfig::default().into_theme(ColorDepth::TrueColor)
    }
}

impl Theme {
    /// Static theme for NO_COLOR mode – all colors are `Color::Reset`.
    pub fn no_color() -> Self {
        Theme {
            focused_border: Color::Reset,
            unfocused_border: Color::Reset,
            selected_bg: Color::Reset,
            unselected_bg: Color::Reset,
            graph_colors: [Color::Reset; 8],
            commit_merge: Color::Reset,
            commit_default: Color::Reset,
            commit_secondary: Color::Reset,
            text_primary: Color::Reset,
            decoration_tag: Color::Reset,
            decoration_local: Color::Reset,
            decoration_remote: Color::Reset,
            decoration_head: Color::Reset,
            diff_added: Color::Reset,
            diff_added_word_fg: Color::Reset,
            diff_added_word_bg: Color::Reset,
            diff_removed: Color::Reset,
            diff_removed_word_fg: Color::Reset,
            diff_removed_word_bg: Color::Reset,
            diff_modified: Color::Reset,
            diff_renamed: Color::Reset,
            diff_hunk_header: Color::Reset,
            diff_file_header: Color::Reset,
            diff_context: Color::Reset,
            diff_selected_file_bg: Color::Reset,
            diff_selected_file_fg: Color::Reset,
            diff_selected_file_border: Color::Reset,
            search_cursor_fg: Color::Reset,
            search_cursor_bg: Color::Reset,
            help_title: Color::Reset,
            scrollbar_thumb: Color::Reset,
            scrollbar_track: Color::Reset,
            scope_text: Color::Reset,
        }
    }

    /// Static theme for 8-color terminals.
    /// Each UI element has a hand-picked ANSI color assignment.
    pub fn eight_color() -> Self {
        Theme {
            focused_border: Color::Magenta,
            unfocused_border: Color::White,
            selected_bg: Color::Blue,
            unselected_bg: Color::Blue,
            graph_colors: [
                Color::Red,
                Color::Green,
                Color::Yellow,
                Color::Blue,
                Color::Magenta,
                Color::Cyan,
                Color::White,
                Color::Red,
            ],
            commit_merge: Color::Yellow,
            commit_default: Color::White,
            commit_secondary: Color::Cyan,
            text_primary: Color::White,
            decoration_tag: Color::Yellow,
            decoration_local: Color::Green,
            decoration_remote: Color::Red,
            decoration_head: Color::Cyan,
            diff_added: Color::Green,
            diff_added_word_fg: Color::Green,
            diff_added_word_bg: Color::Black,
            diff_removed: Color::Red,
            diff_removed_word_fg: Color::Red,
            diff_removed_word_bg: Color::Black,
            diff_modified: Color::Yellow,
            diff_renamed: Color::Blue,
            diff_hunk_header: Color::Cyan,
            diff_file_header: Color::Yellow,
            diff_context: Color::White,
            diff_selected_file_bg: Color::White,
            diff_selected_file_fg: Color::Black,
            diff_selected_file_border: Color::Cyan,
            search_cursor_fg: Color::Black,
            search_cursor_bg: Color::Yellow,
            help_title: Color::Yellow,
            scrollbar_thumb: Color::White,
            scrollbar_track: Color::Black,
            scope_text: Color::Yellow,
        }
    }

    /// Static theme for 16-color terminals.
    /// Uses bright ANSI colors for better visual distinction.
    pub fn sixteen_color() -> Self {
        Theme {
            focused_border: Color::LightMagenta,
            unfocused_border: Color::White,
            selected_bg: Color::Blue,
            unselected_bg: Color::Black,
            graph_colors: [
                Color::LightRed,
                Color::LightGreen,
                Color::LightYellow,
                Color::LightBlue,
                Color::LightMagenta,
                Color::LightCyan,
                Color::Red,
                Color::Green,
            ],
            commit_merge: Color::LightYellow,
            commit_default: Color::White,
            commit_secondary: Color::Cyan,
            text_primary: Color::White,
            decoration_tag: Color::LightYellow,
            decoration_local: Color::LightGreen,
            decoration_remote: Color::LightRed,
            decoration_head: Color::Red,
            diff_added: Color::LightGreen,
            diff_added_word_fg: Color::LightGreen,
            diff_added_word_bg: Color::DarkGray,
            diff_removed: Color::LightRed,
            diff_removed_word_fg: Color::LightRed,
            diff_removed_word_bg: Color::DarkGray,
            diff_modified: Color::LightYellow,
            diff_renamed: Color::LightBlue,
            diff_hunk_header: Color::LightCyan,
            diff_file_header: Color::LightYellow,
            diff_context: Color::DarkGray,
            diff_selected_file_bg: Color::Gray,
            diff_selected_file_fg: Color::Black,
            diff_selected_file_border: Color::LightCyan,
            search_cursor_fg: Color::Black,
            search_cursor_bg: Color::White,
            help_title: Color::LightYellow,
            scrollbar_thumb: Color::Gray,
            scrollbar_track: Color::DarkGray,
            scope_text: Color::LightYellow,
        }
    }
}

impl Theme {
    /// Number of built-in preset themes (excluding the custom/startup theme).
    pub const PRESET_COUNT: usize = 4;

    /// Return the name of the i-th built-in preset theme (0-indexed).
    pub fn preset_name(i: usize) -> &'static str {
        match i {
            0 => "Default",
            1 => "Monokai",
            2 => "Nord",
            3 => "Solarized Dark",
            _ => "Unknown",
        }
    }

    /// Return the i-th built-in preset theme (0-indexed).
    /// Panics if i >= PRESET_COUNT.
    pub fn preset(i: usize) -> Self {
        match i {
            0 => Theme::preset_default(),
            1 => Theme::preset_monokai(),
            2 => Theme::preset_nord(),
            3 => Theme::preset_solarized_dark(),
            _ => panic!("preset index out of bounds: {}", i),
        }
    }

    fn preset_default() -> Self {
        Theme::default()
    }

    fn preset_monokai() -> Self {
        Theme {
            focused_border: Color::Rgb(174, 129, 255),
            unfocused_border: Color::Rgb(117, 113, 94),
            selected_bg: Color::Rgb(73, 72, 62),
            unselected_bg: Color::Rgb(39, 40, 34),
            graph_colors: [
                Color::Rgb(249, 38, 114),
                Color::Rgb(166, 226, 46),
                Color::Rgb(230, 219, 116),
                Color::Rgb(102, 217, 239),
                Color::Rgb(174, 129, 255),
                Color::Rgb(253, 151, 31),
                Color::Rgb(117, 113, 94),
                Color::Rgb(249, 38, 114),
            ],
            commit_merge: Color::Rgb(253, 151, 31),
            commit_default: Color::Rgb(248, 248, 242),
            commit_secondary: Color::Rgb(117, 113, 94),
            text_primary: Color::Rgb(248, 248, 242),
            decoration_tag: Color::Rgb(230, 219, 116),
            decoration_local: Color::Rgb(166, 226, 46),
            decoration_remote: Color::Rgb(249, 38, 114),
            decoration_head: Color::Rgb(166, 226, 46),
            diff_added: Color::Rgb(166, 226, 46),
            diff_added_word_fg: Color::Rgb(39, 40, 34),
            diff_added_word_bg: Color::Rgb(166, 226, 46),
            diff_removed: Color::Rgb(249, 38, 114),
            diff_removed_word_fg: Color::Rgb(39, 40, 34),
            diff_removed_word_bg: Color::Rgb(249, 38, 114),
            diff_modified: Color::Rgb(230, 219, 116),
            diff_renamed: Color::Rgb(102, 217, 239),
            diff_hunk_header: Color::Rgb(174, 129, 255),
            diff_file_header: Color::Rgb(230, 219, 116),
            diff_context: Color::Rgb(117, 113, 94),
            diff_selected_file_bg: Color::Rgb(248, 248, 242),
            diff_selected_file_fg: Color::Rgb(39, 40, 34),
            diff_selected_file_border: Color::Rgb(166, 226, 46),
            search_cursor_fg: Color::Rgb(39, 40, 34),
            search_cursor_bg: Color::Rgb(230, 219, 116),
            help_title: Color::Rgb(253, 151, 31),
            scrollbar_thumb: Color::Rgb(174, 129, 255),
            scrollbar_track: Color::Rgb(73, 72, 62),
            scope_text: Color::Rgb(102, 217, 239),
        }
    }

    fn preset_nord() -> Self {
        Theme {
            focused_border: Color::Rgb(136, 192, 208),
            unfocused_border: Color::Rgb(76, 86, 106),
            selected_bg: Color::Rgb(67, 76, 94),
            unselected_bg: Color::Rgb(46, 52, 64),
            graph_colors: [
                Color::Rgb(191, 97, 106),
                Color::Rgb(163, 190, 140),
                Color::Rgb(235, 203, 139),
                Color::Rgb(94, 129, 172),
                Color::Rgb(180, 142, 173),
                Color::Rgb(136, 192, 208),
                Color::Rgb(129, 161, 193),
                Color::Rgb(208, 135, 112),
            ],
            commit_merge: Color::Rgb(235, 203, 139),
            commit_default: Color::Rgb(236, 239, 244),
            commit_secondary: Color::Rgb(129, 161, 193),
            text_primary: Color::Rgb(236, 239, 244),
            decoration_tag: Color::Rgb(235, 203, 139),
            decoration_local: Color::Rgb(163, 190, 140),
            decoration_remote: Color::Rgb(191, 97, 106),
            decoration_head: Color::Rgb(136, 192, 208),
            diff_added: Color::Rgb(163, 190, 140),
            diff_added_word_fg: Color::Rgb(46, 52, 64),
            diff_added_word_bg: Color::Rgb(163, 190, 140),
            diff_removed: Color::Rgb(191, 97, 106),
            diff_removed_word_fg: Color::Rgb(46, 52, 64),
            diff_removed_word_bg: Color::Rgb(191, 97, 106),
            diff_modified: Color::Rgb(235, 203, 139),
            diff_renamed: Color::Rgb(180, 142, 173),
            diff_hunk_header: Color::Rgb(136, 192, 208),
            diff_file_header: Color::Rgb(235, 203, 139),
            diff_context: Color::Rgb(76, 86, 106),
            diff_selected_file_bg: Color::Rgb(216, 222, 233),
            diff_selected_file_fg: Color::Rgb(46, 52, 64),
            diff_selected_file_border: Color::Rgb(136, 192, 208),
            search_cursor_fg: Color::Rgb(46, 52, 64),
            search_cursor_bg: Color::Rgb(235, 203, 139),
            help_title: Color::Rgb(235, 203, 139),
            scrollbar_thumb: Color::Rgb(129, 161, 193),
            scrollbar_track: Color::Rgb(67, 76, 94),
            scope_text: Color::Rgb(136, 192, 208),
        }
    }

    fn preset_solarized_dark() -> Self {
        Theme {
            focused_border: Color::Rgb(133, 153, 0),
            unfocused_border: Color::Rgb(101, 123, 131),
            selected_bg: Color::Rgb(0, 43, 54),
            unselected_bg: Color::Rgb(7, 54, 66),
            graph_colors: [
                Color::Rgb(220, 50, 47),
                Color::Rgb(133, 153, 0),
                Color::Rgb(181, 137, 0),
                Color::Rgb(38, 139, 210),
                Color::Rgb(211, 54, 130),
                Color::Rgb(42, 161, 152),
                Color::Rgb(108, 113, 196),
                Color::Rgb(203, 75, 22),
            ],
            commit_merge: Color::Rgb(181, 137, 0),
            commit_default: Color::Rgb(147, 161, 161),
            commit_secondary: Color::Rgb(101, 123, 131),
            text_primary: Color::Rgb(147, 161, 161),
            decoration_tag: Color::Rgb(181, 137, 0),
            decoration_local: Color::Rgb(133, 153, 0),
            decoration_remote: Color::Rgb(220, 50, 47),
            decoration_head: Color::Rgb(38, 139, 210),
            diff_added: Color::Rgb(133, 153, 0),
            diff_added_word_fg: Color::Rgb(0, 43, 54),
            diff_added_word_bg: Color::Rgb(133, 153, 0),
            diff_removed: Color::Rgb(220, 50, 47),
            diff_removed_word_fg: Color::Rgb(0, 43, 54),
            diff_removed_word_bg: Color::Rgb(220, 50, 47),
            diff_modified: Color::Rgb(181, 137, 0),
            diff_renamed: Color::Rgb(38, 139, 210),
            diff_hunk_header: Color::Rgb(42, 161, 152),
            diff_file_header: Color::Rgb(181, 137, 0),
            diff_context: Color::Rgb(101, 123, 131),
            diff_selected_file_bg: Color::Rgb(238, 232, 213),
            diff_selected_file_fg: Color::Rgb(0, 43, 54),
            diff_selected_file_border: Color::Rgb(133, 153, 0),
            search_cursor_fg: Color::Rgb(0, 43, 54),
            search_cursor_bg: Color::Rgb(181, 137, 0),
            help_title: Color::Rgb(181, 137, 0),
            scrollbar_thumb: Color::Rgb(108, 113, 196),
            scrollbar_track: Color::Rgb(0, 43, 54),
            scope_text: Color::Rgb(42, 161, 152),
        }
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
    fn test_parse_color_str_hex() {
        assert_eq!(parse_color_str("#FF0000"), Color::Rgb(255, 0, 0));
        assert_eq!(parse_color_str("#00FF00"), Color::Rgb(0, 255, 0));
    }

    #[test]
    fn test_parse_color_str_ansi_index() {
        assert_eq!(parse_color_str("52"), Color::Indexed(52));
        assert_eq!(parse_color_str("22"), Color::Indexed(22));
        assert_eq!(parse_color_str("0"), Color::Indexed(0));
        assert_eq!(parse_color_str("255"), Color::Indexed(255));
    }

    #[test]
    fn test_parse_color_str_named() {
        assert_eq!(parse_color_str("red"), Color::Red);
        assert_eq!(parse_color_str("green"), Color::Green);
        assert_eq!(parse_color_str("blue"), Color::Blue);
        assert_eq!(parse_color_str("gray"), Color::Gray);
        assert_eq!(parse_color_str("GREY"), Color::Gray);
        assert_eq!(parse_color_str("lightred"), Color::LightRed);
        assert_eq!(parse_color_str("darkgray"), Color::DarkGray);
    }

    #[test]
    fn test_parse_color_str_dash_variants() {
        assert_eq!(parse_color_str("light-red"), Color::LightRed);
        assert_eq!(parse_color_str("light-green"), Color::LightGreen);
        assert_eq!(parse_color_str("LIGHT-BLUE"), Color::LightBlue);
        assert_eq!(parse_color_str("dark-gray"), Color::DarkGray);
    }

    #[test]
    fn test_parse_color_str_space_variants() {
        assert_eq!(parse_color_str("light red"), Color::LightRed);
        assert_eq!(parse_color_str("light green"), Color::LightGreen);
        assert_eq!(parse_color_str("dark gray"), Color::DarkGray);
    }

    #[test]
    fn test_parse_color_str_underscore_variants() {
        assert_eq!(parse_color_str("light_red"), Color::LightRed);
        assert_eq!(parse_color_str("dark_gray"), Color::DarkGray);
    }

    #[test]
    fn test_parse_color_str_unknown_fallback() {
        assert_eq!(parse_color_str("unknown"), Color::Rgb(128, 128, 128));
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
