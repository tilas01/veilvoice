// SPDX-License-Identifier: GPL-3.0-or-later
//! A second passphrase that opens a different, empty VeilVoice.
//!
//! # What this is for, and what it is honestly worth
//!
//! Somebody can be made to unlock a program. A decoy passphrase gives them
//! something true to say: it opens VeilVoice, the application works, and there
//! is nothing in it.
//!
//! **It does not give you deniability, and anyone who tells you otherwise is
//! selling something.** VeilVoice is open source. This file is published. An
//! adversary who knows what they are looking at knows the feature exists, can
//! read exactly how it works, and can simply ask for the other passphrase. What
//! a decoy buys is a way to *comply* without revealing; what it does not buy is
//! any argument that there is nothing more to reveal. [`SCOPE`] says that in
//! the words a front end must show, and it is the most important thing this
//! crate produces.
//!
//! # The destructive duress passphrase is deliberately not here
//!
//! The roadmap asked for two things: a decoy, and a duress passphrase that
//! destroys data. The second is not shipped, and [`WHY_NO_DESTRUCTION`] is the
//! reason in full. In short: VeilVoice cannot promise a file is gone.
//!
//! On flash storage a write does not overwrite. The controller maps a logical
//! block to a new physical page and leaves the old one holding the data until
//! it is garbage-collected, which may be minutes or may be never, and no
//! program running as an ordinary user can reach it. This project already
//! documents that about its own secure-erase feature and refuses to overstate
//! it there.
//!
//! A destructive duress passphrase would be believed in exactly the situation
//! where being wrong costs the most. Somebody types it expecting the recordings
//! to be gone; the ciphertext is still in unmapped pages; and they then behave
//! as though it is not. **A control people rely on and that does not work is
//! worse than no control at all.** So there is not one.
//!
//! # Typing the wrong one by mistake
//!
//! The other failure the roadmap named, and the reason this shape was chosen.
//! Because the decoy destroys nothing, typing it by accident costs a
//! relaunch and nothing else. There is no state to recover and no decision that
//! cannot be taken back. That is not a happy accident; it is why the
//! destructive design was rejected rather than made safer.
//!
//! # Both passphrases are checked the same way
//!
//! Which one matched must not be visible in how long the check took. Both are
//! derived with the same Argon2id parameters and compared in constant time, and
//! **both are always derived**, in every branch: see [`Store::judge`], which is
//! the one place an unlock goes through. Returning as soon as one matched would
//! make that answer measurably faster than the other, and tell an observer with
//! a stopwatch which of the two they had just watched somebody type.
//!
//! The same applies to having one at all. A copy with no decoy set derives
//! against a record that nothing opens, so the time an unlock takes does not
//! answer "is there a decoy on this machine".
//!
//! # Where it is kept, and what holds the real passphrase
//!
//! [`Store`] keeps one file beside the app lock, under a name derived from the
//! vault's index like the lock's own two copies, so a folder listing does not
//! say which file it is. What is in it is the decoy's verifier and nothing else.
//!
//! **The real passphrase is not in it.** It lives in the app lock, once, and
//! setting or removing a decoy proves it against that. A second copy of it here
//! would be two places that have to agree about the same secret with no way for
//! either to notice the other had changed, which is the shape of F-141.
//!
//! A decoy is not a failed attempt at the real passphrase either. Typing it does
//! not count towards the rate limit, because the moment somebody uses this
//! feature is the moment a cooldown would hurt them most.
//!
//! # In plain words
//!
//! You can set a second passphrase, from Settings or with `veilvoice decoy set`.
//! Typing it opens VeilVoice normally, except that it is empty: no recordings,
//! no projects, no history. It is there for the situation where somebody is
//! standing over you asking you to unlock your computer.
//!
//! It has to be properly different from your real one, and VeilVoice refuses a
//! pair that is nearly the same. Setting or removing it asks for your real
//! passphrase first, so nobody can do either behind your back at a window you
//! left unlocked.
//!
//! Two honest warnings, and please read them.
//!
//! It does **not** hide the fact that a second passphrase might exist. This
//! program's source code is public and this feature is described in it, so
//! anybody who recognises VeilVoice can ask you for the other one. It buys you
//! a way to hand something over. It does not buy you an argument.
//!
//! And there is **no passphrase that destroys your recordings**, deliberately.
//! On modern storage, deleting a file does not reliably remove it, so a feature
//! that claimed to would be lying to you at the worst possible moment.

