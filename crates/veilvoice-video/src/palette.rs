// SPDX-License-Identifier: GPL-3.0-or-later
//! Colours: the site's own tokens, and one per speaker.
//!
//! # One source of colour, cross-checked by a test
//!
//! The hexes below are Tokyo Night, and they are the same ones
//! `website/css/themes.css` declares. They are written out here rather than
//! parsed at run time because a crate should not need the website to be on disk
//! to draw a circle, and a test reads that stylesheet and fails if the two ever
//! disagree, which is the same arrangement `veilvoice-gui` has had since the
//! themes existed.
//!
//! # Ten speaker colours, ordered by measurement rather than by eye
//!
//! Six of them are palette tokens. The other four are from the wider Tokyo
//! Night set, chosen to sit between the tokens.
//!
//! The **order** was first written down by looking at a hue wheel, and it was
//! wrong: a test comparing every pair found a further-apart pair than the one
//! put first. So the order is now computed rather than judged, by
//! [`distance`], the "redmean" approximation, which is the cheap standard
//! stand-in for perceptual difference and weights green most because the eye
//! does.
//!
//! Slot 0 and slot 1 are **the** furthest-apart pair in the set, because two
//! speakers is the common case. Every slot after that is the colour whose
//! nearest neighbour among the ones already used is furthest away, which is a maximin
//! order, so the table degrades gracefully: a recording with four people uses
//! four colours chosen to be as separable as four can be, rather than the first
//! four somebody listed.
//!
//! Ten colours cannot all be far apart. Under this metric the furthest pair
//! scores 507 and the closest pair anywhere in the set scores 63, and the
//! closest pair is only ever reached by a recording with nine or ten people in
//! it.
//!
//! One colour is deliberately not a hue at all: the near-white foreground
//! token, separated from every saturated colour by **lightness**, the axis
//! that is still free once the wheel is full.
//!
//! # Colour is never the only signal
//!
//! Somebody who cannot separate two of these needs the name, and the name is
//! always drawn beside the circle and always in the subtitles. A player that
//! showed only colours would be one about eight per cent of men could not use.
//!
//! # In plain words
//!
//! The colours, and which one each speaker gets.
//!
//! They are the same colours the website uses, taken from one place so the
//! application, the website and anything VeilVoice draws cannot drift apart. A
//! test compares them against the site's own stylesheet and fails the build if
//! they do.
//!
//! Speaker colours are handed out to be as distinct from each other as the number
//! of people allows, so that a glance at the picture tells you who is talking.

/// The page background.
pub const BG: &str = "#1a1b26";
/// A panel or inset behind the waveform.
pub const BG_INSET: &str = "#16161e";
/// Hairlines and dividers.
pub const BORDER: &str = "#2f3549";
/// Body text.
pub const FG: &str = "#c0caf5";
/// Secondary text.
pub const MUTED: &str = "#737aa2";

/// One complete colour scheme, matching one `[data-theme]` block in
/// `website/css/themes.css` and one entry in `veilvoice-gui`'s theme table.
///
/// The field names are the CSS custom properties one for one: `bg` is `--bg`,
/// `accent_2` is `--accent-2`. A test reads that stylesheet and fails if any of
/// them ever disagree, which is the same arrangement the desktop application
/// has had since the themes existed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Stable identifier, matching the website's `data-theme` value.
    pub id: &'static str,
    /// Human-readable name, as the pickers show it.
    pub name: &'static str,
    /// Whether this is a light scheme.
    pub light: bool,

    /// The page behind everything.
    pub bg: &'static str,
    /// A raised surface.
    pub bg_soft: &'static str,
    /// A panel or inset, behind the waveform.
    pub bg_inset: &'static str,
    /// Hairlines and dividers.
    pub border: &'static str,
    /// Body text.
    pub fg: &'static str,
    /// Secondary text.
    pub muted: &'static str,
    /// The project's primary colour.
    pub accent: &'static str,
    /// The "veiled" half of the mark.
    pub accent_2: &'static str,
    /// Values and figures.
    pub cyan: &'static str,
    /// Success.
    pub ok: &'static str,
    /// Warning.
    pub warn: &'static str,
    /// Error.
    pub err: &'static str,
}

