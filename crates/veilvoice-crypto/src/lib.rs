// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-crypto
//!
//! Key derivation, post-quantum-hybrid key agreement, authenticated encryption
//! and amnesic secret storage for VeilVoice.
//!
//! ## What this crate is for
//!
//! [`veilvoice_core`](../veilvoice_core/index.html) makes a voice
//! unrecognisable; it does not hide the *words*, and it is not meant to. When a
//! recording needs to stay secret as well, at rest on disk or in transit to
//! someone else, that is this crate's job.
//!
//! - [`kdf`], Argon2id, for turning a password into a key.
//! - [`hybrid`], X25519 + ML-KEM-768, so a recording captured today is not
//!   readable by a quantum adversary tomorrow.
//! - [`aead`], XChaCha20-Poly1305, with random nonces and authenticated
//!   associated data.
//! - [`container`], the `.veil` file format that ties the three together.
//! - [`amnesia`], page-locked, zeroizing, constant-time-comparable secrets.
//! - [`shred`], secure erasure, and an honest account of what that is worth
//!   on flash storage.
//! - [`privatefile`], writing a file that is owner-only from the moment it
//!   exists, rather than world-readable until a second syscall tightens it.
//! - [`lock`], the application lock: an Argon2id verifier with a rate limit,
//!   which protects against casual access and says so rather than pretending to
//!   be tamper-proof.
//!
//! ## Threat model, stated plainly
//!
//! This crate protects data **at rest and in transit** against an attacker who
//! later obtains the file, including one who stores it until quantum hardware
//! exists. It does **not** protect against an attacker who is already running
//! code as you, or who can read this process's memory: page-locking keeps keys
//! out of the swap file, not out of a debugger. Hibernation writes RAM to disk
//! wholesale and defeats locking entirely.
//!
//! ## Example
//!
//! ```
//! use veilvoice_crypto::{container, kdf};
//!
//! # fn main() -> Result<(), veilvoice_crypto::Error> {
//! // Cheap parameters so the doctest is fast; real callers use the default.
//! let params = kdf::KdfParams::weak_for_tests();
//! let sealed = container::seal_with_password(b"pass phrase", b"audio bytes", params)?;
//! assert_eq!(container::open_with_password(b"pass phrase", &sealed)?, b"audio bytes");
//! assert!(container::open_with_password(b"wrong", &sealed).is_err());
//! # Ok(())
//! # }
//! ```
//!
//! VeilVoice contains **no `unsafe` code at all**, including the page-locking
//! in [`amnesia`], which goes through a safe wrapper.
//!
//! # In plain words
//!
//! This is the locking.
//!
//! Two separate things use it. Recordings are sealed on your disk so that somebody
//! who takes the disk cannot listen to them, and the app can be put behind a
//! password so somebody who picks up your unlocked computer cannot open it.
//!
//! Those two passwords are deliberately different. Opening the program should not
//! be the same act as unsealing everything it has ever written.
//!
//! The maths is chosen to be slow to guess and to still be safe if somebody one
//! day builds a quantum computer. Nothing is sent anywhere; the sealing happens on
//! your machine and the key is made from your password each time.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod aead;
pub mod amnesia;
pub mod container;
pub mod hoard;
pub mod hybrid;
pub mod kdf;
pub mod lock;
pub mod privatefile;
pub mod shred;
pub mod tape;
pub mod vault;
pub mod weave;

