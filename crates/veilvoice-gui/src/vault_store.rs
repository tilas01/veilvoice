// SPDX-License-Identifier: GPL-3.0-or-later
//! Where the desktop application keeps its own files, and what the app lock
//! buys for them.
//!
//! # The short version, and it is the honest one
//!
//! **With an app lock set**, everything VeilVoice writes about itself lives in
//! [`veilvoice_crypto::hoard`]: encrypted, padded to a few fixed sizes, under
//! filenames derived from the lock passphrase, with decoy files sown among
//! them. Somebody who opens the folder without the passphrase cannot tell
//! which file is the settings, which is the integrity record, which holds
//! anything, and which is junk.
//!
//! **With no app lock**, none of that is possible and none of it is claimed.
//! There is no passphrase, so there is no key, so there is nothing to derive a
//! name from or encrypt with. The files sit in the open under their own names,
//! exactly as they did before this module existed, and the security tab says
//! so in those words.
//!
//! That is the whole bargain, and it is the answer to a fair question about
//! the app lock: what is it actually *for*, if it is only a password prompt on
//! a window whose files anybody can read? This is what it is for. Setting a
//! passphrase is what turns the folder from a set of labelled files into a set
//! of indistinguishable ones.
//!
//! # What it still does not buy
//!
//! Repeated here rather than left in the crypto crate, because this is the
//! module the application calls and the place somebody looks:
//!
//! - It does not hide that VeilVoice is installed. The folder is named
//!   `veilvoice` and the lock file is in it under its own name -- it has to
//!   be, since it is what checks the passphrase.
//! - It does not stop anybody deleting the folder.
//! - It is no protection at all while the application is open and unlocked.
//! - Anybody who has the passphrase has everything.
//!
//! # Moving in, and the risk that comes with it
//!
//! The first unlock after a lock is set migrates the existing plain files in:
//! each is read, written as a hoard record, and the original securely erased.
//!
//! This is the moment to be plain about a consequence that is easy to
//! under-state. Once the files are in the hoard, **the passphrase is the only
//! way back to them**. Losing it does not lock you out of a window whose files
//! you could still read by hand; it loses the settings, the integrity record
//! and the policies for good. [`veilvoice_crypto::lock`] keeps a second copy of
//! the lock for exactly this reason, and the setup screen says the sentence out
//! loud rather than burying it.

use std::path::{Path, PathBuf};

use veilvoice_crypto::hoard::{Audit, Hoard, StoreKey};

/// The logical names of every record the application keeps.
///
/// Named here rather than spelled at each call site so that the migration, the
/// audit and the readers cannot disagree about what exists. A record not in
/// this list is not migrated and not audited.
///
/// # What is deliberately not here, and why
///
/// `settings.conf` is **not** an obfuscated record, and this is the one place
/// that decision is explained rather than assumed.
///
/// The settings file says which theme to use, how big the window was, and
/// whether movement is reduced. All three are needed to draw the window --
/// including the lock screen itself, which is the first thing drawn and the
/// last thing that could wait. A record inside the hoard cannot be read until
/// a passphrase has produced a key, and a passphrase cannot be typed until
/// there is a window to type it into. Putting the settings in there would mean
/// every locked VeilVoice opened with the default theme at the default size
/// and then jumped to the user's when they unlocked.
///
/// So the settings stay in the open, and what that costs is stated plainly:
/// somebody reading the folder learns which theme you chose, roughly how big
/// your window is, and which optional features you switched on. They do not
/// learn what you have processed, what VeilVoice measured while doing it, what
/// it found when it checked itself, or what any of the other records hold.
///
/// This is a real limit rather than a temporary one. Closing it properly means
/// splitting appearance out from everything else so only the handful of
/// drawing settings sit in the open, and that is worth doing; it is not worth
/// pretending is already done.
pub mod records {
    /// What the application measured about its own running.
    pub const MEASURED: &str = "measured";

