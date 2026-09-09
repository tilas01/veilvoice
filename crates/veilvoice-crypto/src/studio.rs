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
/// are and roughly how large each is, and nothing else.
///
/// Those two facts are not hidden inside a vault, and the documentation says so
/// rather than implying otherwise. What hides them is the folder the vault is
/// in: [`make_decoy_in`] fills it with vaults of exactly this size holding
/// nothing, and [`find_or_make`] finds the real one by opening it rather than
/// by its name, so the count and the sizes stop identifying anything. The
/// program folder's own storage takes the other route, padding, which
/// [`crate::hoard`] is for and is a different trade.
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
        self.unseal_secret(&sealed, id.as_bytes())
    }

    /// Change what a recording is called.
    ///
    /// Only the index is rewritten. The audio is sealed under the recording's
    /// identifier rather than its name, so renaming does not re-encrypt
    /// anything and cannot lose the recording if it is interrupted: either the
    /// new index lands or the old one stays.
    ///
    /// A name that is not in the vault is an error rather than a silent
    /// no-operation. Somebody renaming a recording that is not there has a
    /// wrong identifier, and telling them so is more use than appearing to
    /// succeed.
    pub fn rename(&self, id: &str, name: &str) -> Result<(), Error> {
        if !safe_id(id) {
            return Err(Error::BadHeader);
        }
        let mut all = self.list()?;
        let Some(entry) = all.iter_mut().find(|e| e.id == id) else {
            return Err(Error::NoSuchTake);
        };
        // Stored as given. `render_index` is the one place that knows the
        // index format and is where a name is made safe for it, stripping the
        // tab and the newline that would otherwise forge a field or an entry.
        //
        // This used to clean the name here as well, and that second copy was
        // **weaker**: it stripped newlines and not tabs, in a format whose
        // fields are tab separated. Two rules for one job, one of them wrong,
        // is worse than one rule, so this defers to the writer.
        entry.name = name.to_string();
        self.write_index(&all)
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

    /// The same, decrypting straight into locked memory.
    ///
    /// A recording is the whole point of this vault and it is large. Going
    /// through [`Self::unseal`] would put every byte of it into an ordinary
    /// heap `Vec` first, where the kernel is free to page it out, and only then
    /// copy it into a [`Secret`]. That window is not brief for a file of any
    /// size, and it is exactly the window this vault exists to close, so the
    /// index goes through `unseal` and the audio goes through here.
    fn unseal_secret(&self, sealed: &[u8], aad: &[u8]) -> Result<Secret, Error> {
        if sealed.len() < crate::aead::NONCE_LEN {
            return Err(Error::Truncated);
        }
        let mut nonce = [0u8; crate::aead::NONCE_LEN];
        nonce.copy_from_slice(&sealed[..crate::aead::NONCE_LEN]);
        let key = self.secret_key();
        crate::aead::open_secret(&key, &nonce, aad, &sealed[crate::aead::NONCE_LEN..])
    }
}

/// How long an identifier is, in characters.
///
/// Named because two things read it: the generator below, and the arithmetic
/// that works out how large a decoy will be. A second literal in either place
/// would be a fact written twice.
const ID_LEN: usize = 20;

/// A random, opaque identifier: [`ID_LEN`] lower-case letters and digits that
/// say nothing about what they name.
///
/// One byte is drawn per character and five bits of each are used, so the
/// identifier carries five bits per character: a hundred of them at the length
/// set above. That is far more than a vault will ever hold and it is not a
/// compromise for space: the alphabet has 32 letters in it, so five bits per
/// character is exactly what one character holds, and taking more would need a
/// base conversion for no benefit.
fn new_id() -> Result<String, Error> {
    let mut raw = [0u8; ID_LEN];
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

/// What a decoy vault looks like from outside, so it looks like the real one.
///
/// Taken from a real vault rather than invented, because the whole value of a
/// decoy is that the two cannot be told apart by looking. A decoy built to a
/// guessed shape is a decoy that stands out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// How many recordings the vault appears to hold.
    pub recordings: usize,
    /// Bytes in each, before sealing.
    pub each: usize,
    /// Bytes in the index before it is sealed.
    ///
    /// Carried because the index file's size is visible on the disk and the
    /// rest of the shape does not determine it: names are stored in there, and
    /// a vault whose recordings are called something has a larger index than
    /// one whose recordings are called nothing. A decoy that skipped this would
    /// be the vault in the folder with the smallest index file.
    pub index: usize,
}

