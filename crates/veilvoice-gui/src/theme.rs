// SPDX-License-Identifier: GPL-3.0-or-later
//! Colour schemes for the desktop app.
//!
//! # One palette, three front-ends
//!
//! Every theme here is the same set of twelve tokens the website defines in
//! `website/css/themes.css`, with the same names and the same hex values, and
//! Tokyo Night additionally matches the escape codes the CLI emits. The three
//! front-ends are meant to read as one program rather than as three tools that
//! happen to share a name, and the way that is kept true is by copying the
//! numbers rather than by approximating them. Tests assert **every token of
//! every theme** against the stylesheet, in **both directions** -- so a colour
//! changed on either side, a theme removed from either side, or a theme added
//! to the website and forgotten here all fail the build rather than shipping as
//! two products that no longer look alike.
//!
//! # Why the active theme is an index, not a lock
//!
//! Colours are read on every repaint, from the UI thread, dozens of times a
//! frame. A `Mutex` around the palette would be a lock taken hundreds of times
//! a second to read a constant, and a poisoned one would take the window with
//! it. Instead the themes are a `const` array and the selection is a single
//! `AtomicUsize`: reading is one relaxed load, it cannot fail, and it cannot
//! be poisoned.
//!
//! The index is clamped on read as well as on write. A value that somehow got
//! out of range would otherwise panic on a slice index, in a paint loop, which
//! is the worst possible place for it -- so the read saturates to the default
//! instead.
//!
//! # In plain words
//!
//! The colour schemes the application ships with, and the fonts.
//!
//! They are the same schemes the website offers, defined once so the two cannot
//! drift apart. Choosing one applies it straight away and it is remembered for
//! next time.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// The theme table, once the user's palettes have been folded in.
///
/// Empty until [`load_custom`] runs. See [`themes`] for why this is a
/// `OnceLock` rather than anything that can be written twice.
static TABLE: OnceLock<Vec<Theme>> = OnceLock::new();

/// One complete colour scheme.
///
/// The field names match the CSS custom properties one for one: `bg` is
/// `--bg`, `accent_2` is `--accent-2`, and so on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Stable identifier, matching the website's `data-theme` value. This is
    /// what gets written to the preferences file, so it must never change for
    /// an existing theme.
    pub id: &'static str,
    /// Human-readable name, as shown in the picker.
    pub name: &'static str,
    /// Whether this is a light scheme. Drives egui's base `Visuals`.
    pub light: bool,

    /// Window and central-panel background.
    pub bg: Color32,
    /// Raised surface, for widgets.
    pub bg_soft: Color32,
    /// Deeper background, for inset panels.
    pub bg_inset: Color32,
    /// Borders and separators.
    pub border: Color32,
    /// Primary text.
    pub fg: Color32,
    /// Secondary text.
    pub muted: Color32,
    /// The project's primary colour.
    pub accent: Color32,
    /// The "veiled" half of the mark.
    pub accent_2: Color32,
    /// Values and figures.
    pub cyan: Color32,
    /// Success.
    pub ok: Color32,
    /// Warning.
    pub warn: Color32,
    /// Error.
    pub err: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// Every theme, in the order the picker shows them.
