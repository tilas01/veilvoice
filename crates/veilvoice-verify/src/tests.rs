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
            //
            // **Replaced with `~`, not with `.`.** A full stop is in the set
            // that opens a gap below, so a `\n` followed by the indentation of
            // an embedded script read as a sentence with a hole in it. Every
            // line of the player's inline JavaScript has that shape, and the
            // only reason this did not fire years ago is that those lines were
            // under the length threshold. `~` is in no set here and cannot
            // start or end a gap.
            let line = line.replace("\\n", "~~").replace("\\t", "~~");
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
/// Every command line drawing that gets made is shown somewhere.
///
/// `tools/shots/terminal.py` draws one picture per help screen in its
/// `COMMANDS` list. The README lists them by hand, so a screen added to that
/// list produced a drawing nobody ever saw: `cli-fix.svg` was generated,
/// committed, checked against the program's own output, and referenced from no
/// page at all.
///
/// The same shape as the tabs, one file along. A generated set and a
/// hand-written list of it drift the moment the set grows.
#[test]
fn every_command_line_drawing_is_shown_in_the_readme() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root");

    let tool = std::fs::read_to_string(root.join("tools/shots/terminal.py"))
        .expect("the tool that draws them");
    let names: Vec<String> = tool
        .split("COMMANDS = [")
        .nth(1)
        .and_then(|rest| rest.split("\n]").next())
        .expect("`COMMANDS` has to be findable")
        .lines()
        .filter_map(|line| {
            let (_, rest) = line.trim().split_once("(\"")?;
            let (name, _) = rest.split_once('"')?;
            Some(name.to_string())
        })
        .collect();
    assert!(!names.is_empty(), "no drawing names were found to check");

    let readme = std::fs::read_to_string(root.join("README.md")).expect("the README");
    let missing: Vec<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|name| !readme.contains(&format!("cli-{name}.svg")))
        .collect();
    assert!(
        missing.is_empty(),
        "README.md shows no drawing of these command line screens: {}. Every \
         one that `tools/shots/terminal.py` draws is committed, so one nothing \
         references is a picture nobody will ever see.",
        missing.join(", ")
    );
}

/// Every tab the window shows has a picture in the README and on the website.
///
/// The count was checked and the *list* was not, so both carried a
/// hand-written table of nine tabs and went on carrying it after there were
/// eleven. The Studio and the Browser shipped in v0.1.20 and appeared in
/// neither, which is the whole "what it looks like" section quietly describing
/// a different application from the one released.
///
/// The keys come from `Tab::key` in the window's own source, so a tab added
/// tomorrow fails this rather than being noticed by a reader.
#[test]
fn every_tab_has_a_picture_in_the_readme_and_on_the_website() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root");

    let app = std::fs::read_to_string(root.join("crates/veilvoice-gui/src/app.rs"))
        .expect("the window's own source");
    let keys: Vec<String> = app
        .split("pub fn key(self) -> &'static str {")
        .nth(1)
        .and_then(|rest| rest.split("\n    }").next())
        .expect("`Tab::key` has to be findable")
        .lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("=> \"")?;
            let (key, _) = rest.split_once('"')?;
            Some(key.to_string())
        })
        .collect();
    assert!(!keys.is_empty(), "no tab keys were found to check");

    for (what, path) in [
        ("README.md", root.join("README.md")),
        ("website/index.html", root.join("website/index.html")),
    ] {
        let text = std::fs::read_to_string(&path).expect("a readable page");
        let missing: Vec<&str> = keys
            .iter()
            .map(String::as_str)
            .filter(|key| !text.contains(&format!("gui-{key}.png")))
            .collect();
        assert!(
            missing.is_empty(),
            "{what} shows no picture of these tabs: {}. Every tab the window \
             has needs one, or the section describes a different application \
             from the one that ships.",
            missing.join(", ")
        );
    }
}

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

