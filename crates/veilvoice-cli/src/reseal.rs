// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice reseal`: repairing files that were sealed under no passphrase.
//!
//! # Why this command exists
//!
//! [`veilvoice_crypto::reseal`] has the account of the fault. In short:
//! releases 0.1.15 to 0.1.22 could seal a recording under an empty passphrase
//! while reporting that it had been sealed with the app lock, and a file in
//! that state opens for anybody who has it.
//!
//! Nothing about such a file is visibly wrong, so there is nothing for a person
//! to look for. The only way to find them is to try the empty passphrase on
//! each file and see which ones open, which is what this does.
//!
//! # It shows the list before it asks for anything
//!
//! The sweep runs first and prints what it found, including the files that are
//! sealed properly, so the number of files checked is on the screen next to the
//! number that need repairing. Only then is a passphrase asked for, and only if
//! there is something to repair. `--dry-run` stops after the list.
//!
//! # The passphrase is proved against the lock first
//!
//! A mistyped passphrase here would reseal every affected recording under a
//! passphrase nobody knows, which is a worse outcome than the fault. So where
//! this machine has an app lock, the passphrase has to open it before anything
//! is written, through the same rate-limited [`LockStore::unlock`] the window
//! and `veilvoice lock` use.
//!
//! Where there is no lock, there is nothing to prove it against, and it is
//! asked for twice instead. That case is real: somebody who reset their lock
//! after 0.1.22 still has the recordings it sealed.
//!
//! # In plain words
//!
//! Finds recordings that an older VeilVoice locked with a blank password by
//! mistake, and locks them properly with your real one. It shows you the list
//! first, and it does not touch anything that was already locked properly.

use crate::atrest::{prompt_secret, read_new_password};
use crate::theme::{colour, err, field, heading, ok, paint, warn};
use std::path::PathBuf;
use veilvoice_crypto::{kdf, layout, lock, reseal, LockStore};

/// Run the repair.
pub fn run(folders: Vec<PathBuf>, dry_run: bool) -> Result<(), String> {
    println!("{}", heading("Reseal"));
    for line in crate::lock::wrap(reseal::WARNING, 66) {
        println!("{}", paint(colour::MUTED, &format!("  {line}")));
    }
    println!();

    let folders = where_to_look(folders)?;
    let mut found = Vec::new();
    for folder in &folders {
        println!("{}", field("folder", &folder.display().to_string()));
        found.extend(reseal::sweep(folder));
    }
    println!();

    let mut affected = Vec::new();
    let mut sealed = 0usize;
    let mut unreadable = Vec::new();
    for entry in found {
        match &entry.standing {
            reseal::Standing::OpensWithNothing => affected.push(entry.path),
            reseal::Standing::Sealed | reseal::Standing::PublicKey => sealed += 1,
            reseal::Standing::Unreadable(why) => unreadable.push((entry.path, why.clone())),
            // `sweep` leaves these out, and a line here claiming otherwise
            // would be the kind of dead branch nobody notices going stale.
            reseal::Standing::NotAContainer => {}
        }
    }

    println!(
        "{}",
        field(
            "checked",
            &format!(
                "{} sealed properly, {} sealed under nothing",
                sealed,
                affected.len()
            )
        )
    );
    for (path, why) in &unreadable {
        println!(
            "{}",
            warn(&format!("could not check {}: {why}", path.display()))
        );
    }

    if affected.is_empty() {
        println!();
        println!(
            "{}",
            ok("nothing in those folders was sealed under an empty passphrase")
        );
        print_reach();
        return Ok(());
    }

    println!();
    println!("{}", paint(colour::YELLOW, "THESE OPEN FOR ANYBODY"));
    for path in &affected {
        println!("{}", field("file", &path.display().to_string()));
    }

    if dry_run {
        println!();
        println!("{}", ok("nothing was touched: this was a dry run"));
        return Ok(());
    }

    let passphrase = ask(&mut applock()?)?;

    println!();
    let mut failed = 0usize;
    for path in &affected {
        match reseal::reseal(path, passphrase.expose(), kdf::KdfParams::default()) {
            Ok(()) => println!("{}", ok(&format!("resealed {}", path.display()))),
            Err(e) => {
                failed += 1;
                println!("{}", err(&format!("{}: {e}", path.display())));
            }
        }
    }

    if failed > 0 {
        return Err(format!(
            "{failed} of {} could not be resealed, and the lines above say why. \
             Every one of those is still the file it was, so running this again \
             is safe",
            affected.len()
        ));
    }

    println!();
    println!(
        "{}",
        ok("every one of those now needs your app-lock passphrase")
    );
    print_reach();
    Ok(())
}

