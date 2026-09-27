// SPDX-License-Identifier: GPL-3.0-or-later
//! Starting again from the window: what would go, then going.
//!
//! # Two steps, always, and never one
//!
//! Look, then agree. [`veilvoice_crypto::reset::plan`] answers what is in the
//! folder and how much of it there is, that answer is drawn, and only then is
//! there a button that removes anything. There is no path through this module
//! that deletes on one press, because "reset" is a word people press before
//! they have finished reading it.
//!
//! Where the plan holds something nothing can put back -- recordings, colour
//! schemes somebody wrote -- the word `RESET` has to be typed, exactly as
//! `veilvoice reset` asks for it at a terminal and for the same reason: a button
//! is a thing a hand does and a word is a thing a person does.
//!
//! # Off the drawing thread
//!
//! Both halves go to a worker. Counting a vault means walking a directory and
//! removing one means unlinking every file in it, and a window that stops
//! answering in the middle of deleting somebody's recordings is a window they
//! will assume has broken. The same shape [`crate::integrity`] uses, and the
//! thread is detached for the same reason: there is nothing to join.
//!
//! # What it does not do
//!
//! It does not close VeilVoice by itself. The report is the part worth reading
//! and a window that vanishes as the last file goes has thrown it away, so
//! closing is a button underneath it. Until that is pressed the window is
//! holding settings that no longer exist on disk, which is harmless and is
//! exactly why the button says what it says.
//!
//! # In plain words
//!
//! Puts VeilVoice back to how it was when you installed it. It shows you
//! everything it is about to remove, with how big it is, before it removes any
//! of it, and it can leave your password and the key to your recordings alone.

use crate::theme::palette as p;
use egui::{RichText, Ui};
use std::sync::mpsc::{self, Receiver};
use veilvoice_crypto::reset::{self, Keep, Plan, Report};

/// The word somebody types where the plan cannot be undone.
///
/// The command line's own constant, so there is one word rather than two that
/// have to be kept the same.
pub const TYPED: &str = "RESET";

/// Where the reset is up to.
enum State {
    /// Nothing asked for yet.
    Idle,
    /// A worker is counting what is there.
    Looking,
    /// This is what would go.
    Planned(Plan),
    /// A worker is removing it.
    Working,
    /// It is done, and this is what happened.
    Done(Report),
    /// It could not be looked at or could not be done.
    Failed(String),
}

/// What a worker sends back.
enum Answer {
    /// The plan, or why there is none.
    Planned(Result<Plan, String>),
    /// What the removal managed.
    Carried(Report),
}

/// The reset, as the window holds it.
pub struct Reset {
    state: State,
    /// Whether to leave the app lock alone. Not persisted: it is a choice about
    /// one reset, and a remembered answer to "keep your keys?" is the wrong
    /// thing to remember.
    keep_keys: bool,
    /// What has been typed into the confirmation field.
    typed: String,
    pending: Option<Receiver<Answer>>,
}

impl Default for Reset {
    fn default() -> Self {
        Self {
            state: State::Idle,
            // The safer of the two, on purpose. Somebody who has not read the
            // difference between them keeps their recordings.
            keep_keys: true,
            typed: String::new(),
            pending: None,
        }
    }
}

