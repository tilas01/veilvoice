// SPDX-License-Identifier: GPL-3.0-or-later
//! The first run: the four things worth deciding before anything else.
//!
//! # What this replaced
//!
//! Two checkboxes about animation. Everything that actually matters -- the app
//! lock, the passphrase recordings are encrypted with, whether the window locks
//! itself -- was left to be discovered on a tab most people never opened.
//!
//! That is a defensible choice for a preference and a bad one for a
//! protection. A default nobody is shown is not a question, it is an answer,
//! and for a privacy tool the answer it was quietly giving was "none of it".
//!
//! # What it asks, and what it will not do
//!
//! Four cards, each skippable, each stating what it buys before asking for
//! anything:
//!
//! 1. **Appearance.** The two animation choices, kept from the old panel.
//! 2. **The app lock.** A passphrase for the window, and -- since 0.1.18 --
//!    the key that names and encrypts VeilVoice's own files. The card says
//!    both, and says the sentence that has to be said out loud: forget it and
//!    those files are gone.
//! 3. **The recording passphrase.** What veiled recordings are encrypted
//!    with. Separate from the app lock by default, with the option to use one
//!    passphrase for both and a plain statement of what that trades.
//! 4. **Locking itself.** On at half an hour, with the delay and the off
//!    switch right there.
//!
//! **Nothing here is a gate.** Every card has a way past it, and skipping all
//! four leaves VeilVoice exactly as it was before this module existed. A setup
//! flow that will not let somebody reach the program is a setup flow they
//! resent; this one is a set of offers made at the moment they make sense.
//!
//! The tour runs after it, so a person meets the decisions first and the tabs
//! second, which is the order they matter in.
//!
//! # In plain words
//!
//! The first time you open VeilVoice it offers you a password for the app, a
//! password for your recordings, and a timer that locks the window when you
//! walk away. You can skip any of them and set them later.

use crate::theme::palette as p;
use egui::{RichText, Ui};

/// Which card is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Step {
    /// How the interface should look. First because it is the lightest, and
    /// because somebody who bounces off the setup entirely has still answered
    /// the one question with no security consequence.
    #[default]
    Appearance,
    /// A passphrase for the window, and for VeilVoice's own files.
    AppLock,
    /// A passphrase for the recordings themselves.
    Recording,
    /// Whether the window locks itself, and after how long.
    Autolock,
}

impl Step {
    fn next(self) -> Option<Self> {
        match self {
            Self::Appearance => Some(Self::AppLock),
            Self::AppLock => Some(Self::Recording),
            Self::Recording => Some(Self::Autolock),
            Self::Autolock => None,
        }
    }

    /// One-based position, for "step 2 of 4".
    fn position(self) -> usize {
        match self {
            Self::Appearance => 1,
            Self::AppLock => 2,
            Self::Recording => 3,
            Self::Autolock => 4,
        }
    }

    const COUNT: usize = 4;
}

/// What the setup is holding while it runs.
#[derive(Default)]
pub struct FirstRun {
    /// The card showing.
    pub step: Step,
    /// Typed app-lock passphrase, and its confirmation.
    lock_entry: String,
    lock_repeat: String,
    /// Typed recording passphrase, and its confirmation.
    rec_entry: String,
    rec_repeat: String,
    /// Whether the recording passphrase should be the app-lock one.
    same_passphrase: bool,
    /// Set once the lock has been asked for, so the card stops offering.
    lock_requested: bool,
}

/// What the panel wants the application to do after drawing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Outcome {
    /// Still going.
    #[default]
    Continue,
    /// Every card is answered or skipped.
    Finished,
}

