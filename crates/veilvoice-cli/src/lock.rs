// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice lock` — manage the application lock from the command line.
//!
//! The lock guards the desktop app: with one set, VeilVoice asks for a password
//! before it will show anything or start a live scramble. Managing it from here
//! exists because a headless machine still has a config directory, and because
//! anything the GUI can do to a file on disk should be inspectable without the
//! GUI.
//!
//! Every path through this module prints [`veilvoice_crypto::lock::SCOPE`], for
//! one reason: a lock the user believes is stronger than it is has made them
//! *less* safe, not more.

use crate::atrest::{prompt_secret, read_new_password};
use crate::theme::{colour, field, heading, ok, paint, warn};
use clap::Subcommand;
use std::path::{Path, PathBuf};
use veilvoice_crypto::{kdf, lock, LockStore};

#[derive(Subcommand)]
pub enum Action {
    /// Report whether a lock is set, and where it lives.
    Status,
    /// Set a lock. Refuses if one is already configured.
    Set,
    /// Change the password on an existing lock.
    Change,
    /// Remove the lock, after proving the current password.
    Remove,
}

/// Resolve the lock file, preferring an explicit `--path`.
fn resolve(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    match explicit {
        Some(p) => Ok(p),
        None => lock::default_path().ok_or_else(|| {
            "cannot work out where this platform keeps configuration \
             (no APPDATA, XDG_CONFIG_HOME or HOME) — pass --path"
                .to_string()
        }),
    }
}

/// Print the honest scope note, wrapped for a terminal.
fn print_scope() {
    println!("{}", paint(colour::MUTED, "  What this is worth:"));
    for line in wrap(lock::SCOPE, 66) {
        println!("{}", paint(colour::MUTED, &format!("    {line}")));
    }
}

/// Greedy word wrap. The scope note is single-sourced from the crypto crate, so
/// it arrives as one long string and has to be broken here rather than there.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub fn run(action: Action, path: Option<PathBuf>) -> Result<(), String> {
    let path = resolve(path)?;
    println!("{}", heading("App lock"));
    println!("{}", field("File", &path.display().to_string()));

    match action {
        Action::Status => status(&path),
        Action::Set => set(&path),
        Action::Change => change(&path),
        Action::Remove => remove(&path),
    }
}

fn status(path: &Path) -> Result<(), String> {
    let store = LockStore::open(path).map_err(|e| e.to_string())?;
    match store {
        None => {
            println!("{}", field("State", "not set"));
            println!();
            println!(
                "{}",
                paint(colour::MUTED, "  Set one with: veilvoice lock set")
            );
        }
        Some(store) => {
            println!("{}", field("State", "set"));
            println!(
                "{}",
                field("Failed attempts", &store.failures().to_string())
            );
            match store.cooldown() {
                Some(wait) => println!(
                    "{}",
                    warn(&format!(
                        "rate limited — {} s before the next attempt",
                        wait.as_secs()
                    ))
                ),
                None => println!("{}", field("Rate limit", "not currently in force")),
            }
        }
    }
    println!();
    print_scope();
    Ok(())
}

fn set(path: &Path) -> Result<(), String> {
    if LockStore::open(path).map_err(|e| e.to_string())?.is_some() {
        return Err("a lock is already set here — use `veilvoice lock change`".into());
    }
    println!();
    print_scope();
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  This password unlocks the app. Do NOT reuse the passphrase you"
        )
    );
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  use for encrypted recordings — they are deliberately separate,"
        )
    );
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  so that unlocking the app does not unseal the recordings."
        )
    );

    let password = read_new_password()?;
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Deriving verifier (Argon2id, deliberately slow)..."
        )
    );
    LockStore::create(path, password.expose(), kdf::KdfParams::default())
        .map_err(|e| e.to_string())?;
    println!("{}", ok("app lock set"));
    Ok(())
}

fn change(path: &Path) -> Result<(), String> {
    let mut store = open_or_explain(path)?;
    let current = prompt_secret("Current password: ")?;
    println!("{}", paint(colour::MUTED, "  Now the new one."));
    let new = read_new_password()?;
    store
        .change_password(current.expose(), new.expose())
        .map_err(|e| e.to_string())?;
    println!("{}", ok("app lock password changed"));
    Ok(())
}

fn remove(path: &Path) -> Result<(), String> {
    let store = open_or_explain(path)?;
    let current = prompt_secret("Current password: ")?;
    store.remove(current.expose()).map_err(|e| e.to_string())?;
    println!(
        "{}",
        ok("app lock removed — VeilVoice will open freely again")
    );
    Ok(())
}

fn open_or_explain(path: &Path) -> Result<LockStore, String> {
    LockStore::open(path)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no lock is set here — use `veilvoice lock set`".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_keeps_every_word_and_respects_the_width() {
        let lines = wrap(lock::SCOPE, 40);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| l.len() <= 40 || !l.contains(' ')));
        let rejoined = lines.join(" ");
        let original: Vec<&str> = lock::SCOPE.split_whitespace().collect();
        assert_eq!(rejoined.split_whitespace().collect::<Vec<_>>(), original);
    }

    #[test]
    fn wrapping_handles_an_empty_string() {
        assert!(wrap("", 20).is_empty());
    }

    #[test]
    fn an_explicit_path_wins_over_the_platform_default() {
        let chosen = PathBuf::from("somewhere/else.bin");
        assert_eq!(resolve(Some(chosen.clone())).unwrap(), chosen);
    }

    /// The lock lifecycle through the same store the subcommands drive. The
    /// prompts themselves need a terminal, so this exercises the layer beneath.
    #[test]
    fn a_lock_can_be_set_proven_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("applock.bin");
        let weak = kdf::KdfParams::weak_for_tests();

        assert!(LockStore::open(&path).unwrap().is_none());
        LockStore::create(&path, b"app password", weak).unwrap();
        assert!(status(&path).is_ok());

        let mut store = LockStore::open(&path).unwrap().unwrap();
        assert!(store.unlock(b"recording password").is_err());
        store.unlock(b"app password").unwrap();

        LockStore::open(&path)
            .unwrap()
            .unwrap()
            .remove(b"app password")
            .unwrap();
        assert!(!path.exists());
    }
}