///
/// Index 0 is the default and is the one a clamped-out-of-range read lands on.
pub const THEMES: &[Theme] = &[
    Theme {
        id: "tokyo-night",
        name: "Tokyo Night",
        light: false,
        bg: rgb(0x1a1b26),
        bg_soft: rgb(0x1f2335),
        bg_inset: rgb(0x16161e),
        border: rgb(0x2f3549),
        fg: rgb(0xc0caf5),
        muted: rgb(0x737aa2),
        accent: rgb(0x7aa2f7),
        accent_2: rgb(0xbb9af7),
        cyan: rgb(0x7dcfff),
        ok: rgb(0x9ece6a),
        warn: rgb(0xe0af68),
        err: rgb(0xf7768e),
    },
    Theme {
        id: "gruvbox",
        name: "Gruvbox",
        light: false,
        bg: rgb(0x282828),
        bg_soft: rgb(0x32302f),
        bg_inset: rgb(0x1d2021),
        border: rgb(0x504945),
        fg: rgb(0xebdbb2),
        muted: rgb(0x928374),
        accent: rgb(0x83a598),
        accent_2: rgb(0xd3869b),
        cyan: rgb(0x8ec07c),
        ok: rgb(0xb8bb26),
        warn: rgb(0xfabd2f),
        err: rgb(0xfb4934),
    },
    Theme {
        id: "dracula",
        name: "Dracula",
        light: false,
        bg: rgb(0x282a36),
        bg_soft: rgb(0x343746),
        bg_inset: rgb(0x21222c),
        border: rgb(0x44475a),
        fg: rgb(0xf8f8f2),
        muted: rgb(0x6272a4),
        accent: rgb(0xbd93f9),
        accent_2: rgb(0xff79c6),
        cyan: rgb(0x8be9fd),
        ok: rgb(0x50fa7b),
        warn: rgb(0xf1fa8c),
        err: rgb(0xff5555),
    },
    Theme {
        id: "nord",
        name: "Nord",
        light: false,
        bg: rgb(0x2e3440),
        bg_soft: rgb(0x3b4252),
        bg_inset: rgb(0x272c36),
        border: rgb(0x4c566a),
        fg: rgb(0xeceff4),
        muted: rgb(0x7b88a1),
        accent: rgb(0x88c0d0),
        accent_2: rgb(0xb48ead),
        cyan: rgb(0x8fbcbb),
        ok: rgb(0xa3be8c),
        warn: rgb(0xebcb8b),
        err: rgb(0xbf616a),
    },
    Theme {
        id: "catppuccin",
        name: "Catppuccin Mocha",
        light: false,
        bg: rgb(0x1e1e2e),
        bg_soft: rgb(0x313244),
        bg_inset: rgb(0x181825),
        border: rgb(0x45475a),
        fg: rgb(0xcdd6f4),
        muted: rgb(0x7f849c),
        accent: rgb(0x89b4fa),
        accent_2: rgb(0xcba6f7),
        cyan: rgb(0x94e2d5),
        ok: rgb(0xa6e3a1),
        warn: rgb(0xf9e2af),
        err: rgb(0xf38ba8),
    },
    Theme {
        id: "everforest",
        name: "Everforest",
        light: false,
        bg: rgb(0x2d353b),
        bg_soft: rgb(0x343f44),
        bg_inset: rgb(0x272e33),
        border: rgb(0x475258),
        fg: rgb(0xd3c6aa),
        muted: rgb(0x859289),
        accent: rgb(0xa7c080),
        accent_2: rgb(0xd699b6),
        cyan: rgb(0x83c092),
        ok: rgb(0xa7c080),
        warn: rgb(0xdbbc7f),
        err: rgb(0xe67e80),
    },
    Theme {
        id: "solarized",
        name: "Solarized Dark",
        light: false,
        bg: rgb(0x002b36),
        bg_soft: rgb(0x073642),
        bg_inset: rgb(0x00212b),
        border: rgb(0x0f4b5c),
        fg: rgb(0x93a1a1),
        muted: rgb(0x657b83),
        accent: rgb(0x268bd2),
        accent_2: rgb(0xd33682),
        cyan: rgb(0x2aa198),
        ok: rgb(0x859900),
        warn: rgb(0xb58900),
        err: rgb(0xdc322f),
    },
    Theme {
        id: "rose-pine",
        name: "Rose Pine",
        light: false,
        bg: rgb(0x191724),
        bg_soft: rgb(0x1f1d2e),
        bg_inset: rgb(0x14121f),
        border: rgb(0x33304a),
        fg: rgb(0xe0def4),
        muted: rgb(0x6e6a86),
        accent: rgb(0x9ccfd8),
        accent_2: rgb(0xc4a7e7),
        cyan: rgb(0x31748f),
        ok: rgb(0xa6da95),
        warn: rgb(0xf6c177),
        err: rgb(0xeb6f92),
    },
    Theme {
        id: "paper",
        name: "Paper (light)",
        light: true,
        bg: rgb(0xfaf4ed),
        bg_soft: rgb(0xf2e9e1),
        bg_inset: rgb(0xfffaf3),
        border: rgb(0xdfd8d0),
        fg: rgb(0x575279),
        muted: rgb(0x797593),
        accent: rgb(0x286983),
        accent_2: rgb(0x907aa9),
        cyan: rgb(0x56949f),
        ok: rgb(0x618774),
        warn: rgb(0xea9d34),
        err: rgb(0xb4637a),
    },
];

/// The index of the theme currently in force.
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

/// The theme currently in force.
///
/// Clamped on read: an index out of range would otherwise panic on a slice
/// index inside the paint loop, which is the worst place for it.
pub fn active() -> &'static Theme {
    let index = ACTIVE.load(Ordering::Relaxed);
    let table = themes();
    &table[index.min(table.len() - 1)]
}

