// SPDX-License-Identifier: GPL-3.0-or-later
//! The short tour on a first run, and after an upgrade.
//!
//! # What it is for
//!
//! The window has nine tabs and nothing said what any of them were. Somebody
//! opening this for the first time met a tab strip and had to guess, and two
//! of the nine, Monitor and Lock, are not what their names suggest to a person
//! who has not read the documentation.
//!
//! So: one card per tab, one sentence each, skippable at any point, and gone
//! for good once seen. It is not a walkthrough with arrows pointing at
//! controls. It is the paragraph a person would have read in a manual, offered
//! at the moment they would have wanted it, and it takes about twenty seconds.
//!
//! # Why it comes back after an upgrade
//!
//! Only as far as the tabs that are new. A tour that replays in full on every
//! upgrade is a tour people learn to skip, and one that never comes back means
//! a tab added in a later release is never introduced to anybody who was
//! already a user.
//!
//! What is stored is the list of tabs that were toured, not a "seen" flag and
//! not the version number. A flag cannot answer the question an upgrade asks,
//! and the version can only answer it indirectly: comparing versions tells you
//! *that* something changed, and the tab list tells you *what*, which is the
//! thing being shown. It also means a release that adds no tab shows nobody
//! anything, which is the common case and the right behaviour for it.
//!
//! # Portable or installed
//!
//! The last card says which one this copy is, in those words, because it is
//! the question behind "where did my settings go" and "why is it not in my
//! menu". It is a statement rather than a prompt: `Install` is a tab, the
//! decision is made there, and a tour is a bad place to ask somebody to commit
//! to anything.
//!
//! # It is drawn over the window, not instead of it
//!
//! **Roadmap item 171.** This used to replace the whole body of the window:
//! `update` drew the card and returned, so the tabs it was describing were not
//! on screen while it described them. A card saying what the Monitor tab is
//! for, with no Monitor tab visible anywhere, is a paragraph from a manual
//! rather than a tour of anything.
//!
//! Now it is an [`egui::Modal`]: the window draws normally underneath, dimmed,
//! and the card sits over it. The reader can see the tab strip the card is
//! talking about. The backdrop takes the clicks, so nothing behind it can be
//! pressed by accident, and the tab strip is drawn disabled as well so that it
//! looks as unavailable as it is.
//!
//! **Escape closes it and a click on the shade does not.** Those are usually
//! the same gesture on a modal, and they should not be here: a modal asks a
//! question and clicking away from it means "not now", where a tour is
//! something somebody is stepping through, and a misjudged click halfway along
//! should not throw away the half they have not seen. Escape is deliberate and
//! the buttons are deliberate. A stray click is not.
//!
//! # It can be asked for again
//!
//! Settings, under Interface. The stored tab list decides whether the tour
//! *offers* itself; it has nothing to say about whether somebody may ask for
//! it, and before this there was no way to ask. Somebody who skipped it on the
//! day they installed VeilVoice had no route back to it at all, which made the
//! skip button a permanent decision taken in the first thirty seconds.
//!
//! A person asking gets every card, including the ones they have seen, because
//! "show me that again" is not a question about which tabs are new.

use crate::theme::palette as p;
use egui::{RichText, Ui};

/// How wide the card is allowed to get.
///
/// A reading measure rather than the window's width. At 1400 pixels a sentence
/// runs the whole way across and the eye loses the line coming back; the usual
/// advice is 60 to 80 characters and this is about 75 at the default size. The
/// window can be any width and the card should not get harder to read as it
/// grows.
const CARD_WIDTH: f32 = 720.0;

/// How much of the window's height the card may occupy before its text
/// scrolls.
///
/// A share rather than a number of pixels, because the thing being protected
/// is the shade around the card: a card that reaches the edges is not an
/// overlay any more, it is the replacement this stopped being. Below this the
/// text scrolls inside the card, so a short window never hides the buttons,
/// which is the same rule the tab bodies follow.
const CARD_HEIGHT_SHARE: f32 = 0.8;

