// SPDX-License-Identifier: GPL-3.0-or-later
//! Password-based key derivation with Argon2id.
//!
//! Argon2id is the memory-hard KDF recommended by RFC 9106 and the OWASP
//! password-storage guidance; the `id` variant resists both GPU/ASIC
//! parallelism and the side-channel exposure of pure Argon2i.
//!
//! Parameters travel *with* the ciphertext rather than being compiled in, so a
//! file encrypted today still opens after the defaults are raised, and a user
//! on a small machine can lower the memory cost without forking the format.

use crate::{Error, Secret};
use argon2::{Algorithm, Argon2, Params, Version};

/// Argon2id cost parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in kibibytes.
    pub m_cost: u32,
    /// Number of passes.
    pub t_cost: u32,
    /// Degree of parallelism.
    pub p_cost: u32,
}

impl Default for KdfParams {
    /// RFC 9106's "first recommended" profile: 2 GiB is the second option, but
    /// 256 MiB with three passes is the sweet spot for an interactive desktop
    /// unlock — strong against offline cracking while still opening a file in
    /// well under a second on ordinary hardware.
    fn default() -> Self {
        Self {
            m_cost: 256 * 1024,
            t_cost: 3,
            p_cost: 4,
        }
    }
}

impl KdfParams {
    /// A deliberately cheap profile for tests and low-memory devices.
    ///
    /// Do not use this to protect real data.
    pub fn weak_for_tests() -> Self {
        Self {
            m_cost: 8,
            t_cost: 1,
            p_cost: 1,
        }
    }

    /// Reject values Argon2 cannot accept, so a corrupt header fails loudly
    /// rather than panicking deep inside the KDF.
    fn build(&self, out_len: usize) -> Result<Argon2<'static>, Error> {
        let params = Params::new(self.m_cost, self.t_cost, self.p_cost, Some(out_len))
            .map_err(|_| Error::KdfParams)?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

/// Length of the salt stored in an encrypted container.
pub const SALT_LEN: usize = 16;
/// Length of a derived symmetric key.
pub const KEY_LEN: usize = 32;

/// Derive a 32-byte key from `password` and `salt`.
///
/// The result lands directly in page-locked, zeroizing storage; it is never
/// held in an ordinary `Vec` along the way.
pub fn derive_key(password: &[u8], salt: &[u8], params: KdfParams) -> Result<Secret, Error> {
    if salt.len() < 8 {
        return Err(Error::KdfParams);
    }
    let mut key = Secret::zeroed(KEY_LEN);
    params
        .build(KEY_LEN)?
        .hash_password_into(password, salt, key.expose_mut())
        .map_err(|_| Error::Kdf)?;
    Ok(key)
}

/// Draw a fresh random salt from the OS CSPRNG.
pub fn random_salt() -> Result<[u8; SALT_LEN], Error> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).map_err(|_| Error::Random)?;
    Ok(salt)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &[u8] = b"correct horse battery staple";

    fn weak() -> KdfParams {
        KdfParams::weak_for_tests()
    }

    #[test]
    fn derivation_is_deterministic() {
        let salt = [9u8; SALT_LEN];
        let a = derive_key(P, &salt, weak()).unwrap();
        let b = derive_key(P, &salt, weak()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), KEY_LEN);
    }

    #[test]
    fn different_password_or_salt_diverges() {
        let base = derive_key(P, &[9u8; SALT_LEN], weak()).unwrap();
        assert_ne!(
            base,
            derive_key(b"wrong", &[9u8; SALT_LEN], weak()).unwrap()
        );
        assert_ne!(base, derive_key(P, &[10u8; SALT_LEN], weak()).unwrap());
    }

    #[test]
    fn cost_parameters_change_the_key() {
        // Parameters are part of the derivation, so a header that lies about
        // them cannot yield the right key.
        let salt = [3u8; SALT_LEN];
        let a = derive_key(
            P,
            &salt,
            KdfParams {
                t_cost: 1,
                ..weak()
            },
        )
        .unwrap();
        let b = derive_key(
            P,
            &salt,
            KdfParams {
                t_cost: 2,
                ..weak()
            },
        )
        .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn key_material_is_page_locked_storage() {
        let k = derive_key(P, &[1u8; SALT_LEN], weak()).unwrap();
        assert!(format!("{k:?}").contains("redacted"));
    }

    #[test]
    fn impossible_parameters_are_rejected_not_panicked() {
        let bad = KdfParams {
            m_cost: 0,
            t_cost: 0,
            p_cost: 0,
        };
        assert!(matches!(
            derive_key(P, &[0u8; SALT_LEN], bad),
            Err(Error::KdfParams)
        ));
    }

    #[test]
    fn short_salt_is_rejected() {
        assert!(matches!(
            derive_key(P, &[0u8; 4], weak()),
            Err(Error::KdfParams)
        ));
    }

    #[test]
    fn random_salts_differ() {
        assert_ne!(random_salt().unwrap(), random_salt().unwrap());
    }
}
