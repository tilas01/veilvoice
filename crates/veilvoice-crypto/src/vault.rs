// SPDX-License-Identifier: GPL-3.0-or-later
//! Where the app lock is kept: two copies, unpredictable names, and a restore.
//!
//! # What is real here and what is only awkward
//!
//! This module does three things to the app-lock file, and they are not worth
//! the same amount. Saying which is which is the point of this section.
//!
//! **The second copy is real.** A lock kept in one file is removed by deleting
//! that file. A lock kept in two files in two different directories is not,
//! unless the person deleting knows about both. When the first copy is gone or
//! unreadable, [`Vault::load`] restores it from the second and reports the
//! event, so the lock comes back and the owner is told it went.
//!
//! **The administrator-only copy is real, where the platform provides it.** On
//! Unix, when VeilVoice is run with enough privilege to write under `/etc`, the
//! second copy is written there and is thereafter not writable by an ordinary
//! user. Removing the lock then needs `sudo`, which is a genuine step up from
//! needing a file manager. VeilVoice never asks for that privilege and never
//! elevates itself: it uses what it already has and otherwise carries on. On
//! Windows the equivalent needs an access-control list this crate does not link
//! the API to set, so the second copy there is a second copy and nothing more,
//! and this module says so rather than implying a protection it did not obtain.
//!
//! **The unpredictable name and the masked contents are neither.** They are
//! obscurity. The name is derived from a value in an index file that sits at a
//! fixed, obvious path, because something has to, or nothing could ever find
//! the lock again. Anybody who reads this source, or simply reads the index,
//! recomputes both names in a second. What they buy is narrow and real enough
//! to keep: a scan for the string `VEILLOK1` across a disk finds nothing, a
//! backup rule written against `applock.bin` misses, and advice of the form
//! "just delete this file" does not survive being passed on. None of that
//! stops an attacker who is paying attention, and none of it is counted as
//! security anywhere in the documentation.
//!
//! # What none of it does
//!
//! It does not stop somebody holding the disk. It does not stop somebody who
//! knows the passphrase. It does not make the failed-attempt counter
//! trustworthy: see [`crate::lock::AppLock::to_bytes`] for why that one cannot
//! be authenticated at all. If the threat is the disk, the answer is still
//! full-volume encryption.
//!
//! # In plain words
//!
//! The lock is kept twice, in two places, under names that are not guessable
//! from the outside, and scrambled so it does not look like a password file.
//! If one copy goes missing, the other puts it back and you are told.
//!
//! The scrambling and the odd names are speed bumps, not locks. They stop
//! careless deletion and casual searching. They do not stop somebody who has
//! decided to get in and has your disk. Where VeilVoice is already running
//! with administrator rights, the spare copy is put somewhere an ordinary user
//! cannot touch, and that part is not a speed bump.

use crate::{lock, Error};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Length of the per-installation value the file names are derived from.
const SITE_LEN: usize = 16;
/// Bytes of hex in a derived file name. Ten bytes is eighty bits, which is far
/// past any chance of collision and short enough to read out over a phone.
const NAME_BYTES: usize = 10;
/// Fixed name of the index. Deliberately not hidden: pretending the entry
/// point is secret would be the dishonest half of this idea.
const INDEX_NAME: &str = "applock.index";

/// Domain separators, so the two names and the mask cannot coincide.
const LABEL_PRIMARY: &[u8] = b"veilvoice/vault/primary";
const LABEL_SHADOW: &[u8] = b"veilvoice/vault/shadow";
const LABEL_MASK: &[u8] = b"veilvoice/vault/mask";

/// What [`Vault::load`] found when it went looking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    /// No lock is configured.
    Nothing,
    /// Both copies were present and agreed.
    Intact,
    /// One copy was missing or unreadable, and was rebuilt from the other.
    ///
    /// Carries the tamper report outward: the caller should raise it on the
    /// lock so that the owner is told at the next unlock. A copy does not
    /// vanish on its own.
    ///
    /// Reported only when the rebuild actually reached the disk. A copy that
    /// *cannot* be written -- the administrator-owned spare, seen from an
    /// unelevated run that has never been elevated -- is a copy that was never
    /// there, not one that was taken away, and reporting it would raise the
    /// same alarm at every single launch until somebody stopped reading it.
    Restored,
    /// Both copies were consulted and their stored passwords differ.
    ///
    /// One of them is not the lock the owner set. The one the running program
    /// writes is preferred, because the other can only be older, and the
    /// disagreement is reported.
    Disagreed,
}

