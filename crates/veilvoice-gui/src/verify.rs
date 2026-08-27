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
//! is a feature — it is what somebody downloads before they trust anything
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
//! # Nothing here downloads anything
//!
//! Not even the key: it is compiled in, and its fingerprint is checked against
//! a constant a reader can compare with the README. `veilvoice-verify` is
//! still the tool for fetching a release, because it is the one that can be
//! checked before it is run.

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

/// The tab's state.
#[derive(Default)]
pub struct Verify {
    /// The file being checked.
    pub download: Option<PathBuf>,
    /// The signed list of hashes.
    pub sums: Option<PathBuf>,
    /// The detached signature over that list.
    pub signature: Option<PathBuf>,

    job: Option<mpsc::Receiver<Result<Checked, Error>>>,
    report: Option<Result<Checked, Error>>,
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
                self.report = Some(Err(Error::Io(
                    "the check stopped without answering".to_string(),
                )));
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
    fn slot_row(&mut self, ui: &mut Ui, slot: Slot, label: &str, what: &str) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{label:<16}"))
                    .color(p::muted())
                    .small(),
            );
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
            ui.label(RichText::new(text).color(if current.is_some() {
                p::fg()
            } else {
                p::muted()
            }));
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

    /// The answer, in the colour it deserves.
    fn verdict(&self, ui: &mut Ui) {
        match &self.report {
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
            let answer = (|| {
                let sums = std::fs::read_to_string(&sums)
                    .map_err(|e| Error::Io(format!("cannot read {}: {e}", sums.display())))?;
                let signature = std::fs::read_to_string(&signature)
                    .map_err(|e| Error::Io(format!("cannot read {}: {e}", signature.display())))?;
                veilvoice_check::check_file(&download, &sums, &signature)
            })();
            let _ = tx.send(answer);
        });
    }
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
            report: Some(Err(Error::BadSignature)),
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
        match verify.report {
            Some(Err(Error::Io(ref why))) => assert!(why.contains("without answering"), "{why}"),
            other => panic!("expected a reported failure, got {other:?}"),
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
}
