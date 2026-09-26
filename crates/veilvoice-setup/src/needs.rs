// SPDX-License-Identifier: GPL-3.0-or-later
//! What this machine is missing, asked once and at the right moment.
//!
//! **Roadmap item 163.** Three things need `ffmpeg`: `import`, `video` and the
//! Studio's render. Each of them found out at the moment it was used and
//! printed the install line. That is the right refusal at the wrong time.
//! Somebody learns their recording cannot be turned into a video **after they
//! have finished making it**, which is the one point in the process where the
//! answer is least useful and most annoying.
//!
//! So the question is asked in one place, before anything depends on it, and
//! the answer is one list rather than three separate discoveries.
//!
//! # What it asks, and what it does not
//!
//! Three things:
//!
//! * Is `ffmpeg` here? Three commands need it and none of them can do their
//!   last step without it.
//! * Is GnuPG here? Nothing in VeilVoice needs it, and that is the point: it is
//!   what lets somebody check a VeilVoice release with a program this project
//!   did not write, which is the only check that escapes the circularity of a
//!   download verifying itself.
//! * Is this copy installed, or running out of a folder?
//!
//! The third is here rather than in [`crate::install`] because it belongs to
//! the same question. Somebody running a portable copy who has not been told
//! that portable is a supported choice will assume something went wrong, and
//! somebody who meant to install and did not is a person whose next terminal
//! will not find `veilvoice`. Both are facts about this machine, wanted at the
//! same moment, and reporting them in one place is the whole of this module.
//!
//! **Nothing here changes anything, spawns anything, or reaches the network.**
//! Every probe is a `PATH` lookup or a file test. [`look`] is cheap enough to
//! call at a launch, which is the only reason it can be called at one.
//!
//! # A no is remembered
//!
//! Being asked the same question at every launch is how a prompt becomes
//! something people dismiss without reading, and this project has already
//! written that argument down about tamper alarms. So a decline is recorded,
//! per companion, in a file the reader can open and delete: see [`declined`].
//!
//! It is per companion rather than one switch. Somebody who does not want
//! `ffmpeg` has said nothing about GnuPG, and treating the two as one answer is
//! putting words in their mouth.
//!
//! A decline is **not** a claim that the software is absent or unwanted for
//! ever. [`look`] still reports what is there; what a decline suppresses is the
//! offer. `veilvoice companions` shows everything regardless, because somebody
//! who types the name of the thing is asking.
//!
//! # In plain words
//!
//! Checks once, when VeilVoice starts, whether the few optional things are here,
//! instead of finding out when you are halfway through something.
//!
//! If you say no to one of them, it remembers, and does not ask again. The
//! answer is kept in a small text file you can read and delete.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::companions::{self, Companion, Offer, Presence};

/// The file a decline is remembered in.
///
/// Read from `veilvoice-crypto`'s list of everything VeilVoice keeps between
/// runs rather than spelled again here, because that list is what the About
/// tab shows and what the reset names, and a file this module invented a name
/// for would be one the reader is never told about.
pub fn declined_file() -> &'static str {
    veilvoice_crypto::layout::entry(veilvoice_crypto::layout::Item::Companions).name
}

/// What is written at the top of that file, so it explains itself.
///
/// The file will be found by somebody who did not put it there, in a directory
/// they are looking through for another reason, and a list of bare words with
/// no heading is a thing people delete or worry about. Neither is wanted.
pub const PREAMBLE: &str = "\
# VeilVoice: optional software you have said you do not want to be offered.
#
# One name per line. Nothing here is a setting that changes what VeilVoice
# does; it only stops the offer being made again at every launch.
#
# Delete a line to be asked about that one again, or delete the file to be
# asked about all of them. Nothing else reads this.
";

/// Whether this copy is installed, and whether it is the one running.
///
/// Three states rather than a boolean, for the reason [`crate::install::Status`]
/// gives: "something is installed" and "you are running the installed copy" are
/// different facts, and a reader told the first when the second is false will
/// edit a portable folder and wonder why nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Running {
    /// This is the installed copy.
    Installed(PathBuf),
    /// An installed copy exists and this is not it.
    PortableBesideAnInstall {
        /// Where this copy is running from.
        here: PathBuf,
        /// Where the installed one is.
        installed: PathBuf,
    },
    /// Running out of a folder, with nothing installed. **Not a fault.**
    Portable(PathBuf),
    /// This system offers nowhere to install to, or the running program could
    /// not be located.
    Unknown(String),
}

