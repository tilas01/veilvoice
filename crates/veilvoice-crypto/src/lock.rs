// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! The application lock: an Argon2id password verifier with a rate limit.
//!
//! # What this is worth, stated before anything else
//!
//! **This is not a security boundary against someone holding the disk.** It
//! cannot be. A local application has nowhere to hide a secret from the machine
//! it runs on: whoever can read this file can also delete it, and deleting it
//! removes the lock. That is not a defect in the implementation, it is what a
//! local lock *is*.
//!
//! What it does buy is real and worth having: someone who picks up your unlocked
//! computer — a flatmate, a colleague, a border officer with your session open —
//! cannot open VeilVoice, see which files you have processed, or start a live
//! scramble. That is the threat this defends against, and [`SCOPE`] says so in
//! the words the user sees.
//!
//! If the threat is an attacker with the disk, the answer is full-volume
//! encryption (LUKS, BitLocker, FileVault) plus [`crate::container`] for the
//! recordings themselves. Neither of those is replaced by this module.
//!
//! # Why a verifier and not a key
//!
//! The lock stores `Argon2id(domain ‖ password, salt)` and compares it in
//! constant time. It deliberately does **not** derive a key that encrypts
//! anything, because there is nothing here it could usefully encrypt: the
//! recordings have their own password (a *different* one — see
//! [`crate::container`]), and pretending the app lock protected them would be
//! exactly the overclaim this project refuses to make.
//!
//! The stored verifier is therefore a password hash sitting on disk in the
//! clear, and must be treated like one. Argon2id at the default cost (256 MiB,
//! t=3, p=4) makes an offline attack expensive per guess, which matters for a
//! decent passphrase and does not save a bad one.
//!
//! # Rate limiting
//!
//! Wrong attempts are counted and the count is **persisted**, so killing the
//! process does not hand an attacker a fresh budget. After three free attempts
//! the wait doubles — 5 s, 10 s, 20 s … capped at fifteen minutes.
//!
//! The counter is stored in the same unauthenticated file as the verifier, and
//! the wait is measured against the system clock. Someone who can edit the file
//! or move the clock defeats both. Again: casual access, not the disk.
//!
//! # Separate from the recording password
//!
//! The password that unlocks the app and the password that encrypts recordings
//! are two different passwords, on purpose, so that unlocking the app is not the
//! same act as unsealing everything it has ever written. To make that structural
//! rather than merely conventional, the verifier is derived over a domain-
//! separated input, so the same passphrase used in both places still produces
//! unrelated values.

use crate::{kdf, Error, Secret};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// What the app lock protects against, and what it does not, in the words a
/// front-end should show the user.
///
/// Single-sourced so the CLI and the GUI cannot drift into two different
/// promises, and asserted by the tests so it cannot quietly become a boast.
pub const SCOPE: &str = "The app lock keeps someone who picks up your unlocked computer out of \
     VeilVoice. It is not disk encryption and it is not tamper-proof: anyone who \
     can write to this machine's files can delete the lock, and anyone holding \
     the disk can attack the stored password hash offline. If that is the \
     threat, encrypt the whole volume.";

/// Magic bytes at the start of a lock file.
pub const MAGIC: &[u8; 8] = b"VEILLOK1";
/// Format version this build writes.
pub const FORMAT_VERSION: u8 = 1;
/// Exact size of a lock file, in bytes.
pub const LOCK_LEN: usize = 84;

/// Domain separator, so the app-lock verifier can never coincide with a key
/// derived from the same passphrase anywhere else in this crate.
const DOMAIN: &[u8] = b"veilvoice/app-lock/v1\0";

/// Failed attempts allowed before the wait starts.
const FREE_ATTEMPTS: u32 = 3;
/// The first enforced wait, in seconds. It doubles from here.
const BASE_DELAY_SECS: u64 = 5;
/// The longest the wait ever gets. Beyond a quarter of an hour the attacker has
/// long since moved to attacking the file directly, and the honest user is the
/// only one still being punished.
const MAX_DELAY_SECS: u64 = 15 * 60;

/// How long to refuse the next attempt after `failures` consecutive failures.
///
/// Public because it is the whole of the rate-limit policy, and a policy nobody
/// can read or test is not a policy.
pub fn delay_secs(failures: u32) -> u64 {
    let Some(step) = failures.checked_sub(FREE_ATTEMPTS + 1) else {
        return 0;
    };
    // Guard the shift before it can overflow; everything past this is capped
    // anyway.
    if step >= 12 {
        return MAX_DELAY_SECS;
    }
    (BASE_DELAY_SECS << step).min(MAX_DELAY_SECS)
}

