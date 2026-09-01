// SPDX-License-Identifier: GPL-3.0-or-later
//! The `.veil` encrypted container format.
//!
//! A single self-describing blob: everything needed to decrypt except the
//! password or private key travels with the ciphertext, so a file stays
//! readable after the default KDF costs are raised.
//!
//! ```text
//!  offset  size  field
//!       0     8  magic "VEILVOX1"
//!       8     1  format version (1)
//!       9     1  mode: 1 = password, 2 = hybrid public key
//!      10     2  reserved, must be zero
//!      12     4  Argon2id m_cost (KiB, little-endian)     password mode only
//!      16     4  Argon2id t_cost                          password mode only
//!      20     4  Argon2id p_cost                          password mode only
//!      24    16  Argon2id salt                            password mode only
//!      40    24  XChaCha20 nonce
//!      64     4  encapsulation length (little-endian)
//!      68     N  encapsulation                            hybrid mode only
//!    68+N     …  ciphertext ‖ Poly1305 tag
//! ```
//!
//! **The entire header is the AEAD's associated data.** Editing any byte of it
//! by downgrading the KDF cost, swapping the mode or corrupting the salt, makes
//! decryption fail rather than silently changing behaviour. Unused fields are
//! written as zero and are still authenticated, so they cannot be used as a
//! covert channel or a downgrade vector.
//!
//! # In plain words
//!
//! The shape of an encrypted `.veil` file.
//!
//! Everything needed to open it travels inside it, apart from the passphrase or
//! the key. That means a file made today still opens years later, even after
//! VeilVoice has changed how hard it makes the encryption by default: the file
//! remembers what it was made with.
//!
//! The part at the front that describes the file is itself covered by the tamper
//! check, so it cannot be edited to make the rest open more easily.

use crate::{aead, hybrid, kdf, Error, Secret};

/// Magic bytes at the start of every container.
pub const MAGIC: &[u8; 8] = b"VEILVOX1";
/// Format version this build writes.
pub const FORMAT_VERSION: u8 = 1;
/// Fixed header length in bytes, before any encapsulation.
pub const HEADER_LEN: usize = 68;

const MODE_PASSWORD: u8 = 1;
const MODE_HYBRID: u8 = 2;

/// How a container is locked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Argon2id over a user password.
    Password,
    /// X25519 + ML-KEM-768 to a recipient's public key.
    Hybrid,
}

/// A parsed container header.
#[derive(Clone, Debug)]
pub struct Header {
    /// How the container is locked.
    pub mode: Mode,
    /// Argon2id costs (meaningful in password mode).
    pub kdf: kdf::KdfParams,
    /// Argon2id salt (meaningful in password mode).
    pub salt: [u8; kdf::SALT_LEN],
    /// AEAD nonce.
    pub nonce: [u8; aead::NONCE_LEN],
    /// Hybrid KEM encapsulation (empty in password mode).
    pub encapsulation: Vec<u8>,
}

impl Header {
    /// Serialise exactly as it appears on disk. This byte string is also the
    /// AEAD associated data.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.encapsulation.len());
        out.extend_from_slice(MAGIC);
        out.push(FORMAT_VERSION);
        out.push(match self.mode {
            Mode::Password => MODE_PASSWORD,
            Mode::Hybrid => MODE_HYBRID,
        });
        out.extend_from_slice(&[0u8; 2]); // reserved
        out.extend_from_slice(&self.kdf.m_cost.to_le_bytes());
        out.extend_from_slice(&self.kdf.t_cost.to_le_bytes());
        out.extend_from_slice(&self.kdf.p_cost.to_le_bytes());
        out.extend_from_slice(&self.salt);
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&(self.encapsulation.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.encapsulation);
        out
    }

    /// Parse a header, returning it with the offset at which ciphertext starts.
    pub fn parse(bytes: &[u8]) -> Result<(Self, usize), Error> {
        if bytes.len() < HEADER_LEN {
            return Err(Error::Truncated);
        }
        if &bytes[..8] != MAGIC {
            return Err(Error::BadMagic);
        }
        if bytes[8] != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(bytes[8]));
        }
        let mode = match bytes[9] {
            MODE_PASSWORD => Mode::Password,
            MODE_HYBRID => Mode::Hybrid,
            other => return Err(Error::UnsupportedMode(other)),
        };
        // Reserved bytes are authenticated; refuse anything non-zero rather
        // than let a future flag be silently ignored by an old build.
        if bytes[10] != 0 || bytes[11] != 0 {
            return Err(Error::BadHeader);
        }

        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let kdf = kdf::KdfParams {
            m_cost: u32_at(12),
            t_cost: u32_at(16),
            p_cost: u32_at(20),
        };
        let mut salt = [0u8; kdf::SALT_LEN];
        salt.copy_from_slice(&bytes[24..40]);
        let mut nonce = [0u8; aead::NONCE_LEN];
        nonce.copy_from_slice(&bytes[40..64]);

        let enc_len = u32_at(64) as usize;
        match mode {
            Mode::Password if enc_len != 0 => return Err(Error::BadHeader),
            Mode::Hybrid if enc_len != hybrid::ENCAPSULATION_LEN => return Err(Error::BadHeader),
            _ => {}
        }
        let end = HEADER_LEN.checked_add(enc_len).ok_or(Error::BadHeader)?;
        if bytes.len() < end {
            return Err(Error::Truncated);
        }
        let encapsulation = bytes[HEADER_LEN..end].to_vec();

        Ok((
            Self {
                mode,
                kdf,
                salt,
                nonce,
                encapsulation,
            },
            end,
        ))
    }
}

