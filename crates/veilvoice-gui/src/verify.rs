// SPDX-License-Identifier: GPL-3.0-or-later
//! The verify tab: drop a download on the window and be told what it is.
//!
//! # Why this exists here rather than in the verifier
//!
//! The ask was drag-and-drop verification. There were two ways to have it:
//! link `eframe` into `veilvoice-verify`, or move the checking out of that
//! binary so both front ends call the same code.
//!
//! The portable verifier is the one program in this project whose *smallness*
//! is a feature: it is what somebody downloads before they trust anything
//! else here, and a 1.5 MB single file is part of why it is checkable at all.
//! Putting a GUI toolkit in it would have cost that for a convenience the
//! desktop application was already the right place for. So the arithmetic
//! moved to `veilvoice-check` and this is a second caller, not a second
//! implementation.
//!
//! # Three files, and the tab says so before it is given any
//!
//! Verifying needs the download, the `SHA256SUMS` and the `SHA256SUMS.asc`.
//! Dropping one file on a window and getting a verdict would be a lie, and an
//! interface that discovers the other two are missing *after* the drop teaches
//! people that verification is fiddly rather than that it needs three things.
//! All three slots are visible from the start, and a drop fills whichever one
//! the file's name says it is.
//!
//! # Marker 97: one press, three answers
//!
//! The tab used to answer one question, and it was not the question somebody
//! actually has. "Is this zip the published one" is a step; "is the program I
//! am about to run the published one" is the thing they want to know, and it
//! was being left to the command line.
//!
//! So one press now checks the archive against the signed hash list, then every
//! file extracted out of that archive against the signed contents list the
//! release publishes beside it, and then runs the GnuPG on this machine over
//! the same signature and shows what it said. Three answers, one button, and
//! each one drawn separately so a pass on one is never mistaken for a pass on
//! another.
//!
//! Two things are deliberately **not** failures. A release that published no
//! contents list -- everything before v0.1.15 -- simply has no such row, rather
//! than a warning about a file that was never meant to be there. And a GnuPG
//! that cannot run on this machine is drawn in the quiet colour: it is a fact
//! about the computer and says nothing whatever about the download.
//!
//! # Nothing here downloads anything
//!
//! Not even the key: it is compiled in, and its fingerprint is checked against
//! a constant a reader can compare with the README. `veilvoice-verify` is
//! still the tool for fetching a release, because it is the one that can be
//! checked before it is run.
//!
//! # In plain words
//!
//! Drop a download on the window and be told whether it is genuine.
//!
//! It checks the signature over the list of hashes first, and only then compares
//! your file against that list. That order matters: a list of hashes that has not
//! been checked is just some numbers somebody sent you.
//!
//! Drop the downloaded archive and the hash list and signature sitting beside it
//! are picked up on their own. Nothing is downloaded and nothing leaves the
//! machine.
//!
//! One press checks the zip, then every file you unzipped out of it, and then
//! asks your own GnuPG the same question and shows you its answer.

use crate::layout::column;
use crate::theme::palette as p;
use eframe::egui::{self, RichText, Ui};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use veilvoice_check::{Checked, Error};

/// Which of the three files a dropped path is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Download,
    Sums,
    Signature,
}

/// Work out what a dropped file is from its name.
///
/// By name rather than by content: the signature and the list are both text,
/// the download is anything, and asking the user "which of these is which"
/// after they have dropped three files is worse than getting it right for the
/// names this project actually publishes. A wrong guess is visible and one
/// click to correct, because every slot also has its own button.
fn slot_for(path: &Path) -> Slot {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name.ends_with(".asc") || name.ends_with(".sig") {
        Slot::Signature
    } else if name.contains("sha256sums") || name == "sha256sum.txt" {
        Slot::Sums
    } else {
        Slot::Download
    }
}

/// Everything one press of **check** found out.
///
/// **Marker 97.** The tab used to answer one question -- is this archive the
/// published one -- and it now answers three, because the other two are what
/// somebody actually wants to know and both were being left to the command
/// line. The extra work is done on the same worker thread and reported in the
/// same place, so there is still one button.
#[derive(Debug, Default)]
pub struct Report {
    /// The archive against the signed hash list. The question this tab has
    /// always answered.
    pub file: Option<Result<Checked, Error>>,
    /// Every file extracted out of that archive, against the signed contents
    /// list, where the release published one.
    pub contents: Option<Contents>,
    /// The same signature, through the GnuPG on this machine.
    pub gnupg: Option<Gnupg>,
}

/// What the extracted folder turned out to hold.
#[derive(Debug, Default)]
pub struct Contents {
    /// The folder that was looked at.
    pub folder: String,
    /// How many files the release published for this archive.
    pub total: usize,
    /// How many of them are on disk, unchanged.
    pub good: usize,
    /// Everything wrong, one line each, ready to draw.
    pub problems: Vec<String>,
    /// Why nothing could be checked, when that is the answer.
    pub unusable: Option<String>,
}

/// What this machine's GnuPG said.
#[derive(Debug, Default)]
pub struct Gnupg {
    /// Where GnuPG was found, or why it was not asked.
    pub found: String,
    /// What was done to the keyring and how to undo it.
    pub note: Vec<String>,
    /// GnuPG's answer in one line.
    pub verdict: String,
    /// Whether that answer agrees with this program's own.
    pub agrees: bool,
    /// Whether GnuPG gave an answer at all. A GnuPG that could not run says
    /// nothing about the download and must not be drawn as a failure.
    pub answered: bool,
    /// What GnuPG printed, for when the answer needs its evidence.
    pub said: Vec<String>,
}

