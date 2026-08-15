// SPDX-License-Identifier: GPL-3.0-or-later
//! Tokyo Night styling for egui.
//!
//! The palette matches the CLI's escape codes exactly, so the two front-ends
//! read as one program rather than two tools that happen to share a name.

use egui::{FontFamily, FontId, Rounding, Stroke, TextStyle, Visuals};

/// Tokyo Night, as egui colours.
pub mod palette {
    use egui::Color32;

    /// Window and central-panel background.
    pub const BG: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x26);
    /// Deeper background, for inset panels.
    pub const BG_DARK: Color32 = Color32::from_rgb(0x16, 0x16, 0x1e);
    /// Raised surface, for widgets.
    pub const SURFACE: Color32 = Color32::from_rgb(0x24, 0x28, 0x3b);
    /// Hovered surface.
    pub const SURFACE_HI: Color32 = Color32::from_rgb(0x2f, 0x33, 0x49);
    /// Borders and separators.
    pub const BORDER: Color32 = Color32::from_rgb(0x41, 0x48, 0x68);
    /// Primary text.
    pub const FG: Color32 = Color32::from_rgb(0xc0, 0xca, 0xf5);
    /// Secondary text.
    pub const MUTED: Color32 = Color32::from_rgb(0x56, 0x5f, 0x89);
    /// Accent — the project's primary colour.
    pub const BLUE: Color32 = Color32::from_rgb(0x7a, 0xa2, 0xf7);
    /// Values and figures.
    pub const CYAN: Color32 = Color32::from_rgb(0x7d, 0xcf, 0xff);
    /// The "veiled" half of the mark.
    pub const PURPLE: Color32 = Color32::from_rgb(0xbb, 0x9a, 0xf7);
    /// Success.
    pub const GREEN: Color32 = Color32::from_rgb(0x9e, 0xce, 0x6a);
    /// Warning.
    pub const YELLOW: Color32 = Color32::from_rgb(0xe0, 0xaf, 0x68);
    /// Error.
    pub const RED: Color32 = Color32::from_rgb(0xf7, 0x76, 0x8e);
}

/// Places JetBrains Mono is normally installed.
///
/// The font is not vendored: it is Apache-2.0 and redistributable, but shipping
/// a binary blob would undercut the claim that everything here is auditable
/// from source. If it is installed the UI uses it; otherwise egui's own
/// monospace face stands in, which looks close enough that nothing breaks.
const JETBRAINS_MONO_PATHS: &[&str] = &[
    // Windows
    r"C:\Windows\Fonts\JetBrainsMono-Regular.ttf",
    // macOS
    "/Library/Fonts/JetBrainsMono-Regular.ttf",
    "/System/Library/Fonts/JetBrainsMono-Regular.ttf",
    // Linux
    "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
    "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
    "/usr/share/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf",
];

fn user_font_paths() -> Vec<std::path::PathBuf> {
    let mut paths: Vec<std::path::PathBuf> = JETBRAINS_MONO_PATHS
        .iter()
        .map(std::path::PathBuf::from)
        .collect();
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = std::path::PathBuf::from(home);
        paths.push(home.join(".local/share/fonts/JetBrainsMono-Regular.ttf"));
        paths.push(home.join("AppData/Local/Microsoft/Windows/Fonts/JetBrainsMono-Regular.ttf"));
        paths.push(home.join("Library/Fonts/JetBrainsMono-Regular.ttf"));
    }
    paths
}

/// Load JetBrains Mono if the system has it. Returns whether it was found.
pub fn install_fonts(ctx: &egui::Context) -> bool {
    for path in user_font_paths() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("jetbrains".to_owned(), egui::FontData::from_owned(bytes));
        for family in [FontFamily::Monospace, FontFamily::Proportional] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, "jetbrains".to_owned());
        }
        ctx.set_fonts(fonts);
        return true;
    }
    false
}

/// Apply the Tokyo Night visuals and a monospace-everywhere type scale.
pub fn install(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(palette::FG);
    visuals.panel_fill = palette::BG;
    visuals.window_fill = palette::BG;
    visuals.extreme_bg_color = palette::BG_DARK;
    visuals.faint_bg_color = palette::BG_DARK;
    visuals.window_stroke = Stroke::new(1.0, palette::BORDER);
    visuals.selection.bg_fill = palette::BLUE.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, palette::BLUE);
    visuals.hyperlink_color = palette::CYAN;

    let rounding = Rounding::same(4.0);
    for (widget, fill) in [
        (&mut visuals.widgets.noninteractive, palette::BG_DARK),
        (&mut visuals.widgets.inactive, palette::SURFACE),
        (&mut visuals.widgets.hovered, palette::SURFACE_HI),
        (&mut visuals.widgets.active, palette::SURFACE_HI),
        (&mut visuals.widgets.open, palette::SURFACE),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.rounding = rounding;
        widget.bg_stroke = Stroke::new(1.0, palette::BORDER);
        widget.fg_stroke = Stroke::new(1.0, palette::FG);
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, palette::BLUE);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, palette::BLUE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette::MUTED);

    ctx.set_visuals(visuals);

    // Monospace throughout: this is a tool for people who care what the numbers
    // say, and a fixed advance keeps live readouts from jittering as they
    // update.
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(20.0, FontFamily::Monospace)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Monospace)),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Monospace)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Monospace)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;

    #[test]
    fn palette_matches_the_cli_escape_codes() {
        // Both front-ends must be the same program to look at. These values are
        // duplicated in veilvoice-cli's `theme::colour`.
        assert_eq!(palette::BLUE, Color32::from_rgb(122, 162, 247));
        assert_eq!(palette::GREEN, Color32::from_rgb(158, 206, 106));
        assert_eq!(palette::RED, Color32::from_rgb(247, 118, 142));
        assert_eq!(palette::MUTED, Color32::from_rgb(86, 95, 137));
    }

    #[test]
    fn font_search_covers_every_supported_platform() {
        let paths = user_font_paths();
        assert!(paths
            .iter()
            .any(|p| p.to_string_lossy().contains("Windows")));
        assert!(paths
            .iter()
            .any(|p| p.to_string_lossy().contains("usr/share/fonts")));
        assert!(paths
            .iter()
            .any(|p| p.to_string_lossy().contains("Library/Fonts")));
    }

    #[test]
    fn styling_applies_without_a_window() {
        let ctx = egui::Context::default();
        install(&ctx);
        assert_eq!(ctx.style().visuals.panel_fill, palette::BG);
        assert!(ctx.style().text_styles.contains_key(&TextStyle::Monospace));
    }

    /// Missing JetBrains Mono must degrade to the built-in face, never panic.
    #[test]
    fn missing_font_is_not_fatal() {
        let ctx = egui::Context::default();
        let _found = install_fonts(&ctx);
        install(&ctx);
    }
}
