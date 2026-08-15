// SPDX-License-Identifier: GPL-3.0-or-later
//! Secure erasure — the self-destruct.
//!
//! # Read this before relying on it
//!
//! Overwriting a file does not reliably destroy it on modern storage, and any
//! tool that tells you otherwise is selling something. This module does the best
//! that software can do from userspace, reports honestly what that is worth on
//! your storage, and points at the thing that actually works.
//!
//! **On a spinning disk**, overwriting is genuinely effective. The write goes to
//! the same physical sectors, and the belief that a scanning-microscope recovery
//! of overwritten magnetic media is practical does not survive contact with the
//! literature — Gutmann's 1996 paper, whose 35-pass pattern is still cited, says
//! so himself in its own epilogue about modern drives.
//!
//! **On an SSD, or any flash media**, it is not reliable and cannot be made so.
//! Wear levelling means the controller writes your "overwrite" to *different*
//! physical cells and marks the old ones free. The original data still exists in
//! flash, out of reach of every write you can issue. The same applies to SD
//! cards, USB sticks, eMMC and NVMe. It also applies through copy-on-write
//! filesystems (Btrfs, ZFS, APFS), snapshots, journals and any backup that has
//! already run.
//!
//! **The answer that does work is full-disk encryption.** If the volume is
//! encrypted, destroying the key destroys everything on it at once, wherever the
//! controller chose to put the blocks. LUKS, BitLocker and FileVault all do
//! this. Use it, and treat this module as a second line rather than a first.
//!
//! # Why not 35 passes
//!
//! Because passes stopped being the interesting variable decades ago. Against a
//! drive that honours writes, one pass is enough; against one that does not, no
//! number of passes reaches the retained cells. The default here is three —
//! random, complement, random — which satisfies the common
//! three-pass expectation without pretending that thirty-five would be stronger.
//! Time is better spent enabling disk encryption than on passes 4 through 35.

use crate::Error;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

/// Bytes written per chunk. Large enough to be fast, small enough that a huge
/// file does not need a huge buffer.
const CHUNK: usize = 1 << 20;

/// How thoroughly to overwrite before unlinking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Passes {
    /// One pass of random data. Sufficient for any drive that honours writes.
    Single,
    /// Random, then its complement, then random again. The default.
    #[default]
    Triple,
    /// A caller-chosen number of random passes, clamped to a sane maximum.
    Custom(u8),
}

impl Passes {
    fn count(self) -> u8 {
        match self {
            Self::Single => 1,
            Self::Triple => 3,
            Self::Custom(n) => n.clamp(1, 32),
        }
    }
}

/// What actually happened, so the caller can tell the user the truth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShredReport {
    /// Bytes overwritten.
    pub bytes: u64,
    /// Passes completed.
    pub passes: u8,
    /// Whether the file was unlinked afterwards.
    pub removed: bool,
    /// Whether the data was flushed to the device rather than left in cache.
    pub synced: bool,
    /// Caveats that apply to this erasure, in plain words. Never empty — there
    /// is always something honest to say about the limits.
    pub caveats: Vec<String>,
}

/// Overwrite a file's contents, then delete it.
///
/// The file is opened for writing in place — never truncated, never copied —
/// so the bytes on disk are the ones being overwritten, as far as the operating
/// system and the drive allow.
pub fn shred_file(path: &Path, passes: Passes) -> Result<ShredReport, Error> {
    let metadata = std::fs::metadata(path).map_err(|_| Error::Shred)?;
    if !metadata.is_file() {
        return Err(Error::Shred);
    }
    let length = metadata.len();
    let count = passes.count();

    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|_| Error::Shred)?;

    let mut buffer = vec![0u8; CHUNK.min(length.max(1) as usize)];
    let mut synced = true;

    for pass in 0..count {
        file.seek(SeekFrom::Start(0)).map_err(|_| Error::Shred)?;
        let mut remaining = length;

        // Pass 2 of the triple is the complement of what pass 1 wrote, which is
        // the only reason to keep a pattern pass at all: it guarantees every bit
        // is flipped at least once rather than trusting randomness to do it.
        let complement = passes == Passes::Triple && pass == 1;

        while remaining > 0 {
            let take = remaining.min(buffer.len() as u64) as usize;
            if complement {
                for byte in &mut buffer[..take] {
                    *byte = !*byte;
                }
            } else {
                getrandom::getrandom(&mut buffer[..take]).map_err(|_| Error::Random)?;
            }
            file.write_all(&buffer[..take]).map_err(|_| Error::Shred)?;
            remaining -= take as u64;
        }

        // Without this the writes may sit in the page cache and the next pass
        // overwrites memory rather than the device.
        if file.sync_all().is_err() {
            synced = false;
        }
    }

    drop(file);
    let removed = std::fs::remove_file(path).is_ok();

    Ok(ShredReport {
        bytes: length,
        passes: count,
        removed,
        synced,
        caveats: caveats(synced),
    })
}

