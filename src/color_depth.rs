use ratatui::style::Color;
use terminfo::{capability as cap, Database};

/// Terminal color depth support levels.
/// Detected automatically from the terminfo database at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    /// No ANSI color output (NO_COLOR environment variable set).
    NoColor,
    /// 8 standard ANSI colors (black, red, green, yellow, blue, magenta, cyan, white)
    Color8,
    /// 16 ANSI colors (8 standard + 8 bright variants)
    Color16,
    /// 256-color palette (standard xterm-256)
    Color256,
    /// 24-bit true color (16.7 million colors)
    TrueColor,
}

impl ColorDepth {
    /// Detect terminal color depth from environment variables and terminfo.
    ///
    /// Checks `NO_COLOR` first (https://no-color.org), then `COLORTERM`
    /// for truecolor indicators, then queries the terminfo database
    /// via `TERM` for the `MaxColors` capability. Falls back to 8-color.
    pub fn detect() -> Self {
        if is_no_color() {
            return Self::NoColor;
        }
        if is_truecolor() {
            return Self::TrueColor;
        }
        terminfo_colors()
    }

    /// Convert RGB components to the best Color representation for this depth.
    /// Returns `Color::Reset` when NO_COLOR is active.
    pub fn rgb_to_color(&self, r: u8, g: u8, b: u8) -> Color {
        match self {
            Self::NoColor => Color::Reset,
            Self::TrueColor => Color::Rgb(r, g, b),
            Self::Color256 => rgb_to_color256(r, g, b),
            Self::Color16 => rgb_to_color16(r, g, b),
            Self::Color8 => rgb_to_color8(r, g, b),
        }
    }

    /// Convert a pre-existing `Color::Rgb` to the appropriate representation.
    /// Non-RGB colors are returned unchanged, except when NO_COLOR is active.
    #[allow(dead_code)]
    pub fn adapt_color(&self, color: Color) -> Color {
        match self {
            Self::NoColor => Color::Reset,
            _ => match color {
                Color::Rgb(r, g, b) => self.rgb_to_color(r, g, b),
                other => other,
            },
        }
    }
}