use crate::{kdf, Error};

/// Which passphrase was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    /// The real one. Everything is here.
    Real,
    /// The decoy. VeilVoice opens, and it is empty.
    Decoy,
    /// Neither.
    Wrong,
}

/// One passphrase's stored form.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Verifier {
    salt: [u8; kdf::SALT_LEN],
    expected: Vec<u8>,
}

impl Verifier {
    /// Derive from `passphrase` once and keep what it produced, with the salt
    /// that produced it, so a later attempt can be compared against it.
    ///
    /// What is kept is the derived key itself rather than a hash of it, because
    /// [`Verifier::matches`] must do the whole derivation every time: a cheaper
    /// comparison would answer faster for a wrong passphrase than a right one,
    /// and how long an answer took is the one thing a decoy must not reveal.
    fn create(passphrase: &[u8], params: kdf::KdfParams) -> Result<Self, Error> {
        let salt = kdf::random_salt()?;
        let key = kdf::derive_key(passphrase, &salt, params)?;
        Ok(Self {
            salt,
            expected: key.expose().to_vec(),
        })
    }

    /// Derive and compare. Always does the full derivation.
    fn matches(&self, passphrase: &[u8], params: kdf::KdfParams) -> Result<bool, Error> {
        let key = kdf::derive_key(passphrase, &self.salt, params)?;
        Ok(constant_time_eq(key.expose(), &self.expected))
    }
}

/// Compare without letting the time taken depend on where they differ.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut difference = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        difference |= x ^ y;
    }
    difference == 0
}

/// How similar two passphrases may be before the pair is refused.
///
/// A decoy that differs from the real passphrase by one character is not a
/// decoy: somebody watching a keyboard learns both at once, and somebody
/// typing under pressure gives away the wrong one.
pub const LEAST_DIFFERENCE: usize = 4;

/// Why a pair was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// A passphrase was empty.
    Empty,
    /// The two are the same.
    Identical,
    /// The two are too alike to tell apart under pressure.
    TooAlike {
        /// How many characters differ.
        differing: usize,
        /// How many must.
        least: usize,
    },
    /// The passphrase given as the real one is not this machine's.
    NotTheRealOne,
    /// The key derivation failed.
    Crypto(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "a passphrase cannot be empty"),
            Self::Identical => write!(
                f,
                "the decoy is the same as the real passphrase, so it would open the \
                 real thing"
            ),
            Self::TooAlike { differing, least } => write!(
                f,
                "the two passphrases differ in only {differing} {} and need at least \
                 {least}. A decoy that is nearly the real one is not a decoy: \
                 somebody watching you type learns both at once, and somebody typing \
                 under pressure gives away the wrong one",
                if *differing == 1 { "place" } else { "places" }
            ),
            Self::NotTheRealOne => write!(
                f,
                "that is not this machine's passphrase, so there is nothing to set a \
                 decoy beside"
            ),
            Self::Crypto(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Refused {}

/// How many positions two passphrases differ in.
///
/// Length difference counts, so `"hunter2"` against `"hunter2222"` is three
/// apart rather than zero.
fn differences(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let shared = a.len().min(b.len());
    let mut count = a.len().abs_diff(b.len());
    for i in 0..shared {
        if a[i] != b[i] {
            count += 1;
        }
    }
    count
}

/// A decoy passphrase's stored form, on its own.
///
/// Holds no passphrase and no key: the Argon2id output of the decoy and the salt
/// that produced it, and nothing about the real passphrase at all.
///
/// **That absence is the design.** The first version of this module held both
/// verifiers in one value, which meant the real passphrase was stored in two
/// places, here and in the app lock, with two salts and no way for either to
/// notice the other had changed. That is the shape of F-141, where the window
/// wrote a lock somewhere the window never looked. The real passphrase has one
/// home, [`crate::lock::AppLock`], and [`Store::judge`] is what asks both
/// questions in one breath.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoy {
    verifier: Verifier,
    params: kdf::KdfParams,
}