    /// Every record, with the plain filename each one migrates from.
    ///
    /// The plain name is what the file was called before there was a lock, and
    /// is what it goes back to being if the lock is removed.
    ///
    /// # Adding to this list is not free
    ///
    /// A name here is migrated in on the first unlock and the plain file is
    /// **shredded**. So a file listed here that anything still reads by its
    /// plain path is a file that gets destroyed, silently, on somebody's next
    /// unlock. `every_record_is_actually_read_through_the_store` refuses that,
    /// and it exists because the first version of this module listed three
    /// records and read one.
    ///
    /// `integrity.manifest` and `last-crash.txt` were on this list and are
    /// deliberately not now. Both are still read elsewhere by their plain
    /// paths, and the manifest is read by `veilvoice guard` from the *command
    /// line*, which has no unlocked session and therefore no key. Moving it in
    /// would mean prompting for the app-lock passphrase on every
    /// `veilvoice guard` run: a different feature with a different argument,
    /// not a detail of this one.
    pub const ALL: &[(&str, &str)] = &[(MEASURED, "measured.dat")];
}

/// How many decoys a folder is kept stocked with.
///
/// Enough that the real records are a minority of what is there, few enough
/// that the folder is not absurd. The exact number is not a security
/// parameter: it blurs the count of real records, it does not hide it, and
/// pretending otherwise would be the overstatement this project spends its
/// time avoiding.
const DECOYS: usize = 24;

/// The application's own storage, locked or not.
#[derive(Default)]
pub struct VaultStore {
    /// The directory everything lives in, whether obfuscated or not.
    dir: Option<PathBuf>,
    /// The obfuscated store, once a passphrase has produced its key.
    hoard: Option<Hoard>,
    /// What the last audit found, for the security tab to show.
    audit: Option<Audit>,
}