/// One card: the tab it is about, and what that tab is for.
///
/// The keys match `Tab::key`, and `app.rs` has a test that every tab has a
/// card and every card has a tab, so a tab added without a sentence fails the
/// build rather than shipping unexplained.
pub const CARDS: &[(&str, &str, &str)] = &[
    (
        "file",
        "Anonymise file",
        "A recording in, the same words in a voice nobody owns out. Encrypted \
         at rest by default, because the words survive on purpose and a file \
         anybody can read is a transcript anybody can read.",
    ),
    (
        "group",
        "Group",
        "One recording with several people in it. Each gets a different voice, \
         so a listener can still follow who is who, and every voiceprint is \
         destroyed just as thoroughly.",
    ),
    (
        "studio",
        "Studio",
        "The same thing on a microphone as you speak, into a virtual cable \
         that other programs can listen to, and a locked vault to keep what \
         was said in if you want one. The vault opens with both of your \
         passphrases at once, the one on this application and the one on your \
         recordings, and neither on its own.",
    ),
    (
        "browser",
        "Browser",
        "What is in the vault. The names and dates are sealed with the \
         recordings, so a disk shows how many files there are and roughly how \
         large, and nothing about what any of them is.",
    ),
    (
        "monitor",
        "Monitor",
        "Not a level meter. It watches for another program picking up a real \
         microphone while you are being veiled, which is the way this can \
         quietly fail to protect you.",
    ),
    (
        "lock",
        "Lock",
        "A passphrase on this application, separate from the one on any \
         recording. Worth what a lock on a drawer is worth: it stops somebody \
         at your keyboard, not somebody with your disk. This tab also shows \
         what VeilVoice's own files looked like when it first ran, and whether \
         they still do. That record is taken without being asked for, and is \
         sealed with your passphrase if you set one here.",
    ),
    (
        "verify",
        "Verify",
        "Check that a VeilVoice download is the one that was published, using \
         the signature and, if you have GnuPG, your own copy of it as well.",
    ),
    (
        "settings",
        "Settings",
        "Theme, animation, autolock, and what the interface tells you. \
         Everything here is stored beside the application and goes nowhere.",
    ),
    (
        "install",
        "Install",
        "Put this copy somewhere permanent, or leave it where it is. Either \
         works. The tab takes itself away once there is nothing left to do.",
    ),
    (
        "about",
        "About",
        "Versions, what drew the window, what the lock covers, and somewhere \
         to report a fault.",
    ),
];

/// Where the tour is up to.
#[derive(Default)]
pub struct Tour {
    /// Which card is showing. `None` means it is not running.
    at: Option<usize>,
    /// The cards this run is showing, as indices into [`CARDS`].
    showing: Vec<usize>,
}

/// Every tab key the tour knows, for storing once it has run.
pub fn all_keys() -> Vec<String> {
    CARDS.iter().map(|(key, _, _)| (*key).to_string()).collect()
}

impl Tour {
    /// Start the tour from the beginning, showing every card.
    pub fn start(&mut self) {
        self.showing = (0..CARDS.len()).collect();
        self.at = Some(0);
    }

    /// Start it showing only the cards whose tabs are not in `known`.
    ///
    /// Used after an upgrade: somebody who has been using this for months is
    /// shown what is new and nothing else. If nothing is new, nothing runs.
    pub fn start_new_only(&mut self, known: &[String]) {
        self.showing = CARDS
            .iter()
            .enumerate()
            .filter(|(_, (key, _, _))| !known.iter().any(|seen| seen == key))
            .map(|(index, _)| index)
            .collect();
        self.at = if self.showing.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Whether the tour is on screen.
    pub fn running(&self) -> bool {
        self.at.is_some()
    }

    /// Stop it.
    pub fn stop(&mut self) {
        self.at = None;
        self.showing.clear();
    }

    /// Start it again from the beginning, because somebody asked.
    ///
    /// The same thing [`start`](Self::start) does, and a separate name because
    /// the two mean different things: `start` is the window deciding that a
    /// reader has not seen this, and this is a reader saying "show me that
    /// again". The second one is not a question about which tabs are new, so
    /// it shows every card whatever is stored.
    pub fn restart(&mut self) {
        self.start();
    }

    /// Draw the current card over the window. Returns true once it finished.
    ///
    /// `installed` decides the sentence on the last card, and it is a fact
    /// about where this binary is rather than a preference.
    ///
    /// Takes the context rather than a `Ui`, because it is no longer drawn
    /// inside anything: the window paints its tabs as usual and this goes on
    /// top of all of it. See the module note for why that is the whole point
    /// of roadmap item 171 and not a presentational change.
    pub fn overlay(&mut self, ctx: &egui::Context, installed: bool) -> bool {
        let Some(at) = self.at else { return false };
        let Some(&card) = self.showing.get(at) else {
            self.stop();
            return true;
        };
        let (_, title, body) = CARDS[card];
        let last = at + 1 >= self.showing.len();
        let total = self.showing.len();
        // `content_rect` rather than the whole window: it is the area inside
        // the panels, which is what the modal's own backdrop covers, and it is
        // what the card has to fit inside.
        let height = ctx.content_rect().height() * CARD_HEIGHT_SHARE;

        let modal = egui::Modal::new(egui::Id::new("veilvoice-tour"))
            .frame(
                egui::Frame::new()
                    .fill(p::surface())
                    .stroke(egui::Stroke::new(1.0, p::border()))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(24, 20)),
            )
            .show(ctx, |ui| {
                ui.set_max_width(CARD_WIDTH);
                let mut step = Step::Stay;

                // The text scrolls and the buttons do not. On a window shorter
                // than the card, a single scroller around the whole thing puts
                // "next" below the fold, and a tour whose only way forward has
                // to be scrolled to is a tour that looks stuck.
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} of {total}", at + 1))
                                .small()
                                .color(p::muted()),
                        );
                        ui.add_space(4.0);
                        ui.label(RichText::new(title).size(18.0).color(p::fg()).strong());
                        ui.add_space(8.0);
                        ui.label(RichText::new(body).color(p::fg()));
                        if last {
                            this_copy(ui, installed);
                        }
                    });

                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    let next = if last { "done" } else { "next" };
                    if ui.button(next).clicked() {
                        step = if last { Step::Finish } else { Step::Forward };
                    }
                    if at > 0 && ui.button("back").clicked() {
                        step = Step::Back;
                    }
                    if !last && ui.button("skip the rest").clicked() {
                        step = Step::Finish;
                    }
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "  Escape closes this. Settings, under Interface, brings it back.",
                    )
                    .small()
                    .color(p::muted()),
                );
                step
            });

        // Escape, and deliberately not a click on the shade. `should_close`
        // treats the two as one gesture, which is right for a dialogue asking
        // a question and wrong for something somebody is stepping through: a
        // misjudged click should not throw away the half they have not read.
        // The other two conditions are `should_close`'s own, and they matter:
        // Escape belongs to whatever is on top, and to an open dropdown before
        // this.
        let escaped = modal.is_top_modal
            && !modal.any_popup_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));

        match modal.inner {
            Step::Stay if escaped => {
                self.stop();
                true
            }
            Step::Stay => false,
            Step::Forward => {
                self.at = Some(at + 1);
                false
            }
            Step::Back => {
                self.at = Some(at.saturating_sub(1));
                false
            }
            Step::Finish => {
                self.stop();
                true
            }
        }
    }
}

