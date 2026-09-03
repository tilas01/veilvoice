// SPDX-License-Identifier: GPL-3.0-or-later
//! The obfuscated program folder: what VeilVoice keeps on disk, under names
//! that mean nothing and beside files that hold nothing.
//!
//! # What this buys, stated before anything else
//!
//! Somebody who opens VeilVoice's folder without the app-lock passphrase sees
//! a few dozen files with names like `k7Qa1mXv9pLd0RtYbN3zHwFe`, all of them
//! full of bytes that look random, all of them one of a handful of sizes.
//! They cannot tell which files hold settings, which hold measurements, which
//! hold anything at all, and which are junk this module wrote precisely so
//! that the question has no answer from outside.
//!
//! That is the whole claim. It is worth having and it is smaller than it
//! sounds, so here is the other half, in the same breath:
//!
//! - **It does not hide that you use VeilVoice.** The folder is called
//!   `veilvoice`, the lock file sits in it under its own name, and the
//!   application is on disk. Anybody looking knows.
//! - **It does not hide how much you have.** File count and the bucket sizes
//!   are visible. Decoys blur that number; they do not erase it.
//! - **It is not protection from someone who has your passphrase**, and it is
//!   not protection while the application is open and unlocked. At that moment
//!   everything here is readable, because it has to be.
//! - **It does not stop deletion.** Anybody who can read this folder can empty
//!   it. What they cannot do is empty it *quietly*: see the roster below.
//! - **Somebody who knows VeilVoice knows what these files are.** The format
//!   is public, this file is the specification, and a forensic examiner who
//!   recognises it will recognise it here. Obfuscation is not steganography
//!   and this module does not pretend otherwise.
//!
//! What it does buy is the thing the app lock could not previously offer: a
//! reason to exist beyond a password prompt. Before this, the lock verified a
//! passphrase and guarded a window; the files behind it sat in the clear under
//! their own names, and deleting the lock file removed the whole obstacle.
//! Now the passphrase derives the key that names and opens these records, so
//! deleting the lock does not reveal them -- it destroys the only copy of the
//! salt they were derived through, and takes them with it. That is a real
//! change in what the lock is worth, and also a real way to lose your data,
//! which is why [`crate::lock`] keeps a second copy and the interface says so.
//!
//! # How a record is found
//!
//! Every record has a *logical* name that only the program uses: `settings`,
//! `measured`, `tour`. The file it lives in is named
//!
//! ```text
//! base64url(HMAC-SHA256(store_key, "veilvoice/hoard/name" || logical)[..18])
//! ```
//!
//! which is twenty-four characters of base64 with no padding and no extension.
//! Eighteen bytes rather than a round sixteen so the encoding comes out exact:
//! twenty-four characters with nothing to pad, which is one less thing to tell
//! a name apart from a decoy.
//!
//! The derivation is deterministic, so the program does not search: it
//! computes the name it wants and opens that file. This is what makes the
//! selection *cryptographic* rather than a lookup table. There is no index
//! mapping `settings` to a filename, because an index is exactly the thing an
//! attacker would want. Without the key there is no way to run the derivation,
//! and with the key there is no need to store it.
//!
//! # What is inside one
//!
//! ```text
//! [24-byte nonce][ChaCha20-Poly1305 over: [4-byte length][data][junk]]
//! ```
//!
//! The junk pads every record up to one of a few fixed sizes, so a file's
//! length says which bucket it fell in and nothing finer. The additional data
//! for the AEAD is the filename itself, which binds a record to its name: two
//! real files cannot be swapped without the swap being detected, because each
//! one authenticates the name it is supposed to be under.
//!
//! The per-record key is a separate HKDF branch, so one record's key says
//! nothing about another's.
//!
//! # The roster, and the one deletion claim this can honestly make
//!
//! A record can be modified, and the AEAD catches that: any edit fails to
//! authenticate and [`Hoard::audit`] reports the record as tampered with.
//!
//! Deletion is harder, because a file that is not there looks exactly like a
//! file that was never written. So the hoard keeps one more record, the
//! roster, listing the logical names that should exist. It is stored like any
//! other record, under a derived name, encrypted and padded, so it is not
//! identifiable from outside either.
//!
//! That gives a real answer for deletion of *some* of the folder: a record in
//! the roster whose file is gone was deleted, and audit says so. It gives no
//! answer for deletion of *all* of it, including the roster, and nothing
//! stored in this folder ever could -- at that point the only evidence is that
//! the folder is empty, which you can see for yourself. The roster is missing
//! while the lock exists is itself reported, which is the closest honest
//! approximation, and it is stated as what it is rather than dressed up.
//!
//! # In plain words
//!
//! VeilVoice's own files are encrypted and given meaningless names, and a pile
//! of decoy files sits among them so nobody can tell which is which. Only the
//! program, once you have unlocked it, can work out which file is which.
//!
//! Anybody looking at the folder still knows you use VeilVoice, and can still
//! delete the lot. What they cannot do is read any of it, work out how much of
//! it there is, or change any of it without VeilVoice telling you next time
//! you unlock.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::amnesia::Secret;
use crate::{aead, kdf, Error};

