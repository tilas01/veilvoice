// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice update`: fetch the new release, check it, and put it in place.
//!
//! **Roadmap item 179.** Everything that decides anything is in
//! [`veilvoice_verify::update`], and this file is the terminal in front of it:
//! it prints what is happening, asks for the app lock when the machine has one,
//! and turns the result into an exit status. Nothing here fetches, checks or
//! replaces, so the window doing the same thing later does not get a second
//! implementation of any of it.
//!
//! # Why it asks before it starts, and only then
//!
//! The passphrase is collected once, before anything is downloaded, because
//! [`veilvoice_verify::update::perform`] refuses a locked machine without one
//! and there is no sense downloading eighty megabytes to be told so. It is
//! never collected where an app lock is not set: a program that asks for a
//! passphrase it has no use for is teaching people to type one at whatever
//! asks.
//!
//! # `--check` is the old behaviour, kept
//!
//! `veilvoice update --check` reports the version and does nothing else, which
//! is exactly what the desktop button did before this existed. Somebody who
//! wants to know and then decide for themselves is not being made to update,
//! and the account of what a version number on a page is worth is printed with
//! it.

use std::io::IsTerminal;

use veilvoice_verify::update::{self, Error, Record, Step};

use crate::theme::{err, field, heading, ok, warn};

/// Run the command. `check_only` reports and stops.
pub fn run(check_only: bool, assume_yes: bool) -> Result<(), String> {
    let current = env!("CARGO_PKG_VERSION");

    println!("{}", heading("Update"));
    println!("{}", field("Installed", current));

    let offer = match update::offered(current) {
        Err(Error::NoPlatform) => {
            println!();
            println!("{}", warn(&Error::NoPlatform.to_string()));
            return Ok(());
        }
        Err(error) => return Err(error.to_string()),
        Ok(None) => {
            println!();
            println!("{}", ok("this is the newest release published"));
            println!();
            println!("  {}", veilvoice_setup::update::SCOPE);
            return Ok(());
        }
        Ok(Some(offer)) => offer,
    };

    println!("{}", field("Published", &offer.tag));
    println!("{}", field("For this platform", &offer.archive));

    if check_only {
        println!();
        println!("  Run `veilvoice update` to fetch it, check it and install it.");
        println!();
        println!("  {}", veilvoice_setup::update::SCOPE);
        return Ok(());
    }

    // Said before anything is fetched, because it is the reason somebody should
    // be willing to let a program replace itself at all.
    println!();
    println!("  Everything downloaded is checked against the signature made with");
    println!("  the key built into this program, before the archive is opened and");
    println!("  long before anything of yours is replaced. If any of that fails,");
    println!("  nothing is replaced and you are told what failed.");

    if !assume_yes && !confirm(&offer)? {
        println!();
        println!("  Nothing was downloaded.");
        return Ok(());
    }

    // Before the download, for the reason in the module note.
    let secret = if update::lock_is_set() {
        println!();
        println!("  This machine has an app lock. The record of what VeilVoice's own");
        println!("  files should look like is sealed with that passphrase and has to");
        println!("  be rewritten by the update, so it is needed now.");
        Some(crate::atrest::prompt_secret("App lock passphrase: ")?)
    } else {
        None
    };

    println!();
    let done = update::perform(&offer, secret.as_ref().map(|s| s.expose()), &mut |step| {
        println!("{}", ok(&describe(&step)))
    })
    .map_err(|error| error.to_string())?;

    println!();
    for name in &done.replaced {
        println!("{}", ok(&format!("replaced {name}")));
    }
    println!(
        "{}",
        field("Now running from", &done.into.display().to_string())
    );
    println!("{}", field("Version", &done.tag));

    match &done.record {
        Record::Sealed => println!(
            "{}",
            ok("the integrity record was re-taken and sealed with your app lock")
        ),
        Record::Plain => {
            println!("{}", ok("the integrity record was re-taken"));
            // Said rather than implied. A plain record catches a file changed by
            // accident and not one changed by somebody covering their tracks,
            // and somebody who has just let a program replace itself is owed
            // that distinction at the moment it is made.
            println!(
                "{}",
                warn(
                    "it is written in the clear, because there is no app lock on this \
                     machine. It will notice a file that changed by accident and not \
                     one changed by somebody who thought to rewrite the record too."
                )
            );
        }
        Record::NotTaken(why) => println!(
            "{}",
            warn(&format!(
                "the update succeeded and the integrity record was NOT re-taken: {why}. \
                 The next check will report these new files as changed, because to it \
                 they are. Run `veilvoice guard init` to take a fresh one."
            ))
        ),
    }

    for stale in &done.left_behind {
        println!(
            "{}",
            warn(&format!(
                "{} could not be removed and is still there. It is the copy you were \
                 running, which this system will not delete while it runs. Remove it \
                 when convenient.",
                stale.display()
            ))
        );
    }

    println!();
    println!("  Start VeilVoice again to run {}.", done.tag);
    Ok(())
}

/// One line per step, in the present participle, because it is printed as the
/// step begins rather than after it.
fn describe(step: &Step) -> String {
    match step {
        Step::Fetching(name) => format!("fetching {name}"),
        Step::Checking => "checking the signature, the hash list and every file inside".to_string(),
        Step::Opening => "opening the archive, and hashing what came out of it".to_string(),
        Step::Replacing => "putting the new files in place".to_string(),
        Step::Recording => "re-taking the integrity record".to_string(),
    }
}

/// Ask before replacing the program the person is running.
///
/// An update is reversible in the sense that the previous release can be
/// downloaded again, and it is not reversible in the sense that matters at the
/// moment it happens: the binary somebody is using is gone. So it is asked
/// rather than assumed, and `--yes` is the written form of the same answer.
fn confirm(offer: &update::Offer) -> Result<bool, String> {
    if !std::io::stdin().is_terminal() {
        return Err(err(
            "this is not a terminal, so nothing can be confirmed here. Pass --yes if \
             you meant to update without being asked.",
        ));
    }
    println!();
    print!("  Replace {} with {}? [y/N] ", offer.current, offer.tag);
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return Ok(false);
    }
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every step has a line, and each reads as something in progress rather
    /// than something finished. A step printed as done before it has run is
    /// how a report ends up claiming an update that failed halfway.
    #[test]
    fn every_step_is_described_as_it_happens() {
        let steps = [
            Step::Fetching("SHA256SUMS".to_string()),
            Step::Checking,
            Step::Opening,
            Step::Replacing,
            Step::Recording,
        ];
        for step in steps {
            let line = describe(&step);
            assert!(!line.is_empty(), "{step:?} has no line");
            let first = line.split_whitespace().next().unwrap();
            assert!(
                first.ends_with("ing"),
                "{first:?} is not what is happening now: {line}"
            );
        }
    }

    /// This file must not do any of the work. Every decision belongs to
    /// `veilvoice_verify::update`, so the window gets the same one.
    #[test]
    fn the_terminal_does_not_fetch_check_or_replace_anything() {
        let source = include_str!("update.rs").replace("\r\n", "\n");
        let shipped = source.split("#[cfg(test)]").next().unwrap_or("");
        let body: String = shipped
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in [
            "fs::copy",
            "fs::rename",
            "fs::remove",
            "Command::new",
            "sha256",
            "verify_detached",
        ] {
            assert!(
                !body.contains(forbidden),
                "this file calls {forbidden:?}: the update is decided in one place and \
                 this is not it"
            );
        }
    }
}