/// Seconds since the Unix epoch, negative before it.
fn unix_now() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs().min(i64::MAX as u64) as i64,
        Err(e) => -(e.duration().as_secs().min(i64::MAX as u64) as i64),
    }
}

/// A password verifier plus its attempt history.
///
/// This is the on-disk state. Most callers want [`LockStore`], which ties it to
/// a file and persists every attempt.
#[derive(Clone, Debug)]
pub struct AppLock {
    params: kdf::KdfParams,
    salt: [u8; kdf::SALT_LEN],
    /// Held in a [`Secret`] for the constant-time comparison and the wipe on
    /// drop. Page-locking it is over-caution — the same bytes are on disk — but
    /// costs nothing and keeps one type for key-shaped material.
    verifier: Secret,
    failures: u32,
    /// Unix seconds of the most recent failure, or 0 if there has not been one.
    last_failure: i64,
}

impl AppLock {
    /// Create a lock for `password`.
    pub fn create(password: &[u8], params: kdf::KdfParams) -> Result<Self, Error> {
        let salt = kdf::random_salt()?;
        let verifier = derive_verifier(password, &salt, params)?;
        Ok(Self {
            params,
            salt,
            verifier,
            failures: 0,
            last_failure: 0,
        })
    }

    /// Check `password`, recording the outcome.
    ///
    /// Returns [`Error::AppLockCooldown`] while the rate limit is in force,
    /// without touching the KDF — an attacker should not be able to spend our
    /// CPU either.
    pub fn verify(&mut self, password: &[u8]) -> Result<(), Error> {
        self.verify_at(password, unix_now())
    }

    fn verify_at(&mut self, password: &[u8], now: i64) -> Result<(), Error> {
        if let Some(wait) = self.cooldown_at(now) {
            return Err(Error::AppLockCooldown(wait));
        }
        let candidate = derive_verifier(password, &self.salt, self.params)?;
        if candidate == self.verifier {
            self.failures = 0;
            self.last_failure = 0;
            Ok(())
        } else {
            self.failures = self.failures.saturating_add(1);
            self.last_failure = now;
            Err(Error::AppLockRejected)
        }
    }

    /// Seconds still to wait before another attempt is accepted.
    pub fn cooldown(&self) -> Option<Duration> {
        self.cooldown_at(unix_now()).map(Duration::from_secs)
    }

    fn cooldown_at(&self, now: i64) -> Option<u64> {
        let wait = delay_secs(self.failures);
        if wait == 0 {
            return None;
        }
        // A clock that moved backwards since the last failure gives a negative
        // elapsed time. Treat that as no time having passed rather than as
        // credit against the wait.
        let elapsed = now.saturating_sub(self.last_failure).max(0) as u64;
        (elapsed < wait).then(|| wait - elapsed)
    }

    /// Consecutive failed attempts recorded so far.
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// The Argon2id cost this lock was created with.
    pub fn params(&self) -> kdf::KdfParams {
        self.params
    }

