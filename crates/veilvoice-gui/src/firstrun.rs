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
//! The walkthrough in [`crate::tour`] runs after it, and carries the rest of
//! the setup: encryption at rest, whether this copy is installed, and what
//! each tab is for. It does not offer the app lock again, because this did,
//! thirty seconds earlier, on a card of its own.
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
    /// What this machine actually reports, and the one decision that follows
    /// from it.
    ///
    /// **Roadmap item 135.** Last, because it is the card that reports rather than
    /// asks: somebody who skipped everything else has still been shown where
    /// their recordings will go and how much room there is for them, which are
    /// the two facts a setup screen usually asserts and never measures.
    Machine,
}

impl Step {
    /// The step after this one, or `None` at the end of the tour.
    fn next(self) -> Option<Self> {
        match self {
            Self::Appearance => Some(Self::AppLock),
            Self::AppLock => Some(Self::Recording),
            Self::Recording => Some(Self::Autolock),
            Self::Autolock => Some(Self::Machine),
            Self::Machine => None,
        }
    }

    /// One-based position, for "step 2 of 4".
    fn position(self) -> usize {
        match self {
            Self::Appearance => 1,
            Self::AppLock => 2,
            Self::Recording => 3,
            Self::Autolock => 4,
            Self::Machine => 5,
        }
    }

    /// How many cards the tour has.
    ///
    /// Written down rather than counted from the enum, because the enum has no
    /// way to be counted without a crate or a macro, and because the number is
    /// used for the "card 3 of 5" line a reader sees. The test below asserts
    /// that walking from the first card to the last takes exactly this many
    /// steps, so a card added without changing this fails.
    const COUNT: usize = 5;
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
    /// What the last card reads from this machine, read once and elsewhere.
    ///
    /// Not a set of fields here, because the point is that none of them exists
    /// until a worker has been and got them. See [`Reading`] and F-216.
    reading: crate::offthread::Answer<Reading>,
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
        devices: (usize, usize),
        motion: crate::prefs::Motion,
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
            Step::Machine => self.machine(ui, prefs, devices, motion),
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

    /// The appearance step: pick a palette and see it applied immediately.
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

    /// The app-lock step: set a passphrase for VeilVoice itself, or decline
    /// it.
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

    /// The recording step: the at-rest passphrase, and what it is separate
    /// from.
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