pub use amnesia::Secret;
pub use lock::{AppLock, LockStore};
pub use shred::{shred_file, Passes, ShredReport};
pub use tape::Tape;
pub use vault::Vault;

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Everything that can go wrong in this crate.
///
/// Decryption failures are deliberately coarse: [`Error::Decrypt`] does not say
/// *why* authentication failed, because distinguishing a wrong password from a
/// corrupt tag would hand an attacker an oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The OS random number generator was unavailable.
    Random,
    /// Argon2 rejected the cost parameters, or the salt was too short.
    KdfParams,
    /// The declared memory cost is legal but above the ceiling this caller set.
    /// See [`kdf::KdfParams::within`].
    KdfCostRefused {
        /// The memory cost the file asked for, in KiB.
        requested: u32,
        /// The ceiling the caller allowed, in KiB.
        ceiling: u32,
    },
    /// Key derivation failed.
    Kdf,
    /// A key was not the expected length.
    KeyLength,
    /// Encryption failed.
    Encrypt,
    /// Decryption or authentication failed.
    Decrypt,
    /// KEM encapsulation failed.
    Encapsulate,
    /// KEM decapsulation failed.
    Decapsulate,
    /// A public key or encapsulation was malformed.
    BadKeyEncoding,
    /// The data does not start with the container magic.
    BadMagic,
    /// The container header is structurally invalid.
    BadHeader,
    /// The container ended sooner than its header promised.
    Truncated,
    /// The container uses a format version this build does not support.
    UnsupportedVersion(u8),
    /// The container uses an unknown locking mode.
    UnsupportedMode(u8),
    /// The container is locked a different way than the call assumed.
    WrongMode,
    /// A file could not be securely erased.
    Shred,
    /// The path named for erasure is a symbolic link. Following it would
    /// overwrite whatever it points at while deleting only the link, so it is
    /// refused rather than obeyed.
    ShredSymlink,
    /// The app-lock password was wrong.
    AppLockRejected,
    /// Too many wrong app-lock attempts. The payload is the number of seconds
    /// still to wait before another attempt will be considered.
    AppLockCooldown(u64),
    /// The app-lock file could not be read, written or removed.
    AppLockStore,
    /// A lock is already set here, so creating one was refused.
    ///
    /// Its own variant because it was reported as [`Self::AppLockStore`], and
    /// that message -- "could not read or write the app-lock file" -- reads as
    /// a broken installation rather than as "you already have one of these".
    /// A user who saw it had no way to tell a refusal from a failure, which is
    /// half of what made F-141 so confusing to hit.
    AppLockExists,
    /// The password was changed, but the second copy of the lock could not be
    /// updated, so it still holds the previous one.
    AppLockSpareStale,
    /// A [`tape::Tape`] was asked to copy itself into a buffer that is not its
    /// own length. Refused rather than part-filled: a recording written into a
    /// buffer sized for a different one is a truncated file nothing would flag.
    TapeLength,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::Random => "the operating system random number generator is unavailable",
            Self::KdfParams => "invalid key-derivation parameters",
            Self::KdfCostRefused { requested, ceiling } => {
                return write!(
                    f,
                    "this file asks for {requested} KiB of memory to open, above the \
                     {ceiling} KiB ceiling this caller allows"
                )
            }
            Self::Kdf => "key derivation failed",
            Self::KeyLength => "key has the wrong length",
            Self::Encrypt => "encryption failed",
            Self::Decrypt => "decryption failed: wrong key, or the data was altered",
            Self::Encapsulate => "key encapsulation failed",
            Self::Decapsulate => "key decapsulation failed",
            Self::BadKeyEncoding => "malformed key or encapsulation",
            Self::BadMagic => "not a VeilVoice container",
            Self::BadHeader => "malformed container header",
            Self::Truncated => "container is truncated",
            Self::UnsupportedVersion(v) => return write!(f, "unsupported container version {v}"),
            Self::UnsupportedMode(m) => return write!(f, "unsupported container mode {m}"),
            Self::WrongMode => "container is locked with a different method",
            Self::Shred => "could not securely erase the file",
            Self::ShredSymlink => {
                "that path is a symbolic link, so erasing it would destroy whatever it \
                 points at and delete only the link; name the real file instead"
            }
            Self::AppLockRejected => "wrong app-lock password",
            Self::AppLockCooldown(secs) => {
                return write!(f, "too many attempts, so wait {secs}s before trying again")
            }
            Self::AppLockStore => "could not read or write the app-lock file",
            Self::AppLockExists => {
                "an app lock is already set on this machine. Change its password \
                 from the security tab, or remove it there first"
            }
            Self::AppLockSpareStale => {
                "the password was changed here, but the administrator-owned copy of the lock \
                 still holds the previous one. Run VeilVoice as an administrator once to \
                 finish the change."
            }
            Self::TapeLength => {
                "the buffer offered for the recording is not the length of the recording"
            }
        };
        f.write_str(msg)
    }
}

impl std::error::Error for Error {}