/// Every palette, in the order the pickers show them.
///
/// Index 0 is the default, and is what an unknown identifier falls back to.
pub const PALETTES: &[Palette] = &[
    Palette {
        id: "tokyo-night",
        name: "Tokyo Night",
        light: false,
        bg: "#1a1b26",
        bg_soft: "#1f2335",
        bg_inset: "#16161e",
        border: "#2f3549",
        fg: "#c0caf5",
        muted: "#737aa2",
        accent: "#7aa2f7",
        accent_2: "#bb9af7",
        cyan: "#7dcfff",
        ok: "#9ece6a",
        warn: "#e0af68",
        err: "#f7768e",
    },
    Palette {
        id: "gruvbox",
        name: "Gruvbox",
        light: false,
        bg: "#282828",
        bg_soft: "#32302f",
        bg_inset: "#1d2021",
        border: "#504945",
        fg: "#ebdbb2",
        muted: "#928374",
        accent: "#83a598",
        accent_2: "#d3869b",
        cyan: "#8ec07c",
        ok: "#b8bb26",
        warn: "#fabd2f",
        err: "#fb4934",
    },
    Palette {
        id: "dracula",
        name: "Dracula",
        light: false,
        bg: "#282a36",
        bg_soft: "#343746",
        bg_inset: "#21222c",
        border: "#44475a",
        fg: "#f8f8f2",
        muted: "#6272a4",
        accent: "#bd93f9",
        accent_2: "#ff79c6",
        cyan: "#8be9fd",
        ok: "#50fa7b",
        warn: "#f1fa8c",
        err: "#ff5555",
    },
    Palette {
        id: "nord",
        name: "Nord",
        light: false,
        bg: "#2e3440",
        bg_soft: "#3b4252",
        bg_inset: "#272c36",
        border: "#4c566a",
        fg: "#eceff4",
        muted: "#7b88a1",
        accent: "#88c0d0",
        accent_2: "#b48ead",
        cyan: "#8fbcbb",
        ok: "#a3be8c",
        warn: "#ebcb8b",
        err: "#bf616a",
    },
    Palette {
        id: "catppuccin",
        name: "Catppuccin Mocha",
        light: false,
        bg: "#1e1e2e",
        bg_soft: "#313244",
        bg_inset: "#181825",
        border: "#45475a",
        fg: "#cdd6f4",
        muted: "#7f849c",
        accent: "#89b4fa",
        accent_2: "#cba6f7",
        cyan: "#94e2d5",
        ok: "#a6e3a1",
        warn: "#f9e2af",
        err: "#f38ba8",
    },
    Palette {
        id: "everforest",
        name: "Everforest",
        light: false,
        bg: "#2d353b",
        bg_soft: "#343f44",
        bg_inset: "#272e33",
        border: "#475258",
        fg: "#d3c6aa",
        muted: "#859289",
        accent: "#a7c080",
        accent_2: "#d699b6",
        cyan: "#83c092",
        ok: "#a7c080",
        warn: "#dbbc7f",
        err: "#e67e80",
    },
    Palette {
        id: "solarized",
        name: "Solarized Dark",
        light: false,
        bg: "#002b36",
        bg_soft: "#073642",
        bg_inset: "#00212b",
        border: "#0f4b5c",
        fg: "#93a1a1",
        muted: "#657b83",
        accent: "#268bd2",
        accent_2: "#d33682",
        cyan: "#2aa198",
        ok: "#859900",
        warn: "#b58900",
        err: "#dc322f",
    },
    Palette {
        id: "rose-pine",
        name: "Rose Pine",
        light: false,
        bg: "#191724",
        bg_soft: "#1f1d2e",
        bg_inset: "#14121f",
        border: "#33304a",
        fg: "#e0def4",
        muted: "#6e6a86",
        accent: "#9ccfd8",
        accent_2: "#c4a7e7",
        cyan: "#31748f",
        ok: "#a6da95",
        warn: "#f6c177",
        err: "#eb6f92",
    },
    Palette {
        id: "paper",
        name: "Paper (light)",
        light: true,
        bg: "#faf4ed",
        bg_soft: "#f2e9e1",
        bg_inset: "#fffaf3",
        border: "#dfd8d0",
        fg: "#575279",
        muted: "#797593",
        accent: "#286983",
        accent_2: "#907aa9",
        cyan: "#56949f",
        ok: "#618774",
        warn: "#ea9d34",
        err: "#b4637a",
    },
];