    /// The machine card: what this computer says, read once rather than per frame.
    ///
    /// # Why the reading is not done here
    ///
    /// Every answer on this card is a question with a cost.
    /// `veilvoice_setup::space::free_bytes` spawns `df` on Unix and
    /// `fsutil.exe` on Windows; the graphics probe asks the driver. Written
    /// inline, which is how this card was written, each of them ran **once per
    /// frame** for as long as the card was on screen: a process forked sixty
    /// times a second to print a number that changes about as often as
    /// somebody empties a bin. See F-216.
    ///
    /// So the reading happens on a worker and the card draws whatever has come
    /// back. The card still measures rather than asserting, which is roadmap
    /// item 135's whole point; it measures once.
    ///
    /// # And the device counts come from the caller
    ///
    /// There is one enumeration of the audio hardware in this program, in
    /// `app`, at startup. **F-165** is what a second one cost: two enumerators
    /// running beside each other killed the process on Windows. This card used
    /// to make its own, per frame, which is the same defect with a worse
    /// multiplier. It now takes the count from the place that already has it,
    /// so there is one answer rather than two that could disagree.
    fn machine(
        &mut self,
        ui: &mut Ui,
        prefs: &mut crate::settings::Settings,
        devices: (usize, usize),
        motion: crate::prefs::Motion,
    ) -> bool {
        self.reading.ask_once(ui.ctx(), read_machine);
        let reading = self.reading.get();
        card(ui, "What this machine says", |ui| {
            ui.label(
                RichText::new(
                    "Read from this computer just now, rather than assumed. \
                     Nothing here is sent anywhere, and nothing on this card has \
                     to be answered.",
                )
                .color(p::muted()),
            );
            ui.add_space(12.0);

            // Where the recordings will go, and how much room is there for
            // them. The two belong together: a folder nobody can find and a
            // disk with nothing left on it are the same problem to somebody
            // whose recording did not save.
            ui.label(RichText::new("Where recordings will go").color(p::blue()));
            match reading {
                None => still_reading(ui, motion),
                Some(Reading { dir: None, .. }) => {
                    ui.label(
                        RichText::new(
                            "This system does not say where an application should \
                             keep its files, so nothing will be kept between runs \
                             and the vault cannot be opened. The About tab says \
                             the same thing in more detail.",
                        )
                        .color(p::red()),
                    );
                }
                Some(Reading {
                    dir: Some(dir),
                    free,
                    ..
                }) => {
                    ui.label(RichText::new(dir.display().to_string()).color(p::cyan()));
                    ui.label(
                        RichText::new(match free {
                            Some(free) => format!(
                                "{} free there, which is room for about {} of an \
                                 hour's veiled audio.",
                                crate::studio::size(usize::try_from(*free).unwrap_or(usize::MAX)),
                                // An hour of 48 kHz mono 16-bit audio, which is
                                // what the recorder writes. Worked out from the
                                // free space rather than stated, so it is this
                                // machine's answer. Unsigned, so it cannot go
                                // below zero and is not clamped as though it
                                // could.
                                free / (48_000 * 2 * 3_600)
                            ),
                            None => "This system would not say how much room is \
                                     free there, so check before a long recording."
                                .to_string(),
                        })
                        .small()
                        .color(p::muted()),
                    );
                }
            }

            ui.add_space(12.0);
            ui.label(RichText::new("Sound devices").color(p::blue()));
            let (inputs, outputs) = devices;
            ui.label(
                RichText::new(match (inputs, outputs) {
                    (0, 0) => "None found. Anonymising a file still works; the \
                               live and Studio tabs need a microphone."
                        .to_string(),
                    (0, _) => "No microphone found. Anonymising a file works; \
                               recording does not."
                        .to_string(),
                    (i, o) => format!(
                        "{i} to record from, {o} to play to. The Settings tab \
                         picks which."
                    ),
                })
                .small()
                .color(p::muted()),
            );

            ui.add_space(12.0);
            ui.label(RichText::new("Drawing the window").color(p::blue()));
            // Outside the reading, and deliberately: this is a preference
            // rather than a measurement, so it is there to be changed on the
            // first frame whether or not the driver has answered yet.
            let mut accelerated = prefs.acceleration();
            if ui
                .checkbox(
                    &mut accelerated,
                    "Ask the graphics driver to draw the window",
                )
                .changed()
            {
                prefs.set_acceleration(accelerated);
            }
            // Roadmap item 170. The probe's own sentence, which names what was
            // found on this machine, rather than a paragraph written once for
            // every machine. This card is called "What this machine says" and
            // this was the one section on it that did not say anything about
            // the machine.
            match reading {
                None => still_reading(ui, motion),
                Some(machine) => {
                    ui.label(
                        RichText::new(machine.acceleration_reason.as_str())
                            .small()
                            .color(p::muted()),
                    );
                    // And a plain statement of whether the sentence above is a
                    // finding or a fallback. A reader deciding whether to touch
                    // the tick needs to know which, and an interface that reads
                    // the same either way is claiming a measurement it may not
                    // have taken.
                    if !machine.graphics_answered {
                        ui.label(
                            RichText::new(
                                "This is the setting every machine started on before it \
                                 could be asked, so nothing is lost by it. The About tab \
                                 shows what the driver actually gave.",
                            )
                            .small()
                            .color(p::muted()),
                        );
                    }
                }
            }
        });
        buttons(ui, "finish", None).0
    }

