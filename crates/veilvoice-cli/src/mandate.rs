// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice mandate` -- the two things VeilVoice insists on, and how to stop.
//!
//! The command-line front end to [`veilvoice_policy::Mandate`]. That module
//! holds the data and the history; this file decides what is printed, and makes
//! relaxing a requirement a deliberate act rather than a flag somebody typed
//! once.
//!
//! # This is the opposite tool to `veilvoice policy`
//!
//! A sealed policy can only ever make VeilVoice **stricter**, and is meant for
//! somebody setting rules for somebody else. The mandate is your own baseline
//! for your own machine, and it is the only thing here that can be **relaxed**.
//!
//! The two compose in one direction only: the effective requirement is the
//! mandate **or** whatever the sealed policy adds. So relaxing the mandate
//! cannot switch off something an administrator has fixed, and `status` says so
//! when that is what is happening, rather than reporting "not required" at
//! somebody who will then find it is still required.
//!
//! # Why relaxing asks twice
//!
//! Turning off encryption at rest means every recording VeilVoice writes from
//! then on is a plain file of everything that was said. De-identification took
//! the voiceprint out; it did not take the words out. That is a real choice
//! somebody may have real reasons to make, so it is offered, but it is not
//! offered casually, and it is written down with the date.
//!
//! # In plain words
//!
//! VeilVoice asks for a password for itself and encrypts your recordings unless
//! you tell it not to. This is where you tell it not to, and where you can see
//! when you did.

use crate::theme::{colour, field, heading, ok, paint, warn};
use std::path::PathBuf;
use veilvoice_policy::{Field as MField, Mandate};
use veilvoice_policy::{Policy, Requirement};

/// Where the mandate file lives.
fn path() -> Result<PathBuf, String> {
    veilvoice_policy::mandate_path().ok_or_else(|| {
        "this platform did not say where to keep configuration (no APPDATA, \
         XDG_CONFIG_HOME or HOME), so there is nowhere to keep a mandate"
            .to_string()
    })
}

fn load() -> Result<(PathBuf, Mandate), String> {
    let path = path()?;
    let mandate = Mandate::load(&path)?;
    Ok((path, mandate))
}

/// Whether the sealed policy independently fixes this requirement on.
///
/// Read so that `status` can tell the difference between "you are insisting on
/// this" and "you stopped insisting, and it is required anyway". Reporting the
/// second as though it were "not required" would be the more dangerous error in
/// the opposite direction: a user relaxing a requirement and being told it is
/// off when it is not.
fn sealed_also_requires(f: MField) -> bool {
    let Some(dir) = crate::policy::policy_dir() else {
        return false;
    };
    let requirement = match f {
        MField::AppLock => Requirement::AppLock,
        MField::Encryption => Requirement::EncryptRecordings,
    };
    matches!(Policy::load(&dir), Ok(Some(p)) if p.requires(&requirement))
}

fn describe(f: MField) -> &'static str {
    match f {
        MField::AppLock => "a password for VeilVoice itself, asked for at launch",
        MField::Encryption => "every recording encrypted where it is stored",
    }
}

/// What is required now, and how it got that way.
pub fn status() -> Result<(), String> {
    println!("{}", heading("What VeilVoice insists on"));
    let (path, mandate) = load()?;

    for f in [MField::AppLock, MField::Encryption] {
        let yours = mandate.requires(f);
        let sealed = sealed_also_requires(f);
        let value = match (yours, sealed) {
            (true, _) => ok("required"),
            (false, true) => warn("required by the sealed policy, not by you"),
            (false, false) => warn("not required"),
        };
        println!("{}", field(f.key(), &value));
        println!("    {}", describe(f));
    }

    println!();
    if mandate.is_default() {
        println!("{}", field("state", "the default: both required"));
        println!("  Nothing here has been turned off. To stop insisting on one:");
        println!();
        println!("    veilvoice mandate relax --encryption");
        println!();
    } else {
        println!("{}", field("state", "relaxed from the default"));
        println!("  To go back to insisting on both:");
        println!();
        println!("    veilvoice mandate reset");
        println!();
    }

    println!("{}", field("file", &path.display().to_string()));
    println!("{}", field("changes", &mandate.history().len().to_string()));
    if !mandate.history().is_empty() {
        println!("  `veilvoice mandate history` lists them.");
    }
    Ok(())
}

/// The log of every change, oldest first.
pub fn history() -> Result<(), String> {
    println!("{}", heading("How the requirements got this way"));
    let (path, mandate) = load()?;

    if mandate.history().is_empty() {
        println!("{}", field("changes", "none"));
        println!();
        println!("  Both requirements are as they came. Nothing has been turned off");
        println!("  or back on, so there is nothing to show.");
        return Ok(());
    }

    println!();
    for change in mandate.history() {
        let line = change.describe();
        let colour = if change.to {
            colour::GREEN
        } else {
            colour::YELLOW
        };
        println!("  {}", paint(colour, &line));
    }
    println!();
    println!("{}", field("file", &path.display().to_string()));
    Ok(())
}

