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
//!
//! # In plain words
//!
//! The command line for settings that can only be tightened.
//!
//! It shows what is currently required, and what a job would actually run with
//! once those requirements are applied, so a value shown is a value used.

use crate::atrest::{prompt_secret, read_new_password};
use crate::theme::{colour, err, field, heading, ok, paint, warn};
use std::path::PathBuf;
use veilvoice_policy::{Policy, Posture, Requirement, Verification};

/// Where the policy files live.
///
/// Derived from the app lock's location rather than resolved again, so there is
/// one answer to "where does VeilVoice keep things".
pub fn policy_dir() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name("").join("policy"))
}

/// Where policies are kept, or why this machine cannot say.
///
/// [`policy_dir`] answers `None` when no environment variable names a
/// configuration directory, which is a real state on a stripped-down machine
/// rather than a bug. This is the wrapper that turns that into the sentence a
/// reader gets, in one place, so every subcommand explains it the same way.
fn dir() -> Result<PathBuf, String> {
    policy_dir().ok_or_else(|| {
        "this platform did not say where to keep configuration (no APPDATA, \
         XDG_CONFIG_HOME or HOME), so there is nowhere to keep a policy"
            .to_string()
    })
}

/// The settings a command will run with, once this machine's policy is applied.
///
/// **F-338.** Only the window used to apply the policy. Every command here
/// that processes a recording ignored it, so a policy requiring encryption at
/// rest, metadata cleaning, accent neutralisation or a minimum intensity was
/// no obstacle to anybody with a terminal. Now each one passes its flags
/// through here first.
///
/// As in the window, this only ever tightens, and it never changes the
/// policy. Anything it overruled is said on standard error, so a flag that
/// did not take effect is not a surprise. A policy that cannot be read is
/// reported and not applied, which is also what the window does.
pub fn apply(posture: Posture) -> Posture {
    apply_from(policy_dir(), posture)
}

/// [`apply`], with the policy folder passed in so a test can choose it.
fn apply_from(dir: Option<PathBuf>, posture: Posture) -> Posture {
    let Some(dir) = dir else {
        return posture;
    };
    match Policy::load(&dir) {
        Ok(Some(policy)) => {
            let applied = policy.constrain(posture);
            for line in overruled(&posture, &applied) {
                eprintln!("{}", warn(&line));
            }
            applied
        }
        Ok(None) => posture,
        Err(error) => {
            eprintln!(
                "{}",
                warn(&format!(
                    "the policy in {} could not be read, so it was not applied: {error}",
                    dir.display()
                ))
            );
            posture
        }
    }
}

/// What a policy changed, one sentence for each thing.
fn overruled(asked: &Posture, applied: &Posture) -> Vec<String> {
    let mut said = Vec::new();
    if applied.encrypt_recordings && !asked.encrypt_recordings {
        said.push("the policy requires encryption at rest, so the result is encrypted".into());
    }
    if applied.clean_metadata && !asked.clean_metadata {
        said.push("the policy requires metadata cleaning, so the result is cleaned".into());
    }
    if applied.neutralise_accent && !asked.neutralise_accent {
        said.push("the policy requires accent neutralisation, so --keep-accent is ignored".into());
    }
    if applied.intensity > asked.intensity {
        said.push(format!(
            "the policy sets a minimum intensity of {:.2}, so {:.2} was raised to it",
            applied.intensity, asked.intensity
        ));
    }
    said
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
    use veilvoice_policy::Posture;

    fn loosest() -> Posture {
        Posture::most_permissive()
    }

    /// **F-338.** A policy in the folder tightens what a command asked for.
    #[test]
    fn a_policy_tightens_what_the_command_line_asked_for() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = veilvoice_policy::Policy::new();
        policy
            .require(veilvoice_policy::Requirement::EncryptRecordings)
            .require(veilvoice_policy::Requirement::NeutraliseAccent)
            .require(veilvoice_policy::Requirement::MinimumIntensity(80));
        std::fs::write(dir.path().join("policy.txt"), policy.to_text()).unwrap();

        let applied = super::apply_from(Some(dir.path().to_path_buf()), loosest());
        assert!(applied.encrypt_recordings);
        assert!(applied.neutralise_accent);
        assert!(!applied.clean_metadata, "nothing required it");
        assert!(applied.intensity >= 0.8 - f32::EPSILON);
        assert!(applied.is_at_least_as_strict_as(&loosest()));
    }

    /// With no policy, or no folder for one, nothing changes.
    #[test]
    fn without_a_policy_the_flags_stand() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            super::apply_from(Some(dir.path().to_path_buf()), loosest()),
            loosest()
        );
        assert_eq!(super::apply_from(None, loosest()), loosest());
    }

    /// Every change a policy makes is said, and nothing else is.
    #[test]
    fn what_a_policy_overruled_is_said() {
        let mut applied = loosest();
        applied.encrypt_recordings = true;
        applied.intensity = 0.5;
        assert_eq!(super::overruled(&loosest(), &applied).len(), 2);
        assert!(super::overruled(&applied, &applied).is_empty());
    }

    /// **F-338.** Every command that processes a recording goes through the
    /// policy before it starts.
    #[test]
    fn every_processing_command_applies_the_policy() {
        let main = include_str!("main.rs");
        for arm in [
            "Command::Anonymise {",
            "Command::Live {",
            "Command::Record {",
        ] {
            let at = main.find(arm).expect("the command exists");
            let body = &main[at..at + 1500.min(main.len() - at)];
            assert!(
                body.contains("policy::apply("),
                "{arm} runs without applying the policy"
            );
        }
    }

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