fn is_no_color() -> bool {
    std::env::var("NO_COLOR")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

fn is_truecolor() -> bool {
    if let Ok(val) = std::env::var("COLORTERM") {
        if val == "truecolor" || val == "24bit" {
            return true;
        }
    }
    false
}

/// Query the terminfo database for the `MaxColors` capability.
/// Maps the returned count to the appropriate ColorDepth.
fn terminfo_colors() -> ColorDepth {
    let db = match Database::from_env() {
        Ok(db) => db,
        Err(_) => return ColorDepth::Color8,
    };
    match db.get::<cap::MaxColors>() {
        Some(cap::MaxColors(n)) if n >= 256 => ColorDepth::Color256,
        Some(cap::MaxColors(n)) if n >= 16 => ColorDepth::Color16,
        Some(cap::MaxColors(n)) if n >= 8 => ColorDepth::Color8,
        _ => ColorDepth::Color8,
    }
}

/// Approximate an RGB color using the 256-color palette.
/// Uses the standard xterm 256-color palette:
/// - Colors 0-15: system ANSI colors
/// - Colors 16-231: 6x6x6 RGB cube (36 steps per channel)
/// - Colors 232-255: 24 grayscale steps
fn rgb_to_color256(r: u8, g: u8, b: u8) -> Color {
    let ri = rgb_to_cube_index(r);
    let gi = rgb_to_cube_index(g);
    let bi = rgb_to_cube_index(b);
    let _cube = 16 + 36 * ri as u8 + 6 * gi as u8 + bi as u8;

    let _gray = 8 + (10 * rgb_to_gray(r, g, b)) as u8;

    let cube_r = cube_index_to_rgb(ri as u8);
    let cube_g = cube_index_to_rgb(gi as u8);
    let cube_b = cube_index_to_rgb(bi as u8);
    let cube_dist = color_distance(r, g, b, cube_r, cube_g, cube_b);

    let gray_val = gray_index_to_rgb(rgb_to_gray(r, g, b));
    let gray_dist = color_distance(r, g, b, gray_val, gray_val, gray_val);

    if cube_dist <= gray_dist {
        Color::Indexed(_cube)
    } else {
        Color::Indexed(232 + rgb_to_gray(r, g, b) as u8)
    }
}

/// Convert an RGB component to the nearest 6x6x6 cube index (0-5).
fn rgb_to_cube_index(v: u8) -> usize {
    match v {
        0..=47 => 0,
        48..=114 => 1,
        115..=154 => 2,
        155..=194 => 3,
        195..=234 => 4,
        _ => 5,
    }
}

/// Convert a cube index to the corresponding RGB component.
fn cube_index_to_rgb(i: u8) -> u8 {
    match i {
        0 => 0,
        1 => 95,
        2 => 135,
        3 => 175,
        4 => 215,
        _ => 255,
    }
}

/// Map an RGB color to a grayscale level (0-23).
fn rgb_to_gray(r: u8, g: u8, b: u8) -> usize {
    let gray = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
    let level = (gray * 23 + 127) / 255;
    level as usize
}

/// Convert a grayscale index to the corresponding gray value.
fn gray_index_to_rgb(idx: usize) -> u8 {
    let value = (idx * 10) as u32 + 8;
    value as u8
}

/// Squared distance between two RGB colors.
fn color_distance(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> u32 {
    let dr = r1 as i32 - r2 as i32;
    let dg = g1 as i32 - g2 as i32;
    let db = b1 as i32 - b2 as i32;
    (dr * dr + dg * dg + db * db) as u32
}

/// 8 standard ANSI colors with their approximate RGB reference values.
const COLORS_8: [(Color, u8, u8, u8); 8] = [
    (Color::Black, 0, 0, 0),
    (Color::Red, 205, 0, 0),
    (Color::Green, 0, 205, 0),
    (Color::Yellow, 205, 205, 0),
    (Color::Blue, 0, 0, 238),
    (Color::Magenta, 205, 0, 205),
    (Color::Cyan, 0, 205, 205),
    (Color::White, 229, 229, 229),
];

/// 16 ANSI colors (8 standard + 8 bright) with their approximate RGB reference values.
const COLORS_16: [(Color, u8, u8, u8); 16] = [
    (Color::Black, 0, 0, 0),
    (Color::Red, 205, 0, 0),
    (Color::Green, 0, 205, 0),
    (Color::Yellow, 205, 205, 0),
    (Color::Blue, 0, 0, 238),
    (Color::Magenta, 205, 0, 205),
    (Color::Cyan, 0, 205, 205),
    (Color::White, 229, 229, 229),
    (Color::DarkGray, 127, 127, 127),
    (Color::LightRed, 255, 0, 0),
    (Color::LightGreen, 0, 255, 0),
    (Color::LightYellow, 255, 255, 0),
    (Color::LightBlue, 0, 0, 255),
    (Color::LightMagenta, 255, 0, 255),
    (Color::LightCyan, 0, 255, 255),
    (Color::Gray, 255, 255, 255),
];

/// Find the nearest color in a static palette by Euclidean RGB distance.
fn nearest_color(r: u8, g: u8, b: u8, palette: &[(Color, u8, u8, u8)]) -> Color {
    let mut best = palette[0].0;
    let mut best_dist = u32::MAX;
    for &(color, cr, cg, cb) in palette {
        let dist = color_distance(r, g, b, cr, cg, cb);
        if dist < best_dist {
            best_dist = dist;
            best = color;
        }
    }
    best
}

/// Approximate an RGB color using the 8 standard ANSI colors.
fn rgb_to_color8(r: u8, g: u8, b: u8) -> Color {
    nearest_color(r, g, b, &COLORS_8)
}

/// Approximate an RGB color using the 16 ANSI colors (8 standard + 8 bright).
fn rgb_to_color16(r: u8, g: u8, b: u8) -> Color {
    nearest_color(r, g, b, &COLORS_16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_detect_truecolor_from_colorterm() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("NO_COLOR");
        std::env::set_var("COLORTERM", "truecolor");
        std::env::set_var("TERM", "");
        assert_eq!(ColorDepth::detect(), ColorDepth::TrueColor);
        std::env::remove_var("COLORTERM");
    }

    #[test]
    fn test_detect_truecolor_from_24bit() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("NO_COLOR");
        std::env::set_var("COLORTERM", "24bit");
        std::env::set_var("TERM", "");
        assert_eq!(ColorDepth::detect(), ColorDepth::TrueColor);
        std::env::remove_var("COLORTERM");
    }

    #[test]
    fn test_detect_256color_from_terminfo() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("NO_COLOR");
        std::env::remove_var("COLORTERM");
        std::env::set_var("TERM", "xterm-256color");
        assert_eq!(ColorDepth::detect(), ColorDepth::Color256);
    }

    #[test]
    fn test_detect_16color_from_terminfo() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("NO_COLOR");
        std::env::remove_var("COLORTERM");
        std::env::set_var("TERM", "xterm-16color");
        assert_eq!(ColorDepth::detect(), ColorDepth::Color16);
    }

    #[test]
    fn test_detect_8color_from_terminfo() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("NO_COLOR");
        std::env::remove_var("COLORTERM");
        std::env::set_var("TERM", "xterm");
        assert_eq!(ColorDepth::detect(), ColorDepth::Color8);
    }

    #[test]
    fn test_detect_dumb_terminal_falls_back_to_8color() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("NO_COLOR");
        std::env::remove_var("COLORTERM");
        std::env::set_var("TERM", "dumb");
        assert_eq!(ColorDepth::detect(), ColorDepth::Color8);
    }

    #[test]
    fn test_rgb_to_color_truecolor() {
        let depth = ColorDepth::TrueColor;
        assert_eq!(depth.rgb_to_color(255, 0, 0), Color::Rgb(255, 0, 0));
        assert_eq!(depth.rgb_to_color(0, 255, 0), Color::Rgb(0, 255, 0));
        assert_eq!(depth.rgb_to_color(0, 0, 255), Color::Rgb(0, 0, 255));
    }

    #[test]
    fn test_rgb_to_color256_produces_indexed() {
        let depth = ColorDepth::Color256;
        match depth.rgb_to_color(255, 0, 0) {
            Color::Indexed(_) => {}
            other => panic!("Expected Indexed, got {:?}", other),
        }
    }

    #[test]
    fn test_rgb_to_color256_bounds() {
        let depth = ColorDepth::Color256;
        for r in &[0, 64, 128, 192, 255u8] {
            for g in &[0, 64, 128, 192, 255u8] {
                for b in &[0, 64, 128, 192, 255u8] {
                    match depth.rgb_to_color(*r, *g, *b) {
                        Color::Indexed(_n) => {}
                        other => panic!("Expected Indexed, got {:?}", other),
                    }
                }
            }
        }
    }

    #[test]
    fn test_rgb_to_color8_produces_named() {
        let depth = ColorDepth::Color8;
        assert_eq!(depth.rgb_to_color(0, 0, 0), Color::Black);
        assert_eq!(depth.rgb_to_color(255, 0, 0), Color::Red);
        assert_eq!(depth.rgb_to_color(0, 255, 0), Color::Green);
        assert_eq!(depth.rgb_to_color(0, 0, 255), Color::Blue);
    }

    #[test]
    fn test_rgb_to_color16_produces_named() {
        let depth = ColorDepth::Color16;
        assert_eq!(depth.rgb_to_color(0, 0, 0), Color::Black);
        assert_eq!(depth.rgb_to_color(255, 0, 0), Color::LightRed);
        assert_eq!(depth.rgb_to_color(0, 255, 0), Color::LightGreen);
        assert_eq!(depth.rgb_to_color(255, 255, 255), Color::Gray);
    }

    #[test]
    fn test_rgb_to_cube_index_bounds() {
        assert_eq!(rgb_to_cube_index(0), 0);
        assert_eq!(rgb_to_cube_index(47), 0);
        assert_eq!(rgb_to_cube_index(48), 1);
        assert_eq!(rgb_to_cube_index(240), 5);
        assert_eq!(rgb_to_cube_index(255), 5);
    }

    #[test]
    fn test_rgb_to_gray_black() {
        assert_eq!(rgb_to_gray(0, 0, 0), 0);
    }

    #[test]
    fn test_rgb_to_gray_white() {
        assert_eq!(rgb_to_gray(255, 255, 255), 23);
    }

    #[test]
    fn test_color_distance_same() {
        assert_eq!(color_distance(128, 128, 128, 128, 128, 128), 0);
    }

    #[test]
    fn test_color_distance_different() {
        assert!(color_distance(0, 0, 0, 255, 255, 255) > 0);
    }

    #[test]
    fn test_adapt_color_non_rgb_passthrough() {
        let depth = ColorDepth::Color8;
        assert_eq!(depth.adapt_color(Color::White), Color::White);
    }

    #[test]
    fn test_adapt_color_rgb_converts() {
        let depth = ColorDepth::Color8;
        match depth.adapt_color(Color::Rgb(255, 0, 0)) {
            Color::Red => {}
            other => panic!("Expected Red, got {:?}", other),
        }
    }

    #[test]
    fn test_no_color_detected() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("NO_COLOR", "1");
        assert_eq!(ColorDepth::detect(), ColorDepth::NoColor);
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_no_color_empty_value_ignored() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("NO_COLOR", "");
        assert_ne!(ColorDepth::detect(), ColorDepth::NoColor);
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_no_color_takes_priority_over_colorterm() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("NO_COLOR", "1");
        std::env::set_var("COLORTERM", "truecolor");
        assert_eq!(ColorDepth::detect(), ColorDepth::NoColor);
        std::env::remove_var("NO_COLOR");
        std::env::remove_var("COLORTERM");
    }

    #[test]
    fn test_no_color_rgb_to_color_returns_reset() {
        let depth = ColorDepth::NoColor;
        assert_eq!(depth.rgb_to_color(255, 0, 0), Color::Reset);
        assert_eq!(depth.rgb_to_color(0, 0, 0), Color::Reset);
    }

    #[test]
    fn test_no_color_adapt_color_returns_reset() {
        let depth = ColorDepth::NoColor;
        assert_eq!(depth.adapt_color(Color::Rgb(255, 0, 0)), Color::Reset);
        assert_eq!(depth.adapt_color(Color::White), Color::Reset);
    }
}