/// What a frame of the card asked for.
///
/// Returned out of the modal rather than acted on inside it, because the
/// closure holds `&mut self` for as long as it runs and the Escape key cannot
/// be read until it has finished. One value out, one decision made, in one
/// place.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Nothing was pressed.
    Stay,
    /// The next card.
    Forward,
    /// The one before.
    Back,
    /// Done, or skipped: the two end the tour the same way, because both mean
    /// the reader is finished with it and neither should bring it back on the
    /// next launch.
    Finish,
}

/// The closing paragraph on the last card: which kind of copy this is.
fn this_copy(ui: &mut Ui, installed: bool) {
    ui.add_space(16.0);
    ui.label(RichText::new("This copy").color(p::cyan()).strong());
    ui.label(
        RichText::new(if installed {
            "Installed. It is on this machine for good, it is on your \
             menu or path, and its settings live in your account. \
             Removing it is the same as removing any other program."
        } else {
            "Portable. It runs from wherever you put it and installs \
             nothing: move the folder and VeilVoice moves with it, \
             delete the folder and it is gone. That is a perfectly \
             good way to keep using it. The Install tab is there if \
             you would rather it were permanent."
        })
        .color(p::fg()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lock card says the integrity record is taken for you, and the
    /// window is what takes it.
    ///
    /// Both halves, because a sentence in a tour is a promise about behaviour
    /// and this one was written the day the behaviour it describes had been
    /// there for four releases with nothing telling anybody. If the start in
    /// `app.rs` is ever removed, the card is left claiming something untrue to
    /// every new reader, which is the failure this checks for.
    #[test]
    fn the_lock_card_promises_what_the_window_actually_does() {
        let card = CARDS
            .iter()
            .find(|(tab, _, _)| *tab == "lock")
            .expect("a card for the lock tab");
        assert!(
            card.2.contains("record"),
            "the lock card no longer mentions the integrity record: {}",
            card.2
        );
        let app = include_str!("app.rs");
        assert!(
            app.contains("app.integrity.start(None)"),
            "nothing starts the integrity record at launch any more, and the \
             tour still tells every new reader that one was taken"
        );
    }

    #[test]
    fn every_card_has_a_sentence_and_no_two_share_a_tab() {
        let mut keys: Vec<&str> = CARDS.iter().map(|(key, _, _)| *key).collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), before, "two cards claim the same tab");
        for (key, title, body) in CARDS {
            assert!(!key.is_empty() && !title.is_empty());
            assert!(
                body.len() > 60,
                "{key} has a sentence too short to explain anything"
            );
        }
    }

    #[test]
    fn a_first_run_sees_everything() {
        let mut tour = Tour::default();
        assert!(!tour.running());
        tour.start();
        assert!(tour.running());
        assert_eq!(tour.showing.len(), CARDS.len());
    }

    #[test]
    fn an_upgrade_shows_only_what_is_new() {
        let known: Vec<String> = CARDS
            .iter()
            .take(CARDS.len() - 2)
            .map(|(key, _, _)| (*key).to_string())
            .collect();
        let mut tour = Tour::default();
        tour.start_new_only(&known);
        assert!(tour.running(), "two new tabs should start a tour");
        assert_eq!(tour.showing.len(), 2);
    }

    #[test]
    fn what_is_stored_is_every_tab_the_tour_covered() {
        // The stored list is what "which of these is new to you" is answered
        // against, so it has to be complete when the tour finishes.
        assert_eq!(all_keys().len(), CARDS.len());
    }

    /// Roadmap item 171. Asking for it is not a question about which tabs are
    /// new, so it shows all of them whatever is stored.
    #[test]
    fn asking_for_it_again_shows_every_card() {
        let every: Vec<String> = CARDS.iter().map(|(key, _, _)| (*key).to_string()).collect();
        let mut tour = Tour::default();
        // The state somebody in this position is actually in: they have seen
        // the whole thing, so `start_new_only` would show them nothing.
        tour.start_new_only(&every);
        assert!(!tour.running());

        tour.restart();
        assert!(tour.running(), "asking for it must start it");
        assert_eq!(
            tour.showing.len(),
            CARDS.len(),
            "a reader who asked to see it again was shown a subset"
        );
    }

    /// Roadmap item 171. The tour is drawn over the window rather than instead of
    /// it, and that is a claim about `app.rs` as much as about this file: the
    /// whole point is lost if the window returns before it draws the tabs the
    /// cards are describing.
    ///
    /// Read from the source, because what is asserted is that a statement is
    /// *not reachable*. A rendering test would prove it for one frame.
    #[test]
    fn the_window_still_draws_underneath_the_tour() {
        let app = include_str!("app.rs").replace("\r\n", "\n");
        let at = app
            .find("self.tour.overlay(")
            .expect("the window draws the tour as an overlay");
        let central = app
            .rfind("egui::CentralPanel::default().show(root, |ui| {")
            .expect("the window has a central panel");
        assert!(
            at > central,
            "the tour is drawn inside the central panel again, so it replaces \
             the tabs it is describing rather than sitting over them"
        );
        assert!(
            !app.contains("self.tour.panel("),
            "the old panel that returned instead of drawing the window is back"
        );
    }

    /// The card has to fit in the window it is drawn over, and its buttons have
    /// to stay reachable. Both are properties of the numbers rather than of a
    /// frame, so both are checked here.
    #[test]
    fn the_card_leaves_the_window_visible_around_it() {
        // Against the real window sizes rather than against the share on its
        // own: what has to hold is that there is window left around the card,
        // and on the shortest window this application will open at, which is
        // where it is tightest.
        for height in [crate::window::MINIMUM[1], 900.0, 1440.0] {
            let card = height * CARD_HEIGHT_SHARE;
            let shade = height - card;
            assert!(
                shade >= 48.0,
                "at {height} the shade around the card is {shade} pixels, too \
                 thin to read as an overlay rather than as a replacement"
            );
            assert!(
                card > 200.0,
                "at {height} the card is {card} pixels and a sentence does not \
                 fit in it"
            );
        }
        for width in [crate::window::MINIMUM[0], 1100.0, 1920.0] {
            assert!(
                CARD_WIDTH <= width,
                "at {width} across, the card is wider than the window and \
                 would be cut off"
            );
        }
    }

    /// Escape is handled and a click on the shade is deliberately not.
    ///
    /// `ModalResponse::should_close` treats the two as one gesture. Using it
    /// would mean a misjudged click halfway through the tour threw away the
    /// half the reader had not seen, so this asserts the code does not call it.
    #[test]
    fn a_stray_click_outside_the_card_does_not_end_the_tour() {
        let source = include_str!("tour.rs").replace("\r\n", "\n");
        let overlay = source.find("pub fn overlay(").expect("the overlay exists");
        // Bounded at the end of the function. Unbounded, this reads its own
        // test as part of the code it is checking and fails on the string it
        // is looking for, which is a guard that can only ever fail.
        let end = source[overlay..]
            .find("\nenum Step {")
            .map(|at| overlay + at)
            .expect("the step enum follows the overlay");
        let body = &source[overlay..end];
        assert!(
            body.contains("Key::Escape"),
            "nothing reads Escape, so the overlay has no keyboard way out"
        );
        assert!(
            !body.contains("should_close()"),
            "should_close() closes on a backdrop click as well, which ends the \
             tour on a stray press"
        );
    }

    #[test]
    fn an_upgrade_that_adds_no_tabs_shows_nothing() {
        let known: Vec<String> = CARDS.iter().map(|(key, _, _)| (*key).to_string()).collect();
        let mut tour = Tour::default();
        tour.start_new_only(&known);
        assert!(
            !tour.running(),
            "a tour with nothing new to say must not run"
        );
    }
}