    /// Serialise exactly as it appears on disk.
    ///
    /// ```text
    ///  offset  size  field
    ///       0     8  magic "VEILLOK1"
    ///       8     1  format version (1)
    ///       9     3  reserved, must be zero
    ///      12     4  Argon2id m_cost (KiB, little-endian)
    ///      16     4  Argon2id t_cost
    ///      20     4  Argon2id p_cost
    ///      24    16  salt
    ///      40    32  verifier
    ///      72     4  consecutive failed attempts
    ///      76     8  Unix seconds of the most recent failure
    /// ```
    ///
    /// Nothing here is authenticated, and that is deliberate: any key we could
    /// authenticate it with would have to sit beside it in the same file. A MAC
    /// would look like tamper-proofing without being any, which is worse than
    /// the honest absence of one.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(LOCK_LEN);
        out.extend_from_slice(MAGIC);
        out.push(FORMAT_VERSION);
        out.extend_from_slice(&[0u8; 3]); // reserved
        out.extend_from_slice(&self.params.m_cost.to_le_bytes());
        out.extend_from_slice(&self.params.t_cost.to_le_bytes());
        out.extend_from_slice(&self.params.p_cost.to_le_bytes());
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(self.verifier.expose());
        out.extend_from_slice(&self.failures.to_le_bytes());
        out.extend_from_slice(&self.last_failure.to_le_bytes());
        debug_assert_eq!(out.len(), LOCK_LEN);
        out
    }

    /// Parse a lock file.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < LOCK_LEN {
            return Err(Error::Truncated);
        }
        if bytes.len() > LOCK_LEN {
            return Err(Error::BadHeader);
        }
        if &bytes[..8] != MAGIC {
            return Err(Error::BadMagic);
        }
        if bytes[8] != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(bytes[8]));
        }
        // Refuse a future flag rather than ignore it, as the container does.
        if bytes[9..12] != [0u8; 3] {
            return Err(Error::BadHeader);
        }

        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let params = kdf::KdfParams {
            m_cost: u32_at(12),
            t_cost: u32_at(16),
            p_cost: u32_at(20),
        };
        // Validate the costs here rather than only at the first verification.
        // They are attacker-controlled — this file is read before anyone has
        // authenticated — and `KdfParams::checked` is the single funnel that
        // bounds them (see F-2 and F-3). Refusing at parse time means the
        // failure is reported as "this lock file is broken", which is true and
        // actionable, rather than as a password that never works.
        params.checked()?;
        let mut salt = [0u8; kdf::SALT_LEN];
        salt.copy_from_slice(&bytes[24..40]);

        let mut verifier_bytes = [0u8; kdf::KEY_LEN];
        verifier_bytes.copy_from_slice(&bytes[40..72]);
        let verifier = Secret::new(&mut verifier_bytes);

        let mut last = [0u8; 8];
        last.copy_from_slice(&bytes[76..84]);

        Ok(Self {
            params,
            salt,
            verifier,
            failures: u32_at(72),
            last_failure: i64::from_le_bytes(last),
        })
    }
}

/// Derive the verifier for `password`.
///
/// The domain separator is prepended so that this value cannot collide with a
/// container key derived from the same passphrase and salt. The joined buffer is
/// wiped as soon as it has been consumed.
fn derive_verifier(password: &[u8], salt: &[u8], params: kdf::KdfParams) -> Result<Secret, Error> {
    let mut bound = Vec::with_capacity(DOMAIN.len() + password.len());
    bound.extend_from_slice(DOMAIN);
    bound.extend_from_slice(password);
    let bound = Secret::new(&mut bound);
    kdf::derive_key(bound.expose(), salt, params)
}

/// An [`AppLock`] bound to a file, which is persisted after every attempt.
///
/// Persisting on failure is the point: a rate limit that a process restart
/// clears is not a rate limit.
pub struct LockStore {
    path: PathBuf,
    lock: AppLock,
}

impl LockStore {
    /// Load the lock at `path`, or `Ok(None)` if no lock is configured there.
    ///
    /// A file that exists but does not parse is an error, not an absent lock:
    /// silently treating a corrupt lock as "unlocked" would turn one bad byte
    /// into an open door.
    pub fn open(path: &Path) -> Result<Option<Self>, Error> {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::AppLockStore),
        };
        Ok(Some(Self {
            path: path.to_path_buf(),
            lock: AppLock::parse(&bytes)?,
        }))
    }

    /// Create a lock at `path`, refusing to overwrite one already there.
    ///
    /// The refusal is done by the *creation* rather than by a prior
    /// `path.exists()` test. Checking and then writing is a race, and the two
    /// ways it loses both matter here: another process can win between the two
    /// steps, and a symbolic link planted at the lock path would have been
    /// followed, so `fs::write` would have overwritten whatever it pointed at.
    /// `create_new` asks the kernel to fail if anything is already there,
    /// which is one atomic answer to both.
    pub fn create(path: &Path, password: &[u8], params: kdf::KdfParams) -> Result<Self, Error> {
        let store = Self {
            path: path.to_path_buf(),
            lock: AppLock::create(password, params)?,
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|_| Error::AppLockStore)?;
            }
        }
        write_private(path, &store.lock.to_bytes(), true)?;
        Ok(store)
    }

    /// Check `password` and persist the outcome.
    ///
    /// A failure to write the updated attempt count does not change the verdict
    /// — the attempt really did succeed or fail — so the write is best-effort
    /// here. The consequence of losing it is a rate limit that resets, which is
    /// already true of anyone who can delete the file.
    pub fn unlock(&mut self, password: &[u8]) -> Result<(), Error> {
        let result = self.lock.verify(password);
        // Nothing to persist if we never got as far as an attempt.
        if !matches!(result, Err(Error::AppLockCooldown(_))) {
            let _ = self.save();
        }
        result
    }

    /// Replace the password, after proving the current one.
    pub fn change_password(&mut self, current: &[u8], new: &[u8]) -> Result<(), Error> {
        self.unlock(current)?;
        self.lock = AppLock::create(new, self.lock.params)?;
        self.save()
    }

    /// Remove the lock, after proving the password.
    ///
    /// Proving it is a courtesy to the honest user, not a control: the file can
    /// simply be deleted by anyone who can reach it, which [`SCOPE`] says.
    pub fn remove(mut self, current: &[u8]) -> Result<(), Error> {
        self.unlock(current)?;
        std::fs::remove_file(&self.path).map_err(|_| Error::AppLockStore)
    }

    /// Seconds still to wait before another attempt is accepted.
    pub fn cooldown(&self) -> Option<Duration> {
        self.lock.cooldown()
    }

    /// Consecutive failed attempts recorded so far.
    pub fn failures(&self) -> u32 {
        self.lock.failures()
    }

    /// Where this lock is stored.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn save(&self) -> Result<(), Error> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|_| Error::AppLockStore)?;
            }
        }
        write_private(&self.path, &self.lock.to_bytes(), false)
    }
}

