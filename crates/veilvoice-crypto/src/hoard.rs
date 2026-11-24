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
//! which is twenty-four characters of base64 with no padding and no
//! extension. Eighteen bytes rather than a round sixteen so the encoding comes
//! out exact: nothing to pad, which is one less thing to tell a name apart
//! from a decoy.
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
//! ["V" 2][24-byte nonce][ChaCha20-Poly1305 over: [4-byte length][data][junk]]
//! ```
//!
//! The first two bytes say which format the rest is in. **F-337.** Records
//! used to be put through one of thirty-one reversible encodings before the
//! cipher and another after it, and their names through one more. None of it
//! added strength: the AEAD already makes a record indistinguishable from
//! random, and the encodings are published here, so anybody who can read the
//! source can undo them. What they did cost was compute and room, since every
//! record was padded for the widest encoding at eight times its length. New
//! records are written without them. [`crate::weave`] is kept only to open a
//! record an older VeilVoice wrote, and that record is rewritten in the current
//! format, under its current name, the first time it is read.
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

/// The first two bytes of every record this version writes.
///
/// Outside the seal, and saying nothing secret: which format the rest is in.
/// Its first byte is one no encoding in [`crate::weave`] ever used as its
/// marker, so a record from before F-337 is never taken for a current one.
const FORMAT: [u8; 2] = [b'V', 2];

/// The length of [`FORMAT`], which is also the length of the outer marker the
/// older format began with.
const OUTER_MARKER: usize = 2;

