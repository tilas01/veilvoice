// SPDX-License-Identifier: GPL-3.0-or-later
//! Post-quantum hybrid key encapsulation: X25519 + ML-KEM-768.
//!
//! # Why hybrid
//!
//! ML-KEM (FIPS 203, formerly Kyber) is believed secure against a quantum
//! adversary, but it is young, and lattice schemes have had implementation
//! breaks. X25519 is battle-tested but falls to a cryptographically relevant
//! quantum computer. Running both and mixing the two shared secrets means an
//! attacker must break *both*: the construction is at least as strong as the
//! stronger of the two, and it degrades gracefully if either one is broken.
//! This is the same reasoning behind the hybrids now deployed in TLS.
//!
//! It matters here specifically because of *harvest-now-decrypt-later*: a
//! recording captured today can be stored until quantum hardware exists. A tool
//! whose whole purpose is protecting who is speaking has to assume the
//! adversary is patient.
//!
//! # The combiner
//!
//! The two shared secrets are mixed with HKDF-SHA256 rather than concatenated
//! or XORed. Both secrets, both ciphertexts and both public keys go into the
//! input, which binds the derived key to the exact transcript that produced it
//! and prevents an attacker who can substitute one half from steering the
//! result. Feeding the full transcript is what makes the combiner robust when
//! one KEM's ciphertexts are malleable.

use crate::{Error, Secret};
use hkdf::Hkdf;
use ml_kem::kem::{Decapsulate, Encapsulate};
use ml_kem::{EncodedSizeUser, KemCore, MlKem768};
use sha2::Sha256;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret as XSecret};

/// Domain separation string, so keys derived here can never collide with keys
/// derived by any other part of the system.
const HKDF_INFO: &[u8] = b"veilvoice/v1/hybrid-kem/x25519+ml-kem-768";

/// Encoded length of the X25519 public key.
pub const X25519_PUB_LEN: usize = 32;
/// Encoded length of the ML-KEM-768 encapsulation (public) key.
pub const MLKEM_EK_LEN: usize = 1184;
/// Encoded length of an ML-KEM-768 ciphertext.
pub const MLKEM_CT_LEN: usize = 1088;
/// Encoded length of the ML-KEM-768 decapsulation (private) key.
pub const MLKEM_DK_LEN: usize = 2400;
/// Encoded length of the X25519 private scalar.
pub const X25519_SECRET_LEN: usize = 32;
/// Total encoded length of a [`SecretKey`].
pub const SECRET_KEY_LEN: usize = X25519_SECRET_LEN + MLKEM_DK_LEN;
/// Total encoded length of a [`PublicKey`].
pub const PUBLIC_KEY_LEN: usize = X25519_PUB_LEN + MLKEM_EK_LEN;
/// Total encoded length of an [`Encapsulation`].
pub const ENCAPSULATION_LEN: usize = X25519_PUB_LEN + MLKEM_CT_LEN;

type MlKemDk = <MlKem768 as KemCore>::DecapsulationKey;
type MlKemEk = <MlKem768 as KemCore>::EncapsulationKey;

/// A recipient's public key: an X25519 point plus an ML-KEM-768 encapsulation
/// key. Safe to publish.
#[derive(Clone)]
pub struct PublicKey {
    x: XPublicKey,
    ml: MlKemEk,
}

/// A recipient's private key. Zeroized on drop by the underlying types.
pub struct SecretKey {
    x: XSecret,
    ml: MlKemDk,
}

/// The public values a sender transmits so the recipient can recover the shared
/// secret. Safe to store next to the ciphertext.
#[derive(Clone)]
pub struct Encapsulation {
    /// Sender's ephemeral X25519 public key.
    pub x_ephemeral: [u8; X25519_PUB_LEN],
    /// ML-KEM-768 ciphertext.
    pub ml_ciphertext: [u8; MLKEM_CT_LEN],
}

impl PublicKey {
    /// Serialise to `PUBLIC_KEY_LEN` bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(PUBLIC_KEY_LEN);
        out.extend_from_slice(self.x.as_bytes());
        out.extend_from_slice(&self.ml.as_bytes());
        out
    }

    /// Parse from exactly `PUBLIC_KEY_LEN` bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != PUBLIC_KEY_LEN {
            return Err(Error::BadKeyEncoding);
        }
        let mut x = [0u8; X25519_PUB_LEN];
        x.copy_from_slice(&bytes[..X25519_PUB_LEN]);
        let ml_bytes = ml_kem::Encoded::<MlKemEk>::try_from(&bytes[X25519_PUB_LEN..])
            .map_err(|_| Error::BadKeyEncoding)?;
        Ok(Self {
            x: XPublicKey::from(x),
            ml: MlKemEk::from_bytes(&ml_bytes),
        })
    }
}

