// SPDX-License-Identifier: GPL-3.0-or-later
//! Asking for a file without stopping the window.
//!
//! # The defect this exists to fix
//!
//! Every file picker in this application was opened with `rfd`'s **blocking**
//! API, straight from the frame that handled the click:
//!
//! ```ignore
//! if ui.button("choose file…").clicked() {
//!     if let Some(path) = rfd::FileDialog::new().pick_file() { … }
//! }
//! ```
//!
//! `pick_file` does not return until the person has chosen a file or cancelled.
//! It is called from inside `update`, which is the render loop, so for as long
//! as that dialog is open **VeilVoice draws nothing at all**: the window does
//! not repaint, animations stop, the meters freeze, and dragging it leaves a
//! trail of stale pixels. Somebody browsing for a recording for thirty seconds
//! has a frozen application for thirty seconds.
//!
//! It is also the answer to "it lags when I select things", which is a real
//! report and an accurate one. There were seven of these.
//!
//! # How this is avoided, and the one platform where it cannot be
//!
//! The dialog runs on a thread of its own and the answer comes back down a
//! channel, which [`Pending::poll`] reads without waiting. `update` starts the
//! ask and returns immediately; the window keeps painting the whole time.
//!
//! **macOS is the exception, and it is not a shortcut.** `NSOpenPanel` must be
//! driven from the main thread; opening one anywhere else does not work, and
//! on some versions it does not fail politely either. So on macOS the ask is
//! made inline, exactly as before. That platform keeps the old behaviour
//! because the alternative is a picker that does not appear, and a frozen
//! window is better than no dialog.
//!
//! # In plain words
//!
//! When you click "choose a file", the box that opens used to freeze the rest
//! of VeilVoice until you picked something. Now it opens beside the
//! application and everything carries on running while you browse.
//!
//! On Apple computers it still waits, because macOS insists that file pickers
//! are opened by the main part of a program and there is no way around that
//! which actually works.

use std::path::PathBuf;
use std::sync::mpsc;

/// What is being asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ask {
    /// An existing file to open.
    Open {
        /// A named filter, as `(label, extensions)`. Empty for any file.
        filter: Option<(String, Vec<String>)>,
    },
    /// A path to write to.
    Save {
        /// The name to offer.
        suggested: String,
        /// A named filter, as `(label, extensions)`.
        filter: Option<(String, Vec<String>)>,
    },
    /// A directory.
    Folder,
}

impl Ask {
    /// An open dialog with no filter.
    pub fn open() -> Self {
        Self::Open { filter: None }
    }

    /// An open dialog restricted to these extensions.
    pub fn open_filtered(label: &str, extensions: &[&str]) -> Self {
        Self::Open {
            filter: Some((
                label.to_string(),
                extensions.iter().map(|e| e.to_string()).collect(),
            )),
        }
    }

    /// A save dialog offering this name.
    pub fn save(suggested: &str) -> Self {
        Self::Save {
            suggested: suggested.to_string(),
            filter: None,
        }
    }

    /// A save dialog offering this name, restricted to these extensions.
    pub fn save_filtered(suggested: &str, label: &str, extensions: &[&str]) -> Self {
        Self::Save {
            suggested: suggested.to_string(),
            filter: Some((
                label.to_string(),
                extensions.iter().map(|e| e.to_string()).collect(),
            )),
        }
    }

    /// Show it, here, now. Blocks until answered.
    fn show(self) -> Option<PathBuf> {
        let with_filter = |mut dialog: rfd::FileDialog,
                           filter: Option<(String, Vec<String>)>|
         -> rfd::FileDialog {
            if let Some((label, extensions)) = filter {
                let refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
                dialog = dialog.add_filter(&label, &refs);
            }
            dialog
        };
        match self {
            Ask::Open { filter } => with_filter(rfd::FileDialog::new(), filter).pick_file(),
            Ask::Save { suggested, filter } => {
                with_filter(rfd::FileDialog::new().set_file_name(&suggested), filter).save_file()
            }
            Ask::Folder => rfd::FileDialog::new().pick_folder(),
        }
    }
}

/// A file dialog that is open, or has just been answered.
///
/// Held by whichever panel asked. `None` inside means nothing is being asked.
#[derive(Default)]
pub struct Pending {
    waiting: Option<mpsc::Receiver<Option<PathBuf>>>,
}

impl Pending {
    /// Nothing is being asked.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a dialog is open right now.
    ///
    /// A caller should disable the button that started it: two pickers open at
    /// once is two answers arriving for one question, and the second would
    /// overwrite the first with no way to tell which was which.
    pub fn is_open(&self) -> bool {
        self.waiting.is_some()
    }

    /// Start asking. Does nothing if a dialog is already open.
    pub fn start(&mut self, ask: Ask) {
        if self.waiting.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.waiting = Some(rx);

        // macOS requires the panel on the main thread. See the module note:
        // this is the platform's rule, not a corner being cut.
        if cfg!(target_os = "macos") {
            let _ = tx.send(ask.show());
            return;
        }
        std::thread::spawn(move || {
            // The receiver is dropped if the panel that asked goes away. That
            // is ordinary -- the answer simply has nowhere to go -- so the
            // failure is discarded rather than unwrapped.
            let _ = tx.send(ask.show());
        });
    }