impl Running {
    /// A line for a reader, which never calls portable a problem.
    pub fn describe(&self) -> String {
        match self {
            Running::Installed(path) => {
                format!("running the installed copy, from {}", path.display())
            }
            Running::PortableBesideAnInstall { here, installed } => format!(
                "running from {}, while an installed copy sits in {}. Changes to \
                 one are not changes to the other.",
                here.display(),
                installed.display()
            ),
            Running::Portable(here) => format!(
                "running from {}, with nothing installed. That is a supported way \
                 to use VeilVoice and nothing needs to be done about it.",
                here.display()
            ),
            Running::Unknown(why) => format!("could not tell: {why}"),
        }
    }

    /// Whether anything here is worth putting in front of somebody.
    ///
    /// False for a plain portable copy. Portable is the default posture of this
    /// project, and a launch notice saying so every time would be telling people
    /// off for using the program the way it is meant to be used.
    pub fn worth_saying(&self) -> bool {
        matches!(
            self,
            Running::PortableBesideAnInstall { .. } | Running::Unknown(_)
        )
    }
}

/// One thing this machine has not got, and what could be done about it.
#[derive(Debug, Clone)]
pub struct Absent {
    /// Which piece of software.
    pub companion: &'static Companion,
    /// What the probe actually found, kept so a caller can tell
    /// [`Presence::NotDetected`] from [`Presence::Unknown`] rather than being
    /// handed a boolean.
    pub presence: Presence,
    /// What could be done about it on this platform.
    pub offer: Offer,
}

/// What one look at this machine found.
#[derive(Debug, Clone)]
pub struct Check {
    /// The things that are not here and have not been declined, in the order
    /// [`WANTED`] gives.
    pub absent: Vec<Absent>,
    /// The things that are not here and **have** been declined. Kept apart
    /// rather than dropped, so `veilvoice companions` can still show them and
    /// a front end can offer to ask again.
    pub declined: Vec<Absent>,
    /// Whether this copy is installed.
    pub running: Running,
}

impl Check {
    /// Whether there is anything a reader should be shown at a launch.
    ///
    /// The whole point of the module is that this is usually false, and that
    /// when it is false nothing is printed at all. A launch notice that appears
    /// every time is a launch notice nobody reads.
    pub fn worth_showing(&self) -> bool {
        !self.absent.is_empty() || self.running.worth_saying()
    }
}

/// The companions a launch asks about, by key.
///
/// Two, and deliberately not [`companions::ALL`]. The audio routing driver and
/// the editor are things somebody reaches for when they want them; these two
/// are things whose absence stops work that has already been started, or stops
/// a check this project wants people to be able to make. Asking about all six
/// at a launch would be a list, and a list is something to close.
pub const WANTED: &[&str] = &["ffmpeg", "gnupg"];

/// Look at this machine. Changes nothing.
pub fn look() -> Check {
    let remembered = declined();
    let mut absent = Vec::new();
    let mut put_off = Vec::new();

    for key in WANTED {
        let Some(companion) = companions::by_key(key) else {
            continue;
        };
        let presence = companion.detect();
        if presence.is_present() {
            continue;
        }
        let offer = companion.offer();
        if matches!(offer, Offer::NotOnThisPlatform) {
            continue;
        }
        let found = Absent {
            companion,
            presence,
            offer,
        };
        if remembered.contains(*key) {
            put_off.push(found);
        } else {
            absent.push(found);
        }
    }

    Check {
        absent,
        declined: put_off,
        running: running(),
    }
}

/// Which copy of VeilVoice is running.
fn running() -> Running {
    let Some(here) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
    else {
        return Running::Unknown("this program's own location could not be read".to_string());
    };
    let Some(installed) = crate::install::bin_dir() else {
        return Running::Unknown(
            "this system offers no per-user program directory, so there is nothing \
             to compare against"
                .to_string(),
        );
    };
    if here == installed {
        return Running::Installed(here);
    }
    if installed.join(exe_name()).exists() {
        return Running::PortableBesideAnInstall { here, installed };
    }
    Running::Portable(here)
}

/// The command line's file name on this platform.
fn exe_name() -> &'static str {
    if cfg!(windows) {
        "veilvoice.exe"
    } else {
        "veilvoice"
    }
}

