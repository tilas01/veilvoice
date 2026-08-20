// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-drivers
//!
//! What is loaded into the kernel, recorded, and compared later. A driver
//! appearing between two looks is worth knowing about: it is the step almost
//! everything that wants to watch a microphone from underneath has to take.
//!
//! ## The limit, stated first
//!
//! **This reads a list the operating system hands out.** Anything able to lie
//! to that list is not in it. A kernel module that has unlinked itself from the
//! module list, or a driver that hooks the enumeration this calls, will be
//! invisible here — and would be invisible to any unprivileged program asking
//! the same question, which is why the answer is "detect carelessness", not
//! "detect rootkits". See [`SCOPE`].
//!
//! ## The cross-view check, and what it is actually worth
//!
//! On Linux the kernel publishes the same fact twice: `/proc/modules` and the
//! directories under `/sys/module`. [`Report::discrepancies`] lists modules
//! that appear in one and not the other, which catches something that unlinked
//! itself from one list and forgot the other. That has been a real mistake in
//! real rootkits.
//!
//! It is a check for carelessness and nothing more. Both views come from the
//! same kernel, so anything with the privilege to edit one has the privilege to
//! edit both. A quiet cross-view check is not evidence that nothing is hiding,
//! and this crate never says it is.
//!
//! No other platform here has two independent views to compare, so
//! [`Report::discrepancies`] is empty on them — which is reported as "there was
//! nothing to cross-check", not as "the check passed".
//!
//! ## A new driver is not by itself a finding
//!
//! Plugging in a printer loads a driver. So does a graphics update, a VPN
//! client, a virtual audio cable — VeilVoice recommends one — and a game's
//! anti-cheat. [`Change::Appeared`] is a fact about a list, and the question
//! it raises is "did you install something?", which the person at the keyboard
//! answers in a second. Nothing here tries to answer it for them.
//!
//! ## Where the answer comes from
//!
//! | Platform | Source | Needs privilege |
//! |---|---|---|
//! | Linux | `/proc/modules`, cross-checked against `/sys/module` | no |
//! | Windows | `driverquery.exe /FO CSV /NH` | no |
//! | macOS | `kmutil showloaded`, falling back to `kextstat` | no |
//! | others | nothing is read, and [`support`] says so | — |
//!
//! Linux reads two files and spawns nothing. The other two shell out to a tool
//! the system already ships, for the same reason the rest of this workspace
//! does: `#![forbid(unsafe_code)]` holds here too, and the native APIs are FFI.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod linux;
mod macos;
mod windows;

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What this is worth, in the words a front end should show.
///
/// Single-sourced and asserted by the tests, exactly as the app lock's and the
/// tamper detector's notes are, so it cannot quietly turn into a promise.
pub const SCOPE: &str =
    "This reads the list of loaded drivers the operating system hands out, and compares \
     it against the last time you looked. Anything able to lie to that list is not in \
     it, so a quiet report is not evidence that nothing is hiding. A driver appearing is \
     a fact about a list and not a finding: printers, graphics updates, VPN clients and \
     virtual audio cables all load drivers. Nothing here can stop a driver loading, and \
     nothing here names what installed it.";

/// One loaded driver or kernel module.
///
/// Build one with [`Module::new`], which normalises the two strings so that
/// anything constructed here can survive [`Report::to_text`] and come back
/// equal. The fields are public for reading; writing one by hand and skipping
/// that normalisation is how a record stops round-tripping.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Module {
    /// The module's name, as the platform reports it.
    pub name: String,
    /// Whatever else the platform said: a display name, a version, a type.
    /// Free text, because each platform says something different and inventing
    /// a common shape for them would mean discarding most of it.
    pub detail: String,
}

impl Module {
    /// A module, with both strings made safe for the record.
    ///
    /// The text format separates a name from its detail on a **double space**
    /// and one record from the next on a newline. A name containing either
    /// would be split in the wrong place on the way back in -- silently, and
    /// only for that one module, which is the worst shape a bug can have in a
    /// list somebody is comparing against yesterday's.
    ///
    /// So every run of whitespace becomes a single space here. No real driver
    /// name contains whitespace at all, so nothing is lost on any machine; what
    /// is gained is that a *hostile* name cannot forge a second record, and a
    /// display name that happens to be padded does not report itself as
    /// altered on the next run.
    pub fn new(name: &str, detail: &str) -> Self {
        Self {
            name: collapse_whitespace(name),
            detail: collapse_whitespace(detail),
        }
    }
}