/// The palette a render uses unless one is named.
pub const DEFAULT_ID: &str = "tokyo-night";

/// The palette with this identifier.
///
/// `None` rather than a fallback: a caller who typed a theme name meant it, and
/// silently drawing in a different one answers a question they did not ask. The
/// command line turns this into an error that lists what it could have been.
pub fn by_id(id: &str) -> Option<&'static Palette> {
    PALETTES.iter().find(|palette| palette.id == id)
}

/// The default palette. Tokyo Night, and the same hexes the constants above
/// carry.
pub fn default_palette() -> &'static Palette {
    &PALETTES[0]
}

/// Every identifier, for an error message or a picker.
pub fn ids() -> Vec<&'static str> {
    PALETTES.iter().map(|palette| palette.id).collect()
}

/// The ten speaker colours, in the order slots are handed out.
///
/// See the module note for why the order is what it is, and why the tenth is
/// pale rather than another hue.
///
/// # One set, for every palette
///
/// These do **not** change with the chosen palette, and that is a decision
/// rather than an omission. A palette here has six chromatic tokens; ten
/// mutually separable colours cannot be got out of six without inventing four,
/// and four invented colours are four colours whose separation nobody has
/// measured. This set was measured: the closest pair anywhere in it scores 63
/// under [`distance`], and that pair is only ever reached by a recording with
/// nine or ten people in it.
///
/// What the palette *does* decide is everything around them -- the page, the
/// panel, the hairlines, the text -- so a render in Gruvbox is a Gruvbox
/// picture with these ten circles in it. And the ink drawn on each circle is
/// computed by [`ink_on`] rather than assumed, so the names stay readable on a
/// light palette as well as a dark one.
pub const SPEAKERS: [&str; 10] = [
    "#73daca", // 0  teal      -- furthest-apart pair with slot 1
    "#ff007c", // 1  magenta
    "#ff9e64", // 2  orange
    "#bb9af7", // 3  purple    -- the palette's accent-2
    "#9ece6a", // 4  green     -- the palette's ok
    "#7aa2f7", // 5  blue      -- the palette's accent
    "#f7768e", // 6  red-pink  -- the palette's err
    "#c0caf5", // 7  pale      -- separated by lightness, not hue
    "#7dcfff", // 8  cyan      -- the palette's cyan
    "#e0af68", // 9  yellow    -- the palette's warn
];

/// The colour for a speaker slot.
///
/// Wraps past ten, exactly as the voice table does and for the same reason: the
/// function is total so a caller cannot panic, but two speakers sharing a
/// colour is a real collision and the conversation crate refuses an eleventh
/// speaker long before this is reached.
pub fn speaker(slot: usize) -> &'static str {
    SPEAKERS[slot % SPEAKERS.len()]
}

/// How far apart two colours look, by the "redmean" approximation.
///
/// A cheap, widely used stand-in for a perceptual colour distance: it weights
/// green most, because the eye takes most of its luminance from green, and
/// shifts the red and blue weights by where the pair sits on the red axis.
///
/// Used to *order* the speaker colours rather than to make any claim about
/// what somebody can see. Nothing here decides that colour is sufficient --
/// see the note at the top of this file about the name always being drawn.
pub fn distance(a: &str, b: &str) -> f32 {
    let (Some((r1, g1, b1)), Some((r2, g2, b2))) = (rgb(a), rgb(b)) else {
        return 0.0;
    };
    let mean = (r1 as f32 + r2 as f32) / 2.0;
    let dr = r1 as f32 - r2 as f32;
    let dg = g1 as f32 - g2 as f32;
    let db = b1 as f32 - b2 as f32;
    ((2.0 + mean / 256.0) * dr * dr + 4.0 * dg * dg + (2.0 + (255.0 - mean) / 256.0) * db * db)
        .sqrt()
}

/// Parse `#rrggbb` into its three channels.
///
/// Returns `None` for anything that is not exactly that, rather than guessing:
/// a colour that half-parsed would be drawn in some arbitrary shade and look
/// like a design decision.
pub fn rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some((
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ))
}

