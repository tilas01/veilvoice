// SPDX-License-Identifier: GPL-3.0-or-later
//! The studio vault: a key that exists only when both locks have been opened.
//!
//! # The one thing this adds
//!
//! Everything else in this crate is protected by one secret. The app lock
//! guards the window; a sealed recording is opened by its own passphrase. Each
//! is a single point: whoever has that one secret has the thing it guards.
//!
//! A [`StudioKey`] is derived from **both**, and from neither alone. A laptop
//! stolen with VeilVoice already unlocked opens nothing here, because the
//! at-rest passphrase was never entered. An at-rest passphrase learned by any
//! means opens nothing here either, because it is not the app lock. Both, at
//! the same time, on the same machine, or the vault stays shut.
//!
//! # How the two are combined
//!
//! Not concatenated, and not one encrypting the other. Both secrets go into
//! HKDF-SHA256 as input keying material, under a salt that names this vault and
//! its version, and the output is the vault key:
//!
//! ```text
//! ikm  = app_lock_key || at_rest_key
//! salt = "veilvoice/studio-vault/v1"
//! key  = HKDF-SHA256(ikm, salt, info)
//! ```
//!
//! HKDF-Extract mixes the whole of the input, so an attacker holding one half
//! and guessing the other faces the full cost of the half they are guessing.
//! Concatenating the two *ciphertexts* instead, or encrypting once with each
//! key in turn, would let each layer be attacked separately, which is the
//! mistake this shape exists to avoid.
//!
//! The length of each half is bound into the info string. Without that,
//! `("ab", "c")` and `("a", "bc")` would produce the same input keying material
//! and therefore the same vault key, which is a collision an attacker chooses
//! rather than finds.
//!
//! # What it is worth, and what it is not
//!
//! It raises the cost of a stolen machine and of a leaked passphrase, and it
//! turns one compromise into two. Both of those are real.
//!
//! It does **not** defeat somebody who is watching this process while both
//! secrets are entered: at that moment the derived key exists in memory, and
//! this crate has never claimed to beat an attacker who is already inside the
//! process. It is page-locked and zeroized like every other secret here, which
//! narrows the window and does not close it. The vault is a second lock on the
//! door, not a guard in the room.
//!
//! # In plain words
//!
//! The recordings the studio makes are locked with a key made out of *two* of
//! your passwords at once. Somebody who learns one of them still cannot open
//! them, and neither can somebody who walks off with the computer while the app
//! is open.
//!
//! What it cannot do is protect you from something already running inside
//! VeilVoice at the moment you type both.

use crate::{Error, Secret};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroize;

/// Bytes in a studio vault key.
pub const KEY_LEN: usize = 32;

/// Names this construction and its version in the HKDF salt.
///
/// Versioned so that changing how the two halves are combined produces a
/// different key rather than silently reinterpreting an existing vault.
const SALT: &[u8] = b"veilvoice/studio-vault/v1";

/// The HKDF info label.
const INFO: &[u8] = b"studio vault key";

/// A key that exists only while both locks are open.
///
/// No `Debug`, no `Clone`, and no way to read the bytes out except
/// [`StudioKey::expose`], which the sealing code needs. Wiped when dropped,
/// because the [`Secret`] inside it is.
pub struct StudioKey(Secret);

impl StudioKey {
    /// Derive the vault key from both secrets.
    ///
    /// `app_lock` is the key material the app lock produced when the window was
    /// unlocked; `at_rest` is the key material the recording passphrase
    /// produced. Both are required, and an empty one is refused rather than
    /// treated as "no second factor": a vault that quietly degraded to one
    /// secret when the other was missing would be the exact failure this type
    /// exists to prevent, and it would do it silently.
    pub fn derive(app_lock: &Secret, at_rest: &Secret) -> Result<Self, Error> {
        if app_lock.is_empty() || at_rest.is_empty() {
            return Err(Error::StudioNeedsBoth);
        }

        // Exact capacity, so the Vec never reallocates and never leaves a copy
        // of half the key material behind on the heap.
        let mut ikm = Vec::with_capacity(app_lock.len() + at_rest.len());
        ikm.extend_from_slice(app_lock.expose());
        ikm.extend_from_slice(at_rest.expose());

        // The split point, bound into the info. Without it, moving one byte
        // from the end of the first secret to the start of the second gives the
        // same concatenation and therefore the same key.
        let mut info = Vec::with_capacity(INFO.len() + 8);
        info.extend_from_slice(INFO);
        info.extend_from_slice(&(app_lock.len() as u64).to_le_bytes());

        let hk = Hkdf::<Sha256>::new(Some(SALT), &ikm);
        let mut key = Secret::zeroed(KEY_LEN);
        let result = hk.expand(&info, key.expose_mut()).map_err(|_| Error::Kdf);

        // Wiped whether or not the expansion succeeded: the failure path still
        // had both secrets in this buffer.
        ikm.zeroize();
        result?;
        Ok(Self(key))
    }