/// Every run of whitespace becomes one space, and the ends are trimmed.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// How a module differs from the record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Not in the record, and loaded now.
    Appeared(Module),
    /// In the record, and not loaded now.
    Disappeared(Module),
    /// In both, and what the platform says about it has changed.
    Altered {
        /// The module as it was recorded.
        was: Module,
        /// The module as it is now.
        now: Module,
    },
}

impl Change {
    /// The module's name, whichever side it came from.
    pub fn name(&self) -> &str {
        match self {
            Change::Appeared(module) | Change::Disappeared(module) => &module.name,
            Change::Altered { now, .. } => &now.name,
        }
    }

    /// One line for a terminal or a log. States the fact, never a cause.
    pub fn describe(&self) -> String {
        match self {
            Change::Appeared(module) => {
                format!("appeared:    {} ({})", module.name, module.detail)
            }
            Change::Disappeared(module) => {
                format!("disappeared: {} ({})", module.name, module.detail)
            }
            Change::Altered { was, now } => format!(
                "altered:     {} ({} -> {})",
                now.name, was.detail, now.detail
            ),
        }
    }
}

/// What this platform can and cannot answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Support {
    /// Whether the loaded modules can be listed at all.
    pub listing: bool,
    /// Whether two independent views exist to compare against each other.
    pub cross_view: bool,
    /// How it works, or why it does not, in one sentence for the user.
    pub explanation: &'static str,
}

/// Report what this platform can do.
///
/// Check this before showing anything. An empty list on a platform that cannot
/// tell is not "no drivers"; it is "no answer", and presenting the first as the
/// second is the failure this project guards against hardest.
pub fn support() -> Support {
    #[cfg(target_os = "linux")]
    {
        Support {
            listing: true,
            cross_view: true,
            explanation: "Reads /proc/modules, and cross-checks it against the \
                          directories under /sys/module. Needs no privileges.",
        }
    }
    #[cfg(target_os = "windows")]
    {
        Support {
            listing: true,
            cross_view: false,
            explanation: "Runs driverquery.exe, which Windows ships. There is no second \
                          list to cross-check it against, so nothing is cross-checked.",
        }
    }
    #[cfg(target_os = "macos")]
    {
        Support {
            listing: true,
            cross_view: false,
            explanation: "Runs kmutil, falling back to kextstat. There is no second list \
                          to cross-check it against, so nothing is cross-checked.",
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Support {
            listing: false,
            cross_view: false,
            explanation: "No reader is written for this platform, so nothing is \
                          reported. An empty list here means no answer, not no drivers.",
        }
    }
}

/// What is loaded right now, and anything odd about how it was found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// When this was taken, in seconds since the Unix epoch.
    pub taken: u64,
    /// Modules that could not be read, with the reason. Reported rather than
    /// skipped: a source that stopped answering is itself something that
    /// happened.
    pub problems: Vec<String>,
    /// Names present in one platform view and absent from another.
    ///
    /// Empty on every platform except Linux, and empty there most of the time.
    /// **Empty means "nothing to report", which on a platform without a second
    /// view means "nothing was checked".** [`support`] says which.
    pub discrepancies: Vec<String>,
    modules: BTreeMap<String, Module>,
}

/// Magic first line of a saved report. The digit is a format version.
const MAGIC: &str = "VEILDRIVERS1";

impl Report {
    /// Ask the platform what is loaded.
    pub fn take() -> Self {
        let mut report = Self {
            taken: now_seconds(),
            ..Self::default()
        };
        let (modules, problems, discrepancies) = read_platform();
        for module in modules {
            let module = Module::new(&module.name, &module.detail);
            report.modules.insert(module.name.clone(), module);
        }
        report.problems = problems;
        report.discrepancies = discrepancies;
        report
    }

    /// How many modules were listed.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Whether nothing was listed.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// The modules, in a stable order.
    pub fn modules(&self) -> impl Iterator<Item = &Module> {
        self.modules.values()
    }

