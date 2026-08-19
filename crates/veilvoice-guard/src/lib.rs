// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-guard
//!
//! Tamper **detection** for VeilVoice's own files: a manifest of what they
//! should be, a check of what they are, and a best-effort answer to "what
//! changed them".
//!
//! ## What this is, and the word it deliberately does not use
//!
//! It is not tamper-*proof*. Nothing that runs as an ordinary program on your
//! computer can be. A guard running as you can be killed by anything else
//! running as you; a guard running as root can be stopped by root. The only
//! honest verb here is **detect**, and where detection can be defeated that is
//! said rather than glossed.
//!
//! This is the same limit the app lock has, for the same reason, and the
//! project answers it the same way: state it plainly, in the place the user
//! reads. See [`SCOPE`].
//!
//! ## What it actually does
//!
//! - [`Manifest::of`] records each file's size and SHA-256.
//! - [`Manifest::check`] compares that against what is on disk now and reports
//!   what was **modified**, **removed** or **added**.
//! - [`blame`] tries to name the process responsible for a change. It usually
//!   cannot, and says so instead of guessing.
//!
//! ## The manifest is only as trustworthy as where it is kept
//!
//! Written plainly, a manifest detects accidental corruption, an interrupted
//! update, a file swapped by something careless -- and an attacker who did not
//! think to rewrite it. It does not detect one who did, because they can
//! recompute it as easily as this crate can.
//!
//! To raise that bar, seal the manifest with
//! [`veilvoice_crypto::container::seal_with_password`] and keep the passphrase
//! out of the manifest's own directory. Then rewriting it undetectably requires
//! the passphrase as well as write access. That is a real improvement and still
//! not proof: an attacker who is present *while* you type the passphrase has
//! everything. [`Manifest::seal`] and [`Manifest::open_sealed`] do this.
//!
//! ## What a privileged helper would add, and why there is not one here
//!
//! A root service using `fanotify` (Linux) or a SACL plus Security event 4663
//! (Windows) could attribute every write reliably, and `fanotify` with
//! `FAN_OPEN_PERM` could even block one. That is genuinely stronger than
//! anything in this crate.
//!
//! It is also an installer, a privileged daemon and a much larger attack
//! surface bolted onto a project that currently needs no privileges at all --
//! and it still could not stop a root-level attacker, only watch one. So the
//! unprivileged half ships first, on its own merits, and `ROADMAP.md` records
//! what the privileged half would need to be worth adding.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod blame;
mod manifest;

pub use blame::{who_touched, who_touched as blame_path, Blame};
pub use manifest::{files_in as manifest_files_in, Change, Entry, Manifest, Report};

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What tamper detection is worth, in the words a front-end should show.
///
/// Single-sourced and asserted by the tests, exactly as the app lock's note is,
/// so it cannot quietly turn into a promise.
pub const SCOPE: &str =
    "Tamper detection tells you that VeilVoice's files changed. It cannot stop them \
     changing, and it is not tamper-proof: anyone who can rewrite the files can rewrite \
     the record of them too, unless you seal it with a passphrase kept somewhere else. \
     Naming the program responsible needs system auditing that is usually switched off, \
     so most of the time it will honestly report that it does not know.";

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    Io(std::io::Error),
    /// The manifest is not in a form this build understands.
    Malformed(String),
    /// The manifest could not be sealed or opened.
    Crypto(veilvoice_crypto::Error),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<veilvoice_crypto::Error> for Error {
    fn from(e: veilvoice_crypto::Error) -> Self {
        Self::Crypto(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "input/output error: {e}"),
            Self::Malformed(m) => write!(f, "malformed manifest: {m}"),
            Self::Crypto(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The claim must keep stating the limit. If someone edits this into a
    /// promise, this is what stops it shipping -- the same guard the app lock's
    /// scope note has.
    #[test]
    fn the_scope_note_states_the_limit_rather_than_a_guarantee() {
        let scope = SCOPE.to_lowercase();
        assert!(scope.contains("cannot stop them changing"));
        assert!(scope.contains("not tamper-proof"));
        assert!(
            scope.contains("does not know"),
            "the usual attribution answer must be admitted"
        );
        for boast in ["prevent", "unbreakable", "guarantee", "protects against"] {
            assert!(!scope.contains(boast), "overclaim: {boast}");
        }
    }
}
