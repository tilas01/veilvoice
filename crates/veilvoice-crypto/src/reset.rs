// SPDX-License-Identifier: GPL-3.0-or-later
//! Putting this machine back to a new install.
//!
//! # What a reset is for
//!
//! Somebody has a VeilVoice that is wrong. A policy they no longer want, a
//! settings file that has been edited by hand, a folder full of vaults from a
//! job that is finished, a lock whose passphrase they have decided to change by
//! starting over. Until this existed the answer was "find the folder yourself
//! and delete the right things out of it", and that answer is a person deleting
//! the wrong file.
//!
//! # It works by exclusion, and that is the point
//!
//! A reset that removes a **list** of things leaves behind whatever was added
//! to the folder after the list was written, which is the failure mode of every
//! uninstaller that has ever left half of itself on a disk. So this removes
//! everything in the folder and keeps only what it was asked to keep.
//!
//! [`crate::layout`] is what puts a name on each thing, so somebody can read
//! what they are about to lose. It does not decide what goes. Anything the
//! layout has never heard of is removed too, and described as one of the files
//! VeilVoice writes.
//!
//! # Nothing happens until somebody has read it
//!
//! [`plan`] only looks. It returns what would go, what would stay, how many
//! files and how many bytes, and whether anything in it is **irreplaceable**:
//! recordings, and colour schemes somebody wrote. That last answer is what a
//! front end turns into a typed confirmation, because a settings file has
//! defaults to come back to and a recording does not.
//!
//! [`carry_out`] is the half that removes, and it takes a plan rather than a
//! directory, so the thing removed is the thing that was described.
//!
//! # Keeping the keys
//!
//! [`Keep::Keys`] leaves the app lock exactly as it is. That is the common case
//! and it is not a compromise: somebody resetting a misconfigured interface
//! wants their recordings afterwards, and the recordings are sealed with the
//! lock. Losing it would mean losing them, which is the opposite of what they
//! asked for.
//!
//! What it keeps is the lock and nothing else. The vaults are removed either
//! way, because they are what "start again" means. A person who wants to keep
//! their recordings and their lock is not resetting; they are changing a
//! setting.
//!
//! # What it does not pretend to do
//!
//! **It deletes rather than erases.** The bytes go where deleted bytes go on
//! this filesystem, and [`crate::shred`] explains at length why overwriting
//! them is not the guarantee it sounds like on flash storage. A reset that
//! spent an hour overwriting a vault and still could not promise the blocks
//! were gone would be buying a feeling. Full-volume encryption is the answer to
//! that threat and it always was.
//!
//! **It only touches this folder.** An administrator's copy of the lock, where
//! the platform allowed one, is under `/etc` and an ordinary user cannot remove
//! it. A reset that quietly left it behind would leave a lock that comes back
//! by itself on the next launch, from [`crate::vault::Vault::load`]'s own
//! restore, so the plan reports it by name and says what it needs.
//!
//! # In plain words
//!
//! Puts VeilVoice back to how it was the day you installed it. It shows you
//! everything it is about to remove, with how much of it there is, before it
//! removes anything, and it can leave your password and the recordings' key
//! alone if that is all you wanted.

use crate::{layout, vault, Error};
use std::path::{Path, PathBuf};

/// What a reset leaves behind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Keep {
    /// Nothing. The folder goes back to the state it was in before VeilVoice
    /// first ran.
    #[default]
    Nothing,
    /// The app lock, and with it the key to anything sealed with the app lock.
    Keys,
}

/// One thing a reset would remove, in the words somebody can read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Doomed {
    /// What it is, from [`layout`] where the layout knows it.
    pub label: String,
    /// The line under that.
    pub note: String,
    /// Every path this covers. One, for a named file or folder; many, for the
    /// files VeilVoice writes under derived names.
    pub paths: Vec<PathBuf>,
    /// How many files, counting inside folders.
    pub files: usize,
    /// How much, in bytes.
    pub bytes: u64,
    /// Whether nothing can put it back.
    pub irreplaceable: bool,
}

/// What a reset would do, before it does any of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The folder this is about.
    pub dir: PathBuf,
    /// What would go.
    pub going: Vec<Doomed>,
    /// What would stay, and why it would stay.
    pub keeping: Vec<(String, PathBuf)>,
    /// A copy of the lock this reset cannot reach, where there is one.
    ///
    /// Only ever set when the lock is being removed. See the module note: a
    /// lock left under `/etc` restores itself, so it is reported rather than
    /// silently skipped.
    pub beyond_reach: Option<PathBuf>,
}

impl Plan {
    /// How much would go altogether.
    pub fn bytes(&self) -> u64 {
        self.going.iter().map(|going| going.bytes).sum()
    }

    /// How many files would go altogether.
    pub fn files(&self) -> usize {
        self.going.iter().map(|going| going.files).sum()
    }

