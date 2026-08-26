// SPDX-License-Identifier: GPL-3.0-or-later
//! How much this program says, and what it returns when it says nothing.
//!
//! # The exit status comes first, and that is not an accident
//!
//! This module exists because of one requirement: **there is a verbosity level
//! called "nothing"**. A tool that prints nothing and returns zero when a
//! signature did not verify is worse than a noisy one -- it is a tool that
//! reports success by staying quiet, which is the shape every failure takes
//! when nobody is watching.
//!
//! So the statuses are defined and documented first, and `--quiet` is only
//! usable *because* they are. [`Status`] gives every outcome its own number,
//! they are stable, and `--help` prints the table.
//!
//! # Failing, refusing, and the difference
//!
//! Two outcomes are both "not success" and must never be confused:
//!
//! * [`Status::Refused`] -- a check ran and the answer was wrong. The signature
//!   did not verify, or a hash did not match. Somebody may have tampered with
//!   a release.
//! * [`Status::Incomplete`] -- a check did not run. A download failed, a file
//!   was missing, a tool was not installed. **Nothing has been proven either
//!   way**, which is not the same as nothing being wrong.
//!
//! The existing reporting already keeps these apart in the words it prints.
//! This gives them separate numbers so a script can tell them apart too, since
//! a script is exactly the reader who gets no words.
//!
//! # In plain words
//!
//! This decides how much the program prints -- from every detail down to
//! absolutely nothing -- and makes sure that when it prints nothing it still
//! *tells* you the answer, through the number every program hands back when it
//! finishes. Something has to carry the answer. If it is not the text on the
//! screen, it has to be the number.
//!
//! And it keeps two different bad outcomes apart: "I checked and it was wrong"
//! is not the same as "I could not check". The first means somebody may have
//! tampered with your download. The second usually means your internet hiccuped.

use std::process::ExitCode;
use std::sync::atomic::{AtomicU8, Ordering};

/// The level in force, set once at startup and read everywhere.
///
/// A process-wide value rather than a parameter threaded through every command:
/// there are around forty places that print, the level is the same for all of
/// them for the whole run, and a parameter that has to reach all forty is a
/// parameter that will one day not reach one of them.
static LEVEL: AtomicU8 = AtomicU8::new(Loudness::Normal as u8);

/// Set the level. Called once, from `main`, before anything is printed.
pub fn set_level(level: Loudness) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

/// The level in force.
pub fn level() -> Loudness {
    match LEVEL.load(Ordering::Relaxed) {
        0 => Loudness::Nothing,
        1 => Loudness::Minimal,
        3 => Loudness::Everything,
        // Anything unrecognised reads as the default rather than as silence.
        // Of the two ways to be wrong here, printing too much is the one that
        // cannot hide an answer.
        _ => Loudness::Normal,
    }
}

/// What happened, as a number a script can read.
///
/// These are **stable**. A number that changes meaning between versions breaks
/// every script that trusted it, silently, in the direction of "it worked".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    /// Everything asked for was done and every check passed.
    Success = 0,

    /// The command line could not be understood. Nothing was attempted.
    ///
    /// One rather than two, because this is the one outcome that is certainly
    /// the caller's own doing and never a statement about a release.
    Usage = 1,

    /// **A check ran and failed.** A signature did not verify, or a hash did
    /// not match the one that was published.
    ///
    /// This is the number that means *do not run what you downloaded*.
    Refused = 2,

    /// A check could not be completed: a download failed, a file was missing,
    /// a needed tool was not there.
    ///
    /// Nothing was proven, and nothing was disproven.
    Incomplete = 3,

    /// A build was attempted and the compiler stopped.
    ///
    /// Separate from [`Status::Incomplete`] because it is actionable in a
    /// different place: the output of the build says what is wrong, and no
    /// amount of retrying changes it.
    BuildFailed = 4,

    /// A build finished, and what came out does **not** match the published
    /// build for this platform.
    ///
    /// Deliberately not [`Status::Refused`]. A reproducibility difference is a
    /// finding to look into and publish -- most causes are boring, and calling
    /// it tampering would be a claim this program cannot support.
    NotReproducible = 5,

    /// Build dependencies are missing and were not installed, because nobody
    /// said yes.
    ///
    /// A refusal by the *operator*, not by this program, and it exits non-zero
    /// so an unattended run does not look like it succeeded.
    DependenciesMissing = 6,

    /// Files were built or verified, and putting them in place failed.
    ///
    /// What was verified is still verified; only the copy did not happen.
    InstallFailed = 7,
}

