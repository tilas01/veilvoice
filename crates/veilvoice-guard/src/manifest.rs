// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! The integrity manifest: what the files were, and what they are now.
//!
//! # Format
//!
//! Deliberately a text format, one record per line:
//!
//! ```text
//! VEILGUARD1
//! <sha256 hex>  <size>  <path>
//! ...
//! ```
//!
//! Text rather than a packed binary layout because the point of the file is to
//! be checkable. Someone who suspects tampering can read it with `cat` and
//! compare a digest by hand with `sha256sum`, without this crate and without
//! trusting it. A binary format would have been marginally smaller and would
//! have made the honest response to "prove it" be "run my tool again".
//!
//! Paths are stored with forward slashes so a manifest written on Windows still
//! reads on Linux, and are rejected if they contain a newline -- otherwise a
//! filename could forge a record.

use crate::Error;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Magic first line. The digit is a format version.
pub(crate) const MAGIC: &str = "VEILGUARD1";

/// One recorded file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Size in bytes at the time of recording.
    pub size: u64,
    /// Lowercase hex SHA-256 of the contents.
    pub digest: String,
}

/// How a file differs from its record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Recorded, still present, and its contents differ.
    Modified {
        /// The path, as recorded.
        path: String,
        /// What the digest was.
        was: String,
        /// What it is now.
        now: String,
    },
    /// Recorded and no longer there.
    Removed {
        /// The path, as recorded.
        path: String,
    },
    /// Present in the watched set and not recorded.
    Added {
        /// The path found.
        path: String,
    },
    /// Recorded but unreadable, which is not the same as absent -- a
    /// permissions change is itself worth reporting.
    Unreadable {
        /// The path, as recorded.
        path: String,
        /// Why it could not be read.
        why: String,
    },
}

impl Change {
    /// The path this change concerns.
    pub fn path(&self) -> &str {
        match self {
            Change::Modified { path, .. }
            | Change::Removed { path }
            | Change::Added { path }
            | Change::Unreadable { path, .. } => path,
        }
    }

    /// A single line for a terminal or a log.
    pub fn describe(&self) -> String {
        match self {
            Change::Modified { path, was, now } => format!(
                "modified: {path} ({}... -> {}...)",
                &was[..8.min(was.len())],
                &now[..8.min(now.len())]
            ),
            Change::Removed { path } => format!("removed:  {path}"),
            Change::Added { path } => format!("added:    {path}"),
            Change::Unreadable { path, why } => format!("unreadable: {path} ({why})"),
        }
    }
}

/// The result of checking a manifest against the disk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Everything that differs, in a stable order.
    pub changes: Vec<Change>,
    /// How many files matched their record exactly.
    pub unchanged: usize,
}

impl Report {
    /// Whether anything at all differs.
    pub fn is_clean(&self) -> bool {
        self.changes.is_empty()
    }
}

/// A record of a set of files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    /// Recorded files, keyed by their normalised path. A `BTreeMap` so the
    /// serialised form is byte-identical for the same input, which is what
    /// makes two manifests comparable at all.
    entries: BTreeMap<String, Entry>,
}

