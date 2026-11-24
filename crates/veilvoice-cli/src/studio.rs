// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice studio`: the Recording Studio's vault, from the command line.
//!
//! The window's Browser lists, plays and exports what the Recording Studio
//! keeps. This reaches the same vault through the same code, so a machine with
//! no display, or somebody who would rather type, can reach it too. **F-339**:
//! until this, the vault had no command at all.
//!
//! It asks for the two passphrases the window asks for: the app lock's, then
//! the recording passphrase. The app lock is checked the way the window checks
//! it, through the decoy store, so the decoy passphrase opens an empty studio
//! here exactly as it does there, and takes the same time to do it.
//!
//! It never makes a vault. The window makes one the first time it records, and
//! asking about an empty folder here says so and leaves the folder empty.

use crate::atrest::prompt_secret;
use crate::theme::{heading, ok, warn};
use std::path::{Path, PathBuf};
use veilvoice_crypto::decoy::{self, Opened};
use veilvoice_crypto::studio::{self, Entry, Studio, StudioKey};
use veilvoice_crypto::{lock, Secret};

/// What to do with the vault.
#[derive(clap::Subcommand)]
pub enum Action {
    /// What the vault holds: names, dates and sizes. Nothing is decrypted.
    List,
    /// Write one recording out of the vault as an ordinary WAV.
    ///
    /// What leaves the vault is not sealed, so this needs --yes.
    Export {
        /// The recording's id, as `veilvoice studio list` prints it.
        id: String,
        /// Where to write it. Defaults to `<id>.wav` here, and is never
        /// written over.
        #[arg(short, long)]
        to: Option<PathBuf>,
        /// Confirm that an unsealed copy is what you want.
        #[arg(long)]
        yes: bool,
    },
    /// Remove one recording from the vault, for good.
    Remove {
        /// The recording's id, as `veilvoice studio list` prints it.
        id: String,
        /// Confirm the removal. It cannot be undone.
        #[arg(long)]
        yes: bool,
    },
}

/// What the window says when the pair of passphrases does not open the vault.
const WRONG_PAIR: &str = "that pair did not open the vault. Both passphrases have to be \
                          the ones it was made with, and either one being wrong looks \
                          exactly like this";

/// Run one action.
pub fn run(action: Action) -> Result<(), String> {
    println!("{}", heading("Recording Studio vault"));
    // Refused before any passphrase is asked for, so nobody types two
    // passphrases to be told the command needed a flag.
    match &action {
        Action::Export { yes: false, .. } => {
            println!(
                "{}",
                warn(
                    "an exported recording is an ordinary, unsealed file. Pass --yes to write one."
                )
            );
            return Ok(());
        }
        Action::Remove { yes: false, .. } => {
            println!(
                "{}",
                warn("removing a recording cannot be undone. Pass --yes to remove it.")
            );
            return Ok(());
        }
        _ => {}
    }

    let vault = open()?;
    match action {
        Action::List => {
            list(vault.as_ref());
            Ok(())
        }
        Action::Export { id, to, .. } => {
            let vault = vault.ok_or_else(|| no_such(&id))?;
            let entry = find(&vault, &id)?;
            let path = to.unwrap_or_else(|| PathBuf::from(format!("{}.wav", entry.id)));
            export_to(&vault, &entry, &path)?;
            println!("{}", ok(&format!("wrote {}", path.display())));
            println!(
                "  It is not sealed: what leaves the vault is an ordinary file, and the \
                 voice in it is still the veiled one."
            );
            Ok(())
        }
        Action::Remove { id, .. } => {
            let vault = vault.ok_or_else(|| no_such(&id))?;
            let entry = find(&vault, &id)?;
            vault.remove(&entry.id).map_err(|e| e.to_string())?;
            println!("{}", ok(&format!("removed {}", entry.id)));
            Ok(())
        }
    }
}

/// Check the app lock and open the vault, or `None` for an empty studio.
fn open() -> Result<Option<Studio>, String> {
    let (store, _) = lock::open_default().map_err(|e| e.to_string())?;
    let Some(mut store) = store else {
        return Err(
            "no app lock is set here, and the Recording Studio's vault needs one. \
                    `veilvoice lock set` makes it."
                .to_string(),
        );
    };
    let app = prompt_secret("App-lock passphrase: ")?;
    let given = std::str::from_utf8(app.expose()).map_err(|_| "that passphrase is not text")?;

    // The window's order exactly: through the decoy store, which asks both
    // questions in one breath. A decoy store that will not open falls back to
    // the lock alone, because a broken decoy must never be what stops an
    // unlock.
    let base = lock::default_dir().ok_or("no configuration folder on this system")?;
    let answer = match decoy::Store::at(&base) {
        Ok(decoys) => decoys.judge(&mut store, given).map_err(|e| e.to_string())?,
        Err(_) => match store.unlock(app.expose()) {
            Ok(()) => Opened::Real,
            Err(_) => Opened::Wrong,
        },
    };
    match answer {
        Opened::Wrong => return Err("that passphrase does not open VeilVoice".to_string()),
        Opened::Decoy => return Ok(None),
        Opened::Real => {}
    }

    let rest = prompt_secret("Recording passphrase: ")?;
    let parent = lock::default_path()
        .map(|path| path.with_file_name("studio"))
        .ok_or("no configuration folder on this system")?;
    open_with(&parent, &app, &rest)
}