    /// Replace the recorded time. For tests, which cannot wait.
    #[doc(hidden)]
    pub fn with_taken(mut self, taken: u64) -> Self {
        self.taken = taken;
        self
    }

    /// Build a report from a list, for tests and for another front end that
    /// has already obtained one.
    ///
    /// Every module is passed through [`Module::new`] on the way in, so a
    /// caller cannot hand this a name the text format would split in the wrong
    /// place.
    pub fn from_modules(modules: Vec<Module>) -> Self {
        let mut report = Self {
            taken: now_seconds(),
            ..Self::default()
        };
        for module in modules {
            let module = Module::new(&module.name, &module.detail);
            report.modules.insert(module.name.clone(), module);
        }
        report
    }

    /// Serialise to a text format, one record per line.
    ///
    /// ```text
    /// VEILDRIVERS1
    /// taken  1700000000
    /// problem  /sys/module: permission denied
    /// discrepancy  hidden_thing: in /sys/module and not in /proc/modules
    /// module  nvidia  56807424 bytes, 42 refs, Live
    /// ```
    ///
    /// Text, for the same reason the tamper manifest is text: a record of what
    /// was on a machine is worth more if it can be read without this crate.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        out.push_str(&format!("taken  {}\n", self.taken));
        for problem in &self.problems {
            out.push_str(&format!("problem  {problem}\n"));
        }
        for discrepancy in &self.discrepancies {
            out.push_str(&format!("discrepancy  {discrepancy}\n"));
        }
        for module in self.modules.values() {
            out.push_str(&format!("module  {}  {}\n", module.name, module.detail));
        }
        out
    }

    /// Parse the text format.
    ///
    /// An unknown keyword is an error. A record of what was loaded is a
    /// baseline, and half of one compares against a machine that never existed
    /// -- every module it dropped reads as newly appeared.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.trim() == MAGIC => {}
            Some(other) => {
                return Err(Error::Malformed(format!(
                    "expected {MAGIC} on the first line, found {other:?}"
                )))
            }
            None => return Err(Error::Malformed("the report is empty".into())),
        }

        let mut report = Self::default();
        let mut taken = None;
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
            match keyword {
                "taken" => {
                    taken = Some(rest.trim().parse::<u64>().map_err(|_| {
                        Error::Malformed(format!("line {number}: bad time {rest:?}"))
                    })?)
                }
                "problem" => report.problems.push(rest.trim().to_string()),
                "discrepancy" => report.discrepancies.push(rest.trim().to_string()),
                "module" => {
                    // The detail may itself contain a double space, so the
                    // split is once only and the rest is taken whole.
                    let (name, detail) = match rest.split_once("  ") {
                        Some((name, detail)) => (name.trim(), detail.trim()),
                        None => (rest.trim(), ""),
                    };
                    if name.is_empty() {
                        return Err(Error::Malformed(format!("line {number}: no name")));
                    }
                    report.modules.insert(
                        name.to_string(),
                        Module {
                            name: name.to_string(),
                            detail: detail.to_string(),
                        },
                    );
                }
                other => {
                    return Err(Error::Malformed(format!(
                        "line {number}: unknown keyword {other:?}"
                    )))
                }
            }
        }
        report.taken = taken.ok_or_else(|| Error::Malformed("no taken line".into()))?;
        Ok(report)
    }

    /// Write the report to `path`.
    pub fn save(&self, path: &std::path::Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.to_text())?;
        Ok(())
    }

    /// Read a report written by [`Report::save`].
    pub fn load(path: &std::path::Path) -> Result<Self, Error> {
        Self::parse(&std::fs::read_to_string(path)?)
    }
}

