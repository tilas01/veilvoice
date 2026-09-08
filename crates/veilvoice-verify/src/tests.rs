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
//!
//! # In plain words
//!
//! The verifier's own tests, and most of them are about failure rather than
//! success.
//!
//! That is deliberate. A verifier that accepted everything would pass every
//! happy-path test ever written and would ship looking perfect while doing the
//! opposite of its job. So most of what is here is corrupted signatures, wrong
//! keys, truncated files and mismatched hashes, and the question each time is
//! whether it says no.

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
/// So the source itself is checked. Every `print!`, `println!` and `eprintln!`
/// in `lib.rs` must be reached through one of the three macros that gate on
/// the level, or from inside an explicit `if report::level() >= ...` block, or
/// be one of the few lines that are not reports about a check at all.
///
/// Both streams, in one pass. They were two tests to begin with, and the
/// standard-output one did not understand the explicit gate, so the first four
/// commands written after it were flagged for doing exactly the right thing.
/// A rule enforced two ways is a rule with two definitions.
#[test]
fn every_line_printed_by_a_check_goes_through_the_level() {
    // A source-reading test, so the line endings have to be settled first.
    // F-72: these searched for "\n}\n" and passed on every machine
    // whose checkout uses LF. GitHub's Windows runners default to
    // core.autocrlf=true, so the file arrives with CRLF, the pattern
    // matches nothing, and three tests failed there and nowhere else --
    // including on the developer machine that had just run them.
    // Normalised here as well as pinned in .gitattributes: a test that
    // depends on a git setting is a test somebody will trip over.
    let source = include_str!("lib.rs").replace("\r\n", "\n");

    // What is allowed to print unconditionally, and why.
    //
    // The single exception, and it lives in one function so that this list
    // has one entry rather than one per command that prints its own help.
    let asked_for_directly = ["print!(\"{text}\");"];

    // The macro bodies themselves, which are where the gating lives.
    let inside_a_macro = [
        r#"            println!($($arg)*);"#,
        r#"            print!($($arg)*);"#,
        r#"            println!("        {}", format!($($arg)*));"#,
    ];

    let mut ungated = Vec::new();
    let mut gate: Option<usize> = None;

    for (number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let indent = line.len() - line.trim_start().len();

        // An explicit gate opens here and closes at the brace that matches its
        // indentation. Tracked rather than matched by regex because the block
        // is several lines long and every line inside it is protected.
        if trimmed.starts_with("if report::level() >= Loudness::")
            || trimmed.starts_with("if crate::report::level() >= ")
        {
            gate = Some(indent);
            continue;
        }
        if let Some(opened) = gate {
            if trimmed == "}" && indent == opened {
                gate = None;
                continue;
            }
        }

        let prints = trimmed.contains("println!(")
            || trimmed.contains("print!(")
            || trimmed.contains("eprintln!(");
        if !prints || trimmed.starts_with("//") {
            continue;
        }
        if gate.is_some() || inside_a_macro.contains(&line) {
            continue;
        }
        if asked_for_directly.contains(&trimmed) {
            continue;
        }
        ungated.push(format!("lib.rs:{}: {trimmed}", number + 1));
    }

    assert!(
        ungated.is_empty(),
        "these lines print without asking the level, so `--quiet` is not quiet:\n{}",
        ungated.join("\n")
    );
}

/// Every exit this program can take is one of the documented statuses.
///
/// `ExitCode::FAILURE` is the shape this used to have and the one to keep out:
/// it is 1, which now means "the command line could not be understood", so a
/// leftover `FAILURE` would report a bad signature as a typing mistake.
#[test]
fn nothing_exits_with_an_undocumented_status() {
    let source = include_str!("lib.rs").replace("\r\n", "\n");
    let mut stray = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("///") {
            continue;
        }
        if trimmed.contains("ExitCode::FAILURE") {
            stray.push(format!("lib.rs:{}: {trimmed}", number + 1));
        }
    }
    assert!(
        stray.is_empty(),
        "ExitCode::FAILURE is 1, which now means a usage error. Use a Status:\n{}",
        stray.join("\n")
    );
}

// ---------------------------------------------------------------------------
// A test that reads source has to say which line endings it expects
// ---------------------------------------------------------------------------