impl Shape {
    /// Measure a real vault, to build decoys that match it.
    pub fn of(studio: &Studio) -> Result<Self, Error> {
        let entries = studio.list()?;
        let total: usize = entries.iter().map(|e| e.bytes).sum();
        Ok(Self {
            recordings: entries.len(),
            // The mean, so a decoy is the same size overall. Matching every
            // individual length would copy the real vault's fingerprint into
            // the decoy, which is the opposite of what a decoy is for.
            each: if entries.is_empty() {
                0
            } else {
                total / entries.len()
            },
            index: render_index(&entries).len(),
        })
    }

    /// What one vault of this shape occupies, in bytes, as files on a disk.
    ///
    /// # Why this is here and not where it is asked for
    ///
    /// The number depends on the sealed file layout: a nonce and a tag on
    /// every file, and the index beside them. That layout is this module's, so
    /// the arithmetic is this module's too. An interface that worked it out for
    /// itself would be a second copy of the format, and the two would part
    /// company the first time a field was added here.
    ///
    /// It is exact rather than approximate, and a test builds decoys and adds
    /// up the real files to prove it stays exact.
    pub fn bytes_on_disk(&self) -> u64 {
        // Every sealed file carries a nonce in front and a tag behind.
        let overhead = (crate::aead::NONCE_LEN + crate::aead::TAG_LEN) as u64;

        let recordings = (self.each as u64)
            .saturating_add(overhead)
            .saturating_mul(self.recordings as u64);
        let index = overhead.saturating_add(self.index_len() as u64);

        recordings.saturating_add(index)
    }

    /// The shortest index a decoy of this shape can be written with: every
    /// entry present and every name empty.
    fn bare_index(&self) -> usize {
        // The identifier, three tabs, a `made` of zero which is one digit, the
        // byte count as text, an empty name, and a newline.
        self.recordings
            .saturating_mul(ID_LEN + 3 + 1 + digits(self.each) + 1)
    }

    /// The index length a decoy of this shape will actually be written with.
    ///
    /// The measured length, unless that is shorter than a decoy of this many
    /// recordings can be, in which case the shortest one is what gets written.
    /// Both this and [`make_decoy`] read it, so the size a panel promises and
    /// the size the disk receives cannot part company.
    fn index_len(&self) -> usize {
        self.index.max(self.bare_index())
    }
}

/// How many decimal digits `n` is written with.
///
/// The index stores the byte count as text, so its width is part of the size on
/// disk. Written out rather than reached for through a formatted `String`,
/// because this is asked once per decoy per redraw of a panel.
fn digits(n: usize) -> usize {
    let mut d = 1;
    let mut n = n;
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}