/// The tab's state.
#[derive(Default)]
pub struct Verify {
    /// The file being checked.
    pub download: Option<PathBuf>,
    /// The signed list of hashes.
    pub sums: Option<PathBuf>,
    /// The detached signature over that list.
    pub signature: Option<PathBuf>,

    job: Option<mpsc::Receiver<Report>>,
    report: Option<Report>,
    /// Set while files are over the window, so the drop target can light up.
    hovering: bool,
    /// The file picker, while it is open.
    choosing: crate::dialog::Pending,
    /// Which slot the open picker was started for.
    ///
    /// Remembered because the answer arrives frames later, by which time the
    /// loop that drew the button is over. Without it a chosen file lands in
    /// whichever slot happened to be drawn when the dialog closed.
    choosing_for: Option<Slot>,
}

/// The width the slot labels are given, so the file names beside them start
/// level. Wide enough for `SHA256SUMS.asc`, which is the longest.
const SLOT_LABEL_WIDTH: f32 = 132.0;

/// The width the file name is given, so the three `choose…` buttons land at
/// one x whatever is in the slots. Wide enough for
/// `the signature over that list`, the longest of the three placeholders; a
/// name longer than this pushes its own button right, and a row where the
/// name is the interesting part is the right place to spend the space.
const SLOT_NAME_WIDTH: f32 = 226.0;

impl Verify {
    /// Whether a check is running, so the app keeps repainting.
    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    /// Whether the window has to keep drawing for this panel's sake.
    ///
    /// Busy *or* hovering. An idle egui window repaints only when something
    /// asks it to, and dragging a file over it is not by itself something that
    /// does -- so without this the drop target never lit up and the dropped
    /// file did not appear until the mouse moved for some other reason.
    pub fn wants_repaint(&self) -> bool {
        self.job.is_some() || self.hovering
    }

    /// Take the worker's answer if it has one. Never waits.
    pub fn drain(&mut self) {
        let Some(rx) = &self.job else { return };
        match rx.try_recv() {
            Ok(answer) => {
                self.report = Some(answer);
                self.job = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.report = Some(Report {
                    file: Some(Err(Error::Io(
                        "the check stopped without answering".to_string(),
                    ))),
                    ..Report::default()
                });
                self.job = None;
            }
        }
    }

    /// Put a dropped or chosen file into the slot its name says it belongs in.
    pub fn accept(&mut self, path: PathBuf) {
        let slot = slot_for(&path);
        match slot {
            Slot::Download => self.download = Some(path.clone()),
            Slot::Sums => self.sums = Some(path.clone()),
            Slot::Signature => self.signature = Some(path.clone()),
        }
        // A release is downloaded as three files into one folder, and the other
        // two are almost always sitting beside the one that was just dropped.
        // Asking somebody to find them by hand three times is asking them to
        // give up on verifying, which is the outcome this panel exists to
        // prevent.
        self.fill_from_beside(&path);

        // A new file makes the last answer stale, and a stale verdict beside a
        // different file is the worst thing this panel could show.
        self.report = None;
    }

    /// Fill the empty slots from the folder this file came from.
    ///
    /// **Only the empty ones.** A slot somebody chose themselves is never
    /// replaced: they said which file they meant, and a guess from a name
    /// overriding that is how the wrong thing gets verified and reported as
    /// right.
    ///
    /// Only exact names are taken -- `SHA256SUMS` and `SHA256SUMS.asc`, as the
    /// release publishes them. Anything looser starts matching files that
    /// happen to be nearby, and a hash list is not something to be clever
    /// about finding.
    fn fill_from_beside(&mut self, path: &std::path::Path) {
        let Some(folder) = path.parent() else {
            return;
        };
        if self.sums.is_none() {
            let candidate = folder.join("SHA256SUMS");
            if candidate.is_file() {
                self.sums = Some(candidate);
            }
        }
        if self.signature.is_none() {
            let candidate = folder.join("SHA256SUMS.asc");
            if candidate.is_file() {
                self.signature = Some(candidate);
            }
        }
    }