/// **F-72.** Every `include_str!` of this project's own source is normalised
/// before it is searched.
///
/// Three tests here searched for `"\n}\n"` and passed on every machine whose
/// checkout uses LF. GitHub's Windows runners default to `core.autocrlf=true`,
/// so the file arrives with CRLF, the pattern matches nothing, and the tests
/// failed there and nowhere else -- including on the Windows machine that had
/// just run them and watched them pass, because its git is set to `input`.
///
/// `.gitattributes` now pins the whole tree to LF, which is the real fix and
/// also protects every generator's byte-for-byte `--check`. This is the second
/// line of defence, because a test that depends on a git setting is a test
/// somebody will trip over on a machine nobody here owns.
#[test]
fn every_test_that_reads_source_normalises_its_line_endings() {
    let source = include_str!("tests.rs").replace("\r\n", "\n");
    // Assembled at run time so this line does not contain the thing it looks
    // for. Written out in full, the guard matched itself and reported its own
    // detection as the defect -- an honest failure, and a useless one.
    let invocation = concat!("include_str", "!(");
    let normalised = concat!(".repl", "ace(");

    let mut bare = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || !trimmed.contains(invocation) {
            continue;
        }
        if !trimmed.contains(normalised) {
            bare.push(format!("tests.rs:{}: {trimmed}", number + 1));
        }
    }

    assert!(
        bare.is_empty(),
        "these read source without settling the line endings first:\n{}",
        bare.join("\n")
    );
}

/// The failure mode itself, so it is on record as reachable rather than
/// theoretical.
///
/// This is what the three failing tests were doing, against the two forms the
/// same file takes on two machines.
#[test]
fn searching_for_a_brace_on_its_own_line_fails_against_crlf() {
    let lf = "fn thing() {\n    ()\n}\n\nfn next() {}\n";
    let crlf = lf.replace('\n', "\r\n");

    assert!(lf.find("\n}\n").is_some(), "LF is what the tests assumed");
    assert!(
        crlf.find("\n}\n").is_none(),
        "if this ever matches, the defect was something else"
    );
    // And the fix, applied to the awkward form.
    assert!(crlf.replace("\r\n", "\n").find("\n}\n").is_some());
}

/// `.gitattributes` exists and pins text to LF.
///
/// Checked from a test rather than trusted, because it is the thing that keeps
/// every generator's byte comparison honest on a contributor's machine, and
/// nothing else in the build would notice it being deleted.
#[test]
fn the_repository_pins_its_line_endings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let attributes = std::fs::read_to_string(root.join(".gitattributes"))
        .expect(".gitattributes is what keeps the generators' byte checks honest");
    assert!(
        attributes.contains("* text=auto eol=lf"),
        "text has to be pinned to LF for every checkout:\n{attributes}"
    );
    // And the formats where a wrong guess corrupts a file silently.
    for binary in ["*.png", "*.wav", "*.gif"] {
        assert!(
            attributes.contains(&format!("{binary}   binary"))
                || attributes.contains(&format!("{binary}  binary"))
                || attributes.contains(&format!("{binary} binary")),
            "{binary} must be marked binary:\n{attributes}"
        );
    }
}

/// **Marker 97.** The contents list decides which paths get read and what they
/// are compared against, so it is checked against the signed hash list *before*
/// it is parsed.
///
/// Written as a test over the source because the ordering is the whole
/// property and it cannot be observed from outside: a version that parsed
/// first and checked afterwards would give the same answers on every good
/// release and would be doing what a downloaded text file told it to on a bad
/// one.
#[test]
fn the_contents_list_is_verified_before_it_is_parsed() {
    let source = include_str!("lib.rs").replace("\r\n", "\n");
    let body = source
        .split("fn manifest(")
        .nth(1)
        .expect("the manifest reader has to be findable");
    let body = body.split("\nfn ").next().unwrap();
    let checked = body
        .find("check_file")
        .expect("the contents list must be checked against the signed hash list");
    let parsed = body.find("contents::parse").expect("and then parsed");
    assert!(
        checked < parsed,
        "the contents list is parsed before it is verified"
    );
}