/// The older format's inner marker, which sat inside the sealed region before
/// the length.
const LEGACY_MARKER: usize = 2;

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
    // Saturating for the same reason the caller checks its multiplication: a
    // length near the top of the address space would otherwise round up to a
    // smaller number than it started at.
    len.div_ceil(1_048_576).saturating_mul(1_048_576)
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

    /// The name an older VeilVoice gave the same record: the same HMAC put
    /// through one of the length-preserving encodings in [`crate::weave`].
    ///
    /// Only ever used to find a record written before F-337 so it can be read
    /// and moved to [`Hoard::name_for`]. For some records the encoding drawn
    /// was the identity, and then the two names are the same.
    fn legacy_name_for(&self, logical: &str) -> Result<String, Error> {
        let mut out = [0u8; NAME_BYTES];
        self.key.expand(INFO_NAME, logical, &mut out)?;
        let woven = crate::weave::Weave::for_name(&out).apply(&out);
        Ok(base64url(&woven))
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

    /// Seal and write one record under its name, in the current format.
    ///
    /// What is written is not the caller's bytes, is not stored under the
    /// caller's name, and is not the size of the caller's data: it is sealed,
    /// named by the key, and padded to a bucket.
    fn write_raw(&self, logical: &str, data: &[u8]) -> Result<(), Error> {
        let name = self.name_for(logical)?;

        // The bucket comes from the data's length alone, so a file's size says
        // which bucket it fell in and nothing finer. Checked, because this is a
        // length from a caller and the project ships 32-bit builds.
        let padded_len = data
            .len()
            .checked_add(LEN_PREFIX)
            .map(bucket_for)
            .ok_or(Error::Encrypt)?;
        let len = u32::try_from(data.len()).map_err(|_| Error::Encrypt)?;
        let mut plain = vec![0u8; padded_len];
        plain[..LEN_PREFIX].copy_from_slice(&len.to_le_bytes());
        plain[LEN_PREFIX..LEN_PREFIX + data.len()].copy_from_slice(data);
        fill_random(&mut plain[LEN_PREFIX + data.len()..])?;

        let mut record_key = Secret::zeroed(kdf::KEY_LEN);
        self.key
            .expand(INFO_REC, logical, record_key.expose_mut())?;
        let nonce = aead::random_nonce()?;
        let sealed = aead::seal(&record_key, &nonce, name.as_bytes(), &plain);
        // The record in the clear, until a moment ago.
        {
            use zeroize::Zeroize;
            plain.zeroize();
        }
        let sealed = sealed?;

        let mut bytes = Vec::with_capacity(OUTER_MARKER + nonce.len() + sealed.len());
        bytes.extend_from_slice(&FORMAT);
        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(&sealed);

        std::fs::create_dir_all(&self.dir).map_err(|_| Error::AppLockStore)?;
        // Replaced in one step, so a crash part-way leaves the old record. F-335.
        crate::privatefile::replace_owner_only(&self.dir.join(&name), &bytes)
            .map_err(|_| Error::AppLockStore)?;

        // A copy an older VeilVoice wrote under its old name is stale now.
        let legacy = self.legacy_name_for(logical)?;
        if legacy != name {
            remove_if_present(&self.dir.join(legacy))?;
        }
        Ok(())
    }

    /// Find and open a record, in whichever format it was written.
    ///
    /// The current name first, then the name an older VeilVoice used. The flag
    /// is true when the record is in the older format, wherever it was found,
    /// so the caller can move it.
    fn fetch(&self, logical: &str) -> Result<Option<(Vec<u8>, bool)>, Error> {
        let name = self.name_for(logical)?;
        if let Some(bytes) = read_if_present(&self.dir.join(&name))? {
            let older = bytes.get(..OUTER_MARKER) != Some(&FORMAT[..]);
            return self
                .open_bytes(logical, &name, &bytes)
                .map(|data| Some((data, older)));
        }
        let legacy = self.legacy_name_for(logical)?;
        if legacy == name {
            return Ok(None);
        }
        match read_if_present(&self.dir.join(&legacy))? {
            Some(bytes) => self
                .open_bytes(logical, &legacy, &bytes)
                .map(|data| Some((data, true))),
            None => Ok(None),
        }
    }

    /// Read a record back, or `None` if it was never written.
    ///
    /// A file that is present but does not authenticate returns
    /// [`Error::Decrypt`] rather than `None`: that is tampering, and it must
    /// not be reported as absence. A record in the older format is rewritten
    /// in the current one as it is read.
    pub fn read(&self, logical: &str) -> Result<Option<Vec<u8>>, Error> {
        match self.fetch(logical)? {
            Some((data, true)) => {
                self.write_raw(logical, &data)?;
                Ok(Some(data))
            }
            Some((data, false)) => Ok(Some(data)),
            None => Ok(None),
        }
    }

    /// Undo [`Hoard::write_raw`] for bytes already read off the disk, which
    /// were stored under `name`.
    ///
    /// Split out from the read so that a record can be opened from memory, which
    /// is what the tests do rather than going through the filesystem.
    fn open_bytes(&self, logical: &str, name: &str, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        if bytes.len() < OUTER_MARKER + aead::NONCE_LEN + aead::TAG_LEN + LEN_PREFIX {
            return Err(Error::Truncated);
        }
        if bytes[..OUTER_MARKER] != FORMAT {
            return self.open_legacy(logical, name, bytes);
        }
        let plain = self.unseal(logical, name, &bytes[OUTER_MARKER..])?;
        let len = u32::from_le_bytes([plain[0], plain[1], plain[2], plain[3]]) as usize;
        // Compared without adding: `len` comes from the record, and on the
        // 32-bit ARM build, where release builds do not check for overflow,
        // `LEN_PREFIX + len` could wrap. `plain` is at least `LEN_PREFIX` long,
        // because the length check above leaves room for it.
        if len > plain.len() - LEN_PREFIX {
            return Err(Error::BadHeader);
        }
        Ok(plain[LEN_PREFIX..LEN_PREFIX + len].to_vec())
    }

    /// Open the AEAD around a record: the nonce, then the sealed body, with the
    /// file's own name as the additional data.
    fn unseal(&self, logical: &str, name: &str, blob: &[u8]) -> Result<Vec<u8>, Error> {
        if blob.len() < aead::NONCE_LEN + aead::TAG_LEN {
            return Err(Error::Truncated);
        }
        let mut nonce = [0u8; aead::NONCE_LEN];
        nonce.copy_from_slice(&blob[..aead::NONCE_LEN]);
        let mut record_key = Secret::zeroed(kdf::KEY_LEN);
        self.key
            .expand(INFO_REC, logical, record_key.expose_mut())?;
        aead::open(
            &record_key,
            &nonce,
            name.as_bytes(),
            &blob[aead::NONCE_LEN..],
        )
    }

    /// Open a record in the format VeilVoice wrote before F-337: an outer
    /// encoding around the sealed blob, and an inner one around the data.
    fn open_legacy(&self, logical: &str, name: &str, bytes: &[u8]) -> Result<Vec<u8>, Error> {
        if bytes.len() < OUTER_MARKER + aead::NONCE_LEN + aead::TAG_LEN + LEGACY_MARKER + LEN_PREFIX
        {
            return Err(Error::Truncated);
        }
        let outer = crate::weave::Weave::from_id([bytes[0], bytes[1]])?;
        let blob = outer.undo(&bytes[OUTER_MARKER..])?;
        let plain = self.unseal(logical, name, &blob)?;
        if plain.len() < LEGACY_MARKER + LEN_PREFIX {
            return Err(Error::Truncated);
        }
        let chosen = crate::weave::Weave::from_id([plain[0], plain[1]])?;
        let len = u32::from_le_bytes([plain[2], plain[3], plain[4], plain[5]]) as usize;
        let start = LEGACY_MARKER + LEN_PREFIX;
        if len > plain.len() - start {
            return Err(Error::BadHeader);
        }
        crate::weave::decode(chosen, &plain[start..start + len])
    }

    /// Remove a record, under either name it may have, and drop it from the
    /// roster.
    pub fn remove(&self, logical: &str) -> Result<(), Error> {
        remove_if_present(&self.dir.join(self.name_for(logical)?))?;
        remove_if_present(&self.dir.join(self.legacy_name_for(logical)?))?;
        let mut names = self.roster()?;
        if names.remove(logical) {
            self.save_roster(&names)?;
        }
        Ok(())
    }

    /// The logical names the roster says should exist.
    ///
    /// A roster in the older format is rewritten in the current one as it is
    /// read, like any other record.
    pub fn roster(&self) -> Result<BTreeSet<String>, Error> {
        let Some((plain, older)) = self.fetch(ROSTER)? else {
            return Ok(BTreeSet::new());
        };
        let text = String::from_utf8(plain).map_err(|_| Error::BadHeader)?;
        let names: BTreeSet<String> = text
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
        if older {
            self.save_roster(&names)?;
        }
        Ok(names)
    }

    /// Write the list of logical names, itself as an ordinary record.
    ///
    /// The roster goes through the same sealing and padding as anything else,
    /// so the file that says what is stored is not distinguishable from the
    /// files it names.
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
            // The same on-disk shape as a real record: the two-byte outer
            // marker, the nonce, a bucket of body, and the tag. A decoy that
            // was two bytes shorter than every real record would be a decoy an
            // observer could sort out at a glance, which is the one thing the
            // decoys exist to prevent.
            let bucket = BUCKETS[(raw[0] as usize) % 3];
            let mut body = vec![0u8; OUTER_MARKER + aead::NONCE_LEN + bucket + aead::TAG_LEN];
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

        // Both names count as ours: a record an older version wrote is not a
        // stranger in the folder before it has been moved.
        let mut ours = BTreeSet::new();
        for logical in std::iter::once(ROSTER).chain(roster.iter().map(String::as_str)) {
            ours.insert(self.name_for(logical)?);
            ours.insert(self.legacy_name_for(logical)?);
        }
        for logical in &roster {
            match self.fetch(logical) {
                Ok(Some(_)) => report.intact.push(logical.clone()),
                Ok(None) => report.missing.push(logical.clone()),
                Err(Error::AppLockStore) => return Err(Error::AppLockStore),
                Err(_) => report.tampered.push(logical.clone()),
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

/// The file's bytes, `None` if there is no such file, and an error for any
/// other reason it cannot be read.
fn read_if_present(path: &std::path::Path) -> Result<Option<Vec<u8>>, Error> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::AppLockStore),
    }
}

