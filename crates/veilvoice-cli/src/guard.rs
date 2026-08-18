// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! `veilvoice guard` -- record what VeilVoice's files should be, and check them.
//!
//! Detection, not prevention. See [`veilvoice_guard::SCOPE`], which every path
//! through this module prints, for the same reason the app lock prints its own:
//! a protection someone over-trusts has made them less safe, not more.
//!
//! # What the three steps actually do
//!
//! * **`init`** walks the files that make up this installation and records a
//!   SHA-256 for each. Optionally sealed with a passphrase, so the record
//!   itself cannot be quietly rewritten to match tampered files.
//! * **`check`** re-walks and reports what is **modified**, **removed** and
//!   **added**. All three matter: an added file in the installation directory
//!   is as interesting as a changed one.
//! * **`blame`** tries to say *which process* made a change, and says plainly
//!   when it cannot.
//!
//! # Why attribution usually fails, and why that is reported rather than hidden
//!
//! Attribution needs the operating system to have been recording. On Linux that
//! means an `auditd` watch; on Windows a SACL on the path plus the audit policy
//! enabled, and reading it needs elevation. Neither is on by default on a
//! normal machine.
//!
//! So the common answer is "something changed this file and I cannot tell you
//! what", and this module prints exactly that rather than an empty list. An
//! empty list reads as *nothing happened*, which is the opposite of the truth,
//! and is the same mistake as a monitor reporting an empty machine because a
//! registry query silently matched nothing.
//!
//! # The bound, again
//!
//! A manifest running as the user protects nothing from that user, and detects
//! rather than prevents even when it works. Anything that can write these files
//! can write the manifest beside them. That is why the passphrase-sealed record
//! exists, why [`veilvoice_guard::SCOPE`] is printed on every path through this
//! module, and why the word "tamper-proof" appears nowhere in it.

use crate::theme::{colour, err, field, heading, ok, paint, warn};
use clap::Subcommand;
use std::path::{Path, PathBuf};
use veilvoice_guard::{blame_path, manifest_files_in, Manifest, SCOPE};

#[derive(Subcommand)]
pub enum Action {
    /// Record the current state of the watched files.
    Init {
        /// Files to record. Defaults to the running binary and the app lock.
        files: Vec<PathBuf>,
        /// Seal the record with a passphrase, so it cannot be rewritten to
        /// match a tampered file without knowing it.
        #[arg(long)]
        sealed: bool,
    },
    /// Compare the watched files against the record.
    Check {
        /// Also report files that have appeared in this directory.
        #[arg(long, value_name = "DIR")]
        watch_dir: Option<PathBuf>,
    },
    /// Show where the record is kept, and what it is worth.
    Status,
}

/// Where the manifest lives, beside the app lock.
fn manifest_path(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let lock = veilvoice_crypto::lock::default_path().ok_or_else(|| {
        "cannot work out where this platform keeps configuration \
         (no APPDATA, XDG_CONFIG_HOME or HOME) - pass --path"
            .to_string()
    })?;
    Ok(lock.with_file_name("integrity.manifest"))
}

/// A sealed manifest sits beside the plain one, with a different suffix.
fn sealed_path(base: &Path) -> PathBuf {
    veilvoice_crypto::container::veil_path(base)
}

fn print_scope() {
    println!("{}", paint(colour::MUTED, "  What this is worth:"));
    for line in crate::lock::wrap(SCOPE, 66) {
        println!("{}", paint(colour::MUTED, &format!("    {line}")));
    }
}

/// The files worth watching when the user names none: the running binary, and
/// the app lock beside it.
fn default_targets() -> Vec<PathBuf> {
    let mut targets = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        targets.push(exe);
    }
    if let Some(lock) = veilvoice_crypto::lock::default_path() {
        if lock.exists() {
            targets.push(lock);
        }
    }
    targets
}

pub fn run(action: Action, path: Option<PathBuf>) -> Result<(), String> {
    let store = manifest_path(path)?;
    println!("{}", heading("Integrity"));
    println!("{}", field("Record", &store.display().to_string()));

    match action {
        Action::Init { files, sealed } => init(&store, files, sealed),
        Action::Check { watch_dir } => check(&store, watch_dir),
        Action::Status => status(&store),
    }
}

fn init(store: &Path, files: Vec<PathBuf>, sealed: bool) -> Result<(), String> {
    let targets = if files.is_empty() {
        default_targets()
    } else {
        files
    };
    if targets.is_empty() {
        return Err("nothing to record - name some files".into());
    }

    let manifest = Manifest::of(&targets).map_err(|e| e.to_string())?;
    if manifest.is_empty() {
        return Err("none of those files could be read".into());
    }

    for recorded in manifest.paths() {
        println!(
            "{}",
            paint(colour::MUTED, &format!("  recorded  {recorded}"))
        );
    }

    if sealed {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Choose a passphrase for the record. Keep it somewhere other than"
            )
        );
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  beside the record, or sealing it proves nothing."
            )
        );
        let password = crate::atrest::read_new_password()?;
        let bytes = manifest
            .seal(password.expose())
            .map_err(|e| e.to_string())?;
        let out = sealed_path(store);
        std::fs::write(&out, bytes).map_err(|e| format!("{}: {e}", out.display()))?;
        println!(
            "{}",
            ok(&format!("sealed record written to {}", out.display()))
        );
    } else {
        manifest.save(store).map_err(|e| e.to_string())?;
        println!("{}", ok(&format!("record written to {}", store.display())));
        println!(
            "{}",
            warn("this record is unsealed - anything that can rewrite your files can rewrite it")
        );
    }

    println!();
    print_scope();
    Ok(())
}