/// A release that published no contents list is still checkable.
///
/// Everything before v0.1.15 is in that position, and a verifier that refused
/// them would be refusing files it can check perfectly well. `None` is a state,
/// not an error.
#[test]
fn a_release_without_a_contents_list_is_not_a_failure() {
    let found = discover::Found {
        directory: std::path::PathBuf::from("."),
        archives: Vec::new(),
        sums: None,
        signature: None,
        contents: None,
    };
    assert!(matches!(manifest(&found), Manifest::None));
}

/// A contents list with no signed hash list beside it cannot be used, and
/// "cannot be used" is reported rather than quietly skipped.
#[test]
fn a_contents_list_with_nothing_to_check_it_against_is_unusable() {
    let found = discover::Found {
        directory: std::path::PathBuf::from("."),
        archives: Vec::new(),
        sums: None,
        signature: None,
        contents: Some(std::path::PathBuf::from("CONTENTS.sha256")),
    };
    match manifest(&found) {
        Manifest::Unusable(why) => assert!(why.contains("signed hash list"), "{why}"),
        other => panic!("expected Unusable, got {}", matches_name(&other)),
    }
}

/// A name for a [`Manifest`], for a failing assertion to print.
fn matches_name(manifest: &Manifest) -> &'static str {
    match manifest {
        Manifest::None => "None",
        Manifest::Unusable(_) => "Unusable",
        Manifest::Ready(_) => "Ready",
    }
}

/// **Marker 97.** GnuPG being unusable is not a statement about the download.
///
/// The distinction is the one a verifier is most tempted to get wrong: a
/// missing keyring directory reads like a failure, and reporting it as one
/// tells somebody not to run a release that is entirely sound. Only an answer
/// from GnuPG counts, and only a bad answer counts against.
#[test]
fn a_gnupg_that_cannot_run_is_never_counted_against_the_release() {
    let source = include_str!("lib.rs").replace("\r\n", "\n");
    let body = source
        .split("fn report_gnupg(")
        .nth(1)
        .expect("the GnuPG report has to be findable");
    let body = body.split("\nfn ").next().unwrap();
    for arm in [
        "the signing key could not be added to your keyring",
        "GnuPG could not check the signature",
    ] {
        let at = body.find(arm).unwrap_or_else(|| panic!("{arm} is gone"));
        // The next `problems += 1` must belong to a later arm, not this one.
        let rest = &body[at..];
        let next_arm = rest.find("Nothing about the download changed");
        let next_count = rest.find("problems += 1");
        assert!(
            next_arm.is_some() && next_arm < next_count,
            "{arm} counts against the release"
        );
    }
}

/// **F-108.** A directory that was named has to exist, or nothing is checked.
///
/// The search falls back through the current directory, the folder holding
/// this program, Downloads and Desktop, which is right when nobody said where
/// to look and wrong the moment somebody does. Naming a directory that is not
/// there used to fall through to that list, check whatever it turned up, print
/// INTACT and exit 0, without the path the person typed appearing anywhere.
///
/// Read out of the source rather than by running the binary, because the
/// failure needs a machine with a release lying around somewhere findable to
/// reproduce, which is exactly the condition that made it invisible. What has
/// to stay true is that `command_auto` refuses before it searches.
#[test]
fn a_named_directory_that_is_not_there_is_refused_before_anything_is_searched() {
    let source = include_str!("lib.rs").replace("\r\n", "\n");
    let body = source
        .split("fn command_auto(")
        .nth(1)
        .expect("command_auto has to be findable");
    let body = body.split("\nfn ").next().unwrap();

    let guard = body
        .find("is_dir()")
        .expect("command_auto no longer checks that a named directory exists");
    let search = body
        .find("discover::search(")
        .expect("command_auto no longer searches");
    assert!(
        guard < search,
        "the existence check has to come before the search; otherwise a \
         mistyped path is answered with a result about somewhere else"
    );

    assert!(
        body.contains("named.display()"),
        "the refusal has to name the directory it was given, or the reader \
         cannot tell which path was wrong"
    );
}

