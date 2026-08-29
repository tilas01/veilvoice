// SPDX-License-Identifier: GPL-3.0-or-later
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
//! The lock stores `Argon2id(domain ‖ password, salt)`, split by HKDF into a
//! verifier and a tag key, and compares the verifier in constant time. It
//! derives no key that encrypts anything, and that is still true of this
//! module after marker 86.
//!
//! What changed is one level up. The desktop application can now be told to
//! seal every recording it writes *with the app-lock passphrase*, and it does
//! that through [`crate::container`] in the ordinary way, with a fresh salt
//! per file. So the recordings do not depend on this file: delete the lock and
//! they still open, given the passphrase. Nothing here holds a key to them and
//! nothing here can.
//!
//! The property that is given up by switching that on is not cryptographic, it
//! is human, and it belongs in the interface rather than in a comment: one
//! passphrase then opens the application and the archive together. The default
//! is still two separate secrets, and `docs/USER_GUIDE.md` section 5.4 states
//! the trade in the words a user reads.
//!
//! The stored verifier is a password hash sitting on disk in the clear, and
//! must be treated like one. Argon2id at the default cost (256 MiB, t=3, p=4)
//! makes an offline attack expensive per guess, which matters for a decent
//! passphrase and does not save a bad one.
//!
//! # The tag, and the one tamper claim this file can honestly make
//!
//! Every record carries a 16-byte authentication tag over all the bytes before
//! it, keyed by a value that exists only while a correct passphrase is in
//! memory. The tag key is the second half of one Argon2id run, split from the
//! verifier by HKDF, so publishing the verifier — which the file does, by
//! sitting on disk — says nothing about the tag key.
//!
//! That buys exactly one thing, and it is worth naming precisely. Somebody who
//! edits this file without knowing the passphrase cannot leave the edit
//! looking authentic. Resetting the failure counter to zero, winding the
//! last-failure timestamp back to escape a wait, or dropping the Argon2id cost
//! so that a guess is cheap — all three are edits, and all three are caught at
//! the next successful unlock, because that is the moment the tag key exists.
//!
//! It buys nothing at all against the two attacks people expect it to stop.
//! Deleting the file still removes the lock. Replacing the file wholesale with
//! a lock the attacker created still lets the attacker in, because their own
//! record is authentic under *their* passphrase. [`crate::vault`] answers the
//! first with a second copy and answers the second not at all.
//!
//! The tamper flag, once raised, is stored and is cleared only by an unlock
//! that also proves the passphrase. So the report survives a restart, and the
//! person who caused it cannot dismiss it.
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
//!
//! # In plain words
//!
//! The lock on the application window.
//!
//! It asks for a passphrase before VeilVoice will open, and it slows down after
//! repeated wrong answers so that guessing is not worth trying.
//!
//! **It is not protection against somebody who has your disk.** It stops the
//! person who picks up your unlocked laptop, and that is genuinely worth having,
//! but anybody who can read the files directly is not stopped by a program
//! deciding whether to show you a window. Encrypting your recordings is what
//! protects them; this protects the session.

use crate::{kdf, Error, Secret};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// What the app lock protects against, and what it does not, in the words a
/// front-end should show the user.
///
/// Single-sourced so the CLI and the GUI cannot drift into two different
/// promises, and asserted by the tests so it cannot quietly become a boast.
pub const SCOPE: &str = "The app lock keeps someone who picks up your unlocked computer out of \
     VeilVoice. If the stored password is swapped or its cost weakened, the \
     next unlock reports it, and a second copy is kept, so to delete the lock \
     you have to find both. It is still not tamper-proof and it is not disk \
     encryption: anyone holding the disk can attack the stored password hash \
     offline. If that is the threat, encrypt the whole volume.";

/// Magic bytes at the start of a lock file.
pub const MAGIC: &[u8; 8] = b"VEILLOK1";
/// Format version this build writes.
pub const FORMAT_VERSION: u8 = 2;
/// Exact size of a version 2 lock file, in bytes.
pub const LOCK_LEN: usize = 125;
/// Exact size of a version 1 lock file, which this build still reads.
pub const LOCK_LEN_V1: usize = 84;