/// Every directory under `parent` that is shaped like a vault.
///
/// Shaped like one means it holds an index. That is the only thing that can be
/// seen from outside, and it is deliberately the only thing looked at: a real
/// vault and a decoy are the same shape here, and which of them opens is a
/// question only a key can answer.
///
/// Sorted by name so the order does not depend on how the filesystem happens to
/// hand directories back, which would otherwise make a test flaky and, worse,
/// make the order a decoy is tried in vary between machines.
pub fn vault_dirs(parent: &std::path::Path) -> Result<Vec<std::path::PathBuf>, Error> {
    let mut out = Vec::new();
    let listing = match std::fs::read_dir(parent) {
        Ok(listing) => listing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(_) => return Err(Error::AppLockStore),
    };
    for entry in listing.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join(INDEX).is_file() {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// Open the one vault under `parent` that `key` unlocks, making it on a first
/// run.
///
/// # Why the vault is found rather than named
///
/// A decoy is only worth making if it cannot be told from the real thing, and a
/// real vault at a fixed, known name is told from a decoy by reading the name.
/// So every vault under `parent`, real and decoy alike, is a directory with an
/// opaque identifier for a name, and the only thing that distinguishes them is
/// that exactly one of them opens.
///
/// This tries each in turn. Trying is cheap: the expensive part of unlocking is
/// deriving the key, which happens once before this is called, and each attempt
/// after that is opening one small sealed index.
///
/// # What happens when nothing opens
///
/// The error from the last attempt is returned, and **no vault is made**.
/// Making a fresh one there would be the worst answer available: somebody who
/// mistyped a passphrase would be shown an empty vault and would reasonably
/// conclude their recordings were gone.
///
/// A vault is made only when `parent` holds none at all, which is a first run.
/// It is made with an index written immediately, so that a vault holding
/// nothing is the same shape on the disk as a decoy holding nothing, from the
/// moment it exists.
pub fn find_or_make(parent: &std::path::Path, key: StudioKey) -> Result<Studio, Error> {
    std::fs::create_dir_all(parent).map_err(|_| Error::AppLockStore)?;
    migrate_flat(parent)?;

    let mut studio = Studio {
        dir: parent.to_path_buf(),
        key,
    };

    let mut last = None;
    for dir in vault_dirs(parent)? {
        studio.dir = dir;
        match studio.list() {
            Ok(_) => return Ok(studio),
            Err(error) => last = Some(error),
        }
    }
    if let Some(error) = last {
        return Err(error);
    }

    studio.dir = parent.join(new_id()?);
    std::fs::create_dir_all(&studio.dir).map_err(|_| Error::AppLockStore)?;
    studio.write_index(&[])?;
    Ok(studio)
}

/// Move a vault written straight into `parent` down into a directory of its
/// own.
///
/// # The layout this converts from
///
/// Before decoys existed there was one vault and it sat directly in `parent`,
/// because there was nothing for it to be confused with. There is now, and a
/// vault sitting where decoys are siblings would be the one directory that is
/// not a directory, which gives it away completely.
///
/// # Interrupted half way
///
/// The index moves **last**, so `parent` still holding an index means the move
/// did not finish, and this runs again on the next open. It moves into the
/// directory already made rather than a new one when there is exactly one, so
/// running again finishes the job instead of splitting the vault in two.
fn migrate_flat(parent: &std::path::Path) -> Result<(), Error> {
    if !parent.join(INDEX).is_file() {
        return Ok(());
    }

    // A directory already there is a previous run of this that did not finish.
    // Anything else means a vault and a half, which is not a state this can
    // reach and not one to guess at.
    let existing: Vec<std::path::PathBuf> = std::fs::read_dir(parent)
        .map_err(|_| Error::AppLockStore)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    let into = match existing.len() {
        0 => parent.join(new_id()?),
        1 => existing[0].clone(),
        _ => return Err(Error::AppLockStore),
    };
    std::fs::create_dir_all(&into).map_err(|_| Error::AppLockStore)?;

    for entry in std::fs::read_dir(parent)
        .map_err(|_| Error::AppLockStore)?
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() || path.file_name() == Some(std::ffi::OsStr::new(INDEX)) {
            continue;
        }
        std::fs::rename(&path, into.join(entry.file_name())).map_err(|_| Error::AppLockStore)?;
    }
    std::fs::rename(parent.join(INDEX), into.join(INDEX)).map_err(|_| Error::AppLockStore)?;
    Ok(())
}

/// Make one decoy under `parent`, named the way a real vault is named.
///
/// Returns where it went. The name is drawn from the same generator that names
/// recordings and vaults, so a decoy is not distinguishable from the real vault
/// by its name any more than by its size.
pub fn make_decoy_in(parent: &std::path::Path, shape: Shape) -> Result<std::path::PathBuf, Error> {
    let dir = parent.join(new_id()?);
    make_decoy(&dir, shape)?;
    Ok(dir)
}

/// Fill `dir` with a vault that never held anything.
///
/// # What a decoy is, and what it deliberately is not
///
/// It is **not** a vault with weak contents, or a vault whose passphrase is
/// written down somewhere, or a vault holding harmless recordings. Any of those
/// is a vault that rewards cracking, and a decoy that rewards cracking teaches
/// an attacker that cracking works.
///
/// It is a vault whose contents never existed. The files are random bytes
/// sealed under a key generated here and dropped before this function returns,
/// so nobody holds it: not the person who made the decoy, not this code, not
/// anybody who takes the disk. Opened by brute force, it yields bytes that
/// parse as nothing, because there is nothing under them to find.
///
/// # What it buys, stated plainly
///
/// It raises the cost of a search. Somebody who takes a disk and finds nine
/// vaults must attack all nine to learn which one matters, and eight of them
/// cannot be finished at any price.
///
/// It does **not** make the real vault unfindable to somebody who watches you
/// open it, reads this process's memory, or has any other way to see which
/// directory you actually use. Decoys are cover against a search of the disk,
/// not against being observed, and anybody relying on them should know which of
/// those they are facing.
pub fn make_decoy(dir: impl AsRef<std::path::Path>, shape: Shape) -> Result<(), Error> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir).map_err(|_| Error::AppLockStore)?;

    // Generated, used, and never returned. Dropping it is the point: a key
    // nobody holds is a key nobody can be made to give up.
    let key = StudioKey(Secret::random(KEY_LEN)?);
    let decoy = Studio {
        dir: dir.to_path_buf(),
        key,
    };

    let mut entries = Vec::with_capacity(shape.recordings);
    for _ in 0..shape.recordings {
        let id = new_id()?;
        // Random bytes rather than silence or a tone. A decoy full of zeroes
        // compresses differently from a real recording, and a stored size that
        // does not match its entropy is exactly the tell this exists to avoid.
        let mut filler = vec![0u8; shape.each];
        getrandom::getrandom(&mut filler).map_err(|_| Error::Random)?;
        let sealed = decoy.seal(&filler, id.as_bytes())?;
        crate::privatefile::write_owner_only(&dir.join(&id), &sealed)
            .map_err(|_| Error::AppLockStore)?;
        entries.push(Entry {
            id,
            // Filled in below, once it is known how much room is left to fill.
            name: String::new(),
            made: 0,
            bytes: shape.each,
        });
    }
    pad_index(&mut entries, shape.index_len())?;
    decoy.write_index(&entries)
}

