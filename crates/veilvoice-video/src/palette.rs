// SPDX-License-Identifier: GPL-3.0-or-later
//! Colours: the site's own tokens, and one per speaker.
//!
//! # One source of colour, cross-checked by a test
//!
//! The hexes below are Tokyo Night, and they are the same ones
//! `website/css/themes.css` declares. They are written out here rather than
//! parsed at run time because a crate should not need the website to be on disk
//! to draw a circle — and a test reads that stylesheet and fails if the two ever
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
//! [`distance`] — the "redmean" approximation, which is the cheap standard
//! stand-in for perceptual difference and weights green most because the eye
//! does.
//!
//! Slot 0 and slot 1 are **the** furthest-apart pair in the set, because two
//! speakers is the common case. Every slot after that is the colour whose
//! nearest neighbour among the ones already used is furthest away — a maximin
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
//! token, separated from every saturated colour by **lightness** — the axis
//! that is still free once the wheel is full.
//!
//! # Colour is never the only signal
//!
//! Somebody who cannot separate two of these needs the name, and the name is
//! always drawn beside the circle and always in the subtitles. A player that
//! showed only colours would be one about eight per cent of men could not use.

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

/// The ten speaker colours, in the order slots are handed out.
///
/// See the module note for why the order is what it is, and why the tenth is
/// pale rather than another hue.
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
/// computes contrast rather than trusting it — writing that check for the
/// custom palettes found the default theme's `--muted` failing at 2.76:1 — and
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