/// HKDF label for the filename key. Distinct from every other label in the
/// project so a name can never coincide with a key.
const INFO_NAME: &[u8] = b"veilvoice/hoard/name";
/// HKDF label prefix for a record's own encryption key.
const INFO_REC: &[u8] = b"veilvoice/hoard/rec";

/// How many bytes of the name HMAC end up in the filename.
///
/// Eighteen encodes to exactly twenty-four base64 characters with no padding.
/// 144 bits is far past any collision concern for a few dozen files; the
/// choice is driven by the encoding coming out clean.
const NAME_BYTES: usize = 18;

/// The length prefix inside the padded plaintext.
const LEN_PREFIX: usize = 4;

/// The sizes a record is padded up to, in bytes of plaintext.
///
/// A record's file length reveals which of these it landed in and nothing
/// finer. Beyond the largest, records round up to a whole mebibyte.
const BUCKETS: &[usize] = &[256, 1024, 4096, 16_384, 65_536, 262_144, 1_048_576];

/// The logical name of the roster record.
const ROSTER: &str = "\u{0}roster";

/// The key that names and opens everything in the hoard.
///
/// Derived from the app-lock passphrase alongside the verifier and the tag
/// key, under its own HKDF label, so it is independent of both. It exists only
/// while the application is unlocked.
pub struct StoreKey(Secret);

impl StoreKey {
    /// Wrap raw key material. The caller is trusted to have derived it.
    pub fn from_secret(secret: Secret) -> Self {
        Self(secret)
    }

    /// Derive a subkey under a label.
    fn expand(&self, label: &[u8], logical: &str, out: &mut [u8]) -> Result<(), Error> {
        let hk = hkdf::Hkdf::<sha2::Sha256>::from_prk(self.0.expose()).map_err(|_| Error::Kdf)?;
        let mut info = Vec::with_capacity(label.len() + 1 + logical.len());
        info.extend_from_slice(label);
        info.push(b'/');
        info.extend_from_slice(logical.as_bytes());
        hk.expand(&info, out).map_err(|_| Error::Kdf)
    }
}

/// Base64url, no padding. Sixty-four characters, none of which needs escaping
/// in a filename on any platform this project targets.
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

/// The bucket a payload of this length pads up to.
fn bucket_for(len: usize) -> usize {
    for &b in BUCKETS {
        if len <= b {
            return b;
        }
    }
    len.div_ceil(1_048_576) * 1_048_576
}

/// What an audit found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Audit {
    /// Records that opened and authenticated cleanly.
    pub intact: Vec<String>,
    /// Records whose file is present but failed to authenticate: it was
    /// edited, truncated, or put under the wrong name.
    pub tampered: Vec<String>,
    /// Records the roster says should exist whose file is gone.
    pub missing: Vec<String>,
    /// Files in the folder that are not records of ours. Decoys land here, and
    /// so does anything else somebody dropped in; the two are not
    /// distinguishable and this field does not pretend they are.
    pub unrecognised: usize,
}

impl Audit {
    /// Whether anything was found that a user should be told about.
    pub fn is_clean(&self) -> bool {
        self.tampered.is_empty() && self.missing.is_empty()
    }
}

/// An obfuscated store rooted at a directory.
pub struct Hoard {
    dir: PathBuf,
    key: StoreKey,
}

impl Hoard {
    /// Open the hoard in `dir`. Nothing is read or written until asked.
    pub fn open(dir: impl Into<PathBuf>, key: StoreKey) -> Self {
        Self {
            dir: dir.into(),
            key,
        }
    }

    /// The filename a logical record lives under.
    ///
    /// Deterministic in the store key, which is what lets the program find a
    /// record without keeping an index that would give the game away.
    pub fn name_for(&self, logical: &str) -> Result<String, Error> {
        let mut out = [0u8; NAME_BYTES];
        self.key.expand(INFO_NAME, logical, &mut out)?;
        Ok(base64url(&out))
    }