impl VaultStore {
    /// Point at the program folder. Nothing is read or written yet.
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            hoard: None,
            audit: None,
        }
    }

    /// The program folder, if this platform has one.
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Whether records are currently obfuscated.
    ///
    /// False before an unlock and false when there is no lock at all, which
    /// are different situations with the same answer: in neither of them is
    /// anything hidden.
    pub fn is_obfuscated(&self) -> bool {
        self.hoard.is_some()
    }

    /// Take the key from an unlock and open the hoard with it.
    ///
    /// Migrates any plain files in on the way, and tops the decoys up. Returns
    /// what the audit found, so the caller can put a tamper report in front of
    /// somebody who has just proved they own the machine.
    pub fn unlocked(&mut self, key: StoreKey) -> Result<Audit, String> {
        let Some(dir) = self.dir.clone() else {
            return Err("this system has no program folder".to_string());
        };
        let hoard = Hoard::open(&dir, key);

        for (logical, plain) in records::ALL {
            let path = dir.join(plain);
            if !path.exists() {
                continue;
            }
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("could not read {plain} to move it in: {e}"))?;
            hoard
                .write(logical, &bytes)
                .map_err(|e| format!("could not store {plain}: {e}"))?;
            // Erased rather than deleted: a settings file that says which
            // vault you use should not be recoverable from free space after
            // VeilVoice has told you it is now encrypted.
            let _ =
                veilvoice_crypto::shred::shred_file(&path, veilvoice_crypto::shred::Passes::Triple);
            let _ = std::fs::remove_file(&path);
        }

        // Kept stocked rather than sown once. A folder whose decoy count never
        // changes while its record count does is a folder that leaks the
        // difference over time.
        let present = hoard.audit().map_err(|e| e.to_string())?;
        if present.unrecognised < DECOYS {
            let _ = hoard.sow_decoys(DECOYS - present.unrecognised);
        }

        let audit = hoard.audit().map_err(|e| e.to_string())?;
        self.audit = Some(audit.clone());
        self.hoard = Some(hoard);
        Ok(audit)
    }

    /// Forget the key. Called when the window locks.
    pub fn locked(&mut self) {
        self.hoard = None;
    }

    /// What the last audit found, if one has run this session.
    pub fn last_audit(&self) -> Option<&Audit> {
        self.audit.as_ref()
    }

    /// Read a record, from the hoard if it is open and from the plain file if
    /// it is not.
    pub fn read(&self, logical: &str) -> Result<Option<Vec<u8>>, String> {
        if let Some(hoard) = &self.hoard {
            return hoard.read(logical).map_err(|e| e.to_string());
        }
        let Some(path) = self.plain_path(logical) else {
            return Ok(None);
        };
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Write a record, obfuscated if there is a key and plain if there is not.
    pub fn write(&self, logical: &str, bytes: &[u8]) -> Result<(), String> {
        if let Some(hoard) = &self.hoard {
            return hoard.write(logical, bytes).map_err(|e| e.to_string());
        }
        let Some(path) = self.plain_path(logical) else {
            return Err("this system has no program folder".to_string());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        veilvoice_crypto::privatefile::write_owner_only(&path, bytes).map_err(|e| e.to_string())
    }

    /// Where a record sits when nothing is obfuscating it.
    fn plain_path(&self, logical: &str) -> Option<PathBuf> {
        let dir = self.dir.as_ref()?;
        let name = records::ALL
            .iter()
            .find(|(key, _)| *key == logical)
            .map(|(_, plain)| *plain)?;
        Some(dir.join(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use veilvoice_crypto::amnesia::Secret;

    fn key(byte: u8) -> StoreKey {
        let mut raw = [byte; 32];
        StoreKey::from_secret(Secret::new(&mut raw))
    }

    #[test]
    fn without_a_lock_it_writes_plain_files_under_their_own_names() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::new(Some(dir.path().to_path_buf()));
        assert!(!store.is_obfuscated());
        store.write(records::MEASURED, b"theme=dark").unwrap();
        assert!(
            dir.path().join("measured.dat").exists(),
            "with no passphrase there is no key, so there is nothing to hide with"
        );
        assert_eq!(
            store.read(records::MEASURED).unwrap().unwrap(),
            b"theme=dark"
        );
    }

    #[test]
    fn unlocking_moves_the_plain_files_in_and_removes_them() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.write(records::MEASURED, b"theme=dark").unwrap();
        assert!(dir.path().join("measured.dat").exists());

        store.unlocked(key(7)).unwrap();
        assert!(store.is_obfuscated());
        assert!(
            !dir.path().join("measured.dat").exists(),
            "the plain copy has to go, or the obfuscation is decoration"
        );
        assert_eq!(
            store.read(records::MEASURED).unwrap().unwrap(),
            b"theme=dark",
            "and the contents have to survive the move"
        );
    }

    #[test]
    fn once_unlocked_no_filename_says_what_it_holds() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.write(records::MEASURED, b"theme=dark").unwrap();
        store.unlocked(key(7)).unwrap();
        store.write(records::MEASURED, b"4 runs").unwrap();

        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            for (_, plain) in records::ALL {
                assert_ne!(&name, plain, "a plain name survived the move");
            }
        }
    }

    #[test]
    fn decoys_outnumber_the_records() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.write(records::MEASURED, b"x").unwrap();
        store.unlocked(key(7)).unwrap();
        let files = std::fs::read_dir(dir.path()).unwrap().count();
        assert!(
            files >= DECOYS,
            "only {files} files: the real ones are not lost in a crowd"
        );
    }

    #[test]
    fn a_second_unlock_does_not_keep_adding_decoys() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.unlocked(key(7)).unwrap();
        let first = std::fs::read_dir(dir.path()).unwrap().count();
        store.locked();
        store.unlocked(key(7)).unwrap();
        let second = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(
            first, second,
            "a folder that grows by two dozen files per unlock is its own signal"
        );
    }

    #[test]
    fn the_wrong_passphrase_finds_nothing_rather_than_erroring() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.write(records::MEASURED, b"theme=dark").unwrap();
        store.unlocked(key(7)).unwrap();

        let mut other = VaultStore::new(Some(dir.path().to_path_buf()));
        other.unlocked(key(9)).unwrap();
        assert_eq!(
            other.read(records::MEASURED).unwrap(),
            None,
            "a different key derives names nothing is stored under"
        );
    }

    #[test]
    fn locking_forgets_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.unlocked(key(7)).unwrap();
        assert!(store.is_obfuscated());
        store.locked();
        assert!(!store.is_obfuscated());
    }

    #[test]
    fn an_edited_record_is_reported_by_the_audit() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.write(records::MEASURED, b"theme=dark").unwrap();
        store.unlocked(key(7)).unwrap();

        let hoard = Hoard::open(dir.path(), key(7));
        let path = hoard.path_for(records::MEASURED).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();

        store.locked();
        let audit = store.unlocked(key(7)).unwrap();
        assert!(!audit.is_clean());
        assert!(audit.tampered.contains(&records::MEASURED.to_string()));
    }

    /// **F-142.** A record that is migrated but never read is a file destroyed.
    ///
    /// The first version of this module listed `integrity.manifest` and
    /// `last-crash.txt` beside `measured.dat`. Migration reads each plain
    /// file, stores it, and shreds the original -- so on the first unlock
    /// after setting an app lock, the integrity baseline `veilvoice guard`
    /// compares against and the crash log the next launch offers to report
    /// would both have been erased, with nothing reading them back. Neither is
    /// read through this store; both are still read by their plain paths, one
    /// of them from a command line that has no key at all.
    ///
    /// The same shape as F-141 -- written through one path, read through
    /// another -- found by auditing for that shape rather than by somebody
    /// losing a file. So it is pinned: a name in `ALL` has to be read through
    /// the store somewhere, or this fails.
    #[test]
    fn every_record_is_actually_read_through_the_store() {
        let sources = [
            include_str!("vault_store.rs"),
            include_str!("app.rs"),
            include_str!("settings.rs"),
            include_str!("security.rs"),
            include_str!("integrity.rs"),
            include_str!("crashlog.rs"),
        ];
        for (logical, plain) in records::ALL {
            let constant = logical.to_uppercase();
            let read = sources.iter().any(|src| {
                src.contains(&format!("records::{constant}"))
                    && (src.contains("store.read") || src.contains("Measured::load"))
            });
            assert!(
                read,
                "{plain:?} is migrated into the store and shredded, and nothing \
                 reads it back through the store. That destroys it. Either read \
                 it through `VaultStore`, or take it out of `records::ALL`."
            );
        }
    }

    #[test]
    fn every_record_has_a_plain_name_to_migrate_from() {
        // A record added to `ALL` without one would silently never migrate.
        for (logical, plain) in records::ALL {
            assert!(!logical.is_empty());
            assert!(!plain.is_empty());
            assert!(plain.contains('.'), "{plain} does not look like a filename");
        }
    }
}