/// Remove a file, treating one that is already gone as removed.
fn remove_if_present(path: &std::path::Path) -> Result<(), Error> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Error::AppLockStore),
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

/// Fill `buf` from the operating system, treating a refusal as an error
/// rather than falling back to anything.
///
/// An empty buffer is a no-op: `getrandom` is within its rights to refuse a
/// zero-length request, and asking for nothing is not a failure.
fn fill_random(buf: &mut [u8]) -> Result<(), Error> {
    if buf.is_empty() {
        return Ok(());
    }
    getrandom::getrandom(buf).map_err(|_| Error::Random)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sizing the padded buffer cannot wrap on a 32-bit target.
    #[test]
    fn the_padding_size_is_refused_rather_than_wrapping() {
        assert!(
            usize::MAX.checked_add(LEN_PREFIX).is_none(),
            "this length is meant to overflow the addition"
        );
        for len in [0usize, 1, 100, 4096, 1_000_000] {
            let padded = len
                .checked_add(LEN_PREFIX)
                .map(bucket_for)
                .expect("an ordinary record size must not overflow");
            assert!(
                padded >= LEN_PREFIX + len,
                "a {len} byte record was padded to {padded}, which cannot hold it"
            );
        }
        // `bucket_for`'s own rounding saturates rather than wrapping to zero.
        assert!(bucket_for(usize::MAX) >= usize::MAX - 1_048_576);
    }

    fn hoard(dir: &std::path::Path) -> Hoard {
        let mut raw = [7u8; kdf::KEY_LEN];
        Hoard::open(dir, StoreKey::from_secret(Secret::new(&mut raw)))
    }

    fn other_hoard(dir: &std::path::Path) -> Hoard {
        let mut raw = [9u8; kdf::KEY_LEN];
        Hoard::open(dir, StoreKey::from_secret(Secret::new(&mut raw)))
    }

    /// **F-335.** Writing a record that is already there replaces it by a
    /// rename, so a crash part-way cannot leave half of one.
    #[cfg(unix)]
    #[test]
    fn rewriting_a_record_replaces_it_rather_than_rewriting_it() {
        use std::os::unix::fs::MetadataExt;
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"the first").unwrap();
        let path = h.path_for("settings").unwrap();
        let before = std::fs::metadata(&path).unwrap().ino();
        h.write("settings", b"the second").unwrap();
        assert_ne!(std::fs::metadata(&path).unwrap().ino(), before);
        assert_eq!(
            h.read("settings").unwrap().as_deref(),
            Some(&b"the second"[..])
        );
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
        h.write("b", &[0u8; 20]).unwrap();
        let a = std::fs::metadata(h.path_for("a").unwrap()).unwrap().len();
        let b = std::fs::metadata(h.path_for("b").unwrap()).unwrap().len();
        assert_eq!(a, b, "one byte and twenty fall in the same bucket");
    }

    /// A file's size depends on the data's length and nothing else, however
    /// many times the record is rewritten.
    #[test]
    fn the_file_size_does_not_move_between_writes() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        let data = vec![0x41u8; 200];
        let mut sizes = std::collections::BTreeSet::new();
        for _ in 0..80 {
            h.write("settings", &data).unwrap();
            sizes.insert(
                std::fs::metadata(h.path_for("settings").unwrap())
                    .unwrap()
                    .len(),
            );
        }
        assert_eq!(
            sizes.len(),
            1,
            "eighty writes of one record produced {} different sizes: {sizes:?}",
            sizes.len()
        );
    }

    /// **F-337.** A record is padded for its own length, not for the widest
    /// of the encodings the old format could draw.
    #[test]
    fn a_small_record_is_padded_to_the_smallest_bucket() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", &[0x41u8; 200]).unwrap();
        let size = std::fs::metadata(h.path_for("settings").unwrap())
            .unwrap()
            .len();
        let expected = OUTER_MARKER + aead::NONCE_LEN + BUCKETS[0] + aead::TAG_LEN;
        assert_eq!(size, expected as u64);
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

    /// A record the way VeilVoice wrote one before F-337: encoded, sealed,
    /// encoded again, and stored under the woven name.
    fn write_legacy(h: &Hoard, logical: &str, data: &[u8]) {
        let name = h.legacy_name_for(logical).unwrap();
        let (chosen, body) = crate::weave::encode(data).unwrap();
        let padded_len = bucket_for(data.len() * 8 + LEGACY_MARKER + LEN_PREFIX);
        let mut plain = vec![0u8; padded_len];
        plain[..LEGACY_MARKER].copy_from_slice(&chosen.id());
        plain[LEGACY_MARKER..LEGACY_MARKER + LEN_PREFIX]
            .copy_from_slice(&(body.len() as u32).to_le_bytes());
        let start = LEGACY_MARKER + LEN_PREFIX;
        plain[start..start + body.len()].copy_from_slice(&body);
        let mut key = Secret::zeroed(kdf::KEY_LEN);
        h.key.expand(INFO_REC, logical, key.expose_mut()).unwrap();
        let nonce = aead::random_nonce().unwrap();
        let sealed = aead::seal(&key, &nonce, name.as_bytes(), &plain).unwrap();
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&sealed);
        let outer = crate::weave::Weave::random_length_preserving().unwrap();
        let mut bytes = outer.id().to_vec();
        bytes.extend_from_slice(&outer.apply(&blob));
        std::fs::create_dir_all(dir_of(h)).unwrap();
        std::fs::write(dir_of(h).join(&name), bytes).unwrap();
    }

    fn dir_of(h: &Hoard) -> &std::path::Path {
        &h.dir
    }

    /// **F-337.** A record an older VeilVoice wrote still opens, and is moved
    /// to the current format and name the first time it is read.
    #[test]
    fn a_record_in_the_old_format_opens_and_moves_to_the_new_one() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        for round in 0..20 {
            let logical = format!("settings {round}");
            write_legacy(&h, &logical, b"theme=dark");
            assert_eq!(h.read(&logical).unwrap().unwrap(), b"theme=dark");
            let now = std::fs::read(h.path_for(&logical).unwrap()).unwrap();
            assert_eq!(&now[..OUTER_MARKER], &FORMAT, "it was not moved");
            let legacy = h.legacy_name_for(&logical).unwrap();
            if legacy != h.name_for(&logical).unwrap() {
                assert!(!dir.path().join(legacy).exists(), "the old copy was left");
            }
            assert_eq!(h.read(&logical).unwrap().unwrap(), b"theme=dark");
        }
    }

    /// **F-337.** A folder an older VeilVoice wrote, roster and all, audits
    /// clean, and its old files are not counted as strangers.
    #[test]
    fn a_folder_in_the_old_format_audits_clean() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        write_legacy(&h, "measured", b"3 runs");
        write_legacy(&h, ROSTER, b"measured");
        let report = h.audit().unwrap();
        assert!(report.is_clean(), "{report:?}");
        assert_eq!(report.intact, vec!["measured".to_string()]);
        assert_eq!(report.unrecognised, 0);
        assert_eq!(h.read("measured").unwrap().unwrap(), b"3 runs");
    }

    /// The current format's marker can never be read as an old one.
    #[test]
    fn the_format_marker_is_no_old_encoding() {
        assert!(crate::weave::Weave::from_id(FORMAT).is_err());
    }

    #[test]
    fn a_records_plaintext_is_never_on_disk_even_before_the_cipher() {
        // Nothing recognisable reaches the file: the AEAD covers it.
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        for _ in 0..40 {
            h.write("settings", b"passphrase = hunter2").unwrap();
            let raw = std::fs::read(h.path_for("settings").unwrap()).unwrap();
            for needle in [&b"passphrase"[..], &b"hunter2"[..]] {
                assert!(
                    !raw.windows(needle.len()).any(|w| w == needle),
                    "{:?} reached the disk",
                    String::from_utf8_lossy(needle)
                );
            }
        }
    }

    #[test]
    fn a_name_is_always_twenty_four_characters() {
        // A filename whose length varied would separate records from decoys at
        // a glance.
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        for logical in [
            "settings",
            "measured",
            "a",
            "",
            "an unusually long logical name",
            "\u{0}roster",
        ] {
            let name = h.name_for(logical).unwrap();
            assert_eq!(name.len(), 24, "{logical:?} produced {name:?}");
            assert!(is_hoard_shaped(&name));
        }
    }

    #[test]
    fn a_name_is_the_same_every_time_it_is_derived() {
        // A name has to be the same every time it is derived: an unstable one
        // loses the record on the next launch, which is F-141 in a new costume.
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        let first = h.name_for("settings").unwrap();
        for _ in 0..20 {
            assert_eq!(h.name_for("settings").unwrap(), first);
        }
        // And across a freshly opened store with the same key.
        assert_eq!(hoard(dir.path()).name_for("settings").unwrap(), first);
    }

    #[test]
    fn decoys_and_records_share_one_name_shape() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"theme=dark").unwrap();
        h.write("measured", b"3 runs").unwrap();
        h.sow_decoys(30).unwrap();
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| n.len() == 24 && is_hoard_shaped(n)));
        assert!(h.audit().unwrap().is_clean());
    }

    #[test]
    fn the_format_marker_is_the_first_two_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", b"some bytes to seal").unwrap();
        let raw = std::fs::read(h.path_for("settings").unwrap()).unwrap();
        assert_eq!(&raw[..OUTER_MARKER], &FORMAT);
    }

    #[test]
    fn nothing_on_disk_is_a_long_run_of_one_byte() {
        // A well-formed record is sealed ciphertext with a fixed-size frame:
        // no long constant runs, even from all-zero input. This checks the
        // padding and the framing do not accidentally introduce one,
        // which would be a framing bug worth catching regardless.
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        h.write("settings", &[0u8; 300]).unwrap();
        let raw = std::fs::read(h.path_for("settings").unwrap()).unwrap();
        let mut longest = 1;
        let mut run = 1;
        for w in raw.windows(2) {
            if w[0] == w[1] {
                run += 1;
                longest = longest.max(run);
            } else {
                run = 1;
            }
        }
        assert!(
            longest < 16,
            "a run of {longest} identical bytes reached the disk from all-zero input"
        );
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
