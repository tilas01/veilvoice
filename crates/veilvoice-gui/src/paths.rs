// SPDX-License-Identifier: GPL-3.0-or-later
//! Exactly where this copy of VeilVoice is keeping things.
//!
//! # Why the About tab prints these
//!
//! Every one of these locations is derived on the machine it runs on, from the
//! environment the platform provides, and every one of them is different on
//! Windows, macOS and Linux. Somebody backing up a vault, moving a settings
//! file, or wondering why a palette they wrote is not in the picker has one
//! question, and until now the honest answer was "read the source".
//!
//! A guess is worse than nothing here. A person told the wrong directory
//! deletes the wrong directory.
//!
//! # Derived, not listed
//!
//! Each entry calls the function the rest of the application calls, so this
//! panel cannot say one thing while the program does another. The list is not a
//! second description of where things live; it is the same one, read out.
//!
//! A test reads the crate's own source for every `default_path` and
//! `default_dir` and fails naming any this panel does not show, so a location
//! added tomorrow cannot quietly stop being reported.
//!
//! # The one path deliberately not shown
//!
//! The app lock's own files. [`veilvoice_crypto::vault`] gives them names
//! derived from an index rather than a fixed name, which is obscurity and is
//! described as obscurity there: what it buys is that a search of a disk for a
//! known filename misses, and that a backup rule written against one does too.
//! Printing the names in a window would hand both back to anybody standing
//! behind the person reading it. The directory is shown, which is the part that
//! answers "what do I back up".
//!
//! # In plain words
//!
//! The About tab lists the folders VeilVoice is actually using on this
//! computer, so you can find your settings, your vaults and your palettes
//! without guessing.

use std::path::PathBuf;

/// One place VeilVoice keeps something, and what it keeps there.
pub struct Where {
    /// What it is, as the About tab labels it.
    pub label: &'static str,
    /// Where it is, or `None` when this system does not say.
    pub path: Option<PathBuf>,
    /// One line on what lives there, for somebody deciding whether to touch it.
    pub note: &'static str,
}

/// Which of the two arrangements this copy is using, in one sentence.
///
/// Above the list rather than in it, because it is not a path: it is the fact
/// that decides every path below it. Somebody carrying a copy between two
/// computers wants this line before they want any of the others.
///
/// Read from [`veilvoice_crypto::lock::is_portable`] rather than worked out by
/// comparing paths here, so there is one rule for what portable means and this
/// is not a second spelling of it.
pub fn arrangement() -> String {
    if veilvoice_crypto::lock::is_portable() {
        format!(
            "Beside the program, because a folder called {} is there. A copy on \
             a memory stick keeps its settings, its vaults and its lock on the \
             stick.",
            veilvoice_crypto::lock::PORTABLE_DIR
        )
    } else {
        format!(
            "In this platform's own configuration directory. To keep everything \
             beside the program instead, put a folder called {} next to it; to \
             go back, remove that folder. Neither moves what is already there.",
            veilvoice_crypto::lock::PORTABLE_DIR
        )
    }
}

/// Every location, in the order the About tab shows them.
///
/// Ordered from the outside in: the program, then the folder everything else
/// hangs off, then what is in it. Somebody reading down the list sees the shape
/// of the installation rather than an alphabetical pile.
pub fn all() -> Vec<Where> {
    vec![
        Where {
            label: "this program",
            path: std::env::current_exe().ok(),
            note: "the binary running right now, as the system reports it",
        },
        Where {
            label: "settings folder",
            path: veilvoice_crypto::lock::default_dir(),
            note: "everything below is in here, and this is what to back up",
        },
        Where {
            label: "settings",
            path: crate::prefs::default_path(),
            note: "the palette, the motion setting, the device choices",
        },
        Where {
            label: "app lock",
            path: veilvoice_crypto::lock::default_dir(),
            note: "kept here under names derived from an index, which is why \
                   this shows the folder rather than the files",
        },
        Where {
            label: "vaults",
            path: crate::studio::default_dir(),
            note: "the Studio's recordings, and any decoys made beside them",
        },
        Where {
            label: "policies",
            path: crate::policy::default_dir(),
            note: "settings somebody else decided, if any are in force",
        },
        Where {
            label: "palettes",
            path: crate::palettes::default_dir(),
            note: "colour schemes you wrote, read at startup",
        },
        Where {
            label: "crash report",
            path: crate::crashlog::default_path(),
            note: "written only by a failure, and offered on the next launch",
        },
    ]
}