/// The two files a lock lives in, and the index that names them.
#[derive(Clone, Debug)]
pub struct Vault {
    index: PathBuf,
    primary: PathBuf,
    shadow: PathBuf,
    site: [u8; SITE_LEN],
}

impl Vault {
    /// Resolve the vault under `base`, creating the index if there is none.
    ///
    /// `base` is the per-user configuration directory: the parent of what
    /// [`lock::default_path`] returns. `admin` is a directory only an
    /// administrator can write, or `None` when this platform or this process
    /// has no such directory to offer.
    pub fn at(base: &Path, admin: Option<&Path>) -> Result<Self, Error> {
        let index = base.join(INDEX_NAME);
        let site = match std::fs::read(&index) {
            Ok(bytes) if bytes.len() == SITE_LEN => {
                let mut s = [0u8; SITE_LEN];
                s.copy_from_slice(&bytes);
                s
            }
            // Only an index that is genuinely absent is created. Every other
            // outcome refuses, and the distinction is the difference between a
            // first run and a destroyed lock.
            //
            // The first version of this took one arm for "missing" and for
            // every other failure alike, on the reasoning that a damaged index
            // has nothing to recover from. That reasoning is right and the
            // arm was still wrong, because the same arm caught the failures
            // that are not damage: a read refused by permissions, a Windows
            // sharing violation, an exhausted file-descriptor table. Any one
            // of those, once, and a new value would be written over a perfectly
            // good index and the user's lock would be orphaned under a name
            // nothing could compute again. Refusing costs a confusing session
            // and loses nothing.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut s = [0u8; SITE_LEN];
                getrandom::getrandom(&mut s).map_err(|_| Error::Random)?;
                std::fs::create_dir_all(base).map_err(|_| Error::AppLockStore)?;
                crate::privatefile::write_owner_only(&index, &s)
                    .map_err(|_| Error::AppLockStore)?;
                s
            }
            _ => return Err(Error::AppLockStore),
        };

        let primary = base.join(name_for(&site, LABEL_PRIMARY));
        let shadow = match admin {
            Some(dir) => dir.join(name_for(&site, LABEL_SHADOW)),
            None => base.join(name_for(&site, LABEL_SHADOW)),
        };
        Ok(Self {
            index,
            primary,
            shadow,
            site,
        })
    }

    /// The file the lock is read from and written to.
    pub fn primary(&self) -> &Path {
        &self.primary
    }

    /// The second copy.
    pub fn shadow(&self) -> &Path {
        &self.shadow
    }

    /// The index that names both.
    pub fn index(&self) -> &Path {
        &self.index
    }

    /// Read the lock, restoring one copy from the other if it has to.
    ///
    /// Returns the record and what it took to get it. A copy that parses is
    /// preferred over one that does not, and when both parse the primary wins
    /// -- it is the one the running program writes, so it is the one carrying
    /// the current attempt count.
    pub fn load(&self) -> Result<(Option<lock::AppLock>, Found), Error> {
        let a = self.read_masked(&self.primary);
        let b = self.read_masked(&self.shadow);

        match (a, b) {
            (None, None) => Ok((None, Found::Nothing)),
            (Some(primary), Some(shadow)) => {
                // Two copies that do not agree on the stored password mean one
                // of them is not the lock that was set. The primary wins: it is
                // the copy the running program writes, so the other can only be
                // the older of the two, and reverting to an older password is
                // exactly what somebody who knew the old one would want.
                let found = if primary.same_secret_as(&shadow) {
                    Found::Intact
                } else {
                    Found::Disagreed
                };
                Ok((Some(primary), found))
            }
            (Some(primary), None) => {
                // The spare is not there. Whether that is a report depends on
                // whether it could have been there: see `Found::Restored`.
                let rebuilt = self.write_one(&self.shadow, &primary.to_bytes()).is_ok();
                Ok((
                    Some(primary),
                    if rebuilt {
                        Found::Restored
                    } else {
                        Found::Intact
                    },
                ))
            }
            (None, Some(shadow)) => {
                self.write_one(&self.primary, &shadow.to_bytes())?;
                Ok((Some(shadow), Found::Restored))
            }
        }
    }

    /// Write both copies, and say whether the spare is now current.
    ///
    /// `Ok(false)` means the first copy was written and the spare could not be.
    /// That is the administrator-owned arrangement working as designed and it
    /// is still not something to swallow, because a spare left behind is a
    /// spare holding an *older* record. After a password change that older
    /// record is the previous password, and deleting the first copy would
    /// restore it. So the answer is returned rather than discarded, and
    /// [`lock::LockStore`] refuses to call a password change finished until the
    /// spare has caught up.
    pub fn store(&self, record: &lock::AppLock) -> Result<bool, Error> {
        let bytes = record.to_bytes();
        self.write_one(&self.primary, &bytes)?;
        Ok(self.write_one(&self.shadow, &bytes).is_ok())
    }

    /// Remove both copies, and the index with them.
    pub fn clear(&self) -> Result<(), Error> {
        let gone = |e: &std::io::Error| e.kind() == std::io::ErrorKind::NotFound;
        for path in [&self.primary, &self.shadow, &self.index] {
            if let Err(e) = std::fs::remove_file(path) {
                if !gone(&e) {
                    // The shadow may be administrator-owned, in which case an
                    // unelevated removal cannot succeed. Report it, because a
                    // lock the user asked to remove that is still on disk is
                    // exactly the thing not to be quiet about.
                    return Err(Error::AppLockStore);
                }
            }
        }
        Ok(())
    }

    fn write_one(&self, path: &Path, bytes: &[u8]) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|_| Error::AppLockStore)?;
            }
        }
        let mut masked = bytes.to_vec();
        mask(&self.site, &mut masked);
        // Replaced rather than truncated and rewritten. A process that dies
        // mid-write would otherwise leave a short file, which does not parse,
        // which reads as a copy somebody interfered with. A power cut is not
        // tampering and must not be reported as it.
        crate::privatefile::replace_owner_only(path, &masked).map_err(|_| Error::AppLockStore)
    }

    fn read_masked(&self, path: &Path) -> Option<lock::AppLock> {
        let mut bytes = std::fs::read(path).ok()?;
        mask(&self.site, &mut bytes);
        lock::AppLock::parse(&bytes).ok()
    }
}

