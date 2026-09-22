// SPDX-License-Identifier: GPL-3.0-or-later
//! **Roadmap item 164.** The key a download brings with it, and why it is
//! never the key anything is checked against.
//!
//! # The trap this exists to close
//!
//! A VeilVoice release publishes four things beside each archive: the hash
//! list, the detached signature over it, the contents list, and
//! `veilvoice-signing-key.asc`, the public key. That last file is there so a
//! reader can import it into their own GnuPG and run the check themselves, and
//! it is exactly the file that makes a naive verifier worthless.
//!
//! Consider what somebody hands you: an archive, a hash list that matches it, a
//! signature that verifies, and the key that made the signature. Check the
//! signature with that key and everything agrees perfectly. It also proves
//! nothing whatever, because whoever produced the archive produced all four,
//! and a forged set is internally consistent for exactly the same reason a
//! genuine one is. A download that vouches for itself is a download that has
//! said nothing.
//!
//! So the rule, stated once here and depended on everywhere:
//!
//! > **The key an archive carries is never trusted for the answer.** The
//! > signature is checked against the key compiled into this binary, and the
//! > carried key is only ever *compared*, by fingerprint, against that one.
//!
//! # And a mismatch is the whole verdict
//!
//! A carried key whose fingerprint is not [`crate::check::FINGERPRINT`] ends
//! the check there, and nothing else is reported. That is stronger than
//! ignoring it, and it is deliberate.
//!
//! A download carrying a key that is not this project's key is not a download
//! with one odd file in it. It is a release signed by somebody else, packaged
//! to be checked with their key, and the only reason the archive and the hash
//! list are also there is to make the set look complete. Going on to report
//! that the archive matches its own hash list would be true, useless and read
//! as a pass. There is nothing further worth saying, so nothing further is
//! said.
//!
//! The absence of the file is a different fact and is not a failure: releases
//! carry one, an archive copied out of a folder on its own does not, and the
//! embedded key is what does the checking either way.
//!
//! # In plain words
//!
//! A download can come with its own copy of the signing key. This checks that
//! it is the same key VeilVoice was built with, and never uses it for anything
//! else.
//!
//! If a download brings a different key, that is the answer, and nothing else
//! about the download matters.

use std::path::{Path, PathBuf};

use pgp::composed::{Deserializable, SignedPublicKey};

use crate::check::{fingerprint_of, FINGERPRINT};

/// The name a release publishes its public key under.
pub const KEY_FILE: &str = "veilvoice-signing-key.asc";

/// What was found where a carried key would be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Carried {
    /// There is no key file beside the archive.
    ///
    /// Not a failure. See the module note.
    None,
    /// There is one, and its fingerprint is the one compiled in.
    ///
    /// Worth saying out loud and worth saying carefully: this means the
    /// download is *consistent* with the key this binary carries. It is not
    /// what verified the signature and it never will be.
    Ours,
    /// There is one, and it is somebody else's key.
    ///
    /// The whole verdict. Nothing after this is reported.
    Foreign {
        /// The fingerprint the carried key actually has.
        fingerprint: String,
    },
    /// There is one and it could not be read.
    ///
    /// Reported as its own case rather than folded into [`Self::Foreign`]: a
    /// truncated download and a substituted key are different facts, and this
    /// program does not tell somebody they may have been attacked when what
    /// happened is that a file arrived short.
    Unreadable {
        /// Why it could not be read.
        why: String,
    },
}

impl Carried {
    /// Whether this ends the check with nothing further worth reporting.
    pub fn is_the_whole_verdict(&self) -> bool {
        matches!(self, Self::Foreign { .. })
    }
}

/// Where a release's key file would be, beside these files.
pub fn beside(directory: &Path) -> Option<PathBuf> {
    let path = directory.join(KEY_FILE);
    path.is_file().then_some(path)
}

/// Read the key a download carries, and compare it with the one compiled in.
///
/// **Compare, never use.** Nothing in this function returns the parsed key to
/// a caller, and that is not an oversight: a function that handed back a
/// `SignedPublicKey` read off the disk would be one call site away from
/// verifying a signature with it, which is the one thing this module exists to
/// prevent. The answer is a verdict, and the verdict is all there is.
pub fn examine(path: &Path) -> Carried {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(why) => {
            return Carried::Unreadable {
                why: format!("cannot read {}: {why}", path.display()),
            }
        }
    };
    let key: SignedPublicKey = match SignedPublicKey::from_string(&text) {
        Ok((key, _)) => key,
        Err(why) => {
            return Carried::Unreadable {
                why: format!("it does not parse as an OpenPGP public key: {why}"),
            }
        }
    };
    let fingerprint = fingerprint_of(&key);
    if fingerprint == FINGERPRINT {
        Carried::Ours
    } else {
        Carried::Foreign { fingerprint }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key a genuine release carries is the key compiled in.
    ///
    /// The release job exports the same key it signs with, so this is the
    /// arrangement every real download has and the one a reader will meet.
    #[test]
    fn the_projects_own_key_is_recognised_as_ours() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_FILE);
        std::fs::write(&path, crate::check::PUBLIC_KEY).unwrap();
        assert_eq!(examine(&path), Carried::Ours);
        assert!(!examine(&path).is_the_whole_verdict());
        assert_eq!(beside(dir.path()), Some(path));
    }

    /// A download carrying somebody else's key is refused on that alone.
    ///
    /// The forged-set case from the module note, built as close to the real
    /// thing as a test can get: a well-formed OpenPGP public key that is not
    /// this project's. It has to be [`Carried::Foreign`] and it has to be the
    /// whole verdict, because a verifier that noticed this and carried on
    /// reporting hash matches would be reporting on a release signed by
    /// somebody else.
    #[test]
    fn somebody_elses_key_is_the_whole_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_FILE);
        std::fs::write(&path, ANOTHER_KEY).unwrap();
        match examine(&path) {
            Carried::Foreign { fingerprint } => {
                assert_ne!(fingerprint, FINGERPRINT);
                assert!(examine(&path).is_the_whole_verdict());
            }
            other => panic!("a key that is not ours must be Foreign, got {other:?}"),
        }
    }

    /// A key file that is not a key is "could not read", not "attacked".
    #[test]
    fn a_damaged_key_file_is_not_reported_as_a_substitution() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(KEY_FILE);
        std::fs::write(&path, b"-----BEGIN PGP PUBLIC KEY BLOCK-----\nnot a key\n").unwrap();
        assert!(matches!(examine(&path), Carried::Unreadable { .. }));
        assert!(!examine(&path).is_the_whole_verdict());
    }

    /// No key file beside the archive is not a failure.
    #[test]
    fn a_release_with_no_key_file_is_not_a_finding() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(beside(dir.path()), None);
    }

    /// A public key that is not this project's, for the test above.
    ///
    /// An ordinary Ed25519 signing key, generated once with GnuPG and its
    /// public half pasted in. Its private half was never kept and it signs
    /// nothing: the only property under test is that its fingerprint,
    /// `AE63F2CF7F0C458AC6976484F44A8A1372B5D35D`, is not
    /// [`crate::check::FINGERPRINT`].
    ///
    /// A fixed file rather than a key made at test time, so the test build
    /// carries no key generator and the thing being compared against is a
    /// constant, which is what it is in the release this models.
    const ANOTHER_KEY: &str = include_str!("testdata/not-our-key.asc");
}
