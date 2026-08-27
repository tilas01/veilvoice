// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![deny(missing_docs)]
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
//! **both are always derived** even when the first one matches: returning early
//! would make a real passphrase measurably faster than a decoy, which tells an
//! observer with a stopwatch which of the two they just watched somebody type.
//!
//! # In plain words
//!
//! You can set a second passphrase. Typing it opens VeilVoice normally, except
//! that it is empty: no recordings, no projects, no history. It is there for
//! the situation where somebody is standing over you asking you to unlock your
//! computer.
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

use veilvoice_crypto::{kdf, Error};

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

/// A pair of passphrase verifiers, checked together.
///
/// Holds no passphrase and no key: only the Argon2id output of each, and the
/// salt each was derived with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    real: Verifier,
    decoy: Option<Verifier>,
    params: kdf::KdfParams,
}

/// One passphrase's stored form.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Verifier {
    salt: [u8; kdf::SALT_LEN],
    expected: Vec<u8>,
}

impl Verifier {
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
                "the two passphrases differ in only {differing} place(s) and need at \
                 least {least}. A decoy that is nearly the real one is not a decoy: \
                 somebody watching you type learns both at once, and somebody typing \
                 under pressure gives away the wrong one"
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

impl Pair {
    /// A real passphrase with no decoy. This is the ordinary case.
    pub fn only_real(passphrase: &str, params: kdf::KdfParams) -> Result<Self, Refused> {
        if passphrase.is_empty() {
            return Err(Refused::Empty);
        }
        Ok(Self {
            real: Verifier::create(passphrase.as_bytes(), params)
                .map_err(|e| Refused::Crypto(e.to_string()))?,
            decoy: None,
            params,
        })
    }

