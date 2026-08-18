// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! The verifier's own tests.
//!
//! The property that matters most here is not "a good signature is accepted"
//! but **"a bad one is refused"**. A verifier that accepts everything passes
//! every happy-path test ever written, and would ship looking perfect while
//! doing the opposite of its job -- so most of what follows is negative:
//! corrupted signatures, wrong keys, truncated input, mismatched hashes.
//!
//! This file is `//!`-documented rather than `//`-commented so that the
//! reasoning above appears in the generated documentation. A reader deciding
//! whether to trust `veilvoice-verify` should be able to see what it was tested
//! *against* without cloning the repository, because the whole purpose of that
//! binary is to be the thing you check a download with.

use super::*;

#[test]
fn the_embedded_key_parses_and_is_the_expected_one() {
    let key = embedded_key().expect("the compiled-in key must parse");
    assert_eq!(fingerprint_of(&key), FINGERPRINT);
}

#[test]
fn the_embedded_key_carries_no_email_address() {
    // The pseudonym rule, enforced where it would actually ship: this key is
    // compiled into a binary handed to strangers. Its user ID is `tilas01`
    // and nothing else.
    let key = embedded_key().unwrap();
    for uid in key.details.users.iter() {
        let id = String::from_utf8_lossy(uid.id.id()).into_owned();
        assert!(
            !id.contains('@'),
            "the signing key's user id contains an address: {id}"
        );
    }
}

#[test]
fn the_fingerprint_constant_is_written_out_not_computed() {
    // A weak but real check that the constant is a literal: 40 uppercase hex
    // characters. If somebody replaces it with something derived from the key
    // itself, the comparison in `embedded_key` stops being a check at all.
    assert_eq!(FINGERPRINT.len(), 40);
    assert!(FINGERPRINT
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase()));
}

// --- the hash-list parser --------------------------------------------------

#[test]
fn a_hash_is_found_by_its_file_name() {
    let sums = "\
aaaa  first.tar.gz
bbbb  second.tar.gz
";
    assert_eq!(
        digest_from_sums(sums, "second.tar.gz").as_deref(),
        Some("bbbb")
    );
}

#[test]
fn a_binary_mode_star_is_not_part_of_the_name() {
    // `sha256sum` writes `hash *name` in binary mode, and that asterisk is not
    // part of the file name. Treating it as one would make every hash list
    // produced on Windows look like it mentioned no files at all.
    let sums = "cccc *archive.zip\n";
    assert_eq!(
        digest_from_sums(sums, "archive.zip").as_deref(),
        Some("cccc")
    );
}

#[test]
fn a_file_that_is_not_listed_is_not_found() {
    let sums = "aaaa  first.tar.gz\n";
    assert!(digest_from_sums(sums, "other.tar.gz").is_none());
}

#[test]
fn a_name_that_merely_contains_the_wanted_one_does_not_match() {
    // `evil-archive.zip` must not satisfy a request for `archive.zip`.
    let sums = "aaaa  evil-archive.zip\n";
    assert!(digest_from_sums(sums, "archive.zip").is_none());
}

#[test]
fn blank_and_comment_lines_are_skipped() {
    let sums = "\n# a comment\n\naaaa  first.tar.gz\n";
    assert_eq!(
        digest_from_sums(sums, "first.tar.gz").as_deref(),
        Some("aaaa")
    );
}

#[test]
fn a_malformed_line_is_skipped_rather_than_panicking() {
    let sums = "nowhitespaceatall\naaaa  first.tar.gz\n";
    assert_eq!(
        digest_from_sums(sums, "first.tar.gz").as_deref(),
        Some("aaaa")
    );
}

// --- digest comparison -----------------------------------------------------

#[test]
fn digests_compare_case_insensitively_and_ignore_surrounding_space() {
    assert!(digests_match("ABCDEF", "abcdef"));
    assert!(digests_match("  abcdef\n", "abcdef"));
    assert!(!digests_match("abcdef", "abcde0"));
}

// --- signature verification ------------------------------------------------

#[test]
fn a_signature_that_is_not_openpgp_is_refused() {
    let key = embedded_key().unwrap();
    let result = verify_detached(&key, "this is not a signature", b"data");
    assert!(result.is_err(), "arbitrary text must not verify");
}

#[test]
fn an_empty_signature_is_refused() {
    let key = embedded_key().unwrap();
    assert!(verify_detached(&key, "", b"data").is_err());
}

#[test]
fn an_armoured_block_that_is_not_a_signature_is_refused() {
    // The public key is a valid OpenPGP armoured block, and is not a
    // signature. Feeding it in must fail to parse rather than be accepted by
    // something that only checked "does this look armoured".
    let key = embedded_key().unwrap();
    assert!(verify_detached(&key, PUBLIC_KEY, b"data").is_err());
}