/// Which folders to sweep: the ones named, or the one VeilVoice writes to.
fn where_to_look(given: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
    if !given.is_empty() {
        return Ok(given);
    }
    let dir = layout::dir().ok_or_else(|| {
        "cannot work out where this platform keeps configuration (no APPDATA, \
         XDG_CONFIG_HOME or HOME), so name the folder to check"
            .to_string()
    })?;
    Ok(vec![dir])
}

/// The app lock, where this machine has one.
fn applock() -> Result<Option<LockStore>, String> {
    if layout::dir().is_none() {
        return Ok(None);
    }
    lock::open_default().map(|(store, _)| store).map_err(|e| {
        format!(
            "the app lock could not be read, so the passphrase cannot be \
             checked before it is used: {e}"
        )
    })
}

/// Ask for the passphrase, and prove it before handing it back.
///
/// The proving is the point. Resealing under a mistyped passphrase would leave
/// the recordings sealed under something nobody knows, and unlike the fault
/// this repairs, that one has no repair.
fn ask(store: &mut Option<LockStore>) -> Result<veilvoice_crypto::Secret, String> {
    match store {
        Some(store) => {
            let secret = prompt_secret("App-lock passphrase: ")?;
            store.unlock(secret.expose()).map_err(|e| e.to_string())?;
            Ok(secret)
        }
        None => {
            println!();
            println!(
                "{}",
                warn(
                    "there is no app lock on this machine, so there is nothing to \
                     check the passphrase against"
                )
            );
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  It is asked for twice instead. Type the passphrase the app lock\n  \
                     had when those recordings were made: that is what they were\n  \
                     sealed under, and nothing here can tell you whether it is right."
                )
            );
            read_new_password()
        }
    }
}

/// The honest limit, printed whichever way the sweep went.
fn print_reach() {
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Only the folders above were checked. A recording exported somewhere\n  \
             else, copied to another machine, or already sent to somebody is not\n  \
             something this can reach: name that folder to check it too.\n\n  \
             Resealing protects a file from now on. It cannot undo a copy somebody\n  \
             already has."
        )
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With no folder named, the command checks the folder VeilVoice actually
    /// writes to. Hard-coding anything else here would repair a folder nobody
    /// has recordings in and report a clean machine.
    #[test]
    fn the_default_folder_is_the_one_veilvoice_writes_to() {
        if layout::dir().is_none() {
            return; // a platform that does not say where configuration goes
        }
        assert_eq!(
            where_to_look(Vec::new()).unwrap(),
            vec![layout::dir().unwrap()]
        );
    }

    #[test]
    fn a_named_folder_wins_over_the_default() {
        let named = vec![PathBuf::from("somewhere/else")];
        assert_eq!(where_to_look(named.clone()).unwrap(), named);
    }

    /// The warning the command opens with is the crate's own, printed as it
    /// is.
    ///
    /// A repair tool whose warning is a paraphrase of the release note's is how
    /// the two end up disagreeing about which versions are affected, and
    /// somebody then checks the wrong machine. What the warning says is
    /// `veilvoice_crypto::reseal`'s own test; that it is the one being printed
    /// is this one.
    #[test]
    fn the_warning_is_the_shared_one() {
        assert!(include_str!("reseal.rs").contains("reseal::WARNING"));
        assert!(reseal::WARNING.contains("0.1.15 to 0.1.22"));
    }
}