/// No interface string carries a run of spaces left behind by its own source
/// indentation.
///
/// A multi-line string literal in Rust joins its lines only if each one ends in
/// a backslash. Drop the backslashes and the literal still compiles, still
/// passes every test that looks for a word in it, and renders in the window
/// with a twenty-space hole in the middle of a sentence, because the source
/// file's indentation is now part of the text. That is what happened to eleven
/// strings across the interface, and nothing failed.
///
/// The rule below is `rustfmt`'s own width. `rustfmt` reflows code but never
/// the inside of a string literal, so a source line past 100 characters that
/// holds a gap inside a literal is a literal that was joined by hand and not
/// put back together. A deliberate column of help text, meanwhile, is written
/// short and stays well inside the limit: every one in this repository fits in
/// 88 characters, so none of them trips this.
///
/// Two things are deliberately out of scope. Test modules are skipped, because
/// a test legitimately holds wide fixtures: `reg query` output is reproduced
/// space for space, and the tests that read this repository's own source carry
/// needles with the indentation they are searching for. And an escape is not a
/// word, so the `n` of a `\n` cannot be the letter that starts a gap.
#[test]
fn no_interface_string_has_a_gap_where_a_line_continuation_belongs() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut sources = Vec::new();
    let mut pending = vec![crates.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the crates live here") {
            let entry = entry.expect("a readable directory entry");
            let path = entry.path();
            if path.is_dir() {
                // `target` is build output, and can be enormous.
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }
    assert!(
        sources.len() > 50,
        "the walk found {} files, so it is not looking at the workspace",
        sources.len()
    );

    let mut found = Vec::new();
    for path in &sources {
        let text = std::fs::read_to_string(path).expect("a readable source file");
        let text = text.replace("\r\n", "\n");
        // Everything up to the test module: what a person is shown.
        let interface = text.split("\n#[cfg(test)]").next().unwrap();
        for (number, line) in interface.lines().enumerate() {
            if line.chars().count() <= 100 {
                continue;
            }
            // An escape is not a word. Without this the `n` of a `\n` reads as
            // the letter before a gap, and a needle that searches this
            // repository's own indented source looks like broken prose.
            let line = line.replace("\\n", "..").replace("\\t", "..");
            // The gap has to sit between two words to be prose rather than a
            // column: three spaces after a letter or a comma, and a letter
            // after them.
            let bytes: Vec<char> = line.chars().collect();
            let gap = bytes.windows(5).any(|w| {
                (w[0].is_ascii_alphabetic() || w[0] == ',' || w[0] == '.')
                    && w[1] == ' '
                    && w[2] == ' '
                    && w[3] == ' '
                    && (w[4] == ' ' || w[4].is_ascii_alphabetic())
            });
            if gap {
                found.push(format!(
                    "{}:{}",
                    path.file_name().unwrap().to_string_lossy(),
                    number + 1
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "these lines join a string literal without the backslash that would \
         close the gap, so the text renders with the source indentation in the \
         middle of it: {found:?}"
    );
}

/// Nothing a reader is meant to type still names a `veilvoice-verify` program.
///
/// The verifier was an executable of its own until 0.1.18 and is now part of
/// `veilvoice` and of the desktop application. What was left behind was not one
/// stale sentence but a scattering of them: three lists of the binaries a
/// release ships, a recorded terminal session on the front page whose prompt
/// showed a command that no longer runs, the front page's own "ships in every
/// archive" paragraph, a Windows icon check looking for a third executable, and
/// the help text this program prints when it cannot download.
///
/// Each was harmless on its own and the set of them told a reader to run
/// something that does not exist. So the rule is checked rather than
/// remembered: `veilvoice-verify` followed by a flag or a subcommand, or with a
/// path in front of it, is an instruction to run a program, and there is no
/// such program.
///
/// **What is deliberately not checked.** The crate is still called
/// `veilvoice-verify` and naming it is correct. So is describing what the
/// program used to do: `docs/AUDIT.md`, `CHANGELOG.md` and the release notes
/// generated from it are records of what happened, and rewriting a record to
/// match the present is how a project loses the ability to say when something
/// changed. Those files are named here with that reason rather than skipped
/// quietly.
///
/// # In plain words
///
/// Checks that no page tells you to run a program that was removed, while
/// leaving the history that mentions it alone.
/// The README's count of the window's tabs is the number of tabs there are.
///
/// It said "three modes" for as long as there had been nine, because a
/// sentence written when the window had three was never revisited. A count is
/// exactly the kind of fact this repository has a rule about: it appears in
/// prose, nothing derives it, and it is wrong the moment a tab is added.
///
/// Counting the variants of `Tab` rather than the strings a reader sees,
/// because the enum is what decides how many there are.
#[test]
fn the_readme_counts_the_window_tabs_the_window_actually_has() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root");

    let app = std::fs::read_to_string(root.join("crates/veilvoice-gui/src/app.rs"))
        .expect("the window's own source");
    let body = app
        .split("enum Tab {")
        .nth(1)
        .and_then(|rest| rest.split("\n}").next())
        .expect("`enum Tab` has to be findable");
    // A variant is a bare capitalised name on its own line, ending in a comma.
    // Doc comments and attributes are not variants.
    let tabs = body
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.ends_with(',')
                && !line.starts_with("//")
                && !line.starts_with('#')
                && line[..line.len() - 1]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric())
                && line.starts_with(|c: char| c.is_ascii_uppercase())
        })
        .count();
    assert!(tabs >= 2, "found {tabs} tabs, which cannot be right");

    let words = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ];
    let expected = words
        .get(tabs)
        .unwrap_or_else(|| panic!("no word for {tabs} tabs; add one"));

    let readme = std::fs::read_to_string(root.join("README.md")).expect("README.md");
    let claim = readme
        .split(" tabs:")
        .next()
        .and_then(|before| before.rsplit('\n').next())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .expect("README.md must say how many tabs the window has, as `N tabs:`");
    assert!(
        claim.to_ascii_lowercase().ends_with(*expected),
        "README.md says `{claim} tabs`, the window has {tabs} ({expected})"
    );
}