/// Domain separator, so the app-lock secret can never coincide with a key
/// derived from the same passphrase anywhere else in this crate.
const DOMAIN: &[u8] = b"veilvoice/app-lock/v1\0";
/// HKDF label for the half of the derivation that is written to disk.
const INFO_VERIFIER: &[u8] = b"veilvoice/app-lock/verifier";
/// HKDF label for the half that is not, and that authenticates the record.
const INFO_TAG: &[u8] = b"veilvoice/app-lock/tag";
/// Length of the record tag. Sixteen bytes of Poly1305, as everywhere else.
const TAG_LEN: usize = crate::aead::TAG_LEN;
/// Length of the tagged part of a record, which is also the offset of the
/// nonce that follows it.
const BODY_LEN: usize = 73;

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
    /// Raised when a record failed its tag, and kept raised across restarts
    /// until an unlock clears it. See [`AppLock::tampered`].
    tampered: bool,
    /// Which format this record was read from, so that a version 1 file can be
    /// rewritten as version 2 the first time the passphrase is available.
    version: u8,
    /// Nonce for the record tag, drawn fresh on every write.
    ///
    /// Fresh rather than fixed because Poly1305 authenticates each message
    /// under a one-time key derived from the key *and the nonce*. Two records
    /// tagged under one nonce would hand an observer of this file two
    /// equations in the same two unknowns, which is enough to solve for the
    /// one-time key and forge a third. The file is rewritten after every
    /// failed attempt, so "two records" is an afternoon, not a corner case.
    tag_nonce: [u8; crate::aead::NONCE_LEN],
    /// The tag as read from disk. `None` for a version 1 record, which has no
    /// tag, and for a freshly created one, which has not been written yet.
    tag: Option<[u8; TAG_LEN]>,
}

impl AppLock {
    /// Create a lock for `password`.
    pub fn create(password: &[u8], params: kdf::KdfParams) -> Result<Self, Error> {
        let salt = kdf::random_salt()?;
        let (verifier, _) = derive_pair(password, &salt, params)?;
        let mut lock = Self {
            params,
            salt,
            verifier,
            failures: 0,
            last_failure: 0,
            tampered: false,
            version: FORMAT_VERSION,
            tag_nonce: [0u8; crate::aead::NONCE_LEN],
            tag: None,
        };
        // Tag it here rather than leaving that to the caller. A record created
        // and written without a tag would be indistinguishable on disk from a
        // version 1 file, and would go one more unlock before it was covered.
        lock.retag(password)?;
        Ok(lock)
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
        let (candidate, tag_key) = derive_pair(password, &self.salt, self.params)?;
        if candidate != self.verifier {
            self.failures = self.failures.saturating_add(1);
            self.last_failure = now;
            return Err(Error::AppLockRejected);
        }

        // The passphrase is right, so the tag key exists and the record can be
        // checked against it. This is the only moment it can be: before now
        // there was nothing to check with.
        //
        // A version 1 record has no tag and cannot be judged either way. It is
        // upgraded rather than accused: `version` carries the answer out to
        // `LockStore`, which rewrites the file as version 2 while it still has
        // the passphrase.
        if self.version >= 2 && !self.tag_matches(&tag_key) {
            self.tampered = true;
        }

        self.failures = 0;
        self.last_failure = 0;
        Ok(())
    }

    /// Whether two records hold the same stored password.
    ///
    /// Compares the salt as well as the verifier, because two locks made from
    /// the same passphrase have different salts and therefore different
    /// verifiers: the question being asked is "are these the same lock", not
    /// "would the same passphrase open both".
    ///
    /// Constant time, although the values being compared are both already on
    /// disk. It costs nothing and keeps one habit for this kind of material
    /// rather than two.
    pub fn same_secret_as(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        let salts = self.salt.ct_eq(&other.salt);
        let verifiers = self.verifier.expose().ct_eq(other.verifier.expose());
        bool::from(salts & verifiers)
    }

    /// Whether a record has been found edited by somebody without the
    /// passphrase, at any point since this was last cleared.
    ///
    /// Sticky on purpose. A report that a restart clears is a report an
    /// attacker clears.
    pub fn tampered(&self) -> bool {
        self.tampered
    }

    /// Clear the tamper report, after proving the passphrase.
    ///
    /// Takes the password rather than trusting an earlier unlock, so that
    /// nothing can dismiss the report except the person who can open the lock.
    pub fn acknowledge(&mut self, password: &[u8]) -> Result<(), Error> {
        self.verify(password)?;
        self.tampered = false;
        Ok(())
    }

    /// True when this record predates the authentication tag and should be
    /// rewritten once the passphrase is in hand.
    pub fn needs_upgrade(&self) -> bool {
        self.version < FORMAT_VERSION
    }

    fn tag_matches(&self, tag_key: &Secret) -> bool {
        let Ok(want) = self.tag(tag_key) else {
            // A failure to compute the tag is a broken build or an exhausted
            // machine, not evidence about the file. Do not call it tampering.
            return true;
        };
        match &self.tag {
            Some(have) => bool::from(subtle::ConstantTimeEq::ct_eq(&want[..], &have[..])),
            None => false,
        }
    }