    /// Which slots were filled in by looking rather than by being chosen.
    ///
    /// Shown beside them, because a file that appeared without being asked for
    /// is a file somebody should be able to see and change.
    pub fn found_beside(&self, path: &std::path::Path) -> Vec<&'static str> {
        let Some(folder) = path.parent() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if self.sums.as_deref() == Some(folder.join("SHA256SUMS").as_path()) {
            out.push("SHA256SUMS");
        }
        if self.signature.as_deref() == Some(folder.join("SHA256SUMS.asc").as_path()) {
            out.push("SHA256SUMS.asc");
        }
        out
    }

    /// Read what the window was given this frame.
    ///
    /// Called from `update` before the tab is drawn. egui reports hovering and
    /// dropping through the same input state, so both are taken here and the
    /// tab merely renders the result.
    pub fn take_dropped(&mut self, ctx: &egui::Context) {
        let (hovering, dropped) = ctx.input(|i| {
            (
                !i.raw.hovered_files.is_empty(),
                i.raw
                    .dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect::<Vec<_>>(),
            )
        });
        self.hovering = hovering;
        for path in dropped {
            self.accept(path);
        }
    }

    /// The whole tab.
    pub fn tab(&mut self, ui: &mut Ui) {
        // The application scrolls every tab in one place; a second scroller
        // here would trap the wheel in whichever the pointer was over.
        self.body(ui);
    }

    fn body(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Verify a download").color(p::blue()));
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Drop the downloaded archive on this window and the hash list and \
                 signature beside it are picked up automatically. Nothing is \
                 downloaded and nothing leaves this machine.",
            )
            .color(p::muted()),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "One press checks the signature, the archive, every file you \
                 extracted out of it, and then all of it again through your own \
                 GnuPG if you have one.",
            )
            .color(p::muted())
            .small(),
        );

        // What was filled in without being asked for. Shown rather than left to
        // be noticed: a file that appeared on its own is a file somebody should
        // be able to see, and change if it is not the one they meant.
        if let Some(download) = self.download.clone() {
            let found = self.found_beside(&download);
            if !found.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "  found beside it: {}. Change either below if that is not \
                         what you meant",
                        found.join(" and ")
                    ))
                    .small()
                    .color(p::muted()),
                );
            }
        }

        ui.add_space(12.0);
        self.drop_target(ui);
        ui.add_space(12.0);

        self.slot_row(
            ui,
            Slot::Download,
            "the download",
            "the file you downloaded",
        );
        self.slot_row(ui, Slot::Sums, "SHA256SUMS", "the list of hashes");
        self.slot_row(
            ui,
            Slot::Signature,
            "SHA256SUMS.asc",
            "the signature over that list",
        );

        self.gnupg_section(ui);

        ui.add_space(12.0);
        let ready = self.download.is_some() && self.sums.is_some() && self.signature.is_some();
        ui.horizontal(|ui| {
            let busy = self.is_busy();
            if ui
                .add_enabled(ready && !busy, egui::Button::new("check"))
                .clicked()
            {
                self.start();
            }
            if busy {
                ui.spinner();
                ui.label(RichText::new("checking…").color(p::muted()).small());
            } else if !ready {
                ui.label(
                    RichText::new("all three files are needed")
                        .color(p::muted())
                        .small(),
                );
            }
        });

        ui.add_space(10.0);
        self.verdict(ui);

        ui.add_space(14.0);
        ui.label(
            RichText::new("What a pass proves")
                .color(p::yellow())
                .small(),
        );
        ui.label(RichText::new(veilvoice_check::SCOPE).color(p::muted()));
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("key fingerprint  {}", veilvoice_check::FINGERPRINT))
                .color(p::muted())
                .small(),
        );
        ui.label(
            RichText::new(
                "Compare that against the fingerprint in README.md and on the website. A \
                 program telling you its own key is correct has proved nothing.",
            )
            .color(p::muted())
            .small(),
        );
    }

    /// The rectangle that lights up while files are over the window.
    fn drop_target(&self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(520.0), 74.0),
            egui::Sense::hover(),
        );
        let (edge, ink, words) = if self.hovering {
            (p::blue(), p::blue(), "let go")
        } else {
            (p::border(), p::muted(), "drop files here")
        };
        ui.painter()
            .rect_stroke(rect, 9.0, egui::Stroke::new(1.5, edge));
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            words,
            egui::FontId::monospace(15.0),
            ink,
        );
    }

    /// One file slot: what it is, what is in it, and a way to change it.
    ///
    /// The two labels are given fixed-width columns so that the three rows
    /// line up and, with them, the three `choose…` buttons beside them.
    ///
    /// This used to pad the label with `{label:<16}` and hope. Trailing spaces
    /// line nothing up in a proportional font, which is the same habit the
    /// alignment work already took out of this application once: `the
    /// download`, `SHA256SUMS` and `SHA256SUMS.asc` are three different widths
    /// on screen whatever they are padded to, so the second column started in
    /// three different places and every button sat somewhere else again.
    fn slot_row(&mut self, ui: &mut Ui, slot: Slot, label: &str, what: &str) {
        ui.horizontal(|ui| {
            column(ui, SLOT_LABEL_WIDTH, |ui| {
                ui.label(RichText::new(label).color(p::muted()).small());
            });
            let current = match slot {
                Slot::Download => &self.download,
                Slot::Sums => &self.sums,
                Slot::Signature => &self.signature,
            };
            let text = match current {
                Some(path) => path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
                None => what.to_string(),
            };
            column(ui, SLOT_NAME_WIDTH, |ui| {
                ui.label(RichText::new(text).color(if current.is_some() {
                    p::fg()
                } else {
                    p::muted()
                }));
            });
            if ui
                .add_enabled(
                    !self.choosing.is_open(),
                    egui::Button::new("choose…").small(),
                )
                .clicked()
            {
                // Which slot was asked for is remembered here, because the
                // answer arrives some frames later and by then this loop is
                // over. Without it the file would land in whichever slot
                // happened to be drawn when the dialog closed.
                self.choosing_for = Some(slot);
                self.choosing.start(crate::dialog::Ask::open());
            }
        });

        // Straight into the slot that was asked for, not through `accept`: the
        // user said which one by pressing that button, and a guess from the
        // file name would override what they just told us.
        if let Some(path) = self.choosing.taken() {
            match self.choosing_for.take() {
                Some(Slot::Download) => self.download = Some(path),
                Some(Slot::Sums) => self.sums = Some(path),
                Some(Slot::Signature) => self.signature = Some(path),
                None => {}
            }
            self.report = None;
        }
    }

    /// Marker 90. The same check, with a GnuPG this project did not write.
    ///
    /// Here rather than on its own tab because this is where somebody is
    /// already asking "is this download genuine", and the honest answer to that
    /// question includes "and here is how to ask something other than me".
    ///
    /// The commands come from [`veilvoice_gnupg::commands`], which the
    /// portable verifier also uses, so the window and the command line cannot
    /// drift into printing two different recipes.
    fn gnupg_section(&mut self, ui: &mut Ui) {
        ui.add_space(16.0);
        ui.separator();
        ui.label(
            RichText::new("The same check, typed by you")
                .color(p::blue())
                .small(),
        );
        ui.label(
            RichText::new(
                "Pressing check above already runs your GnuPG and shows what it said. \
                 These are the same commands for you to run yourself, which is the part \
                 no program can do for you: the one telling you a download is genuine \
                 came out of that download.",
            )
            .color(p::muted())
            .small(),
        );

        let (Some(sums), Some(signature)) = (self.sums.clone(), self.signature.clone()) else {
            ui.label(
                RichText::new(
                    "choose a hash list and a signature above, and the commands appear here",
                )
                .color(p::muted())
                .small(),
            );
            return;
        };

        match veilvoice_gnupg::on_path() {
            Some(gpg) => ui.label(
                RichText::new(format!("GnuPG found at {}", gpg.display()))
                    .color(p::green())
                    .small(),
            ),
            None => ui.label(
                RichText::new(
                    "GnuPG is not on your PATH. These are the commands if you install it.",
                )
                .color(p::muted())
                .small(),
            ),
        };

        // The signing key beside the hash list, when the release shipped one.
        let key = sums.with_file_name("veilvoice-signing-key.asc");
        let key = key.is_file().then_some(key);
        let commands = veilvoice_gnupg::commands(&sums, &signature, key.as_deref());
        let script = commands.join("\n");

        ui.add_space(4.0);
        for line in &commands {
            ui.label(RichText::new(line).color(p::fg()).monospace());
        }
        ui.add_space(4.0);
        if ui.button("copy these commands").clicked() {
            ui.ctx().copy_text(script);
        }
        ui.label(
            RichText::new(
                "Worth doing. VeilVoice checked the signature with a key built into \
                 itself, and this program came out of the same download you are \
                 checking. GnuPG, and the fingerprint on the website, are the \
                 independent answer.",
            )
            .color(p::muted())
            .small(),
        );
    }

    /// The answer, in the colour it deserves.
    fn verdict(&self, ui: &mut Ui) {
        let Some(report) = &self.report else {
            ui.label(RichText::new("Not checked.").color(p::muted()).small());
            return;
        };
        self.archive_verdict(ui, report);
        self.contents_verdict(ui, report);
        self.gnupg_verdict(ui, report);
    }

    /// Marker 97. Every extracted file against the signed contents list.
    fn contents_verdict(&self, ui: &mut Ui, report: &Report) {
        let Some(contents) = &report.contents else {
            return;
        };
        ui.add_space(10.0);
        ui.label(
            RichText::new(format!("Everything in {}", contents.folder))
                .color(p::blue())
                .small(),
        );
        if let Some(why) = &contents.unusable {
            ui.label(RichText::new(why.clone()).color(p::red()));
            ui.label(
                RichText::new(
                    "Nothing in that folder was checked. Do not treat what is in it as \
                     verified.",
                )
                .color(p::muted())
                .small(),
            );
            return;
        }
        for line in &contents.problems {
            ui.label(RichText::new(line.clone()).color(p::red()).monospace());
        }
        if contents.problems.is_empty() {
            ui.label(
                RichText::new(format!(
                    "all {} files match the signed list, and there is nothing else in \
                     the folder",
                    contents.total
                ))
                .color(p::green()),
            );
        } else {
            ui.label(
                RichText::new(format!(
                    "{} of {} files are as published. Extract the checked archive again \
                     and use what comes out of it.",
                    contents.good, contents.total
                ))
                .color(p::red()),
            );
        }
    }

    /// Marker 97. What this machine's own GnuPG made of the same signature.
    fn gnupg_verdict(&self, ui: &mut Ui, report: &Report) {
        let Some(gnupg) = &report.gnupg else {
            return;
        };
        ui.add_space(10.0);
        ui.label(
            RichText::new("Checked again with your own GnuPG")
                .color(p::blue())
                .small(),
        );
        ui.label(RichText::new(gnupg.found.clone()).color(p::muted()).small());
        for line in &gnupg.note {
            ui.label(RichText::new(line.clone()).color(p::muted()).small());
        }
        if !gnupg.verdict.is_empty() {
            // A GnuPG that could not run is drawn in the quiet colour, never
            // the loud one. It is a fact about this machine and says nothing
            // whatever about the download.
            let colour = if !gnupg.answered {
                p::muted()
            } else if gnupg.agrees {
                p::green()
            } else {
                p::red()
            };
            ui.label(RichText::new(gnupg.verdict.clone()).color(colour));
        }
        for line in &gnupg.said {
            ui.label(
                RichText::new(line.clone())
                    .color(p::muted())
                    .small()
                    .monospace(),
            );
        }
    }

    /// The archive against the signed hash list.
    fn archive_verdict(&self, ui: &mut Ui, report: &Report) {
        match &report.file {
            None => {
                ui.label(RichText::new("Not checked.").color(p::muted()).small());
            }
            Some(Ok(checked)) if checked.matched => {
                ui.label(
                    RichText::new(format!("{} is the file this key published.", checked.name))
                        .color(p::green()),
                );
                ui.label(
                    RichText::new(format!("sha256  {}", checked.actual))
                        .color(p::muted())
                        .small(),
                );
            }
            Some(Ok(checked)) => {
                // The signature was good and the hash was not. Said in the
                // strongest words this panel has, because it is the one outcome
                // that means something is actually wrong with the file.
                ui.label(
                    RichText::new(format!(
                        "{} does NOT match the signed list. Do not run it.",
                        checked.name
                    ))
                    .color(p::red()),
                );
                ui.label(
                    RichText::new(format!("expected  {}", checked.expected))
                        .color(p::muted())
                        .small(),
                );
                ui.label(
                    RichText::new(format!("actual    {}", checked.actual))
                        .color(p::muted())
                        .small(),
                );
            }
            Some(Err(error)) => {
                ui.label(RichText::new(error.to_string()).color(p::red()));
                if matches!(error, Error::NotListed(_)) {
                    ui.label(
                        RichText::new(
                            "That usually means the SHA256SUMS is from a different release \
                             than the file.",
                        )
                        .color(p::muted())
                        .small(),
                    );
                }
            }
        }
    }

    /// Run the check on a thread of its own.
    ///
    /// Hashing a release archive is tens of megabytes of reading, and OpenPGP
    /// verification is not free either. `update()` may start work; it may never
    /// wait for it.
    fn start(&mut self) {
        let (Some(download), Some(sums), Some(signature)) = (
            self.download.clone(),
            self.sums.clone(),
            self.signature.clone(),
        ) else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.job = Some(rx);
        self.report = None;
        std::thread::spawn(move || {
            let _ = tx.send(examine(&download, &sums, &signature));
        });
    }
}

