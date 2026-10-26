// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-meta
//!
//! Strip or spoof the identifying metadata that rides along with media files.
//!
//! ## Why this exists
//!
//! De-identifying a voice accomplishes nothing if the file still says who
//! recorded it. A phone recording routinely carries the device model, the
//! recording software, a precise timestamp and, for images, GPS coordinates
//! accurate to a few metres. That is often a far easier way to identify someone
//! than analysing their voice, and it survives every DSP transform because it
//! is not in the audio at all.
//!
//! ## Strip versus spoof
//!
//! Removing every tag is not always the least conspicuous choice. A file with
//! *no* metadata whatsoever is itself a signal: it says the sender was trying to
//! hide something, and it stands out in a set of otherwise ordinary files.
//! [`Policy`] therefore offers two approaches:
//!
//! - [`Policy::Strip`] removes everything. Best when the file is expected to
//!   be sanitised anyway, or when any false statement would be worse than an
//!   obvious absence.
//! - [`Policy::Realistic`] replaces the tags with plausible, non-identifying
//!   values so the file looks unremarkable rather than scrubbed.
//!
//! ## What this crate cannot do
//!
//! It removes *container* metadata. It cannot remove information encoded in the
//! media itself: a photograph still shows the room it was taken in, and audio
//! still carries its room acoustics and background noise. Nor does it touch
//! filesystem timestamps or the filename, both of which are outside the file,
//! callers that care must handle those separately.
//!
//! # In plain words
//!
//! This strips the hidden labels off a file.
//!
//! Photographs and recordings carry information you never typed: where the picture
//! was taken, which phone or microphone made it, what the file was called before,
//! sometimes a name. Removing the sound of a voice and leaving that behind would
//! be pointless, so this takes it out -- not by blanking the fields, but by
//! removing the parts of the file that hold them.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod audio;
mod image;
mod wav;

pub use audio::{clean_audio_file, clean_audio_tags};
pub use image::{clean_image_bytes, clean_image_file, ImageKind};
pub use wav::{clean_wav_bytes, is_wav};

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How aggressively to rewrite metadata.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    /// Remove every tag, leaving nothing behind.
    #[default]
    Strip,
    /// Replace tags with plausible, non-identifying values, so the file does
    /// not stand out by being conspicuously empty.
    Realistic,
}

/// What changed in a single file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Names of the metadata blocks that were removed or rewritten.
    pub removed: Vec<String>,
    /// Whether the file was modified at all.
    pub changed: bool,
}

impl Report {
    /// Record that `what` was taken out, and that the file therefore changed.
    ///
    /// The two go together on purpose. `changed` decides whether anything is
    /// written back, and setting it separately from adding to the list is how a
    /// report ends up naming a removal that did not happen, or missing one that
    /// did.
    fn note(&mut self, what: impl Into<String>) {
        self.removed.push(what.into());
        self.changed = true;
    }
}

use std::path::Path;

/// Replace the file at `path` with one that `fill` makes beside it.
///
/// **F-281.** Cleaning used to rewrite the file where it stood: `fs::write`
/// truncates first and writes after, and the tag path ran several separate
/// edits over the original. A crash, a full disk or a power cut part way
/// through left a truncated file, and the file being cleaned is usually the
/// only copy of a recording somebody cares about.
///
/// Now the new contents go into a fresh file in the same folder, made with
/// `create_new` so nothing already there is followed or overwritten, and
/// that file is renamed over the original only once it is complete. A rename
/// within one folder is all or nothing, so the original is either untouched
/// or fully replaced. `fill` is handed the fresh file's path, empty, and the
/// original keeps its permissions. The fresh name ends with the original's,
/// extension included, because the tag library recognises a format partly by
/// its extension.
pub(crate) fn replace_with(
    path: &Path,
    fill: impl FnOnce(&Path) -> Result<(), Error>,
) -> Result<(), Error> {
    let folder = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let permissions = std::fs::metadata(path)?.permissions();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);

    let mut attempt = 0u32;
    let fresh = loop {
        let candidate = folder.join(format!(".{}-{stamp}-{attempt}.{name}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => break candidate,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && attempt < 16 => {
                attempt += 1;
            }
            Err(e) => return Err(e.into()),
        }
    };

    let made = fill(&fresh)
        .and_then(|()| Ok(std::fs::set_permissions(&fresh, permissions)?))
        .and_then(|()| Ok(std::fs::rename(&fresh, path)?));
    if made.is_err() {
        let _ = std::fs::remove_file(&fresh);
    }
    made
}

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The file could not be read or written.
    Io(std::io::Error),
    /// The file is not a format this crate understands.
    UnsupportedFormat,
    /// The file is structurally malformed.
    Malformed(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "input/output error: {e}"),
            Self::UnsupportedFormat => f.write_str("unsupported media format"),
            Self::Malformed(m) => write!(f, "malformed file: {m}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// F-281. A replacement that fails part way leaves the original exactly as
    /// it was and nothing beside it, and one that succeeds keeps its
    /// permissions.
    #[test]
    fn a_failed_clean_leaves_the_original_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("only copy.wav");
        std::fs::write(&path, b"the recording").unwrap();

        let failed = replace_with(&path, |fresh| {
            std::fs::write(fresh, b"the rec")?;
            Err(Error::Malformed("the disk filled up".into()))
        });
        assert!(failed.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"the recording");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "a part-written file was left beside the original"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        replace_with(&path, |fresh| Ok(std::fs::write(fresh, b"cleaned")?)).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"cleaned");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "the original's permissions were lost");
        }
    }
}
