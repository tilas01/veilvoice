// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-policy
//!
//! Settings somebody else decided, sealed so they cannot be edited without a
//! passphrase, and, more importantly, built so that editing them without one
//! buys nothing worth having.
//!
//! ## The design decision this crate turns on
//!
//! A policy file has an obvious problem. To *apply* a policy at every launch,
//! the program has to be able to read it at every launch. If reading it needs a
//! passphrase, the user types one every time, and if it does not, then anybody
//! who can write the file can rewrite the policy.
//!
//! The usual answers are a privileged daemon holding the key, or a key hidden
//! in the binary, and neither is honest here: this project needs no privileges,
//! and a key in a binary anybody can download is not a key.
//!
//! So the constraint is moved into the shape of the data. **A requirement can
//! only make VeilVoice stricter.** There is no requirement that turns
//! encryption off, none that lowers the de-identification floor, none that
//! disables the app lock, and there is no room in [`Requirement`] to express
//! one, because every variant is a tightening and the type has no other kind.
//!
//! Then somebody who edits the plain policy file without the passphrase can do
//! exactly one thing: make this machine's VeilVoice **more** restrictive than
//! its owner asked for. That is a nuisance, and it is not a privacy failure:
//! which is the failure this project exists to avoid. The passphrase-sealed
//! copy is what proves the policy is the one the administrator wrote; the
//! shape of the type is what makes the answer survive the seal not having been
//! checked yet.
//!
//! ## What the seal is for, and what it is not
//!
//! [`Policy::seal`] uses the same container as everything else here: Argon2id
//! over the passphrase, X25519 with ML-KEM-768 for the hybrid modes,
//! XChaCha20-Poly1305 for the contents. [`verify`] opens the sealed copy and
//! compares it against the plain one, which is how anybody with the passphrase
//! establishes that the policy in force is the policy that was written.
//!
//! It is **not** enforcement. Anything with write access to VeilVoice's own
//! executable can replace VeilVoice, and no file it reads can prevent that.
//! Anything running as the user can delete the policy entirely. What a sealed
//! policy gives is a policy that cannot be *quietly rewritten into something
//! weaker*, and that is a smaller claim than "enforced" on purpose. See
//! [`SCOPE`].
//!
//! Detecting deletion is `veilvoice-guard`'s job, not this one's: put the
//! policy files in a tamper manifest and the removal shows up there.
//!
//! ## Reading a policy costs nothing
//!
//! [`Policy::load`] reads the plain file and applies it. It never asks for a
//! passphrase, never blocks, and reports the seal as [`Verification::Unchecked`]
//! rather than pretending to have looked. A front end that wants the stronger
//! statement calls [`verify`] when it has a passphrase to offer.
//!
//! # In plain words
//!
//! This lets settings be locked down, and only in one direction.
//!
//! Someone setting up a machine for other people can seal a set of settings so
//! they can be made stricter but never looser. Nobody needs a password to read
//! what the rules are -- only to change them -- because a rule people cannot see
//! is a rule they will trip over.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod policy;

pub use policy::{verify, Policy, Posture, Requirement, Verification, PLAIN_FILE, SEALED_FILE};

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What a sealed policy is worth, in the words a front end should show.
///
/// Single-sourced and asserted by the tests, exactly as the app lock's and the
/// tamper detector's notes are, so it cannot quietly turn into a promise.
pub const SCOPE: &str =
    "A policy can only make VeilVoice stricter. There is no setting here that turns \
     protection off, which is why the policy can be read without a passphrase and \
     applied before anybody has checked the seal: the worst an edited policy can do is \
     restrict this machine further than its owner intended. Sealing proves the policy \
     is the one that was written. It does not enforce it -- anything that can replace \
     VeilVoice's own executable can ignore every word of this, and anything running as \
     you can delete the file.";

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    Io(std::io::Error),
    /// A stored policy is not in a form this build understands.
    Malformed(String),
    /// The sealed copy could not be opened, or did not match.
    Crypto(veilvoice_crypto::Error),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<veilvoice_crypto::Error> for Error {
    fn from(error: veilvoice_crypto::Error) -> Self {
        Self::Crypto(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "input/output error: {error}"),
            Self::Malformed(what) => write!(f, "malformed policy: {what}"),
            Self::Crypto(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The claim must keep stating the limits. If somebody edits this into a
    /// promise, this is what stops it shipping.
    #[test]
    fn the_scope_note_states_the_limits_rather_than_a_guarantee() {
        let scope = SCOPE.to_lowercase();
        assert!(
            scope.contains("only make veilvoice stricter"),
            "the one-way property is the whole design and must be stated first"
        );
        assert!(scope.contains("does not enforce it"));
        assert!(scope.contains("delete the file"));
        for boast in [
            "cannot be bypassed",
            "unbreakable",
            "guarantee",
            "enforced",
            "tamper-proof",
        ] {
            assert!(!scope.contains(boast), "overclaim: {boast}");
        }
    }

    #[test]
    fn an_io_error_displays_and_keeps_its_source() {
        let error = Error::from(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert!(error.to_string().contains("gone"));
        assert!(std::error::Error::source(&error).is_some());
        assert!(Error::Malformed("x".into()).to_string().contains("x"));
    }
}