    /// Borrow the key bytes, for sealing and opening the vault.
    pub fn expose(&self) -> &[u8] {
        self.0.expose()
    }

    /// Whether the operating system agreed to keep this key out of swap.
    ///
    /// Reported rather than assumed, exactly as [`crate::amnesia`] and
    /// [`crate::tape`] report it: locking is best effort, and a caller telling
    /// somebody their vault key is unswappable should be saying what actually
    /// happened.
    pub fn is_locked(&self) -> bool {
        self.0.is_locked()
    }
}

/// One recording in the vault.
///
/// Metadata only: what it is, when it was made and how large. The audio is
/// never in here, so listing a vault does not decrypt any of it.
#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    /// Opaque identifier, and the name of the file on disk.
    pub id: String,
    /// What the person called it.
    pub name: String,
    /// Unix seconds when it was sealed.
    pub made: i64,
    /// Bytes of audio inside, before sealing.
    pub bytes: usize,
}

/// Deliberately opaque: an entry names a recording somebody made, and a name
/// like "meeting with the lawyer" reaching a log is the sort of leak this
/// project is otherwise careful about.
impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Entry({}, {} bytes, redacted name)", self.id, self.bytes)
    }
}

/// A directory of recordings, sealed under a [`StudioKey`].
///
/// # What is on disk, and what it gives away
///
/// One file per recording, named by an identifier that says nothing, plus one
/// index file. The index holds every name and date and is itself sealed under
/// the same key, so a vault sitting on a disk shows how many recordings there
/// are and roughly how large each is, and nothing else. Those two facts are not
/// hidden and the documentation says so rather than implying otherwise: hiding
/// them means padding and decoys, which is what [`crate::hoard`] is for and is
/// a different trade.
pub struct Studio {
    dir: std::path::PathBuf,
    key: StudioKey,
}

/// The index file's name. Fixed rather than derived: a vault whose index cannot
/// be found is a vault nothing can open, and the directory already discloses
/// that it is a vault by existing.
const INDEX: &str = "index.veil";