    /// The full path of a logical record.
    pub fn path_for(&self, logical: &str) -> Result<PathBuf, Error> {
        Ok(self.dir.join(self.name_for(logical)?))
    }

    /// Encrypt and store `data` under `logical`, padded and named so that
    /// neither its content nor its purpose is visible from outside.
    ///
    /// The roster is updated so a later deletion of this record is detectable.
    pub fn write(&self, logical: &str, data: &[u8]) -> Result<(), Error> {
        self.write_raw(logical, data)?;
        if logical != ROSTER {
            let mut names = self.roster()?;
            if names.insert(logical.to_string()) {
                self.save_roster(&names)?;
            }
        }
        Ok(())
    }

    fn write_raw(&self, logical: &str, data: &[u8]) -> Result<(), Error> {
        let name = self.name_for(logical)?;
        let padded_len = bucket_for(LEN_PREFIX + data.len());
        let mut plain = vec![0u8; padded_len];
        let len = u32::try_from(data.len()).map_err(|_| Error::Encrypt)?;
        plain[..LEN_PREFIX].copy_from_slice(&len.to_le_bytes());
        plain[LEN_PREFIX..LEN_PREFIX + data.len()].copy_from_slice(data);
        fill_random(&mut plain[LEN_PREFIX + data.len()..])?;

        let mut record_key = Secret::zeroed(kdf::KEY_LEN);
        self.key
            .expand(INFO_REC, logical, record_key.expose_mut())?;
        let nonce = aead::random_nonce()?;
        let sealed = aead::seal(&record_key, &nonce, name.as_bytes(), &plain)?;

        let mut bytes = Vec::with_capacity(nonce.len() + sealed.len());
        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(&sealed);

        std::fs::create_dir_all(&self.dir).map_err(|_| Error::AppLockStore)?;
        crate::privatefile::write_owner_only(&self.dir.join(&name), &bytes)
            .map_err(|_| Error::AppLockStore)
    }

    /// Read a record back, or `None` if it was never written.
    ///
    /// A file that is present but does not authenticate returns
    /// [`Error::Decrypt`] rather than `None`: that is tampering, and it must
    /// not be reported as absence.
    pub fn read(&self, logical: &str) -> Result<Option<Vec<u8>>, Error> {
        let path = self.path_for(logical)?;
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::AppLockStore),
        };
        self.open_bytes(logical, &bytes).map(Some)
    }

    fn open_bytes(&self, logical: &str, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        if bytes.len() < aead::NONCE_LEN + aead::TAG_LEN + LEN_PREFIX {
            return Err(Error::Truncated);
        }
        let name = self.name_for(logical)?;
        let mut nonce = [0u8; aead::NONCE_LEN];
        nonce.copy_from_slice(&bytes[..aead::NONCE_LEN]);

        let mut record_key = Secret::zeroed(kdf::KEY_LEN);
        self.key
            .expand(INFO_REC, logical, record_key.expose_mut())?;
        let plain = aead::open(
            &record_key,
            &nonce,
            name.as_bytes(),
            &bytes[aead::NONCE_LEN..],
        )?;

        if plain.len() < LEN_PREFIX {
            return Err(Error::Truncated);
        }
        let len = u32::from_le_bytes([plain[0], plain[1], plain[2], plain[3]]) as usize;
        if LEN_PREFIX + len > plain.len() {
            return Err(Error::BadHeader);
        }
        Ok(plain[LEN_PREFIX..LEN_PREFIX + len].to_vec())
    }

    /// Remove a record and drop it from the roster.
    pub fn remove(&self, logical: &str) -> Result<(), Error> {
        let path = self.path_for(logical)?;
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::AppLockStore),
        }
        let mut names = self.roster()?;
        if names.remove(logical) {
            self.save_roster(&names)?;
        }
        Ok(())
    }

    /// The logical names the roster says should exist.
    pub fn roster(&self) -> Result<BTreeSet<String>, Error> {
        let path = self.path_for(ROSTER)?;
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
            Err(_) => return Err(Error::AppLockStore),
        };
        let plain = self.open_bytes(ROSTER, &bytes)?;
        let text = String::from_utf8(plain).map_err(|_| Error::BadHeader)?;
        Ok(text
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect())
    }

    fn save_roster(&self, names: &BTreeSet<String>) -> Result<(), Error> {
        let text = names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        self.write_raw(ROSTER, text.as_bytes())
    }

    /// Write decoy files: names of the same shape, contents of the same
    /// character, holding nothing.
    ///
    /// A decoy is random bytes under a random name. It is not a valid record
    /// under any logical name, so the program never mistakes one for data --
    /// it simply never derives that name. From outside there is nothing to
    /// separate the two, which is the point.
    ///
    /// Returns how many were written. Names that happen to collide with an
    /// existing file are skipped rather than overwritten.
    pub fn sow_decoys(&self, count: usize) -> Result<usize, Error> {
        std::fs::create_dir_all(&self.dir).map_err(|_| Error::AppLockStore)?;
        let mut written = 0;
        for _ in 0..count {
            let mut raw = [0u8; NAME_BYTES];
            fill_random(&mut raw)?;
            let name = base64url(&raw);
            let path = self.dir.join(&name);
            if path.exists() {
                continue;
            }
            let bucket = BUCKETS[(raw[0] as usize) % 3];
            let mut body = vec![0u8; aead::NONCE_LEN + bucket + aead::TAG_LEN];
            fill_random(&mut body)?;
            crate::privatefile::write_owner_only(&path, &body).map_err(|_| Error::AppLockStore)?;
            written += 1;
        }
        Ok(written)
    }

    /// Check every record the roster knows about, and count what else is here.
    ///
    /// This is the tamper report the user sees after unlocking. It can say a
    /// record was edited, and it can say a record the roster expected is gone.
    /// It cannot say anything about a folder somebody emptied entirely,
    /// including the roster, and does not try to.
    pub fn audit(&self) -> Result<Audit, Error> {
        let mut report = Audit::default();
        let roster = self.roster()?;

        let mut ours = BTreeSet::new();
        ours.insert(self.name_for(ROSTER)?);
        for logical in &roster {
            let name = self.name_for(logical)?;
            ours.insert(name.clone());
            match std::fs::read(self.dir.join(&name)) {
                Ok(bytes) => match self.open_bytes(logical, &bytes) {
                    Ok(_) => report.intact.push(logical.clone()),
                    Err(_) => report.tampered.push(logical.clone()),
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    report.missing.push(logical.clone())
                }
                Err(_) => return Err(Error::AppLockStore),
            }
        }

        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !ours.contains(&name) && is_hoard_shaped(&name) {
                    report.unrecognised += 1;
                }
            }
        }
        Ok(report)
    }
}