    /// Whether any of it is something nothing can put back.
    ///
    /// What a front end asks somebody to type a word about. False for a folder
    /// holding nothing but settings, which is the reset somebody does twice in
    /// an afternoon while working out what they want.
    pub fn irreplaceable(&self) -> bool {
        self.going.iter().any(|going| going.irreplaceable)
    }

    /// Whether there is nothing to do.
    pub fn is_empty(&self) -> bool {
        self.going.is_empty()
    }
}

/// What a reset did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// What went, by label.
    pub removed: Vec<String>,
    /// What would not go, and what the filesystem said about it.
    ///
    /// Reported rather than returned as one error, because a reset that
    /// removed eleven of twelve things has done something and the person needs
    /// to know which one is left.
    pub refused: Vec<String>,
}

impl Report {
    /// Whether everything the plan named is gone.
    pub fn complete(&self) -> bool {
        self.refused.is_empty()
    }
}

/// What this reset would do to the folder this copy is using.
///
/// `Err` only where there is no folder to speak of: a platform that does not
/// say where configuration goes. A folder that is not there yet is not an
/// error, it is an empty plan, because "reset a machine that has never run
/// VeilVoice" is a reasonable thing to ask and the honest answer is "there is
/// nothing to remove".
pub fn plan(keep: Keep) -> Result<Plan, Error> {
    let dir = layout::dir().ok_or(Error::AppLockStore)?;
    Ok(plan_in(&dir, keep))
}

/// The same, for a folder named outright.
///
/// Every test goes through here with a temporary directory, which is the only
/// way to test something that deletes without deleting the machine it is
/// tested on.
pub fn plan_in(dir: &Path, keep: Keep) -> Plan {
    let keys = key_paths(dir);
    let mut plan = Plan {
        dir: dir.to_path_buf(),
        going: Vec::new(),
        keeping: Vec::new(),
        beyond_reach: None,
    };

    if keep == Keep::Keys {
        for path in &keys {
            if path.exists() && path.starts_with(dir) {
                plan.keeping.push((
                    layout::entry(layout::Item::AppLock).label.to_string(),
                    path.clone(),
                ));
            }
        }
    } else if let Some(outside) = keys.iter().find(|path| !path.starts_with(dir)) {
        if outside.exists() {
            plan.beyond_reach = Some(outside.clone());
        }
    }

    // Sorted, so the plan reads the same twice and a test can assert on it. A
    // directory listing's order is the filesystem's business.
    let mut present: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.flatten().map(|entry| entry.path()).collect(),
        // Nothing there is not a failure. See `plan`.
        Err(_) => return plan,
    };
    present.sort();

    let kept: Vec<&PathBuf> = if keep == Keep::Keys {
        keys.iter().collect()
    } else {
        Vec::new()
    };
    let mut derived: Vec<PathBuf> = Vec::new();

    for path in present {
        if kept.iter().any(|keeping| **keeping == path) {
            continue;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        match layout::ALL.iter().find(|entry| entry.name == name) {
            Some(entry) => {
                let (files, bytes) = measure(&path);
                plan.going.push(Doomed {
                    label: entry.label.to_string(),
                    note: entry.note.to_string(),
                    paths: vec![path],
                    files,
                    bytes,
                    irreplaceable: entry.irreplaceable,
                });
            }
            // Not in the layout, so nothing here knows what it is for. It is
            // still removed: see the module note on why a reset works by
            // exclusion. In practice this is the obfuscated store's own
            // records, the decoys sown among them, and anything a later
            // release added without telling this module.
            None => derived.push(path),
        }
    }

    if !derived.is_empty() {
        let (files, bytes) = derived
            .iter()
            .map(|path| measure(path))
            .fold((0usize, 0u64), |(files, bytes), (more, larger)| {
                (files + more, bytes + larger)
            });
        plan.going.push(Doomed {
            label: "VeilVoice's own files".to_string(),
            note: "kept under names derived from your password, with decoy \
                   files among them, so this cannot say which is which and \
                   neither can anybody else"
                .to_string(),
            paths: derived,
            files,
            bytes,
            // The records have no copy anywhere and are not written again from
            // a default: what is in them is what the application measured and
            // what somebody chose while it was unlocked.
            irreplaceable: true,
        });
    }

    // The order the layout gives, which is what it costs to lose, and the
    // derived files last because they are the one entry nobody asked for by
    // name.
    plan.going.sort_by_key(|going| {
        layout::ALL
            .iter()
            .position(|entry| entry.label == going.label)
            .unwrap_or(usize::MAX)
    });
    plan
}

