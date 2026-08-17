// SPDX-License-Identifier: GPL-3.0-or-later
//! The app-lock file, coverage-guided.
//!
//! Worse than the container in one specific way: this file is parsed **before
//! anyone has authenticated anything**. It is the first bytes the program
//! touches on a locked machine, so anything that can write it gets a free shot
//! at the parser -- and at the Argon2 cost parameters it carries, which is
//! exactly where F-2 and F-3 were.

#![no_main]

use libfuzzer_sys::fuzz_target;
use veilvoice_crypto::lock::AppLock;

fuzz_target!(|data: &[u8]| {
    let Ok(mut lock) = AppLock::parse(data) else {
        return;
    };

    // Round-trip: what was parsed must serialise back to the same bytes, or the
    // in-memory state is not the state on disk.
    assert_eq!(
        lock.to_bytes(),
        data,
        "lock file did not round-trip to the bytes it was parsed from"
    );

    // A verification against a parsed lock must not panic, whatever the file
    // declared -- this is the call that hands the file's own cost parameters to
    // Argon2.
    let _ = lock.verify(b"not the password");

    // Nor may reading back the rate-limit state, which is derived from a
    // signed timestamp and an attempt count that both came out of the file.
    let _ = lock.cooldown();
    let _ = lock.failures();
});
