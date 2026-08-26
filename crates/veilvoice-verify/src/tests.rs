// SPDX-License-Identifier: GPL-3.0-or-later
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
    assert!(verify_detached(&key, veilvoice_check::PUBLIC_KEY, b"data").is_err());
}

// ---------------------------------------------------------------------------
// The quiet level is only as good as the last line nobody gated
// ---------------------------------------------------------------------------

/// **Nothing may print without asking the level first.**
///
/// `--quiet` is a promise that this program says nothing, and the exit status
/// is the whole answer. One forgotten `println!` breaks that promise, and it
/// breaks it invisibly: every test still passes, the output is still correct
/// at the default level, and the only reader who finds out is the one running
/// it in a pipeline where a stray line is a parse error.
///
/// So the source itself is checked. Every `print!` and `println!` in `main.rs`
/// must be inside one of the three macros that gate on the level, or in the
/// short list below of things that are not reports about a check.
#[test]
fn every_line_printed_by_a_check_goes_through_the_level() {
    let source = include_str!("main.rs");

    // What is allowed to print unconditionally, and why.
    //
    // `--help`, `--version`, `--explain` and `--exit-status` print the thing
    // that was asked for rather than reporting on a check. Somebody who types
    // `--help` wants the help, whatever level they also passed.
    let asked_for_directly = [
        "print!(\"{USAGE}\");",
        "print!(\"{EXPLAIN}\");",
        "print!(\"{}\", Loudness::table());",
        "print!(\"{}\", Status::table());",
        "println!(\"veilvoice-verify {}\", env!(\"CARGO_PKG_VERSION\"));",
        "println!();",
    ];

    // The macro bodies themselves, which are where the gating lives.
    let inside_a_gate = [
        r#"            println!($($arg)*);"#,
        r#"            print!($($arg)*);"#,
        r#"            println!("        {}", format!($($arg)*));"#,
    ];

    let mut ungated = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.contains("println!(") || trimmed.contains("print!(")) {
            continue;
        }
        // Documentation and comments talk about printing; they do not print.
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("eprintln!") || trimmed.contains("eprintln!(") {
            continue;
        }
        if inside_a_gate.contains(&line) {
            continue;
        }
        if asked_for_directly.contains(&trimmed) {
            continue;
        }
        ungated.push(format!("main.rs:{}: {trimmed}", number + 1));
    }

    assert!(
        ungated.is_empty(),
        "these lines print without asking the level, so `--quiet` is not quiet:\n{}",
        ungated.join("\n")
    );
}

/// The counterpart: standard error is gated too, and by the same rule.
///
/// A refusal is the one thing a `--quiet` reader most plausibly still wants,
/// and the answer is still no -- they asked for nothing, and the exit status
/// carries it. What must not happen is *some* refusals honouring that and
/// others not.
#[test]
fn every_line_of_a_refusal_goes_through_the_level_too() {
    let source = include_str!("main.rs");
    let mut outside = Vec::new();
    let mut depth_of_gate: Option<usize> = None;

    for (number, line) in source.lines().enumerate() {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        if trimmed.starts_with("if report::level() >= Loudness::") {
            depth_of_gate = Some(indent);
            continue;
        }
        if let Some(gate) = depth_of_gate {
            if trimmed == "}" && indent == gate {
                depth_of_gate = None;
                continue;
            }
        }
        if trimmed.starts_with("eprintln!") && depth_of_gate.is_none() {
            outside.push(format!("main.rs:{}: {trimmed}", number + 1));
        }
    }

    assert!(
        outside.is_empty(),
        "these refusal lines print without asking the level:\n{}",
        outside.join("\n")
    );
}

/// Every exit this program can take is one of the documented statuses.
///
/// `ExitCode::FAILURE` is the shape this used to have and the one to keep out:
/// it is 1, which now means "the command line could not be understood", so a
/// leftover `FAILURE` would report a bad signature as a typing mistake.
#[test]
fn nothing_exits_with_an_undocumented_status() {
    let source = include_str!("main.rs");
    let mut stray = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("///") {
            continue;
        }
        if trimmed.contains("ExitCode::FAILURE") {
            stray.push(format!("main.rs:{}: {trimmed}", number + 1));
        }
    }
    assert!(
        stray.is_empty(),
        "ExitCode::FAILURE is 1, which now means a usage error. Use a Status:\n{}",
        stray.join("\n")
    );
}