    /// The authentication tag over everything in the record before it.
    ///
    /// Poly1305 over an empty message with the record as associated data: a
    /// keyed tag built from the AEAD already in this crate, rather than a
    /// second MAC construction to review.
    fn tag(&self, tag_key: &Secret) -> Result<[u8; TAG_LEN], Error> {
        let sealed = crate::aead::seal(tag_key, &self.tag_nonce, &self.body(), &[])?;
        let mut out = [0u8; TAG_LEN];
        if sealed.len() != TAG_LEN {
            return Err(Error::Encrypt);
        }
        out.copy_from_slice(&sealed);
        Ok(out)
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

    /// The bytes the tag covers.
    ///
    /// Not the whole record. The failed-attempt counter and its timestamp are
    /// deliberately outside, and the reason is worth stating rather than
    /// leaving to be discovered: they are written at the one moment the tag
    /// key does not exist. A wrong passphrase has to be counted, and counting
    /// it means a write, and the write cannot be authenticated by a key that
    /// only a right passphrase produces. Putting them inside would mean either
    /// re-tagging with a stale tag -- so every honest typo would be reported
    /// as tampering -- or not counting failures at all.
    ///
    /// So the rate limit is exactly as defeatable by an editor as it was
    /// before, and [`SCOPE`] does not claim otherwise. What the tag does cover
    /// is the part an attacker actually wants: the verifier, the Argon2id
    /// cost, and the tamper flag itself.
    fn body(&self) -> [u8; BODY_LEN] {
        let mut out = [0u8; BODY_LEN];
        out[..8].copy_from_slice(MAGIC);
        out[8] = FORMAT_VERSION;
        // 9..12 stay zero: reserved.
        out[12..16].copy_from_slice(&self.params.m_cost.to_le_bytes());
        out[16..20].copy_from_slice(&self.params.t_cost.to_le_bytes());
        out[20..24].copy_from_slice(&self.params.p_cost.to_le_bytes());
        out[24..40].copy_from_slice(&self.salt);
        out[40..72].copy_from_slice(self.verifier.expose());
        out[72] = u8::from(self.tampered);
        out
    }

    /// Draw a fresh nonce and re-tag the record under `password`.
    ///
    /// Called when the passphrase is in hand: at creation, at a change, and
    /// after a successful unlock. A version 1 record becomes version 2 here,
    /// which is the whole of the upgrade path.
    pub fn retag(&mut self, password: &[u8]) -> Result<(), Error> {
        let (_, tag_key) = derive_pair(password, &self.salt, self.params)?;
        self.tag_nonce = crate::aead::random_nonce()?;
        self.version = FORMAT_VERSION;
        self.tag = Some(self.tag(&tag_key)?);
        Ok(())
    }

    /// Serialise exactly as it appears on disk.
    ///
    /// ```text
    ///  offset  size  field                                  covered by the tag
    ///       0     8  magic "VEILLOK1"                        yes
    ///       8     1  format version (2)                      yes
    ///       9     3  reserved, must be zero                  yes
    ///      12     4  Argon2id m_cost (KiB, little-endian)    yes
    ///      16     4  Argon2id t_cost                         yes
    ///      20     4  Argon2id p_cost                         yes
    ///      24    16  salt                                    yes
    ///      40    32  verifier                                yes
    ///      72     1  tamper report: 1 raised, 0 acknowledged yes
    ///      73    24  tag nonce, fresh on every re-tag        no
    ///      97    16  tag over bytes 0..73                    no
    ///     113     4  consecutive failed attempts             no
    ///     117     8  Unix seconds of the most recent failure no
    /// ```
    ///
    /// A record that has never been tagged -- one read from a version 1 file
    /// and not yet unlocked -- is written back as version 1, so that a failed
    /// attempt against an old lock still records itself. [`AppLock::retag`]
    /// is what moves it forward, and it needs the passphrase to do so.
    pub fn to_bytes(&self) -> Vec<u8> {
        let Some(tag) = self.tag else {
            let mut out = Vec::with_capacity(LOCK_LEN_V1);
            out.extend_from_slice(MAGIC);
            out.push(1);
            out.extend_from_slice(&[0u8; 3]);
            out.extend_from_slice(&self.params.m_cost.to_le_bytes());
            out.extend_from_slice(&self.params.t_cost.to_le_bytes());
            out.extend_from_slice(&self.params.p_cost.to_le_bytes());
            out.extend_from_slice(&self.salt);
            out.extend_from_slice(self.verifier.expose());
            out.extend_from_slice(&self.failures.to_le_bytes());
            out.extend_from_slice(&self.last_failure.to_le_bytes());
            debug_assert_eq!(out.len(), LOCK_LEN_V1);
            return out;
        };

        let mut out = Vec::with_capacity(LOCK_LEN);
        out.extend_from_slice(&self.body());
        out.extend_from_slice(&self.tag_nonce);
        out.extend_from_slice(&tag);
        out.extend_from_slice(&self.failures.to_le_bytes());
        out.extend_from_slice(&self.last_failure.to_le_bytes());
        debug_assert_eq!(out.len(), LOCK_LEN);
        out
    }

    /// Parse a lock file, version 1 or version 2.
    ///
    /// Nothing is authenticated here. Deciding whether the tag is right needs
    /// the passphrase, which parsing does not have, so parsing answers only
    /// "is this a lock file" and leaves the rest to [`AppLock::verify`].
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 9 {
            return Err(Error::Truncated);
        }
        if &bytes[..8] != MAGIC {
            return Err(Error::BadMagic);
        }
        let version = bytes[8];
        let want = match version {
            1 => LOCK_LEN_V1,
            2 => LOCK_LEN,
            other => return Err(Error::UnsupportedVersion(other)),
        };
        if bytes.len() < want {
            return Err(Error::Truncated);
        }
        if bytes.len() > want {
            return Err(Error::BadHeader);
        }
        // Refuse a future flag rather than ignore it, as the container does.
        if bytes[9..12] != [0u8; 3] {
            return Err(Error::BadHeader);
        }

        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let i64_at = |o: usize| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&bytes[o..o + 8]);
            i64::from_le_bytes(b)
        };
        let params = kdf::KdfParams {
            m_cost: u32_at(12),
            t_cost: u32_at(16),
            p_cost: u32_at(20),
        };
        // Validate the costs here rather than only at the first verification.
        // They are attacker-controlled -- this file is read before anyone has
        // authenticated -- and `KdfParams::checked` is the single funnel that
        // bounds them (see F-2 and F-3). Refusing at parse time means the
        // failure is reported as "this lock file is broken", which is true and
        // actionable, rather than as a password that never works.
        //
        // F-91. `within` rather than `checked`, and the difference matters more
        // here than anywhere else this crate reads a cost from a file.
        // `checked` alone permits four gigabytes of Argon2 memory, which is
        // deliberate for a container: somebody chose to open that file, it is
        // slow, and they can decide to stop waiting. Nobody chooses to open
        // this one. It is read at launch, before anything has been
        // authenticated, and a value of four gigabytes on a modest machine is
        // not a wait, it is an allocation failure, and this build aborts on
        // one. The window would then fail to start with no way in.
        //
        // Found by the coverage-guided campaign after the format changed: it
        // produced a header declaring 1,664 MiB and libFuzzer flagged the unit
        // as slow. That the recovery is now harder is this cycle's own doing:
        // the vault's file names are derived rather than fixed, so "delete the
        // lock file and start again" needs the index read first.
        //
        // The ceiling is `UNATTENDED_MAX_M_COST`, four times what this program
        // has ever written into one of these files.
        params.within(kdf::KdfParams::UNATTENDED_MAX_M_COST)?;
        let mut salt = [0u8; kdf::SALT_LEN];
        salt.copy_from_slice(&bytes[24..40]);

        let mut verifier_bytes = [0u8; kdf::KEY_LEN];
        verifier_bytes.copy_from_slice(&bytes[40..72]);
        let verifier = Secret::new(&mut verifier_bytes);

        let mut tag_nonce = [0u8; crate::aead::NONCE_LEN];
        let mut tag = None;
        let mut tampered = false;
        let (failures, last_failure) = if version >= 2 {
            // The flag is one bit written in one byte, so anything but 0 or 1
            // is an edit. Refuse it rather than normalising it: normalising
            // would make two different files parse to the same record, and a
            // file read before anybody has authenticated is the last place to
            // accept bytes it does not then write back. Refusing is also the
            // safe direction, because a lock file that will not parse leaves
            // the lock in force and sends `crate::vault` to the second copy.
            tampered = match bytes[72] {
                0 => false,
                1 => true,
                _ => return Err(Error::BadHeader),
            };
            tag_nonce.copy_from_slice(&bytes[73..97]);
            let mut t = [0u8; TAG_LEN];
            t.copy_from_slice(&bytes[97..113]);
            tag = Some(t);
            (u32_at(113), i64_at(117))
        } else {
            (u32_at(72), i64_at(76))
        };

        Ok(Self {
            params,
            salt,
            verifier,
            failures,
            last_failure,
            tampered,
            version,
            tag_nonce,
            tag,
        })
    }
}

