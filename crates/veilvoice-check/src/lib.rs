// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![warn(missing_docs)]
//! Check a VeilVoice release: a file's SHA-256, its line in a `SHA256SUMS`,
//! and the detached OpenPGP signature over that list.
//!
//! # Why this is a library and not only a program
//!
//! `veilvoice-verify` did all of this and did it inside a binary crate, which
//! has no consumers by construction. The desktop application was asked for a
//! **verify tab** — drag a download onto the window and be told whether it is
//! the published one — and there were two ways to have that:
//!
//! * link `eframe` into the 1.5 MB portable verifier, which is the one binary
//!   in this project whose smallness is a feature: it is what somebody
//!   downloads *before* they trust anything else here;
//! * or move the checking out of the binary, so both front ends call the same
//!   code rather than two implementations of it.
//!
//! This is the second. The verifier is unchanged in what it does and prints;
//! it just no longer owns the arithmetic.
//!
//! # What a pass actually proves, and what it does not
//!
//! A good signature over `SHA256SUMS`, plus a matching hash, proves the file is
//! **the one the holder of this key published**. It does not prove the file is
//! safe, that the source compiles to it, or that the key belongs to anybody in
//! particular. The second of those is the check worth having and it is the one
//! this project cannot perform for you — it needs somebody other than the
//! author to have built the same tag and got the same hash.
//!
//! Every front end that uses this has to say so. The words are here, in
//! [`SCOPE`], rather than in whichever interface happens to be showing the
//! answer, so a second front end cannot show a pass without them.
//!
//! # No network, and no GnuPG
//!
//! Nothing in this crate opens a socket or runs a program. It is given bytes
//! and it reports on them. Downloading is the caller's problem, and the two
//! callers that do it shell out to the system's own transfer tool.

use std::fmt::Write as _;
use std::path::Path;

use pgp::composed::{Deserializable, DetachedSignature, SignedPublicKey};
use pgp::types::KeyDetails as _;
use sha2::{Digest, Sha256};

/// The project's signing key, in ASCII armour.
///
/// Read from the copy the website serves, so there is exactly one key file in
/// this repository and no chance of a second one drifting from it. A test
/// asserts this key's fingerprint is [`FINGERPRINT`]; if somebody swaps the
/// file, the build fails rather than a verifier trusting a new key.
pub const PUBLIC_KEY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../website/assets/veilvoice-signing-key.asc"
));

/// The fingerprint, written out rather than derived.
///
/// Deriving it from [`PUBLIC_KEY`] would make this constant agree with the key
/// automatically, which sounds like an improvement and is the opposite of one:
/// the whole point is that a reader can compare this string against the one
/// published in `README.md`, on the website and in the release notes. A value
/// computed from the very file it is meant to authenticate checks nothing.
pub const FINGERPRINT: &str = "8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A";

/// What a passing check is worth, in the words a user should be shown.
pub const SCOPE: &str = "\
A good signature and a matching hash prove this file is the one the holder of \
this key published. They do not prove it is safe, that the source compiles to \
it, or that the key belongs to anybody in particular. The check worth more \
than any of these is somebody other than the author building the same tag and \
getting the same hash -- and that is a check this program cannot perform for \
you.";

/// Something that went wrong, in words a person can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A file could not be read.
    Io(String),
    /// The embedded key, or a signature, does not parse.
    Malformed(String),
    /// The signature did not verify against this key.
    ///
    /// Distinct from the others on purpose: everything else means "the check
    /// could not be completed", and this one means "the check completed and
    /// the answer is no".
    BadSignature,
    /// The list has no line for this file.
    NotListed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(what) => write!(f, "{what}"),
            Self::Malformed(what) => write!(f, "{what}"),
            Self::BadSignature => write!(f, "the signature was not made by this key"),
            Self::NotListed(name) => {
                write!(f, "{name} is not listed in that SHA256SUMS")
            }
        }
    }
}

impl std::error::Error for Error {}

/// The embedded key, with its fingerprint checked.
pub fn key() -> Result<SignedPublicKey, Error> {
    let (key, _) = SignedPublicKey::from_string(PUBLIC_KEY)
        .map_err(|e| Error::Malformed(format!("the embedded public key does not parse: {e}")))?;
    let actual = fingerprint_of(&key);
    if actual != FINGERPRINT {
        return Err(Error::Malformed(format!(
            "the embedded key's fingerprint is {actual}, not {FINGERPRINT}"
        )));
    }
    Ok(key)
}

/// A key's fingerprint, uppercase hex, no spaces.
pub fn fingerprint_of(key: &SignedPublicKey) -> String {
    let mut out = String::new();
    for byte in key.fingerprint().as_bytes() {
        let _ = write!(out, "{byte:02X}");
    }
    out
}

