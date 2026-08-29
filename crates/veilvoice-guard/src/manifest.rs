// SPDX-License-Identifier: GPL-3.0-or-later
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
//!
//! # In plain words
//!
//! The written record of what VeilVoice's files were, so a later check can tell
//! whether they still are.
//!
//! It is plain text on purpose: you can read it, diff it and keep a copy
//! somewhere else. A record you cannot inspect is one you have to take on trust,
//! which rather defeats the point of having it.

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

/// Why this path cannot go in a manifest, if it cannot. **F-83.**
///
/// The record is a line-oriented text file that gets printed to a terminal, and
/// those two facts decide what a path may contain.
///
/// A line break would end the record early and let one entry forge a second.
/// A carriage return returns the cursor to the start of the line, so a crafted
/// path overwrites what the report has already printed and the report says
/// something other than what is recorded. An escape character does more again:
/// colour, cursor movement, clearing the screen. The product of this module is
/// a report somebody reads to decide whether their files have been altered, so
/// a report that can be made to lie is the whole thing failing.
///
/// The refusal covers the C0 and C1 control ranges rather than the two
/// characters that were found, because listing the ones somebody thought of is
/// how the next one gets in. Refusing rather than stripping is deliberate: a
/// path this format cannot represent faithfully is one it must not claim to
/// hold. Such a filename is legal on Unix and vanishingly rare, and being told
/// so is better than a record that quietly describes a different file.
fn unrecordable(path: &str) -> Option<&'static str> {
    if path.contains('\n') {
        return Some("a line break");
    }
    if path.contains('\r') {
        return Some("a carriage return");
    }
    if path.contains('\u{1b}') {
        return Some("an escape character");
    }
    if path.chars().any(char::is_control) {
        return Some("a control character");
    }
    None
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
            if let Some(bad) = unrecordable(&key) {
                return Err(Error::Malformed(format!(
                    "path contains {bad} and cannot be recorded: {key:?}"
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
            // **F-83.** The same refusal `Manifest::of` makes when it writes.
            //
            // Those two ends disagreed. `of` refused to record a path with a
            // line break in it, and `parse` accepted one, so VeilVoice would
            // not write a record it was perfectly happy to read from somebody
            // else. `veilvoice guard check` reads whichever file is at the
            // path it is given, and a record is exactly the kind of thing that
            // gets handed to you.
            //
            // What it costs to accept one is not theoretical. The product of
            // this whole module is a report somebody reads to decide whether
            // their files have been altered, and that report is printed to a
            // terminal. A carriage return in a path returns the cursor to the
            // start of the line, so everything already printed is overwritten
            // by whatever follows: a crafted path can make the report say
            // something other than what is recorded. An escape character does
            // more than that, and can colour, move the cursor or clear the
            // screen.
            //
            // So the whole control range is refused rather than the two
            // characters the fuzzer happened to find, and the line is named.
            // Refusing is right rather than sanitising: a record this format
            // cannot represent faithfully is one it should not claim to hold.
            if let Some(bad) = unrecordable(path) {
                return Err(Error::Malformed(format!(
                    "line {}: path contains {bad} and cannot be recorded: {path:?}",
                    number + 2
                )));
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
    ///
    /// # Why the unattended cost ceiling, and not the generous one
    ///
    /// F-92. This used the same ceiling as any other container, which is four
    /// gigabytes of Argon2 memory, and that ceiling is right for a `.veil` a
    /// person was sent and chose to open: it is slow, they can decide to stop
    /// waiting, and refusing a legitimate-but-expensive file would be worse.
    ///
    /// Nobody chooses to open this one. It sits at a known path beside the app
    /// lock, and since the desktop application started checking it at every
    /// unlock, it is read automatically whenever somebody logs in. Anybody who
    /// can write that directory can leave a sealed manifest declaring four
    /// gigabytes, and on a modest machine that is not a wait, it is an
    /// allocation failure, and this workspace aborts on one. The window would
    /// die immediately after a correct passphrase.
    ///
    /// This is the same defect F-91 fixed on the app-lock file, in the second
    /// place it applies. Fixing one and not the other is the exclusion list
    /// naming the files somebody happened to think of, which is the failure
    /// this project has already recorded twice.
    pub fn open_sealed(password: &[u8], sealed: &[u8]) -> Result<Self, Error> {
        let text = veilvoice_crypto::container::open_with_password_within(
            password,
            sealed,
            veilvoice_crypto::kdf::KdfParams::UNATTENDED_MAX_M_COST,
        )?;
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

    /// **F-83.** A record somebody hands you cannot contain a character that
    /// rewrites the report.
    ///
    /// `Manifest::of` refused a path with a line break in it and `parse`
    /// accepted one, so VeilVoice would not write a record it was happy to
    /// read from somebody else. `veilvoice guard check` reads whichever file
    /// is at the path it is given.
    ///
    /// The carriage return is the one the coverage-guided campaign found. The
    /// rest are here because listing the characters somebody thought of is how
    /// the next one gets in.
    #[test]
    fn a_path_that_could_rewrite_the_report_is_refused_on_the_way_in() {
        let digest = "a".repeat(64);
        for (label, bad) in [
            ("carriage return", "some\rthing"),
            ("escape", "some\u{1b}[2Kthing"),
            ("bell", "some\u{7}thing"),
            ("nul", "some\0thing"),
            ("backspace", "some\u{8}thing"),
        ] {
            let text = format!("{MAGIC}\n{digest}  12  {bad}\n");
            let parsed = Manifest::parse(&text);
            assert!(
                matches!(parsed, Err(Error::Malformed(_))),
                "{label} was accepted: {parsed:?}"
            );
        }
    }

    /// And an ordinary path, including the awkward but legitimate ones, still
    /// parses. A refusal that catches real filenames is a worse bug than the
    /// one it fixes.
    #[test]
    fn an_ordinary_path_is_still_recorded() {
        let digest = "a".repeat(64);
        for good in [
            "notes.wav",
            "a folder/with spaces/recording.wav",
            "C:/Users/somebody/My Documents/x.wav",
            "unicode \u{e9}\u{fc}\u{4e2d}\u{6587}.wav",
            "punctuation!@#$%^&()_+-=[]{};'.wav",
        ] {
            let text = format!("{MAGIC}\n{digest}  12  {good}\n");
            let parsed = Manifest::parse(&text);
            assert!(parsed.is_ok(), "{good:?} was refused: {parsed:?}");
        }
    }

    /// The two ends agree: what `of` will not write, `parse` will not read.
    ///
    /// That asymmetry *was* the finding, so it is the thing to hold rather
    /// than the individual characters.
    #[test]
    fn what_cannot_be_written_cannot_be_read() {
        for bad in ["a\rb", "a\nb", "a\u{1b}b", "a\0b"] {
            assert!(
                unrecordable(bad).is_some(),
                "{bad:?} would be written but not read, or the other way round"
            );
        }
        for good in ["a/b", "a b", "\u{e9}"] {
            assert!(unrecordable(good).is_none(), "{good:?}");
        }
    }

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

    /// F-92. The sealed record is read automatically at every unlock, so the
    /// cost it declares is not a cost anybody chose to pay.
    #[test]
    fn a_sealed_manifest_cannot_demand_more_memory_than_the_machine_has() {
        let mut sealed = veilvoice_crypto::container::seal_with_password(
            b"a passphrase",
            b"nothing that matters",
            veilvoice_crypto::kdf::KdfParams::default(),
        )
        .unwrap();

        // The cost sits at offset 12 in the header and is authenticated, so an
        // edit here makes the tag fail rather than the derivation run. What is
        // being tested is the order: the ceiling has to be consulted *before*
        // the memory is asked for, so a refusal is what comes back rather than
        // an allocation.
        sealed[12..16].copy_from_slice(&(4u32 * 1024 * 1024).to_le_bytes());

        let err = Manifest::open_sealed(b"a passphrase", &sealed).unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("ceiling"),
            "a four gigabyte cost should be refused by the ceiling, before the \
             memory is asked for, rather than attempted and then failed on the \
             authentication tag: {text}"
        );
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