/// Every theme the picker offers: the built-in ones, then any the user added.
///
/// Returns the `const` array until [`load_custom`] has run, which is what every
/// test and the first frames of startup see. After it has run, the same array
/// with the user's palettes appended -- appended rather than merged, so a
/// built-in theme keeps its index and a preferences file written before a
/// palette was added still selects the same scheme.
///
/// Reading this is a `OnceLock::get`, which is one atomic load. No mutex: this
/// is on the path of every repaint, and the reason `ACTIVE` is an
/// `AtomicUsize` in the first place applies just as much here.
pub fn themes() -> &'static [Theme] {
    match TABLE.get() {
        Some(table) => table.as_slice(),
        None => THEMES,
    }
}

/// Read the user's palettes and add them to the table. Returns any complaints.
///
/// Called once, during startup, before the first frame. Calling it again is a
/// no-op that returns no problems -- `OnceLock` accepts one value, and silently
/// keeping the first is right here: the alternative is a theme table that
/// changes shape while indices into it are live.
pub fn load_custom(dir: &std::path::Path) -> Vec<String> {
    let (custom, problems) = crate::palettes::load(dir);
    if custom.is_empty() {
        return problems;
    }
    let mut table: Vec<Theme> = THEMES.to_vec();
    table.extend(custom);
    // Ignore the error: a second call means the table is already built, and the
    // themes it holds are still perfectly good ones.
    let _ = TABLE.set(table);
    problems
}

/// Look a theme up by its stable identifier.
pub fn by_id(id: &str) -> Option<(usize, &'static Theme)> {
    themes().iter().enumerate().find(|(_, t)| t.id == id)
}

/// Switch to `id`, and apply it to `ctx`. Unknown identifiers are ignored, so
/// a preferences file naming a theme this build does not have keeps the
/// default rather than failing to start.
pub fn set_by_id(ctx: &egui::Context, id: &str) -> bool {
    match by_id(id) {
        Some((index, _)) => {
            ACTIVE.store(index, Ordering::Relaxed);
            install(ctx);
            true
        }
        None => false,
    }
}

/// Shorthand accessors, so call sites read as `p::fg()` rather than
/// `theme::active().fg`.
///
/// These were `const` values when there was one theme. They are functions now
/// because the palette is chosen at runtime; the call sites are otherwise
/// unchanged.
pub mod palette {
    use super::active;
    use egui::Color32;

    /// Window and central-panel background.
    pub fn bg() -> Color32 {
        active().bg
    }
    /// Deeper background, for inset panels.
    pub fn bg_dark() -> Color32 {
        active().bg_inset
    }
    /// Raised surface, for widgets.
    pub fn surface() -> Color32 {
        active().bg_soft
    }
    /// Hovered surface: the raised surface, lifted towards the text colour.
    ///
    /// Derived rather than tabulated, so a new theme cannot be added with a
    /// hover state that does not belong to it. `lerp` towards `fg` works for
    /// light schemes as well as dark ones, where simply lightening would not.
    pub fn surface_hi() -> Color32 {
        let t = active();
        blend(t.bg_soft, t.fg, 0.12)
    }
    /// Borders and separators.
    pub fn border() -> Color32 {
        active().border
    }
    /// Primary text.
    pub fn fg() -> Color32 {
        active().fg
    }
    /// Secondary text.
    pub fn muted() -> Color32 {
        active().muted
    }
    /// The project's primary colour.
    pub fn blue() -> Color32 {
        active().accent
    }
    /// Values and figures.
    pub fn cyan() -> Color32 {
        active().cyan
    }
    /// The "veiled" half of the mark.
    pub fn purple() -> Color32 {
        active().accent_2
    }
    /// Success.
    pub fn green() -> Color32 {
        active().ok
    }
    /// Warning.
    pub fn yellow() -> Color32 {
        active().warn
    }
    /// Error.
    pub fn red() -> Color32 {
        active().err
    }