/// The whole check, off the drawing thread.
///
/// A free function rather than a method so it cannot reach the tab's state:
/// everything it needs is in its three arguments and everything it found is in
/// what it returns, which is the only shape that is safe to run on a thread
/// while the window carries on drawing.
fn examine(download: &Path, sums_path: &Path, signature_path: &Path) -> Report {
    let read = |path: &Path| {
        std::fs::read_to_string(path)
            .map_err(|e| Error::Io(format!("cannot read {}: {e}", path.display())))
    };
    let (sums, signature) = match (read(sums_path), read(signature_path)) {
        (Ok(sums), Ok(signature)) => (sums, signature),
        (Err(why), _) | (_, Err(why)) => {
            return Report {
                file: Some(Err(why)),
                ..Report::default()
            }
        }
    };

    let file = veilvoice_check::check_file(download, &sums, &signature);
    // The rest only when the archive itself is the published one. Reporting on
    // an extracted folder after the archive failed would be answering a
    // question nobody should still be asking.
    let go_on = matches!(&file, Ok(checked) if checked.matched);
    Report {
        contents: go_on
            .then(|| examine_contents(download, sums_path, &sums, &signature))
            .flatten(),
        gnupg: Some(examine_gnupg(sums_path, signature_path)),
        file: Some(file),
    }
}