/// Open the vault under `parent` with the two passphrases, never making one.
fn open_with(parent: &Path, app: &Secret, rest: &Secret) -> Result<Option<Studio>, String> {
    if studio::vault_dirs(parent)
        .map_err(|e| e.to_string())?
        .is_empty()
    {
        return Ok(None);
    }
    let key = StudioKey::derive(app, rest).map_err(|e| e.to_string())?;
    let vault = studio::find_or_make(parent, key).map_err(|_| WRONG_PAIR.to_string())?;
    vault.list().map_err(|_| WRONG_PAIR.to_string())?;
    Ok(Some(vault))
}

/// Print what the vault holds.
fn list(vault: Option<&Studio>) {
    let entries = vault.and_then(|v| v.list().ok()).unwrap_or_default();
    if entries.is_empty() {
        println!("  The vault is empty.");
        return;
    }
    for entry in &entries {
        println!(
            "  {}  {}  {:>9}  {}",
            entry.id,
            day(entry.made),
            size(entry.bytes),
            entry.name
        );
    }
}

/// The entry whose id is `id`.
fn find(vault: &Studio, id: &str) -> Result<Entry, String> {
    vault
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| no_such(id))
}

/// What is said for an id the vault does not hold, decoy or not.
fn no_such(id: &str) -> String {
    format!("there is no recording called {id} in the vault")
}

/// Write one recording to `path`, owner-only, and never over a file that is
/// already there.
fn export_to(vault: &Studio, entry: &Entry, path: &Path) -> Result<(), String> {
    let wav = vault.load(&entry.id).map_err(|e| e.to_string())?;
    veilvoice_crypto::privatefile::write_owner_only_new(path, wav.expose())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// A Unix time as the day it fell on, in UTC.
fn day(seconds: i64) -> String {
    // Howard Hinnant's days-to-civil: exact for every day this program will
    // ever see, and no date library for one line of output.
    let z = seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A byte count the way a person reads one.
fn size(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(text: &str) -> Secret {
        let mut bytes = text.as_bytes().to_vec();
        Secret::new(&mut bytes)
    }

    /// A folder with no vault in it is not given one.
    #[test]
    fn an_empty_folder_is_not_given_a_vault() {
        let dir = tempfile::tempdir().unwrap();
        assert!(open_with(dir.path(), &secret("app"), &secret("rest"))
            .unwrap()
            .is_none());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    /// **F-339.** The vault the window made opens here with the same pair,
    /// and a wrong pair is refused rather than shown as empty.
    #[test]
    fn the_windows_vault_opens_with_the_same_pair() {
        let dir = tempfile::tempdir().unwrap();
        let key = StudioKey::derive(&secret("app"), &secret("rest")).unwrap();
        let made = studio::find_or_make(dir.path(), key).unwrap();
        made.store("take one", 1_700_000_000, b"some audio")
            .unwrap();

        let vault = open_with(dir.path(), &secret("app"), &secret("rest"))
            .unwrap()
            .expect("the vault opens");
        assert_eq!(vault.list().unwrap().len(), 1);
        assert!(open_with(dir.path(), &secret("app"), &secret("wrong")).is_err());
        assert_eq!(
            studio::vault_dirs(dir.path()).unwrap().len(),
            1,
            "no vault was added"
        );
    }

    /// An export never writes over a file that is already there.
    #[test]
    fn an_export_never_writes_over_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let key = StudioKey::derive(&secret("app"), &secret("rest")).unwrap();
        let vault = studio::find_or_make(&dir.path().join("studio"), key).unwrap();
        let entry = vault.store("take one", 0, b"the audio").unwrap();
        let out = dir.path().join("take.wav");
        export_to(&vault, &entry, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"the audio");
        assert!(export_to(&vault, &entry, &out).is_err());
        assert_eq!(std::fs::read(&out).unwrap(), b"the audio");
    }

    #[test]
    fn dates_and_sizes_read_the_way_a_person_reads_them() {
        assert_eq!(day(0), "1970-01-01");
        assert_eq!(day(1_700_000_000), "2023-11-14");
        assert_eq!(day(951_782_400), "2000-02-29");
        assert_eq!(size(512), "512 B");
        assert_eq!(size(1_234_567), "1.2 MB");
    }
}
