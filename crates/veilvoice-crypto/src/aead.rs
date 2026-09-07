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
use chacha20poly1305::aead::{Aead, AeadInPlace, Payload};
use chacha20poly1305::{KeyInit, Tag, XChaCha20Poly1305, XNonce};

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

/// Decrypt and verify **into protected memory**, never into an ordinary `Vec`.
///
/// # Why this exists beside `open`
///
/// `open` asks the `aead` crate for the plaintext and gets back a plain
/// `Vec<u8>`. That vector is ordinary pageable heap: the kernel may write it to
/// swap, and nothing wipes it until somebody copies it somewhere safer and
/// wipes it by hand. For a passphrase-sized secret that window is small. For a
/// recording it is the whole recording, in the clear, in memory the operating
/// system is free to put on disk, for as long as it takes to copy several
/// megabytes.
///
/// The Studio vault exists precisely so that a recording is never anywhere
/// unprotected, and it was decrypting through `open`, so the guarantee had a
/// hole in it the size of the file. This closes it: the buffer is a [`Secret`]
/// from the start, so it is page-locked where the operating system allows and
/// wiped on drop, and the ciphertext is decrypted **in place** inside it.
/// Nothing is copied afterwards because there is nothing to copy from.
///
/// A failed tag check drops the buffer, so the partially decrypted bytes are
/// wiped rather than returned or left lying about.
///
/// `open` is kept for the callers whose plaintext is small and immediately
/// parsed, where a `Vec` is the honest shape and the protection this adds would
/// be a `Secret` around four bytes of header.
///
/// # In plain words
///
/// The same decryption, writing the result straight into the protected memory
/// rather than into ordinary memory and moving it afterwards.
pub fn open_secret(
    key: &Secret,
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Secret, Error> {
    let split = ciphertext
        .len()
        .checked_sub(TAG_LEN)
        .ok_or(Error::Decrypt)?;
    let mut plain = Secret::zeroed(split);
    plain.expose_mut().copy_from_slice(&ciphertext[..split]);
    let tag = Tag::from_slice(&ciphertext[split..]);
    cipher(key)?
        .decrypt_in_place_detached(XNonce::from_slice(nonce), aad, plain.expose_mut(), tag)
        .map_err(|_| Error::Decrypt)?;
    Ok(plain)
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
    fn decrypting_into_protected_memory_gives_the_same_plaintext() {
        let (k, n) = (key(), random_nonce().unwrap());
        let msg = b"the quick brown fox";
        let ct = seal(&k, &n, b"header", msg).unwrap();
        let plain = open_secret(&k, &n, b"header", &ct).unwrap();
        assert_eq!(plain.expose(), msg);
        // And the same answer as the `Vec` route, so the two cannot diverge.
        assert_eq!(plain.expose(), &open(&k, &n, b"header", &ct).unwrap()[..]);
    }

    #[test]
    fn protected_decryption_refuses_the_same_things_the_other_one_does() {
        let (k, n) = (key(), random_nonce().unwrap());
        let mut ct = seal(&k, &n, b"h", b"secret payload").unwrap();

        // Too short to hold a tag at all: refused rather than subtracting past
        // zero. `checked_sub` is what stops that being a panic on the length.
        assert!(matches!(
            open_secret(&k, &n, b"h", &[]),
            Err(Error::Decrypt)
        ));
        assert!(matches!(
            open_secret(&k, &n, b"h", &ct[..TAG_LEN - 1]),
            Err(Error::Decrypt)
        ));

        // Wrong associated data, and a flipped ciphertext bit.
        assert!(matches!(
            open_secret(&k, &n, b"other", &ct),
            Err(Error::Decrypt)
        ));
        ct[3] ^= 1;
        assert!(matches!(
            open_secret(&k, &n, b"h", &ct),
            Err(Error::Decrypt)
        ));
    }

    #[test]
    fn an_empty_plaintext_round_trips_through_protected_memory() {
        // The tag alone, with nothing under it: the length arithmetic has to
        // land on zero rather than on an error or a panic.
        let (k, n) = (key(), random_nonce().unwrap());
        let ct = seal(&k, &n, b"", b"").unwrap();
        assert_eq!(ct.len(), TAG_LEN);
        assert!(open_secret(&k, &n, b"", &ct).unwrap().expose().is_empty());
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
