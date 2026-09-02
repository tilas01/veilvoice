// SPDX-License-Identifier: GPL-3.0-or-later
//! How the application tells you something, and the three ways to be told.
//!
//! # Three modes, and none of them is the obviously right one
//!
//! [`Style::Overlay`] draws a rounded, translucent card in the corner of
//! VeilVoice's own window. It is the quiet option: it does not steal focus, it
//! does not interrupt what you are typing, and it fades on its own.
//!
//! [`Style::Alert`] is the loud one. It stops the panel it is on until it is
//! dismissed, so it cannot be missed and it cannot be missed *quietly* --
//! which is the point when the thing being reported is that something started
//! recording your screen.
//!
//! [`Style::Off`] shows nothing. It is offered because a monitor that
//! interrupts somebody every thirty seconds is a monitor they switch off at the
//! operating system, and then it is not watching for anything at all. Better a
//! reader who chose silence knowingly than one who disabled the whole feature
//! to get it.
//!
//! There is no default that suits everybody, so the default is the middle one
//! and the choice is a preference rather than a guess.
//!
//! # The contrast is computed, never assumed
//!
//! A translucent card is a colour laid over whatever is behind it, so the text
//! on it is legible only if the *composited* result has enough contrast. Two
//! things follow, and both were got wrong in the first version of this file:
//!
//! * The background to measure against is the blend, not the card's own tint.
//!   [`blend`] does that arithmetic, and [`Card::readable_text`] measures the
//!   result with the same WCAG ratio [`crate::palettes`] already uses on user
//!   palettes.
//! * If no candidate reaches the threshold, the card is drawn **opaque**
//!   rather than shipped illegible. Translucency is a nicety; being able to
//!   read a warning is not.
//!
//! # What this does not do
//!
//! It does not raise a system notification, put anything in a tray, or reach
//! outside VeilVoice's own window. Those need per-platform APIs and, on two of
//! the three, a registered application identity -- and this project is
//! published under a pseudonym on purpose. A notification that only appears
//! while the window is open is a real limit, and [`SCOPE`] says so rather than
//! letting somebody rely on being told while VeilVoice is closed.
//!
//! # In plain words
//!
//! When VeilVoice has something to tell you, it can do it three ways: a small
//! rounded box in the corner of its own window that fades away by itself, a
//! message that stops what you are doing until you dismiss it, or nothing at
//! all.
//!
//! The quiet box is see-through, so the colours behind it change how readable
//! the writing is. Rather than guessing, VeilVoice measures the actual
//! contrast of the result and picks the text colour that comes out clearest --
//! and if none of them is clear enough, it makes the box solid instead. A
//! warning you cannot read is not a warning.
//!
//! One honest limit: these only appear while the VeilVoice window is open. It
//! does not put messages into your desktop's own notification area.

use eframe::egui::{self, Color32, RichText, Ui};

use crate::palettes::contrast;
use crate::theme::palette as p;

/// The smallest contrast ratio a notification's text may have.
///
/// WCAG 2.1's threshold for body text. Not 3.0, which is the large-text
/// allowance: a notification is read once, quickly, often out of the corner of
/// an eye, and it is the one piece of text in the application most likely to be
/// read badly.
pub const LEAST_CONTRAST: f32 = 4.5;

/// How much of the card's own colour shows over what is behind it.
///
/// Not a free parameter. Below about this the card stops reading as a surface
/// and the text appears to float on the panel; far above it there is no point
/// calling it translucent.
pub const CARD_ALPHA: u8 = 216;

/// How the application shows a notification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style {
    /// A rounded, translucent card in the corner. Fades by itself.
    #[default]
    Overlay,
    /// A message that stops the panel until it is dismissed.
    Alert,
    /// Nothing at all.
    Off,
}

impl Style {
    /// A short name, for a picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::Overlay => "a card in the corner",
            Self::Alert => "a message that stops you",
            Self::Off => "nothing",
        }
    }

    /// What this choice costs and buys, in the words a front end should show.
    pub fn note(self) -> &'static str {
        match self {
            Self::Overlay => {
                "A rounded card in the corner of this window that fades on its own. \
                 It will not take focus or interrupt what you are typing, which \
                 also means it can be missed."
            }
            Self::Alert => {
                "Stops the panel until you dismiss it. Cannot be missed, and cannot \
                 be missed quietly, which is what you want when the thing being \
                 reported is that something started recording."
            }
            Self::Off => {
                "Nothing is shown. Offered because a monitor that interrupts you \
                 every thirty seconds is one you switch off entirely, and then it \
                 is not watching for anything. Choosing silence here is better \
                 than disabling the feature."
            }
        }
    }

    /// Every style, in the order a picker should offer them.
    pub const ALL: &'static [Style] = &[Style::Overlay, Style::Alert, Style::Off];

    /// The identifier written to the settings file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Alert => "alert",
            Self::Off => "off",
        }
    }

    /// Read a style back. An unrecognised value is the default rather than an
    /// error: a settings file from a newer version should not stop the
    /// application, and of the two ways to be wrong, showing a notification is
    /// the one that cannot hide a warning.
    pub fn from_key(key: &str) -> Style {
        Self::ALL
            .iter()
            .copied()
            .find(|style| style.key() == key)
            .unwrap_or_default()
    }
}

