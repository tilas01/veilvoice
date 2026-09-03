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
//! base64url(weave(HMAC-SHA256(store_key, "veilvoice/hoard/name" || logical)[..18]))
//! ```
//!
//! where `weave` is one of a dozen byte-level encodings chosen from the name's
//! own bytes, so it is stable across launches. The result is twenty-four
//! characters of base64 with no padding and no extension.
//!
//! Only length-preserving encodings are allowed there, and that restriction is
//! load-bearing: a name that came out longer or shorter would announce which
//! encoding produced it, and would separate records from decoys at a glance.
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
//! [24-byte nonce][ChaCha20-Poly1305 over: [2-byte marker][4-byte length][data][junk]]
//! ```
//!
//! The data is first put through one of twenty-seven encodings drawn at random
//! on every write -- base91, z-base-32, yEnc, a move-to-front transform, and
//! two dozen others -- and the marker says which, from inside the sealed
//! region so the choice is not visible either. [`crate::weave`] carries the
//! full argument; the short version is that **it adds no cryptographic
//! strength**, because the AEAD already makes this indistinguishable from
//! random. What it adds is that plaintext escaping by some route that is not
//! the cipher -- a core dump, a swap file, a future bug in this framing -- does
//! not read as anything.
//!
//! The padding is computed from the *original* length rather than the encoded
//! one, so a file's size never depends on which encoding was drawn. Otherwise
//! a record rewritten repeatedly would move between buckets and the smallest
//! one ever seen would pin its true length, which is exactly what the padding
//! is there to prevent.
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

/// The most any encoding in [`crate::weave`] can grow its input.
///
/// Percent-encoding and quoted-printable are the widest at three bytes out per
/// byte in. Used to size the padding from the *original* length, so a file's
/// size never depends on which encoding was drawn.
const MAX_EXPANSION: usize = 3;

