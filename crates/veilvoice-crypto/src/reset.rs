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
///
/// [`Keep::Keys`] is the default, and which way round that goes is a decision
/// rather than a detail. A `Keep::default()` meaning "remove the key to the
/// recordings" is the wrong way round for a type whose mistaken value costs
/// somebody their recordings: the safe answer has to be the one a caller
/// arrives at by not thinking about it. The window had it this way round
/// already and the command line did not, which is how the two ended up
/// disagreeing about what `veilvoice reset` with no flags means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Keep {
    /// Nothing. The folder goes back to the state it was in before VeilVoice
    /// first ran.
    Nothing,
    /// The app lock, and with it the key to anything sealed with the app lock.
    #[default]
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
    /// Why this reset will not go ahead, where it will not.
    ///
    /// A plan carrying this has an empty [`Plan::going`], so a front end that
    /// ignores the field still removes nothing, and [`carry_out`] refuses it
    /// outright. It exists because "keep the keys" is a promise, and a reset
    /// that cannot work out which files the keys are in cannot keep it: going
    /// ahead would remove them while the plan said they were staying.
    pub blocked: Option<String>,
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
    let keys = keys_in(dir);
    let mut plan = Plan {
        dir: dir.to_path_buf(),
        going: Vec::new(),
        keeping: Vec::new(),
        beyond_reach: None,
        blocked: None,
    };

    // A promise this reset cannot keep stops it here, with nothing planned, so
    // the only way a front end can get past it is by asking for a reset that
    // keeps nothing. Checked before the folder is even read: there is nothing
    // to describe, because nothing is going to happen.
    if keep == Keep::Keys {
        if let Some(why) = keys.unresolved {
            plan.blocked = Some(why);
            return plan;
        }
    }

    if keep == Keep::Keys {
        for path in &keys.paths {
            if path.exists() && path.starts_with(dir) {
                plan.keeping.push((
                    layout::entry(layout::Item::AppLock).label.to_string(),
                    path.clone(),
                ));
            }
        }
    } else if let Some(outside) = keys.paths.iter().find(|path| !path.starts_with(dir)) {
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
        keys.paths.iter().collect()
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
    // Stated here as well as in `plan_in`, which returns a blocked plan with
    // nothing in it. Two front ends and anything written later all call this,
    // and a refusal that depends on the caller having read a field is not a
    // refusal.
    if let Some(why) = &plan.blocked {
        report.refused.push(why.clone());
        return report;
    }
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

/// Everything "keep the keys" has to keep, or the reason it cannot be worked
/// out.
struct Keys {
    /// The files themselves.
    paths: Vec<PathBuf>,
    /// Why this is not the whole list, where it is not. See [`Plan::blocked`]:
    /// a partial list is the dangerous answer, because the files missing from
    /// it are removed by a reset that said it was keeping them.
    unresolved: Option<String>,
}

/// The app lock's own files: the index, the two copies it names, the decoy
/// record and the single file the vault replaced.
///
/// Resolved through [`vault`] rather than worked out here, because the derived
/// names come from the index and that module is the one place that knows how.
/// The index is read rather than created: `Vault::at` makes one where there is
/// none, and a function that is only *looking* must not leave a new lock index
/// in a folder that had no lock.
///
/// # Why the list is longer than the lock
///
/// Two of these were missing, and both are things somebody who ticked "keep my
/// password" would have lost:
///
/// - **The decoy record.** [`crate::decoy`] keeps it under a derived name, so
///   nothing in [`layout`] names it and it fell into the group of files a
///   reset removes without knowing what they are. The decoy passphrase is part
///   of how the lock behaves, not a vault, so a reset that keeps the lock and
///   removes this leaves a lock whose second passphrase has silently stopped
///   working.
/// - **The file the vault replaced.** [`crate::lock::open_in`] still adopts
///   `applock.bin`, because every user who set a lock through the window in
///   0.1.17 has one and ignoring it would take their lock away (F-141). It is
///   not in the layout either, so the same reset removed the *only* lock those
///   machines have.
fn keys_in(dir: &Path) -> Keys {
    let index = dir.join(layout::entry(layout::Item::AppLock).name);
    let legacy = dir.join(crate::lock::LEGACY_NAME);
    if !index.is_file() {
        // No index. There may still be the file the vault replaced, which is
        // the whole lock on a machine that has not been through a vaulted
        // release, and is therefore exactly what keeping the keys keeps.
        let paths = if legacy.is_file() {
            vec![legacy]
        } else {
            Vec::new()
        };
        return Keys {
            paths,
            unresolved: None,
        };
    }
    let Ok(vault) = vault::Vault::at(dir, vault::admin_dir_present().as_deref()) else {
        // An index that will not read. The derived names cannot be worked out
        // from it, so the two copies and the decoy record cannot be named at
        // all: see `Plan::blocked` for why that refuses the reset rather than
        // keeping the one file it can name.
        return Keys {
            paths: vec![index],
            unresolved: Some(
                "the app lock's index will not read, so the files it names cannot be \
                 worked out. Keeping the keys would mean keeping files this cannot \
                 identify, and removing the rest of the folder would take them with it"
                    .to_string(),
            ),
        };
    };
    let mut paths = vec![
        vault.index().to_path_buf(),
        vault.primary().to_path_buf(),
        vault.shadow().to_path_buf(),
        vault.decoy().to_path_buf(),
    ];
    if legacy.is_file() {
        paths.push(legacy);
    }
    Keys {
        paths,
        unresolved: None,
    }
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
    ///
    /// The app lock's index here is twelve bytes of the words "a named file",
    /// which no vault can parse. That is deliberate and it is also what hid
    /// two defects for as long as it did: every test that asked for
    /// [`Keep::Keys`] went down the branch for an index that will not read,
    /// and never once exercised the branch that resolves a real one. Anything
    /// about what keeping the keys keeps belongs in
    /// [`folder_with_a_real_lock`] instead.
    fn folder() -> tempfile::TempDir {
        // The folder under test is the whole machine as far as these tests are
        // concerned. Without this, a container running as root has an
        // `/etc/veilvoice` and the second copy of the lock lands outside the
        // folder, so the same assertions ask a different question here than on
        // a laptop or a continuous-integration runner.
        vault::pretend_admin_dir(None);
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

    /// Cheap Argon2id, so a test that makes a real lock finishes.
    fn weak() -> crate::kdf::KdfParams {
        crate::kdf::KdfParams::weak_for_tests()
    }

    /// A folder with a **real** app lock in it: a vault, its two copies, a
    /// decoy record, and the file the vault replaced.
    ///
    /// Everything else the layout names is there too, so a plan made here is
    /// the plan a used machine gets. Returns the paths the keys are in, in the
    /// order a reader would name them.
    fn folder_with_a_real_lock() -> (tempfile::TempDir, Vec<PathBuf>) {
        let dir = folder();
        // The twelve-byte index from `folder` has to go first: `create_in`
        // writes a real one, and a reset is being asked about what is there
        // afterwards.
        std::fs::remove_file(dir.path().join(layout::entry(layout::Item::AppLock).name)).unwrap();

        let mut store = crate::lock::create_in(dir.path(), b"the real one", weak()).unwrap();
        let decoys = crate::decoy::Store::at(dir.path()).unwrap();
        decoys
            .set(&mut store, "the real one", "the other one")
            .expect("a decoy is set");
        std::fs::write(dir.path().join(crate::lock::LEGACY_NAME), b"the old lock").unwrap();

        let vault = vault::Vault::at(dir.path(), vault::admin_dir_present().as_deref()).unwrap();
        let paths = vec![
            vault.index().to_path_buf(),
            vault.primary().to_path_buf(),
            vault.shadow().to_path_buf(),
            vault.decoy().to_path_buf(),
            dir.path().join(crate::lock::LEGACY_NAME),
        ];
        for path in &paths {
            assert!(path.is_file(), "{} was not written", path.display());
        }
        (dir, paths)
    }

    /// Not passing a choice keeps the keys.
    ///
    /// Which way round this goes is the decision, not the detail. The window
    /// had it this way and the command line did not, so `veilvoice reset` and
    /// the window's own button meant different things, and the one that
    /// differed was the one that removed the key to the recordings.
    #[test]
    fn the_default_is_to_keep_the_keys() {
        assert_eq!(Keep::default(), Keep::Keys);
    }

    /// Keeping the keys keeps **all** of them.
    ///
    /// The decoy record and the file the vault replaced are kept under derived
    /// or legacy names that [`layout`] does not list, so both fell into the
    /// group of files a reset removes without knowing what they are. Somebody
    /// who ticked "keep my password" lost their decoy passphrase, and on a
    /// machine that had only ever set a lock through the 0.1.17 window they
    /// lost the lock itself.
    #[test]
    fn keeping_the_keys_keeps_the_decoy_and_the_old_lock_file() {
        let (dir, keys) = folder_with_a_real_lock();
        let plan = plan_in(dir.path(), Keep::Keys);
        assert!(plan.blocked.is_none(), "{:?}", plan.blocked);

        for path in &keys {
            assert!(
                plan.keeping.iter().any(|(_, kept)| kept == path),
                "{} is part of the lock and was not kept: {:?}",
                path.display(),
                plan.keeping
            );
            assert!(
                !plan
                    .going
                    .iter()
                    .flat_map(|going| &going.paths)
                    .any(|doomed| doomed == path),
                "{} is both kept and removed",
                path.display()
            );
        }

        // And the reset still happens: this is not a plan that keeps
        // everything because it failed to work anything out.
        assert!(!plan.is_empty(), "nothing at all was planned");
        carry_out(&plan);
        for path in &keys {
            assert!(path.is_file(), "{} went anyway", path.display());
        }
        assert!(
            !dir.path()
                .join(layout::entry(layout::Item::Vaults).name)
                .exists(),
            "the vaults stayed, and they are what starting again means"
        );
    }

    /// A reset that removes the keys removes all of them too.
    ///
    /// The other direction of the same list. A reset asked for the whole
    /// folder that left the decoy record or the old lock file behind would
    /// leave a machine that is not the new install it reports.
    #[test]
    fn removing_the_keys_removes_the_decoy_and_the_old_lock_file() {
        let (dir, keys) = folder_with_a_real_lock();
        let plan = plan_in(dir.path(), Keep::Nothing);
        assert!(plan.keeping.is_empty(), "{:?}", plan.keeping);
        carry_out(&plan);
        for path in &keys {
            assert!(!path.exists(), "{} is still there", path.display());
        }
    }

    /// A machine whose only lock is the file the vault replaced keeps it.
    ///
    /// There is no index to resolve, so the plan cannot be built from the
    /// vault at all, and the answer still has to be "that file is the lock".
    #[test]
    fn the_old_lock_file_alone_is_still_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join(crate::lock::LEGACY_NAME);
        crate::LockStore::create(&legacy, b"from 0.1.17", weak()).unwrap();
        std::fs::write(
            dir.path().join(layout::entry(layout::Item::Settings).name),
            b"{}",
        )
        .unwrap();

        let plan = plan_in(dir.path(), Keep::Keys);
        assert!(plan.blocked.is_none(), "{:?}", plan.blocked);
        assert!(
            plan.keeping.iter().any(|(_, path)| *path == legacy),
            "the only lock this machine has was not kept: {:?}",
            plan.keeping
        );
        carry_out(&plan);
        assert!(legacy.is_file(), "the only lock this machine has went");
        assert!(crate::LockStore::open(&legacy).unwrap().is_some());
    }

    /// A reset that cannot work out where the keys are does not go ahead.
    ///
    /// "Keep the keys" is a promise. An index that will not read means the
    /// derived names cannot be worked out, so the two copies and the decoy
    /// record cannot be told apart from the files a reset removes. Keeping the
    /// one file that can be named and removing the rest would have kept the
    /// promise in the plan and broken it on the disk.
    #[test]
    fn a_lock_that_will_not_read_blocks_the_reset() {
        // `folder`'s index is twelve bytes of text, which is exactly this case.
        let dir = folder();
        let plan = plan_in(dir.path(), Keep::Keys);

        let why = plan.blocked.as_ref().expect("the reset refused");
        assert!(why.contains("will not read"), "{why}");
        assert!(
            plan.is_empty() && plan.keeping.is_empty(),
            "a blocked plan described work it must not do"
        );

        // The half that removes refuses it as well, because two front ends and
        // anything written later all call it, and a refusal that depends on
        // the caller reading a field is not a refusal.
        let before: Vec<PathBuf> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .collect();
        let report = carry_out(&plan);
        assert!(!report.complete(), "a blocked plan reported success");
        assert_eq!(report.removed, Vec::<String>::new());
        assert_eq!(report.refused, vec![why.clone()]);
        for path in before {
            assert!(path.exists(), "{} went", path.display());
        }

        // And the way through is the reset that keeps nothing, which needs
        // none of this worked out.
        let whole = plan_in(dir.path(), Keep::Nothing);
        assert!(whole.blocked.is_none());
        assert!(!whole.is_empty());
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
        let (dir, _) = folder_with_a_real_lock();
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