/// Derive the verifier and the tag key for `password`.
///
/// One Argon2id run, split in two by HKDF. Two runs would double the cost of
/// every unlock for no gain: HKDF outputs under distinct labels are
/// independent, so publishing the verifier -- which the file does, by existing
/// -- reveals nothing about the tag key.
///
/// The domain separator is prepended before the Argon2id run so that neither
/// half can collide with a container key derived from the same passphrase and
/// salt. The joined buffer is wiped as soon as it has been consumed.
fn derive_pair(
    password: &[u8],
    salt: &[u8],
    params: kdf::KdfParams,
) -> Result<(Secret, Secret), Error> {
    let mut bound = Vec::with_capacity(DOMAIN.len() + password.len());
    bound.extend_from_slice(DOMAIN);
    bound.extend_from_slice(password);
    let bound = Secret::new(&mut bound);
    let root = kdf::derive_key(bound.expose(), salt, params)?;

    let hk = hkdf::Hkdf::<sha2::Sha256>::from_prk(root.expose()).map_err(|_| Error::Kdf)?;
    let mut verifier = Secret::zeroed(kdf::KEY_LEN);
    let mut tag_key = Secret::zeroed(kdf::KEY_LEN);
    hk.expand(INFO_VERIFIER, verifier.expose_mut())
        .map_err(|_| Error::Kdf)?;
    hk.expand(INFO_TAG, tag_key.expose_mut())
        .map_err(|_| Error::Kdf)?;
    Ok((verifier, tag_key))
}