/// Grow the names until the index is exactly the length a real one was.
///
/// # Why the names are padded rather than left empty
///
/// The index is sealed, so nobody can read a name out of it or see how the
/// length is divided between them. What anybody can see is the size of the
/// file, and that size is the length of the text plus a fixed overhead. A real
/// vault's recordings are called something and a decoy's are called nothing, so
/// without this every decoy in a folder is the one with the smallest index.
///
/// The share is even because the division is invisible: only the total is on
/// the disk. The characters are random rather than repeated for the same reason
/// the audio is, which is that a file whose size does not match its entropy is
/// itself a tell if the sealing is ever broken.
///
/// `want` shorter than the bare minimum is left alone rather than forced. It
/// means the real index was smaller than a decoy of the same count can be, and
/// the honest answer to that is a decoy a few bytes larger, not a corrupt one.
fn pad_index(entries: &mut [Entry], want: usize) -> Result<(), Error> {
    if entries.is_empty() {
        return Ok(());
    }
    let bare = render_index(entries).len();
    let Some(spare) = want.checked_sub(bare) else {
        return Ok(());
    };

    let n = entries.len();
    for (i, entry) in entries.iter_mut().enumerate() {
        // The last one takes the remainder, so the total is exact rather than
        // short by up to one byte per entry.
        let take = if i + 1 == n {
            spare - (spare / n) * (n - 1)
        } else {
            spare / n
        };
        let mut raw = vec![0u8; take];
        getrandom::getrandom(&mut raw).map_err(|_| Error::Random)?;
        // Lower-case letters: one byte each, and neither a tab nor a newline,
        // so the rendered length is the number of characters asked for.
        entry.name = raw.iter().map(|b| (b'a' + b % 26) as char).collect();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(bytes: &[u8]) -> Secret {
        let mut copy = bytes.to_vec();
        Secret::new(&mut copy)
    }

    /// A vault in a temporary directory, with a take already in it.
    fn vault_with_a_take() -> (tempfile::TempDir, Studio, String) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let key = StudioKey::derive(&secret(b"app-lock"), &secret(b"at-rest")).unwrap();
        let vault = Studio::open(dir.path(), key).unwrap();
        let entry = vault
            .store("first name", 1_700_000_000, b"RIFFfake")
            .unwrap();
        let id = entry.id.clone();
        (dir, vault, id)
    }

    #[test]
    fn renaming_changes_the_name_and_nothing_else() {
        let (_dir, vault, id) = vault_with_a_take();
        let before = vault.list().unwrap();
        vault.rename(&id, "second name").unwrap();
        let after = vault.list().unwrap();

        assert_eq!(after.len(), 1);
        assert_eq!(after[0].name, "second name");
        // The identifier, the date and the size are untouched: renaming
        // rewrites the index and never re-seals the audio.
        assert_eq!(after[0].id, before[0].id);
        assert_eq!(after[0].made, before[0].made);
        assert_eq!(after[0].bytes, before[0].bytes);
        // And the recording still opens, which is the thing a rename must not
        // be able to break.
        assert_eq!(vault.load(&id).unwrap().expose(), b"RIFFfake");
    }

    #[test]
    fn a_newline_in_a_name_cannot_forge_a_second_entry() {
        // The index is one entry per line. A name carrying a newline would be
        // read back as the start of another entry, and the vault would lose a
        // recording rather than refuse the name.
        let (_dir, vault, id) = vault_with_a_take();
        vault.rename(&id, "quiet\nname\tforged\t1\t2").unwrap();
        let after = vault.list().unwrap();
        assert_eq!(after.len(), 1, "a name split the index into two entries");
        assert!(!after[0].name.contains('\n'), "a newline survived");
        // And a tab, which is what separates the fields: a name carrying one
        // would forge an identifier, a date and a size for an entry that does
        // not exist.
        assert!(
            !after[0].name.contains('\t'),
            "a tab survived: {:?}",
            after[0].name
        );
        assert_eq!(vault.load(&id).unwrap().expose(), b"RIFFfake");
    }

    #[test]
    fn renaming_something_that_is_not_there_is_refused() {
        let (_dir, vault, _id) = vault_with_a_take();
        // A well-formed identifier that is simply not in this vault.
        assert!(matches!(
            vault.rename("0123456789abcdef", "new"),
            Err(Error::NoSuchTake)
        ));
        // And one that is not an identifier at all: a rename must not be a
        // route to writing outside the vault directory.
        assert!(matches!(
            vault.rename("../../etc/passwd", "new"),
            Err(Error::BadHeader)
        ));
        // Neither attempt disturbed what is there.
        assert_eq!(vault.list().unwrap()[0].name, "first name");
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
    fn a_decoy_looks_like_the_real_thing_from_outside() {
        // The whole value of a decoy is that the two cannot be told apart by
        // looking at the directory, so that is what this checks: the same
        // number of files, the same shape of names, an index present in both.
        let real_dir = tempfile::tempdir().unwrap();
        let real = a_studio(real_dir.path());
        for i in 0..4 {
            real.store(&format!("take {i}"), 100 + i, &vec![9u8; 2048])
                .unwrap();
        }
        let shape = Shape::of(&real).unwrap();
        assert_eq!(shape.recordings, 4);
        assert_eq!(shape.each, 2048);

        let decoy_dir = tempfile::tempdir().unwrap();
        make_decoy(decoy_dir.path(), shape).unwrap();

        let names = |d: &std::path::Path| {
            let mut v: Vec<String> = std::fs::read_dir(d)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
                .collect();
            v.sort();
            v
        };
        let r = names(real_dir.path());
        let d = names(decoy_dir.path());
        assert_eq!(
            r.len(),
            d.len(),
            "a different number of files gives it away"
        );
        assert!(d.contains(&INDEX.to_string()), "a decoy has an index too");
        for name in &d {
            assert!(
                name == INDEX || safe_id(name),
                "{name:?} is not shaped like a real identifier"
            );
        }
    }

    #[test]
    fn a_decoy_holds_nothing_and_nobody_can_open_it() {
        // Not a vault with weak contents: a vault whose contents never existed.
        // The key is generated inside `make_decoy` and dropped before it
        // returns, so there is no key to find, to leak, or to be compelled to
        // hand over.
        let dir = tempfile::tempdir().unwrap();
        make_decoy(
            dir.path(),
            Shape {
                recordings: 3,
                each: 512,
                index: 0,
            },
        )
        .unwrap();

        // The real vault's key opens none of it, and neither does any other.
        for (app, rest) in [(&b"app-lock"[..], &b"at-rest"[..]), (&b"x"[..], &b"y"[..])] {
            let key = StudioKey::derive(&secret(app), &secret(rest)).unwrap();
            let attempt = Studio::open(dir.path(), key).unwrap();
            assert!(attempt.list().is_err(), "a decoy index opened");
        }
    }

    #[test]
    fn a_first_run_makes_one_vault_and_the_next_run_finds_it() {
        let home = tempfile::tempdir().unwrap();
        let key = || StudioKey::derive(&secret(b"app"), &secret(b"rest")).unwrap();

        let first = find_or_make(home.path(), key()).unwrap();
        let made = first.dir().to_path_buf();
        first.store("a take", 7, b"audio").unwrap();

        // The vault is a directory of its own with an opaque name, not the
        // parent, so a decoy beside it is not told apart by reading the name.
        assert_eq!(made.parent().unwrap(), home.path());
        assert!(safe_id(made.file_name().unwrap().to_str().unwrap()));

        let again = find_or_make(home.path(), key()).unwrap();
        assert_eq!(again.dir(), made, "the second run made a second vault");
        assert_eq!(again.list().unwrap().len(), 1);
    }

    #[test]
    fn a_new_vault_has_an_index_from_the_moment_it_exists() {
        // Otherwise a vault holding nothing is an empty directory and a decoy
        // holding nothing is a directory with a file in it, which is a tell
        // that lasts until the first recording.
        let home = tempfile::tempdir().unwrap();
        let key = StudioKey::derive(&secret(b"app"), &secret(b"rest")).unwrap();
        let vault = find_or_make(home.path(), key).unwrap();
        assert!(vault.dir().join(INDEX).is_file());
        assert_eq!(
            vault_dirs(home.path()).unwrap(),
            vec![vault.dir().to_path_buf()]
        );
    }

    #[test]
    fn the_wrong_pair_finds_nothing_and_makes_nothing() {
        // The dangerous failure this forbids: a mistyped passphrase creating a
        // second, empty vault, which would look exactly like the recordings
        // having been lost.
        let home = tempfile::tempdir().unwrap();
        let right = StudioKey::derive(&secret(b"app"), &secret(b"rest")).unwrap();
        find_or_make(home.path(), right)
            .unwrap()
            .store("a take", 1, b"audio")
            .unwrap();

        let wrong = StudioKey::derive(&secret(b"app"), &secret(b"typo")).unwrap();
        assert!(find_or_make(home.path(), wrong).is_err());
        assert_eq!(vault_dirs(home.path()).unwrap().len(), 1);
    }

    #[test]
    fn the_real_vault_is_found_among_its_decoys() {
        let home = tempfile::tempdir().unwrap();
        let key = || StudioKey::derive(&secret(b"app"), &secret(b"rest")).unwrap();
        let vault = find_or_make(home.path(), key()).unwrap();
        vault.store("a take", 1, &vec![7u8; 4096]).unwrap();
        let real = vault.dir().to_path_buf();
        let shape = Shape::of(&vault).unwrap();
        drop(vault);

        for _ in 0..5 {
            make_decoy_in(home.path(), shape).unwrap();
        }
        assert_eq!(vault_dirs(home.path()).unwrap().len(), 6);

        let found = find_or_make(home.path(), key()).unwrap();
        assert_eq!(found.dir(), real, "a decoy was taken for the vault");
        assert_eq!(found.list().unwrap().len(), 1);
    }

    #[test]
    fn a_decoy_is_the_same_shape_on_the_disk_as_the_vault_it_copies() {
        // Same number of files, same total size, same style of name. What a
        // search of the disk can see is the same for both.
        let home = tempfile::tempdir().unwrap();
        let key = StudioKey::derive(&secret(b"app"), &secret(b"rest")).unwrap();
        let vault = find_or_make(home.path(), key).unwrap();
        for i in 0..3 {
            vault
                .store(&format!("take {i}"), i, &vec![3u8; 8192])
                .unwrap();
        }
        let shape = Shape::of(&vault).unwrap();
        let decoy = make_decoy_in(home.path(), shape).unwrap();

        let weigh = |d: &std::path::Path| -> (usize, u64) {
            let files: Vec<_> = std::fs::read_dir(d).unwrap().flatten().collect();
            (
                files.len(),
                files.iter().map(|f| f.metadata().unwrap().len()).sum(),
            )
        };
        assert_eq!(weigh(vault.dir()), weigh(&decoy));
        assert!(safe_id(decoy.file_name().unwrap().to_str().unwrap()));
    }

    #[test]
    fn a_vault_written_the_old_way_moves_down_into_a_directory_of_its_own() {
        let home = tempfile::tempdir().unwrap();
        let key = || StudioKey::derive(&secret(b"app"), &secret(b"rest")).unwrap();

        // The layout before decoys existed: the vault straight in the parent.
        let old = Studio::open(home.path(), key()).unwrap();
        old.store("kept", 42, b"the recording").unwrap();
        drop(old);
        assert!(home.path().join(INDEX).is_file());

        let moved = find_or_make(home.path(), key()).unwrap();
        assert_ne!(moved.dir(), home.path(), "it did not move");
        assert!(
            !home.path().join(INDEX).exists(),
            "the old index is still there"
        );
        let entries = moved.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "kept");
        assert_eq!(
            moved.load(&entries[0].id).unwrap().expose(),
            b"the recording",
            "the audio did not come with it"
        );
    }

    #[test]
    fn a_move_interrupted_half_way_finishes_rather_than_splitting_the_vault() {
        let home = tempfile::tempdir().unwrap();
        let key = || StudioKey::derive(&secret(b"app"), &secret(b"rest")).unwrap();
        let old = Studio::open(home.path(), key()).unwrap();
        let kept = old.store("kept", 42, b"the recording").unwrap();
        drop(old);

        // What an interruption leaves: the directory made and the recording
        // moved into it, and the index still in the parent because it goes
        // last. The move must resume into that directory, not a second one.
        let half = home.path().join(new_id().unwrap());
        std::fs::create_dir(&half).unwrap();
        std::fs::rename(home.path().join(&kept.id), half.join(&kept.id)).unwrap();

        let moved = find_or_make(home.path(), key()).unwrap();
        assert_eq!(moved.dir(), half, "it started a second vault");
        assert_eq!(vault_dirs(home.path()).unwrap().len(), 1);
        assert_eq!(moved.load(&kept.id).unwrap().expose(), b"the recording");
    }

    #[test]
    fn a_decoy_takes_exactly_the_room_its_shape_says_it_will() {
        // The interface offering decoys shows how much room they will take and
        // how many will fit, and both come from `bytes_on_disk`. If that
        // arithmetic drifts from the file format the panel is lying about the
        // disk, so this builds real decoys and adds up the real files.
        for shape in [
            Shape {
                recordings: 3,
                each: 2048,
                index: 0,
            },
            Shape {
                recordings: 1,
                each: 999_999,
                index: 0,
            },
            Shape {
                recordings: 0,
                each: 0,
                index: 0,
            },
            Shape {
                recordings: 7,
                each: 1,
                index: 0,
            },
        ] {
            let dir = tempfile::tempdir().unwrap();
            make_decoy(dir.path(), shape).unwrap();
            let actual: u64 = std::fs::read_dir(dir.path())
                .unwrap()
                .map(|e| e.unwrap().metadata().unwrap().len())
                .sum();
            assert_eq!(
                actual,
                shape.bytes_on_disk(),
                "{shape:?} was predicted at {} bytes and took {actual}",
                shape.bytes_on_disk()
            );
        }
    }

    #[test]
    fn a_decoy_is_not_full_of_zeroes() {
        // A decoy of silence compresses differently from a real recording, and
        // a file whose size does not match its entropy is the exact tell this
        // is meant to avoid.
        let dir = tempfile::tempdir().unwrap();
        make_decoy(
            dir.path(),
            Shape {
                recordings: 1,
                each: 4096,
                index: 0,
            },
        )
        .unwrap();
        for file in std::fs::read_dir(dir.path()).unwrap() {
            let bytes = std::fs::read(file.unwrap().path()).unwrap();
            let zeroes = bytes.iter().filter(|&&b| b == 0).count();
            assert!(
                zeroes * 4 < bytes.len(),
                "a decoy file is mostly zeroes, which is a tell"
            );
        }
    }

    #[test]
    fn decoys_differ_from_each_other() {
        // Identical decoys are one decoy copied, and an attacker who notices
        // that has learned they are all fake.
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let shape = Shape {
            recordings: 2,
            each: 1024,
            index: 0,
        };
        make_decoy(a.path(), shape).unwrap();
        make_decoy(b.path(), shape).unwrap();

        let read_all = |d: &std::path::Path| {
            let mut v: Vec<Vec<u8>> = std::fs::read_dir(d)
                .unwrap()
                .map(|e| std::fs::read(e.unwrap().path()).unwrap())
                .collect();
            v.sort();
            v
        };
        assert_ne!(read_all(a.path()), read_all(b.path()));
    }

    #[test]
    fn the_shape_of_an_empty_vault_does_not_divide_by_zero() {
        let dir = tempfile::tempdir().unwrap();
        let studio = a_studio(dir.path());
        let shape = Shape::of(&studio).unwrap();
        assert_eq!(shape.recordings, 0);
        assert_eq!(shape.each, 0);

        // And a decoy of that shape is an empty vault, which is still a
        // believable thing for somebody to have.
        let decoy = tempfile::tempdir().unwrap();
        make_decoy(decoy.path(), shape).unwrap();
        assert!(decoy.path().join(INDEX).exists());
    }

    /// A recording never passes through an ordinary heap buffer on its way out.
    ///
    /// `Studio::load` used to call `unseal`, which calls `aead::open`, which
    /// hands back a plain `Vec<u8>`. The whole recording was decrypted into
    /// pageable memory the kernel may write to swap, and only then copied into
    /// a `Secret` and the vector wiped. Every test here passed: the bytes were
    /// right and the wrong key still opened nothing. The defect was in where
    /// the right bytes had been, which no round trip can see.
    ///
    /// So this reads the source. It is a blunt check and it is the only kind
    /// that can express "and it never went anywhere else on the way".
    #[test]
    fn a_recording_is_decrypted_straight_into_locked_memory() {
        let source = include_str!("studio.rs").replace("\r\n", "\n");
        let body = source
            .split("pub fn load(&self, id: &str)")
            .nth(1)
            .expect("Studio::load has to be findable");
        let body = body.split("\n    /// ").next().unwrap_or(body);
        assert!(
            body.contains("unseal_secret"),
            "Studio::load must decrypt into a Secret, not into a Vec: {body}"
        );
        assert!(
            !body.contains("self.unseal("),
            "Studio::load is back on the Vec path: {body}"
        );

        // And the two really are different routes, so the assertion above is
        // about something rather than about a name.
        let secret_route = source
            .split("fn unseal_secret(")
            .nth(1)
            .expect("unseal_secret has to be findable");
        assert!(secret_route.contains("open_secret("));
    }

    #[test]
    fn the_key_reports_its_locking_rather_than_claiming_it() {
        let key = StudioKey::derive(&secret(b"one"), &secret(b"two")).unwrap();
        // Best effort, budget-dependent: this asserts the report exists and is
        // a plain answer, not that the lock was granted.
        let _: bool = key.is_locked();
    }
}