/// Remove what the plan named.
///
/// Takes the plan rather than a directory, so what is removed is what was
/// described to somebody. A path in the plan that has gone in the meantime is
/// not a failure: it was going anyway.
pub fn carry_out(plan: &Plan) -> Report {
    let mut report = Report::default();
    for going in &plan.going {
        let mut failed = Vec::new();
        for path in &going.paths {
            let outcome = if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
            match outcome {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => failed.push(format!("{}: {e}", path.display())),
            }
        }
        if failed.is_empty() {
            report.removed.push(going.label.clone());
        } else {
            for line in failed {
                report.refused.push(line);
            }
        }
    }
    report
}

/// The app lock's own files: the index, and the two copies it names.
///
/// Resolved through [`vault`] rather than worked out here, because the two
/// copies' names are derived from the index and that module is the one place
/// that knows how. The index is read rather than created: `Vault::at` makes one
/// where there is none, and a function that is only *looking* must not leave a
/// new lock index in a folder that had no lock.
fn key_paths(dir: &Path) -> Vec<PathBuf> {
    let index = dir.join(layout::entry(layout::Item::AppLock).name);
    if !index.is_file() {
        return Vec::new();
    }
    let Ok(vault) = vault::Vault::at(dir, vault::admin_dir_present().as_deref()) else {
        // An index that will not read. The lock cannot be resolved, so the
        // index is all that can be named, and it is named: a reset keeping the
        // keys keeps it, and a reset removing them removes it.
        return vec![index];
    };
    vec![
        vault.index().to_path_buf(),
        vault.primary().to_path_buf(),
        vault.shadow().to_path_buf(),
    ]
}