/// **No test in the desktop crate may open a device, a dialog or a window.**
///
/// # Two access violations, a day apart, from the same mistake
///
/// **F-163.** A Studio test played a recording and then locked the window, to
/// assert that locking releases what is playing. Its comment said in as many
/// words: no audio device in a test runner, so `play` will not start a stream.
/// The Windows runner has one. A stream started, tearing it down took the whole
/// test binary with it, and every test in the crate had already passed.
///
/// **F-165.** A setup-card test asked the machine how many audio devices it
/// has, twice, to check the answer was stable. The crate's test binary already
/// enumerates once, deliberately, and a second enumerator beside it killed the
/// process the same way.
///
/// Both were fixed one at a time. This is the guard, so the third is caught
/// here rather than on a build machine somebody has to go and read.
///
/// # Why this crate and not every crate
///
/// The desktop crate's test binary is the one that links cpal, `rfd`, egui and
/// winit together, and it is where both crashes happened. A narrower guard that
/// is exactly right is worth more than a wide one that has to be argued with;
/// widen it the day another binary does the same thing.
///
/// # What a test may do instead
///
/// Read the source of the function it is about, which is how
/// `locking_the_window_stops_a_take_that_is_playing` asserts the one line in
/// `close` that matters, and how the setup card is checked to be measuring the
/// machine rather than carrying a number. A test whose correctness depends on
/// the machine it runs on is not testing the thing it names.
#[test]
fn no_desktop_test_opens_a_device_a_dialog_or_a_window() {
    let gui = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory")
        .join("veilvoice-gui")
        .join("src");

    let mut sources = Vec::new();
    let mut pending = vec![gui.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the desktop crate's source") {
            let entry = entry.expect("a readable directory entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }
    assert!(
        sources.len() > 20,
        "the walk found {} files, so it is not looking at the desktop crate",
        sources.len()
    );

    /// What reaches the platform. Each of these opens something the machine
    /// owns: a sound card, a file panel, or a stream on either.
    const REACHES_THE_PLATFORM: &[&str] = &[
        "devices::list(",
        "devices::open(",
        "playback::start(",
        "LiveSession::start(",
        "LiveSession::start_recording(",
        "rfd::FileDialog",
    ];

    /// The one deliberate exception, and it is one call in one test.
    ///
    /// Enumerating once, on purpose, is how the window's device pickers are
    /// known to survive a machine with no sound card. A second enumerator is
    /// what F-165 was, so the exception is the test's name rather than the
    /// call: another test may not borrow it.
    const ALLOWED: &str = "building_the_app_with_real_device_enumeration_does_not_panic";

    let mut offenders = Vec::new();
    for path in &sources {
        let name = path
            .file_name()
            .expect("a file")
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(path)
            .expect("a readable source file")
            .replace("\r\n", "\n");

        // A file under `src/<module>/tests.rs` is test code whole; anything
        // else is test code only after its `#[cfg(test)]`.
        let is_test_file = name == "tests.rs";
        let body = if is_test_file {
            text.as_str()
        } else {
            match text.split_once("\n#[cfg(test)]") {
                Some((_, tests)) => tests,
                None => continue,
            }
        };

        // Which test each line belongs to, so the exception can be by name.
        let mut current = String::new();
        for (number, line) in body.lines().enumerate() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                current = rest.split('(').next().unwrap_or("").to_string();
            }
            if trimmed.starts_with("//") {
                continue;
            }
            if current == ALLOWED {
                continue;
            }
            for reaching in REACHES_THE_PLATFORM {
                let Some(at) = line.find(reaching) else {
                    continue;
                };
                // A needle is not a call. `dialog.rs`'s own guard searches the
                // source for `rfd::FileDialog`, and the string it searches for
                // is not an opened dialog. An odd number of quotes before the
                // match means it is inside one.
                if line[..at].matches('"').count() % 2 == 1 {
                    continue;
                }
                offenders.push(format!(
                    "{name}:{}: {} (in {current})",
                    number + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these tests open something the machine owns, which is how F-163 and \
         F-165 killed the desktop test binary on Windows after every test had \
         passed. Assert the thing the test names instead, by reading the source \
         of the function it is about:\n{}",
        offenders.join("\n")
    );
}

/// **F-166.** Nothing outside the session builds a recorder.
///
/// The rate a recorder is built with is written into the WAV header, and it has
/// to be the rate the device agreed to. Only `LiveSession::start_recording`
/// knows that, because it is the function that asks the device. Both front ends
/// used to build their own from `config.sample_rate`, which is the rate that
/// was asked for, and on any machine not running at 48 kHz the take came out
/// fast and sharp.
///
/// The signature no longer carries a rate, so a third caller cannot repeat it
/// by passing the wrong one. This is the other half: a third caller cannot
/// repeat it by going around the session either.
#[test]
fn only_the_session_builds_a_recorder() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory")
        .to_path_buf();

    /// Where the rate is known, and therefore the one place this may appear.
    const HOME: &str = "veilvoice-audio";

    let mut sources = Vec::new();
    let mut pending = vec![crates.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("a crate directory") {
            let entry = entry.expect("a readable directory entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }
    assert!(
        sources.len() > 100,
        "the walk found {} files, so it is not looking at the workspace",
        sources.len()
    );

    let mut offenders = Vec::new();
    for path in &sources {
        if path.components().any(|c| c.as_os_str() == HOME) {
            continue;
        }
        let text = std::fs::read_to_string(path)
            .expect("a readable source file")
            .replace("\r\n", "\n");
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            let Some(at) = line.find("record::start(") else {
                continue;
            };
            // A needle is not a call: this guard names the string it looks for.
            if line[..at].matches('"').count() % 2 == 1 {
                continue;
            }
            offenders.push(format!(
                "{}:{}: {}",
                path.display(),
                number + 1,
                line.trim()
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "these build a recorder outside `{HOME}`, so they choose the rate its \
         WAV header is written with, and the only correct answer is the one the \
         device gave `LiveSession::start_recording`. Ask that function for the \
         recorders instead, with a `Keeping`:\n{}",
        offenders.join("\n")
    );
}

/// **Marker 130.** One place in the window starts a live session.
///
/// There were two: the live tab and the Studio, each with a session of its own,
/// so veiling on one and recording on the other opened the same microphone
/// twice. Live scramble is the Studio now, and `start_session` is the only
/// starter, which is most of what moving it was worth.
#[test]
fn the_desktop_starts_a_live_session_in_exactly_one_place() {
    let gui = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory")
        .join("veilvoice-gui")
        .join("src");

    let mut sources = Vec::new();
    let mut pending = vec![gui.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the desktop crate's source") {
            let entry = entry.expect("a readable directory entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }

    let mut starters = Vec::new();
    for path in &sources {
        let name = path
            .file_name()
            .expect("a file")
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(path)
            .expect("a readable source file")
            .replace("\r\n", "\n");
        // Which function each line is in, so the message names it.
        let mut current = String::new();
        for (number, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                current = rest.split('(').next().unwrap_or("").to_string();
            }
            if trimmed.starts_with("//") {
                continue;
            }
            for needle in ["LiveSession::start(", "LiveSession::start_recording("] {
                let Some(at) = line.find(needle) else {
                    continue;
                };
                if line[..at].matches('"').count() % 2 == 1 {
                    continue;
                }
                starters.push(format!("{name}:{}: in {current}", number + 1));
            }
        }
    }

    assert_eq!(
        starters.len(),
        1,
        "a live session is started in {} places in the desktop crate. Two \
         starters is two opens of the same microphone, which is what having a \
         live tab beside the Studio was:\n{}",
        starters.len(),
        starters.join("\n")
    );
    assert!(
        starters[0].starts_with("studio.rs:"),
        "the one starter should be the Studio's, and it is {}",
        starters[0]
    );
}

/// **Marker 126.** Nothing in an audio callback allocates, locks or prints.
///
/// A callback runs on the operating system's audio thread with a deadline
/// measured in milliseconds. Allocating in one takes a global lock in the
/// allocator, blocking on a mutex hands the thread to whoever holds it, and
/// printing takes the lock on standard output. Each of those is somebody
/// else's schedule deciding when this thread runs again, and missing the
/// deadline is an audible click in the veiled voice, or a dropped block in a
/// recording.
///
/// Every buffer these callbacks use is sized once, before the stream starts.
/// That is a fact about how they are written, and until now it was a fact
/// nothing checked: the comments say "sized once, here, so the callback never
/// allocates", and a comment is not a guard. This reads the callbacks
/// themselves.
///
/// Written rather than measured, deliberately. A test that counted
/// allocations would need a global allocator hook and a running stream, which
/// means a machine with a sound card, which is what F-163 and F-165 were about.
/// Reading the source finds the same mistake on a build machine with no audio
/// at all.
#[test]
fn no_audio_callback_allocates_or_blocks() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory")
        .to_path_buf();

    /// Where a realtime callback is handed to the platform. The closure that
    /// follows one of these is the body with the deadline on it.
    const OPENS_A_STREAM: &[&str] = &["build_input_stream(", "build_output_stream("];

    /// What may not appear inside one, and what each would cost.
    ///
    /// `try_lock` and `try_push` are the non-blocking forms and are what this
    /// code already uses, so a needle that is a prefix of one is matched on the
    /// call rather than on the name: see `reaches` below.
    const FORBIDDEN: &[(&str, &str)] = &[
        ("vec![", "allocates"),
        ("Vec::", "allocates"),
        (".to_vec()", "allocates"),
        (".to_owned()", "allocates"),
        (".to_string()", "allocates"),
        ("String::", "allocates"),
        ("format!", "allocates"),
        (".collect()", "allocates"),
        ("Box::new", "allocates"),
        (".clone()", "may allocate"),
        (".push(", "may reallocate; the ring's `try_push` does not"),
        (".insert(", "may reallocate"),
        (".extend(", "may reallocate"),
        (".resize(", "may reallocate"),
        (".reserve(", "allocates"),
        (".lock()", "blocks; `try_lock` is what this code uses"),
        ("println!", "takes the lock on standard output"),
        ("eprintln!", "takes the lock on standard error"),
        ("print!", "takes the lock on standard output"),
    ];

    let mut sources = Vec::new();
    let mut pending = vec![crates.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("a crate directory") {
            let entry = entry.expect("a readable directory entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }

    let mut callbacks = 0;
    let mut offenders = Vec::new();
    for path in &sources {
        let name = path
            .file_name()
            .expect("a file")
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(path)
            .expect("a readable source file")
            .replace("\r\n", "\n");

        for opener in OPENS_A_STREAM {
            let mut from = 0;
            while let Some(at) = text[from..].find(opener) {
                let at = from + at;
                from = at + opener.len();
                // A needle is not a call: this guard names what it looks for.
                let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
                if text[line_start..at].matches('"').count() % 2 == 1 {
                    continue;
                }

                // The data callback is the first closure after the opener, and
                // its body is from its `{` to the matching `}`.
                let Some(brace) = text[at..].find("| {").map(|n| at + n + 2) else {
                    continue;
                };
                let mut depth = 0usize;
                let mut end = brace;
                for (offset, byte) in text[brace..].char_indices() {
                    match byte {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = brace + offset;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                callbacks += 1;

                let before = text[..brace].matches('\n').count();
                for (number, line) in text[brace..end].lines().enumerate() {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//") {
                        continue;
                    }
                    for (needle, cost) in FORBIDDEN {
                        let Some(hit) = line.find(needle) else {
                            continue;
                        };
                        // `try_lock()` and `try_push(` end in the needle and are
                        // the non-blocking forms. Only the bare call counts.
                        if line[..hit].ends_with("try_") {
                            continue;
                        }
                        if line[..hit].matches('"').count() % 2 == 1 {
                            continue;
                        }
                        offenders.push(format!(
                            "{name}:{}: {} ({needle} {cost})",
                            before + number + 1,
                            line.trim()
                        ));
                    }
                }
            }
        }
    }

    assert!(
        callbacks >= 3,
        "the walk found {callbacks} audio callbacks, and there are at least \
         three: the live path's input and output, and playback's. A guard that \
         finds none passes for the wrong reason"
    );
    assert!(
        offenders.is_empty(),
        "these lines run on an audio thread with a deadline in milliseconds, \
         and each of them can miss it. Do the work before the stream starts, \
         into a buffer sized once:\n{}",
        offenders.join("\n")
    );
}
