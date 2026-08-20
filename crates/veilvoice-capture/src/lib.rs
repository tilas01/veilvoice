// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-capture
//!
//! Which screen-recording programs are running, an allowlist for the ones you
//! meant to run, and a plain account of the two things this cannot do.
//!
//! ## VeilVoice does not hide itself from your recorder
//!
//! Stated first, because it is the question people actually have: **you can
//! record VeilVoice's window with OBS, and nothing here stops you.** Screen
//! capture of this application is not blocked, not degraded, and not detected
//! as an attack. If you are making a video about VeilVoice, or streaming while
//! you use it, it will appear on the recording exactly like any other window.
//!
//! That is not only a choice, it is also a limit. Excluding a window from
//! capture means `SetWindowDisplayAffinity` on Windows and the equivalent
//! elsewhere, which is FFI — and every crate in this workspace carries
//! `#![forbid(unsafe_code)]`, which is a front-page claim. So the exclusion is
//! **not built**, and `ROADMAP.md` records it as a decision waiting on the
//! maintainer rather than as an oversight. Anybody who needs a window that
//! cannot be recorded does not have it here, and should know that before they
//! rely on it.
//!
//! ## Telling you, and then not telling you again
//!
//! A monitor that says "OBS is running" every thirty seconds while you
//! deliberately record a tutorial is a monitor you turn off, and then it is not
//! watching for the recorder you did **not** start. So [`Allowlist`] exists:
//! name a program once, and it stops raising a notification.
//!
//! Allowed is not hidden. [`Sighting::allowed`] is a flag on a sighting that is
//! still in the report, and [`Report::all`] still lists it. Only
//! [`Report::worth_saying`] filters, because that is the one a notification
//! reads. Something that vanished from the interface entirely would be a
//! setting for lying to yourself.
//!
//! ## Running is not recording, and this crate never confuses them
//!
//! Zoom being open does not mean Zoom is sharing your screen; Discord running
//! does not mean anybody is watching. This reports **what is running**, and
//! [`programs::Purpose`] separates a program whose job is recording from one
//! that merely can. Every sentence produced here keeps the distinction, because
//! a privacy tool that announces surveillance every time somebody opens a chat
//! application has taught its user to ignore it.
//!
//! Knowing whether a capture is actually in progress means asking the
//! compositor who holds a capture session, and that is FFI on every platform
//! here. Same trade, same answer, same place it is recorded.
//!
//! ## And it only knows the programs it knows
//!
//! [`programs::ALL`] is a table of names. Something not in it is not reported,
//! and something written to record a screen quietly would not be called
//! `obs64.exe`. An empty report is not evidence that nothing is recording, and
//! [`SCOPE`] says so.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod processes;
pub mod programs;

use std::collections::BTreeSet;
use std::path::Path;

pub use programs::{Program, Purpose};

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What this is worth, in the words a front end should show.
///
/// Single-sourced and asserted by the tests, exactly as every other scope note
/// in this project is, so it cannot quietly turn into a promise.
pub const SCOPE: &str =
    "This lists screen-recording programs that are running, from a table of names this \
     build knows. It cannot see a program that is not in that table, so an empty report \
     is not evidence that nothing is recording. It cannot tell whether a program that is \
     running is actually capturing anything -- a meeting application being open is not \
     somebody watching your screen. And it does not hide VeilVoice's own window from \
     capture: you can record this application, deliberately, and nothing here prevents \
     it.";

/// Programs the user has said they meant to run.
///
/// A set of [`Program::key`] values, kept in a plain text file. Nothing here is
/// secret and there is nothing to protect: an allowlist is a note to yourself
/// about which notifications you have already read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Allowlist {
    keys: BTreeSet<String>,
}

/// Magic first line. The digit is a format version.
const MAGIC: &str = "VEILCAPTURE1";

impl Allowlist {
    /// An allowlist that allows nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stop notifying about this program.
    ///
    /// Refuses a key this build does not know. An allowlist entry for a
    /// misspelled program silently allows nothing, and the user believes they
    /// have turned a notification off.
    pub fn allow(&mut self, key: &str) -> Result<(), Error> {
        let key = key.trim();
        if programs::by_key(key).is_none() {
            return Err(Error::Unknown(key.to_string()));
        }
        self.keys.insert(key.to_string());
        Ok(())
    }