/// SHA-256 of a file, read in chunks.
///
/// Streamed rather than read whole: a release archive is tens of megabytes and
/// there is no reason for this to need that much memory at once. The web
/// verifier had the same problem in the other direction -- finding F-36.
pub fn sha256_file(path: &Path) -> Result<String, Error> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .map_err(|e| Error::Io(format!("cannot open {}: {e}", path.display())))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| Error::Io(format!("cannot read {}: {e}", path.display())))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

/// SHA-256 of bytes already in memory.
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Compare two hex digests without caring about case or stray whitespace.
///
/// Not constant time, and deliberately so: both values are public, and there
/// is no secret here for a timing difference to leak. Saying that plainly is
/// better than a `subtle` dependency that implies there was a threat.
pub fn digests_match(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Find a file's line in a `sha256sum`-format list.
///
/// The format is `<hex>  <name>`, and `sha256sum` writes a `*` before the name
/// for a binary-mode hash. Only the file's base name is compared: the list is
/// written with plain names, and the file being checked is usually somewhere
/// else entirely.
pub fn digest_from_sums(sums: &str, wanted: &str) -> Option<String> {
    for line in sums.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `continue`, not `?`. A `?` here returns from the whole function on
        // the first line that has no whitespace in it, so one malformed line
        // near the top of a list makes every hash below it invisible -- and
        // the answer would be "not listed", which reads as "wrong release"
        // rather than as "this file is unreadable". The portable verifier's
        // own test caught this within a minute of the code moving here.
        let Some((digest, name)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let name = name.trim().trim_start_matches('*');
        // A line like `aaaa   ` has a digest and no name. Without this it
        // matches a `wanted` of "" -- which is what a path with no final
        // component gives -- and the caller is handed a digest for a file that
        // was never listed. Nothing downstream would notice: it looks exactly
        // like a successful lookup.
        if name.is_empty() {
            continue;
        }
        if name == wanted {
            return Some(digest.trim().to_string());
        }
    }
    None
}

/// Every name a `SHA256SUMS` lists, in order.
///
/// For an interface that wants to say *what it could have checked* when the
/// file dropped on it is not in the list. "Not listed" on its own leaves the
/// reader guessing whether they downloaded the wrong thing or the wrong
/// release.
pub fn names_in_sums(sums: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in sums.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((_, name)) = line.split_once(char::is_whitespace) {
            let name = name.trim().trim_start_matches('*');
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// Verify a detached signature over `data` using `key`.
pub fn verify_detached(key: &SignedPublicKey, signature: &str, data: &[u8]) -> Result<(), Error> {
    let (signature, _) = DetachedSignature::from_string(signature).map_err(|e| {
        Error::Malformed(format!("the signature file does not parse as OpenPGP: {e}"))
    })?;

    // Try the primary key and every subkey. Release signatures are normally
    // made by a signing subkey rather than the primary -- checking only the
    // primary would reject every genuine signature this project has ever made.
    if signature.verify(key, data).is_ok() {
        return Ok(());
    }
    for subkey in &key.public_subkeys {
        if signature.verify(subkey, data).is_ok() {
            return Ok(());
        }
    }
    Err(Error::BadSignature)
}

/// What a full check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// The name looked up in the list.
    pub name: String,
    /// The hash of the file on disk.
    pub actual: String,
    /// The hash the signed list gives for it.
    pub expected: String,
    /// Whether the two are the same.
    pub matched: bool,
    /// The fingerprint of the key the signature verified against.
    pub fingerprint: String,
}

/// The whole check: signature first, then the hash.
///
/// # The order is the point
///
/// The signature is verified over the **bytes of the list** before any number
/// in that list is read. A checker that compared the hash first and verified
/// afterwards would, for the moment between the two, be trusting an unsigned
/// document — and an attacker who can hand you a file can hand you a
/// `SHA256SUMS` to go with it. Getting this order wrong produces a program that
/// passes all its own tests and proves nothing.
pub fn check_file(file: &Path, sums: &str, signature: &str) -> Result<Checked, Error> {
    let key = key()?;
    verify_detached(&key, signature, sums.as_bytes())?;

    // A path with no final component -- a directory, a root, `..` -- has no
    // name to look up, and asking for one would be asking the list about
    // nothing. Refused here rather than turned into an empty string that some
    // malformed line might happen to match.
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| Error::Io(format!("{} does not name a file", file.display())))?;
    let expected = digest_from_sums(sums, &name).ok_or_else(|| Error::NotListed(name.clone()))?;
    let actual = sha256_file(file)?;

    Ok(Checked {
        matched: digests_match(&actual, &expected),
        name,
        actual,
        expected,
        fingerprint: fingerprint_of(&key),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn the_embedded_key_parses_and_is_the_published_fingerprint() {
        let key = key().expect("the embedded key must parse");
        assert_eq!(fingerprint_of(&key), FINGERPRINT);
    }

    #[test]
    fn a_known_vector_hashes_correctly() {
        // The SHA-256 of the empty string, which is the one digest anybody can
        // check against a reference without running this code.
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_file_hashes_the_same_as_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("thing.bin");
        // Larger than the 64 KiB read buffer, so the chunked path is the one
        // being tested rather than a single read.
        let data: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
        std::fs::File::create(&path)
            .unwrap()
            .write_all(&data)
            .unwrap();
        assert_eq!(sha256_file(&path).unwrap(), sha256_bytes(&data));
    }

    #[test]
    fn digests_compare_without_caring_about_case_or_space() {
        assert!(digests_match("ABCD", "abcd"));
        assert!(digests_match("  abcd\n", "abcd"));
        assert!(!digests_match("abcd", "abce"));
        assert!(!digests_match("", "abcd"));
    }

    #[test]
    fn a_sums_line_is_found_by_base_name_in_either_mode() {
        let sums = "\
# a comment
aaaa  plain.txt
bbbb *binary.bin

cccc  spaced name.zip
";
        assert_eq!(digest_from_sums(sums, "plain.txt").as_deref(), Some("aaaa"));
        assert_eq!(
            digest_from_sums(sums, "binary.bin").as_deref(),
            Some("bbbb")
        );
        assert_eq!(
            digest_from_sums(sums, "spaced name.zip").as_deref(),
            Some("cccc")
        );
        assert_eq!(digest_from_sums(sums, "missing"), None);
    }

    /// A line with a digest and no name must not answer a lookup for "",
    /// which is what a path with no final component gives.
    #[test]
    fn a_nameless_line_matches_nothing() {
        let sums = "aaaa   \nbbbb  real.zip\n";
        assert_eq!(digest_from_sums(sums, ""), None);
        assert_eq!(digest_from_sums(sums, "real.zip").as_deref(), Some("bbbb"));
    }

    /// And a path with nothing to look up is refused rather than turned into
    /// that empty string in the first place.
    #[test]
    fn a_path_with_no_file_name_is_refused() {
        let error = check_file(std::path::Path::new(".."), "aaaa  x\n", "sig")
            .expect_err("`..` names no file");
        // Refused before the signature is even parsed would be wrong too; what
        // matters is that it never reaches a lookup with an empty name.
        assert!(!matches!(error, Error::NotListed(ref n) if n.is_empty()));
    }

    #[test]
    fn the_names_can_be_listed_for_an_error_message() {
        let sums = "aaaa  one.zip\nbbbb *two.tar.gz\n# note\n";
        assert_eq!(names_in_sums(sums), vec!["one.zip", "two.tar.gz"]);
        assert!(names_in_sums("").is_empty());
    }

    /// The order this checks things in is the whole of its value, so it is
    /// asserted rather than assumed: an unverifiable signature must stop the
    /// check before any hash from that list is read.
    #[test]
    fn a_bad_signature_stops_the_check_before_any_hash_is_believed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("release.zip");
        std::fs::write(&path, b"pretend this is a release").unwrap();
        let sums = format!("{}  release.zip\n", sha256_file(&path).unwrap());

        // A hash that would match perfectly, under a signature that is not one.
        let error = check_file(&path, &sums, "not an OpenPGP signature at all")
            .expect_err("an unparseable signature cannot pass");
        assert!(
            matches!(error, Error::Malformed(_)),
            "expected a parse failure, got {error:?}"
        );
    }

    /// "Not listed" is a different answer from "does not match", and a front
    /// end has to be able to tell them apart: one means the wrong release, the
    /// other means a bad file.
    #[test]
    fn a_file_missing_from_the_list_is_its_own_answer() {
        let sums = "aaaa  something-else.zip\n";
        assert_eq!(digest_from_sums(sums, "mine.zip"), None);
        let error = Error::NotListed("mine.zip".into());
        assert!(error.to_string().contains("mine.zip"));
        assert_ne!(error, Error::BadSignature);
    }

    /// The scope note has to state the limits, not only the capability. Every
    /// front end shows this text, so it is checked here once.
    #[test]
    fn the_scope_note_says_what_a_pass_does_not_prove() {
        let scope = SCOPE.to_lowercase();
        for phrase in [
            "do not prove it is safe",
            "the source compiles to it",
            "belongs to anybody in particular",
            "somebody other than the author",
        ] {
            assert!(scope.contains(phrase), "the scope note must say {phrase:?}");
        }
    }
}