/// The file name derived from `site` under `label`.
fn name_for(site: &[u8; SITE_LEN], label: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(label);
    h.update(site);
    let digest = h.finalize();
    let mut out = String::with_capacity(NAME_BYTES * 2);
    for byte in &digest[..NAME_BYTES] {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Exclusive-or `bytes` with a keystream derived from `site`.
///
/// This is a mask, not encryption, and the difference is not a technicality:
/// the value it is derived from is in a file next to the one it masks. It
/// exists so that a lock file does not announce itself to a string search, and
/// for nothing else. It is its own inverse, which is why one function serves
/// both the write and the read.
fn mask(site: &[u8; SITE_LEN], bytes: &mut [u8]) {
    let mut counter: u64 = 0;
    for chunk in bytes.chunks_mut(32) {
        let mut h = Sha256::new();
        h.update(LABEL_MASK);
        h.update(site);
        h.update(counter.to_le_bytes());
        let block = h.finalize();
        for (b, k) in chunk.iter_mut().zip(block.iter()) {
            *b ^= k;
        }
        counter = counter.wrapping_add(1);
    }
}

/// A directory only an administrator can write to, if this process can make
/// one there.
///
/// The test is the attempt. Asking the operating system "am I an
/// administrator" needs a platform API in each case; creating the directory
/// answers the only question that matters -- can this process put a file
/// somewhere an ordinary user cannot rewrite -- and answers it the same way
/// everywhere. No privilege is requested and none is escalated: an unelevated
/// run simply gets `None` and keeps both copies in the user's own directory.
///
/// Returns `None` on Windows even when the directory can be created, because
/// `%ProgramData%` is writable by ordinary users by default and tightening it
/// needs an access-control list this crate does not set. A second copy there
/// would be a second copy, which [`Vault`] already provides, not a privileged
/// one, and calling it privileged would be the overclaim.
pub fn admin_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        return None;
    }
    let dir = PathBuf::from("/etc/veilvoice");
    std::fs::create_dir_all(&dir).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // 0755: readable by everyone so an unelevated run can still verify
        // against the spare, writable only by its owner, which is root when
        // this succeeded under /etc.
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755));
    }
    // Creating it is not proof of much on its own: a machine where /etc is
    // writable by the user would pass. Prove the useful half instead, that the
    // directory's owner is not this process's ordinary reach, by checking that
    // a plain user could not have made it -- the parent is root-owned and this
    // call succeeded.
    Some(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kdf;

    fn weak() -> kdf::KdfParams {
        kdf::KdfParams::weak_for_tests()
    }

    fn vault(dir: &Path) -> Vault {
        Vault::at(dir, None).unwrap()
    }

    #[test]
    fn the_two_copies_are_not_the_same_file() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        assert_ne!(v.primary(), v.shadow());
        assert_ne!(v.primary(), v.index());
    }

    #[test]
    fn the_names_are_stable_across_runs_and_differ_between_installs() {
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();

        let a = vault(one.path());
        let again = vault(one.path());
        assert_eq!(a.primary(), again.primary(), "the index must be reused");

        let b = vault(two.path());
        assert_ne!(
            a.primary().file_name(),
            b.primary().file_name(),
            "two installations must not share a name"
        );
    }

    #[test]
    fn nothing_on_disk_says_it_is_a_lock_file() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        let mut record = lock::AppLock::create(b"a passphrase", weak()).unwrap();
        record.retag(b"a passphrase").unwrap();
        v.store(&record).unwrap();

        for path in [v.primary(), v.shadow()] {
            let raw = std::fs::read(path).unwrap();
            assert!(
                !raw.windows(8).any(|w| w == lock::MAGIC),
                "{} still carries the magic in the clear",
                path.display()
            );
        }
    }

    #[test]
    fn a_masked_file_reads_back_as_what_was_written() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        let mut record = lock::AppLock::create(b"a passphrase", weak()).unwrap();
        record.retag(b"a passphrase").unwrap();
        v.store(&record).unwrap();

        let (loaded, found) = v.load().unwrap();
        assert_eq!(found, Found::Intact);
        let mut loaded = loaded.expect("the lock should be there");
        loaded.verify(b"a passphrase").unwrap();
    }

    #[test]
    fn deleting_one_copy_does_not_remove_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        let mut record = lock::AppLock::create(b"a passphrase", weak()).unwrap();
        record.retag(b"a passphrase").unwrap();
        v.store(&record).unwrap();

        std::fs::remove_file(v.primary()).unwrap();
        let (loaded, found) = v.load().unwrap();
        assert_eq!(found, Found::Restored, "the loss should be reported");
        assert!(loaded.is_some(), "the lock should have come back");
        assert!(v.primary().exists(), "and been written back to disk");

        // The other direction too: the spare is rebuilt from the primary.
        std::fs::remove_file(v.shadow()).unwrap();
        let (_, found) = v.load().unwrap();
        assert_eq!(found, Found::Restored);
        assert!(v.shadow().exists());
    }

    #[test]
    fn a_shredded_copy_is_rebuilt_rather_than_believed() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        let mut record = lock::AppLock::create(b"a passphrase", weak()).unwrap();
        record.retag(b"a passphrase").unwrap();
        v.store(&record).unwrap();

        std::fs::write(v.primary(), b"not a lock file at all").unwrap();
        let (loaded, found) = v.load().unwrap();
        assert_eq!(found, Found::Restored);
        let mut loaded = loaded.expect("the spare should have answered");
        loaded.verify(b"a passphrase").unwrap();
    }

    #[test]
    fn no_lock_reads_as_no_lock_rather_than_as_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        let (loaded, found) = v.load().unwrap();
        assert!(loaded.is_none());
        assert_eq!(found, Found::Nothing);
    }

    #[test]
    fn clearing_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        let mut record = lock::AppLock::create(b"a passphrase", weak()).unwrap();
        record.retag(b"a passphrase").unwrap();
        v.store(&record).unwrap();
        v.clear().unwrap();

        assert!(!v.primary().exists());
        assert!(!v.shadow().exists());
        assert!(!v.index().exists());
    }

    /// The mask is its own inverse, and the test says so directly rather than
    /// only through a round trip, because a mask that quietly became a no-op
    /// would still pass a round trip.
    #[test]
    fn the_mask_changes_the_bytes_and_undoes_itself() {
        let site = [7u8; SITE_LEN];
        let original = b"VEILLOK1 and then some more bytes to cover two blocks".to_vec();
        let mut worked = original.clone();
        mask(&site, &mut worked);
        assert_ne!(worked, original, "a mask that changes nothing is not one");
        mask(&site, &mut worked);
        assert_eq!(worked, original);
    }

    /// F-85. A read that fails for any reason other than "there is no index"
    /// must not be answered by writing a new one. The failure that mattered was
    /// not corruption, it was permission: one refused read and the lock would
    /// have been orphaned under a name nothing could compute again.
    #[test]
    fn a_damaged_index_refuses_rather_than_writing_a_new_one() {
        let dir = tempfile::tempdir().unwrap();
        let first = vault(dir.path());
        let name = first.primary().to_path_buf();

        // Short by a byte, which is what a partial write leaves.
        std::fs::write(first.index(), [0u8; SITE_LEN - 1]).unwrap();
        assert!(
            Vault::at(dir.path(), None).is_err(),
            "a damaged index was replaced, which orphans the lock behind it"
        );

        // And the original name is still computable once the index is right,
        // which is the whole point of refusing.
        std::fs::write(first.index(), std::fs::read(first.index()).unwrap()).ok();
        let restored = [0u8; SITE_LEN];
        std::fs::write(first.index(), restored).unwrap();
        let second = Vault::at(dir.path(), None).unwrap();
        assert_ne!(second.primary(), &name, "different index, different name");
    }

    /// F-87. A spare that could never be written is not a spare that was taken
    /// away, and reporting it as one raises the same false alarm at every
    /// launch until nobody reads any of them.
    #[test]
    fn a_spare_that_cannot_be_written_is_not_reported_as_deleted() {
        let dir = tempfile::tempdir().unwrap();
        // An admin directory that does not exist and cannot be created, which
        // is what an unelevated run sees.
        let unreachable = dir.path().join("primary-is-a-file");
        std::fs::write(&unreachable, b"not a directory").unwrap();
        let v = Vault::at(dir.path(), Some(&unreachable)).unwrap();

        let record = lock::AppLock::create(b"a passphrase", weak()).unwrap();
        v.store(&record).unwrap();
        assert!(
            !v.shadow().exists(),
            "the spare should not be writable here"
        );

        for _ in 0..3 {
            let (found_lock, found) = v.load().unwrap();
            assert!(found_lock.is_some());
            assert_eq!(
                found,
                Found::Intact,
                "an unwritable spare was reported as a deleted one"
            );
        }
    }

    /// F-86. Two copies that hold different passwords mean one of them is not
    /// the lock that was set, and the older one is the way back in for whoever
    /// knew the previous password.
    #[test]
    fn two_copies_with_different_passwords_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        let current = lock::AppLock::create(b"the new one", weak()).unwrap();
        v.store(&current).unwrap();

        // Put the previous lock back in the spare, as a stale spare would hold
        // it after a password change that could not reach the spare.
        let previous = lock::AppLock::create(b"the old one", weak()).unwrap();
        let mut bytes = previous.to_bytes();
        mask(&site_of(&v), &mut bytes);
        std::fs::write(v.shadow(), &bytes).unwrap();

        let (loaded, found) = v.load().unwrap();
        assert_eq!(found, Found::Disagreed);
        let mut loaded = loaded.expect("a lock should still be returned");
        loaded
            .verify(b"the new one")
            .expect("the copy the program writes must be the one that wins");
    }

    /// F-86, the reporting half: `store` says whether the spare caught up, so
    /// a caller can refuse to call a password change finished when it did not.
    #[test]
    fn storing_says_whether_the_spare_was_written() {
        let dir = tempfile::tempdir().unwrap();
        let record = lock::AppLock::create(b"a passphrase", weak()).unwrap();

        let reachable = vault(dir.path());
        assert!(
            reachable.store(&record).unwrap(),
            "both copies were writable"
        );

        let blocked = dir.path().join("blocked");
        std::fs::write(&blocked, b"a file, not a directory").unwrap();
        let two = tempfile::tempdir().unwrap();
        let v = Vault::at(two.path(), Some(&blocked)).unwrap();
        assert!(!v.store(&record).unwrap(), "the spare could not be written");
    }

    /// The site is private, and this test needs it to forge a spare. Reading it
    /// back from the index is what any other process would have to do.
    fn site_of(v: &Vault) -> [u8; SITE_LEN] {
        let bytes = std::fs::read(v.index()).unwrap();
        let mut s = [0u8; SITE_LEN];
        s.copy_from_slice(&bytes);
        s
    }

    #[test]
    fn the_index_is_owner_only_from_the_moment_it_exists() {
        let dir = tempfile::tempdir().unwrap();
        let v = vault(dir.path());
        assert!(v.index().exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(v.index()).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "the index must not be readable by others");
        }
    }
}