/// What the application measured about its own running, kept between sessions.
///
/// Small on purpose. These are the numbers the About panel shows and then
/// forgets: how long a frame took, which renderer drew it, how much faster
/// than real time the engine ran. Keeping them makes the panel able to say
/// "and it was like this last time too", which is the difference between a
/// number and a measurement.
///
/// It is written through [`VaultStore`], so with an app lock set it is
/// encrypted under a derived name like everything else there, and with no lock
/// it is a plain file called `measured.dat`. Nothing about it is sent anywhere:
/// VeilVoice has no networking crate in its dependency graph and this is not
/// the exception.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Measured {
    /// The best frame time seen, in milliseconds.
    pub frame_ms: f32,
    /// How many times faster than real time the engine last ran.
    pub speed: f32,
    /// How many sessions have been recorded.
    pub sessions: u32,
}

impl Measured {
    /// Read it back, or the defaults if nothing has been recorded.
    ///
    /// A record that does not parse reads as absent rather than as an error.
    /// These are numbers for a panel; a corrupt one is worth losing silently,
    /// and it is emphatically not worth blocking a launch over. A record that
    /// fails to *authenticate* is a different matter and is reported by the
    /// audit, which runs at unlock.
    pub fn load(store: &VaultStore) -> Self {
        let Ok(Some(bytes)) = store.read(records::MEASURED) else {
            return Self::default();
        };
        let Ok(text) = String::from_utf8(bytes) else {
            return Self::default();
        };
        let mut out = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "frame_ms" => out.frame_ms = value.trim().parse().unwrap_or(0.0),
                "speed" => out.speed = value.trim().parse().unwrap_or(0.0),
                "sessions" => out.sessions = value.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
        out
    }