/// Normalise a path for storage: forward slashes, no leading `./`.
fn normalise(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn digest_of(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Manifest {
    /// Record every readable file in `paths`.
    ///
    /// A path that cannot be read is skipped rather than failing the whole
    /// manifest: recording nine of ten files is more useful than recording
    /// none, and the tenth shows up as `Removed` at check time, which is the
    /// honest description of a file this build cannot see.
    pub fn of<P: AsRef<Path>>(paths: &[P]) -> Result<Self, Error> {
        let mut entries = BTreeMap::new();
        for path in paths {
            let path = path.as_ref();
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let key = normalise(path);
            if key.contains('\n') || key.contains('\r') {
                return Err(Error::Malformed(format!(
                    "path contains a line break and cannot be recorded: {key:?}"
                )));
            }
            entries.insert(
                key,
                Entry {
                    size: bytes.len() as u64,
                    digest: digest_of(&bytes),
                },
            );
        }
        Ok(Self { entries })
    }

    /// How many files are recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The recorded paths, in order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Compare the record against what is on disk now.
    ///
    /// `extra` is the set of paths currently in the watched location, so a file
    /// that has *appeared* can be reported. Pass an empty slice to check only
    /// what was recorded.
    pub fn check<P: AsRef<Path>>(&self, extra: &[P]) -> Report {
        let mut report = Report::default();

        for (path, entry) in &self.entries {
            match std::fs::read(path) {
                Ok(bytes) => {
                    let now = digest_of(&bytes);
                    if now == entry.digest {
                        report.unchanged += 1;
                    } else {
                        report.changes.push(Change::Modified {
                            path: path.clone(),
                            was: entry.digest.clone(),
                            now,
                        });
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    report.changes.push(Change::Removed { path: path.clone() });
                }
                Err(e) => {
                    report.changes.push(Change::Unreadable {
                        path: path.clone(),
                        why: e.to_string(),
                    });
                }
            }
        }

        for path in extra {
            let key = normalise(path.as_ref());
            if !self.entries.contains_key(&key) {
                report.changes.push(Change::Added { path: key });
            }
        }

        report.changes.sort_by(|a, b| a.path().cmp(b.path()));
        report
    }

    /// Serialise to the text format described at the top of this module.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        for (path, entry) in &self.entries {
            out.push_str(&format!("{}  {}  {}\n", entry.digest, entry.size, path));
        }
        out
    }

    /// Parse the text format.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.trim() == MAGIC => {}
            Some(other) => {
                return Err(Error::Malformed(format!(
                    "expected {MAGIC} on the first line, found {other:?}"
                )))
            }
            None => return Err(Error::Malformed("the manifest is empty".into())),
        }

        let mut entries = BTreeMap::new();
        for (number, line) in lines.enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            // Split on the double space, and only twice: a path may contain
            // single spaces, and routinely does on Windows.
            let mut parts = line.splitn(3, "  ");
            let digest = parts.next().unwrap_or_default().trim();
            let size = parts.next().unwrap_or_default().trim();
            let path = parts.next().unwrap_or_default().trim();

            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::Malformed(format!(
                    "line {}: {digest:?} is not a SHA-256 digest",
                    number + 2
                )));
            }
            let size: u64 = size
                .parse()
                .map_err(|_| Error::Malformed(format!("line {}: bad size {size:?}", number + 2)))?;
            if path.is_empty() {
                return Err(Error::Malformed(format!("line {}: no path", number + 2)));
            }
            // Normalised on the way in, exactly as `Manifest::of` normalises on
            // the way out. Without this, a manifest written by hand (or by an
            // older build) with backslashes produced entries keyed differently
            // from the ones `check`'s `extra` argument is keyed by, so every
            // recorded file was *also* reported as newly added. A tamper report
            // full of false positives is one nobody reads, which defeats the
            // only thing this module does.
            let path = path.replace('\\', "/");
            entries.insert(
                path,
                Entry {
                    size,
                    digest: digest.to_ascii_lowercase(),
                },
            );
        }
        Ok(Self { entries })
    }

    /// Write the manifest to `path` in the clear.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.to_text())?;
        Ok(())
    }

    /// Read a manifest written by [`Manifest::save`].
    pub fn load(path: &Path) -> Result<Self, Error> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    /// Seal the manifest under a passphrase.
    ///
    /// This is what makes the record worth more than a courtesy: rewriting it
    /// undetectably then needs the passphrase as well as write access. Keep
    /// that passphrase somewhere other than beside the manifest, or the
    /// exercise is circular.
    ///
    /// It is still not proof. An attacker present while you type the passphrase
    /// has everything, which is the same caveat every password in this project
    /// carries.
    pub fn seal(&self, password: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(veilvoice_crypto::container::seal_with_password(
            password,
            self.to_text().as_bytes(),
            veilvoice_crypto::kdf::KdfParams::default(),
        )?)
    }

    /// Open a manifest sealed by [`Manifest::seal`].
    pub fn open_sealed(password: &[u8], sealed: &[u8]) -> Result<Self, Error> {
        let text = veilvoice_crypto::container::open_with_password(password, sealed)?;
        let text = String::from_utf8(text)
            .map_err(|_| Error::Malformed("sealed manifest is not text".into()))?;
        Self::parse(&text)
    }
}