impl Reset {
    /// Whether a worker is running, so the window keeps repainting.
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Collect a finished worker. True when something changed, which is the
    /// window's cue to repaint.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.pending else {
            return false;
        };
        match rx.try_recv() {
            Ok(Answer::Planned(Ok(plan))) => {
                self.state = State::Planned(plan);
                self.pending = None;
                true
            }
            Ok(Answer::Planned(Err(why))) => {
                self.state = State::Failed(why);
                self.pending = None;
                true
            }
            Ok(Answer::Carried(report)) => {
                self.state = State::Done(report);
                self.typed.clear();
                self.pending = None;
                true
            }
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.state = State::Failed("the reset stopped unexpectedly".to_string());
                self.pending = None;
                true
            }
        }
    }

    /// Ask a worker what is there.
    fn look(&mut self) {
        if self.pending.is_some() {
            return;
        }
        let keep = self.keep();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.state = State::Looking;
        // Detached, like every other worker here: it holds nothing the process
        // needs back, and the window must never join a thread walking a disk.
        std::thread::spawn(move || {
            let answer = reset::plan(keep).map_err(|_| {
                "this platform did not say where to keep configuration, so there \
                 is no folder to reset"
                    .to_string()
            });
            let _ = tx.send(Answer::Planned(answer));
        });
    }

    /// Ask a worker to carry out a plan it has been shown.
    fn carry_out(&mut self, plan: Plan) {
        if self.pending.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.state = State::Working;
        std::thread::spawn(move || {
            let _ = tx.send(Answer::Carried(reset::carry_out(&plan)));
        });
    }

    /// Which reset the checkbox is currently asking for.
    fn keep(&self) -> Keep {
        if self.keep_keys {
            Keep::Keys
        } else {
            Keep::Nothing
        }
    }

    /// Whether the plan on screen may be carried out yet.
    ///
    /// The whole gate, in one place so it can be read and tested: a plan with
    /// nothing irreplaceable in it needs the button and nothing else, and one
    /// with recordings in it needs the word as well.
    fn may_proceed(plan: &Plan, typed: &str) -> bool {
        !plan.irreplaceable() || typed.trim() == TYPED
    }

    /// Draw it.
    pub fn panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Start again").color(p::fg()).strong());
        ui.label(
            RichText::new(
                "Puts this machine back to how it was the day VeilVoice was \
                 installed: settings, policies, the mandate, the vaults and the \
                 app lock. Nothing happens until you have seen what would go.",
            )
            .small()
            .color(p::muted()),
        );
        ui.add_space(10.0);

        match &self.state {
            State::Idle => self.offer(ui),
            State::Looking => {
                ui.label(RichText::new("counting what is there...").color(p::muted()));
            }
            State::Working => {
                ui.label(RichText::new("removing it...").color(p::muted()));
            }
            State::Planned(_) => self.agree(ui),
            State::Done(_) => self.finished(ui),
            State::Failed(why) => {
                ui.label(RichText::new(why).color(p::yellow()));
                if ui.button("try again").clicked() {
                    self.state = State::Idle;
                }
            }
        }
    }

    /// The offer, before anything has been counted.
    fn offer(&mut self, ui: &mut Ui) {
        ui.checkbox(
            &mut self.keep_keys,
            "Keep my password, and the key to my recordings",
        );
        ui.label(
            RichText::new(
                "  On, the app lock stays exactly as it is. Off, it goes with \
                 everything else, and anything sealed with it can never be \
                 opened again: there is no other copy of that key anywhere.",
            )
            .small()
            .color(p::muted()),
        );
        ui.add_space(10.0);
        if ui.button("Show me what would go").clicked() {
            self.look();
        }
    }

    /// The plan, and the decision.
    fn agree(&mut self, ui: &mut Ui) {
        let State::Planned(plan) = &self.state else {
            return;
        };

        if plan.is_empty() {
            ui.label(
                RichText::new("There is nothing to remove: this is already a new install.")
                    .color(p::green()),
            );
            ui.add_space(8.0);
            if ui.button("done").clicked() {
                self.state = State::Idle;
            }
            return;
        }

        ui.label(
            RichText::new(plan.dir.display().to_string())
                .small()
                .color(p::cyan()),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("This would be removed")
                .color(p::yellow())
                .strong(),
        );
        ui.add_space(4.0);
        for going in &plan.going {
            ui.label(RichText::new(format!(
                "{}  ({}, {})",
                going.label,
                files(going.files),
                size(going.bytes)
            )));
            ui.label(
                RichText::new(format!("    {}", going.note))
                    .small()
                    .color(p::muted()),
            );
            if going.irreplaceable {
                ui.label(
                    RichText::new("    nothing can put this back")
                        .small()
                        .color(p::yellow()),
                );
            }
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "{} altogether, {}",
                files(plan.files()),
                size(plan.bytes())
            ))
            .color(p::fg()),
        );

        if !plan.keeping.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("This stays").color(p::cyan()).strong());
            for (label, path) in &plan.keeping {
                ui.label(
                    RichText::new(format!("    {label}: {}", path.display()))
                        .small()
                        .color(p::muted()),
                );
            }
        }

        if let Some(path) = &plan.beyond_reach {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "A second copy of the app lock is at {}, where an administrator \
                     wrote it. Removing it needs the same privilege, and left alone \
                     it restores the app lock by itself on the next launch. Remove \
                     that file as an administrator, or run `veilvoice reset` as one.",
                    path.display()
                ))
                .small()
                .color(p::yellow()),
            );
        }

        ui.add_space(12.0);
        if plan.irreplaceable() {
            ui.label(
                RichText::new(
                    "This cannot be undone. The files are deleted rather than \
                     overwritten: on an SSD, an SD card or a memory stick, \
                     overwriting cannot promise the old blocks are gone either, \
                     which is why this does not spend an hour pretending to.",
                )
                .small()
                .color(p::yellow()),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("type {TYPED}")).color(p::muted()));
                ui.add(egui::TextEdit::singleline(&mut self.typed).desired_width(140.0));
            });
        }

        let allowed = Self::may_proceed(plan, &self.typed);
        ui.add_space(8.0);
        let mut go = false;
        let mut stop = false;
        ui.horizontal(|ui| {
            go = ui
                .add_enabled(allowed, egui::Button::new("Remove all of it"))
                .clicked();
            stop = ui.button("Never mind").clicked();
        });
        if go {
            // Cloned out of the state, because carrying it out replaces the
            // state it is being read from. The plan the worker is given is the
            // plan that was on screen, which is the whole point of passing one
            // around rather than a directory.
            let plan = plan.clone();
            self.carry_out(plan);
        } else if stop {
            self.typed.clear();
            self.state = State::Idle;
        }
    }

    /// What happened, and the way out.
    fn finished(&mut self, ui: &mut Ui) {
        let State::Done(report) = &self.state else {
            return;
        };
        for label in &report.removed {
            ui.label(RichText::new(format!("removed {label}")).color(p::green()));
        }
        for line in &report.refused {
            ui.label(RichText::new(line).color(p::yellow()));
        }
        ui.add_space(8.0);
        if report.complete() {
            ui.label(
                RichText::new(
                    "Done. This window is still holding what it read at startup, \
                     so close it: the next launch starts as a first run.",
                )
                .small()
                .color(p::muted()),
            );
            ui.add_space(8.0);
            if ui.button("Close VeilVoice").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else {
            ui.label(
                RichText::new(
                    "Some of it is still there, and the lines above say which. \
                     Nothing else was left behind.",
                )
                .small()
                .color(p::yellow()),
            );
            if ui.button("start again").clicked() {
                self.state = State::Idle;
            }
        }
    }
}