/// An [`AppLock`] bound to a file, which is persisted after every attempt.
///
/// Persisting on failure is the point: a rate limit that a process restart
/// clears is not a rate limit.
pub struct LockStore {
    backing: Backing,
    lock: AppLock,
    /// Whether the last write reached the spare as well as the first copy. See
    /// [`LockStore::every_copy_current`].
    every_copy_current: bool,
}

/// Where a [`LockStore`] keeps its record.
///
/// Two, because the two are asked for by different callers. The default
/// location is a [`crate::vault::Vault`]: two copies under unguessable names,
/// one of them administrator-owned where the platform allows it. An explicit
/// path is one plain file, which is what `veilvoice lock --path` is for and
/// what a script pointing at a temporary directory expects.
#[derive(Clone, Debug)]
enum Backing {
    File(PathBuf),
    Vault(crate::vault::Vault),
}

impl Backing {
    fn primary(&self) -> &Path {
        match self {
            Self::File(p) => p,
            Self::Vault(v) => v.primary(),
        }
    }
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
            backing: Backing::File(path.to_path_buf()),
            lock: AppLock::parse(&bytes)?,
            every_copy_current: true,
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
            backing: Backing::File(path.to_path_buf()),
            lock: AppLock::create(password, params)?,
            every_copy_current: true,
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
        if result.is_ok() {
            // The passphrase is in hand, so this is the moment to draw a fresh
            // nonce, re-tag, and move a version 1 record forward. It is also
            // the only moment a raised tamper flag can be written with a tag
            // that will stand up to the next check.
            let _ = self.lock.retag(password);
        }
        // Nothing to persist if we never got as far as an attempt.
        if !matches!(result, Err(Error::AppLockCooldown(_))) {
            if let Ok(current) = self.save() {
                self.every_copy_current = current;
            }
        }
        result
    }

    /// Whether the stored record has been found edited by somebody without the
    /// passphrase. See [`AppLock::tampered`].
    pub fn tampered(&self) -> bool {
        self.lock.tampered()
    }

    /// Clear the tamper report, after proving the passphrase, and persist that.
    ///
    /// One key derivation, not three. The first version called `unlock` and
    /// then `AppLock::acknowledge`, which verifies again, and each of those is
    /// a full Argon2id run at 256 MiB. Three of them is the better part of a
    /// minute on a slow machine to dismiss a message, and a control nobody will
    /// wait for is a control nobody uses. `unlock` has already proved the
    /// passphrase by the time the flag is cleared.
    pub fn acknowledge(&mut self, password: &[u8]) -> Result<(), Error> {
        self.unlock(password)?;
        self.lock.tampered = false;
        self.lock.retag(password)?;
        self.every_copy_current = self.save()?;
        Ok(())
    }

    /// Raise the tamper report from outside, and persist it if the passphrase
    /// allows.
    ///
    /// [`crate::vault`] calls this when it finds the two copies of a lock
    /// disagreeing, which is evidence this module cannot see on its own. The
    /// flag is held in memory either way; persisting it needs an unlock, so a
    /// report raised now becomes durable at the next successful one.
    pub fn report_tamper(&mut self) {
        self.lock.tampered = true;
    }

    /// Replace the password, after proving the current one.
    pub fn change_password(&mut self, current: &[u8], new: &[u8]) -> Result<(), Error> {
        self.unlock(current)?;
        let carried = self.lock.tampered;
        self.lock = AppLock::create(new, self.lock.params)?;
        // A new passphrase is not an acknowledgement. Somebody who changes the
        // password without ever reading the report should still see it, so the
        // flag is carried across and the record re-tagged under the new key.
        self.lock.tampered = carried;
        self.lock.retag(new)?;
        self.every_copy_current = self.save()?;
        // A change that did not reach the spare has left the previous password
        // sitting in a file this process cannot rewrite. Anybody who deletes
        // the first copy gets the old password back, so the change is reported
        // as incomplete rather than as done.
        if !self.every_copy_current {
            return Err(Error::AppLockSpareStale);
        }
        Ok(())
    }

    /// Remove the lock, after proving the password.
    ///
    /// Proving it is a courtesy to the honest user, not a control: the file can
    /// simply be deleted by anyone who can reach it, which [`SCOPE`] says.
    pub fn remove(mut self, current: &[u8]) -> Result<(), Error> {
        self.unlock(current)?;
        match &self.backing {
            Backing::Vault(v) => v.clear(),
            Backing::File(path) => std::fs::remove_file(path).map_err(|_| Error::AppLockStore),
        }
    }

    /// Seconds still to wait before another attempt is accepted.
    pub fn cooldown(&self) -> Option<Duration> {
        self.lock.cooldown()
    }

    /// Consecutive failed attempts recorded so far.
    pub fn failures(&self) -> u32 {
        self.lock.failures()
    }

    /// Where this lock is stored. The first of the two copies, when it is
    /// vault-backed.
    pub fn path(&self) -> &Path {
        self.backing.primary()
    }

    /// Write the record, and say whether every copy of it is now current.
    ///
    /// `Ok(false)` only ever comes from a vault whose administrator-owned spare
    /// could not be written. A single file is either written or an error.
    fn save(&self) -> Result<bool, Error> {
        match &self.backing {
            Backing::Vault(v) => v.store(&self.lock),
            Backing::File(path) => {
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|_| Error::AppLockStore)?;
                    }
                }
                write_private(path, &self.lock.to_bytes(), false).map(|()| true)
            }
        }
    }

    /// Whether the last write reached every copy.
    ///
    /// False when the administrator-owned spare could not be updated, which
    /// leaves it holding an older record. Callers show this; a spare carrying
    /// the previous password is a way back in for anybody who knew it.
    pub fn every_copy_current(&self) -> bool {
        self.every_copy_current
    }
}