    /// A real passphrase and a decoy.
    ///
    /// Refuses a decoy too close to the real one. See [`Refused::TooAlike`]:
    /// this is the one check that decides whether the feature is worth having.
    pub fn with_decoy(real: &str, decoy: &str, params: kdf::KdfParams) -> Result<Self, Refused> {
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
            real: Verifier::create(real.as_bytes(), params)
                .map_err(|e| Refused::Crypto(e.to_string()))?,
            decoy: Some(
                Verifier::create(decoy.as_bytes(), params)
                    .map_err(|e| Refused::Crypto(e.to_string()))?,
            ),
            params,
        })
    }

    /// Whether a decoy is set at all.
    pub fn has_decoy(&self) -> bool {
        self.decoy.is_some()
    }

    /// Which passphrase this is.
    ///
    /// **Both are always derived**, even when the first matches. Returning
    /// early would make the real passphrase measurably faster than the decoy,
    /// and somebody with a stopwatch would learn which of the two they had just
    /// watched being typed. Argon2id at the configured cost takes long enough
    /// for that difference to be obvious.
    pub fn open(&self, given: &str) -> Result<Opened, Error> {
        let real = self.real.matches(given.as_bytes(), self.params)?;
        let decoy = match &self.decoy {
            Some(verifier) => verifier.matches(given.as_bytes(), self.params)?,
            // No decoy set. The work is still done against the real verifier's
            // salt so that having a decoy and not having one take the same
            // time, which is itself worth hiding: an observer must not be able
            // to tell that this copy has one configured.
            None => {
                let _ = self.real.matches(given.as_bytes(), self.params)?;
                false
            }
        };
        Ok(if real {
            Opened::Real
        } else if decoy {
            Opened::Decoy
        } else {
            Opened::Wrong
        })
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

    #[test]
    fn the_real_passphrase_opens_the_real_thing() {
        let pair = Pair::only_real("correct horse battery", params()).unwrap();
        assert_eq!(pair.open("correct horse battery").unwrap(), Opened::Real);
        assert_eq!(pair.open("something else entirely").unwrap(), Opened::Wrong);
        assert!(!pair.has_decoy());
    }

    #[test]
    fn the_decoy_opens_the_empty_one() {
        let pair = Pair::with_decoy("the real one", "a different thing", params()).unwrap();
        assert_eq!(pair.open("the real one").unwrap(), Opened::Real);
        assert_eq!(pair.open("a different thing").unwrap(), Opened::Decoy);
        assert_eq!(pair.open("neither of them").unwrap(), Opened::Wrong);
        assert!(pair.has_decoy());
    }

    /// **The check that decides whether the feature is worth having.** A decoy
    /// one character from the real passphrase is not a decoy.
    #[test]
    fn a_decoy_too_close_to_the_real_one_is_refused() {
        let refused = Pair::with_decoy("hunter2000", "hunter2001", params()).unwrap_err();
        assert!(matches!(refused, Refused::TooAlike { differing: 1, .. }));
        let words = refused.to_string();
        assert!(words.contains("watching you type"), "{words}");
        assert!(words.contains("under pressure"), "{words}");

        assert_eq!(
            Pair::with_decoy("same", "same", params()).unwrap_err(),
            Refused::Identical
        );
        assert_eq!(
            Pair::with_decoy("", "anything at all", params()).unwrap_err(),
            Refused::Empty
        );
    }

    /// Length counts as difference, or "hunter2" and "hunter2222" would pass.
    #[test]
    fn a_longer_version_of_the_same_passphrase_is_still_too_close() {
        assert_eq!(differences("hunter2", "hunter2222"), 3);
        assert_eq!(differences("abcd", "abcd"), 0);
        assert_eq!(differences("abcd", "wxyz"), 4);
        assert!(Pair::with_decoy("hunter2", "hunter222", params()).is_err());
        assert!(Pair::with_decoy("hunter2", "totally different", params()).is_ok());
    }

    /// **Both are always derived.** Returning as soon as the real one matches
    /// would make it measurably faster than the decoy, and Argon2id takes long
    /// enough that somebody with a stopwatch would see it.
    #[test]
    fn matching_the_real_one_still_derives_the_decoy() {
        let source = include_str!("lib.rs");
        let start = source.find("pub fn open(").expect("the function");
        let end = source[start..].find("\n    }\n").expect("its end") + start;
        let body = &source[start..end];
        // No early return between the two derivations.
        let real_at = body.find("let real =").expect("the real derivation");
        let decoy_at = body.find("let decoy =").expect("the decoy derivation");
        assert!(real_at < decoy_at);
        assert!(
            !body[real_at..decoy_at].contains("return"),
            "an early return here is a timing oracle:\n{}",
            &body[real_at..decoy_at]
        );
    }

    /// Having a decoy and not having one must take the same time, or an
    /// observer learns that this copy has one configured.
    #[test]
    fn a_pair_with_no_decoy_still_does_the_second_derivation() {
        let source = include_str!("lib.rs");
        let start = source.find("pub fn open(").expect("the function");
        let end = source[start..].find("\n    }\n").expect("its end") + start;
        let body = &source[start..end];
        let none_arm = body.find("None => {").expect("the no-decoy arm");
        let after = &body[none_arm..];
        assert!(
            after.contains("self.real.matches"),
            "the no-decoy case must still do the work:\n{after}"
        );
    }

    /// Nothing in this crate deletes anything. The argument for that is only as
    /// good as the code continuing not to.
    #[test]
    fn nothing_here_destroys_anything() {
        let source = include_str!("lib.rs");
        let body = source.split("#[cfg(test)]").next().unwrap();
        let code: String = body
            .lines()
            .filter(|line| {
                let t = line.trim_start();
                !t.starts_with("//") && !t.starts_with("///") && !t.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");
        for destructive in [
            "remove_file",
            "remove_dir",
            "shred",
            "truncate",
            "set_len",
            "File::create",
        ] {
            assert!(
                !code.contains(destructive),
                "{destructive} appears in a crate whose whole argument is that it \
                 destroys nothing"
            );
        }
    }

    /// **The most important thing this crate outputs.** A reader who takes a
    /// decoy for deniability is worse off than one who never had it.
    #[test]
    fn the_scope_note_refuses_to_promise_deniability() {
        let scope = SCOPE.to_lowercase();
        assert!(scope.contains("does not hide"), "{scope}");
        assert!(scope.contains("open source"), "{scope}");
        assert!(
            scope.contains("ask you for the other one"),
            "the concrete thing an adversary does: {scope}"
        );
        assert!(scope.contains("does not buy you an argument"), "{scope}");
        assert!(scope.contains("nothing is destroyed"), "{scope}");
        for overclaim in ["undetectable", "plausible deniability", "untraceable"] {
            assert!(!scope.contains(overclaim), "\"{overclaim}\" in:\n{scope}");
        }
    }

    /// The refusal to ship destruction states the mechanism, not just the
    /// conclusion, because the conclusion alone reads as laziness.
    #[test]
    fn the_refusal_to_destroy_explains_the_storage_and_not_only_the_choice() {
        let why = WHY_NO_DESTRUCTION.to_lowercase();
        assert!(why.contains("a write does not overwrite"), "{why}");
        assert!(why.contains("fresh physical page"), "{why}");
        assert!(why.contains("may be never"), "{why}");
        assert!(
            why.contains("worse than no control at all"),
            "the reason it is a refusal rather than a gap: {why}"
        );
    }

    #[test]
    fn comparing_is_constant_time_and_length_safe() {
        assert!(constant_time_eq(b"abcd", b"abcd"));
        assert!(!constant_time_eq(b"abcd", b"abce"));
        assert!(!constant_time_eq(b"abcd", b"abc"));
        assert!(constant_time_eq(b"", b""));
    }
}