/// What a decoy record starts with.
const MAGIC: &[u8; 8] = b"VEILDEC1";

/// The only format there is so far.
const FORMAT_VERSION: u8 = 1;

/// Exactly how long a record is: magic, version, three reserved bytes, the three
/// costs, the salt, the derived key.
pub const RECORD_LEN: usize = 8 + 1 + 3 + 12 + kdf::SALT_LEN + kdf::KEY_LEN;

impl Decoy {
    /// Make a decoy from the real passphrase and the decoy one.
    ///
    /// Both are needed even though only one is stored, because the rule that
    /// makes the feature worth having is a rule about the pair: see
    /// [`Refused::TooAlike`].
    ///
    /// `params` are the app lock's own. Anything else would make the two
    /// derivations take different lengths of time, which is the one thing this
    /// module exists to prevent.
    pub fn create(real: &str, decoy: &str, params: kdf::KdfParams) -> Result<Self, Refused> {
        if real.is_empty() || decoy.is_empty() {
            return Err(Refused::Empty);
        }
        if real == decoy {
            return Err(Refused::Identical);
        }
        let differing = differences(real, decoy);
        if differing < LEAST_DIFFERENCE {
            return Err(Refused::TooAlike {
                differing,
                least: LEAST_DIFFERENCE,
            });
        }
        Ok(Self {
            verifier: Verifier::create(decoy.as_bytes(), params)
                .map_err(|e| Refused::Crypto(e.to_string()))?,
            params,
        })
    }

    /// A record no passphrase opens, for the sake of taking the same time.
    ///
    /// Used where no decoy is set: [`Store::judge`] derives against this so that
    /// a copy with a decoy configured and a copy without cannot be told apart by
    /// how long an unlock took. The salt is fresh and the expected value is
    /// random, so nothing opens it, which is exactly what is wanted.
    fn nothing(params: kdf::KdfParams) -> Result<Self, Error> {
        let mut filler = [0u8; kdf::KEY_LEN];
        getrandom::getrandom(&mut filler).map_err(|_| Error::Random)?;
        Ok(Self {
            verifier: Verifier {
                salt: kdf::random_salt()?,
                expected: filler.to_vec(),
            },
            params,
        })
    }

    /// Derive and compare. Always does the full derivation.
    fn matches(&self, given: &str) -> Result<bool, Error> {
        self.verifier.matches(given.as_bytes(), self.params)
    }

    /// The record, for writing.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(RECORD_LEN);
        out.extend_from_slice(MAGIC);
        out.push(FORMAT_VERSION);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&self.params.m_cost.to_le_bytes());
        out.extend_from_slice(&self.params.t_cost.to_le_bytes());
        out.extend_from_slice(&self.params.p_cost.to_le_bytes());
        out.extend_from_slice(&self.verifier.salt);
        out.extend_from_slice(&self.verifier.expected);
        debug_assert_eq!(out.len(), RECORD_LEN);
        out
    }

    /// Read a record.
    ///
    /// Nothing here is authenticated, exactly as in
    /// [`crate::lock::AppLock::parse`]: judging the contents needs a passphrase
    /// and parsing has none. What it does do is bound the cost parameters,
    /// because this file is read before anybody has proved anything and a header
    /// declaring four gigabytes of Argon2 memory is an allocation failure rather
    /// than a wait. The ceiling is the lock's own, by the same argument: see
    /// F-91.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 12 {
            return Err(Error::Truncated);
        }
        if &bytes[..8] != MAGIC {
            return Err(Error::BadMagic);
        }
        if bytes[8] != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(bytes[8]));
        }
        // Refused rather than ignored, as the container and the lock both do: a
        // reserved byte that will mean something one day must not have meant
        // nothing today.
        if bytes[9..12] != [0u8; 3] {
            return Err(Error::BadHeader);
        }
        if bytes.len() < RECORD_LEN {
            return Err(Error::Truncated);
        }
        if bytes.len() > RECORD_LEN {
            return Err(Error::BadHeader);
        }
        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let params = kdf::KdfParams {
            m_cost: u32_at(12),
            t_cost: u32_at(16),
            p_cost: u32_at(20),
        };
        params.within(kdf::KdfParams::UNATTENDED_MAX_M_COST)?;
        let mut salt = [0u8; kdf::SALT_LEN];
        salt.copy_from_slice(&bytes[24..24 + kdf::SALT_LEN]);
        Ok(Self {
            verifier: Verifier {
                salt,
                expected: bytes[24 + kdf::SALT_LEN..RECORD_LEN].to_vec(),
            },
            params,
        })
    }
}