    /// The answer, if one has arrived. Never waits.
    ///
    /// Returns `Some(None)` when the dialog was cancelled, which is a real
    /// answer and different from "still open".
    pub fn poll(&mut self) -> Option<Option<PathBuf>> {
        let receiver = self.waiting.as_ref()?;
        match receiver.try_recv() {
            Ok(answer) => {
                self.waiting = None;
                Some(answer)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                // The thread went away without answering. Treated as a cancel:
                // leaving it "open" for ever would disable the button that
                // started it and there would be no way back.
                self.waiting = None;
                Some(None)
            }
        }
    }

    /// The answer, if one arrived and was a path.
    pub fn taken(&mut self) -> Option<PathBuf> {
        self.poll().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_pending_is_not_open_and_has_no_answer() {
        let mut pending = Pending::new();
        assert!(!pending.is_open());
        assert_eq!(pending.poll(), None);
        assert_eq!(pending.taken(), None);
    }

    /// A dropped sender is a cancel, not a dialog that stays open for ever.
    /// Otherwise the button that started it is disabled with no way back.
    #[test]
    fn a_thread_that_never_answers_is_treated_as_a_cancel() {
        let (tx, rx) = mpsc::channel();
        let mut pending = Pending { waiting: Some(rx) };
        assert!(pending.is_open());
        drop(tx);
        assert_eq!(pending.poll(), Some(None));
        assert!(!pending.is_open(), "the button has to become usable again");
    }

    /// Cancelling is an answer, and a different one from "still waiting".
    #[test]
    fn cancelling_is_reported_as_an_answer_rather_than_as_silence() {
        let (tx, rx) = mpsc::channel();
        let mut pending = Pending { waiting: Some(rx) };
        assert_eq!(pending.poll(), None, "nothing sent yet");
        tx.send(None).unwrap();
        assert_eq!(pending.poll(), Some(None), "cancelled");
        assert!(!pending.is_open());
    }

    #[test]
    fn a_chosen_path_comes_back_once_and_closes_the_dialog() {
        let (tx, rx) = mpsc::channel();
        let mut pending = Pending { waiting: Some(rx) };
        tx.send(Some(PathBuf::from("/tmp/a.wav"))).unwrap();
        assert_eq!(pending.taken(), Some(PathBuf::from("/tmp/a.wav")));
        assert!(!pending.is_open());
        assert_eq!(pending.taken(), None, "and only once");
    }

    /// Two asks at once would be two answers to one question, and the second
    /// would quietly overwrite the first.
    #[test]
    fn starting_a_second_dialog_while_one_is_open_does_nothing() {
        let (tx, rx) = mpsc::channel();
        let mut pending = Pending { waiting: Some(rx) };
        pending.start(Ask::open());
        assert!(pending.is_open());
        tx.send(Some(PathBuf::from("first"))).unwrap();
        assert_eq!(pending.taken(), Some(PathBuf::from("first")));
    }

    /// The builders carry what they were given, so a caller's filter is not
    /// silently dropped on the way to the dialog.
    #[test]
    fn the_asks_carry_their_filters_and_names() {
        match Ask::open_filtered("audio", &["wav", "mp3"]) {
            Ask::Open {
                filter: Some((label, extensions)),
            } => {
                assert_eq!(label, "audio");
                assert_eq!(extensions, vec!["wav", "mp3"]);
            }
            other => panic!("{other:?}"),
        }
        match Ask::save("out.wav") {
            Ask::Save { suggested, filter } => {
                assert_eq!(suggested, "out.wav");
                assert_eq!(filter, None);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(Ask::open(), Ask::Open { filter: None });
    }

    /// **The point of the module.** No panel may call the blocking picker
    /// directly any more; every ask goes through here, which is the only place
    /// that knows about threads and about macOS.
    #[test]
    fn nothing_outside_this_module_opens_a_dialog_on_the_render_thread() {
        let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(&here).expect("src/") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name == "dialog.rs" {
                continue;
            }
            let source = std::fs::read_to_string(&path)
                .unwrap_or_default()
                .replace("\r\n", "\n");
            for (number, line) in source.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with("///") {
                    continue;
                }
                if trimmed.contains("rfd::FileDialog") {
                    offenders.push(format!("{name}:{}: {}", number + 1, line.trim()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these open a file dialog on the render loop, which freezes the window \
             until it is answered:\n{}",
            offenders.join("\n")
        );
    }
}

#[cfg(test)]
mod house_style {
    /// **No dashes in anything the application shows.**
    ///
    /// Not the em dash, and not the doubled hyphen this project used in its
    /// place. A dash is almost always a colon, a semicolon, a full stop or a
    /// pair of brackets wearing a disguise, and the sentence reads better once
    /// it has been made to choose.
    ///
    /// Checked across the whole crate rather than in one file, because the
    /// strings a reader sees are spread through every panel. Comments are
    /// exempt: this is about the interface, and a sweep of the rest of the
    /// repository is a separate decision that has not been taken.
    #[test]
    fn no_dashes_in_anything_the_interface_says() {
        let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        for entry in std::fs::read_dir(&here).expect("src/") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let source = std::fs::read_to_string(&path)
                .unwrap_or_default()
                .replace("\r\n", "\n");
            // Code only, and only outside the tests: a test may legitimately
            // quote a dash in order to assert something about it, and this
            // very test contains two.
            let body = source.split("#[cfg(test)]").next().unwrap_or("");
            for (number, line) in body.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                if line.contains('\u{2014}') || line.contains(" -- ") {
                    offenders.push(format!("{name}:{}: {}", number + 1, line.trim()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "these carry a dash into the interface; rewrite the sentence with a \
             colon, a semicolon, a full stop or brackets:\n{}",
            offenders.join("\n")
        );
    }
}
