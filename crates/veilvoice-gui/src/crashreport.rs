// SPDX-License-Identifier: GPL-3.0-or-later
//! Offering the report from the last crash, on the run after it.
//!
//! # What was wrong with where this used to be
//!
//! A report has been written to disk since v0.1.10, and the interface
//! mentioned it: one line on the About tab saying the previous run ended
//! unexpectedly and where the file is.
//!
//! About is the last tab somebody who has just had a crash will open. It is
//! where you go to read version numbers. The person this notice is for
//! restarted the application to get back to what they were doing, landed on
//! the tab they were using, and saw nothing. So the report existed, was
//! accurate, was written for a person to read, and was never read by one.
//!
//! It is now shown above whichever tab you land on, once, on the first run
//! after a crash. That is the panel the application already uses for anything
//! it needs somebody to see whatever they are looking at.
//!
//! # Why it is offered rather than sent
//!
//! Nothing is transmitted, and nothing here could transmit it: this project
//! contains no network client and CI fails the build if one enters the
//! dependency graph. A crash reporter that uploads is the ordinary shape of
//! this feature and it is the wrong shape for this program, because a report
//! from a tool people use to protect themselves is a report about a person who
//! was being careful.
//!
//! So the report is shown, in full, in the window. The whole text, not a
//! summary of it, because "would you like to send this" is only a real
//! question if you can read what "this" is. Then two buttons: copy it, and
//! open the issue tracker in a browser. What happens after that is the
//! person's decision and their clipboard.
//!
//! # And it says what is in it
//!
//! The report carries the version, the operating system and processor, the
//! panic message and the source location. It carries no file names, no
//! settings, no passphrase and nothing about the audio. That list is in the
//! panel rather than in this comment, because somebody deciding whether to
//! paste it into a public issue tracker needs it in front of them.

use crate::theme::palette as p;
use egui::{RichText, Ui};

/// Where a report goes, if the person wants to file one.
///
/// A new issue rather than the issue list: somebody arriving from a crash has
/// a report in their clipboard and wants a box to put it in, not a search.
pub const NEW_ISSUE: &str = "https://github.com/tilas01/veilvoice/issues/new";

/// The issue tracker itself, linked from About whether or not anything crashed.
pub const ISSUES: &str = "https://github.com/tilas01/veilvoice/issues";

/// How long the copy button says it copied, in seconds.
const COPIED_FOR: f64 = 2.5;

/// What the panel is showing, kept across frames.
#[derive(Default)]
pub struct Offer {
    /// The report, read once. `None` means there was none, or it was dismissed.
    report: Option<(String, String)>,
    /// Whether the file's whole contents are open.
    reading: bool,
    /// When the copy button was last pressed.
    copied: Option<f64>,
    /// Set once the file has been looked for, so a missing report is not
    /// re-read from disk on every frame of every launch.
    looked: bool,
}

impl Offer {
    /// Read the previous report, if there is one. Cheap after the first call.
    pub fn look(&mut self) {
        if self.looked {
            return;
        }
        self.looked = true;
        self.report =
            crate::crashlog::previous().map(|(path, text)| (path.display().to_string(), text));
    }

    /// Whether there is anything to show.
    pub fn waiting(&self) -> bool {
        self.report.is_some()
    }

    /// Draw the offer. Returns true when it has been dealt with and the panel
    /// should go away.
    ///
    /// Dismissing deletes the file. That is deliberate and it is the reason
    /// the whole text is on screen first: a notice that keeps coming back is a
    /// notice people learn to close without reading, and the report is of no
    /// use to anybody once its owner has decided not to file it.
    pub fn panel(&mut self, ui: &mut Ui) -> bool {
        let Some((path, text)) = self.report.clone() else {
            return false;
        };
        let now = ui.input(|i| i.time);
        let mut finished = false;

        ui.label(
            RichText::new("The previous run ended unexpectedly.")
                .color(p::yellow())
                .strong(),
        );
        ui.label(
            RichText::new(format!(
                "A report was written to {path}. It was written on this machine and \
                 has been sent nowhere: VeilVoice has no network code at all, and \
                 nothing here can send it. If you would like the fault fixed, the \
                 whole of it is below to copy into an issue."
            ))
            .color(p::muted())
            .size(12.0),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "It contains the version, this operating system and processor, and \
                 the error. It contains no file names, no settings, no passphrase \
                 and nothing about any audio.",
            )
            .color(p::fg())
            .size(12.0),
        );
        ui.add_space(8.0);

