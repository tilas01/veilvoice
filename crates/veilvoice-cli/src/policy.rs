// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice policy` -- settings that can only be tightened.
//!
//! The command-line front end to [`veilvoice_policy`]. That crate holds the
//! logic and the honest account of what a sealed policy is worth; this file
//! decides where the two files live and prints them.
//!
//! # Where the policy lives
//!
//! ```text
//! <config>/veilvoice/policy/policy.txt     read at every launch, no passphrase
//! <config>/veilvoice/policy/policy.sealed  the same policy under a passphrase
//! ```
//!
//! Per-user, beside everything else this program keeps. **Not** a machine-wide
//! location: writing to one would need administrator rights, and a policy this
//! program applies to itself does not become enforcement by living somewhere
//! only root can write. What it would become is a thing that looks like
//! enforcement, which is worse than the honest version.
//!
//! # Why `remove` needs no passphrase
//!
//! Because it could not meaningfully require one. Anybody who can run this
//! command can delete the two files with the file manager, and a program that
//! pretends otherwise is teaching its user something false. `--yes` is there so
//! it is not done by accident, and the message says plainly what the
//! passphrase is and is not for.

use crate::atrest::{prompt_secret, read_new_password};
use crate::theme::{colour, err, field, heading, ok, paint, warn};
use std::path::PathBuf;
use veilvoice_policy::{Policy, Requirement, Verification};

/// Where the policy files live.
///
/// Derived from the app lock's location rather than resolved again, so there is
/// one answer to "where does VeilVoice keep things".
pub fn policy_dir() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name("").join("policy"))
}

fn dir() -> Result<PathBuf, String> {
    policy_dir().ok_or_else(|| {
        "this platform did not say where to keep configuration (no APPDATA, \
         XDG_CONFIG_HOME or HOME), so there is nowhere to keep a policy"
            .to_string()
    })
}