/// Whether this machine has a decoy passphrase, and whether it can be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// No decoy is set.
    Absent,
    /// One is set.
    Set,
    /// There is a record and it does not read.
    ///
    /// Its own answer rather than folded into [`Absent`](Self::Absent), because
    /// the two need different things said about them: one is a choice nobody has
    /// made, and the other is a decoy somebody believes they have and does not.
    /// A person in the second situation would hand over a passphrase that opens
    /// nothing, and find that out at the worst possible moment.
    Unreadable,
}

/// The decoy passphrase as this machine keeps it.
///
/// Sits beside the app lock under a name derived from the same index: see
/// [`crate::vault`]. It is one file, and losing it loses a decoy and nothing
/// else.
pub struct Store {
    vault: crate::vault::Vault,
}

impl Store {
    /// The store under `base`, which is the folder the app lock lives in.
    ///
    /// Reads or creates the vault index, exactly as opening a lock does, because
    /// the decoy's file name is derived from it.
    pub fn at(base: &std::path::Path) -> Result<Self, Error> {
        Ok(Self {
            vault: crate::vault::Vault::at(base, crate::vault::admin_dir_present().as_deref())?,
        })
    }

    /// The store for this copy of VeilVoice.
    pub fn here() -> Result<Self, Error> {
        let base = crate::lock::default_dir().ok_or(Error::AppLockStore)?;
        Self::at(&base)
    }

    /// Whether a decoy is set, and whether it reads.
    pub fn state(&self) -> State {
        if !self.vault.has_decoy_file() {
            return State::Absent;
        }
        match self.vault.load_decoy() {
            Some(_) => State::Set,
            None => State::Unreadable,
        }
    }

    /// Set or replace the decoy passphrase.
    ///
    /// `real` is checked against the app lock first, and not as a formality. The
    /// too-alike rule is a rule about the pair, so a caller free to pass any
    /// string as the real one could set a decoy one character away from the real
    /// passphrase by telling this something else. It also means somebody who has
    /// walked away from an unlocked window cannot have a decoy set behind their
    /// back by whoever sits down next.
    pub fn set(
        &self,
        lock: &crate::lock::LockStore,
        real: &str,
        decoy: &str,
    ) -> Result<(), Refused> {
        self.prove(lock, real)?;
        let record = Decoy::create(real, decoy, lock.params())?;
        self.vault
            .store_decoy(&record)
            .map_err(|e| Refused::Crypto(e.to_string()))
    }

    /// Remove the decoy passphrase, after proving the real one.
    ///
    /// Proving it matters as much here as it does for setting one: a decoy
    /// removed without the real passphrase is a decoy somebody else can take
    /// away, and the person who relies on it would not know until they needed it.
    pub fn remove(&self, lock: &crate::lock::LockStore, real: &str) -> Result<(), Refused> {
        self.prove(lock, real)?;
        self.vault
            .clear_decoy()
            .map_err(|e| Refused::Crypto(e.to_string()))
    }