/// Where the declines are remembered, or `None` on a platform that names
/// nowhere.
///
/// Beside the app lock, the integrity record and everything else VeilVoice
/// keeps per user, through the one function that decides where that is. A
/// fourth copy of the environment-variable arithmetic is what this project has
/// a rule against.
pub fn path() -> Option<PathBuf> {
    veilvoice_crypto::layout::path(veilvoice_crypto::layout::Item::Companions)
}

/// The companions a decline has been recorded for.
///
/// An unreadable file yields an empty set rather than an error. The cost of
/// getting this wrong in that direction is one offer somebody has already
/// refused; the cost the other way is a machine where nothing is ever offered
/// because a file could not be opened, and no way to find out why.
pub fn declined() -> BTreeSet<String> {
    let Some(path) = path() else {
        return BTreeSet::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return BTreeSet::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Remember that this one is not wanted.
///
/// Idempotent, and it refuses a key no companion has, so a typo at a front end
/// cannot fill this file with words that will never match anything.
pub fn decline(key: &str) -> Result<(), String> {
    if companions::by_key(key).is_none() {
        return Err(format!("{key} is not a companion this knows about"));
    }
    let mut keys = declined();
    if !keys.insert(key.to_string()) {
        return Ok(());
    }
    write(&keys)
}

/// Ask about this one again. Unknown keys are not an error here: forgetting
/// something that was never remembered is the outcome wanted either way.
pub fn ask_again(key: &str) -> Result<(), String> {
    let mut keys = declined();
    if !keys.remove(key) {
        return Ok(());
    }
    write(&keys)
}

/// Ask about everything again, by removing the file rather than emptying it.
///
/// A file that exists and is empty and a file that is not there mean the same
/// thing to [`declined`], and the one that is not there is the one somebody
/// looking through the directory does not have to work out.
pub fn ask_again_about_everything() -> Result<(), String> {
    let Some(path) = path() else {
        return Ok(());
    };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("could not remove {}: {e}", path.display())),
    }
}