impl FirstRun {
    /// Draw the current card.
    ///
    /// Takes the settings and the security state because it changes both, and
    /// returns whether it is done rather than deciding that itself: the caller
    /// owns what happens next, which is the tour.
    pub fn panel(
        &mut self,
        ui: &mut Ui,
        prefs: &mut crate::settings::Settings,
        security: &mut crate::security::Security,
    ) -> Outcome {
        ui.add_space(18.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("Setting up VeilVoice")
                    .size(20.0)
                    .color(p::fg())
                    .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "step {} of {}. Every one of them can be skipped, and \
                     changed later in Settings.",
                    self.step.position(),
                    Step::COUNT
                ))
                .small()
                .color(p::muted()),
            );
        });
        ui.add_space(16.0);

        let advance = match self.step {
            Step::Appearance => self.appearance(ui, prefs),
            Step::AppLock => self.app_lock(ui, security),
            Step::Recording => self.recording(ui, security),
            Step::Autolock => self.autolock(ui, prefs),
        };

        if advance {
            match self.step.next() {
                Some(next) => {
                    self.step = next;
                    // A card whose question is already answered elsewhere has
                    // nothing to ask, so it is stepped past rather than shown
                    // with everything greyed out.
                    if self.should_skip(self.step, security) {
                        if let Some(after) = self.step.next() {
                            self.step = after;
                        } else {
                            return Outcome::Finished;
                        }
                    }
                }
                None => return Outcome::Finished,
            }
        }
        Outcome::Continue
    }

    /// Whether a card has nothing left to ask.
    ///
    /// The app lock and the recording passphrase can both be set already --
    /// from the command line, from a previous run, or from a copied
    /// configuration. Asking somebody to set a thing they have set is how a
    /// setup flow teaches people to click through it without reading.
    fn should_skip(&self, step: Step, security: &crate::security::Security) -> bool {
        match step {
            Step::AppLock => security.has_lock(),
            Step::Recording => security.has_recording_passphrase(),
            _ => false,
        }
    }

    fn appearance(&mut self, ui: &mut Ui, prefs: &mut crate::settings::Settings) -> bool {
        card(ui, "How it should look", |ui| {
            ui.label(
                RichText::new(
                    "Both are on. Nothing here leaves your machine, and neither \
                     affects what VeilVoice does to a recording.",
                )
                .color(p::muted()),
            );
            ui.add_space(10.0);
            prefs.first_run_appearance(ui);
        });
        buttons(ui, "continue", None).0
    }

    fn app_lock(&mut self, ui: &mut Ui, security: &mut crate::security::Security) -> bool {
        let mut advance = false;
        card(ui, "A password for VeilVoice itself", |ui| {
            ui.label(RichText::new(
                "It stops somebody who picks up your unlocked computer from \
                 opening VeilVoice, seeing what you have processed, or starting \
                 a live scramble.",
            ));
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "It also encrypts VeilVoice's own files and gives them \
                     meaningless names, with decoy files among them, so the \
                     folder says nothing about what you have done. Without a \
                     password none of that is possible: there is no key.",
                )
                .color(p::muted()),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Forget this password and those files are gone. It is not a \
                     lock you can take off; it is the only way back to them.",
                )
                .color(p::yellow()),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "It is not protection against somebody who has your disk. \
                     For that, use full-volume encryption as well.",
                )
                .small()
                .color(p::muted()),
            );
            ui.add_space(12.0);

            if self.lock_requested {
                ui.label(RichText::new("setting it now…").color(p::muted()));
                if security.has_lock() {
                    advance = true;
                }
                return;
            }

            field(ui, "password", &mut self.lock_entry);
            field(ui, "again", &mut self.lock_repeat);
            let matched = !self.lock_entry.is_empty() && self.lock_entry == self.lock_repeat;
            if !self.lock_entry.is_empty() && !matched {
                ui.label(
                    RichText::new("the two entries differ")
                        .color(p::yellow())
                        .small(),
                );
            }
            ui.add_space(8.0);
            if ui
                .add_enabled(matched, egui::Button::new("set this password"))
                .clicked()
            {
                let entry = std::mem::take(&mut self.lock_entry);
                self.lock_repeat.clear();
                security.set_lock_from_setup(entry);
                self.lock_requested = true;
            }
        });
        let (next, _) = buttons(ui, "skip for now", None);
        advance || next
    }

    fn recording(&mut self, ui: &mut Ui, security: &mut crate::security::Security) -> bool {
        let mut advance = false;
        card(ui, "A password for your recordings", |ui| {
            ui.label(RichText::new(
                "Veiled recordings are encrypted before they are written. This \
                 is what opens them again.",
            ));
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "A different password from the one above, by default, \
                     because they protect different things: that one guards a \
                     session, this one guards files that outlive it.",
                )
                .color(p::muted()),
            );
            ui.add_space(10.0);

            if security.has_lock() {
                ui.checkbox(
                    &mut self.same_passphrase,
                    "Use the VeilVoice password for recordings too",
                );
                ui.label(
                    RichText::new(
                        "  One password to remember instead of two. What it \
                         trades is that one password then opens the \
                         application and everything it has written.",
                    )
                    .small()
                    .color(p::muted()),
                );
                ui.add_space(8.0);
            }

            if self.same_passphrase {
                security.prefer_app_lock_sealing(true);
                ui.label(
                    RichText::new(
                        "Recordings will be sealed with the VeilVoice password. \
                         Unlock once and it is in hand.",
                    )
                    .color(p::muted()),
                );
                return;
            }

            field(ui, "password", &mut self.rec_entry);
            field(ui, "again", &mut self.rec_repeat);
            let matched = !self.rec_entry.is_empty() && self.rec_entry == self.rec_repeat;
            if !self.rec_entry.is_empty() && !matched {
                ui.label(
                    RichText::new("the two entries differ")
                        .color(p::yellow())
                        .small(),
                );
            }
            ui.add_space(8.0);
            if ui
                .add_enabled(matched, egui::Button::new("use this password"))
                .clicked()
            {
                let entry = std::mem::take(&mut self.rec_entry);
                self.rec_repeat.clear();
                security.set_recording_passphrase(entry);
                advance = true;
            }
        });
        let (next, _) = buttons(ui, "skip for now", None);
        advance || next
    }

    fn autolock(&mut self, ui: &mut Ui, prefs: &mut crate::settings::Settings) -> bool {
        card(ui, "Locking itself when you walk away", |ui| {
            ui.label(RichText::new(
                "VeilVoice locks its window again after half an hour with \
                 nobody touching it.",
            ));
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "A job running does not count as touching it. If you start \
                     a long render and leave the room, that is exactly when you \
                     would want it locked.",
                )
                .small()
                .color(p::muted()),
            );
            ui.add_space(12.0);
            prefs.first_run_autolock(ui);
        });
        buttons(ui, "finish", None).0
    }
}

