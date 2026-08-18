// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! The `.veil` container header, coverage-guided.
//!
//! This is the parser that reads a file somebody sent you. The properties are
//! the same three the deterministic campaign in
//! `crates/veilvoice-crypto/tests/parser_fuzz.rs` asserts -- never panic, never
//! hang, never claim success for something it did not fully understand -- but
//! explored by feedback rather than by construction.

#![no_main]

use libfuzzer_sys::fuzz_target;
use veilvoice_crypto::container;

fuzz_target!(|data: &[u8]| {
    let Ok((header, body)) = container::Header::parse(data) else {
        // A refusal is a correct outcome for almost every input. What must
        // never happen is a panic, and reaching this line at all proves there
        // was not one.
        return;
    };

    // An `Ok` is a claim about the buffer, so check the claim.
    assert!(
        body <= data.len(),
        "parse reported a ciphertext offset of {body} in a {} byte buffer",
        data.len()
    );

    // Re-serialising must reproduce exactly the bytes it came from. This is the
    // property that catches a field silently normalised on the way through --
    // which would mean the AEAD's associated data is not the header actually
    // stored, and the whole downgrade defence rests on those being the same
    // bytes.
    assert_eq!(
        header.to_bytes(),
        &data[..body],
        "header did not round-trip to the bytes it was parsed from"
    );

    // Opening with a wrong password must fail rather than panic, whatever the
    // header declares. This is the path that reaches Argon2 with
    // attacker-controlled cost parameters -- where F-2 and F-3 lived.
    let _ = container::open_with_password(b"not the password", data);

    // And the unattended ceiling must refuse rather than panic, too.
    let _ = container::open_with_password_within(
        b"not the password",
        data,
        veilvoice_crypto::kdf::KdfParams::UNATTENDED_MAX_M_COST,
    );
});