/// Write the set, preamble and all.
fn write(keys: &BTreeSet<String>) -> Result<(), String> {
    let Some(path) = path() else {
        return Err(
            "this platform names no configuration directory, so an answer cannot be \
             remembered"
                .to_string(),
        );
    };
    if keys.is_empty() {
        return ask_again_about_everything();
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not make {}: {e}", parent.display()))?;
    }
    let mut text = String::from(PREAMBLE);
    for key in keys {
        text.push_str(key);
        text.push('\n');
    }
    std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key a launch asks about is a companion that exists.
    ///
    /// A key with no companion behind it would be silently skipped by [`look`],
    /// so the list would shrink without anything saying so.
    #[test]
    fn every_wanted_companion_exists() {
        for key in WANTED {
            assert!(
                companions::by_key(key).is_some(),
                "{key} is asked about at launch and is not a companion"
            );
        }
    }

    /// The launch check asks about the two whose absence stops work, and not
    /// about the four that are conveniences.
    ///
    /// Written as an assertion because the tempting change is to pass
    /// `companions::ALL` here, which turns a short notice into a list of six,
    /// and a list of six into something people close without reading.
    #[test]
    fn a_launch_asks_about_fewer_things_than_the_companions_page_shows() {
        assert!(
            WANTED.len() < companions::ALL.len(),
            "a launch asks about everything, which is a list rather than a notice"
        );
        assert!(WANTED.contains(&"ffmpeg"), "the one three commands need");
        assert!(
            WANTED.contains(&"gnupg"),
            "the one that lets a release be checked by a program this project did \
             not write"
        );
    }

    /// Looking changes nothing and can be done twice.
    #[test]
    fn looking_is_free_of_side_effects() {
        let before = declined();
        let first = look();
        let second = look();
        assert_eq!(first.absent.len(), second.absent.len());
        assert_eq!(first.running, second.running);
        assert_eq!(before, declined(), "looking changed what had been declined");
    }

    /// A plain portable copy is not something to tell somebody about.
    ///
    /// Portable is this project's default posture and its documentation says so
    /// in several places. A launch notice that fires on it would be telling
    /// people off for using VeilVoice the way it is meant to be used.
    #[test]
    fn running_portable_is_not_worth_saying() {
        assert!(!Running::Portable(PathBuf::from("/somewhere")).worth_saying());
        assert!(!Running::Installed(PathBuf::from("/somewhere")).worth_saying());
        assert!(Running::PortableBesideAnInstall {
            here: PathBuf::from("/a"),
            installed: PathBuf::from("/b"),
        }
        .worth_saying());
        assert!(Running::Unknown("no idea".into()).worth_saying());
    }

    /// Nothing said about a portable copy reads as a fault.
    #[test]
    fn the_portable_line_does_not_call_it_a_problem() {
        let said = Running::Portable(PathBuf::from("/somewhere"))
            .describe()
            .to_lowercase();
        for blame in [
            "error",
            "warning",
            "should",
            "must",
            "failed",
            "not installed",
        ] {
            assert!(
                !said.contains(blame),
                "portable is described as a fault: {said}"
            );
        }
        assert!(said.contains("supported"), "{said}");
    }

    /// A launch says nothing when there is nothing to say.
    ///
    /// This is the whole reason the module can be called at a launch at all. A
    /// notice that appears every time is a notice people learn to close before
    /// reading, and this project has already written that argument down about
    /// tamper alarms. The common case on a machine with `ffmpeg` and GnuPG
    /// installed is silence.
    #[test]
    fn a_launch_with_nothing_missing_shows_nothing() {
        let quiet = Check {
            absent: Vec::new(),
            declined: Vec::new(),
            running: Running::Portable(PathBuf::from("/somewhere")),
        };
        assert!(!quiet.worth_showing());

        // A decline on its own is still silence: the person has answered.
        let answered = Check {
            absent: Vec::new(),
            declined: vec![Absent {
                companion: companions::by_key("ffmpeg").expect("ffmpeg is a companion"),
                presence: Presence::NotDetected,
                offer: Offer::NoKnownRoute("nothing here".into()),
            }],
            running: Running::Installed(PathBuf::from("/somewhere")),
        };
        assert!(
            !answered.worth_showing(),
            "something declined is being offered again, which is the thing this \
             exists to stop"
        );

        // Something missing and unanswered is worth one line.
        let missing = Check {
            absent: vec![Absent {
                companion: companions::by_key("ffmpeg").expect("ffmpeg is a companion"),
                presence: Presence::NotDetected,
                offer: Offer::NoKnownRoute("nothing here".into()),
            }],
            declined: Vec::new(),
            running: Running::Installed(PathBuf::from("/somewhere")),
        };
        assert!(missing.worth_showing());

        // And so is a portable copy sitting beside an installed one, because
        // editing one of those and expecting the other to change is a real way
        // to lose an afternoon.
        let confusing = Check {
            absent: Vec::new(),
            declined: Vec::new(),
            running: Running::PortableBesideAnInstall {
                here: PathBuf::from("/a"),
                installed: PathBuf::from("/b"),
            },
        };
        assert!(confusing.worth_showing());
    }

    /// A key no companion has is refused rather than written.
    #[test]
    fn a_decline_for_something_that_does_not_exist_is_refused() {
        let error = decline("not-a-companion").expect_err("it must be refused");
        assert!(error.contains("not a companion"), "{error}");
    }

    /// The file says what it is, to somebody who did not put it there.
    #[test]
    fn the_remembered_file_explains_itself() {
        assert!(PREAMBLE.starts_with('#'), "every line of it is a comment");
        for line in PREAMBLE.lines() {
            assert!(
                line.is_empty() || line.starts_with('#'),
                "a line that is not a comment would be read back as a key: {line}"
            );
        }
        let said = PREAMBLE.to_lowercase();
        assert!(said.contains("delete"), "it must say how to undo it");
        assert!(
            said.contains("only stops the offer"),
            "it must say what it does not do"
        );
    }

    /// The preamble is skipped when the file is read back.
    ///
    /// Written against the parsing rather than the disk: a test that wrote to
    /// the real configuration directory would change the machine it runs on.
    #[test]
    fn the_preamble_is_not_read_back_as_a_list_of_keys() {
        let text = format!("{PREAMBLE}ffmpeg\n\n  gnupg  \n");
        let keys: BTreeSet<String> = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_string)
            .collect();
        assert_eq!(
            keys,
            ["ffmpeg", "gnupg"]
                .into_iter()
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        );
    }

    /// The record sits with everything else VeilVoice keeps per user.
    #[test]
    fn the_record_sits_beside_the_app_lock() {
        let (Some(record), Some(lock)) = (path(), veilvoice_crypto::lock::default_path()) else {
            return; // no configuration directory on this machine
        };
        assert_eq!(record.parent(), lock.parent());
        assert_eq!(record.file_name().unwrap(), declined_file());
    }
}