/// Marker 97. Every file in the extracted folder, against the signed list.
///
/// The order is the one the whole project keeps: `CONTENTS.sha256` is checked
/// against the signed hash list **before** it is parsed, because it decides
/// which paths get read and what they are compared against.
fn examine_contents(
    download: &Path,
    sums_path: &Path,
    sums: &str,
    signature: &str,
) -> Option<Contents> {
    use veilvoice_check::contents;

    let list = sums_path.with_file_name(contents::CONTENTS);
    if !list.is_file() {
        // Every release before v0.1.15 is in this position, and it is not a
        // failure. Nothing is drawn rather than a row saying so, because a
        // panel that reports the absence of an optional file teaches people to
        // worry about it.
        return None;
    }
    let folder = download
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let unusable = |why: String| {
        Some(Contents {
            folder: folder.clone(),
            unusable: Some(why),
            ..Contents::default()
        })
    };

    match veilvoice_check::check_file(&list, sums, signature) {
        Err(why) => return unusable(format!("{why}")),
        Ok(checked) if !checked.matched => {
            return unusable(
                "CONTENTS.sha256 is not the list this release signed: the hashes do not \
                 agree."
                    .to_string(),
            )
        }
        Ok(_) => {}
    }
    let text = match std::fs::read_to_string(&list) {
        Ok(text) => text,
        Err(e) => return unusable(format!("cannot read {}: {e}", list.display())),
    };
    let all = match contents::parse(&text) {
        Ok(all) => all,
        Err(why) => return unusable(format!("{why}")),
    };
    let name = download.file_name()?.to_string_lossy().into_owned();
    let section = contents::for_archive(&all, &name)?;

    let root = download.parent().unwrap_or(Path::new("."));
    let outcomes = contents::check(root, section);
    let sweep = contents::extras(root, section);
    let good = outcomes.iter().filter(|o| o.is_good()).count();

    let mut problems = Vec::new();
    for outcome in &outcomes {
        match &outcome.verdict {
            contents::Verdict::Matches => {}
            contents::Verdict::Differs { .. } => {
                problems.push(format!("CHANGED  {}", outcome.path))
            }
            contents::Verdict::Missing => problems.push(format!("MISSING  {}", outcome.path)),
            contents::Verdict::Unreadable(why) => {
                problems.push(format!("UNREADABLE  {}: {why}", outcome.path))
            }
            // F-99. A link or a directory standing where a file should be is
            // not the published file, whatever it points at.
            contents::Verdict::NotAFile(what) => {
                problems.push(format!("{what} WHERE A FILE SHOULD BE  {}", outcome.path))
            }
        }
    }
    for extra in &sweep.extras {
        problems.push(format!(
            "NOT PART OF THE RELEASE  {}",
            extra.strip_prefix(root).unwrap_or(extra).display()
        ));
    }
    // F-98. Unknown is not empty. A folder this could not open must not be
    // drawn as one it looked in and found nothing.
    for shut in &sweep.unreadable {
        problems.push(format!(
            "COULD NOT LOOK INSIDE  {}",
            shut.strip_prefix(root).unwrap_or(shut).display()
        ));
    }
    Some(Contents {
        folder: section.roots().into_iter().next().unwrap_or(folder),
        total: outcomes.len(),
        good,
        problems,
        unusable: None,
    })
}