        let just_copied = self.copied.is_some_and(|at| now - at < COPIED_FOR);
        ui.horizontal_wrapped(|ui| {
            if ui.button("copy the report").clicked() {
                ui.ctx().copy_text(text.clone());
                self.copied = Some(now);
            }
            ui.hyperlink_to("open a new issue", NEW_ISSUE);
            let label = if self.reading {
                "hide what it says"
            } else {
                "read what it says"
            };
            if ui.button(label).clicked() {
                self.reading = !self.reading;
            }
            if ui.button("dismiss and delete it").clicked() {
                crate::crashlog::clear();
                self.report = None;
                finished = true;
            }
            if just_copied {
                ui.label(
                    RichText::new("copied, ready to paste")
                        .color(p::green())
                        .small(),
                );
            }
        });

        if self.reading {
            ui.add_space(6.0);
            // Scrolled and bounded: a panic message can carry a formatted
            // value of any size, and this sits above the tab somebody is
            // trying to use.
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(text).color(p::fg()).monospace().size(12.0));
                });
        }

        if just_copied {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
        finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The source of this file, up to its tests, so a test can check the code
    /// rather than matching its own string literals.
    fn source() -> &'static str {
        let whole = include_str!("crashreport.rs");
        whole.split("\n#[cfg(test)]").next().unwrap_or(whole)
    }

    /// The same, with comments removed.
    ///
    /// The scan below looks for the machinery of sending something. The
    /// comments at the top of this file explain at length why there is none,
    /// and to do that they have to use the words: the first run of that test
    /// failed on the sentence "a crash reporter that uploads". A check that
    /// forbids describing the thing it forbids is a check that punishes
    /// writing the explanation down.
    fn code() -> String {
        source()
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn nothing_is_offered_when_nothing_crashed() {
        let mut offer = Offer::default();
        assert!(!offer.waiting(), "a fresh offer has nothing to show");
        // `look` is what reads the disk; before it, `waiting` must still be
        // honest rather than optimistic.
        offer.looked = true;
        assert!(!offer.waiting());
    }

    #[test]
    fn the_disk_is_read_once_and_not_every_frame() {
        let mut offer = Offer::default();
        offer.look();
        assert!(offer.looked, "the first look must record that it looked");
        // A second look changes nothing, which is what stops a launch with no
        // report from stat-ing a file sixty times a second.
        let before = offer.report.is_some();
        offer.look();
        assert_eq!(offer.report.is_some(), before);
    }

    #[test]
    fn the_offer_names_the_project_and_not_a_third_party() {
        // A crash reporter that points somewhere other than this repository is
        // sending somebody's report to a stranger.
        assert!(NEW_ISSUE.starts_with("https://github.com/tilas01/veilvoice/"));
        assert!(ISSUES.starts_with("https://github.com/tilas01/veilvoice/"));
    }

    #[test]
    fn nothing_here_transmits_anything() {
        // The panel offers a clipboard and a browser link. If a request ever
        // appears in this file it is a change to what the project claims about
        // itself, and it should fail here first.
        let text = code();
        for forbidden in ["reqwest", "ureq", "TcpStream", "http://", "post(", "upload"] {
            assert!(
                !text.contains(forbidden),
                "this file must not learn to send anything: found {forbidden}"
            );
        }
    }

    #[test]
    fn the_panel_says_what_the_report_holds_before_offering_it() {
        // "Would you like to send this" is only a real question if the person
        // can see what "this" is, so the list of contents and the way to read
        // the whole file are both required.
        let text = source();
        assert!(text.contains("no passphrase"), "must say what is not in it");
        assert!(
            text.contains("read what it says"),
            "must offer the whole text"
        );
        assert!(
            text.contains("has been sent nowhere"),
            "must say it was not sent"
        );
    }
}
