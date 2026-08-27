// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice capture` -- which screen recorders are running, and which of them
//! you have said you meant to run.
//!
//! The command-line front end to [`veilvoice_capture`]. That crate holds the
//! table, the allowlist and the honest account of the three things it cannot
//! do; this file decides where the allowlist lives and prints the result.
//!
//! # Where the allowlist lives
//!
//! ```text
//! <config>/veilvoice/capture/allow.txt
//! ```
//!
//! Beside everything else this program keeps. Plain text, nothing secret in it:
//! an allowlist is a note to yourself about which notifications you have
//! already read.
//!
//! # The exit code
//!
//! `veilvoice capture check` exits non-zero when something **not allowed** is
//! running, so it can be a step in a script that refuses to start recording
//! something sensitive while a recorder is open. Allowing a program is
//! precisely how you tell that script you meant it.
//!
//! It exits zero when the listing itself failed, and says so on the way past.
//! A check that cannot see is not a check that passed, but neither is it a
//! reason to fail a script — the difference is in the words, and the words are
//! printed.
//!
//! # In plain words
//!
//! Tells you which screen recorders are running, and lets you say you meant to
//! start one so it stops being mentioned.
//!
//! It cannot tell whether a program is actually recording, only that it is open,
//! and it says so. A meeting application being open is not somebody watching your
//! screen.

use crate::theme::{colour, err, field, heading, ok, paint, warn};
use std::path::PathBuf;
use veilvoice_capture::{programs, Allowlist, Report};

/// Where the allowlist is kept.
///
/// Derived from the app lock's location rather than resolved again, so there is
/// one answer to "where does VeilVoice keep things".
pub fn capture_dir() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name("").join("capture"))
}

fn allow_path() -> Result<PathBuf, String> {
    Ok(capture_dir()
        .ok_or_else(|| {
            "this platform did not say where to keep configuration (no APPDATA, \
             XDG_CONFIG_HOME or HOME), so there is nowhere to keep an allowlist"
                .to_string()
        })?
        .join("allow.txt"))
}

fn load() -> Result<Allowlist, String> {
    let path = allow_path()?;
    Allowlist::load(&path).map_err(|error| format!("{}: {error}", path.display()))
}

fn save(allowlist: &Allowlist) -> Result<(), String> {
    let path = allow_path()?;
    allowlist
        .save(&path)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

/// What is running, what is allowed, and what this cannot see.
pub fn status() -> Result<(), String> {
    println!("{}", heading("Screen capture"));
    let allowlist = load()?;
    let report = Report::take(&allowlist);

    for problem in &report.problems {
        println!("{}", err(&format!("could not look properly: {problem}")));
    }

    if report.is_empty() {
        println!(
            "{}",
            field("running now", "none of the programs this build knows")
        );
    } else {
        println!();
        for sighting in report.all() {
            let line = sighting.describe();
            if sighting.allowed {
                println!("{}", paint(colour::MUTED, &format!("  {line}")));
            } else {
                println!("{}", warn(&line));
            }
        }
        println!();
    }

    println!("{}", field("allowed", &allowlist.len().to_string()));
    for key in allowlist.keys() {
        let name = programs::by_key(key)
            .map(|program| program.name)
            .unwrap_or(key);
        println!("{}", field("  ", &format!("{key} -- {name}")));
    }

    println!();
    println!("{}", paint(colour::YELLOW, "WHAT THIS IS WORTH"));
    for line in crate::sentry::wrap(veilvoice_capture::SCOPE, 72) {
        println!("  {line}");
    }
    println!();
    println!("  veilvoice capture allow <KEY>    stop notifying about one");
    println!("  veilvoice capture deny <KEY>     start again");
    println!("  veilvoice capture list           every program this build knows");
    Ok(())
}

/// Every program in the table, whether it is running or not.
/// Where to point a calling program so your voice goes through VeilVoice.
///
/// Prints the route, which program to change and where, and -- as plainly as
/// the rest -- the two things it does not do.
pub fn calls() -> Result<(), String> {
    use veilvoice_capture::comms;

    println!("{}", heading("Talking through VeilVoice"));
    println!();
    println!("  your microphone  ->  veilvoice live  ->  a virtual audio cable");
    println!("                                                   |");
    println!("                                                   v");
    println!("                                       the calling program, with");
    println!("                                       the cable as its microphone");
    println!();

    // The cable this machine actually has, named, so the instructions can be
    // followed rather than translated.
    #[cfg(feature = "live")]
    let cable = veilvoice_audio::devices::find_virtual_cable().map(|device| device.name);
    #[cfg(not(feature = "live"))]
    let cable: Option<String> = None;

    match &cable {
        Some(name) => println!("{}", field("this machine's cable", name)),
        None => {
            println!(
                "{}",
                warn("no virtual audio cable was found on this machine")
            );
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  `veilvoice companions` will install VB-CABLE, BlackHole or PipeWire."
                )
            );
        }
    }

    let (found, problems) = comms::running();
    println!();
    println!("{}", heading("Programs found running"));
    if found.is_empty() {
        println!(
            "{}",
            paint(colour::MUTED, "  none of the ones this build knows")
        );
    }
    for comm in &found {
        println!("{}", field(comm.name, comm.where_to_look));
    }
    for problem in &problems {
        println!("{}", warn(problem));
    }

    println!();
    println!(
        "{}",
        heading("Every program this build knows where to look in")
    );
    for comm in comms::COMMS {
        println!("{}", field(comm.name, comm.where_to_look));
    }
    println!();
    for line in crate::sentry::wrap(comms::ANY_PROGRAM, 72) {
        println!("  {line}");
    }

    println!();
    println!("{}", paint(colour::YELLOW, "WHAT THIS DOES NOT DO"));
    for line in crate::sentry::wrap(comms::INCOMING, 72) {
        println!("  {line}");
    }
    println!();
    for line in crate::sentry::wrap(comms::NO_INTERCEPTION, 72) {
        println!("  {line}");
    }
    Ok(())
}