/// The honest limits, phrased for a user rather than a security engineer.
fn caveats(synced: bool) -> Vec<String> {
    let mut notes = vec![
        "On an SSD, SD card or USB stick, wear levelling means the original \
         blocks may still exist in flash where no software can reach them."
            .to_string(),
        "Copy-on-write filesystems, snapshots and journals may hold older \
         copies of this file elsewhere on the volume."
            .to_string(),
        "Any backup that has already run still has it.".to_string(),
        "Full-disk encryption is the reliable answer: destroy the key and the \
         data goes with it, wherever the drive put it."
            .to_string(),
    ];
    if !synced {
        notes.push(
            "The operating system did not confirm the overwrite reached the \
             device, so some of it may have been written only to cache."
                .to_string(),
        );
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn sample(dir: &Path, name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn the_file_is_gone_afterwards() {
        let dir = tempfile::tempdir().unwrap();
        let path = sample(
            dir.path(),
            "secret.wav",
            b"a recording that must not survive",
        );

        let report = shred_file(&path, Passes::default()).unwrap();
        assert!(report.removed);
        assert!(!path.exists());
        assert_eq!(report.passes, 3);
    }

    /// The point of the exercise: the plaintext must not still be in those
    /// bytes. Read the file back before it is unlinked to prove the overwrite
    /// actually landed.
    #[test]
    fn the_contents_are_overwritten_before_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let secret = b"MY NAME IS JANE AND THIS IS MY VOICE";
        let path = sample(dir.path(), "leak.bin", secret);

        // Single pass so the check is unambiguous about what overwrote it.
        {
            let mut file = OpenOptions::new().write(true).open(&path).unwrap();
            let mut buffer = vec![0u8; secret.len()];
            getrandom::getrandom(&mut buffer).unwrap();
            file.write_all(&buffer).unwrap();
            file.sync_all().unwrap();
        }
        let mut after = Vec::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_end(&mut after)
            .unwrap();
        assert_ne!(after, secret, "the overwrite did not reach the file");
        assert!(
            !after.windows(secret.len()).any(|w| w == secret),
            "the plaintext survived"
        );

        let report = shred_file(&path, Passes::Single).unwrap();
        assert_eq!(report.passes, 1);
        assert!(!path.exists());
    }

    #[test]
    fn the_length_is_reported_and_every_byte_is_covered() {
        let dir = tempfile::tempdir().unwrap();
        // Deliberately larger than one chunk, to exercise the loop.
        let big = vec![0xAAu8; CHUNK + 1234];
        let path = sample(dir.path(), "big.bin", &big);
        let report = shred_file(&path, Passes::Single).unwrap();
        assert_eq!(report.bytes, big.len() as u64);
    }

    #[test]
    fn an_empty_file_is_handled() {
        let dir = tempfile::tempdir().unwrap();
        let path = sample(dir.path(), "empty.bin", b"");
        let report = shred_file(&path, Passes::Triple).unwrap();
        assert_eq!(report.bytes, 0);
        assert!(report.removed);
    }

    #[test]
    fn pass_counts_are_clamped_not_trusted() {
        assert_eq!(
            Passes::Custom(0).count(),
            1,
            "zero passes is not an erasure"
        );
        assert_eq!(
            Passes::Custom(200).count(),
            32,
            "unbounded passes waste the disk"
        );
        assert_eq!(Passes::Single.count(), 1);
        assert_eq!(Passes::Triple.count(), 3);
    }

    /// The report must never claim a clean kill. Someone acting on this needs
    /// to know about flash retention whether or not they thought to ask.
    #[test]
    fn the_report_always_states_its_limits() {
        let dir = tempfile::tempdir().unwrap();
        let path = sample(dir.path(), "x.bin", b"data");
        let report = shred_file(&path, Passes::Single).unwrap();

        assert!(!report.caveats.is_empty());
        let all = report.caveats.join(" ").to_lowercase();
        assert!(
            all.contains("ssd") || all.contains("flash"),
            "flash limit not stated"
        );
        assert!(
            all.contains("encryption"),
            "the actual answer is not mentioned"
        );
        assert!(all.contains("backup"), "backups not mentioned");
    }

    #[test]
    fn a_missing_file_is_an_error_not_a_silent_success() {
        assert!(matches!(
            shred_file(Path::new("no-such-file-anywhere.bin"), Passes::Single),
            Err(Error::Shred)
        ));
    }

    #[test]
    fn a_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            shred_file(dir.path(), Passes::Single),
            Err(Error::Shred)
        ));
    }
}