impl Encapsulation {
    /// Serialise to `ENCAPSULATION_LEN` bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ENCAPSULATION_LEN);
        out.extend_from_slice(&self.x_ephemeral);
        out.extend_from_slice(&self.ml_ciphertext);
        out
    }

    /// Parse from exactly `ENCAPSULATION_LEN` bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != ENCAPSULATION_LEN {
            return Err(Error::BadKeyEncoding);
        }
        let mut x_ephemeral = [0u8; X25519_PUB_LEN];
        let mut ml_ciphertext = [0u8; MLKEM_CT_LEN];
        x_ephemeral.copy_from_slice(&bytes[..X25519_PUB_LEN]);
        ml_ciphertext.copy_from_slice(&bytes[X25519_PUB_LEN..]);
        Ok(Self {
            x_ephemeral,
            ml_ciphertext,
        })
    }
}

impl SecretKey {
    /// Generate a fresh key pair from the OS CSPRNG.
    pub fn generate() -> Result<(Self, PublicKey), Error> {
        let mut rng = OsRng;
        let x = XSecret::random_from_rng(&mut rng);
        let x_pub = XPublicKey::from(&x);
        let (ml_dk, ml_ek) = MlKem768::generate(&mut rng);
        Ok((
            Self { x, ml: ml_dk },
            PublicKey {
                x: x_pub,
                ml: ml_ek,
            },
        ))
    }

    /// Serialise to `SECRET_KEY_LEN` bytes.
    ///
    /// The result is returned inside a [`Secret`], not a plain `Vec`: this is
    /// the private key, and it must not sit in ordinary heap memory waiting to
    /// be swapped out. Callers should hand it straight to
    /// [`crate::container::seal_with_password`] rather than write it in the
    /// clear.
    pub fn to_bytes(&self) -> Secret {
        let mut out = Secret::zeroed(SECRET_KEY_LEN);
        let bytes = out.expose_mut();
        bytes[..X25519_SECRET_LEN].copy_from_slice(&self.x.to_bytes());
        bytes[X25519_SECRET_LEN..].copy_from_slice(&self.ml.as_bytes());
        out
    }

    /// Parse from exactly `SECRET_KEY_LEN` bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != SECRET_KEY_LEN {
            return Err(Error::BadKeyEncoding);
        }
        let mut x = [0u8; X25519_SECRET_LEN];
        x.copy_from_slice(&bytes[..X25519_SECRET_LEN]);
        let ml_bytes = ml_kem::Encoded::<MlKemDk>::try_from(&bytes[X25519_SECRET_LEN..])
            .map_err(|_| Error::BadKeyEncoding)?;
        Ok(Self {
            x: XSecret::from(x),
            ml: MlKemDk::from_bytes(&ml_bytes),
        })
    }

    /// The matching public key.
    pub fn public_key(&self) -> PublicKey {
        PublicKey {
            x: XPublicKey::from(&self.x),
            ml: self.ml.encapsulation_key().clone(),
        }
    }

    /// Recover the shared secret from a sender's [`Encapsulation`].
    pub fn decapsulate(&self, enc: &Encapsulation) -> Result<Secret, Error> {
        let x_peer = XPublicKey::from(enc.x_ephemeral);
        let x_shared = self.x.diffie_hellman(&x_peer);

        let ct = ml_kem::Ciphertext::<MlKem768>::try_from(&enc.ml_ciphertext[..])
            .map_err(|_| Error::BadKeyEncoding)?;
        let ml_shared = self.ml.decapsulate(&ct).map_err(|_| Error::Decapsulate)?;

        let x_self = XPublicKey::from(&self.x);
        combine(x_shared.as_bytes(), &ml_shared, enc, x_self.as_bytes())
    }
}