pub fn list() -> Result<(), String> {
    println!("{}", heading("Programs this build knows"));
    println!("  Anything not on this list is not reported. The list will never be");
    println!("  complete, and an empty report is not evidence that nothing is");
    println!("  recording.");
    println!();
    for program in programs::ALL {
        println!("{}", paint(colour::BLUE, program.name));
        println!("{}", field("key", program.key));
        println!("{}", field("what", program.what));
        println!(
            "{}",
            field(
                "kind",
                match program.purpose {
                    programs::Purpose::Recorder => "recording the screen is what it does",
                    programs::Purpose::Capable =>
                        "can share a screen, which is not the same as doing it",
                }
            )
        );
        if program.processes.is_empty() {
            println!(
                "{}",
                field("matches", "nothing, on purpose -- see the reference page")
            );
        }
        println!();
    }
    Ok(())
}

/// Stop notifying about one program.
pub fn allow(key: &str) -> Result<(), String> {
    println!("{}", heading("Allow a recorder"));
    let mut allowlist = load()?;
    allowlist.allow(key).map_err(|error| error.to_string())?;
    save(&allowlist)?;
    let name = programs::by_key(key)
        .map(|program| program.name)
        .unwrap_or(key);
    println!("{}", ok(&format!("{name} will not raise a notification")));
    println!();
    println!("  It still appears in `veilvoice capture status`. Allowed means muted,");
    println!("  not hidden -- a setting that removed it from the interface entirely");
    println!("  would be a setting for lying to yourself.");
    Ok(())
}

/// Start notifying about one program again.
pub fn deny(key: &str) -> Result<(), String> {
    println!("{}", heading("Stop allowing a recorder"));
    let mut allowlist = load()?;
    if !allowlist.allows(key) {
        println!("{}", warn(&format!("{key} was not allowed")));
        return Ok(());
    }
    allowlist.deny(key);
    save(&allowlist)?;
    println!("{}", ok(&format!("{key} will raise a notification again")));
    Ok(())
}

/// Look now, and let the exit code answer.
///
/// Returns `true` when something not allowed is running.
pub fn check() -> Result<bool, String> {
    let allowlist = load()?;
    let report = Report::take(&allowlist);
    for problem in &report.problems {
        // Printed, and deliberately not turned into a failure. A check that
        // could not see is not a check that passed, and it is not a reason to
        // fail somebody's script either. The sentence is the honest part.
        println!("{}", err(&format!("could not look properly: {problem}")));
    }
    let unallowed = report.worth_saying();
    if unallowed.is_empty() {
        println!("{}", ok("nothing unallowed is running"));
        return Ok(false);
    }
    for sighting in &unallowed {
        println!("{}", warn(&sighting.describe()));
    }
    println!();
    println!("  If you meant to run that, `veilvoice capture allow <KEY>` stops");
    println!("  this asking again.");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_capture_directory_sits_beside_the_app_lock() {
        let Some(dir) = capture_dir() else {
            return; // a platform with nowhere to keep configuration
        };
        assert!(dir.ends_with("capture"), "{}", dir.display());
        let lock = veilvoice_crypto::lock::default_path().unwrap();
        assert_eq!(dir.parent(), lock.parent());
    }

    /// Every state directory this program keeps must be its own, or two
    /// features share a folder and each other's filenames.
    #[test]
    fn every_state_directory_is_distinct() {
        let dirs: Vec<PathBuf> = [
            capture_dir(),
            crate::sentry::state_dir(),
            crate::policy::policy_dir(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let mut seen = dirs.clone();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), dirs.len(), "two features share a directory");
    }

    /// Both listings must render without touching the allowlist on disk.
    #[test]
    fn the_program_list_prints_every_entry() {
        list().expect("listing the table needs no configuration directory");
    }
}