/// Relative luminance, as WCAG defines it, from 0.0 to 1.0.
///
/// Used to pick readable text over a speaker's colour. The project already
/// computes contrast rather than trusting it. Writing that check for the
/// custom palettes found the default theme's `--muted` failing at 2.76:1, and
/// this is the same arithmetic in the same spirit.
pub fn luminance(hex: &str) -> f32 {
    let Some((r, g, b)) = rgb(hex) else {
        return 0.0;
    };
    let channel = |value: u8| {
        let v = value as f32 / 255.0;
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

/// The contrast ratio between two colours, from 1.0 to 21.0.
pub fn contrast(a: &str, b: &str) -> f32 {
    let (first, second) = (luminance(a), luminance(b));
    let (lighter, darker) = if first > second {
        (first, second)
    } else {
        (second, first)
    };
    (lighter + 0.05) / (darker + 0.05)
}

/// Black or white, whichever is readable on `background`.
pub fn ink_on(background: &str) -> &'static str {
    if contrast(background, "#000000") >= contrast(background, "#ffffff") {
        "#000000"
    } else {
        "#ffffff"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hexes here must be the ones the website declares. If a colour is
    /// changed in one place, this is what says so.
    #[test]
    fn every_token_matches_the_website_stylesheet() {
        let css = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/css/themes.css"),
        )
        .expect("themes.css should be readable from the crate directory");
        let start = css.find(":root,").expect("the default block");
        let block = &css[start..css[start..].find('}').unwrap() + start];

        for (token, expected) in [
            ("--bg:", BG),
            ("--bg-inset:", BG_INSET),
            ("--border:", BORDER),
            ("--fg:", FG),
            ("--muted:", MUTED),
            ("--accent:", SPEAKERS[5]),
            ("--ok:", SPEAKERS[4]),
            ("--err:", SPEAKERS[6]),
            ("--cyan:", SPEAKERS[8]),
            ("--warn:", SPEAKERS[9]),
            ("--accent-2:", SPEAKERS[3]),
        ] {
            let at = block
                .find(token)
                .unwrap_or_else(|| panic!("{token} is not in themes.css"));
            let rest = &block[at + token.len()..];
            let found = rest.trim_start();
            assert!(
                found.to_ascii_lowercase().starts_with(expected),
                "{token} is {} in themes.css and {expected} here",
                &found[..7.min(found.len())]
            );
        }
    }

    #[test]
    fn every_speaker_colour_is_a_valid_hex_and_they_are_all_different() {
        let mut seen = Vec::new();
        for (slot, colour) in SPEAKERS.iter().enumerate() {
            assert!(rgb(colour).is_some(), "slot {slot} is not a colour");
            assert!(!seen.contains(colour), "slot {slot} repeats {colour}");
            seen.push(colour);
        }
        assert_eq!(SPEAKERS.len(), 10);
    }

    /// Two speakers is the common case, so slots 0 and 1 must be **the**
    /// furthest-apart pair in the set. The first version of this table failed
    /// exactly here, which is why the order is computed rather than judged.
    /// Every palette here is a palette the website has, with the same hexes.
    ///
    /// The same arrangement `veilvoice-gui` has had since the themes existed:
    /// the values are written out so a crate needs no website on disk to draw
    /// a circle, and this reads the stylesheet and fails if the two ever part
    /// company. Without it a theme could be changed on the site and a rendered
    /// video would quietly keep the old colours.
    #[test]
    fn every_palette_matches_the_website_stylesheet() {
        let css = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/css/themes.css"),
        )
        .expect("themes.css should be readable from the crate directory");

        for palette in PALETTES {
            // The block for this theme: from its selector to the closing brace.
            let marker = format!("[data-theme=\"{}\"]", palette.id);
            let start = css
                .find(&marker)
                .unwrap_or_else(|| panic!("{} is not in themes.css", palette.id));
            let block = &css[start..];
            let end = block.find('}').expect("a theme block must close");
            let block = &block[..end];

            for (token, value) in [
                ("--bg", palette.bg),
                ("--bg-soft", palette.bg_soft),
                ("--bg-inset", palette.bg_inset),
                ("--border", palette.border),
                ("--fg", palette.fg),
                ("--muted", palette.muted),
                ("--accent", palette.accent),
                ("--accent-2", palette.accent_2),
                ("--cyan", palette.cyan),
                ("--ok", palette.ok),
                ("--warn", palette.warn),
                ("--err", palette.err),
            ] {
                // `--accent` would otherwise match `--accent-2`.
                let wanted = format!("{token}:");
                let at = block
                    .match_indices(&wanted)
                    .find(|(index, _)| {
                        block[..*index].ends_with(char::is_whitespace) || *index == 0
                    })
                    .unwrap_or_else(|| panic!("{} has no {token}", palette.id))
                    .0;
                let rest = &block[at + wanted.len()..];
                let declared = rest.trim_start();
                assert!(
                    declared.to_lowercase().starts_with(&value.to_lowercase()),
                    "{} {token} is {} here and {} in themes.css",
                    palette.id,
                    value,
                    &declared[..7.min(declared.len())]
                );
            }
        }
    }

    /// The stylesheet must not hold a theme this crate has never heard of. A
    /// picker offering nine and a renderer knowing eight is a picker with one
    /// entry that silently draws in the wrong colours.
    #[test]
    fn the_stylesheet_has_no_theme_this_crate_is_missing() {
        let css = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/css/themes.css"),
        )
        .expect("themes.css should be readable from the crate directory");

        for (index, _) in css.match_indices("[data-theme=\"") {
            let rest = &css[index + "[data-theme=\"".len()..];
            let id: String = rest.chars().take_while(|c| *c != '"').collect();
            assert!(
                by_id(&id).is_some(),
                "themes.css declares {id:?} and this crate has no palette for it"
            );
        }
    }

    #[test]
    fn the_default_is_tokyo_night_and_matches_the_constants() {
        let default = default_palette();
        assert_eq!(default.id, DEFAULT_ID);
        assert_eq!(default.bg, BG);
        assert_eq!(default.bg_inset, BG_INSET);
        assert_eq!(default.border, BORDER);
        assert_eq!(default.fg, FG);
        assert_eq!(default.muted, MUTED);
    }

    /// An unknown name is refused rather than falling back. A caller who typed
    /// a theme meant it, and drawing in a different one answers a question
    /// they did not ask.
    #[test]
    fn an_unknown_identifier_is_refused_rather_than_defaulted() {
        assert!(by_id("tokyo-night").is_some());
        assert!(by_id("gruvbox").is_some());
        assert!(by_id("solarised").is_none(), "not a spelling this has");
        assert!(by_id("").is_none());
        assert!(by_id("TOKYO-NIGHT").is_none(), "identifiers are exact");
    }

    #[test]
    fn every_identifier_is_unique_and_every_colour_parses() {
        let mut ids = ids();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "two palettes share an identifier");
        assert!(count >= 9, "the website offers nine; this has {count}");

        for palette in PALETTES {
            for (what, colour) in [
                ("bg", palette.bg),
                ("bg-soft", palette.bg_soft),
                ("bg-inset", palette.bg_inset),
                ("border", palette.border),
                ("fg", palette.fg),
                ("muted", palette.muted),
                ("accent", palette.accent),
                ("accent-2", palette.accent_2),
                ("cyan", palette.cyan),
                ("ok", palette.ok),
                ("warn", palette.warn),
                ("err", palette.err),
            ] {
                assert!(
                    rgb(colour).is_some(),
                    "{} {what} is {colour:?}, which is not #rrggbb",
                    palette.id
                );
            }
        }
    }

    /// Text has to be readable on the page in every palette, light or dark.
    /// WCAG's floor for body text is 4.5:1; this is the arithmetic, not a
    /// judgement.
    #[test]
    fn body_text_is_readable_on_the_page_in_every_palette() {
        for palette in PALETTES {
            let ratio = contrast(palette.fg, palette.bg);
            assert!(
                ratio >= 4.5,
                "{}: fg on bg is {ratio:.2}:1, under 4.5",
                palette.id
            );
        }
    }

    /// The speaker circles are one measured set shared by every palette, so
    /// the thing that has to hold per palette is that a name drawn on a circle
    /// is readable. `ink_on` computes that rather than assuming it.
    #[test]
    fn a_name_on_a_speaker_circle_is_readable_whatever_the_palette() {
        for slot in 0..SPEAKERS.len() {
            let circle = speaker(slot);
            let ratio = contrast(ink_on(circle), circle);
            assert!(
                ratio >= 4.5,
                "slot {slot} ({circle}) gives {ratio:.2}:1 for its label"
            );
        }
    }

    #[test]
    fn the_first_two_slots_are_the_furthest_apart_pair_in_the_set() {
        let first_pair = distance(SPEAKERS[0], SPEAKERS[1]);
        for (i, a) in SPEAKERS.iter().enumerate() {
            for b in SPEAKERS.iter().skip(i + 1) {
                assert!(
                    distance(a, b) <= first_pair + 0.5,
                    "{a} and {b} are further apart than slots 0 and 1"
                );
            }
        }
    }

    /// Every slot after the first two must be the colour whose nearest
    /// neighbour among those already used is furthest away. That is what makes
    /// a four-speaker recording use four well-separated colours rather than
    /// the first four somebody happened to list.
    #[test]
    fn the_order_is_maximin_so_it_degrades_gracefully() {
        for slot in 2..SPEAKERS.len() {
            let used = &SPEAKERS[..slot];
            let nearest = |colour: &str| {
                used.iter()
                    .map(|other| distance(colour, other))
                    .fold(f32::INFINITY, f32::min)
            };
            let chosen = nearest(SPEAKERS[slot]);
            for later in &SPEAKERS[slot + 1..] {
                assert!(
                    nearest(later) <= chosen + 0.5,
                    "slot {slot} should have been {later}: it is further from the \
                     colours already in use"
                );
            }
        }
    }

    /// The figures quoted in the module documentation, checked.
    #[test]
    fn the_spread_is_what_the_documentation_says() {
        let mut furthest: f32 = 0.0;
        let mut closest = f32::INFINITY;
        for (i, a) in SPEAKERS.iter().enumerate() {
            for b in SPEAKERS.iter().skip(i + 1) {
                furthest = furthest.max(distance(a, b));
                closest = closest.min(distance(a, b));
            }
        }
        assert!(
            (furthest - 507.0).abs() < 2.0,
            "furthest pair is {furthest}"
        );
        assert!((closest - 63.0).abs() < 2.0, "closest pair is {closest}");
    }

    #[test]
    fn a_distance_between_two_non_colours_is_zero_rather_than_a_panic() {
        assert_eq!(distance("nonsense", "#7aa2f7"), 0.0);
        assert_eq!(distance("#7aa2f7", "#7aa2f7"), 0.0);
    }

    /// Every speaker colour has to be visible against the page it is drawn on.
    /// Computed, not assumed -- the same rule the custom palettes follow.
    #[test]
    fn every_speaker_colour_is_visible_on_the_background() {
        for (slot, colour) in SPEAKERS.iter().enumerate() {
            let ratio = contrast(colour, BG);
            assert!(
                ratio >= 3.0,
                "slot {slot} ({colour}) is {ratio:.2}:1 against the background, and a \
                 shape needs 3:1"
            );
        }
    }

    /// A name drawn on a speaker's colour must be readable on it.
    #[test]
    fn the_ink_chosen_for_each_colour_is_readable_on_it() {
        for colour in SPEAKERS {
            let ink = ink_on(colour);
            assert!(
                contrast(colour, ink) >= 4.5,
                "{ink} on {colour} is only {:.2}:1",
                contrast(colour, ink)
            );
        }
    }

    #[test]
    fn a_colour_that_is_not_a_colour_is_refused_rather_than_guessed_at() {
        for bad in ["", "#", "#fff", "#gggggg", "7aa2f7", "#7aa2f7f7", "blue"] {
            assert!(rgb(bad).is_none(), "{bad:?} parsed as a colour");
        }
        assert_eq!(luminance("nonsense"), 0.0);
    }

    #[test]
    fn luminance_and_contrast_are_the_documented_ranges() {
        assert!(luminance("#000000") < 1e-6);
        assert!((luminance("#ffffff") - 1.0).abs() < 1e-4);
        assert!((contrast("#000000", "#ffffff") - 21.0).abs() < 0.01);
        assert!((contrast("#7aa2f7", "#7aa2f7") - 1.0).abs() < 1e-4);
    }

    #[test]
    fn asking_past_the_table_wraps() {
        assert_eq!(speaker(0), SPEAKERS[0]);
        assert_eq!(speaker(10), SPEAKERS[0]);
        let _ = speaker(usize::MAX);
    }
}