    /// That `real` really is this machine's passphrase.
    fn prove(&self, lock: &crate::lock::LockStore, real: &str) -> Result<(), Refused> {
        match lock.would_open(real.as_bytes()) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Refused::NotTheRealOne),
            Err(e) => Err(Refused::Crypto(e.to_string())),
        }
    }

    /// Which passphrase was typed.
    ///
    /// **Two derivations, always, in every branch.** The decoy is derived first
    /// and unconditionally, against a record that opens to nothing where no decoy
    /// is set, so that having one configured and not having one take the same
    /// time. Then the real passphrase: through
    /// [`crate::lock::LockStore::unlock`] when the decoy did not match, and
    /// through [`crate::lock::LockStore::would_open`] when it did, which records
    /// nothing. A decoy that was typed is not a failed attempt at the real
    /// passphrase, and counting it as one would start the rate limit at the
    /// moment a delay is least wanted.
    ///
    /// The one branch that is not two derivations is the rate limit, which
    /// answers without deriving anything. That is deliberate and it is not a
    /// leak: a cooldown announces itself anyway, in words, to whoever is looking
    /// at the screen.
    ///
    /// **A decoy opens during a cooldown.** The decoy is derived and compared
    /// before the real lock is consulted, and the branch that answers
    /// [`Opened::Decoy`] never reaches [`crate::lock::LockStore::unlock`], so
    /// the rate limit does not apply to it. That is the right way round. The
    /// moment somebody needs this feature is the moment they are least able to
    /// wait out a delay, and a fumbled real passphrase a minute earlier is
    /// exactly how they would arrive at it. What it costs is that the decoy
    /// itself is not rate limited, and what that buys an attacker is a full
    /// Argon2id run per guess at a passphrase that opens nothing.
    pub fn judge(&self, lock: &mut crate::lock::LockStore, given: &str) -> Result<Opened, Error> {
        let record = match self.vault.load_decoy() {
            Some(record) => record,
            // No decoy, or one that will not read. Both do the work anyway: how
            // long an unlock took must not answer "is there a decoy on this
            // machine", and it must not answer "is the decoy on this machine
            // broken" either.
            None => Decoy::nothing(lock.params())?,
        };
        let decoyed = record.matches(given)?;

        if decoyed {
            // Cannot happen while `set` refuses an identical pair, and handled
            // rather than assumed: if both would open, the real one wins and
            // goes through the ordinary unlock, so nothing about the session is
            // left half done.
            if !lock.would_open(given.as_bytes())? {
                return Ok(Opened::Decoy);
            }
        }

        match lock.unlock(given.as_bytes()) {
            Ok(()) => Ok(Opened::Real),
            Err(Error::AppLockRejected) => Ok(Opened::Wrong),
            Err(other) => Err(other),
        }
    }
}

/// What a decoy is worth, in the words a front end must show.
pub const SCOPE: &str = "\
A decoy passphrase opens VeilVoice with nothing in it. It is a way to comply \
with somebody who is standing over you, without handing over your recordings.

It does NOT hide that a second passphrase might exist. VeilVoice is open source \
and this feature is documented, so anybody who recognises the program knows it \
is there and can simply ask you for the other one. A decoy buys you something \
to hand over. It does not buy you an argument that there is nothing more.

Nothing is destroyed, and nothing can be. Typing the decoy by mistake costs you \
a relaunch and nothing else.";

/// Why no passphrase destroys anything, and why that is the honest choice.
pub const WHY_NO_DESTRUCTION: &str = "\
There is no passphrase that deletes your recordings, deliberately.

On modern storage a write does not overwrite. The drive's controller puts the \
new data in a fresh physical page and leaves the old one holding the original \
until it is collected later, which may be minutes and may be never. No program \
running as an ordinary user can reach those pages. VeilVoice already says this \
about its secure-erase feature rather than overstating it, and the same fact \
governs here.