/// Write the lock file so it is owner-only from the moment it exists.
///
/// The previous version used `fs::write` — which creates with the process
/// umask, usually world-readable — and chmod'd afterwards, so the stored
/// password verifier was readable by every other local user for the window
/// between the two calls. That window reopened on **every save**, and a save
/// happens after every failed unlock attempt.
///
/// `exclusive` additionally requires that nothing is already at the path, which
/// is how a lock is created without a check-then-write race and without
/// following a symbolic link planted there. See
/// [`crate::privatefile`] for the whole argument.
fn write_private(path: &Path, bytes: &[u8], exclusive: bool) -> Result<(), Error> {
    let result = if exclusive {
        crate::privatefile::write_owner_only_new(path, bytes)
    } else {
        crate::privatefile::write_owner_only(path, bytes)
    };
    result.map_err(|_| Error::AppLockStore)
}

/// Where the lock file lives on this platform, if the environment says.
///
/// Resolved from environment variables rather than a directories crate: it is
/// twenty lines, it adds no dependency to a security crate, and it returns
/// `None` instead of guessing when the environment does not say. A caller that
/// gets `None` should tell the user it cannot find a config directory rather
/// than scattering a lock file into the working directory.
pub fn default_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else if cfg!(target_os = "macos") {
        let mut p = PathBuf::from(std::env::var_os("HOME")?);
        p.push("Library/Application Support");
        p
    } else {
        match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            Some(v) => PathBuf::from(v),
            None => {
                let mut p = PathBuf::from(std::env::var_os("HOME")?);
                p.push(".config");
                p
            }
        }
    };
    Some(base.join("veilvoice").join("applock.bin"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weak() -> kdf::KdfParams {
        kdf::KdfParams::weak_for_tests()
    }

    #[test]
    fn the_right_password_opens_it_and_a_wrong_one_does_not() {
        let mut lock = AppLock::create(b"open sesame", weak()).unwrap();
        assert!(matches!(
            lock.verify(b"open sesamf"),
            Err(Error::AppLockRejected)
        ));
        assert_eq!(lock.verify(b"open sesame"), Ok(()));
    }

    #[test]
    fn a_success_clears_the_failure_history() {
        let mut lock = AppLock::create(b"pw", weak()).unwrap();
        for _ in 0..3 {
            let _ = lock.verify(b"nope");
        }
        assert_eq!(lock.failures(), 3);
        lock.verify(b"pw").unwrap();
        assert_eq!(lock.failures(), 0);
        assert!(lock.cooldown().is_none());
    }

    /// The rate limit is the whole defence against a script guessing all night,
    /// so its shape is asserted rather than assumed.
    #[test]
    fn the_wait_is_free_then_doubles_then_caps() {
        assert_eq!(delay_secs(0), 0);
        assert_eq!(delay_secs(3), 0, "three attempts are free");
        assert_eq!(delay_secs(4), 5);
        assert_eq!(delay_secs(5), 10);
        assert_eq!(delay_secs(6), 20);
        assert_eq!(delay_secs(20), MAX_DELAY_SECS);
        assert_eq!(delay_secs(u32::MAX), MAX_DELAY_SECS, "must not overflow");
    }

    #[test]
    fn a_rate_limited_attempt_is_refused_without_consulting_the_password() {
        let mut lock = AppLock::create(b"pw", weak()).unwrap();
        for _ in 0..4 {
            let _ = lock.verify_at(b"nope", 1_000);
        }
        // Even the correct password is refused while the wait is in force.
        assert!(matches!(
            lock.verify_at(b"pw", 1_000),
            Err(Error::AppLockCooldown(5))
        ));
        // And once it has elapsed, it opens.
        assert_eq!(lock.verify_at(b"pw", 1_005), Ok(()));
    }

    /// Rolling the clock backwards must not be a way to shorten the wait.
    #[test]
    fn a_clock_that_moves_backwards_does_not_shorten_the_wait() {
        let mut lock = AppLock::create(b"pw", weak()).unwrap();
        for _ in 0..4 {
            let _ = lock.verify_at(b"nope", 10_000);
        }
        assert!(matches!(
            lock.verify_at(b"pw", 9_000),
            Err(Error::AppLockCooldown(5))
        ));
    }

    #[test]
    fn it_round_trips_through_its_file_format() {
        let mut lock = AppLock::create(b"pw", weak()).unwrap();
        let _ = lock.verify_at(b"wrong", 4_242);
        let bytes = lock.to_bytes();
        assert_eq!(bytes.len(), LOCK_LEN);
        assert_eq!(&bytes[..8], MAGIC);

        let mut back = AppLock::parse(&bytes).unwrap();
        assert_eq!(back.failures(), 1);
        assert_eq!(back.params(), weak());
        assert_eq!(back.verify(b"pw"), Ok(()));
    }

    /// A failed attempt must survive a restart, or the rate limit is theatre.
    #[test]
    fn the_failure_count_survives_a_reload() {
        let mut lock = AppLock::create(b"pw", weak()).unwrap();
        for _ in 0..4 {
            let _ = lock.verify_at(b"wrong", 100);
        }
        let reloaded = AppLock::parse(&lock.to_bytes()).unwrap();
        assert_eq!(reloaded.failures(), 4);
        assert_eq!(reloaded.cooldown_at(100), Some(5));
    }

    #[test]
    fn malformed_lock_files_are_rejected_cleanly() {
        let lock = AppLock::create(b"pw", weak()).unwrap();
        let good = lock.to_bytes();

        assert!(matches!(AppLock::parse(b"short"), Err(Error::Truncated)));

        let mut long = good.clone();
        long.push(0);
        assert!(matches!(AppLock::parse(&long), Err(Error::BadHeader)));

        let mut magic = good.clone();
        magic[0] = b'X';
        assert!(matches!(AppLock::parse(&magic), Err(Error::BadMagic)));

        let mut version = good.clone();
        version[8] = 7;
        assert!(matches!(
            AppLock::parse(&version),
            Err(Error::UnsupportedVersion(7))
        ));

        let mut reserved = good.clone();
        reserved[9] = 1;
        assert!(matches!(AppLock::parse(&reserved), Err(Error::BadHeader)));

        // Costs are attacker-controlled and must be bounded at parse time, not
        // only when a password is eventually tried against them.
        for (offset, value) in [(12usize, u32::MAX), (20, u32::MAX), (16, 0)] {
            let mut bad = good.clone();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(
                matches!(AppLock::parse(&bad), Err(Error::KdfParams)),
                "offset {offset} value {value} was accepted"
            );
        }
    }

    /// Regression: the verifier used to be written with the process umask and
    /// only chmod'd afterwards, so it was world-readable for a window on every
    /// single save — and a save happens after every failed attempt.
    #[cfg(unix)]
    #[test]
    fn the_lock_file_is_owner_only_from_the_moment_it_exists() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("applock.bin");

        let mut store = LockStore::create(&path, b"pw", weak()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "created with mode {mode:o}");

        // And it stays that way across the rewrite a failed attempt triggers.
        let _ = store.unlock(b"nope");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "after a save, mode is {mode:o}");
    }

    /// Creating must fail atomically rather than by testing `exists()` first,
    /// so a symbolic link at the lock path is refused instead of followed.
    #[cfg(unix)]
    #[test]
    fn a_symlink_at_the_lock_path_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim");
        std::fs::write(&victim, b"important").unwrap();
        let path = dir.path().join("applock.bin");
        std::os::unix::fs::symlink(&victim, &path).unwrap();

        assert!(matches!(
            LockStore::create(&path, b"pw", weak()),
            Err(Error::AppLockStore)
        ));
        assert_eq!(std::fs::read(&victim).unwrap(), b"important");
    }

    /// The verifier must not be a key anything else could derive from the same
    /// passphrase and salt. Domain separation is what guarantees that.
    #[test]
    fn the_verifier_is_domain_separated_from_container_keys() {
        let salt = [5u8; kdf::SALT_LEN];
        let plain = kdf::derive_key(b"same passphrase", &salt, weak()).unwrap();
        let bound = derive_verifier(b"same passphrase", &salt, weak()).unwrap();
        assert_ne!(plain, bound, "app lock and container key must not coincide");
    }

    #[test]
    fn the_stored_verifier_is_not_the_password_or_a_bare_hash_of_it() {
        let lock = AppLock::create(b"hunter2", weak()).unwrap();
        let bytes = lock.to_bytes();
        assert!(
            !bytes.windows(7).any(|w| w == b"hunter2"),
            "the password itself is in the file"
        );
    }

    #[test]
    fn the_store_persists_attempts_across_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("applock.bin");

        let mut store = LockStore::create(&path, b"pw", weak()).unwrap();
        assert!(path.exists());
        assert!(matches!(store.unlock(b"no"), Err(Error::AppLockRejected)));

        let mut reopened = LockStore::open(&path).unwrap().expect("lock is there");
        assert_eq!(reopened.failures(), 1, "the failure did not reach disk");
        reopened.unlock(b"pw").unwrap();
        assert_eq!(reopened.failures(), 0);
    }

    #[test]
    fn no_lock_file_means_no_lock_but_a_broken_one_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("applock.bin");
        assert!(LockStore::open(&missing).unwrap().is_none());

        let corrupt = dir.path().join("corrupt.bin");
        std::fs::write(&corrupt, vec![0u8; LOCK_LEN]).unwrap();
        assert!(
            LockStore::open(&corrupt).is_err(),
            "a corrupt lock must not read as an absent one"
        );
    }

    #[test]
    fn creating_over_an_existing_lock_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("applock.bin");
        LockStore::create(&path, b"pw", weak()).unwrap();
        assert!(matches!(
            LockStore::create(&path, b"other", weak()),
            Err(Error::AppLockStore)
        ));
    }

    #[test]
    fn changing_and_removing_need_the_current_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("applock.bin");
        let mut store = LockStore::create(&path, b"first", weak()).unwrap();

        assert!(store.change_password(b"wrong", b"second").is_err());
        store.change_password(b"first", b"second").unwrap();
        assert!(matches!(
            store.unlock(b"first"),
            Err(Error::AppLockRejected)
        ));
        store.unlock(b"second").unwrap();

        let store = LockStore::open(&path).unwrap().unwrap();
        assert!(store.remove(b"nope").is_err());
        assert!(path.exists(), "a failed removal must leave the lock alone");

        let store = LockStore::open(&path).unwrap().unwrap();
        store.remove(b"second").unwrap();
        assert!(!path.exists());
    }

    /// The user-facing claim must keep stating the limit. If someone edits this
    /// into a promise, this test is what stops it shipping.
    #[test]
    fn the_scope_note_states_the_limit_rather_than_a_guarantee() {
        let scope = SCOPE.to_lowercase();
        assert!(scope.contains("not disk encryption"));
        assert!(scope.contains("not tamper-proof"));
        assert!(scope.contains("delete"), "deletion must be admitted");
        assert!(scope.contains("offline"), "offline attack must be admitted");
        for boast in ["unbreakable", "impossible", "guarantee"] {
            assert!(!scope.contains(boast), "overclaim: {boast}");
        }
    }

    #[test]
    fn the_default_path_is_under_a_config_directory_when_the_environment_says() {
        if let Some(path) = default_path() {
            assert!(
                path.ends_with("veilvoice/applock.bin") || path.ends_with("veilvoice\\applock.bin")
            );
            assert!(path.parent().is_some());
        }
    }
}