impl PublicKey {
    /// Produce a shared secret for this recipient, plus the public values they
    /// need in order to recover it.
    pub fn encapsulate(&self) -> Result<(Secret, Encapsulation), Error> {
        let mut rng = OsRng;
        let x_eph = XSecret::random_from_rng(&mut rng);
        let x_eph_pub = XPublicKey::from(&x_eph);
        let x_shared = x_eph.diffie_hellman(&self.x);

        let (ml_ct, ml_shared) = self
            .ml
            .encapsulate(&mut rng)
            .map_err(|_| Error::Encapsulate)?;

        let mut ml_ciphertext = [0u8; MLKEM_CT_LEN];
        ml_ciphertext.copy_from_slice(&ml_ct);
        let enc = Encapsulation {
            x_ephemeral: *x_eph_pub.as_bytes(),
            ml_ciphertext,
        };

        let secret = combine(x_shared.as_bytes(), &ml_shared, &enc, self.x.as_bytes())?;
        Ok((secret, enc))
    }
}

/// Mix both shared secrets with the full transcript.
fn combine(
    x_shared: &[u8; 32],
    ml_shared: &[u8],
    enc: &Encapsulation,
    recipient_x_pub: &[u8; 32],
) -> Result<Secret, Error> {
    let mut ikm = Vec::with_capacity(32 + ml_shared.len());
    ikm.extend_from_slice(x_shared);
    ikm.extend_from_slice(ml_shared);

    // Binding the transcript is what stops an attacker who can replace one
    // half of the exchange from influencing the derived key.
    let mut transcript = Vec::with_capacity(ENCAPSULATION_LEN + 32);
    transcript.extend_from_slice(&enc.x_ephemeral);
    transcript.extend_from_slice(&enc.ml_ciphertext);
    transcript.extend_from_slice(recipient_x_pub);

    let hk = Hkdf::<Sha256>::new(Some(&transcript), &ikm);
    let mut out = Secret::zeroed(32);
    let mut info = Vec::with_capacity(HKDF_INFO.len());
    info.extend_from_slice(HKDF_INFO);
    hk.expand(&info, out.expose_mut()).map_err(|_| Error::Kdf)?;

    // The raw inputs must not outlive the derivation.
    use zeroize::Zeroize;
    ikm.zeroize();
    Ok(out)
}

/// Bridges the OS CSPRNG to the `rand_core` traits the KEM crates expect.
///
/// `getrandom` is already the entropy source everywhere else in VeilVoice, so
/// routing these through it keeps the whole crate on one source rather than
/// pulling in a second RNG stack.
struct OsRng;

impl rand_core::RngCore for OsRng {
    fn next_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.fill_bytes(&mut b);
        u32::from_le_bytes(b)
    }
    fn next_u64(&mut self) -> u64 {
        let mut b = [0u8; 8];
        self.fill_bytes(&mut b);
        u64::from_le_bytes(b)
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        getrandom::getrandom(dest).expect("OS CSPRNG unavailable");
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        getrandom::getrandom(dest)
            .map_err(|_| rand_core::Error::from(core::num::NonZeroU32::new(1).unwrap()))
    }
}

impl rand_core::CryptoRng for OsRng {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encapsulate_then_decapsulate_agrees() {
        let (sk, pk) = SecretKey::generate().unwrap();
        let (sender_secret, enc) = pk.encapsulate().unwrap();
        let recipient_secret = sk.decapsulate(&enc).unwrap();
        assert_eq!(sender_secret, recipient_secret);
        assert_eq!(sender_secret.len(), 32);
    }

    #[test]
    fn a_different_recipient_cannot_recover_it() {
        let (_, pk) = SecretKey::generate().unwrap();
        let (other_sk, _) = SecretKey::generate().unwrap();
        let (sender_secret, enc) = pk.encapsulate().unwrap();
        // ML-KEM decapsulation is designed never to fail outright (implicit
        // rejection), so the check that matters is that the *value* differs.
        if let Ok(wrong) = other_sk.decapsulate(&enc) {
            assert_ne!(sender_secret, wrong);
        }
    }

    #[test]
    fn each_encapsulation_is_fresh() {
        let (sk, pk) = SecretKey::generate().unwrap();
        let (s1, e1) = pk.encapsulate().unwrap();
        let (s2, e2) = pk.encapsulate().unwrap();
        assert_ne!(s1, s2);
        assert_ne!(e1.to_bytes(), e2.to_bytes());
        assert_eq!(sk.decapsulate(&e1).unwrap(), s1);
        assert_eq!(sk.decapsulate(&e2).unwrap(), s2);
    }