So a destructive passphrase would be believed at exactly the moment when being \
wrong costs the most: somebody types it, assumes the recordings are gone, and \
acts accordingly while the ciphertext is still on the disk. A control that \
people rely on and that does not work is worse than no control at all.";

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> kdf::KdfParams {
        // The weak parameters, so the suite is not a minute of Argon2id.
        kdf::KdfParams::weak_for_tests()
    }

    /// A folder with a lock in it, and the store beside it.
    fn machine(passphrase: &str) -> (tempfile::TempDir, crate::lock::LockStore, Store) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let lock = crate::lock::create_in(dir.path(), passphrase.as_bytes(), params())
            .expect("a lock in a folder of our own");
        let store = Store::at(dir.path()).expect("the store beside it");
        (dir, lock, store)
    }

    #[test]
    fn with_no_decoy_the_real_passphrase_opens_and_nothing_else_does() {
        let (_dir, mut lock, store) = machine("correct horse battery");
        assert_eq!(store.state(), State::Absent);
        assert_eq!(
            store.judge(&mut lock, "correct horse battery").unwrap(),
            Opened::Real
        );
        assert_eq!(
            store.judge(&mut lock, "something else entirely").unwrap(),
            Opened::Wrong
        );
    }

    #[test]
    fn the_decoy_opens_the_empty_one_and_the_real_one_still_works() {
        let (_dir, mut lock, store) = machine("the real one");
        store
            .set(&lock, "the real one", "a different thing")
            .unwrap();
        assert_eq!(store.state(), State::Set);

        assert_eq!(
            store.judge(&mut lock, "the real one").unwrap(),
            Opened::Real
        );
        assert_eq!(
            store.judge(&mut lock, "a different thing").unwrap(),
            Opened::Decoy
        );
        assert_eq!(
            store.judge(&mut lock, "neither of them").unwrap(),
            Opened::Wrong
        );
    }

    /// **Typing the decoy is not a failed attempt at the real passphrase.**
    ///
    /// The rate limit exists for somebody guessing. Somebody using the decoy is
    /// not guessing, and counting their decoy against them would put the real
    /// lock into a cooldown at the one moment a delay is least wanted: while
    /// whoever asked them to unlock it is still standing there.
    #[test]
    fn the_decoy_does_not_count_against_the_real_passphrase() {
        let (_dir, mut lock, store) = machine("the real one");
        store
            .set(&lock, "the real one", "a different thing")
            .unwrap();

        for _ in 0..5 {
            assert_eq!(
                store.judge(&mut lock, "a different thing").unwrap(),
                Opened::Decoy
            );
        }
        assert_eq!(
            lock.failures(),
            0,
            "the decoy was counted as a wrong answer"
        );
        assert!(
            lock.cooldown().is_none(),
            "the decoy started the rate limit"
        );
        assert_eq!(
            store.judge(&mut lock, "the real one").unwrap(),
            Opened::Real
        );
    }

    /// A wrong passphrase is still a wrong passphrase.
    #[test]
    fn a_wrong_one_is_still_counted() {
        let (_dir, mut lock, store) = machine("the real one");
        store
            .set(&lock, "the real one", "a different thing")
            .unwrap();
        assert_eq!(
            store.judge(&mut lock, "not either of them").unwrap(),
            Opened::Wrong
        );
        assert_eq!(lock.failures(), 1);
    }

    /// **The check that decides whether the feature is worth having.** A decoy
    /// one character from the real passphrase is not a decoy.
    #[test]
    fn a_decoy_too_close_to_the_real_one_is_refused() {
        let refused = Decoy::create("hunter2000", "hunter2001", params()).unwrap_err();
        assert!(matches!(refused, Refused::TooAlike { differing: 1, .. }));
        let words = refused.to_string();
        assert!(words.contains("watching you type"), "{words}");
        assert!(words.contains("under pressure"), "{words}");
        // One place, not "1 place(s)". A sentence somebody reads while deciding
        // whether their decoy is safe should read like one.
        assert!(words.contains("only 1 place "), "{words}");
        assert!(!words.contains("place(s)"), "{words}");

        assert_eq!(
            Decoy::create("same", "same", params()).unwrap_err(),
            Refused::Identical
        );
        assert_eq!(
            Decoy::create("", "anything at all", params()).unwrap_err(),
            Refused::Empty
        );
    }

    /// Length counts as difference, or "hunter2" and "hunter2222" would pass.
    #[test]
    fn a_longer_version_of_the_same_passphrase_is_still_too_close() {
        assert_eq!(differences("hunter2", "hunter2222"), 3);
        assert_eq!(differences("abcd", "abcd"), 0);
        assert_eq!(differences("abcd", "wxyz"), 4);
        assert!(Decoy::create("hunter2", "hunter222", params()).is_err());
        assert!(Decoy::create("hunter2", "totally different", params()).is_ok());
    }

    /// Setting a decoy takes the real passphrase, and it is checked.
    ///
    /// Two things fail without this. The too-alike rule is a rule about the pair,
    /// so a caller who could name any string as the real one could set a decoy a
    /// character away from the real passphrase by naming something else. And
    /// somebody who walks away from an unlocked window could have a decoy set
    /// behind their back by whoever sits down next.
    #[test]
    fn a_decoy_cannot_be_set_without_the_real_passphrase() {
        let (_dir, lock, store) = machine("the real one");
        assert_eq!(
            store
                .set(&lock, "a guess at the real one", "a decoy entirely")
                .unwrap_err(),
            Refused::NotTheRealOne
        );
        assert_eq!(store.state(), State::Absent);

        store
            .set(&lock, "the real one", "a decoy entirely")
            .unwrap();
        assert_eq!(
            store.remove(&lock, "a guess at the real one").unwrap_err(),
            Refused::NotTheRealOne
        );
        assert_eq!(store.state(), State::Set, "a decoy was removed by a guess");

        store.remove(&lock, "the real one").unwrap();
        assert_eq!(store.state(), State::Absent);
    }

    /// Replacing one leaves one decoy, not two.
    #[test]
    fn setting_a_second_decoy_replaces_the_first() {
        let (_dir, mut lock, store) = machine("the real one");
        store.set(&lock, "the real one", "the first decoy").unwrap();
        store
            .set(&lock, "the real one", "the second decoy")
            .unwrap();
        assert_eq!(
            store.judge(&mut lock, "the second decoy").unwrap(),
            Opened::Decoy
        );
        assert_eq!(
            store.judge(&mut lock, "the first decoy").unwrap(),
            Opened::Wrong,
            "the decoy that was replaced still opens"
        );
    }

    /// A removed decoy stops opening anything.
    #[test]
    fn a_removed_decoy_is_a_wrong_passphrase() {
        let (_dir, mut lock, store) = machine("the real one");
        store.set(&lock, "the real one", "the decoy").unwrap();
        store.remove(&lock, "the real one").unwrap();
        assert_eq!(store.judge(&mut lock, "the decoy").unwrap(), Opened::Wrong);
        assert_eq!(
            store.judge(&mut lock, "the real one").unwrap(),
            Opened::Real
        );
    }

    /// The record survives a round trip, and nothing about the real passphrase
    /// is in it.
    #[test]
    fn a_record_reads_back_as_what_was_written() {
        let decoy = Decoy::create("the real one", "the decoy", params()).unwrap();
        let bytes = decoy.to_bytes();
        assert_eq!(bytes.len(), RECORD_LEN);
        assert_eq!(&bytes[..8], MAGIC);
        assert_eq!(Decoy::parse(&bytes).unwrap(), decoy);
        assert!(Decoy::parse(&bytes).unwrap().matches("the decoy").unwrap());
        assert!(!Decoy::parse(&bytes)
            .unwrap()
            .matches("the real one")
            .unwrap());
    }

    /// A record read before anybody has authenticated cannot be allowed to name
    /// any cost it likes.
    ///
    /// The same argument as F-91 made about the lock: this file is read at
    /// launch, nobody chose to open it, and four gigabytes of Argon2 memory on a
    /// modest machine is not a wait but an allocation failure, in a build that
    /// aborts on one.
    #[test]
    fn a_record_declaring_an_absurd_cost_is_refused_at_parse_time() {
        let decoy = Decoy::create("the real one", "the decoy", params()).unwrap();
        let mut bytes = decoy.to_bytes();
        bytes[12..16].copy_from_slice(&(4u32 * 1024 * 1024).to_le_bytes());
        assert!(matches!(
            Decoy::parse(&bytes),
            Err(Error::KdfCostRefused { .. })
        ));

        // And the ordinary refusals. Long enough to have a magic, and it is
        // the wrong one; then short enough to have nothing at all.
        assert!(matches!(
            Decoy::parse(b"NOTOURS!\x01\x00\x00\x00"),
            Err(Error::BadMagic)
        ));
        assert!(matches!(Decoy::parse(b"tiny"), Err(Error::Truncated)));
        assert!(matches!(
            Decoy::parse(&decoy.to_bytes()[..30]),
            Err(Error::Truncated)
        ));
        let mut wrong_version = decoy.to_bytes();
        wrong_version[8] = 9;
        assert!(matches!(
            Decoy::parse(&wrong_version),
            Err(Error::UnsupportedVersion(9))
        ));
        let mut reserved = decoy.to_bytes();
        reserved[10] = 1;
        assert!(matches!(Decoy::parse(&reserved), Err(Error::BadHeader)));
        let mut trailing = decoy.to_bytes();
        trailing.push(0);
        assert!(matches!(Decoy::parse(&trailing), Err(Error::BadHeader)));
    }

    /// A decoy record that will not read is its own answer, and the real
    /// passphrase still works.
    ///
    /// Somebody in this position believes they have a decoy and does not. Saying
    /// so is the whole reason [`State::Unreadable`] is not folded into
    /// [`State::Absent`]: they would otherwise find out while handing the
    /// passphrase over.
    #[test]
    fn a_damaged_decoy_is_reported_and_does_not_stop_the_real_one() {
        let (dir, mut lock, store) = machine("the real one");
        store.set(&lock, "the real one", "the decoy").unwrap();
        let vault = crate::vault::Vault::at(dir.path(), None).unwrap();
        std::fs::write(vault.decoy(), b"not a decoy record at all").unwrap();

        assert_eq!(store.state(), State::Unreadable);
        assert_eq!(
            store.judge(&mut lock, "the real one").unwrap(),
            Opened::Real
        );
        assert_eq!(store.judge(&mut lock, "the decoy").unwrap(), Opened::Wrong);
    }

    /// The decoy record does not sit under a name that announces it.
    ///
    /// Obscurity, described as obscurity in [`crate::vault`], and the point is
    /// narrow and real: a folder listing must not show which file is the decoy,
    /// or the feature's existence on this machine is written on the disk.
    #[test]
    fn the_record_is_not_under_a_telling_name() {
        let (dir, lock, store) = machine("the real one");
        store.set(&lock, "the real one", "the decoy").unwrap();
        let vault = crate::vault::Vault::at(dir.path(), None).unwrap();
        let name = vault
            .decoy()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(
            !name.contains("decoy") && !name.contains("lock"),
            "the decoy's file is called {name}"
        );
        assert_ne!(vault.decoy(), vault.primary());
        assert_ne!(vault.decoy(), vault.shadow());
        assert_ne!(vault.decoy(), vault.index());

        // And it does not announce itself to a search of the disk either.
        let bytes = std::fs::read(vault.decoy()).unwrap();
        assert!(
            !bytes.windows(MAGIC.len()).any(|w| w == MAGIC),
            "the record's own magic is readable in the file"
        );
    }

    /// **Both are always derived.** Returning as soon as the decoy matches, or
    /// skipping the decoy when none is set, would make one answer measurably
    /// faster than another, and Argon2id takes long enough for somebody with a
    /// stopwatch to see it.
    ///
    /// Read from the source, because what is asserted is that a statement is
    /// *not* there. A timing test would measure this machine's load.
    #[test]
    fn every_branch_does_the_same_amount_of_work() {
        let source = include_str!("decoy.rs").replace("\r\n", "\n");
        let start = source.find("pub fn judge(").expect("the function");
        let end = source[start..].find("\n    }\n").expect("its end") + start;
        let body = &source[start..end];

        // No decoy set, and the work is done anyway.
        assert!(
            body.contains("Decoy::nothing(lock.params())"),
            "the no-decoy case must still derive:\n{body}"
        );
        // The decoy derivation happens before anything can return.
        let derive_at = body.find("record.matches(given)").expect("the derivation");
        assert!(
            !body[..derive_at].contains("return"),
            "something returns before the decoy is derived, which tells an \
             observer which branch they are in:\n{}",
            &body[..derive_at]
        );
        // And the decoy branch still proves the real passphrase.
        let decoy_at = body.find("if decoyed {").expect("the decoy branch");
        assert!(
            body[decoy_at..].contains("lock.would_open(given.as_bytes())"),
            "the decoy branch skips the real derivation:\n{}",
            &body[decoy_at..]
        );
    }

    /// The scope note says the two things that must not be softened.
    #[test]
    fn the_scope_note_still_says_the_uncomfortable_part() {
        assert!(SCOPE.contains("does NOT hide"), "{SCOPE}");
        assert!(SCOPE.contains("open source"), "{SCOPE}");
        assert!(WHY_NO_DESTRUCTION.contains("worse than no control"));
    }
}
