// SPDX-License-Identifier: GPL-3.0-or-later
//! The release contents list parser, coverage-guided.
//!
//! **Marker 97.** `CONTENTS.sha256` lists every file inside every release
//! archive with its SHA-256, and a verifier reads it to decide which paths on
//! disk to open and what to compare them against. It is covered by the signed
//! `SHA256SUMS`, and every caller is told to check that before parsing.
//!
//! "Every caller is told to" is not a property of the code. A caller can get
//! the order wrong, a future front end can be written by somebody who did not
//! read the note, and the consequence would be a file of somebody else's
//! choosing deciding which paths a verifier opens. So the parser is fuzzed as
//! though nothing had checked it, which is the only assumption that stays true.
//!
//! What this is looking for, specifically: a path that escapes the release
//! directory, a panic on a line that is not what the release job writes, and a
//! digest that is accepted without being one.

#![no_main]

use libfuzzer_sys::fuzz_target;
use veilvoice_check::contents;

fuzz_target!(|data: &[u8]| {
    // The caller reads this file as text, so anything that is not UTF-8 never
    // reaches the parser.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(all) = contents::parse(text) else {
        return;
    };

    for archive in &all {
        // An archive name is used to look a section up and is never joined onto
        // a path, but an empty one would match a caller's empty string and
        // hand back the wrong section.
        assert!(
            !archive.archive.is_empty(),
            "a section with no archive name was accepted"
        );
        assert_eq!(
            contents::for_archive(&all, &archive.archive).is_some(),
            true,
            "a section that parsed cannot be found again"
        );

        for member in &archive.members {
            let path = &member.path;
            // The property the whole parser exists for: these strings are
            // joined onto a directory and opened.
            assert!(!path.is_empty(), "an empty path was accepted");
            assert!(!path.starts_with('/'), "an absolute path was accepted: {path:?}");
            assert!(!path.contains('\\'), "a backslash was accepted: {path:?}");
            assert!(
                !path.split('/').any(|part| part == ".." || part == "."),
                "a path that leaves the release was accepted: {path:?}"
            );
            assert!(
                path.as_bytes().get(1) != Some(&b':'),
                "a drive letter was accepted: {path:?}"
            );

            // And the digest is a digest, because a caller compares it against
            // one it computed and a shorter string would compare unequal
            // forever rather than loudly.
            assert_eq!(member.digest.len(), 64, "a short digest was accepted");
            assert!(
                member.digest.bytes().all(|b| b.is_ascii_hexdigit()),
                "a digest with a non-hex character was accepted: {:?}",
                member.digest
            );
        }

        // `roots` is what an extras walk starts from, so an empty component
        // there would walk the directory holding the archive rather than the
        // release inside it.
        for root in archive.roots() {
            assert!(!root.is_empty(), "an empty root directory was produced");
        }
    }

    // `check` and `extras` are deliberately not called: both read the
    // filesystem, which is slow and nondeterministic and would make a crash
    // unreproducible. Every path they would open has been asserted above.
});