/// A bordered card, so each step reads as one thing rather than a page of text.
fn card(ui: &mut Ui, title: &str, contents: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(p::bg_dark())
        .stroke(egui::Stroke::new(1.0, p::border()))
        .inner_margin(16.0)
        .corner_radius(6.0)
        .show(ui, |ui| {
            ui.set_max_width(560.0);
            ui.label(RichText::new(title).size(16.0).color(p::fg()).strong());
            ui.add_space(10.0);
            contents(ui);
        });
}

/// A password field with its label, laid out like the rest of the application.
fn field(ui: &mut Ui, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(p::muted()));
        ui.add(
            egui::TextEdit::singleline(value)
                .password(true)
                .desired_width(240.0),
        );
    });
}

/// The row that moves on. Returns whether it was pressed.
fn buttons(ui: &mut Ui, forward: &str, _unused: Option<&str>) -> (bool, bool) {
    ui.add_space(14.0);
    let mut pressed = false;
    crate::layout::centred_row(ui, |ui| {
        pressed = ui.button(forward).clicked();
    });
    (pressed, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_run_in_order_and_then_stop() {
        let mut step = Step::default();
        let mut seen = vec![step];
        while let Some(next) = step.next() {
            step = next;
            seen.push(step);
        }
        assert_eq!(
            seen,
            vec![
                Step::Appearance,
                Step::AppLock,
                Step::Recording,
                Step::Autolock
            ]
        );
        assert_eq!(seen.len(), Step::COUNT);
    }

    #[test]
    fn every_step_knows_where_it_is() {
        let mut step = Step::default();
        let mut expected = 1;
        loop {
            assert_eq!(step.position(), expected);
            match step.next() {
                Some(next) => {
                    step = next;
                    expected += 1;
                }
                None => break,
            }
        }
        assert_eq!(expected, Step::COUNT, "the count and the walk must agree");
    }

    #[test]
    fn a_question_already_answered_is_not_asked_again() {
        let run = FirstRun::default();
        let mut security = crate::security::Security::default();
        assert!(
            !run.should_skip(Step::AppLock, &security),
            "with no lock set, the card has something to ask"
        );
        security.set_recording_passphrase("already chosen".into());
        assert!(
            run.should_skip(Step::Recording, &security),
            "a passphrase set from the command line must not be asked for again"
        );
    }

    #[test]
    fn appearance_and_autolock_are_always_shown() {
        // Neither can be "already answered": both have a default that is a
        // real choice, and both are worth stating once.
        let run = FirstRun::default();
        let security = crate::security::Security::default();
        assert!(!run.should_skip(Step::Appearance, &security));
        assert!(!run.should_skip(Step::Autolock, &security));
    }

    #[test]
    fn nothing_here_is_a_gate() {
        // Every card's source has a way past it. Read from the source rather
        // than by driving egui, which needs a context these tests do not build.
        let source = include_str!("firstrun.rs");
        for card in [
            "fn appearance",
            "fn app_lock",
            "fn recording",
            "fn autolock",
        ] {
            let at = source.find(card).expect("every card exists");
            let body = &source[at..at + 4000.min(source.len() - at)];
            assert!(
                body.contains("buttons(ui,"),
                "{card} has no way past it, which makes the setup a gate"
            );
        }
    }
}