/// Marker 97. The same signature, through the GnuPG this machine already has.
fn examine_gnupg(sums: &Path, signature: &Path) -> Gnupg {
    let gpg = match veilvoice_gnupg::Gnupg::found() {
        Err(why) => {
            return Gnupg {
                found: format!("{why}. The commands below are what to run if you install it."),
                ..Gnupg::default()
            }
        }
        Ok(gpg) => gpg,
    };
    let mut report = Gnupg {
        found: format!("GnuPG found at {}", gpg.program().display()),
        ..Gnupg::default()
    };
    match gpg.import(veilvoice_check::PUBLIC_KEY, veilvoice_check::FINGERPRINT) {
        // Not drawn as a failure. GnuPG being unusable on this machine says
        // nothing about the download.
        Err(why) => {
            report.verdict = format!("GnuPG could not be asked: {why}");
            return report;
        }
        Ok(import) => report.note = import.note(),
    }
    match gpg.verify(signature, sums, veilvoice_check::FINGERPRINT) {
        Err(why) => report.verdict = format!("GnuPG could not be asked: {why}"),
        Ok(run) => {
            report.answered = true;
            report.agrees = run.outcome.is_good();
            report.verdict = run.outcome.plainly();
            if !report.agrees {
                report.said = run.said.lines().map(str::to_string).collect();
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_file_lands_in_the_slot_its_name_says() {
        assert_eq!(slot_for(Path::new("SHA256SUMS")), Slot::Sums);
        assert_eq!(slot_for(Path::new("sha256sums")), Slot::Sums);
        assert_eq!(slot_for(Path::new("SHA256SUMS.asc")), Slot::Signature);
        assert_eq!(slot_for(Path::new("release.sig")), Slot::Signature);
        assert_eq!(
            slot_for(Path::new("veilvoice-0.1.12-windows.zip")),
            Slot::Download
        );
        // `.asc` wins over the word "sha256sums", because the signature file is
        // named after the thing it signs and would otherwise land in the wrong
        // slot every single time.
        assert_eq!(slot_for(Path::new("SHA256SUMS.asc")), Slot::Signature);
    }

    /// **The three files arrive together.** A release is downloaded as an
    /// archive plus `SHA256SUMS` plus `SHA256SUMS.asc`, into one folder.
    /// Making somebody find each of them by hand is making them give up.
    #[test]
    fn dropping_the_archive_finds_the_hash_list_and_signature_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("veilvoice-v0.1.13-windows-x86_64.zip");
        std::fs::write(&archive, b"pretend").unwrap();
        std::fs::write(dir.path().join("SHA256SUMS"), b"list").unwrap();
        std::fs::write(dir.path().join("SHA256SUMS.asc"), b"sig").unwrap();

        let mut verify = Verify::default();
        verify.accept(archive.clone());

        assert_eq!(verify.download.as_deref(), Some(archive.as_path()));
        assert_eq!(
            verify.sums.as_deref(),
            Some(dir.path().join("SHA256SUMS").as_path())
        );
        assert_eq!(
            verify.signature.as_deref(),
            Some(dir.path().join("SHA256SUMS.asc").as_path())
        );
        assert_eq!(
            verify.found_beside(&archive),
            vec!["SHA256SUMS", "SHA256SUMS.asc"]
        );
    }

    /// A slot somebody chose is never replaced by a guess. They said which
    /// file they meant, and overriding that is how the wrong thing gets
    /// verified and reported as right.
    #[test]
    fn a_file_that_was_chosen_is_never_replaced_by_one_found_nearby() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let archive = dir.path().join("release.zip");
        std::fs::write(&archive, b"pretend").unwrap();
        std::fs::write(dir.path().join("SHA256SUMS"), b"the one beside it").unwrap();

        let chosen = elsewhere.path().join("SHA256SUMS");
        std::fs::write(&chosen, b"the one they picked").unwrap();

        let mut verify = Verify {
            sums: Some(chosen.clone()),
            ..Verify::default()
        };
        verify.accept(archive);

        assert_eq!(
            verify.sums.as_deref(),
            Some(chosen.as_path()),
            "the chosen file must survive"
        );
    }

    /// Only the exact published names. Anything looser starts matching files
    /// that merely happen to be nearby, and a hash list is not something to be
    /// clever about finding.
    #[test]
    fn only_the_exact_names_are_taken_from_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("release.zip");
        std::fs::write(&archive, b"pretend").unwrap();
        for decoy in ["SHA256SUMS.old", "sha256sums.txt", "SHA256SUMS.asc.bak"] {
            std::fs::write(dir.path().join(decoy), b"no").unwrap();
        }

        let mut verify = Verify::default();
        verify.accept(archive.clone());
        assert_eq!(verify.sums, None, "no decoy may be taken");
        assert_eq!(verify.signature, None);
        assert!(verify.found_beside(&archive).is_empty());
    }

    #[test]
    fn accepting_files_fills_the_three_slots() {
        let mut verify = Verify::default();
        verify.accept(PathBuf::from("veilvoice.zip"));
        verify.accept(PathBuf::from("SHA256SUMS"));
        verify.accept(PathBuf::from("SHA256SUMS.asc"));
        assert_eq!(verify.download, Some(PathBuf::from("veilvoice.zip")));
        assert_eq!(verify.sums, Some(PathBuf::from("SHA256SUMS")));
        assert_eq!(verify.signature, Some(PathBuf::from("SHA256SUMS.asc")));
    }

    /// A new file makes the last answer stale, and a stale verdict shown beside
    /// a different file is the worst thing this panel could do.
    #[test]
    fn a_new_file_clears_the_previous_answer() {
        let mut verify = Verify {
            report: Some(Report {
                file: Some(Err(Error::BadSignature)),
                ..Report::default()
            }),
            ..Verify::default()
        };
        verify.accept(PathBuf::from("veilvoice.zip"));
        assert!(verify.report.is_none());
    }

    /// The window has to keep drawing while a file is over it, not only while
    /// a check is running. This is the assertion behind that.
    #[test]
    fn hovering_keeps_the_window_awake() {
        let mut verify = Verify::default();
        assert!(!verify.wants_repaint(), "an idle panel asks for nothing");
        verify.hovering = true;
        assert!(verify.wants_repaint(), "a file over the window is a reason");
        assert!(!verify.is_busy(), "hovering is not a running check");
    }

    #[test]
    fn a_check_cannot_start_without_all_three() {
        let mut verify = Verify::default();
        verify.start();
        assert!(!verify.is_busy());
        verify.accept(PathBuf::from("veilvoice.zip"));
        verify.start();
        assert!(!verify.is_busy());
        verify.accept(PathBuf::from("SHA256SUMS"));
        verify.start();
        assert!(!verify.is_busy(), "still no signature");
    }

    #[test]
    fn a_worker_that_dies_without_answering_is_reported() {
        let (tx, rx) = mpsc::channel();
        drop(tx);
        let mut verify = Verify {
            job: Some(rx),
            ..Verify::default()
        };
        verify.drain();
        assert!(!verify.is_busy());
        match verify.report.as_ref().and_then(|r| r.file.as_ref()) {
            Some(Err(Error::Io(why))) => assert!(why.contains("without answering"), "{why}"),
            other => panic!("expected a reported failure, got {other:?}"),
        }
    }

    /// The GnuPG section is drawn once, not once for each file slot.
    ///
    /// It used to be the last statement of `slot_row`, and `slot_row` is
    /// called three times: for the download, the hash list and the signature.
    /// So the verify tab carried three copies of the heading "The same check,
    /// typed by you", three copies of the paragraph under it and three copies
    /// of "choose a hash list and a signature above", interleaved with the
    /// three file rows. It shipped in every screenshot of that tab.
    ///
    /// Nothing caught it because every piece of it was correct: the section
    /// draws what it should, and `slot_row` draws what it should. Only the
    /// place one is called from was wrong, and no test looked at that.
    ///
    /// This reads the call site rather than the drawing, because where it is
    /// called from is exactly what was wrong.
    #[test]
    fn the_gnupg_section_is_drawn_once() {
        // Only the code, not the tests: this file is read by this test, and
        // the name written in the assertion below is itself a match. The
        // first version counted two calls and one of them was its own.
        let source = include_str!("verify.rs").replace("\r\n", "\n");
        let source = source.split("\n#[cfg(test)]").next().unwrap();
        let calls = source.matches("self.gnupg_section(ui)").count();
        assert_eq!(
            calls, 1,
            "the GnuPG section is called {calls} times; it belongs once, in \
             `body`, after the three slots rather than inside each of them"
        );

        let row = source
            .split("fn slot_row(")
            .nth(1)
            .and_then(|rest| rest.split("\n    fn ").next())
            .expect("slot_row exists");
        assert!(
            !row.contains("gnupg_section"),
            "`slot_row` draws the GnuPG section, so it is drawn once per file \
             slot and the tab carries three copies of it"
        );
    }

    /// The slot labels are given a column rather than padded with spaces.
    ///
    /// `format!("{label:<16}")` lines nothing up in a proportional font, so
    /// the three file names started in three different places and the three
    /// `choose…` buttons beside them did too. The same habit was taken out of
    /// `security.rs` once already, which is why the fix now lives in
    /// `layout::column` and both call it.
    #[test]
    fn the_slot_rows_use_a_column_and_not_padding() {
        let source = include_str!("verify.rs").replace("\r\n", "\n");
        let source = source.split("\n#[cfg(test)]").next().unwrap();
        let row = source
            .split("fn slot_row(")
            .nth(1)
            .and_then(|rest| rest.split("\n    fn ").next())
            .expect("slot_row exists");
        assert!(
            !row.contains(":<"),
            "a slot label is padded to a width with spaces, which aligns \
             nothing in a proportional font"
        );
        assert_eq!(
            row.matches("column(ui,").count(),
            2,
            "both the label and the file name need a fixed column, or the \
             `choose…` buttons do not line up with each other"
        );
    }

    /// Marker 90. The window and the command line must print the same GnuPG
    /// recipe, which is why the body of it lives in `veilvoice-check` and both
    /// call it rather than each keeping a copy.
    #[test]
    fn the_window_prints_the_shared_gnupg_commands() {
        let source = include_str!("verify.rs").replace("\r\n", "\n");
        let start = source
            .find("fn gnupg_section(")
            .expect("the section exists");
        let end = source[start..]
            .find("\n    /// The answer, in the colour")
            .map(|at| start + at)
            .unwrap_or(source.len());
        let body = &source[start..end];
        assert!(
            body.contains("veilvoice_gnupg::commands"),
            "the window builds its own commands, which will drift from the \
             verifier's"
        );
        for forbidden in ["Command::new", "process::Command"] {
            assert!(
                !body.contains(forbidden),
                "the window runs {forbidden:?}: the independent check is \
                 independent because the person runs it"
            );
        }
    }

    /// The whole thing, end to end, against a file this test makes: the
    /// signature check has to fail before any hash is believed, even when the
    /// hash is perfect.
    #[test]
    fn a_perfect_hash_under_a_bad_signature_still_fails() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("release.zip");
        std::fs::write(&file, b"contents").unwrap();
        let sums = format!(
            "{}  release.zip\n",
            veilvoice_check::sha256_bytes(b"contents")
        );
        let error = veilvoice_check::check_file(&file, &sums, "not a signature")
            .expect_err("a bad signature must stop the check");
        assert!(matches!(error, Error::Malformed(_)), "{error:?}");
    }

    /// **Marker 97.** A release with no contents list produces no row about
    /// one. Everything published before v0.1.15 is in that position, and a
    /// panel that reported the absence of an optional file would teach people
    /// to worry about it.
    #[test]
    fn a_release_without_a_contents_list_draws_nothing_about_one() {
        let room = std::env::temp_dir().join(format!(
            "veilvoice-verify-tab-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&room).unwrap();
        let sums = room.join("SHA256SUMS");
        std::fs::write(&sums, b"nothing\n").unwrap();
        assert!(
            examine_contents(
                &room.join("veilvoice-v0.1.15-linux-x86_64.tar.gz"),
                &sums,
                "",
                ""
            )
            .is_none(),
            "no CONTENTS.sha256 beside it means no contents row"
        );
        std::fs::remove_dir_all(&room).ok();
    }

    /// **Marker 97.** A contents list that is not the signed one is refused,
    /// and nothing in the folder is reported on. Parsing it first and checking
    /// afterwards would be letting a downloaded text file choose which paths
    /// get read.
    #[test]
    fn a_contents_list_that_is_not_the_signed_one_checks_nothing() {
        let room = std::env::temp_dir().join(format!(
            "veilvoice-verify-tab-bad-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&room).unwrap();
        let sums = room.join("SHA256SUMS");
        std::fs::write(&sums, b"nothing\n").unwrap();
        std::fs::write(room.join("CONTENTS.sha256"), b"# a.tar.gz\n").unwrap();

        let report = examine_contents(
            &room.join("veilvoice-v0.1.15-linux-x86_64.tar.gz"),
            &sums,
            "nothing\n",
            "not a signature",
        )
        .expect("a list that is there is reported on");
        assert!(report.unusable.is_some(), "{report:?}");
        assert_eq!(report.total, 0, "nothing was checked");
        std::fs::remove_dir_all(&room).ok();
    }

    /// **Marker 97.** A GnuPG that cannot run is never drawn as a failure of
    /// the download. The distinction is the one this is most tempted to get
    /// wrong, and getting it wrong tells somebody not to run a sound release.
    #[test]
    fn a_gnupg_that_gave_no_answer_is_not_a_disagreement() {
        let quiet = Gnupg {
            found: "GnuPG is not on your PATH.".to_string(),
            ..Gnupg::default()
        };
        assert!(!quiet.answered, "nothing was asked");
        assert!(!quiet.agrees, "and nothing agreed");

        // The panel colours on `answered` first, so a GnuPG that said nothing
        // is muted rather than red. Asserted here because the drawing itself
        // needs a window and this is the decision inside it.
        let source = include_str!("verify.rs");
        let body = source
            .split("fn gnupg_verdict(")
            .nth(1)
            .expect("the GnuPG row has to be findable");
        let body = body.split("\n    /// ").next().unwrap();
        let muted = body.find("!gnupg.answered").expect("answered is checked");
        let red = body.find("p::red()").expect("red exists");
        assert!(muted < red, "the not-answered case must be decided first");
    }

    /// **Marker 97.** The extracted folder is only looked at once the archive
    /// itself has passed. Reporting on a folder after the archive failed would
    /// be answering a question nobody should still be asking.
    #[test]
    fn nothing_is_reported_about_a_folder_when_the_archive_failed() {
        let source = include_str!("verify.rs");
        let body = source
            .split("fn examine(")
            .nth(1)
            .expect("the worker has to be findable");
        let body = body.split("\nfn ").next().unwrap();
        assert!(
            body.contains("let go_on = matches!(&file, Ok(checked) if checked.matched)"),
            "the contents check must be gated on the archive passing"
        );
        assert!(
            body.contains("go_on\n            .then("),
            "and the gate must be the thing that decides whether it runs"
        );
    }
}