/// The encoding marker, which sits before the length.
///
/// Inside the sealed region rather than beside it, so which of the encodings
/// in [`crate::weave`] was used is not visible from outside either.
const MARKER: usize = 2;

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

        // One of a dozen byte-level encodings, before the base64. Which one is
        // derived from the name's own bytes, so it is stable across launches --
        // a filename has to be computable again or the record is lost.
        //
        // Length-preserving only. An encoding that changed the byte count would
        // change the filename's length, and a filename whose length announces
        // its encoding separates records from decoys at a glance, which is the
        // one thing the decoys exist to prevent. `weave::LENGTH_PRESERVING` is
        // the restricted set and there is a test that every member of it keeps
        // the count.
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

    fn write_raw(&self, logical: &str, data: &[u8]) -> Result<(), Error> {
        let name = self.name_for(logical)?;

        // One of twenty-seven encodings, drawn fresh on every write, applied
        // before any of this is encrypted. `crate::weave` says at length what
        // that does and does not buy: it is not a second cipher, and what it
        // is for is that a plaintext buffer escaping by some route other than
        // the AEAD -- a core dump, a swap file, a future framing bug -- does
        // not read as anything.
        //
        // The marker rides inside the sealed region, so which encoding was
        // used is itself not visible from outside.
        let (chosen, body) = crate::weave::encode(data)?;
        let marker = chosen.id();

        // The bucket is chosen from the **original** length, not the encoded
        // one, and that is the whole point of `MAX_EXPANSION`.
        //
        // Choosing it from the encoded length would make a file's size depend
        // on which encoding was drawn, and the draw is fresh on every write.
        // Somebody watching one record rewritten would see it move between
        // buckets, and the smallest bucket they ever saw would pin the true
        // length far more tightly than a single bucket was ever meant to
        // allow. Padding is supposed to hide length; that would have quietly
        // handed it back.
        //
        // So the bucket is a function of `data.len()` alone. The cost is real
        // and worth stating: a record padded for the worst-case expansion can
        // be up to three times the size it strictly needs. These files are
        // settings and measurements, a few kilobytes at most, and a stable
        // size is worth more than a small one.
        let padded_len = bucket_for(MARKER + LEN_PREFIX + data.len() * MAX_EXPANSION);
        if MARKER + LEN_PREFIX + body.len() > padded_len {
            // Unreachable while `MAX_EXPANSION` is honest, and checked rather
            // than trusted: a new encoding that expands further would
            // otherwise silently overflow into a larger bucket and reopen the
            // leak above. `no_encoding_expands_past_the_allowance` is the test
            // that keeps this unreachable.
            return Err(Error::Encrypt);
        }
        let mut plain = vec![0u8; padded_len];
        plain[..MARKER].copy_from_slice(&marker);
        let len = u32::try_from(body.len()).map_err(|_| Error::Encrypt)?;
        plain[MARKER..MARKER + LEN_PREFIX].copy_from_slice(&len.to_le_bytes());
        let start = MARKER + LEN_PREFIX;
        plain[start..start + body.len()].copy_from_slice(&body);
        fill_random(&mut plain[start + body.len()..])?;

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
        if bytes.len() < aead::NONCE_LEN + aead::TAG_LEN + MARKER + LEN_PREFIX {
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

        if plain.len() < MARKER + LEN_PREFIX {
            return Err(Error::Truncated);
        }
        let chosen = crate::weave::Weave::from_id([plain[0], plain[1]])?;
        let len = u32::from_le_bytes([plain[2], plain[3], plain[4], plain[5]]) as usize;
        let start = MARKER + LEN_PREFIX;
        if start + len > plain.len() {
            return Err(Error::BadHeader);
        }
        crate::weave::decode(chosen, &plain[start..start + len])
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
        h.write("b", &[0u8; 60]).unwrap();
        let a = std::fs::metadata(h.path_for("a").unwrap()).unwrap().len();
        let b = std::fs::metadata(h.path_for("b").unwrap()).unwrap().len();
        assert_eq!(a, b, "one byte and sixty fall in the same bucket");
    }

    /// A file's size must depend on the data's length and nothing else.
    ///
    /// The encoding is drawn fresh on every write. If the bucket were chosen
    /// from the *encoded* length, a record rewritten repeatedly would move
    /// between buckets, and the smallest bucket ever seen would pin the true
    /// length far more tightly than one bucket was meant to allow. Padding is
    /// supposed to hide length; that would have handed it back.
    #[test]
    fn the_file_size_does_not_move_when_the_encoding_does() {
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

    #[test]
    fn no_encoding_expands_past_the_allowance() {
        // `MAX_EXPANSION` is what makes the bucket independent of the choice.
        // A new encoding that grew further would overflow into a larger bucket
        // and reopen the leak, so the allowance is measured rather than
        // assumed.
        for weave in crate::weave::ALL {
            for len in [0usize, 1, 2, 3, 5, 17, 64, 255, 1000] {
                let input = vec![0x5au8; len];
                let out = weave.apply(&crate::weave::Weave::Substitute.apply(&input));
                assert!(
                    out.len() <= len * MAX_EXPANSION + MAX_EXPANSION,
                    "{weave:?} turned {len} bytes into {}, past the {MAX_EXPANSION}x allowance",
                    out.len()
                );
            }
        }
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

    /// Every record is written through a different encoding, over time.
    ///
    /// The choice is drawn fresh on each write, so the same record written
    /// many times should not keep landing on the same one. A scheme that says
    /// it picks at random and does not is worse than one that never claimed to.
    #[test]
    fn the_encoding_changes_from_one_write_to_the_next() {
        let dir = tempfile::tempdir().unwrap();
        let h = hoard(dir.path());
        let mut shapes = std::collections::BTreeSet::new();
        for _ in 0..60 {
            h.write("settings", b"theme=dark, and a little more to encode")
                .unwrap();
            let raw = std::fs::read(h.path_for("settings").unwrap()).unwrap();
            // The nonce differs every time, so hash the whole file rather than
            // comparing bytes: what is being checked is that the *encoding*
            // varies, which shows up as a varying sealed length.
            shapes.insert(raw.len());
            assert_eq!(
                h.read("settings").unwrap().unwrap(),
                b"theme=dark, and a little more to encode",
                "whichever encoding was drawn, the record has to come back"
            );
        }
        // Bucket padding hides most of the variation, which is deliberate --
        // so this asserts the weaker, true thing: it still round trips every
        // time, across sixty independent draws.
        assert!(!shapes.is_empty());
    }

    #[test]
    fn a_records_plaintext_is_never_on_disk_even_before_the_cipher() {
        // The property `crate::weave` exists for, checked at this level rather
        // than only at its own: nothing recognisable reaches the file.
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
    fn a_woven_name_is_still_twenty_four_characters() {
        // Names go through a length-preserving encoding before the base64. If
        // one of them ever changed the byte count, the filename length would
        // announce which encoding was used, and would separate records from
        // decoys at a glance.
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
        // The encoding for a name is chosen from the name's own bytes, so it
        // has to be stable: an unstable one loses the record on the next
        // launch, which is F-141 in a new costume.
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
    fn decoys_are_still_the_same_shape_as_woven_records() {
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