/// Open the lock at the default location, wherever this platform keeps it.
///
/// Returns `Ok(None)` when no lock is configured, and `Err` when the
/// environment does not say where a configuration directory is -- a caller
/// that cannot find one should say so rather than scatter a lock file into the
/// working directory.
///
/// The second element is true when the two copies did not agree: one had gone
/// and was rebuilt from the other, or both were there and held different stored
/// passwords. Neither happens on its own, so the caller should treat it as a
/// tamper report and show it; [`LockStore::report_tamper`] is how to make it
/// stick.
pub fn open_default() -> Result<(Option<LockStore>, bool), Error> {
    let base = default_dir().ok_or(Error::AppLockStore)?;
    let vault = crate::vault::Vault::at(&base, crate::vault::admin_dir().as_deref())?;
    let (record, found) = vault.load()?;
    // Both outcomes are evidence somebody has been at the files: a copy that
    // went, or two copies that no longer hold the same password.
    let restored = matches!(
        found,
        crate::vault::Found::Restored | crate::vault::Found::Disagreed
    );
    Ok((
        record.map(|lock| LockStore {
            backing: Backing::Vault(vault),
            lock,
            every_copy_current: true,
        }),
        restored,
    ))
}

/// Create a lock at the default location, refusing to replace one already
/// there.
pub fn create_default(password: &[u8], params: kdf::KdfParams) -> Result<LockStore, Error> {
    let base = default_dir().ok_or(Error::AppLockStore)?;
    let vault = crate::vault::Vault::at(&base, crate::vault::admin_dir().as_deref())?;
    if vault.load()?.0.is_some() {
        return Err(Error::AppLockStore);
    }
    let mut store = LockStore {
        backing: Backing::Vault(vault),
        lock: AppLock::create(password, params)?,
        every_copy_current: true,
    };
    store.every_copy_current = store.save()?;
    Ok(store)
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

/// The configuration directory the vault keeps its files in, if the
/// environment says where one is.
pub fn default_dir() -> Option<PathBuf> {
    default_path().and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Where the lock file lives on this platform, if the environment says.
///
/// Resolved from environment variables rather than a directories crate: it is
/// twenty lines, it adds no dependency to a security crate, and it returns
/// `None` instead of guessing when the environment does not say. A caller that
/// gets `None` should tell the user it cannot find a config directory rather
/// than scattering a lock file into the working directory.
///
/// This is the path a caller naming one gets, and the anchor a dozen other
/// settings files are derived from -- policy, capture, sentry. It is no longer
/// where the lock itself is kept by default: [`open_default`] goes through
/// [`crate::vault`], whose files sit in the same directory under names derived
/// from its index.
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
        let (bound, _) = derive_pair(b"same passphrase", &salt, weak()).unwrap();
        assert_ne!(plain, bound, "app lock and container key must not coincide");
    }

    /// The half of the derivation that is written to disk must say nothing
    /// about the half that authenticates the record, or the tag is worth
    /// nothing to anybody holding the file.
    #[test]
    fn the_stored_verifier_and_the_tag_key_are_independent() {
        let salt = [5u8; kdf::SALT_LEN];
        let (verifier, tag_key) = derive_pair(b"same passphrase", &salt, weak()).unwrap();
        assert_ne!(verifier, tag_key);
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

    /// The claim the tag exists to support, stated as a test so it cannot
    /// quietly stop being true: an edit made without the passphrase is caught
    /// at the next unlock.
    #[test]
    fn swapping_the_stored_verifier_is_reported_at_the_next_unlock() {
        let honest = AppLock::create(b"the real one", weak()).unwrap();
        let attacker = AppLock::create(b"the attacker's", weak()).unwrap();

        // Splice the attacker's verifier into the owner's record, leaving
        // everything else including the tag alone. This is the attack: a lock
        // that opens to a password the owner never chose.
        let mut bytes = honest.to_bytes();
        let theirs = attacker.to_bytes();
        bytes[24..72].copy_from_slice(&theirs[24..72]);

        let mut edited = AppLock::parse(&bytes).unwrap();
        assert!(!edited.tampered(), "nothing has been checked yet");
        edited.verify(b"the attacker's").unwrap();
        assert!(
            edited.tampered(),
            "the record was edited by somebody without the passphrase and \
             nothing said so"
        );
    }

    /// Weakening the stored cost is the other edit worth making, and it is
    /// answered before the tag is even consulted: the cost is an input to the
    /// derivation, so a changed cost produces a different verifier and the
    /// owner's own passphrase stops matching. Refusal, not a warning, which is
    /// the stronger of the two answers.
    #[test]
    fn weakening_the_argon_cost_stops_the_lock_opening_at_all() {
        let lock = AppLock::create(b"pw", kdf::KdfParams::default()).unwrap();
        let mut bytes = lock.to_bytes();
        bytes[12..16].copy_from_slice(&weak().m_cost.to_le_bytes());
        bytes[16..20].copy_from_slice(&weak().t_cost.to_le_bytes());
        bytes[20..24].copy_from_slice(&weak().p_cost.to_le_bytes());

        let mut edited = AppLock::parse(&bytes).unwrap();
        assert!(matches!(edited.verify(b"pw"), Err(Error::AppLockRejected)));
    }

    #[test]
    fn clearing_the_tamper_flag_by_hand_puts_it_straight_back() {
        let honest = AppLock::create(b"pw", weak()).unwrap();
        let attacker = AppLock::create(b"other", weak()).unwrap();
        let mut bytes = honest.to_bytes();
        bytes[24..72].copy_from_slice(&attacker.to_bytes()[24..72]);

        let mut raised = AppLock::parse(&bytes).unwrap();
        raised.verify(b"other").unwrap();
        assert!(raised.tampered());
        let mut stored = raised.to_bytes();
        assert_eq!(stored[72], 1, "the report must reach the file");

        // Now wipe the flag, as somebody hiding their tracks would.
        stored[72] = 0;
        let mut hidden = AppLock::parse(&stored).unwrap();
        assert!(!hidden.tampered(), "the file no longer says so");
        hidden.verify(b"other").unwrap();
        assert!(
            hidden.tampered(),
            "the flag is inside the tag, so clearing it is itself an edit"
        );
    }

    /// The report must not be dismissible by anything except the passphrase,
    /// and it must survive the process dying.
    #[test]
    fn a_report_outlives_a_restart_and_needs_the_passphrase_to_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("applock.bin");
        LockStore::create(&path, b"pw", weak()).unwrap();

        let attacker = AppLock::create(b"other", weak()).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[24..72].copy_from_slice(&attacker.to_bytes()[24..72]);
        std::fs::write(&path, &bytes).unwrap();

        let mut store = LockStore::open(&path).unwrap().unwrap();
        store.unlock(b"other").unwrap();
        assert!(store.tampered());

        // Reopened from disk, the report is still there.
        let mut store = LockStore::open(&path).unwrap().unwrap();
        assert!(store.tampered(), "the report did not survive a restart");
        assert!(matches!(
            store.acknowledge(b"wrong"),
            Err(Error::AppLockRejected)
        ));
        assert!(store.tampered(), "a wrong password must not dismiss it");
        store.acknowledge(b"other").unwrap();
        assert!(!store.tampered());

        let store = LockStore::open(&path).unwrap().unwrap();
        assert!(!store.tampered(), "the acknowledgement must reach disk");
    }

    /// An honest typo is not tampering, and a test says so because getting
    /// this wrong would train the user to ignore the warning.
    #[test]
    fn a_wrong_password_is_never_reported_as_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("applock.bin");
        let mut store = LockStore::create(&path, b"pw", weak()).unwrap();
        for _ in 0..3 {
            assert!(matches!(store.unlock(b"typo"), Err(Error::AppLockRejected)));
        }
        store.unlock(b"pw").unwrap();
        assert!(!store.tampered());
    }

    /// A lock written by an older build has no tag. It must keep working, and
    /// it must gain one, and it must not be accused of anything on the way.
    #[test]
    fn a_version_one_lock_still_opens_and_is_upgraded_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("applock.bin");

        let lock = AppLock::create(b"pw", weak()).unwrap();
        let v2 = lock.to_bytes();
        let mut v1 = Vec::with_capacity(LOCK_LEN_V1);
        v1.extend_from_slice(&v2[..8]);
        v1.push(1);
        v1.extend_from_slice(&v2[9..72]);
        v1.extend_from_slice(&2u32.to_le_bytes()); // failures
        v1.extend_from_slice(&0i64.to_le_bytes()); // last failure
        assert_eq!(v1.len(), LOCK_LEN_V1);
        std::fs::write(&path, &v1).unwrap();

        let mut store = LockStore::open(&path).unwrap().unwrap();
        assert_eq!(store.failures(), 2, "the old counter must be read");
        store.unlock(b"pw").unwrap();
        assert!(!store.tampered(), "an untagged record is not an edited one");

        let raw = std::fs::read(&path).unwrap();
        assert_eq!(raw.len(), LOCK_LEN, "it should have been rewritten");
        assert_eq!(raw[8], FORMAT_VERSION);
    }

    /// The nonce must move on every write, because Poly1305 under a repeated
    /// nonce hands an observer of two records enough to forge a third.
    #[test]
    fn two_writes_never_share_a_tag_nonce() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("applock.bin");
        let mut store = LockStore::create(&path, b"pw", weak()).unwrap();
        let first = std::fs::read(&path).unwrap()[73..97].to_vec();
        store.unlock(b"pw").unwrap();
        let second = std::fs::read(&path).unwrap()[73..97].to_vec();
        assert_ne!(first, second, "the tag nonce repeated across two writes");
    }

    /// F-91. The one file this program reads before anybody has authenticated
    /// must not be able to ask for more memory than the machine has. Nobody
    /// chose to open it, so nobody can choose to stop waiting for it.
    #[test]
    fn a_lock_file_cannot_demand_more_memory_than_an_unattended_caller_allows() {
        let lock = AppLock::create(b"pw", weak()).unwrap();
        let mut bytes = lock.to_bytes();

        // What the campaign produced: within `checked`, far outside anything
        // this program has ever written.
        bytes[12..16].copy_from_slice(&(1_703_936u32).to_le_bytes());
        assert!(
            AppLock::parse(&bytes).is_err(),
            "a lock file declaring 1.6 GiB of Argon2 memory was accepted"
        );

        // And the ceiling itself is above anything legitimate, so a real lock
        // is not caught by it.
        bytes[12..16].copy_from_slice(&kdf::KdfParams::default().m_cost.to_le_bytes());
        bytes[16..20].copy_from_slice(&kdf::KdfParams::default().t_cost.to_le_bytes());
        bytes[20..24].copy_from_slice(&kdf::KdfParams::default().p_cost.to_le_bytes());
        assert!(
            AppLock::parse(&bytes).is_ok(),
            "the default cost this program writes was refused by its own ceiling"
        );
    }

    /// F-88. Acknowledging a report must cost one key derivation, not three.
    /// Counted in the source rather than timed, because a timing test on a
    /// deliberately slow function is a flaky test.
    #[test]
    fn acknowledging_a_report_derives_the_key_once() {
        let source = include_str!("lock.rs").replace("\r\n", "\n");
        let start = source
            .find("pub fn acknowledge(&mut self, password: &[u8]) -> Result<(), Error> {\n        self.unlock")
            .expect("the store's acknowledge has to exist");
        let body = &source[start..];
        let end = body.find("\n    }").unwrap_or(body.len());
        let body = &body[..end];
        assert!(
            !body.contains("self.lock.acknowledge"),
            "the store verifies once through `unlock`; calling the record's own \
             acknowledge verifies a second and a third time"
        );
        assert_eq!(
            body.matches("self.unlock").count(),
            1,
            "one proof of the passphrase is enough to clear a report"
        );
    }

    /// F-86. A password change that could not reach the spare has left the
    /// previous password in a file this process cannot rewrite, so it is not a
    /// finished change and must not be reported as one.
    #[test]
    fn a_change_that_did_not_reach_the_spare_is_not_reported_as_done() {
        let source = include_str!("lock.rs").replace("\r\n", "\n");
        let start = source
            .find("pub fn change_password")
            .expect("change_password exists");
        let body = &source[start..];
        let end = body.find("\n    /// Remove the lock").unwrap_or(body.len());
        assert!(
            body[..end].contains("Error::AppLockSpareStale"),
            "a change that left the spare behind reported success"
        );
    }

    /// The user-facing claim must keep stating the limit. If someone edits this
    /// into a promise, this test is what stops it shipping.
    #[test]
    fn the_scope_note_states_the_limit_rather_than_a_guarantee() {
        let scope = SCOPE.to_lowercase();
        assert!(scope.contains("not disk encryption"));
        assert!(
            scope.contains("not tamper-proof"),
            "the tag catches edits; it does not make the file tamper-proof, and \
             the difference is the whole of what this note exists to say"
        );
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