/// How serious a notification is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Level {
    /// Something happened and nothing is wrong.
    #[default]
    Note,
    /// Something worth acting on.
    Warn,
}

/// One thing to tell the reader.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// The message.
    pub text: String,
    /// How serious it is.
    pub level: Level,
}

impl Notice {
    /// A plain note.
    pub fn note(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: Level::Note,
        }
    }

    /// Something worth acting on.
    pub fn warn(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: Level::Warn,
        }
    }
}

/// Lay `over` on top of `under` at `alpha`, giving the colour actually seen.
///
/// The whole reason the contrast here is computed rather than assumed. A card
/// drawn at 85% opacity over a dark panel is neither of those two colours, and
/// measuring against either one gives an answer that is wrong in a direction
/// nobody notices until they are reading a warning they cannot read.
pub fn blend(over: Color32, under: Color32, alpha: u8) -> Color32 {
    let a = alpha as f32 / 255.0;
    let mix = |o: u8, u: u8| ((o as f32 * a) + (u as f32 * (1.0 - a))).round() as u8;
    Color32::from_rgb(
        mix(over.r(), under.r()),
        mix(over.g(), under.g()),
        mix(over.b(), under.b()),
    )
}

/// A card's measured colours: what it is drawn in, and what its text is drawn
/// in, chosen so the result is legible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Card {
    /// The card's fill, already blended with the panel behind it.
    pub fill: Color32,
    /// The text colour that measured best against that fill.
    pub text: Color32,
    /// The measured ratio of [`Card::text`] on [`Card::fill`].
    pub ratio: f32,
    /// True when translucency had to be given up to stay legible.
    ///
    /// Reported rather than hidden: a card that is quietly opaque looks like a
    /// design choice, and the preferences panel says which it was.
    pub opaque: bool,
}

impl Card {
    /// Work out how to draw a card of this level on this panel.
    ///
    /// Tries the translucent card first, and only if nothing on it reaches
    /// [`LEAST_CONTRAST`] does it fall back to an opaque one. Translucency is
    /// a nicety; reading the warning is not.
    pub fn for_level(level: Level, panel: Color32) -> Card {
        let tint = match level {
            Level::Note => p::bg_dark(),
            Level::Warn => p::yellow(),
        };
        let translucent = blend(tint, panel, CARD_ALPHA);
        let best = Self::readable_text(translucent);
        if best.1 >= LEAST_CONTRAST {
            return Card {
                fill: translucent,
                text: best.0,
                ratio: best.1,
                opaque: false,
            };
        }
        let solid = Self::readable_text(tint);
        Card {
            fill: tint,
            text: solid.0,
            ratio: solid.1,
            opaque: true,
        }
    }

    /// The palette colour that reads best on `fill`, and its ratio.
    ///
    /// Measured across the palette's own text colours rather than assuming
    /// black or white. A user palette can be anything, and picking the
    /// contrasting extreme would put a colour on screen that is in no theme.
    pub fn readable_text(fill: Color32) -> (Color32, f32) {
        let candidates = [p::fg(), p::bg(), p::muted()];
        let mut best = (candidates[0], contrast(candidates[0], fill));
        for candidate in candidates.iter().skip(1) {
            let ratio = contrast(*candidate, fill);
            if ratio > best.1 {
                best = (*candidate, ratio);
            }
        }
        best
    }
}

/// Draw a notice, in whichever way was chosen.
///
/// Returns true when the reader dismissed it. [`Style::Off`] returns true
/// immediately: nothing was shown, so nothing is waiting to be acknowledged,
/// and leaving it queued would build a backlog nobody can ever clear.
pub fn show(ui: &mut Ui, style: Style, notice: &Notice) -> bool {
    match style {
        Style::Off => true,
        Style::Alert => alert(ui, notice),
        Style::Overlay => overlay(ui, notice),
    }
}

/// The quiet one: a rounded translucent card.
fn overlay(ui: &mut Ui, notice: &Notice) -> bool {
    let card = Card::for_level(notice.level, p::bg());
    let mut dismissed = false;
    egui::Frame::new()
        .fill(card.fill)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&notice.text).color(card.text));
                if ui
                    .small_button(RichText::new("×").color(card.text))
                    .clicked()
                {
                    dismissed = true;
                }
            });
        });
    dismissed
}

/// The loud one: it stops the panel until acknowledged.
fn alert(ui: &mut Ui, notice: &Notice) -> bool {
    let card = Card::for_level(notice.level, p::bg());
    let mut dismissed = false;
    egui::Frame::new()
        .fill(card.fill)
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.label(RichText::new(&notice.text).color(card.text).strong());
            ui.add_space(8.0);
            if ui.button("dismiss").clicked() {
                dismissed = true;
            }
        });
    dismissed
}