/// How many files and how many bytes, counting inside a folder.
///
/// A count rather than a file listing, because the listing is the part that
/// cannot be shown: the obfuscated store's names say nothing and a vault's
/// contents are somebody's recordings. What a person needs before agreeing is
/// the size of what is going.
fn measure(path: &Path) -> (usize, u64) {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => {
            let Ok(entries) = std::fs::read_dir(path) else {
                return (0, 0);
            };
            entries
                .flatten()
                .map(|entry| measure(&entry.path()))
                .fold((0, 0), |(files, bytes), (more, larger)| {
                    (files + more, bytes + larger)
                })
        }
        // A symbolic link counts as itself and is not followed. Following one
        // would count somebody else's directory into this total and, worse,
        // would be a promise to remove what it points at.
        Ok(meta) => (1, meta.len()),
        Err(_) => (0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder with one of everything named in it, and some things that are
    /// not.
    fn folder() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        for entry in layout::ALL {
            let path = dir.path().join(entry.name);
            if entry.folder {
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(path.join("something"), b"in the folder").unwrap();
            } else {
                std::fs::write(&path, b"a named file").unwrap();
            }
        }
        // What the obfuscated store and the decoys look like from outside: a
        // name that says nothing.
        for name in ["3f2a1b0c9d8e7f605142", "a1b2c3d4e5f60718293a"] {
            std::fs::write(dir.path().join(name), b"derived").unwrap();
        }
        dir
    }

    #[test]
    fn a_folder_that_was_never_used_has_nothing_to_remove() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_in(&dir.path().join("never-existed"), Keep::Nothing);
        assert!(plan.is_empty(), "something was planned for an empty folder");
        assert_eq!(plan.bytes(), 0);
        assert!(!plan.irreplaceable());
    }

    /// Everything in the folder is accounted for, named or not.
    ///
    /// The rule the module exists for. A file nobody listed is still removed,
    /// because a reset that leaves behind whatever a later release added is a
    /// reset that only worked on the day it was written.
    #[test]
    fn nothing_in_the_folder_is_left_out_of_the_plan() {
        let dir = folder();
        let plan = plan_in(dir.path(), Keep::Nothing);

        let planned: Vec<&PathBuf> = plan.going.iter().flat_map(|going| &going.paths).collect();
        for entry in std::fs::read_dir(dir.path()).unwrap().flatten() {
            assert!(
                planned.iter().any(|path| **path == entry.path()),
                "{} is in the folder and not in the plan",
                entry.path().display()
            );
        }
        assert!(plan.keeping.is_empty(), "nothing was to be kept");
    }

    /// A file the layout has never heard of is removed and described honestly.
    #[test]
    fn a_file_nobody_named_is_still_removed() {
        let dir = folder();
        std::fs::write(dir.path().join("something-from-2028.dat"), b"later").unwrap();
        let plan = plan_in(dir.path(), Keep::Nothing);
        let derived = plan
            .going
            .iter()
            .find(|going| going.label == "VeilVoice's own files")
            .expect("the unnamed files are planned as a group");
        assert!(
            derived
                .paths
                .iter()
                .any(|path| path.ends_with("something-from-2028.dat")),
            "a file added later was not in the plan: {:?}",
            derived.paths
        );

        carry_out(&plan);
        assert!(!dir.path().join("something-from-2028.dat").exists());
    }

    #[test]
    fn keeping_the_keys_keeps_the_lock_and_nothing_else() {
        let dir = folder();
        let plan = plan_in(dir.path(), Keep::Keys);
        let index = dir.path().join(layout::entry(layout::Item::AppLock).name);

        assert!(
            plan.keeping.iter().any(|(_, path)| *path == index),
            "the lock's index was not kept: {:?}",
            plan.keeping
        );
        assert!(
            !plan
                .going
                .iter()
                .flat_map(|going| &going.paths)
                .any(|path| *path == index),
            "the lock's index is both kept and removed"
        );
        assert!(
            plan.going
                .iter()
                .any(|going| going.label == layout::entry(layout::Item::Vaults).label),
            "the vaults are what starting again means, so they go either way"
        );

        let report = carry_out(&plan);
        assert!(report.complete(), "something refused: {:?}", report.refused);
        assert!(index.is_file(), "the lock was removed after all");
        assert!(!dir.path().join("studio").exists());
    }

    #[test]
    fn a_full_reset_leaves_the_folder_empty() {
        let dir = folder();
        let plan = plan_in(dir.path(), Keep::Nothing);
        assert!(
            plan.files() > layout::ALL.len(),
            "the folders were not counted into the total"
        );
        assert!(plan.bytes() > 0);

        let report = carry_out(&plan);
        assert!(report.complete(), "something refused: {:?}", report.refused);
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "the folder still has something in it"
        );
    }

    /// The typed-confirmation question has to be about something real.
    #[test]
    fn what_cannot_be_put_back_is_what_is_reported_as_such() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(layout::entry(layout::Item::Settings).name),
            b"a palette and a motion setting",
        )
        .unwrap();
        let settings_only = plan_in(dir.path(), Keep::Nothing);
        assert!(
            !settings_only.irreplaceable(),
            "a folder holding only settings asked for a typed confirmation"
        );

        std::fs::create_dir(dir.path().join(layout::entry(layout::Item::Vaults).name)).unwrap();
        assert!(
            plan_in(dir.path(), Keep::Nothing).irreplaceable(),
            "a folder holding recordings did not ask for one"
        );
    }

    /// A plan is a plan. Nothing is removed and nothing is created by looking.
    #[test]
    fn looking_changes_nothing() {
        let dir = folder();
        let before: Vec<PathBuf> = {
            let mut all: Vec<PathBuf> = std::fs::read_dir(dir.path())
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .collect();
            all.sort();
            all
        };
        for keep in [Keep::Nothing, Keep::Keys] {
            let _ = plan_in(dir.path(), keep);
        }
        let mut after: Vec<PathBuf> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        after.sort();
        assert_eq!(before, after, "planning changed the folder");
    }

    /// A folder with no lock in it does not get one from being planned against.
    ///
    /// `Vault::at` creates an index where there is none, which is right for
    /// every other caller and wrong for this one: a reset that invented a lock
    /// index would leave the folder further from a new install than it found
    /// it.
    #[test]
    fn planning_does_not_create_a_lock_for_a_machine_that_has_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("settings.conf"), b"only settings").unwrap();
        for keep in [Keep::Nothing, Keep::Keys] {
            let _ = plan_in(dir.path(), keep);
            assert!(
                !dir.path()
                    .join(layout::entry(layout::Item::AppLock).name)
                    .exists(),
                "planning created a lock index"
            );
        }
    }

    /// A link is counted as itself and removed as itself.
    ///
    /// Following one would count somebody else's directory into the total a
    /// person is agreeing to and, far worse, would promise to remove whatever
    /// it points at. The same rule [`crate::shred`] keeps, for the same reason.
    #[cfg(unix)]
    #[test]
    fn a_link_in_the_folder_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::fs::write(elsewhere.path().join("not-ours.txt"), b"somebody else's").unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), dir.path().join("a-link")).unwrap();

        let plan = plan_in(dir.path(), Keep::Nothing);
        assert_eq!(plan.files(), 1, "the link's target was counted");

        carry_out(&plan);
        assert!(
            elsewhere.path().join("not-ours.txt").is_file(),
            "the reset removed what a link pointed at"
        );
    }

    /// What went is reported by name, and what would not go is reported too.
    #[test]
    fn a_reset_says_what_it_managed_and_what_it_did_not() {
        let dir = folder();
        let plan = plan_in(dir.path(), Keep::Nothing);
        let expected = plan.going.len();

        // Removed underneath the plan, which is what a second copy of
        // VeilVoice doing the same thing looks like. It was going anyway, so it
        // is not a refusal.
        std::fs::remove_file(dir.path().join(layout::entry(layout::Item::Settings).name)).unwrap();

        let report = carry_out(&plan);
        assert!(
            report.complete(),
            "a file that had already gone was a failure"
        );
        assert_eq!(report.removed.len(), expected);
    }
}