    /// Mix `a` towards `b` by `t` in 0..=1.
    pub fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
        let t = t.clamp(0.0, 1.0);
        let mix = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t) as u8;
        Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
    }
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
        fonts.font_data.insert(
            "jetbrains".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
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

/// Apply the active theme's visuals and a monospace-everywhere type scale.
pub fn install(ctx: &egui::Context) {
    use palette as p;
    let theme = active();

    let mut visuals = if theme.light {
        Visuals::light()
    } else {
        Visuals::dark()
    };

    visuals.override_text_color = Some(p::fg());
    visuals.panel_fill = p::bg();
    visuals.window_fill = p::bg();
    visuals.extreme_bg_color = p::bg_dark();
    visuals.faint_bg_color = p::bg_dark();
    visuals.window_stroke = Stroke::new(1.0, p::border());
    visuals.selection.bg_fill = p::blue().gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, p::blue());
    visuals.hyperlink_color = p::cyan();

    let rounding = CornerRadius::same(4);
    for (widget, fill) in [
        (&mut visuals.widgets.noninteractive, p::bg_dark()),
        (&mut visuals.widgets.inactive, p::surface()),
        (&mut visuals.widgets.hovered, p::surface_hi()),
        (&mut visuals.widgets.active, p::surface_hi()),
        (&mut visuals.widgets.open, p::surface()),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.corner_radius = rounding;
        widget.bg_stroke = Stroke::new(1.0, p::border());
        widget.fg_stroke = Stroke::new(1.0, p::fg());
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p::blue());
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, p::blue());
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p::muted());

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
    use std::sync::Mutex;

    /// **F-78.** Four tests here read and write the same process-global
    /// `ACTIVE`, and `cargo test` runs them on parallel threads in one
    /// process. Nothing kept them apart, so any two could interleave: one
    /// switched the theme to `paper` and asked whether the palette had
    /// changed, while another put the index back to zero in between and made
    /// the answer no.
    ///
    /// Measured: one failure in forty runs of this module alone, and it fired
    /// for real during a full-workspace run, where there are more threads
    /// competing. A test that fails one run in forty is worse than one that
    /// fails every time, because the answer people learn is "run it again".
    ///
    /// So the tests that touch the global take this in turn. Poisoning is
    /// stepped over rather than propagated: a panic in one test has already
    /// failed that test, and turning it into a failure in every other one
    /// hides which was the real fault.
    static ACTIVE_THEME: Mutex<()> = Mutex::new(());

    fn alone() -> std::sync::MutexGuard<'static, ()> {
        ACTIVE_THEME
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn reset() {
        ACTIVE.store(0, Ordering::Relaxed);
    }

    #[test]
    fn palette_matches_the_cli_escape_codes() {
        let _alone = alone();
        reset();
        // Both front-ends must be the same program to look at. These values are
        // duplicated in veilvoice-cli's `theme::colour`.
        assert_eq!(palette::blue(), Color32::from_rgb(122, 162, 247));
        assert_eq!(palette::green(), Color32::from_rgb(158, 206, 106));
        assert_eq!(palette::red(), Color32::from_rgb(247, 118, 142));
        assert_eq!(palette::muted(), Color32::from_rgb(115, 122, 162));
    }

    /// The website must not gain a theme the app has never heard of.
    ///
    /// The test below walks `THEMES` and checks each against the stylesheet,
    /// which catches a theme changed or removed on either side -- and misses a
    /// theme **added to the website**, because nothing was walking the
    /// stylesheet looking for entries this crate does not know.
    ///
    /// That is the shape of the defect recorded against `html.test.js`: a check
    /// enumerating from a hardcoded list is only ever as wide as the list, and
    /// a page added later went unchecked for precisely that reason. So this
    /// enumerates from the *stylesheet* instead, and the pair of tests closes
    /// the loop in both directions.
    ///
    /// A reader who picks a theme on the website and then opens the app should
    /// find it there. Silently falling back to the default is a small thing
    /// that reads as the application being broken.
    #[test]
    fn the_website_has_no_theme_the_app_is_missing() {
        let css = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/css/themes.css"),
        )
        .expect("themes.css should be readable from the crate directory");

        let mut found = Vec::new();
        let mut rest = css.as_str();
        while let Some(at) = rest.find("[data-theme=\"") {
            let after = &rest[at + "[data-theme=\"".len()..];
            let end = after.find('"').expect("unterminated data-theme selector");
            found.push(after[..end].to_string());
            rest = &after[end..];
        }

        assert!(
            !found.is_empty(),
            "no [data-theme] selectors were found -- the parser, not the CSS, \
             is what changed"
        );

        for id in &found {
            assert!(
                THEMES.iter().any(|t| t.id == id),
                "the website defines the '{id}' theme and the app does not. \
                 Add it to THEMES, or remove it from website/css/themes.css."
            );
        }
    }

    /// The app's themes and the website's are meant to be the same themes, not
    /// two sets that happen to share names. Parsed straight out of the
    /// stylesheet so a change to either side without the other fails here.
    #[test]
    fn every_theme_matches_the_website_stylesheet() {
        let css = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/css/themes.css"),
        )
        .expect("themes.css should be readable from the crate directory");

        for theme in THEMES {
            let marker = format!("[data-theme=\"{}\"]", theme.id);
            let start = css
                .find(&marker)
                .unwrap_or_else(|| panic!("{} is not in themes.css", theme.id));
            let block_end = css[start..].find('}').expect("unterminated block") + start;
            let block = &css[start..block_end];

            for (token, expected) in [
                ("--bg", theme.bg),
                ("--bg-soft", theme.bg_soft),
                ("--bg-inset", theme.bg_inset),
                ("--border", theme.border),
                ("--fg", theme.fg),
                ("--muted", theme.muted),
                ("--accent-2", theme.accent_2),
                ("--cyan", theme.cyan),
                ("--ok", theme.ok),
                ("--warn", theme.warn),
                ("--err", theme.err),
            ] {
                let needle = format!("{token}:");
                let at = block
                    .find(&needle)
                    .unwrap_or_else(|| panic!("{} has no {token}", theme.id));
                let rest = &block[at + needle.len()..];
                let hex: String = rest
                    .trim_start()
                    .trim_start_matches('#')
                    .chars()
                    .take(6)
                    .collect();
                let value = u32::from_str_radix(&hex, 16)
                    .unwrap_or_else(|_| panic!("{} {token} is not a hex colour", theme.id));
                assert_eq!(
                    rgb(value),
                    expected,
                    "{} {token} differs between the app and the website",
                    theme.id
                );
            }

            // `--accent` needs its own lookup: `--accent-2` contains it as a
            // prefix, so a naive `find` would match the wrong line.
            let at = block
                .match_indices("--accent:")
                .next()
                .unwrap_or_else(|| panic!("{} has no --accent", theme.id))
                .0;
            let hex: String = block[at + "--accent:".len()..]
                .trim_start()
                .trim_start_matches('#')
                .chars()
                .take(6)
                .collect();
            assert_eq!(
                rgb(u32::from_str_radix(&hex, 16).unwrap()),
                theme.accent,
                "{} --accent differs between the app and the website",
                theme.id
            );
        }
    }

    /// The website offers exactly these themes, in this order. A theme added to
    /// one and not the other is the kind of drift nobody notices.
    #[test]
    fn the_app_offers_the_same_themes_as_the_website() {
        let js = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/js/theme.js"),
        )
        .expect("theme.js should be readable");
        for theme in THEMES {
            assert!(
                js.contains(&format!("\"{}\"", theme.id)),
                "the website does not offer {}",
                theme.id
            );
        }
    }

    #[test]
    fn switching_theme_changes_the_palette_and_the_visuals() {
        let _alone = alone();
        reset();
        let ctx = egui::Context::default();
        install(&ctx);
        let tokyo = palette::bg();

        assert!(set_by_id(&ctx, "paper"));
        assert_ne!(palette::bg(), tokyo, "the palette did not change");
        assert_eq!(ctx.style().visuals.panel_fill, palette::bg());
        assert!(active().light, "paper is a light scheme");

        // An unknown identifier must be ignored rather than fatal: a
        // preferences file naming a theme this build does not have has to
        // still start.
        assert!(!set_by_id(&ctx, "no-such-theme"));
        assert_eq!(active().id, "paper", "an unknown id changed the theme");
        reset();
    }

    /// An out-of-range index must saturate rather than panic. This is read
    /// inside the paint loop, which is the worst place for an index panic.
    #[test]
    fn an_impossible_index_saturates_instead_of_panicking() {
        let _alone = alone();
        ACTIVE.store(usize::MAX, Ordering::Relaxed);
        let _ = active();
        assert_eq!(active().id, THEMES[THEMES.len() - 1].id);
        reset();
    }

    #[test]
    fn theme_identifiers_are_unique_and_stable() {
        let mut ids: Vec<&str> = THEMES.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate theme id");
        // Written into the preferences file, so they must stay ASCII and plain.
        for theme in THEMES {
            assert!(
                theme.id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{} is not a stable identifier",
                theme.id
            );
            assert!(!theme.name.is_empty());
        }
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
        let _alone = alone();
        reset();
        let ctx = egui::Context::default();
        install(&ctx);
        assert_eq!(ctx.style().visuals.panel_fill, palette::bg());
        assert!(ctx.style().text_styles.contains_key(&TextStyle::Monospace));
    }

    /// Missing JetBrains Mono must degrade to the built-in face, never panic.
    #[test]
    fn missing_font_is_not_fatal() {
        let _alone = alone();
        let ctx = egui::Context::default();
        let _found = install_fonts(&ctx);
        install(&ctx);
    }
}
