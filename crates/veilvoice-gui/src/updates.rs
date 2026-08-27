// SPDX-License-Identifier: GPL-3.0-or-later
//! The manual update check, as the window shows it.
//!
//! [`veilvoice_update`] does the asking and states what the answer is worth.
//! This is the button, the spinner and the result — and the rule that the
//! button is the only thing that ever starts it.
//!
//! # It runs on a thread, and the window never waits for it
//!
//! The check runs a subprocess and waits for a network round trip. On a captive
//! portal that is the full ten-second timeout. `update()` may read, paint and
//! *start* work; it may never wait for any — locked decision 15, and the reason
//! this application was reported as freezing every couple of seconds once
//! already. So the button spawns a thread, the thread sends one message down a
//! channel, and the window drains that channel once a frame and moves on.
//!
//! # Nothing here is automatic
//!
//! There is no timer, no check at startup, and no "check again" on a schedule.
//! [`Updates`] holds no clock. The only path into `veilvoice_update::check` is
//! a click, and a test asserts the state a freshly built panel is in.
//!
//! # In plain words
//!
//! The update check, as a button you press.
//!
//! VeilVoice never checks on its own and never contacts anything unless you ask.
//! An update check that runs by itself is a message to somebody else's server
//! saying that this copy exists and is running now.
//!
//! When you do press it, it compares your version against the newest published one
//! and tells you what it found, including that the answer only tells you what a
//! release page says.

use crate::theme::palette as p;
use eframe::egui::{self, RichText, Ui};
use std::sync::mpsc;
use veilvoice_update::{Error, Report, Verdict};

/// The panel's state.
#[derive(Default)]
pub struct Updates {
    /// The worker, while one is running.
    job: Option<mpsc::Receiver<Result<Report, Error>>>,
    /// The last answer, kept until another is asked for.
    answer: Option<Result<Report, Error>>,
}

impl Updates {
    /// Whether a check is running, so the app knows to keep repainting.
    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// Take the worker's answer if it has one. Called once a frame; never waits.
    ///
    /// `Disconnected` is handled as well as a message: a worker that died
    /// without sending would otherwise leave the panel saying "checking"
    /// forever, which is the failure mode a spinner is worst at showing.
    pub fn drain(&mut self) {
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(answer) => {
                self.answer = Some(answer);
                self.job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.answer = Some(Err(Error::Failed(
                    "the check stopped without answering".to_string(),
                )));
                self.job = None;
            }
        }
    }

    /// Start a check. The only path to the network in this application.
    fn start(&mut self, current: &str) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let current = current.to_string();
        // Detached on purpose. Nothing joins it: the window must not wait, and
        // a check whose answer arrives after the panel was closed is simply
        // dropped by the channel.
        std::thread::spawn(move || {
            let _ = tx.send(veilvoice_update::check(&current));
        });
        self.job = Some(rx);
        self.answer = None;
    }

    /// The whole section, as it appears under "about".
    pub fn section(&mut self, ui: &mut Ui, current: &str) {
        ui.label(RichText::new("Updates").color(p::blue()).small());
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            let busy = self.is_busy();
            if ui
                .add_enabled(!busy, egui::Button::new("check for updates"))
                .on_hover_text("Runs now, once, because you pressed it")
                .clicked()
            {
                self.start(current);
            }
            if busy {
                ui.spinner();
                ui.label(RichText::new("asking…").color(p::muted()).small());
            }
        });

        ui.add_space(8.0);
        match &self.answer {
            None => {
                ui.label(
                    RichText::new("Not checked. Nothing has been asked of any server.")
                        .color(p::muted())
                        .small(),
                );
            }
            Some(Ok(report)) => self.verdict(ui, report),
            Some(Err(error)) => {
                ui.label(RichText::new(error.to_string()).color(p::yellow()).small());
            }
        }

        ui.add_space(10.0);
        ui.label(
            RichText::new(veilvoice_update::SCOPE)
                .color(p::muted())
                .small(),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(veilvoice_update::RELEASES_URL)
                .color(p::muted())
                .small(),
        );
    }

    /// The answer itself, in the colour it deserves.
    fn verdict(&self, ui: &mut Ui, report: &Report) {
        let (text, colour) = match &report.verdict {
            Verdict::UpToDate => (
                format!("{} is the newest published release.", report.current),
                p::green(),
            ),
            Verdict::Newer(latest) => (
                format!(
                    "{latest} has been published. You are running {}.",
                    report.current
                ),
                p::yellow(),
            ),
            // Told plainly rather than called "up to date". Somebody running an
            // unreleased build should know that is what they are running.
            Verdict::Ahead(latest) => (
                format!(
                    "You are running {}, which is ahead of the newest release ({latest}).",
                    report.current
                ),
                p::cyan(),
            ),
            Verdict::Unreadable(latest) => (
                format!(
                    "The newest release is named {latest:?}, which this build cannot \
                     compare with {}. Look at the releases page.",
                    report.current
                ),
                p::yellow(),
            ),
        };
        ui.label(RichText::new(text).color(colour));
        ui.add_space(4.0);
        ui.label(RichText::new(report.caveat()).color(p::muted()).small());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A freshly built panel has asked nothing and is asking nothing. This is
    /// the test that fails if a check at startup is ever added.
    #[test]
    fn a_new_panel_has_asked_nothing() {
        let updates = Updates::default();
        assert!(!updates.is_busy(), "nothing may run before a click");
        assert!(
            updates.answer.is_none(),
            "nothing may be shown before a click"
        );
    }

    /// Draining with no worker is a no-op, and drains nothing into the answer.
    #[test]
    fn draining_without_a_check_running_does_nothing() {
        let mut updates = Updates::default();
        updates.drain();
        assert!(updates.answer.is_none());
        assert!(!updates.is_busy());
    }

    /// A worker that dies without sending must not leave the panel saying
    /// "checking" forever.
    #[test]
    fn a_worker_that_dies_without_answering_is_reported_rather_than_awaited() {
        let (tx, rx) = mpsc::channel();
        drop(tx);
        let mut updates = Updates {
            job: Some(rx),
            answer: None,
        };
        updates.drain();
        assert!(!updates.is_busy(), "the panel must stop waiting");
        match updates.answer {
            Some(Err(Error::Failed(ref why))) => {
                assert!(why.contains("without answering"), "{why}")
            }
            other => panic!("expected a reported failure, got {other:?}"),
        }
    }

    /// An answer is kept until another is asked for, so the result does not
    /// vanish on the next frame.
    #[test]
    fn an_answer_survives_being_drained_once() {
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(veilvoice_update::report("0.1.12", "0.2.0")))
            .unwrap();
        let mut updates = Updates {
            job: Some(rx),
            answer: None,
        };
        updates.drain();
        updates.drain();
        match updates.answer {
            Some(Ok(ref report)) => {
                assert_eq!(report.verdict, Verdict::Newer("0.2.0".into()))
            }
            other => panic!("expected the report to be kept, got {other:?}"),
        }
    }
}