/// Stop insisting on one or both requirements.
pub fn relax(app_lock: bool, encryption: bool, yes: bool) -> Result<(), String> {
    change(app_lock, encryption, false, yes)
}

/// Insist on one or both requirements again.
pub fn insist(app_lock: bool, encryption: bool) -> Result<(), String> {
    change(app_lock, encryption, true, true)
}

fn wanted(app_lock: bool, encryption: bool) -> Result<Vec<MField>, String> {
    let mut fields = Vec::new();
    if app_lock {
        fields.push(MField::AppLock);
    }
    if encryption {
        fields.push(MField::Encryption);
    }
    if fields.is_empty() {
        return Err("name at least one of --app-lock or --encryption".to_string());
    }
    Ok(fields)
}

fn change(app_lock: bool, encryption: bool, to: bool, yes: bool) -> Result<(), String> {
    let fields = wanted(app_lock, encryption)?;
    println!(
        "{}",
        heading(if to {
            "Insist on this again"
        } else {
            "Stop insisting"
        })
    );
    let (path, mut mandate) = load()?;

    // Relaxing is the direction that removes a protection, so it is the
    // direction that has to be confirmed. Insisting only ever adds one back and
    // needs no ceremony.
    if !to && !yes {
        println!();
        for f in &fields {
            println!("  Turning off: {}", describe(*f));
            if *f == MField::Encryption {
                println!();
                println!("  Recordings written after this are plain files. VeilVoice");
                println!("  removes the voiceprint from a recording; it does not remove");
                println!("  the words. Anybody who can read the folder can read what was");
                println!("  said.");
            }
            if *f == MField::AppLock {
                println!();
                println!("  VeilVoice will open without asking for anything. Whatever is");
                println!("  in it is in it, for whoever opens the window.");
            }
            if sealed_also_requires(*f) {
                println!();
                println!(
                    "{}",
                    warn("  a sealed policy also requires this, so it stays on either way")
                );
            }
            println!();
        }
        println!("  The change is written down with today's date, and");
        println!("  `veilvoice mandate history` will show it.");
        println!();
        println!("  Re-run with --yes to proceed.");
        return Ok(());
    }

    let mut changed = false;
    for f in &fields {
        if mandate.set(*f, to) {
            changed = true;
            println!(
                "{}",
                ok(&format!(
                    "{} {}",
                    if to {
                        "insisting on"
                    } else {
                        "no longer insisting on"
                    },
                    f.key()
                ))
            );
        } else {
            println!(
                "{}",
                warn(&format!(
                    "{} was already that way; nothing written",
                    f.key()
                ))
            );
        }
    }

    if !changed {
        return Ok(());
    }
    mandate.save(&path)?;
    println!("{}", field("file", &path.display().to_string()));

    for f in &fields {
        if !to && sealed_also_requires(*f) {
            println!(
                "{}",
                warn(&format!(
                    "{} is still required: a sealed policy fixes it on, and the \
                     mandate cannot loosen that",
                    f.key()
                ))
            );
        }
    }
    Ok(())
}

/// Back to insisting on both.
pub fn reset() -> Result<(), String> {
    println!("{}", heading("Back to the default"));
    let (path, mut mandate) = load()?;
    if !mandate.reset() {
        println!("{}", warn("already the default: both required"));
        return Ok(());
    }
    mandate.save(&path)?;
    println!(
        "{}",
        ok("both the app lock and encryption are required again")
    );
    println!("{}", field("file", &path.display().to_string()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mandate_sits_beside_the_app_lock() {
        let Some(path) = veilvoice_policy::mandate_path() else {
            return; // a platform with nowhere to keep configuration
        };
        let Some(lock) = veilvoice_crypto::lock::default_path() else {
            return;
        };
        assert_eq!(path.parent(), lock.parent());
    }

    #[test]
    fn naming_no_field_is_refused_rather_than_treated_as_all_of_them() {
        // A bare `veilvoice mandate relax` that meant "relax everything" would
        // be the worst possible default for the one command here that removes a
        // protection.
        let error = wanted(false, false).unwrap_err();
        assert!(error.contains("--app-lock"), "{error}");
        assert!(error.contains("--encryption"), "{error}");
    }

    #[test]
    fn naming_one_field_selects_only_that_one() {
        assert_eq!(wanted(false, true).unwrap(), vec![MField::Encryption]);
        assert_eq!(wanted(true, false).unwrap(), vec![MField::AppLock]);
        assert_eq!(
            wanted(true, true).unwrap(),
            vec![MField::AppLock, MField::Encryption]
        );
    }

    #[test]
    fn every_field_has_a_sentence_saying_what_it_costs() {
        for f in [MField::AppLock, MField::Encryption] {
            let text = describe(f);
            assert!(text.len() > 20, "{f:?} is described as {text:?}");
            assert!(
                !text.contains("--"),
                "no em dash stand-ins in prose: {text:?}"
            );
        }
    }
}