/// The conventional path of the sealed form of `path`.
///
/// `.veil` is *appended* rather than substituted, so `recording.veiled.wav`
/// becomes `recording.veiled.wav.veil` and the original name, including what
/// kind of file it is, survives decryption without being guessed at.
pub fn veil_path(path: &std::path::Path) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".veil");
    std::path::PathBuf::from(name)
}

/// Encrypt `plaintext` under a password.
pub fn seal_with_password(
    password: &[u8],
    plaintext: &[u8],
    params: kdf::KdfParams,
) -> Result<Vec<u8>, Error> {
    let header = Header {
        mode: Mode::Password,
        kdf: params,
        salt: kdf::random_salt()?,
        nonce: aead::random_nonce()?,
        encapsulation: Vec::new(),
    };
    let key = kdf::derive_key(password, &header.salt, params)?;
    finish(header, &key, plaintext)
}

/// Encrypt `plaintext` to a recipient's hybrid public key.
pub fn seal_to_public_key(
    recipient: &hybrid::PublicKey,
    plaintext: &[u8],
) -> Result<Vec<u8>, Error> {
    let (key, enc) = recipient.encapsulate()?;
    let header = Header {
        mode: Mode::Hybrid,
        kdf: kdf::KdfParams {
            m_cost: 0,
            t_cost: 0,
            p_cost: 0,
        },
        salt: [0u8; kdf::SALT_LEN],
        nonce: aead::random_nonce()?,
        encapsulation: enc.to_bytes(),
    };
    finish(header, &key, plaintext)
}

