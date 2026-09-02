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

use crate::theme::palette as p;
use egui::{RichText, Ui};

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
        "live",
        "Live scramble",
        "The same thing on a microphone as you speak, into a virtual cable \
         that other programs can listen to. For a call rather than a file.",
    ),
    (
        "group",
        "Group",
        "One recording with several people in it. Each gets a different voice, \
         so a listener can still follow who is who, and every voiceprint is \
         destroyed just as thoroughly.",
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
         at your keyboard, not somebody with your disk.",
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

    /// Draw the current card. Returns true once the tour has finished.
    ///
    /// `installed` decides the sentence on the last card, and it is a fact
    /// about where this binary is rather than a preference.
    pub fn panel(&mut self, ui: &mut Ui, installed: bool) -> bool {
        let Some(at) = self.at else { return false };
        let Some(&card) = self.showing.get(at) else {
            self.stop();
            return true;
        };
        let (_, title, body) = CARDS[card];
        let last = at + 1 >= self.showing.len();

        // A reading measure rather than the window's width. At 1400 pixels a
        // sentence runs the whole way across and the eye loses the line coming
        // back; the usual advice is 60 to 80 characters and this is about 75 at
        // the default size. The window can be any width and the card should not
        // get harder to read as it grows.
        ui.set_max_width(720.0);
        ui.add_space(18.0);
        ui.label(
            RichText::new(format!("{} of {}", at + 1, self.showing.len()))
                .small()
                .color(p::muted()),
        );
        ui.add_space(4.0);
        ui.label(RichText::new(title).size(18.0).color(p::fg()).strong());
        ui.add_space(8.0);
        ui.label(RichText::new(body).color(p::fg()));

        if last {
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

        ui.add_space(18.0);
        let mut finished = false;
        ui.horizontal(|ui| {
            let next = if last { "done" } else { "next" };
            if ui.button(next).clicked() {
                if last {
                    self.stop();
                    finished = true;
                } else {
                    self.at = Some(at + 1);
                }
            }
            if at > 0 && ui.button("back").clicked() {
                self.at = Some(at - 1);
            }
            if !last && ui.button("skip the rest").clicked() {
                self.stop();
                finished = true;
            }
        });
        finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
