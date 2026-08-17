// SPDX-License-Identifier: GPL-3.0-or-later
//! Key and encapsulation decoding, coverage-guided.
//!
//! `PublicKey::from_bytes` reads a `.pub` file, which is a file somebody sent
//! you by definition -- the whole point of a public key is that it arrived from
//! elsewhere. Behind it sit `ml-kem` and `x25519-dalek`, so this target is as
//! much about those dependencies handling malformed encodings as about the
//! wrapper around them.

#![no_main]

use libfuzzer_sys::fuzz_target;
use veilvoice_crypto::hybrid::{Encapsulation, PublicKey, SecretKey};

fuzz_target!(|data: &[u8]| {
    // A public key that parses must re-encode to exactly what it came from,
    // and must be usable without panicking -- encapsulating to a hostile key is
    // the operation `veilvoice anonymise --encrypt-to` performs.
    if let Ok(pk) = PublicKey::from_bytes(data) {
        assert_eq!(pk.to_bytes(), data, "public key did not round-trip");
        let _ = pk.encapsulate();
    }

    // An encapsulation arrives inside a container header, so it is attacker
    // controlled on the decryption path.
    if let Ok(enc) = Encapsulation::from_bytes(data) {
        assert_eq!(enc.to_bytes(), data, "encapsulation did not round-trip");
    }

    // A private key file is decrypted before being decoded, so this is only
    // reachable by someone who already had the passphrase -- but a malformed
    // decoding must still be an error rather than a panic.
    if let Ok(sk) = SecretKey::from_bytes(data) {
        let _ = sk.public_key();
    }
});