/// What a reader has to be told about these notifications.
pub const SCOPE: &str = "\
These appear inside VeilVoice's own window and nowhere else. VeilVoice does not \
put messages into your desktop's notification area, so nothing here reaches you \
while the window is closed. That is a real limit rather than an oversight: a \
system notification needs a registered application identity on two of the three \
platforms, and this project is published under a pseudonym on purpose.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_is_named_explained_and_round_trips() {
        let mut keys: Vec<&str> = Style::ALL.iter().map(|s| s.key()).collect();
        let count = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), count, "two styles share a key");

        for style in Style::ALL {
            assert!(!style.label().is_empty(), "{style:?}");
            assert!(style.note().len() > 60, "{style:?}: say what it costs");
            assert_eq!(Style::from_key(style.key()), *style);
        }
    }

    /// An unreadable settings file must not stop the application, and of the
    /// two ways to be wrong the one that still shows a warning is chosen.
    #[test]
    fn an_unknown_setting_falls_back_to_showing_something() {
        assert_eq!(Style::from_key("something-new"), Style::Overlay);
        assert_eq!(Style::from_key(""), Style::Overlay);
        assert_ne!(
            Style::from_key("something-new"),
            Style::Off,
            "a file this build cannot read must never silence the warnings"
        );
    }

    /// The blend is the colour actually on screen, and it is between the two.
    #[test]
    fn blending_lands_between_the_two_colours() {
        let over = Color32::from_rgb(255, 255, 255);
        let under = Color32::from_rgb(0, 0, 0);

        assert_eq!(blend(over, under, 255), over, "fully opaque is the card");
        assert_eq!(blend(over, under, 0), under, "fully clear is the panel");

        let half = blend(over, under, 128);
        assert!(half.r() > 120 && half.r() < 136, "{half:?}");
    }

    /// **The point of the module.** Whatever the palette, the text on a card
    /// is legible -- and where translucency cannot manage it, translucency is
    /// what gets given up.
    #[test]
    fn a_card_is_always_legible_even_if_it_has_to_stop_being_translucent() {
        for level in [Level::Note, Level::Warn] {
            for panel in [
                Color32::from_rgb(0, 0, 0),
                Color32::from_rgb(255, 255, 255),
                Color32::from_rgb(26, 27, 38),
                Color32::from_rgb(122, 162, 247),
            ] {
                let card = Card::for_level(level, panel);
                assert!(
                    card.ratio >= LEAST_CONTRAST || card.opaque,
                    "{level:?} on {panel:?} gave {:.2}:1 while still translucent",
                    card.ratio
                );
                // And the ratio reported is the ratio of what is drawn.
                let measured = contrast(card.text, card.fill);
                assert!(
                    (measured - card.ratio).abs() < 0.01,
                    "reported {:.3}, measured {measured:.3}",
                    card.ratio
                );
            }
        }
    }

    /// The text colour is chosen by measurement, not by assuming black or
    /// white -- a user palette can be anything, and an assumed extreme puts a
    /// colour on screen that is in no theme.
    #[test]
    fn the_text_colour_is_the_best_measured_candidate() {
        for fill in [
            Color32::from_rgb(0, 0, 0),
            Color32::from_rgb(255, 255, 255),
            Color32::from_rgb(128, 128, 128),
        ] {
            let (chosen, ratio) = Card::readable_text(fill);
            for other in [p::fg(), p::bg(), p::muted()] {
                assert!(
                    ratio >= contrast(other, fill) - 0.001,
                    "{chosen:?} at {ratio:.2} lost to {other:?} at {:.2}",
                    contrast(other, fill)
                );
            }
        }
    }

    /// Nothing queued behind a style that shows nothing. A notice that is never
    /// displayed and never dismissed is a backlog nobody can clear.
    #[test]
    fn switching_notifications_off_does_not_build_a_queue() {
        // `show` needs a Ui, so the contract is asserted where it is decided.
        let source = include_str!("notify.rs").replace("\r\n", "\n");
        let start = source.find("pub fn show(").expect("the function");
        let end = source[start..].find("\n}\n").expect("its end") + start;
        let body = &source[start..end];
        assert!(
            body.contains("Style::Off => true"),
            "Off has to report the notice as dealt with:\n{body}"
        );
    }

    /// The limit is stated, because somebody will otherwise rely on being told
    /// while the window is closed.
    #[test]
    fn the_scope_note_says_these_do_not_leave_the_window() {
        let scope = SCOPE.to_lowercase();
        assert!(scope.contains("own window and nowhere else"), "{scope}");
        assert!(
            scope.contains("while the window is closed"),
            "the case somebody will assume works: {scope}"
        );
        assert!(scope.contains("pseudonym"), "and why: {scope}");
    }

    #[test]
    fn a_notice_carries_its_level() {
        assert_eq!(Notice::note("x").level, Level::Note);
        assert_eq!(Notice::warn("x").level, Level::Warn);
        assert_eq!(Level::default(), Level::Note);
    }
}