    /// Start notifying about this program again.
    pub fn deny(&mut self, key: &str) {
        self.keys.remove(key.trim());
    }

    /// Whether this program is allowed.
    pub fn allows(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    /// The allowed keys, in a stable order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.keys.iter().map(String::as_str)
    }

    /// How many programs are allowed.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether nothing is allowed.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Serialise to a text format, one key per line.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        for key in &self.keys {
            out.push_str(&format!("allow  {key}\n"));
        }
        out
    }

    /// Parse the text format.
    ///
    /// A key this build does not know is an **error**, not a line to skip: it
    /// is either a typo or a file from a newer build, and both mean the user
    /// believes a notification is off when it is not.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.trim() == MAGIC => {}
            Some(other) => {
                return Err(Error::Malformed(format!(
                    "expected {MAGIC} on the first line, found {other:?}"
                )))
            }
            None => return Err(Error::Malformed("the allowlist is empty".into())),
        }
        let mut allowlist = Self::new();
        for (index, line) in lines.enumerate() {
            let number = index + 2;
            if line.trim().is_empty() {
                continue;
            }
            let Some((keyword, rest)) = line.split_once("  ") else {
                return Err(Error::Malformed(format!(
                    "line {number}: no keyword, found {line:?}"
                )));
            };
            if keyword != "allow" {
                return Err(Error::Malformed(format!(
                    "line {number}: unknown keyword {keyword:?}"
                )));
            }
            allowlist
                .allow(rest.trim())
                .map_err(|error| Error::Malformed(format!("line {number}: {error}")))?;
        }
        Ok(allowlist)
    }

    /// Write the allowlist to `path`.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.to_text())?;
        Ok(())
    }

    /// Read an allowlist written by [`Allowlist::save`], treating a missing
    /// file as "nothing is allowed".
    ///
    /// A missing file is the ordinary state before anything has been allowed. A
    /// file that exists and will not parse is reported: quietly starting from
    /// an empty allowlist would turn every suppressed notification back on with
    /// no explanation, which reads as the tool having gone wrong.
    pub fn load(path: &Path) -> Result<Self, Error> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::new()),
            Err(error) => Err(Error::Io(error)),
        }
    }
}

/// One capture-capable program found running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sighting {
    /// Which program it is.
    pub program: &'static Program,
    /// The process name that matched, as the platform reported it.
    pub process: String,
    /// The user has said they meant to run this.
    ///
    /// Still in the report. Only a notification filters on it.
    pub allowed: bool,
}

impl Sighting {
    /// One line for a terminal, a log or a notification.
    ///
    /// Says what is running and what that does and does not mean. Never says
    /// that anything is being recorded, because this cannot know that.
    pub fn describe(&self) -> String {
        let mut line = format!("{} {}", self.program.name, self.program.purpose.phrasing());
        if self.allowed {
            line.push_str(" -- allowed by you, so this is not a notification");
        }
        line
    }
}

/// What is running, and anything that got in the way of finding out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Everything found, allowed or not, in table order.
    pub sightings: Vec<Sighting>,
    /// Why the answer may be short. Empty means the listing itself worked --
    /// **not** that nothing is recording.
    pub problems: Vec<String>,
}

impl Report {
    /// Look at what is running now.
    pub fn take(allowlist: &Allowlist) -> Self {
        let (names, problems) = processes::running();
        let mut sightings: Vec<Sighting> = Vec::new();
        for name in names {
            let Some(program) = programs::matching(&name) else {
                continue;
            };
            // One line per program, not per process: a recorder with four
            // helper processes is one thing running, and saying it four times
            // is how a report becomes noise.
            if sightings.iter().any(|seen| seen.program.key == program.key) {
                continue;
            }
            sightings.push(Sighting {
                program,
                process: name,
                allowed: allowlist.allows(program.key),
            });
        }
        // Table order, so dedicated recorders come before chat applications
        // however the platform happened to list its processes.
        sightings.sort_by_key(|sighting| {
            programs::ALL
                .iter()
                .position(|program| program.key == sighting.program.key)
                .unwrap_or(usize::MAX)
        });
        Self {
            sightings,
            problems,
        }
    }