/// What is in force, and what is known about the seal.
pub fn status() -> Result<(), String> {
    println!("{}", heading("Policy"));
    let dir = dir()?;
    let policy = Policy::load(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;

    match &policy {
        None => {
            println!("{}", field("in force", "nothing"));
            println!();
            println!("  A policy fixes some of VeilVoice's settings so they cannot be");
            println!("  turned off from the interface. Every setting it can reach makes");
            println!("  VeilVoice stricter; there is nothing here that loosens anything.");
            println!();
            println!("    veilvoice policy seal --encrypt-recordings --clean-metadata");
            println!();
        }
        Some(policy) => {
            println!(
                "{}",
                field("in force", &format!("{} requirement(s)", policy.len()))
            );
            if let Some(note) = policy.note() {
                println!("{}", field("note", note));
            }
            println!();
            for requirement in policy.requirements() {
                println!("  {}", paint(colour::CYAN, requirement.keyword()));
                println!("    {}", requirement.describe());
            }
            println!();
            // Loading never asks for a passphrase, so this is all that is
            // known without one -- and it says exactly that rather than
            // implying the seal was checked.
            println!("{}", field("seal", &Verification::Unchecked.describe()));
            println!("  `veilvoice policy verify` checks it, and needs the passphrase.");
            println!();
        }
    }

    println!("{}", paint(colour::YELLOW, "WHAT THIS IS WORTH"));
    for line in crate::sentry::wrap(veilvoice_policy::SCOPE, 72) {
        println!("  {line}");
    }
    Ok(())
}

/// Write a policy and seal it.
pub fn seal(
    requirements: Vec<Requirement>,
    note: Option<String>,
    replace: bool,
) -> Result<(), String> {
    println!("{}", heading("Seal a policy"));
    let dir = dir()?;

    if requirements.is_empty() {
        return Err(
            "a policy with no requirements would fix nothing. Name at least one, for \
             example --encrypt-recordings."
                .to_string(),
        );
    }

    // Replacing a policy somebody else sealed is the one destructive thing
    // here, so it is not done without being asked for. The passphrase would
    // not have prevented it -- the files can be deleted by hand -- so the
    // guard is a flag and an honest sentence rather than a lock.
    if !replace && Policy::load(&dir).map_err(|e| e.to_string())?.is_some() {
        println!(
            "{}",
            warn("a policy is already in force. Pass --replace to write over it.")
        );
        println!("  `veilvoice policy status` shows what is there now.");
        return Ok(());
    }

    let mut policy = Policy::new();
    for requirement in requirements {
        policy.require(requirement);
    }
    if let Some(note) = note {
        policy = policy.with_note(&note).map_err(|error| error.to_string())?;
    }

    println!("  This will fix:");
    for requirement in policy.requirements() {
        println!("    {}", requirement.describe());
    }
    println!();
    println!("  The passphrase seals a copy, so anybody who has it can prove the");
    println!("  policy in force is the one you wrote. It does not stop the policy");
    println!("  being deleted, and nothing here can.");
    println!();

    let password = read_new_password()?;
    policy
        .save(&dir, password.expose())
        .map_err(|error| format!("could not write the policy: {error}"))?;

    println!("{}", ok(&format!("sealed into {}", dir.display())));
    Ok(())
}

/// Check the plain policy against its sealed copy.
pub fn verify() -> Result<bool, String> {
    println!("{}", heading("Verify a policy"));
    let dir = dir()?;
    let password = prompt_secret("Policy passphrase: ")?;
    let checked = veilvoice_policy::verify(&dir, password.expose())
        .map_err(|error| format!("could not check the seal: {error}"))?;

    let line = checked.describe();
    if checked.wants_attention() {
        println!("{}", err(&line));
    } else {
        println!("{}", ok(&line));
    }

    if let Verification::Differs { sealed } = &checked {
        println!();
        println!("  What was sealed:");
        for requirement in sealed.requirements() {
            println!("    {}", requirement.keyword());
        }
        println!();
        println!("  What is in force is in `veilvoice policy status`.");
    }
    Ok(checked.wants_attention())
}

/// Delete both files.
pub fn remove(yes: bool) -> Result<(), String> {
    println!("{}", heading("Remove the policy"));
    let dir = dir()?;
    if Policy::load(&dir).map_err(|e| e.to_string())?.is_none()
        && !dir.join(veilvoice_policy::SEALED_FILE).exists()
    {
        println!("{}", warn("there is no policy to remove"));
        return Ok(());
    }
    if !yes {
        println!();
        println!("  This deletes both files, and does not ask for the passphrase.");
        println!("  It could not usefully ask: anybody who can run this command can");
        println!("  delete the same two files with a file manager. The passphrase");
        println!("  proves a policy is the one that was written; it is not a lock on");
        println!("  the folder, and saying otherwise would be a lie about what this");
        println!("  program does.");
        println!();
        println!("  Re-run with --yes to proceed.");
        return Ok(());
    }
    for name in [veilvoice_policy::PLAIN_FILE, veilvoice_policy::SEALED_FILE] {
        let path = dir.join(name);
        match std::fs::remove_file(&path) {
            Ok(()) => println!("{}", ok(&format!("removed {}", path.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("could not remove {}: {error}", path.display())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_directory_sits_beside_the_app_lock() {
        let Some(dir) = policy_dir() else {
            return; // a platform with nowhere to keep configuration
        };
        assert!(dir.ends_with("policy"), "{}", dir.display());
        let lock = veilvoice_crypto::lock::default_path().unwrap();
        assert_eq!(dir.parent(), lock.parent());
    }

    /// The two state directories must not be the same one, or a canary record
    /// and a policy would share a folder and each other's names.
    #[test]
    fn the_policy_and_sentry_directories_are_distinct() {
        if let (Some(policy), Some(sentry)) = (policy_dir(), crate::sentry::state_dir()) {
            assert_ne!(policy, sentry);
        }
    }
}
