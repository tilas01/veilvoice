// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! # veilvoice-meta
//!
//! Strip or spoof the identifying metadata that rides along with media files.
//!
//! ## Why this exists
//!
//! De-identifying a voice accomplishes nothing if the file still says who
//! recorded it. A phone recording routinely carries the device model, the
//! recording software, a precise timestamp and — for images — GPS coordinates
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
//! - [`Policy::Strip`] — remove everything. Best when the file is expected to
//!   be sanitised anyway, or when any false statement would be worse than an
//!   obvious absence.
//! - [`Policy::Realistic`] — replace the tags with plausible, non-identifying
//!   values so the file looks unremarkable rather than scrubbed.
//!
//! ## What this crate cannot do
//!
//! It removes *container* metadata. It cannot remove information encoded in the
//! media itself: a photograph still shows the room it was taken in, and audio
//! still carries its room acoustics and background noise. Nor does it touch
//! filesystem timestamps or the filename, both of which are outside the file —
//! callers that care must handle those separately.
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
    fn note(&mut self, what: impl Into<String>) {
        self.removed.push(what.into());
        self.changed = true;
    }
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