impl Status {
    /// The number this outcome exits with.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// One line, for the table in `--help`.
    pub fn meaning(self) -> &'static str {
        match self {
            Self::Success => "everything asked for was done and every check passed",
            Self::Usage => "the command line could not be understood; nothing was attempted",
            Self::Refused => "a check ran and FAILED -- do not run what you downloaded",
            Self::Incomplete => "a check could not be completed; nothing was proven either way",
            Self::BuildFailed => "the build was attempted and the compiler stopped",
            Self::NotReproducible => "the build here does not match the published build",
            Self::DependenciesMissing => "build dependencies are missing and were not installed",
            Self::InstallFailed => "the check passed; putting the files in place did not",
        }
    }

    /// Every status, for printing the table and for testing it is complete.
    pub const ALL: &'static [Status] = &[
        Status::Success,
        Status::Usage,
        Status::Refused,
        Status::Incomplete,
        Status::BuildFailed,
        Status::NotReproducible,
        Status::DependenciesMissing,
        Status::InstallFailed,
    ];

    /// The table, as `--help` prints it.
    pub fn table() -> String {
        let mut out = String::from("EXIT STATUS\n");
        for status in Self::ALL {
            out.push_str(&format!("  {}   {}\n", status.code(), status.meaning()));
        }
        out
    }
}

impl From<Status> for ExitCode {
    fn from(status: Status) -> Self {
        ExitCode::from(status.code())
    }
}

/// How much to print.
///
/// Ordered, so a message can ask "am I loud enough to be said".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Loudness {
    /// Not a word. The exit status is the whole answer, which is why
    /// [`Status`] exists and is documented.
    Nothing,
    /// The verdict and nothing else: one line at the end, and refusals.
    Minimal,
    /// Each check as it passes, and every refusal in full.
    #[default]
    Normal,
    /// Everything: the commands run, the paths used, the hashes compared.
    Everything,
}