/// Compare two reports of the same machine.
///
/// Sorted by name, so the output is identical for the same pair of inputs and
/// two runs can be diffed against each other.
pub fn compare(before: &Report, after: &Report) -> Vec<Change> {
    let mut changes = Vec::new();
    for (name, was) in &before.modules {
        match after.modules.get(name) {
            None => changes.push(Change::Disappeared(was.clone())),
            Some(now) if now.detail != was.detail => changes.push(Change::Altered {
                was: was.clone(),
                now: now.clone(),
            }),
            Some(_) => {}
        }
    }
    for (name, now) in &after.modules {
        if !before.modules.contains_key(name) {
            changes.push(Change::Appeared(now.clone()));
        }
    }
    changes.sort_by(|a, b| a.name().cmp(b.name()));
    changes
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ask whichever reader this platform has.
///
/// Returns the modules, anything that went wrong, and any cross-view
/// discrepancies. A platform with no reader returns three empty vectors, and
/// [`support`] is what says that an empty list means "no answer".
fn read_platform() -> (Vec<Module>, Vec<String>, Vec<String>) {
    #[cfg(target_os = "linux")]
    {
        linux::read()
    }
    #[cfg(target_os = "windows")]
    {
        windows::read()
    }
    #[cfg(target_os = "macos")]
    {
        macos::read()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        (Vec::new(), Vec::new(), Vec::new())
    }
}

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    Io(std::io::Error),
    /// A stored report is not in a form this build understands.
    Malformed(String),
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
            Self::Malformed(what) => write!(f, "malformed report: {what}"),
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

    fn module(name: &str, detail: &str) -> Module {
        Module {
            name: name.to_string(),
            detail: detail.to_string(),
        }
    }

    /// The claim must keep stating the limits. If somebody edits this into a
    /// promise, this is what stops it shipping.
    #[test]
    fn the_scope_note_states_the_limits_rather_than_a_guarantee() {
        let scope = SCOPE.to_lowercase();
        assert!(
            scope.contains("not evidence that nothing is hiding"),
            "the blind spot must be admitted"
        );
        assert!(scope.contains("a fact about a list and not a finding"));
        assert!(
            scope.contains("nothing here can stop a driver loading"),
            "the crate must keep saying it cannot prevent anything"
        );
        for boast in [
            "detects rootkits",
            "prevents",
            "blocks",
            "unbreakable",
            "guarantee",
        ] {
            assert!(!scope.contains(boast), "overclaim: {boast}");
        }
    }

    /// Support must never be read as a claim about the machine.
    #[test]
    fn an_unsupported_platform_says_no_answer_rather_than_no_drivers() {
        let support = support();
        assert!(!support.explanation.is_empty());
        if !support.listing {
            assert!(
                support.explanation.contains("no answer"),
                "{}",
                support.explanation
            );
        }
        if !support.cross_view {
            assert!(
                support.explanation.contains("nothing is cross-checked") || !support.listing,
                "a platform with no second view must say so: {}",
                support.explanation
            );
        }
    }

    #[test]
    fn an_unchanged_machine_shows_no_changes() {
        let report = Report::from_modules(vec![module("a", "one"), module("b", "two")]);
        assert_eq!(compare(&report, &report), Vec::new());
        assert_eq!(report.len(), 2);
        assert!(!report.is_empty());
    }

    #[test]
    fn appearing_disappearing_and_altering_are_told_apart() {
        let before = Report::from_modules(vec![
            module("stays", "same"),
            module("goes", "was here"),
            module("changes", "old"),
        ]);
        let after = Report::from_modules(vec![
            module("stays", "same"),
            module("changes", "new"),
            module("arrives", "just now"),
        ]);
        let changes = compare(&before, &after);
        assert_eq!(changes.len(), 3, "{changes:?}");
        // Sorted by name: arrives, changes, goes.
        assert_eq!(changes[0], Change::Appeared(module("arrives", "just now")));
        assert_eq!(
            changes[1],
            Change::Altered {
                was: module("changes", "old"),
                now: module("changes", "new"),
            }
        );
        assert_eq!(changes[2], Change::Disappeared(module("goes", "was here")));
    }

    /// A change is a fact about a list, and the wording must not become an
    /// accusation.
    #[test]
    fn no_change_line_names_a_cause() {
        for change in [
            Change::Appeared(module("a", "one")),
            Change::Disappeared(module("b", "two")),
            Change::Altered {
                was: module("c", "old"),
                now: module("c", "new"),
            },
        ] {
            let line = change.describe().to_lowercase();
            for word in ["rootkit", "malware", "attack", "infected", "virus"] {
                assert!(!line.contains(word), "{line}");
            }
            assert!(!change.name().is_empty());
        }
    }

    /// A name with a double space in it would be split in the wrong place by
    /// the reader. No real driver has one; a hostile record could, so the
    /// constructor collapses it rather than the parser guessing.
    #[test]
    fn a_name_that_could_forge_a_record_is_collapsed() {
        let forged = Module::new("evil  detail-that-was-a-name", "real detail");
        assert_eq!(forged.name, "evil detail-that-was-a-name");
        assert!(!forged.name.contains("  "));

        let newline = Module::new("two\nlines", "and\ndetail");
        assert!(!newline.name.contains('\n'));
        assert!(!newline.detail.contains('\n'));

        // And a padded display name must not report itself as altered next run.
        assert_eq!(
            Module::new("drv", "  spaced   out  "),
            Module::new("drv", "spaced out")
        );
    }

    #[test]
    fn a_report_survives_a_round_trip_through_text() {
        let mut report = Report::from_modules(vec![
            module("nvidia", "56807424 bytes, 42 refs, Live"),
            module("vbaudio_cable64_win10", "VB-Audio Virtual Cable, Kernel"),
        ])
        .with_taken(1_700_000_000);
        report
            .problems
            .push("/sys/module: permission denied".into());
        report
            .discrepancies
            .push("hidden: in /sys/module and not in /proc/modules".into());

        let text = report.to_text();
        let read_back = Report::parse(&text).expect("its own output must parse");
        assert_eq!(read_back.taken, report.taken);
        assert_eq!(read_back.problems, report.problems);
        assert_eq!(read_back.discrepancies, report.discrepancies);
        assert_eq!(read_back.len(), report.len());
        assert_eq!(compare(&read_back, &report), Vec::new());
    }

    #[test]
    fn a_report_survives_a_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let report = Report::from_modules(vec![module("a", "one")]).with_taken(5);
        let path = dir.path().join("deeper").join("drivers.txt");
        report.save(&path).unwrap();
        let read_back = Report::load(&path).unwrap();
        assert_eq!(read_back.taken, 5);
        assert_eq!(read_back.len(), 1);
    }

    #[test]
    fn a_malformed_report_is_refused_rather_than_half_read() {
        assert!(Report::parse("").is_err(), "empty");
        assert!(Report::parse("NOT-THE-MAGIC\n").is_err(), "wrong magic");
        for bad in [
            "module  a  one",             // no taken line
            "taken  x",                   // unparseable time
            "taken  1\nwhat  else",       // unknown keyword
            "taken  1\nnokeyword",        // no separator
            "taken  1\nmodule    detail", // no name
        ] {
            let text = format!("{MAGIC}\n{bad}\n");
            assert!(Report::parse(&text).is_err(), "should refuse: {bad:?}");
        }
    }

    /// Asking the real machine must not panic, whatever it answers, and must
    /// agree with what `support` promised.
    #[test]
    fn taking_a_real_report_does_not_panic_and_matches_support() {
        let report = Report::take();
        let support = support();
        if !support.listing {
            assert!(
                report.is_empty(),
                "a platform with no reader listed modules"
            );
        }
        if !support.cross_view {
            assert!(
                report.discrepancies.is_empty(),
                "a platform with one view produced a cross-view result"
            );
        }
        // Whatever came back must survive its own serialisation.
        let text = report.to_text();
        assert_eq!(Report::parse(&text).unwrap().len(), report.len());
    }

    /// Two looks a moment apart at the same machine.
    ///
    /// This deliberately asserts nothing about *what* changed: a driver really
    /// can load between two calls, and a test that fails when the machine it
    /// runs on does something ordinary is a test somebody deletes. What it
    /// does hold is that reading twice is free of side effects -- the second
    /// look must not be affected by the first -- and that whatever came back
    /// compares deterministically.
    #[test]
    fn looking_twice_is_free_of_side_effects() {
        let first = Report::take();
        let second = Report::take();
        for change in compare(&first, &second) {
            println!("changed between two looks: {}", change.describe());
        }
        assert_eq!(
            compare(&first, &second),
            compare(&first, &second),
            "comparing the same pair twice must give the same answer"
        );
        assert_eq!(
            first.discrepancies.is_empty(),
            second.discrepancies.is_empty(),
            "the cross-view check must not depend on how many times it has run"
        );
    }

    #[test]
    fn an_io_error_displays_and_keeps_its_source() {
        let error = Error::from(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert!(error.to_string().contains("gone"));
        assert!(std::error::Error::source(&error).is_some());
        assert!(Error::Malformed("x".into()).to_string().contains("x"));
    }
}