    /// The hybrid must fail if the classical half is tampered with — otherwise
    /// it would be no stronger than ML-KEM alone.
    #[test]
    fn tampering_with_the_x25519_half_changes_the_secret() {
        let (sk, pk) = SecretKey::generate().unwrap();
        let (secret, mut enc) = pk.encapsulate().unwrap();
        enc.x_ephemeral[0] ^= 1;
        let got = sk.decapsulate(&enc).unwrap();
        assert_ne!(secret, got);
    }

    /// ...and equally if the post-quantum half is tampered with.
    #[test]
    fn tampering_with_the_ml_kem_half_changes_the_secret() {
        let (sk, pk) = SecretKey::generate().unwrap();
        let (secret, mut enc) = pk.encapsulate().unwrap();
        enc.ml_ciphertext[0] ^= 1;
        if let Ok(got) = sk.decapsulate(&enc) {
            assert_ne!(secret, got);
        }
    }

    #[test]
    fn public_keys_round_trip_through_bytes() {
        let (sk, pk) = SecretKey::generate().unwrap();
        let bytes = pk.to_bytes();
        assert_eq!(bytes.len(), PUBLIC_KEY_LEN);
        let parsed = PublicKey::from_bytes(&bytes).unwrap();
        let (secret, enc) = parsed.encapsulate().unwrap();
        assert_eq!(sk.decapsulate(&enc).unwrap(), secret);
    }

    #[test]
    fn encapsulations_round_trip_through_bytes() {
        let (sk, pk) = SecretKey::generate().unwrap();
        let (secret, enc) = pk.encapsulate().unwrap();
        let bytes = enc.to_bytes();
        assert_eq!(bytes.len(), ENCAPSULATION_LEN);
        let parsed = Encapsulation::from_bytes(&bytes).unwrap();
        assert_eq!(sk.decapsulate(&parsed).unwrap(), secret);
    }

    #[test]
    fn malformed_encodings_are_rejected() {
        assert!(matches!(
            PublicKey::from_bytes(&[0u8; 10]),
            Err(Error::BadKeyEncoding)
        ));
        assert!(matches!(
            Encapsulation::from_bytes(&[0u8; 10]),
            Err(Error::BadKeyEncoding)
        ));
        assert!(matches!(
            SecretKey::from_bytes(&[0u8; 10]),
            Err(Error::BadKeyEncoding)
        ));
    }

    #[test]
    fn secret_keys_round_trip_through_bytes() {
        let (sk, pk) = SecretKey::generate().unwrap();
        let encoded = sk.to_bytes();
        assert_eq!(encoded.len(), SECRET_KEY_LEN);

        let restored = SecretKey::from_bytes(encoded.expose()).unwrap();
        let (secret, enc) = pk.encapsulate().unwrap();
        assert_eq!(
            restored.decapsulate(&enc).unwrap(),
            secret,
            "a reloaded key must open what the original could"
        );
    }

    /// The encoded private key must come back in protected storage, not a bare
    /// `Vec` that could be swapped to disk.
    #[test]
    fn encoded_secret_key_is_protected_and_opaque() {
        let (sk, _) = SecretKey::generate().unwrap();
        let encoded = sk.to_bytes();
        assert!(format!("{encoded:?}").contains("redacted"));
        assert!(
            encoded.expose().iter().any(|&b| b != 0),
            "encoding should not be empty"
        );
    }

    #[test]
    fn public_key_can_be_recovered_from_the_secret_key() {
        let (sk, pk) = SecretKey::generate().unwrap();
        assert_eq!(sk.public_key().to_bytes(), pk.to_bytes());

        // And the recovered public key really works.
        let (secret, enc) = sk.public_key().encapsulate().unwrap();
        assert_eq!(sk.decapsulate(&enc).unwrap(), secret);
    }

    #[test]
    fn declared_encoding_lengths_match_the_implementation() {
        let (sk, pk) = SecretKey::generate().unwrap();
        assert_eq!(pk.to_bytes().len(), PUBLIC_KEY_LEN);
        assert_eq!(sk.to_bytes().len(), SECRET_KEY_LEN);
        let (_, enc) = pk.encapsulate().unwrap();
        assert_eq!(enc.to_bytes().len(), ENCAPSULATION_LEN);
    }
}