fn finish(header: Header, key: &Secret, plaintext: &[u8]) -> Result<Vec<u8>, Error> {
    let aad = header.to_bytes();
    let ciphertext = aead::seal(key, &header.nonce, &aad, plaintext)?;
    let mut out = aad;
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a password-locked container.
pub fn open_with_password(password: &[u8], container: &[u8]) -> Result<Vec<u8>, Error> {
    open_with_password_within(password, container, kdf::KdfParams::MAX_M_COST)
}

/// Decrypt a password-locked container, refusing one that declares a memory
/// cost above `max_m_cost`.
///
/// The cost travels with the file so that a container written years ago still
/// opens after the defaults are raised. The price is that a *hostile* file can
/// declare a legitimate-but-large cost and make itself slow and expensive to
/// open. See F-3's residual in `docs/AUDIT.md`. When a person chose the file
/// and can stop waiting, that is an acceptable price and
/// [`open_with_password`] pays it. When nothing human is present, such as a
/// batch job, a service or anything handed files it did not choose, pass
/// [`kdf::KdfParams::UNATTENDED_MAX_M_COST`] here and get
/// [`Error::KdfCostRefused`] instead of the memory.
pub fn open_with_password_within(
    password: &[u8],
    container: &[u8],
    max_m_cost: u32,
) -> Result<Vec<u8>, Error> {
    let (header, body) = Header::parse(container)?;
    if header.mode != Mode::Password {
        return Err(Error::WrongMode);
    }
    // Checked before the derivation, so the memory is never asked for.
    header.kdf.within(max_m_cost)?;
    let key = kdf::derive_key(password, &header.salt, header.kdf)?;
    aead::open(&key, &header.nonce, &container[..body], &container[body..])
}

/// Decrypt a container addressed to `recipient`.
pub fn open_with_secret_key(
    recipient: &hybrid::SecretKey,
    container: &[u8],
) -> Result<Vec<u8>, Error> {
    let (header, body) = Header::parse(container)?;
    if header.mode != Mode::Hybrid {
        return Err(Error::WrongMode);
    }
    let enc = hybrid::Encapsulation::from_bytes(&header.encapsulation)?;
    let key = recipient.decapsulate(&enc)?;
    aead::open(&key, &header.nonce, &container[..body], &container[body..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weak() -> kdf::KdfParams {
        kdf::KdfParams::weak_for_tests()
    }

    const MSG: &[u8] = b"a recording that must never leak";

    #[test]
    fn password_round_trip() {
        let ct = seal_with_password(b"hunter2", MSG, weak()).unwrap();
        assert_eq!(&ct[..8], MAGIC);
        assert!(
            !ct.windows(MSG.len()).any(|w| w == MSG),
            "plaintext visible"
        );
        assert_eq!(open_with_password(b"hunter2", &ct).unwrap(), MSG);
    }

    #[test]
    fn wrong_password_fails() {
        let ct = seal_with_password(b"hunter2", MSG, weak()).unwrap();
        assert!(matches!(
            open_with_password(b"hunter3", &ct),
            Err(Error::Decrypt)
        ));
    }

    #[test]
    fn hybrid_round_trip() {
        let (sk, pk) = hybrid::SecretKey::generate().unwrap();
        let ct = seal_to_public_key(&pk, MSG).unwrap();
        assert_eq!(open_with_secret_key(&sk, &ct).unwrap(), MSG);
    }

    #[test]
    fn a_different_key_cannot_open_it() {
        let (_, pk) = hybrid::SecretKey::generate().unwrap();
        let (other, _) = hybrid::SecretKey::generate().unwrap();
        let ct = seal_to_public_key(&pk, MSG).unwrap();
        assert!(open_with_secret_key(&other, &ct).is_err());
    }

    /// The central property of the format: the header is authenticated, so an
    /// attacker cannot downgrade the KDF cost to make cracking cheap.
    ///
    /// Both downgrade routes are covered. A cost Argon2 still accepts derives a
    /// different key and fails the AEAD; a cost below Argon2's own minimum is
    /// refused outright. Either way the file does not open, which is the
    /// property that matters.
    #[test]
    fn downgrading_the_kdf_cost_is_detected() {
        let strong = kdf::KdfParams {
            m_cost: 64,
            t_cost: 2,
            p_cost: 1,
        };

        let mut valid_downgrade = seal_with_password(b"pw", MSG, strong).unwrap();
        valid_downgrade[12..16].copy_from_slice(&8u32.to_le_bytes());
        assert!(matches!(
            open_with_password(b"pw", &valid_downgrade),
            Err(Error::Decrypt)
        ));

        let mut absurd_downgrade = seal_with_password(b"pw", MSG, strong).unwrap();
        absurd_downgrade[12..16].copy_from_slice(&1u32.to_le_bytes());
        assert!(matches!(
            open_with_password(b"pw", &absurd_downgrade),
            Err(Error::KdfParams)
        ));
    }

    #[test]
    fn tampering_with_the_salt_or_nonce_is_detected() {
        for offset in [24usize, 40] {
            let mut ct = seal_with_password(b"pw", MSG, weak()).unwrap();
            ct[offset] ^= 1;
            assert!(open_with_password(b"pw", &ct).is_err(), "offset {offset}");
        }
    }

    #[test]
    fn tampering_with_the_ciphertext_is_detected() {
        let mut ct = seal_with_password(b"pw", MSG, weak()).unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 1;
        assert!(matches!(
            open_with_password(b"pw", &ct),
            Err(Error::Decrypt)
        ));
    }

    #[test]
    fn modes_are_not_interchangeable() {
        let (sk, pk) = hybrid::SecretKey::generate().unwrap();
        let pw_ct = seal_with_password(b"pw", MSG, weak()).unwrap();
        let hy_ct = seal_to_public_key(&pk, MSG).unwrap();
        assert!(matches!(
            open_with_secret_key(&sk, &pw_ct),
            Err(Error::WrongMode)
        ));
        assert!(matches!(
            open_with_password(b"pw", &hy_ct),
            Err(Error::WrongMode)
        ));
    }

    #[test]
    fn malformed_containers_are_rejected_cleanly() {
        assert!(matches!(
            open_with_password(b"pw", b"short"),
            Err(Error::Truncated)
        ));

        let mut bad_magic = seal_with_password(b"pw", MSG, weak()).unwrap();
        bad_magic[0] = b'X';
        assert!(matches!(
            open_with_password(b"pw", &bad_magic),
            Err(Error::BadMagic)
        ));

        let mut bad_ver = seal_with_password(b"pw", MSG, weak()).unwrap();
        bad_ver[8] = 99;
        assert!(matches!(
            open_with_password(b"pw", &bad_ver),
            Err(Error::UnsupportedVersion(99))
        ));

        let mut reserved = seal_with_password(b"pw", MSG, weak()).unwrap();
        reserved[10] = 1;
        assert!(matches!(
            open_with_password(b"pw", &reserved),
            Err(Error::BadHeader)
        ));
    }

    #[test]
    fn header_round_trips_exactly() {
        let ct = seal_with_password(b"pw", MSG, weak()).unwrap();
        let (h, body) = Header::parse(&ct).unwrap();
        assert_eq!(body, HEADER_LEN);
        assert_eq!(h.to_bytes(), &ct[..body]);

        let (sk, pk) = hybrid::SecretKey::generate().unwrap();
        let hct = seal_to_public_key(&pk, MSG).unwrap();
        let (h2, body2) = Header::parse(&hct).unwrap();
        assert_eq!(body2, HEADER_LEN + hybrid::ENCAPSULATION_LEN);
        assert_eq!(h2.to_bytes(), &hct[..body2]);
        assert_eq!(open_with_secret_key(&sk, &hct).unwrap(), MSG);
    }

    #[test]
    fn empty_payload_round_trips() {
        let ct = seal_with_password(b"pw", b"", weak()).unwrap();
        assert_eq!(open_with_password(b"pw", &ct).unwrap(), b"");
    }

    /// Appending rather than replacing keeps the original extension, so an
    /// opened container is still recognisably a WAV.
    #[test]
    fn veil_paths_append_and_keep_the_original_extension() {
        use std::path::Path;
        assert_eq!(
            veil_path(Path::new("clip.veiled.wav")),
            Path::new("clip.veiled.wav.veil")
        );
        assert_eq!(veil_path(Path::new("notes")), Path::new("notes.veil"));
        assert_eq!(
            veil_path(Path::new("a.b/c.wav")),
            Path::new("a.b/c.wav.veil")
        );
    }

    /// The cost ceiling for a caller with nobody watching. A hostile container
    /// can declare a legal-but-expensive cost; an unattended caller must be
    /// able to decline before the memory is asked for, not after.
    #[test]
    fn an_unattended_caller_can_refuse_an_expensive_container() {
        let expensive = kdf::KdfParams {
            m_cost: 64 * 1024, // 64 MiB, legal and more than we will allow
            t_cost: 1,
            p_cost: 1,
        };
        let ct = seal_with_password(b"pw", MSG, expensive).unwrap();

        // The default path still opens it: a person chose this file.
        assert_eq!(open_with_password(b"pw", &ct).unwrap(), MSG);

        // A caller that set a ceiling gets told, with both numbers.
        match open_with_password_within(b"pw", &ct, 8 * 1024) {
            Err(Error::KdfCostRefused { requested, ceiling }) => {
                assert_eq!(requested, 64 * 1024);
                assert_eq!(ceiling, 8 * 1024);
            }
            other => panic!("expected a refusal, got {other:?}"),
        }

        // And a ceiling above the file's cost is not in the way.
        assert_eq!(
            open_with_password_within(b"pw", &ct, kdf::KdfParams::UNATTENDED_MAX_M_COST).unwrap(),
            MSG
        );
    }

    /// The published unattended ceiling has to be usable: comfortably above
    /// this crate's own default, comfortably below the absurd-value cap.
    /// Checked at compile time, so tightening either constant past the other
    /// fails the build rather than a test run.
    const _: () = assert!(
        kdf::KdfParams::UNATTENDED_MAX_M_COST < kdf::KdfParams::MAX_M_COST,
        "the unattended ceiling must sit below the absurd-value cap"
    );

    #[test]
    fn the_unattended_ceiling_admits_this_crates_own_default() {
        assert!(kdf::KdfParams::UNATTENDED_MAX_M_COST > kdf::KdfParams::default().m_cost);
        assert!(kdf::KdfParams::default()
            .within(kdf::KdfParams::UNATTENDED_MAX_M_COST)
            .is_ok());
    }

    #[test]
    fn two_seals_of_the_same_input_differ() {
        let a = seal_with_password(b"pw", MSG, weak()).unwrap();
        let b = seal_with_password(b"pw", MSG, weak()).unwrap();
        assert_ne!(a, b, "fresh salt and nonce must decorrelate containers");
    }
}