#[test]
fn no_page_tells_a_reader_to_run_a_program_that_no_longer_exists() {
    use std::path::{Path, PathBuf};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");

    // Records of the past, which are allowed to describe it.
    let history = [
        "AUDIT.md",
        "CHANGELOG.md",
        "releases.html",
        "search.html",
        "search-index.json",
    ];

    // What follows the name when it is being run rather than named.
    let invoked = [
        " --",
        " -q",
        " auto",
        " file",
        " deps",
        " gnupg",
        " hash",
        " reproduce",
        " help",
    ];

    fn gather(dir: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                gather(&path, into);
            } else if path.extension().is_some_and(|e| {
                ["md", "html", "txt", "py", "sh", "ps1", "bat", "rs"]
                    .contains(&e.to_string_lossy().as_ref())
            }) {
                into.push(path);
            }
        }
    }

    let mut files = Vec::new();
    for place in [
        "README.md",
        "docs",
        "website",
        "packaging",
        "tools",
        "assets/screenshots",
    ] {
        let path = root.join(place);
        if path.is_dir() {
            gather(&path, &mut files);
        } else if path.is_file() {
            files.push(path);
        }
    }
    assert!(files.len() > 50, "only {} files were read", files.len());

    let mut wrong = Vec::new();
    for path in &files {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if history.contains(&name.as_str()) {
            continue;
        }
        // This test's own explanation names the thing it forbids.
        if path.ends_with("tests.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (number, line) in text.replace("\r\n", "\n").lines().enumerate() {
            let Some(at) = line.find("veilvoice-verify") else {
                continue;
            };
            let rest = &line[at + "veilvoice-verify".len()..];
            // A path in front of it only counts when the name ends there.
            // With another segment after it the line is naming a source file
            // inside the crate; with nothing after it, a leading `./` or an
            // absolute path makes it a command somebody is told to run.
            let ends_here =
                rest.is_empty() || rest.starts_with([' ', '`', '"', '\'', ',', '.', ')']);
            let run = invoked.iter().any(|form| rest.starts_with(form))
                || (line[..at].ends_with('/') && ends_here && !rest.starts_with('.'));
            if run {
                wrong.push(format!("{}:{}: {}", name, number + 1, line.trim()));
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "these tell a reader to run `veilvoice-verify`, which has not been a \
         program since 0.1.18. It is `veilvoice verify`, and the desktop \
         application's Verify tab:\n{}",
        wrong.join("\n")
    );
}