impl Studio {
    /// Open the vault in `dir`, creating the directory if it is not there.
    pub fn open(dir: impl Into<std::path::PathBuf>, key: StudioKey) -> Result<Self, Error> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).map_err(|_| Error::AppLockStore)?;
        Ok(Self { dir, key })
    }

    /// Where the vault lives.
    pub fn dir(&self) -> &std::path::Path {
        &self.dir
    }

    /// The vault key as a [`Secret`], for the AEAD.
    ///
    /// A fresh copy each time, taken into locked memory and wiped when it goes
    /// out of scope, so the working copy never outlives the call that needed
    /// it. `Secret::new` wipes the intermediate `Vec` as it takes ownership.
    fn secret_key(&self) -> Secret {
        let mut bytes = self.key.expose().to_vec();
        Secret::new(&mut bytes)
    }

    /// Every recording in the vault, oldest first.
    ///
    /// An index that will not open is an error rather than an empty list. A
    /// vault that quietly reports "no recordings" when the truth is "the key is
    /// wrong, or this has been tampered with" would be the worst possible
    /// answer: it reads as reassurance.
    pub fn list(&self) -> Result<Vec<Entry>, Error> {
        let path = self.dir.join(INDEX);
        let sealed = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            // Genuinely nothing here yet, which is not the same as unreadable.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(Error::AppLockStore),
        };
        let plain = self.unseal(&sealed, b"veilvoice/studio/index")?;
        let text = String::from_utf8(plain).map_err(|_| Error::BadHeader)?;
        parse_index(&text)
    }

    /// Seal `wav` into the vault under `name`, returning its entry.
    pub fn store(&self, name: &str, made: i64, wav: &[u8]) -> Result<Entry, Error> {
        let id = new_id()?;
        let entry = Entry {
            id: id.clone(),
            name: name.to_string(),
            made,
            bytes: wav.len(),
        };

        // The recording first. An index naming a file that does not exist is a
        // worse state than a file no index mentions: the first looks like loss,
        // the second is recoverable and is what an interrupted store leaves.
        let sealed = self.seal(wav, id.as_bytes())?;
        crate::privatefile::write_owner_only(&self.dir.join(&id), &sealed)
            .map_err(|_| Error::AppLockStore)?;

        let mut all = self.list()?;
        all.push(entry.clone());
        self.write_index(&all)?;
        Ok(entry)
    }

    /// Open one recording into locked memory.
    ///
    /// Returns a [`Secret`], not a `Vec`: this is the audio in the clear, and
    /// the whole vault exists so that it is never anywhere unprotected. A
    /// caller playing it back reads from here and does not write it to a
    /// temporary file, because a temporary file is the thing the vault was
    /// avoiding.
    pub fn load(&self, id: &str) -> Result<Secret, Error> {
        if !safe_id(id) {
            return Err(Error::BadHeader);
        }
        let sealed = std::fs::read(self.dir.join(id)).map_err(|_| Error::AppLockStore)?;
        let plain = self.unseal(&sealed, id.as_bytes())?;
        let mut plain = plain;
        let secret = Secret::new(&mut plain);
        Ok(secret)
    }

    /// Remove one recording and its index entry.
    pub fn remove(&self, id: &str) -> Result<(), Error> {
        if !safe_id(id) {
            return Err(Error::BadHeader);
        }
        let remaining: Vec<Entry> = self.list()?.into_iter().filter(|e| e.id != id).collect();
        self.write_index(&remaining)?;
        // The index is written first here, the opposite order from `store`: an
        // interrupted removal must not leave the index pointing at a file that
        // has gone.
        let _ = std::fs::remove_file(self.dir.join(id));
        Ok(())
    }

    fn write_index(&self, entries: &[Entry]) -> Result<(), Error> {
        let text = render_index(entries);
        let sealed = self.seal(text.as_bytes(), b"veilvoice/studio/index")?;
        crate::privatefile::replace_owner_only(&self.dir.join(INDEX), &sealed)
            .map_err(|_| Error::AppLockStore)
    }

    /// Seal with a fresh nonce, binding `aad` so a file cannot be moved to
    /// another identity inside the same vault and still open.
    fn seal(&self, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>, Error> {
        let nonce = crate::aead::random_nonce()?;
        let key = self.secret_key();
        let mut out = Vec::with_capacity(nonce.len() + plain.len() + crate::aead::TAG_LEN);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&crate::aead::seal(&key, &nonce, aad, plain)?);
        Ok(out)
    }

    fn unseal(&self, sealed: &[u8], aad: &[u8]) -> Result<Vec<u8>, Error> {
        if sealed.len() < crate::aead::NONCE_LEN {
            return Err(Error::Truncated);
        }
        let mut nonce = [0u8; crate::aead::NONCE_LEN];
        nonce.copy_from_slice(&sealed[..crate::aead::NONCE_LEN]);
        let key = self.secret_key();
        crate::aead::open(&key, &nonce, aad, &sealed[crate::aead::NONCE_LEN..])
    }
}

/// A random, opaque identifier: 32 base32-ish characters that say nothing.
fn new_id() -> Result<String, Error> {
    let mut raw = [0u8; 20];
    getrandom::getrandom(&mut raw).map_err(|_| Error::Random)?;
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    Ok(raw
        .iter()
        .map(|b| ALPHABET[(b & 31) as usize] as char)
        .collect())
}