    /// Everything found, allowed or not.
    pub fn all(&self) -> &[Sighting] {
        &self.sightings
    }

    /// The sightings a notification should raise: the ones not allowed.
    ///
    /// The only thing in this crate that filters on [`Sighting::allowed`].
    pub fn worth_saying(&self) -> Vec<&Sighting> {
        self.sightings
            .iter()
            .filter(|sighting| !sighting.allowed)
            .collect()
    }

    /// Whether anything at all was found, allowed or not.
    pub fn is_empty(&self) -> bool {
        self.sightings.is_empty()
    }
}

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    Io(std::io::Error),
    /// A stored allowlist is not in a form this build understands.
    Malformed(String),
    /// A program name this build does not know.
    Unknown(String),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "input/output error: {error}"),
            Self::Malformed(what) => write!(f, "malformed allowlist: {what}"),
            Self::Unknown(key) => write!(
                f,
                "this build does not know a program called {key:?}, so allowing it would \
                 turn off a notification that was never going to be raised. Known: {}",
                programs::ALL
                    .iter()
                    .map(|program| program.key)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sighting(key: &str, allowed: bool) -> Sighting {
        Sighting {
            program: programs::by_key(key).unwrap(),
            process: "whatever".into(),
            allowed,
        }
    }

    /// The claim must keep stating all three limits. If somebody edits this
    /// into a promise, this is what stops it shipping.
    #[test]
    fn the_scope_note_states_the_limits_rather_than_a_guarantee() {
        let scope = SCOPE.to_lowercase();
        assert!(
            scope.contains("not evidence that nothing is recording"),
            "the table is incomplete and the note must say so"
        );
        assert!(
            scope.contains(
                "cannot tell whether a program that is running is actually \
                            capturing"
            ),
            "running is not recording, and the note must say so"
        );
        assert!(
            scope.contains("does not hide veilvoice's own window"),
            "the thing people will assume must be denied explicitly"
        );
        // Whole claims, not single verbs: the note legitimately contains the
        // word "prevents", in the sentence "nothing here prevents it". A
        // substring check for a verb cannot tell a boast from its denial.
        for boast in [
            "veilvoice prevents",
            "blocks capture",
            "stops recording",
            "guarantee",
            "protects you from",
            "cannot be recorded",
        ] {
            assert!(!scope.contains(boast), "overclaim: {boast}");
        }
    }

    #[test]
    fn allowing_and_denying_a_known_program() {
        let mut allowlist = Allowlist::new();
        assert!(allowlist.is_empty());
        allowlist.allow("obs").unwrap();
        assert!(allowlist.allows("obs"));
        assert_eq!(allowlist.len(), 1);
        assert_eq!(allowlist.keys().collect::<Vec<_>>(), vec!["obs"]);
        allowlist.deny("obs");
        assert!(!allowlist.allows("obs"));
        assert!(allowlist.is_empty());
    }

    /// A misspelled key would silently allow nothing while the user believed a
    /// notification was off.
    #[test]
    fn allowing_a_program_this_build_does_not_know_is_refused() {
        let mut allowlist = Allowlist::new();
        let error = allowlist.allow("obss").expect_err("a typo must be refused");
        assert!(error.to_string().contains("does not know"), "{error}");
        assert!(
            error.to_string().contains("obs"),
            "the known keys must be offered: {error}"
        );
        assert!(allowlist.is_empty());
    }

    #[test]
    fn an_allowlist_survives_a_round_trip_through_text() {
        let mut allowlist = Allowlist::new();
        allowlist.allow("obs").unwrap();
        allowlist.allow("zoom").unwrap();
        let text = allowlist.to_text();
        let read_back = Allowlist::parse(&text).expect("its own output must parse");
        assert_eq!(read_back, allowlist);
        assert_eq!(read_back.to_text(), text, "and byte for byte");
    }

    #[test]
    fn an_allowlist_survives_a_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut allowlist = Allowlist::new();
        allowlist.allow("obs").unwrap();
        let path = dir.path().join("deeper").join("allow.txt");
        allowlist.save(&path).unwrap();
        assert_eq!(Allowlist::load(&path).unwrap(), allowlist);
    }

    /// Nothing allowed yet is the ordinary state, not an error.
    #[test]
    fn a_missing_allowlist_allows_nothing_without_complaining() {
        let dir = tempfile::tempdir().unwrap();
        let allowlist = Allowlist::load(&dir.path().join("not-here.txt")).unwrap();
        assert!(allowlist.is_empty());
    }

    #[test]
    fn a_malformed_allowlist_is_refused_rather_than_half_read() {
        assert!(Allowlist::parse("").is_err(), "empty");
        assert!(Allowlist::parse("NOT-THE-MAGIC\n").is_err());
        for bad in ["deny  obs", "allow  nonsense", "nokeyword"] {
            let text = format!("{MAGIC}\n{bad}\n");
            assert!(Allowlist::parse(&text).is_err(), "should refuse: {bad:?}");
        }
    }

    #[test]
    fn blank_lines_are_tolerated() {
        let mut allowlist = Allowlist::new();
        allowlist.allow("obs").unwrap();
        let padded = allowlist.to_text().replace('\n', "\n\n");
        assert_eq!(Allowlist::parse(&padded).unwrap(), allowlist);
    }

    /// Allowed is muted, never hidden: it stays in the report and only a
    /// notification filters it out.
    #[test]
    fn an_allowed_program_stays_in_the_report_and_out_of_the_notification() {
        let report = Report {
            sightings: vec![sighting("obs", true), sighting("zoom", false)],
            problems: Vec::new(),
        };
        assert_eq!(report.all().len(), 2, "both must stay visible");
        let worth_saying = report.worth_saying();
        assert_eq!(worth_saying.len(), 1);
        assert_eq!(worth_saying[0].program.key, "zoom");
        assert!(!report.is_empty());
    }

    /// An allowed sighting says it is allowed, so somebody reading the full
    /// list knows why it is quiet.
    #[test]
    fn an_allowed_sighting_says_why_it_is_quiet() {
        assert!(sighting("obs", true).describe().contains("allowed by you"));
        assert!(!sighting("obs", false).describe().contains("allowed by you"));
    }

    /// No line may claim that anything is being recorded, because nothing here
    /// can know that.
    #[test]
    fn no_sighting_claims_a_recording_is_happening() {
        for key in ["obs", "zoom", "discord"] {
            for allowed in [true, false] {
                let line = sighting(key, allowed).describe().to_lowercase();
                assert!(
                    !line.contains("is recording your")
                        && !line.contains("is capturing")
                        && !line.contains("being watched"),
                    "{line}"
                );
            }
        }
        // And a merely capable program must not be described as a recorder.
        assert!(sighting("zoom", false).describe().contains("can share"));
    }

    /// A recorder with four helper processes is one thing running.
    #[test]
    fn a_program_is_reported_once_however_many_processes_it_has() {
        let report = Report::take(&Allowlist::new());
        let mut keys: Vec<&str> = report
            .all()
            .iter()
            .map(|sighting| sighting.program.key)
            .collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(before, keys.len(), "a program was reported twice");
    }

    /// Looking at the real machine must not panic, and an empty report must
    /// never be silent about why.
    #[test]
    fn taking_a_real_report_does_not_panic() {
        let report = Report::take(&Allowlist::new());
        for sighting in report.all() {
            assert!(!sighting.describe().is_empty());
        }
        // Whatever it found, the allowlist must be able to mute all of it.
        let mut allowlist = Allowlist::new();
        for sighting in report.all() {
            allowlist.allow(sighting.program.key).unwrap();
        }
        assert!(Report::take(&allowlist).worth_saying().is_empty());
    }

    #[test]
    fn an_io_error_displays_and_keeps_its_source() {
        let error = Error::from(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert!(error.to_string().contains("gone"));
        assert!(std::error::Error::source(&error).is_some());
        assert!(Error::Malformed("x".into()).to_string().contains("x"));
    }
}