/// Whether a filename has the shape this module writes.
///
/// Used only to count decoys, never to decide what to open: a name that looks
/// right is still never read unless it is one the key derives.
fn is_hoard_shaped(name: &str) -> bool {
    name.len() == NAME_BYTES.div_ceil(3) * 4
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn fill_random(buf: &mut [u8]) -> Result<(), Error> {
    if buf.is_empty() {
        return Ok(());
    }
    getrandom::getrandom(buf).map_err(|_| Error::Random)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hoard(dir: &std::path::Path) -> Hoard {
        let mut raw = [7u8; kdf::KEY_LEN];
        Hoard::open(dir, StoreKey::from_secret(Secret::new(&mut raw)))
    }

    fn other_hoard(dir: &std::path::Path) -> Hoard {
        let mut raw = [9u8; kdf::KEY_LEN];
        Hoard::open(dir, StoreKey::from_secret(Secret::new(&mut raw)))
    }

    #[test]
    fn a_record_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        assert_eq!(h.read("settings").unwrap().unwrap(), b"theme=dark");
    }

    #[test]
    fn a_record_that_was_never_written_is_absent_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(hoard(dir.path()).read("nothing").unwrap(), None);
    }

    #[test]
    fn the_filename_gives_nothing_away() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        let name = h.name_for("settings").unwrap();
        assert_eq!(name.len(), 24);
        assert!(!name.contains("settings"));
        assert!(is_hoard_shaped(&name));
    }

    #[test]
    fn a_different_key_derives_a_different_name_for_the_same_record() {
        let dir = tempfile::tempdir().unwrap();
        assert_ne!(
            hoard(dir.path()).name_for("settings").unwrap(),
            other_hoard(dir.path()).name_for("settings").unwrap()
        );
    }

    #[test]
    fn another_key_cannot_read_what_this_one_wrote() {
        let dir = tempfile::tempdir().unwrap();
        hoard(dir.path()).write("settings", b"secret").unwrap();
        // It does not even find the file, which is the stronger statement.
        assert_eq!(other_hoard(dir.path()).read("settings").unwrap(), None);
    }

    #[test]
    fn contents_do_not_appear_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"needle-in-here").unwrap();
        let raw = std::fs::read(h.path_for("settings").unwrap()).unwrap();
        assert!(raw.windows(14).all(|w| w != b"needle-in-here"));
    }

    #[test]
    fn short_and_long_records_are_padded_to_the_same_few_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("a", b"x").unwrap();
        h.write("b", &[0u8; 200]).unwrap();
        let a = std::fs::metadata(h.path_for("a").unwrap()).unwrap().len();
        let b = std::fs::metadata(h.path_for("b").unwrap()).unwrap().len();
        assert_eq!(a, b, "both fall in the 256-byte bucket");
    }

    #[test]
    fn an_edited_record_is_refused_rather_than_returned() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        let path = h.path_for("settings").unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(h.read("settings"), Err(Error::Decrypt)));
    }

    #[test]
    fn two_records_cannot_be_swapped() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("first", b"one").unwrap();
        h.write("second", b"two").unwrap();
        let a = std::fs::read(h.path_for("first").unwrap()).unwrap();
        let b = std::fs::read(h.path_for("second").unwrap()).unwrap();
        std::fs::write(h.path_for("first").unwrap(), &b).unwrap();
        std::fs::write(h.path_for("second").unwrap(), &a).unwrap();
        // The name is authenticated, so a swap is caught rather than silently
        // returning the other record's contents.
        assert!(matches!(h.read("first"), Err(Error::Decrypt)));
        assert!(matches!(h.read("second"), Err(Error::Decrypt)));
    }

    #[test]
    fn audit_reports_an_edited_record() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        let path = h.path_for("settings").unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[aead::NONCE_LEN] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        let report = h.audit().unwrap();
        assert_eq!(report.tampered, vec!["settings".to_string()]);
        assert!(!report.is_clean());
    }

    #[test]
    fn audit_reports_a_deleted_record() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        std::fs::remove_file(h.path_for("settings").unwrap()).unwrap();
        let report = h.audit().unwrap();
        assert_eq!(report.missing, vec!["settings".to_string()]);
        assert!(!report.is_clean());
    }

    #[test]
    fn a_clean_folder_audits_clean() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        h.write("measured", b"3 runs").unwrap();
        let report = h.audit().unwrap();
        assert!(report.is_clean());
        assert_eq!(report.intact.len(), 2);
    }

    #[test]
    fn decoys_are_indistinguishable_from_records_by_shape() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        h.sow_decoys(20).unwrap();
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.len() >= 21);
        assert!(
            names.iter().all(|n| is_hoard_shaped(n)),
            "every file, real or decoy, has the same name shape"
        );
    }

    #[test]
    fn decoys_do_not_disturb_the_records_beside_them() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        h.sow_decoys(30).unwrap();
        assert_eq!(h.read("settings").unwrap().unwrap(), b"theme=dark");
        let report = h.audit().unwrap();
        assert!(report.is_clean());
        assert!(report.unrecognised >= 30, "decoys are counted, not opened");
    }

    #[test]
    fn removing_a_record_takes_it_out_of_the_roster() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"x").unwrap();
        h.remove("settings").unwrap();
        assert!(h.audit().unwrap().is_clean());
        assert_eq!(h.read("settings").unwrap(), None);
    }

    #[test]
    fn a_record_can_be_overwritten_without_growing_the_roster() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"one").unwrap();
        h.write("settings", b"two").unwrap();
        assert_eq!(h.read("settings").unwrap().unwrap(), b"two");
        assert_eq!(h.roster().unwrap().len(), 1);
    }

    #[test]
    fn base64url_matches_the_standard_alphabet() {
        assert_eq!(base64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64url(&[0xff, 0xff, 0xfe]), "___-");
        assert_eq!(base64url(b""), "");
    }

    #[test]
    fn buckets_round_up_and_never_shrink() {
        assert_eq!(bucket_for(1), 256);
        assert_eq!(bucket_for(256), 256);
        assert_eq!(bucket_for(257), 1024);
        assert_eq!(bucket_for(2_000_000), 2_097_152);
        for n in [0usize, 1, 255, 4095, 100_000, 5_000_000] {
            assert!(bucket_for(n) >= n);
        }
    }

    #[test]
    fn a_truncated_file_is_reported_not_returned() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        std::fs::write(h.path_for("settings").unwrap(), b"short").unwrap();
        assert!(matches!(h.read("settings"), Err(Error::Truncated)));
    }

    #[test]
    fn an_empty_record_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("empty", b"").unwrap();
        assert_eq!(h.read("empty").unwrap().unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn a_record_larger_than_every_bucket_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        let big = vec![3u8; 1_200_000];
        h.write("big", &big).unwrap();
        assert_eq!(h.read("big").unwrap().unwrap(), big);
    }
}