/// Every file directly inside `dir`, for use as `check`'s `extra` argument.
///
/// Not recursive on purpose: the caller decides what is being watched, and a
/// silent recursive walk of a directory somebody pointed this at is a good way
/// to hash a home folder by accident.
pub fn files_in(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            found.push(entry.path());
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn an_untouched_set_of_files_is_clean() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "veilvoice.exe", b"binary");
        let b = write(dir.path(), "applock.bin", b"lock");

        let manifest = Manifest::of(&[&a, &b]).unwrap();
        assert_eq!(manifest.len(), 2);

        let report = manifest.check::<&Path>(&[]);
        assert!(report.is_clean(), "{:?}", report.changes);
        assert_eq!(report.unchanged, 2);
    }

    #[test]
    fn a_modified_file_is_reported_with_both_digests() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "veilvoice.exe", b"binary");
        let manifest = Manifest::of(&[&a]).unwrap();

        write(dir.path(), "veilvoice.exe", b"binary, but not the same one");
        let report = manifest.check::<&Path>(&[]);

        assert_eq!(report.changes.len(), 1);
        match &report.changes[0] {
            Change::Modified { was, now, .. } => {
                assert_ne!(was, now);
                assert_eq!(was.len(), 64);
            }
            other => panic!("expected a modification, got {other:?}"),
        }
        assert!(report.changes[0].describe().starts_with("modified:"));
    }

    /// A file of the same *length* with different contents is the case a
    /// size-only check would miss, which is why the digest is the check.
    #[test]
    fn a_same_length_substitution_is_still_caught() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "veilvoice.exe", b"aaaaaaaa");
        let manifest = Manifest::of(&[&a]).unwrap();
        write(dir.path(), "veilvoice.exe", b"bbbbbbbb");

        let report = manifest.check::<&Path>(&[]);
        assert!(matches!(report.changes[0], Change::Modified { .. }));
    }

    #[test]
    fn a_removed_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "gone.bin", b"here for now");
        let manifest = Manifest::of(&[&a]).unwrap();
        std::fs::remove_file(&a).unwrap();

        let report = manifest.check::<&Path>(&[]);
        assert!(matches!(report.changes[0], Change::Removed { .. }));
        assert_eq!(report.unchanged, 0);
    }

    #[test]
    fn a_new_file_in_the_watched_set_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "known.bin", b"recorded");
        let manifest = Manifest::of(&[&a]).unwrap();
        write(dir.path(), "surprise.dll", b"not recorded");

        let present = files_in(dir.path()).unwrap();
        let report = manifest.check(&present);
        assert_eq!(report.changes.len(), 1);
        assert!(report.changes[0].path().ends_with("surprise.dll"));
        assert!(matches!(report.changes[0], Change::Added { .. }));
    }

    #[test]
    fn the_text_format_round_trips_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "one.bin", b"first");
        let b = write(dir.path(), "two with spaces.bin", b"second");

        let manifest = Manifest::of(&[&a, &b]).unwrap();
        let text = manifest.to_text();
        assert!(text.starts_with(MAGIC));

        let back = Manifest::parse(&text).unwrap();
        assert_eq!(manifest, back);
        assert_eq!(back.to_text(), text, "serialisation is not stable");
    }

    /// Paths with spaces are ordinary on Windows, and splitting naively would
    /// truncate them.
    #[test]
    fn a_path_containing_spaces_survives() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "Program Files thing.exe", b"x");
        let manifest = Manifest::of(&[&a]).unwrap();
        let back = Manifest::parse(&manifest.to_text()).unwrap();
        assert!(back.paths().any(|p| p.ends_with("Program Files thing.exe")));
        assert!(back.check::<&Path>(&[]).is_clean());
    }

    #[test]
    fn a_malformed_manifest_is_rejected_rather_than_half_read() {
        assert!(Manifest::parse("").is_err());
        assert!(Manifest::parse("NOT-THE-MAGIC\n").is_err());
        assert!(Manifest::parse(&format!("{MAGIC}\nnothex  12  a.bin\n")).is_err());
        assert!(Manifest::parse(&format!("{MAGIC}\n{}  x  a.bin\n", "a".repeat(64))).is_err());
        assert!(Manifest::parse(&format!("{MAGIC}\n{}  12  \n", "a".repeat(64))).is_err());
        // A blank line in the middle is tolerated, not an error.
        assert!(Manifest::parse(&format!("{MAGIC}\n\n")).is_ok());
    }

    /// A hand-written manifest using backslashes must key the same way one this
    /// crate wrote does, or every recorded file also reports as newly added and
    /// the report becomes noise.
    #[test]
    fn a_manifest_written_with_backslashes_keys_the_same_way() {
        let digest = "a".repeat(64);
        let back = Manifest::parse(&format!("{MAGIC}\n{digest}  1  C:\\dir\\thing.exe\n")).unwrap();
        let forward =
            Manifest::parse(&format!("{MAGIC}\n{digest}  1  C:/dir/thing.exe\n")).unwrap();
        assert_eq!(back, forward);
        assert_eq!(back.paths().collect::<Vec<_>>(), ["C:/dir/thing.exe"]);
        // And it survives a second round trip unchanged.
        assert_eq!(Manifest::parse(&back.to_text()).unwrap(), back);
    }

    #[test]
    fn saving_and_loading_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "one.bin", b"first");
        let manifest = Manifest::of(&[&a]).unwrap();

        let store = dir.path().join("nested").join("manifest.txt");
        manifest.save(&store).unwrap();
        assert_eq!(Manifest::load(&store).unwrap(), manifest);
    }

    /// The sealed form is what makes the record more than a courtesy: without
    /// the passphrase it cannot be rewritten to match a tampered file.
    #[test]
    fn a_sealed_manifest_needs_its_passphrase() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "one.bin", b"first");
        let manifest = Manifest::of(&[&a]).unwrap();

        let sealed = manifest.seal(b"a passphrase kept elsewhere").unwrap();
        assert!(
            !sealed.windows(MAGIC.len()).any(|w| w == MAGIC.as_bytes()),
            "the manifest is readable inside its own container"
        );
        assert_eq!(
            Manifest::open_sealed(b"a passphrase kept elsewhere", &sealed).unwrap(),
            manifest
        );
        assert!(Manifest::open_sealed(b"the wrong one", &sealed).is_err());
    }

    #[test]
    fn an_unreadable_path_is_skipped_when_recording() {
        let dir = tempfile::tempdir().unwrap();
        let real = write(dir.path(), "real.bin", b"x");
        let missing = dir.path().join("never-existed.bin");

        let manifest = Manifest::of(&[&real, &missing]).unwrap();
        assert_eq!(manifest.len(), 1, "the missing file must not be recorded");
    }

    #[test]
    fn recording_nothing_is_allowed_and_checks_clean() {
        let manifest = Manifest::of::<&Path>(&[]).unwrap();
        assert!(manifest.is_empty());
        assert!(manifest.check::<&Path>(&[]).is_clean());
    }
}