/// Copy a portable copy's state into the platform's own configuration
/// directory, without overwriting anything already there.
///
/// # The question this answers, and why it is a question
///
/// Somebody installing a portable copy has state in two possible places and
/// one of two intentions, and neither can be guessed. A person moving off a
/// memory stick onto their own machine wants it carried over. A person
/// installing on a shared or borrowed machine wants it left exactly where it
/// is, which is the case where guessing wrong copies somebody's vault onto a
/// computer they do not own.
///
/// # Nothing is overwritten and nothing is removed
///
/// A name that already exists at the destination is left alone and reported.
/// Overwriting would destroy the settings and vaults of whoever already uses
/// that account, and the portable copy is not removed either: this is a copy,
/// so both work afterwards and the person decides what to do with the stick.
///
/// Directories are copied whole, which is what a vault is.
pub fn carry_over() -> Result<Vec<String>, String> {
    let from = veilvoice_crypto::lock::default_dir()
        .ok_or_else(|| "there is no folder beside this program to carry over".to_string())?;
    let into = veilvoice_crypto::lock::platform_dir().ok_or_else(|| {
        "this system does not say where an application should keep its files, \
         so there is nowhere to carry it to"
            .to_string()
    })?;
    if from == into {
        return Err("this copy is already keeping its state there".to_string());
    }
    let mut said = copy_new_only(&from, &into)?;
    said.push(format!(
        "the copy beside the program is still there, and this copy still uses \
         it while {} sits next to it",
        veilvoice_crypto::lock::PORTABLE_DIR
    ));
    Ok(said)
}

/// Everything in `from` that is not already in `into`, and a line about each.
///
/// Split from [`carry_over`] so the rule that matters can be tested with two
/// temporary directories: nothing at the destination is ever replaced, and the
/// report says which names were left and why.
fn copy_new_only(from: &std::path::Path, into: &std::path::Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(into).map_err(|e| format!("cannot use {}: {e}", into.display()))?;

    let mut said = Vec::new();
    let mut copied = 0usize;
    // Sorted, so the report reads the same twice and a test can assert on it.
    // A directory listing's order is the filesystem's business.
    let mut names: Vec<std::ffi::OsString> = std::fs::read_dir(from)
        .map_err(|e| format!("cannot read {}: {e}", from.display()))?
        .flatten()
        .map(|e| e.file_name())
        .collect();
    names.sort();

    for name in names {
        let target = into.join(&name);
        if target.exists() {
            said.push(format!(
                "left {}: something of that name is already there",
                name.to_string_lossy()
            ));
            continue;
        }
        copy_into(&from.join(&name), &target)?;
        copied += 1;
    }

    said.insert(
        0,
        match copied {
            0 => "nothing was carried over".to_string(),
            1 => format!("one thing carried over to {}", into.display()),
            n => format!("{n} things carried over to {}", into.display()),
        },
    );
    Ok(said)
}

/// One file or one whole directory, recursively.
///
/// Written out rather than reached for through a crate: it is fifteen lines,
/// and a dependency added to copy a folder is a dependency in a graph this
/// project invites people to read.
fn copy_into(from: &std::path::Path, to: &std::path::Path) -> Result<(), String> {
    let failed = |e: std::io::Error| format!("cannot copy {}: {e}", from.display());
    if from.is_dir() {
        std::fs::create_dir_all(to).map_err(failed)?;
        for entry in std::fs::read_dir(from).map_err(failed)?.flatten() {
            copy_into(&entry.path(), &to.join(entry.file_name()))?;
        }
        return Ok(());
    }
    std::fs::copy(from, to).map_err(failed)?;
    Ok(())
}

#[cfg(test)]
#[path = "paths/tests.rs"]
mod tests;