/// Load whichever form of the record exists, asking for a passphrase only if
/// the sealed one is the one that is there.
fn load(store: &Path) -> Result<Manifest, String> {
    let sealed = sealed_path(store);
    if sealed.exists() {
        let bytes = std::fs::read(&sealed).map_err(|e| format!("{}: {e}", sealed.display()))?;
        let password = crate::atrest::prompt_secret("Record passphrase: ")?;
        return Manifest::open_sealed(password.expose(), &bytes).map_err(|e| e.to_string());
    }
    if store.exists() {
        return Manifest::load(store).map_err(|e| e.to_string());
    }
    Err("no record here yet - run `veilvoice guard init` first".into())
}

fn check(store: &Path, watch_dir: Option<PathBuf>) -> Result<(), String> {
    let manifest = load(store)?;
    let sealed = sealed_path(store);
    let extra = match &watch_dir {
        // The record lives in the directory it watches, so it would otherwise
        // report itself as a new file every time -- noise that trains the user
        // to ignore the output, which is the one thing this must not do.
        Some(dir) => manifest_files_in(dir)
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|found| found != store && found != &sealed)
            .collect(),
        None => Vec::new(),
    };

    let report = manifest.check(&extra);
    println!("{}", field("Files recorded", &manifest.len().to_string()));
    println!("{}", field("Unchanged", &report.unchanged.to_string()));
    println!();

    if report.is_clean() {
        println!("{}", ok("nothing has changed"));
        println!();
        print_scope();
        return Ok(());
    }

    println!("{}", err("SOMETHING HAS CHANGED"));
    // Attribution is usually unavailable. Say so per change, but explain the
    // remedy once at the end -- repeating a paragraph after every line buries
    // the changes themselves, which are the thing being reported.
    let mut remedy_once = None;
    for change in &report.changes {
        println!(
            "{}",
            paint(colour::RED, &format!("  {}", change.describe()))
        );
        let who = blame_path(Path::new(change.path()));
        println!(
            "{}",
            paint(colour::MUTED, &format!("      by: {}", who.describe()))
        );
        if let veilvoice_guard::Blame::Unknown { remedy, .. } = &who {
            // `remedy` is a `&'static str`, so copy it out rather than
            // borrowing `who`, which is dropped at the end of this iteration.
            remedy_once = Some(*remedy);
        }
    }
    if let Some(remedy) = remedy_once {
        println!();
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  To get a name for the program responsible:"
            )
        );
        for line in crate::lock::wrap(remedy, 66) {
            println!("{}", paint(colour::MUTED, &format!("    {line}")));
        }
    }

    println!();
    print_scope();
    // A changed file is the answer the user asked for, not a failure of the
    // command, but the exit code has to let a script notice.
    Err("integrity check failed - see the changes above".into())
}

fn status(store: &Path) -> Result<(), String> {
    let sealed = sealed_path(store);
    let state = if sealed.exists() {
        "sealed record present"
    } else if store.exists() {
        "unsealed record present"
    } else {
        "no record yet"
    };
    println!("{}", field("State", state));
    if sealed.exists() {
        println!("{}", field("Sealed record", &sealed.display().to_string()));
    }
    if !sealed.exists() && store.exists() {
        println!(
            "{}",
            warn("unsealed - anything that can rewrite your files can rewrite this too")
        );
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Re-run with: veilvoice guard init --sealed"
            )
        );
    }
    println!();
    print_scope();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sealed_record_sits_beside_the_plain_one() {
        let base = PathBuf::from("/tmp/integrity.manifest");
        assert_eq!(
            sealed_path(&base),
            PathBuf::from("/tmp/integrity.manifest.veil")
        );
    }

    #[test]
    fn an_explicit_path_wins_over_the_platform_default() {
        let chosen = PathBuf::from("somewhere/else.manifest");
        assert_eq!(manifest_path(Some(chosen.clone())).unwrap(), chosen);
    }

    /// The end-to-end flow, through the same manifest the subcommands drive.
    /// The passphrase prompts need a terminal, so the sealed path is covered by
    /// the crate's own tests instead.
    #[test]
    fn a_record_notices_a_changed_file() {
        let dir = tempfile::tempdir().unwrap();
        let watched = dir.path().join("veilvoice.exe");
        std::fs::write(&watched, b"the real binary").unwrap();
        let store = dir.path().join("integrity.manifest");

        Manifest::of(&[&watched]).unwrap().save(&store).unwrap();
        assert!(load(&store).unwrap().check::<&Path>(&[]).is_clean());

        std::fs::write(&watched, b"not the real binary").unwrap();
        let report = load(&store).unwrap().check::<&Path>(&[]);
        assert!(!report.is_clean());
        assert!(report.changes[0].describe().starts_with("modified:"));
    }

    #[test]
    fn checking_without_a_record_explains_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let err = load(&dir.path().join("nothing.manifest")).unwrap_err();
        assert!(err.contains("guard init"), "{err}");
    }
}