/// "one file" or "nine files".
fn files(count: usize) -> String {
    if count == 1 {
        "one file".to_string()
    } else {
        format!("{count} files")
    }
}

/// A size somebody can judge at a glance.
///
/// Binary units, named as binary units, and the same arithmetic the command
/// line prints, because the two describe the same folder and a person may well
/// look at both.
fn size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let bytes = bytes as f64;
    if bytes == 1.0 {
        return "1 byte".to_string();
    }
    if bytes < KIB {
        return format!("{bytes:.0} bytes");
    }
    for (limit, unit) in [
        (KIB * KIB, "KiB"),
        (KIB * KIB * KIB, "MiB"),
        (KIB * KIB * KIB * KIB, "GiB"),
    ] {
        if bytes < limit {
            return format!("{:.1} {unit}", bytes / (limit / KIB));
        }
    }
    format!("{:.1} TiB", bytes / (KIB * KIB * KIB * KIB))
}

#[cfg(test)]
mod tests {
    use super::*;
    use veilvoice_crypto::layout::{entry, Item};

    fn folder_with(names: &[Item]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        for item in names {
            let entry = entry(*item);
            let path = dir.path().join(entry.name);
            if entry.folder {
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(path.join("something"), b"in it").unwrap();
            } else {
                std::fs::write(&path, b"a file").unwrap();
            }
        }
        dir
    }

