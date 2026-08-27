// SPDX-License-Identifier: GPL-3.0-or-later
//! Authenticated encryption with XChaCha20-Poly1305.
//!
//! XChaCha20 rather than plain ChaCha20 because its 192-bit nonce can be drawn
//! at random with no practical collision risk. The 96-bit nonce of RFC 8439
//! ChaCha20-Poly1305 requires a counter and careful state tracking to stay
//! unique across runs; getting that wrong is catastrophic, and a random
//! 192-bit nonce removes the failure mode entirely.
//!
//! Every call is authenticated over associated data as well as the plaintext,
//! which is how the container header in [`crate::container`] is bound to its
//! ciphertext: flipping a bit in the stored KDF parameters produces a
//! decryption failure rather than a silently different key.
//!
//! # In plain words
//!
//! This is the encryption itself: it turns a recording into something unreadable,
//! and it can tell whether the result was tampered with afterwards.
//!
//! Those two jobs go together on purpose. Encryption on its own hides what a file
//! says but does not stop somebody changing it, and a changed file that still
//! decrypts into something is a worse outcome than one that refuses to open. Here,
//! any alteration at all means it will not open, and says so.

use crate::{Error, Secret};
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};

/// Nonce length for XChaCha20-Poly1305, in bytes.
pub const NONCE_LEN: usize = 24;
/// Poly1305 authentication tag length, in bytes.
pub const TAG_LEN: usize = 16;

/// Draw a fresh random nonce from the OS CSPRNG.
pub fn random_nonce() -> Result<[u8; NONCE_LEN], Error> {
    let mut n = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut n).map_err(|_| Error::Random)?;
    Ok(n)
}

fn cipher(key: &Secret) -> Result<XChaCha20Poly1305, Error> {
    if key.len() != 32 {
        return Err(Error::KeyLength);
    }
    Ok(XChaCha20Poly1305::new(key.expose().into()))
}

/// Encrypt `plaintext`, authenticating `aad` alongside it.
///
/// Returns ciphertext with the 16-byte tag appended.
pub fn seal(
    key: &Secret,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, Error> {
    cipher(key)?
        .encrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| Error::Encrypt)
}

/// Decrypt and verify. Any tampering with the ciphertext, the tag, the nonce or
/// `aad` fails here rather than returning wrong plaintext.
pub fn open(
    key: &Secret,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, Error> {
    cipher(key)?
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| Error::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Secret {
        let mut k = [42u8; 32];
        Secret::new(&mut k)
    }

    #[test]
    fn round_trip_recovers_the_plaintext() {
        let (k, n) = (key(), random_nonce().unwrap());
        let msg = b"the quick brown fox";
        let ct = seal(&k, &n, b"header", msg).unwrap();
        assert_ne!(&ct[..], &msg[..], "ciphertext must not be the plaintext");
        assert_eq!(ct.len(), msg.len() + TAG_LEN);
        assert_eq!(open(&k, &n, b"header", &ct).unwrap(), msg);
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let (k, n) = (key(), random_nonce().unwrap());
        let ct = seal(&k, &n, b"", b"").unwrap();
        assert_eq!(open(&k, &n, b"", &ct).unwrap(), b"");
    }

    #[test]
    fn tampering_with_the_ciphertext_is_detected() {
        let (k, n) = (key(), random_nonce().unwrap());
        let mut ct = seal(&k, &n, b"h", b"secret payload").unwrap();
        ct[3] ^= 1;
        assert!(matches!(open(&k, &n, b"h", &ct), Err(Error::Decrypt)));
    }

    #[test]
    fn tampering_with_the_tag_is_detected() {
        let (k, n) = (key(), random_nonce().unwrap());
        let mut ct = seal(&k, &n, b"h", b"secret payload").unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 1;
        assert!(matches!(open(&k, &n, b"h", &ct), Err(Error::Decrypt)));
    }

    /// The property the container format depends on: header bytes are bound to
    /// the ciphertext, so editing them cannot go unnoticed.
    #[test]
    fn changing_the_associated_data_is_detected() {
        let (k, n) = (key(), random_nonce().unwrap());
        let ct = seal(&k, &n, b"header-v1", b"payload").unwrap();
        assert!(matches!(
            open(&k, &n, b"header-v2", &ct),
            Err(Error::Decrypt)
        ));
    }

    #[test]
    fn wrong_key_or_nonce_fails() {
        let (k, n) = (key(), random_nonce().unwrap());
        let ct = seal(&k, &n, b"", b"payload").unwrap();

        let mut other = [1u8; 32];
        assert!(open(&Secret::new(&mut other), &n, b"", &ct).is_err());

        let mut n2 = n;
        n2[0] ^= 1;
        assert!(open(&k, &n2, b"", &ct).is_err());
    }

    #[test]
    fn nonces_do_not_repeat() {
        let a = random_nonce().unwrap();
        let b = random_nonce().unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn wrong_key_length_is_rejected() {
        let k = Secret::zeroed(16);
        assert!(matches!(
            seal(&k, &[0u8; NONCE_LEN], b"", b""),
            Err(Error::KeyLength)
        ));
    }

    #[test]
    fn same_plaintext_encrypts_differently_each_time() {
        let k = key();
        let a = seal(&k, &random_nonce().unwrap(), b"", b"same").unwrap();
        let b = seal(&k, &random_nonce().unwrap(), b"", b"same").unwrap();
        assert_ne!(a, b, "random nonces must decorrelate identical plaintexts");
    }
}