/// Whether `id` is one this vault could have produced.
///
/// Checked before it reaches a path. An identifier read back out of an index is
/// data, and an index is a file somebody could have edited: without this, an id
/// of `../../.bashrc` would send a read or a delete somewhere else entirely.
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// The index, as lines. Tab-separated because a name may contain almost
/// anything except a tab or a newline, and both are rejected on the way in.
fn render_index(entries: &[Entry]) -> String {
    let mut out = String::new();
    for e in entries {
        let name = e.name.replace(['\t', '\n', '\r'], " ");
        out.push_str(&format!("{}\t{}\t{}\t{}\n", e.id, e.made, e.bytes, name));
    }
    out
}

fn parse_index(text: &str) -> Result<Vec<Entry>, Error> {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(4, '\t');
        let id = parts.next().unwrap_or_default();
        let made = parts.next().unwrap_or_default();
        let bytes = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();
        if !safe_id(id) {
            return Err(Error::BadHeader);
        }
        out.push(Entry {
            id: id.to_string(),
            name: name.to_string(),
            made: made.parse().map_err(|_| Error::BadHeader)?,
            bytes: bytes.parse().map_err(|_| Error::BadHeader)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(bytes: &[u8]) -> Secret {
        let mut copy = bytes.to_vec();
        Secret::new(&mut copy)
    }

    #[test]
    fn the_same_pair_always_gives_the_same_key() {
        let a = StudioKey::derive(&secret(b"app-lock-key"), &secret(b"at-rest-key")).unwrap();
        let b = StudioKey::derive(&secret(b"app-lock-key"), &secret(b"at-rest-key")).unwrap();
        assert_eq!(a.expose(), b.expose());
        assert_eq!(a.expose().len(), KEY_LEN);
    }

    #[test]
    fn neither_secret_alone_produces_the_vault_key() {
        // The whole point of the type. Holding one half and an empty other half
        // must not derive anything, let alone the real key.
        let real = StudioKey::derive(&secret(b"app-lock-key"), &secret(b"at-rest-key")).unwrap();

        assert!(matches!(
            StudioKey::derive(&secret(b"app-lock-key"), &secret(b"")),
            Err(Error::StudioNeedsBoth)
        ));
        assert!(matches!(
            StudioKey::derive(&secret(b""), &secret(b"at-rest-key")),
            Err(Error::StudioNeedsBoth)
        ));
        assert!(matches!(
            StudioKey::derive(&secret(b""), &secret(b"")),
            Err(Error::StudioNeedsBoth)
        ));

        // And a wrong half gives a key unrelated to the right one, rather than
        // something close to it.
        let wrong_app = StudioKey::derive(&secret(b"WRONG"), &secret(b"at-rest-key")).unwrap();
        let wrong_rest = StudioKey::derive(&secret(b"app-lock-key"), &secret(b"WRONG")).unwrap();
        assert_ne!(real.expose(), wrong_app.expose());
        assert_ne!(real.expose(), wrong_rest.expose());
        assert_ne!(wrong_app.expose(), wrong_rest.expose());
    }

    #[test]
    fn changing_either_half_by_one_bit_changes_the_key() {
        let base = StudioKey::derive(&secret(b"aaaaaaaa"), &secret(b"bbbbbbbb")).unwrap();
        let first = StudioKey::derive(&secret(b"aaaaaaab"), &secret(b"bbbbbbbb")).unwrap();
        let second = StudioKey::derive(&secret(b"aaaaaaaa"), &secret(b"bbbbbbbc")).unwrap();
        assert_ne!(base.expose(), first.expose());
        assert_ne!(base.expose(), second.expose());
    }

    #[test]
    fn where_the_split_falls_is_part_of_the_key() {
        // Without the length in the info, these two pairs concatenate to the
        // same bytes and would derive the same vault key. That is a collision
        // an attacker picks rather than stumbles on: it would let a vault built
        // from one pair be opened by another.
        let a = StudioKey::derive(&secret(b"ab"), &secret(b"c")).unwrap();
        let b = StudioKey::derive(&secret(b"a"), &secret(b"bc")).unwrap();
        assert_ne!(
            a.expose(),
            b.expose(),
            "the split point is not bound into the key"
        );
    }

    #[test]
    fn swapping_the_two_halves_is_a_different_vault() {
        let forward = StudioKey::derive(&secret(b"one"), &secret(b"two")).unwrap();
        let backward = StudioKey::derive(&secret(b"two"), &secret(b"one")).unwrap();
        assert_ne!(forward.expose(), backward.expose());
    }

    #[test]
    fn the_key_is_key_shaped_rather_than_a_copy_of_its_inputs() {
        // A construction that returned one of its inputs, or their
        // concatenation, would pass every equality test above while providing
        // no mixing at all.
        let app = b"app-lock-key-material";
        let rest = b"at-rest-key-material";
        let key = StudioKey::derive(&secret(app), &secret(rest)).unwrap();
        assert_ne!(key.expose(), &app[..]);
        assert_ne!(key.expose(), &rest[..]);
        let joined: Vec<u8> = app.iter().chain(rest.iter()).copied().collect();
        assert_ne!(key.expose(), &joined[..]);
        assert!(
            !joined.windows(KEY_LEN).any(|w| w == key.expose()),
            "the key appears verbatim inside its own inputs"
        );
    }

    fn a_studio(dir: &std::path::Path) -> Studio {
        let key = StudioKey::derive(&secret(b"app-lock"), &secret(b"at-rest")).unwrap();
        Studio::open(dir, key).unwrap()
    }

    #[test]
    fn a_recording_stored_comes_back_byte_for_byte() {
        let tmp = tempfile::tempdir().unwrap();
        let studio = a_studio(tmp.path());
        assert!(
            studio.list().unwrap().is_empty(),
            "a new vault holds nothing"
        );

        let audio = vec![7u8; 5000];
        let entry = studio.store("interview", 1_767_225_600, &audio).unwrap();

        let listed = studio.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "interview");
        assert_eq!(listed[0].made, 1_767_225_600);
        assert_eq!(listed[0].bytes, audio.len());

        let back = studio.load(&entry.id).unwrap();
        assert_eq!(back.expose(), &audio[..]);
    }

    #[test]
    fn the_other_key_opens_nothing_in_this_vault() {
        // The whole promise. A vault written with one pair of secrets must be
        // opaque to any other pair, including one sharing a half.
        let tmp = tempfile::tempdir().unwrap();
        let entry = {
            let studio = a_studio(tmp.path());
            studio.store("private", 1, &[3u8; 128]).unwrap()
        };

        for (app, rest) in [
            (&b"app-lock"[..], &b"WRONG"[..]),
            (&b"WRONG"[..], &b"at-rest"[..]),
            (&b"WRONG"[..], &b"ALSO-WRONG"[..]),
        ] {
            let key = StudioKey::derive(&secret(app), &secret(rest)).unwrap();
            let other = Studio::open(tmp.path(), key).unwrap();
            assert!(other.list().is_err(), "the index opened with the wrong key");
            assert!(other.load(&entry.id).is_err(), "a recording opened");
        }
    }

    #[test]
    fn nothing_readable_is_left_on_disk() {
        // The audio must not be findable by reading the vault directory, and
        // neither must the name: both are what the vault is for.
        let tmp = tempfile::tempdir().unwrap();
        let studio = a_studio(tmp.path());
        let audio: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
        studio.store("meeting with the lawyer", 1, &audio).unwrap();

        for file in std::fs::read_dir(tmp.path()).unwrap() {
            let bytes = std::fs::read(file.unwrap().path()).unwrap();
            assert!(
                !bytes
                    .windows(audio.len().min(64))
                    .any(|w| w == &audio[..64]),
                "the audio is on disk in the clear"
            );
            assert!(
                !bytes.windows(6).any(|w| w == b"lawyer"),
                "the name is on disk in the clear"
            );
        }
    }

    #[test]
    fn a_recording_cannot_be_moved_to_another_identity_and_still_open() {
        // Each file is sealed with its own id as authenticated data, so
        // swapping two files inside a vault, which somebody with the directory
        // can do without the key, is detected rather than silently serving the
        // wrong recording under the wrong name.
        let tmp = tempfile::tempdir().unwrap();
        let studio = a_studio(tmp.path());
        let first = studio.store("one", 1, b"first recording").unwrap();
        let second = studio.store("two", 2, b"second recording").unwrap();

        let a = tmp.path().join(&first.id);
        let b = tmp.path().join(&second.id);
        let swap = tmp.path().join("swap");
        std::fs::rename(&a, &swap).unwrap();
        std::fs::rename(&b, &a).unwrap();
        std::fs::rename(&swap, &b).unwrap();

        assert!(
            studio.load(&first.id).is_err(),
            "a swapped file still opened"
        );
        assert!(studio.load(&second.id).is_err());
    }

    #[test]
    fn an_identifier_out_of_an_index_cannot_walk_out_of_the_vault() {
        // The index is a file. Somebody who can write it can put anything in
        // the id column, and that value reaches a path, so it is checked.
        assert!(safe_id("abcdefgh2345"));
        for bad in [
            "",
            "../../.bashrc",
            "..",
            "a/b",
            "a\\b",
            "ABC",
            "with space",
            "with.dot",
        ] {
            assert!(!safe_id(bad), "{bad:?} was accepted as an identifier");
        }

        let tmp = tempfile::tempdir().unwrap();
        let studio = a_studio(tmp.path());
        assert!(studio.load("../../.bashrc").is_err());
        assert!(studio.remove("../../.bashrc").is_err());
    }

    #[test]
    fn removing_one_leaves_the_others_openable() {
        let tmp = tempfile::tempdir().unwrap();
        let studio = a_studio(tmp.path());
        let a = studio.store("a", 1, b"aaaa").unwrap();
        let b = studio.store("b", 2, b"bbbb").unwrap();

        studio.remove(&a.id).unwrap();
        let left = studio.list().unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, b.id);
        assert_eq!(studio.load(&b.id).unwrap().expose(), b"bbbb");
        assert!(
            studio.load(&a.id).is_err(),
            "the removed file is still there"
        );
    }

    #[test]
    fn an_unreadable_index_is_an_error_rather_than_an_empty_vault() {
        // Reporting "no recordings" when the truth is "wrong key, or tampered
        // with" reads as reassurance, which is the worst possible answer.
        let tmp = tempfile::tempdir().unwrap();
        let studio = a_studio(tmp.path());
        studio.store("one", 1, b"audio").unwrap();
        std::fs::write(tmp.path().join(INDEX), b"not a sealed index at all").unwrap();
        assert!(studio.list().is_err());
    }

    #[test]
    fn a_name_with_a_tab_or_newline_cannot_forge_a_second_entry() {
        // The index is line-based, so a name carrying a newline could otherwise
        // inject a row, and one carrying a tab could shift every column.
        let tmp = tempfile::tempdir().unwrap();
        let studio = a_studio(tmp.path());
        studio
            .store(
                "evil\tzzzzzzzzzzzz\t9\t9\nzzzzzzzzzzzz\t1\t1\tinjected",
                1,
                b"x",
            )
            .unwrap();
        let listed = studio.list().unwrap();
        assert_eq!(listed.len(), 1, "a name forged an extra entry");
        assert!(!listed[0].name.contains('\t'));
        assert!(!listed[0].name.contains('\n'));
    }

    #[test]
    fn each_stored_recording_gets_an_identifier_of_its_own() {
        let tmp = tempfile::tempdir().unwrap();
        let studio = a_studio(tmp.path());
        let mut seen = std::collections::HashSet::new();
        for i in 0..25 {
            let e = studio.store(&format!("take {i}"), i, b"audio").unwrap();
            assert!(safe_id(&e.id), "{:?} is not a usable identifier", e.id);
            assert!(seen.insert(e.id), "an identifier repeated");
        }
        assert_eq!(studio.list().unwrap().len(), 25);
    }

    #[test]
    fn the_key_reports_its_locking_rather_than_claiming_it() {
        let key = StudioKey::derive(&secret(b"one"), &secret(b"two")).unwrap();
        // Best effort, budget-dependent: this asserts the report exists and is
        // a plain answer, not that the lock was granted.
        let _: bool = key.is_locked();
    }
}