impl Loudness {
    /// The flag that selects this level.
    pub fn flag(self) -> &'static str {
        match self {
            Self::Nothing => "--quiet",
            Self::Minimal => "--brief",
            Self::Normal => "(default)",
            Self::Everything => "--verbose",
        }
    }

    /// What this level shows.
    pub fn describes(self) -> &'static str {
        match self {
            Self::Nothing => "nothing at all -- read the exit status",
            Self::Minimal => "the verdict, and refusals",
            Self::Normal => "each check as it passes, and refusals in full",
            Self::Everything => "the above, plus every command, path and hash",
        }
    }

    /// Read the level out of the arguments, removing the flags that set it.
    ///
    /// The **loudest** flag given wins rather than the last one. `--quiet
    /// --verbose` is a contradiction, and of the two possible readings, the one
    /// that prints more is the one that cannot hide an answer.
    pub fn take_from(args: &mut Vec<String>) -> Loudness {
        let mut chosen: Option<Loudness> = None;
        args.retain(|arg| {
            let level = match arg.as_str() {
                "--quiet" | "-q" => Loudness::Nothing,
                "--brief" => Loudness::Minimal,
                "--normal" => Loudness::Normal,
                "--verbose" | "-v" => Loudness::Everything,
                _ => return true,
            };
            chosen = Some(match chosen {
                Some(already) => already.max(level),
                None => level,
            });
            false
        });
        chosen.unwrap_or_default()
    }

    /// The table, as `--help` prints it.
    pub fn table() -> String {
        let mut out = String::from("HOW MUCH IT SAYS\n");
        for level in [
            Loudness::Nothing,
            Loudness::Minimal,
            Loudness::Normal,
            Loudness::Everything,
        ] {
            out.push_str(&format!("  {:<12} {}\n", level.flag(), level.describes()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers are stable. Written out rather than derived, because the
    /// point of the test is to fail when somebody reorders the enum.
    #[test]
    fn every_status_has_the_number_it_has_always_had() {
        assert_eq!(Status::Success.code(), 0);
        assert_eq!(Status::Usage.code(), 1);
        assert_eq!(Status::Refused.code(), 2);
        assert_eq!(Status::Incomplete.code(), 3);
        assert_eq!(Status::BuildFailed.code(), 4);
        assert_eq!(Status::NotReproducible.code(), 5);
        assert_eq!(Status::DependenciesMissing.code(), 6);
        assert_eq!(Status::InstallFailed.code(), 7);
    }

    /// No two outcomes share a number, and every one is in `ALL`.
    #[test]
    fn the_statuses_are_distinct_and_the_list_is_complete() {
        let mut codes: Vec<u8> = Status::ALL.iter().map(|s| s.code()).collect();
        let count = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), count, "two outcomes share a number");
        assert_eq!(codes, (0..count as u8).collect::<Vec<_>>(), "no gaps");
    }

    /// Only success is zero. This is the whole reason the module exists: at
    /// `--quiet` the number is the entire answer.
    #[test]
    fn nothing_but_success_returns_zero() {
        for status in Status::ALL {
            assert_eq!(
                status.code() == 0,
                *status == Status::Success,
                "{status:?} returns {}",
                status.code()
            );
        }
    }

    /// Checked-and-wrong and could-not-check must have different numbers, or
    /// a script cannot tell tampering from a network hiccup.
    #[test]
    fn a_failed_check_and_an_unfinished_one_are_different_numbers() {
        assert_ne!(Status::Refused.code(), Status::Incomplete.code());
        assert!(Status::Refused.meaning().contains("do not run"));
        assert!(Status::Incomplete.meaning().contains("nothing was proven"));
    }

    /// A difference between this build and the published one is a finding, not
    /// an accusation, and it gets its own number to say so.
    #[test]
    fn a_reproducibility_difference_is_not_reported_as_tampering() {
        assert_ne!(Status::NotReproducible.code(), Status::Refused.code());
        assert!(!Status::NotReproducible.meaning().contains("do not run"));
    }

    #[test]
    fn the_default_is_normal() {
        assert_eq!(Loudness::default(), Loudness::Normal);
        let mut args = vec!["auto".to_string()];
        assert_eq!(Loudness::take_from(&mut args), Loudness::Normal);
        assert_eq!(args, vec!["auto".to_string()]);
    }

    #[test]
    fn each_flag_selects_its_level_and_is_removed() {
        for (flag, want) in [
            ("--quiet", Loudness::Nothing),
            ("-q", Loudness::Nothing),
            ("--brief", Loudness::Minimal),
            ("--normal", Loudness::Normal),
            ("--verbose", Loudness::Everything),
            ("-v", Loudness::Everything),
        ] {
            let mut args = vec!["file".to_string(), flag.to_string(), "x".to_string()];
            assert_eq!(Loudness::take_from(&mut args), want, "{flag}");
            assert_eq!(args, vec!["file".to_string(), "x".to_string()], "{flag}");
        }
    }

    /// Contradictory flags resolve towards saying more. Of the two readings,
    /// only one can hide an answer, so it is not the one taken.
    #[test]
    fn the_loudest_flag_wins_rather_than_the_last() {
        let mut args = vec!["--quiet".to_string(), "--verbose".to_string()];
        assert_eq!(Loudness::take_from(&mut args), Loudness::Everything);

        let mut args = vec!["--verbose".to_string(), "--quiet".to_string()];
        assert_eq!(Loudness::take_from(&mut args), Loudness::Everything);
        assert!(args.is_empty());
    }

    /// The levels are ordered, because every message asks "am I loud enough".
    #[test]
    fn the_levels_are_ordered_from_silent_to_everything() {
        assert!(Loudness::Nothing < Loudness::Minimal);
        assert!(Loudness::Minimal < Loudness::Normal);
        assert!(Loudness::Normal < Loudness::Everything);
    }

    /// Both tables have to be printable and complete: they are the whole
    /// documentation of the quiet mode.
    #[test]
    fn the_help_tables_name_every_level_and_every_status() {
        let statuses = Status::table();
        for status in Status::ALL {
            assert!(
                statuses.contains(status.meaning()),
                "{status:?} missing from the table"
            );
            assert!(statuses.contains(&format!("  {}   ", status.code())));
        }

        let levels = Loudness::table();
        for level in [
            Loudness::Nothing,
            Loudness::Minimal,
            Loudness::Normal,
            Loudness::Everything,
        ] {
            assert!(levels.contains(level.describes()), "{level:?}");
        }
        assert!(levels.contains("--quiet"), "{levels}");
        assert!(
            levels.contains("read the exit status"),
            "the quiet level has to say where the answer went: {levels}"
        );
    }
}
