// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-sentry
//!
//! An early warning that something is going through your files: decoy files
//! that should never change, and a measure of how fast a directory tree is
//! changing.
//!
//! ## The word this crate does not use
//!
//! It does not **prevent** anything. By the time a canary has been encrypted,
//! whatever encrypted it has already been running for some number of seconds
//! and has already reached some number of real files. What this buys is the
//! difference between finding out now and finding out when you next open a
//! document — which is a real difference, and it is the whole of what is on
//! offer. See [`SCOPE`].
//!
//! Stopping a process mid-run needs an interposition point in the kernel:
//! `fanotify` with `FAN_OPEN_PERM` on Linux, a filesystem minifilter on
//! Windows. The Windows one requires a code-signing identity issued to a
//! verified legal entity, which this project does not have and will not get —
//! it is published under a pseudonym on purpose. `ROADMAP.md` records that as
//! a decision rather than an omission.
//!
//! ## The two signals, and what each is worth
//!
//! **[`canary`] — decoy files that should never change.** A file nothing
//! legitimately touches is a clean signal: if its contents differ from what
//! was planted, something walked the directory and wrote to everything it
//! found. Very few false positives, and one enormous hole — it only fires if
//! the attacker *reaches* it. Something that encrypts only `.docx` under one
//! folder will never see a canary planted anywhere else, and this crate cannot
//! tell you that it did not fire because nothing happened.
//!
//! **[`rate`] — how much of a tree changed, and how fast.** No blind spot, and
//! far weaker evidence: a restore from backup, importing a camera card, a
//! compiler, or a synchronisation client catching up all look exactly like a
//! mass rewrite, because they are one. [`rate::Churn`] therefore reports
//! numbers and a [`rate::Concern`] level against a threshold **you** set. It
//! never reports "ransomware", because it does not know that and neither does
//! anything else that only counts files.
//!
//! Used together they are worth more than either: a canary hit *and* a high
//! churn rate at the same moment is a much stronger statement than either
//! alone. That is a judgement for the front end to present, not for this crate
//! to make on the user's behalf.
//!
//! ## What it cannot tell you: who
//!
//! Nothing here names a process. Attribution needs system auditing that is
//! normally switched off, and it lives in `veilvoice-guard` rather than here —
//! `veilvoice_guard::who_touched` takes a path and usually, honestly, answers
//! that it does not know. This crate deliberately does not depend on that one:
//! a project that wants canaries should not have to take a cryptography stack
//! with them.
//!
//! ## Entropy is a hint and never evidence
//!
//! [`entropy`] measures Shannon entropy in bits per byte, and encrypted data
//! sits near 8.0. So does a JPEG, a `.zip`, a video, and this project's own
//! `.veil` container. High entropy is only meaningful for a file whose
//! contents this crate *planted* and therefore knows should have been prose.
//! It is used for exactly that and nothing else.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod canary;
pub mod rate;

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What this crate is worth, in the words a front end should show.
///
/// Single-sourced and asserted by the tests, exactly as the app lock's and the
/// tamper detector's notes are, so it cannot quietly turn into a promise.
pub const SCOPE: &str =
    "Canaries and churn detection warn you that something is going through your files. \
     They cannot stop it. A canary only fires if whatever is running reaches that \
     folder, so a quiet canary is not evidence that nothing happened. Churn detection \
     counts changes and cannot tell ransomware from a backup restore, a photo import or \
     a compiler, so it reports a rate against a threshold you set rather than a verdict. \
     Neither one names the program responsible.";

/// Shannon entropy of `bytes`, in bits per byte, from 0.0 to 8.0.
///
/// Empty input is 0.0 by definition here: there is no distribution to measure,
/// and returning `NaN` would propagate into every comparison downstream.
///
/// **Read the crate documentation before using this to judge a file.** Near-8.0
/// means "incompressible", which is true of encrypted data and equally true of
/// every JPEG, archive and video on the machine. It carries information only
/// about a file whose original contents are known.
pub fn entropy(bytes: &[u8]) -> f32 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for byte in bytes {
        counts[*byte as usize] += 1;
    }
    let total = bytes.len() as f32;
    let mut bits = 0.0f32;
    for count in counts {
        if count == 0 {
            continue;
        }
        let p = count as f32 / total;
        bits -= p * p.log2();
    }
    bits
}

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file or directory could not be read or written.
    Io(std::io::Error),
    /// A stored record is not in a form this build understands.
    Malformed(String),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "input/output error: {error}"),
            Self::Malformed(what) => write!(f, "malformed record: {what}"),
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
        assert!(scope.contains("cannot stop it"));
        assert!(
            scope.contains("not evidence that nothing happened"),
            "the canary's blind spot must be admitted"
        );
        assert!(
            scope.contains("cannot tell ransomware from"),
            "churn detection must not be presented as a detector of ransomware"
        );
        assert!(scope.contains("names the program responsible"));
        for boast in [
            "prevent",
            "block",
            "unbreakable",
            "guarantee",
            "protects against",
            "stops ransomware",
        ] {
            assert!(!scope.contains(boast), "overclaim: {boast}");
        }
    }

    #[test]
    fn entropy_of_nothing_is_zero_rather_than_nan() {
        let measured = entropy(&[]);
        assert!(measured.is_finite());
        assert_eq!(measured, 0.0);
    }

    #[test]
    fn entropy_of_one_repeated_byte_is_zero() {
        assert_eq!(entropy(&[7u8; 4096]), 0.0);
    }

    /// Every byte value once is the maximum: 8 bits per byte.
    #[test]
    fn entropy_of_a_uniform_byte_range_is_eight() {
        let all: Vec<u8> = (0..=255u8).collect();
        assert!((entropy(&all) - 8.0).abs() < 1e-5, "{}", entropy(&all));
    }

    /// Two equally likely values is exactly one bit.
    #[test]
    fn entropy_of_two_equally_likely_values_is_one() {
        let mut bytes = vec![b'a'; 512];
        bytes.extend_from_slice(&[b'b'; 512]);
        assert!((entropy(&bytes) - 1.0).abs() < 1e-5);
    }

    /// English prose sits far below the ceiling. This is the whole basis for
    /// noticing that a planted canary stopped being prose.
    #[test]
    fn prose_is_well_below_encrypted_data() {
        let prose = b"the quick brown fox jumps over the lazy dog, and does so repeatedly, \
                      because that is what the sentence is for";
        let measured = entropy(prose);
        assert!(measured > 2.0, "prose is not zero-entropy: {measured}");
        assert!(measured < 5.5, "prose must not look encrypted: {measured}");
    }

    /// Never outside the range the documentation promises, for any input.
    #[test]
    fn entropy_stays_within_its_documented_range() {
        for length in [1usize, 2, 3, 255, 256, 257, 1000] {
            let bytes: Vec<u8> = (0..length).map(|i| (i * 31 % 256) as u8).collect();
            let measured = entropy(&bytes);
            assert!(
                (0.0..=8.0).contains(&measured),
                "{length} bytes gave {measured}"
            );
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
