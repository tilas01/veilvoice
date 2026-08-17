// SPDX-License-Identifier: GPL-3.0-or-later
//! The integrity manifest parser, coverage-guided.
//!
//! A text format rather than a packed one, which removes a whole class of
//! length-field bug and introduces a different one: it is split on a delimiter,
//! indexed by byte offset, and sliced for display. `Change::describe` takes
//! `&digest[..8]`, and slicing a `String` at a byte offset that is not a
//! character boundary panics.
//!
//! The manifest is normally the user's own record, but "normally" is not a
//! security property: `veilvoice guard check` will read whichever file is at
//! the path, and a record is exactly the kind of thing somebody hands you.

#![no_main]

use libfuzzer_sys::fuzz_target;
use veilvoice_guard::Manifest;

fuzz_target!(|data: &[u8]| {
    // Not every byte string is UTF-8, and the loader would refuse those before
    // the parser ever ran.
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(manifest) = Manifest::parse(text) else {
        return;
    };

    // Serialising and re-parsing must be a fixed point. If it is not, two
    // manifests of the same files are not comparable, which is the only thing
    // this type does.
    let written = manifest.to_text();
    let reparsed = Manifest::parse(&written).expect("a manifest we wrote must parse");
    assert_eq!(reparsed, manifest, "the text format is not a fixed point");
    assert_eq!(reparsed.to_text(), written, "serialisation is not stable");

    // Every recorded path must survive the round trip intact, and the digest
    // slicing in `describe` must not panic on any of them.
    for path in manifest.paths() {
        assert!(!path.is_empty(), "an empty path was recorded");
        assert!(
            !path.contains('\n') && !path.contains('\r'),
            "a path with a line break would forge a record: {path:?}"
        );
    }

    // `check` is deliberately not called: it reads the filesystem, which is
    // both slow and nondeterministic, and would make a crash unreproducible.
    // The display path is exercised directly instead.
    for change in manifest.check::<&std::path::Path>(&[]).changes.iter().take(8) {
        let _ = change.describe();
    }
});