    /// Write it, obfuscated when there is a key and plain when there is not.
    pub fn save(&self, store: &VaultStore) -> Result<(), String> {
        let text = format!(
            "frame_ms = {:.3}\nspeed = {:.2}\nsessions = {}\n",
            self.frame_ms, self.speed, self.sessions
        );
        store.write(records::MEASURED, text.as_bytes())
    }

    /// Fold this session's numbers in.
    ///
    /// The frame time keeps the *best* seen rather than the last: a frame that
    /// took 300 ms because the machine was swapping says nothing about what
    /// VeilVoice costs, and the number people want from this panel is what it
    /// can do rather than what happened once.
    pub fn record(&mut self, frame_ms: f32, speed: f32) {
        if frame_ms > 0.0 && (self.frame_ms <= 0.0 || frame_ms < self.frame_ms) {
            self.frame_ms = frame_ms;
        }
        if speed > 0.0 {
            self.speed = speed;
        }
    }
}

#[cfg(test)]
mod measured_tests {
    use super::*;
    use veilvoice_crypto::amnesia::Secret;

    fn key(byte: u8) -> StoreKey {
        let mut raw = [byte; 32];
        StoreKey::from_secret(Secret::new(&mut raw))
    }

    #[test]
    fn it_round_trips_through_a_plain_folder() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::new(Some(dir.path().to_path_buf()));
        let m = Measured {
            frame_ms: 4.25,
            speed: 98.5,
            sessions: 3,
        };
        m.save(&store).unwrap();
        assert_eq!(Measured::load(&store), m);
    }

    #[test]
    fn it_round_trips_through_an_obfuscated_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.unlocked(key(7)).unwrap();
        let m = Measured {
            frame_ms: 4.25,
            speed: 98.5,
            sessions: 3,
        };
        m.save(&store).unwrap();
        assert_eq!(Measured::load(&store), m);
    }

    #[test]
    fn the_numbers_do_not_appear_in_the_file_once_locked() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = VaultStore::new(Some(dir.path().to_path_buf()));
        store.unlocked(key(7)).unwrap();
        Measured {
            frame_ms: 4.25,
            speed: 98.5,
            sessions: 3,
        }
        .save(&store)
        .unwrap();

        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap();
            assert!(
                !bytes.windows(4).any(|w| w == b"4.25"),
                "a measurement is readable on disk"
            );
        }
    }

    #[test]
    fn nothing_recorded_reads_as_zeroes_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::new(Some(dir.path().to_path_buf()));
        assert_eq!(Measured::load(&store), Measured::default());
    }

    #[test]
    fn a_corrupt_record_reads_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let store = VaultStore::new(Some(dir.path().to_path_buf()));
        store
            .write(records::MEASURED, b"\xff\xfe not text")
            .unwrap();
        assert_eq!(Measured::load(&store), Measured::default());
    }

    #[test]
    fn the_best_frame_time_is_kept_not_the_last() {
        let mut m = Measured::default();
        m.record(8.0, 90.0);
        m.record(4.0, 91.0);
        m.record(300.0, 92.0);
        assert_eq!(m.frame_ms, 4.0, "a swap storm is not a measurement");
        assert_eq!(m.speed, 92.0, "the speed is the latest run, and says so");
    }
}