    /// The keys are kept unless somebody says otherwise.
    ///
    /// Somebody who has not read the difference between the two resets should
    /// end up with the one that leaves their recordings openable. The other
    /// answer cannot be undone by anybody, including us.
    #[test]
    fn the_safer_of_the_two_is_the_one_already_chosen() {
        let reset = Reset::default();
        assert!(reset.keep_keys);
        assert!(reset.keep() == Keep::Keys);
    }

    /// A plan with recordings in it needs the word. A plan without does not.
    #[test]
    fn the_word_is_asked_for_exactly_where_something_cannot_come_back() {
        let settings = folder_with(&[Item::Settings]);
        let plan = reset::plan_in(settings.path(), Keep::Nothing);
        assert!(!plan.irreplaceable());
        assert!(
            Reset::may_proceed(&plan, ""),
            "a folder of settings asked for a typed word"
        );

        let vaults = folder_with(&[Item::Settings, Item::Vaults]);
        let plan = reset::plan_in(vaults.path(), Keep::Nothing);
        assert!(plan.irreplaceable());
        assert!(
            !Reset::may_proceed(&plan, ""),
            "recordings were about to go on one press"
        );
        for wrong in ["reset", "Reset", "RESE", "yes", " "] {
            assert!(
                !Reset::may_proceed(&plan, wrong),
                "{wrong:?} was accepted in place of the word"
            );
        }
        assert!(Reset::may_proceed(&plan, TYPED));
        assert!(
            Reset::may_proceed(&plan, "  RESET \n"),
            "the word typed with a stray space was refused, which reads as the \
             button being broken"
        );
    }

    /// The window and the command line ask for the same word.
    ///
    /// Two spellings of it would mean a person told `RESET` at a terminal and
    /// something else in the window, and one of the two would be wrong in
    /// whichever document described them together.
    #[test]
    fn both_front_ends_ask_for_the_same_word() {
        // The command line's own constant, read from its source because the two
        // crates cannot see each other.
        let cli = include_str!("../../veilvoice-cli/src/reset.rs");
        assert!(
            cli.contains(&format!("pub const TYPED: &str = \"{TYPED}\";")),
            "the command line asks for a different word than the window"
        );
    }

    /// Nothing is removed by drawing.
    ///
    /// The rule the module is built around, checked the only way it can be:
    /// draw every state that has a button in it, against a real folder, and
    /// find the folder afterwards.
    #[test]
    fn drawing_it_removes_nothing() {
        let dir = folder_with(&[Item::Settings, Item::Vaults, Item::Palettes]);
        let before = std::fs::read_dir(dir.path()).unwrap().count();
        let plan = reset::plan_in(dir.path(), Keep::Nothing);

        let ctx = egui::Context::default();
        let mut reset = Reset::default();
        for state in [
            State::Idle,
            State::Looking,
            State::Working,
            State::Planned(plan.clone()),
            State::Done(Report::default()),
            State::Failed("something".to_string()),
        ] {
            reset.state = state;
            let _ = crate::headless_frame(&ctx, Default::default(), |ui| reset.panel(ui));
        }
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            before,
            "drawing the panel changed the folder"
        );
    }

    #[test]
    fn a_size_is_readable_at_every_scale() {
        assert_eq!(size(0), "0 bytes");
        assert_eq!(size(1), "1 byte");
        assert_eq!(size(2048), "2.0 KiB");
        assert_eq!(size(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(files(1), "one file");
        assert_eq!(files(3), "3 files");
    }

    /// Polling with nothing running says so rather than guessing.
    #[test]
    fn polling_with_no_worker_reports_nothing() {
        let mut reset = Reset::default();
        assert!(!reset.poll());
        assert!(!reset.is_busy());
    }
}