    /// The auto-lock step: how long idle before the window locks itself.
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

/// What the last card of the tour reads from this computer.
///
/// One struct rather than four calls, because the point is that they happen
/// together, once, somewhere that is not the thread drawing the window. Each
/// field is an answer that has already been got; a card holding these is a
/// card that cannot ask again by accident. See [`crate::offthread`] and F-216.
///
/// **Roadmap item 135's whole point** is that this card reads the machine
/// rather than carrying numbers written here, because a constant would be a
/// claim about somebody else's hardware stated with the confidence of a
/// measurement. That is unchanged: these are measurements. They are taken
/// once.
struct Reading {
    /// Where recordings will go, if this system says where an application
    /// should keep its files.
    dir: Option<std::path::PathBuf>,
    /// Free bytes there, if the system would say. `None` is reported as not
    /// known rather than as zero, which would read as a full disk.
    free: Option<u64>,
    /// The graphics probe's own sentence about this machine.
    acceleration_reason: String,
    /// Whether that sentence is a finding or the fallback everything starts on.
    graphics_answered: bool,
}

/// Ask this computer everything the last card shows, on a worker thread.
///
/// The device counts are **not** here. There is one enumeration of the audio
/// hardware in this program, in `app`, and F-165 is what a second one cost.
/// The card takes that count from its caller instead.
fn read_machine() -> Reading {
    let dir = veilvoice_crypto::lock::default_dir();
    let free = dir.as_deref().and_then(veilvoice_setup::space::free_bytes);
    // `look` keeps its answer in a `OnceLock`, so asking here both fills this
    // card and means the first frame of the About tab finds the answer already
    // made rather than waiting for the driver itself.
    let probe = crate::probe::look();
    Reading {
        dir,
        free,
        acceleration_reason: probe.acceleration_reason.clone(),
        graphics_answered: probe.graphics_answered,
    }
}

/// The line a card shows where an answer will go, while it is being got.
///
/// Said rather than left blank. A section that is empty for a moment and then
/// is not reads as the window having glitched, and the indicator is the
/// difference between "being read" and "this machine has none".
fn still_reading(ui: &mut Ui, motion: crate::prefs::Motion) {
    crate::progress::strip(
        ui,
        "reading this machine",
        &crate::progress::Reach::unmeasurable(
            "the answers come from three different places and none of them says \
             how long it will be",
        ),
        motion,
    );
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
                Step::Autolock,
                Step::Machine
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

    /// **Roadmap item 135's whole point.** The card has to read the machine
    /// rather than carry numbers written here. A constant would be a claim
    /// about somebody else's hardware, stated with the confidence of a
    /// measurement.
    ///
    /// The reading moved off the drawing thread under F-216, so the
    /// measurement is looked for in [`super::read_machine`] rather than in the
    /// card. That is the same requirement about a different function, and the
    /// test below is what stops it moving back.
    #[test]
    fn the_machine_card_measures_rather_than_asserts() {
        let source = include_str!("firstrun.rs");
        let at = source.find("fn read_machine").expect("the reading exists");
        let rest = &source[at..];
        let reading = rest.split("\n/// ").next().unwrap_or(rest);

        for reads in [
            "veilvoice_setup::space::free_bytes",
            "veilvoice_crypto::lock::default_dir",
            "crate::probe::look()",
        ] {
            assert!(
                reading.contains(reads),
                "the reading no longer asks this machine for {reads}, so the \
                 card is asserting something about somebody else's hardware \
                 instead of measuring this one"
            );
        }

        let at = source.find("fn machine(").expect("the card exists");
        let rest = &source[at..];
        let card = rest.split("\n    fn ").next().unwrap_or(rest);
        // The card shows what was read, and offers the one thing on it that is
        // a preference rather than a measurement.
        for shows in ["self.reading", "prefs.acceleration()"] {
            assert!(card.contains(shows), "the card no longer draws {shows}");
        }
        // And it says so when the machine will not answer, rather than
        // printing a number it did not get.
        assert!(
            card.contains("would not say"),
            "the card has no answer for a system that will not say how much \
             room is free, so it would show one that was never measured"
        );
    }

    /// **F-216.** The card asks this machine once, not once per frame.
    ///
    /// `free_bytes` spawns `df` on Unix and `fsutil.exe` on Windows, and the
    /// graphics probe queries the driver. Written inline in the card, which is
    /// where they were, each of those ran on every frame the card was on
    /// screen: a process forked sixty times a second, on the thread that draws.
    ///
    /// Read from the source, because a frame count is not observable from here
    /// and the defect is a shape rather than a timing: a machine read written
    /// inside the drawing function is the defect, wherever it ends up costing.
    #[test]
    fn the_card_does_not_read_the_machine_while_drawing() {
        let source = include_str!("firstrun.rs");
        let at = source.find("fn machine(").expect("the card exists");
        let rest = &source[at..];
        let card = rest.split("\n    fn ").next().unwrap_or(rest);
        let card: String = card
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        for waits in [
            "space::free_bytes",
            "devices::list",
            "crate::probe::look()",
            "default_dir()",
        ] {
            assert!(
                !card.contains(waits),
                "the card calls {waits} while drawing, so it runs once per \
                 frame on the thread that paints. Read it in `read_machine` \
                 and hand it over, as F-216 did."
            );
        }
        assert!(
            card.contains("ask_once"),
            "the reading is no longer started once, so whatever starts it \
             starts it again on every frame"
        );
    }

    /// The counts come from the one enumeration this program makes.
    ///
    /// **F-165.** Two enumerators of the audio hardware running beside each
    /// other killed the process on Windows, so there is one, in `app`, at
    /// startup. This card used to make its own, inside the frame.
    #[test]
    fn the_device_counts_are_not_enumerated_a_second_time() {
        let source = include_str!("firstrun.rs");
        let shipped = source.split("#[cfg(test)]").next().unwrap_or(source);
        assert!(
            !shipped.contains("devices::list"),
            "the tour enumerates the audio hardware itself. There is one \
             enumeration in this program, in `app`; the count is handed to \
             `panel` rather than made again here. See F-165."
        );
        assert!(
            shipped.contains("devices: (usize, usize)"),
            "the panel no longer takes the counts, so whatever draws them is \
             getting them from somewhere else"
        );
    }

    #[test]
    fn nothing_here_is_a_gate() {
        // Every card's source has a way past it. Read from the source rather
        // than by driving egui, which needs a context these tests do not build.
        //
        // The window used to be a fixed four thousand characters after the
        // function's name, which is a length rather than a body: a card longer
        // than that reported no way past it, and a card shorter than that was
        // checked against the one after it as well. It now ends where the
        // function does.
        let source = include_str!("firstrun.rs");
        for card in [
            "fn appearance",
            "fn app_lock",
            "fn recording",
            "fn autolock",
            "fn machine",
        ] {
            let at = source.find(card).expect("every card exists");
            let rest = &source[at..];
            let body = rest.split("\n    fn ").next().unwrap_or(rest);
            assert!(
                body.contains("buttons(ui,"),
                "{card} has no way past it, which makes the setup a gate"
            );
        }
    }
}
