// SPDX-License-Identifier: GPL-3.0-or-later
//! Reading a `zip` central directory and hashing every file in it.
//!
//! # Why this is written here rather than taken from a crate
//!
//! The same reason as [`super::tar`], and one more. The `zip` crate carries
//! decoders for bzip2, zstd, LZMA and the various AES encryptions, and this
//! reader can never reach any of them: a release archive holds stored and
//! deflated members and nothing else. Linking the lot into the one binary
//! whose smallness is a stated feature, to use two of its cases, is a cost
//! paid for nothing and an attack surface reachable only through input this
//! refuses anyway.
//!
//! What is not written here is the inflating, which is `flate2` -- already in
//! this binary underneath `pgp`, so it costs nothing to reach for and is read
//! by a great many people.
//!
//! # The central directory, not the local headers
//!
//! Members are enumerated from the central directory at the end of the file,
//! which is where a zip's own index lives and what every unpacking tool reads.
//! Walking the local headers forwards instead would disagree with the tool the
//! reader will actually use, and a verifier that checks a different set of
//! files from the one that will be unpacked has checked nothing useful.
//!
//! It also sidesteps the data-descriptor case outright: a member written by a
//! streaming producer has zeroes for its sizes in the local header and the
//! real ones only in the central directory.
//!
//! # In plain words
//!
//! Opens a `.zip` and works out the fingerprint of every file inside it,
//! without unpacking anything.

use std::io::{Read, Seek, SeekFrom};

use crate::check::Error;

/// One member of a zip.
pub enum Member {
    /// An ordinary file, with its path and the SHA-256 of its contents.
    File {
        /// Where it sits inside the archive.
        path: String,
        /// Its SHA-256, lowercase hex.
        digest: String,
    },
    /// Something at a path this reader will not hash, and why.
    ///
    /// Reported rather than dropped, for the reason in [`super::tar::Member`]:
    /// a published path holding something other than a plain file is a finding.
    NotAFile {
        /// Where it sits inside the archive.
        path: String,
        /// What it is, in words.
        what: &'static str,
    },
}

/// The end-of-central-directory record.
const END: u32 = 0x0605_4b50;
/// The zip64 end-of-central-directory locator.
const ZIP64_LOCATOR: u32 = 0x0706_4b50;
/// The zip64 end-of-central-directory record.
const ZIP64_END: u32 = 0x0606_4b50;
/// A central-directory file header.
const CENTRAL: u32 = 0x0201_4b50;
/// A local file header.
const LOCAL: u32 = 0x0403_4b50;

/// A value written as all-ones, meaning "the real one is in a zip64 field".
const SENTINEL_32: u32 = 0xFFFF_FFFF;
/// The same, in sixteen bits.
const SENTINEL_16: u16 = 0xFFFF;

/// Read every member of a zip.
pub fn members(file: &mut (impl Read + Seek)) -> Result<Vec<Member>, Error> {
    let length = file
        .seek(SeekFrom::End(0))
        .map_err(|e| Error::Io(format!("cannot read the archive: {e}")))?;
    let directory = find_directory(file, length)?;

    let mut out = Vec::new();
    let mut at = directory.offset;
    for _ in 0..directory.entries {
        let header = read_at(file, at, 46)?;
        if word(&header, 0) != CENTRAL {
            return Err(Error::Malformed(
                "the archive's index does not begin where it says it does: \
                 this is not a zip file, or it has been damaged"
                    .to_string(),
            ));
        }
        let flags = half(&header, 8);
        let method = half(&header, 10);
        let mut compressed = u64::from(word(&header, 20));
        let mut uncompressed = u64::from(word(&header, 24));
        let name_len = usize::from(half(&header, 28));
        let extra_len = usize::from(half(&header, 30));
        let comment_len = usize::from(half(&header, 32));
        let mut local = u64::from(word(&header, 42));

        let tail = read_at(file, at + 46, name_len + extra_len)?;
        let raw = String::from_utf8_lossy(&tail[..name_len]).into_owned();

        // A zip64 extra field carries the real values for whichever of these
        // were written as all-ones. The order inside it is fixed and the
        // fields present are exactly those that were sentinelled, so it is
        // read in that order rather than by looking each one up.
        if uncompressed == u64::from(SENTINEL_32)
            || compressed == u64::from(SENTINEL_32)
            || local == u64::from(SENTINEL_32)
        {
            let extra = &tail[name_len..];
            let mut wanted: Vec<&mut u64> = Vec::new();
            if uncompressed == u64::from(SENTINEL_32) {
                wanted.push(&mut uncompressed);
            }
            if compressed == u64::from(SENTINEL_32) {
                wanted.push(&mut compressed);
            }
            if local == u64::from(SENTINEL_32) {
                wanted.push(&mut local);
            }
            fill_from_zip64(extra, wanted)?;
        }

        at += 46 + (name_len + extra_len + comment_len) as u64;

        let Some(path) = tidy(&raw) else {
            // A directory entry, which carries no bytes. The manifest lists
            // none, so there is nothing to compare.
            continue;
        };

        // Bit 0 is the old password encryption, bit 6 its stronger form. Both
        // mean the bytes on disk are not the bytes of the file, so a hash of
        // them would be a hash of something nobody published.
        if flags & 0b1 != 0 || flags & 0b100_0000 != 0 {
            out.push(Member::NotAFile {
                path,
                what: "encrypted, so its contents cannot be read",
            });
            continue;
        }

        let digest = match method {
            0 => hash_stored(file, local, compressed)?,
            8 => hash_deflated(file, local, compressed)?,
            _ => {
                out.push(Member::NotAFile {
                    path,
                    what: "compressed in a way a release never uses",
                });
                continue;
            }
        };
        let _ = uncompressed;
        out.push(Member::File { path, digest });
    }
    Ok(out)
}

/// Where the central directory is, and how many entries it holds.
struct Directory {
    /// Byte offset of the first central-directory header.
    offset: u64,
    /// How many headers follow it.
    entries: u64,
}

/// Find the end-of-central-directory record, and the zip64 one behind it.
///
/// The record is at the very end unless the archive carries a comment, so the
/// search runs backwards over the largest a comment can be. Backwards, because
/// the signature can legitimately appear inside a member's compressed bytes and
/// the last one is the real one.
fn find_directory(file: &mut (impl Read + Seek), length: u64) -> Result<Directory, Error> {
    const RECORD: u64 = 22;
    const LARGEST_COMMENT: u64 = 0xFFFF;
    if length < RECORD {
        return Err(Error::Malformed(
            "the archive is too short to be a zip file".to_string(),
        ));
    }
    let window = (RECORD + LARGEST_COMMENT).min(length);
    let from = length - window;
    let tail = read_at(file, from, usize::try_from(window).unwrap_or(0))?;

    let mut end = None;
    for start in (0..=tail.len() - RECORD as usize).rev() {
        if word(&tail, start) == END {
            end = Some(start);
            break;
        }
    }
    let Some(end) = end else {
        return Err(Error::Malformed(
            "the archive has no index at the end of it: this is not a zip file, \
             or it has been damaged"
                .to_string(),
        ));
    };

    let entries = half(&tail, end + 10);
    let offset = word(&tail, end + 16);
    if entries != SENTINEL_16 && offset != SENTINEL_32 {
        return Ok(Directory {
            offset: u64::from(offset),
            entries: u64::from(entries),
        });
    }

    // A zip64 archive. The locator sits immediately before the record above,
    // and points at the real one.
    if end < 20 {
        return Err(Error::Malformed(
            "the archive says it is a zip64 file and carries no zip64 index".to_string(),
        ));
    }
    let locator = end - 20;
    if word(&tail, locator) != ZIP64_LOCATOR {
        return Err(Error::Malformed(
            "the archive says it is a zip64 file and carries no zip64 index".to_string(),
        ));
    }
    let real = long(&tail, locator + 8);
    let record = read_at(file, real, 56)?;
    if word(&record, 0) != ZIP64_END {
        return Err(Error::Malformed(
            "the archive's zip64 index is not where it says it is".to_string(),
        ));
    }
    Ok(Directory {
        offset: long(&record, 48),
        entries: long(&record, 32),
    })
}

/// Take the values a zip64 extra field carries, in the order it carries them.
fn fill_from_zip64(extra: &[u8], wanted: Vec<&mut u64>) -> Result<(), Error> {
    let mut at = 0usize;
    while at + 4 <= extra.len() {
        let tag = half(extra, at);
        let size = usize::from(half(extra, at + 2));
        let body = extra
            .get(at + 4..at + 4 + size)
            .ok_or_else(|| Error::Malformed("the archive's index is cut short".to_string()))?;
        if tag == 0x0001 {
            for (number, slot) in wanted.into_iter().enumerate() {
                let start = number * 8;
                if start + 8 > body.len() {
                    return Err(Error::Malformed(
                        "the archive's zip64 index does not carry the sizes it promised"
                            .to_string(),
                    ));
                }
                *slot = long(body, start);
            }
            return Ok(());
        }
        at += 4 + size;
    }
    Err(Error::Malformed(
        "a member of the archive says its size is elsewhere and does not say where".to_string(),
    ))
}

/// Where one member's bytes begin, read from its own local header.
///
/// The name and extra lengths in a local header can differ from the central
/// directory's, which is why they are read here rather than reused.
fn data_begins(file: &mut (impl Read + Seek), local: u64) -> Result<u64, Error> {
    let header = read_at(file, local, 30)?;
    if word(&header, 0) != LOCAL {
        return Err(Error::Malformed(
            "a member of the archive is not where the index says it is".to_string(),
        ));
    }
    let name_len = u64::from(half(&header, 26));
    let extra_len = u64::from(half(&header, 28));
    Ok(local + 30 + name_len + extra_len)
}

/// Hash a member stored without compression.
fn hash_stored(file: &mut (impl Read + Seek), local: u64, length: u64) -> Result<String, Error> {
    use sha2::{Digest, Sha256};

    let begins = data_begins(file, local)?;
    seek(file, begins)?;
    let mut hasher = Sha256::new();
    let mut left = length;
    let mut buffer = vec![0u8; 64 * 1024];
    while left > 0 {
        let want = usize::try_from(left.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = file
            .read(&mut buffer[..want])
            .map_err(|e| Error::Io(format!("cannot read the archive: {e}")))?;
        if read == 0 {
            return Err(Error::Malformed(
                "a member of the archive is shorter than its index says: it is truncated"
                    .to_string(),
            ));
        }
        hasher.update(&buffer[..read]);
        left -= read as u64;
    }
    Ok(crate::check::hex_of(&hasher.finalize()))
}

/// Hash a deflated member, decompressing as it goes.
///
/// Streamed rather than read whole. A release holds binaries of tens of
/// megabytes and there is no reason for a verifier to hold one in memory, let
/// alone two -- the compressed copy and the decompressed one.
fn hash_deflated(file: &mut (impl Read + Seek), local: u64, length: u64) -> Result<String, Error> {
    use sha2::{Digest, Sha256};

    let begins = data_begins(file, local)?;
    seek(file, begins)?;
    let mut decoder = flate2::read::DeflateDecoder::new(file.take(length));
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = decoder.read(&mut buffer).map_err(|e| {
            Error::Malformed(format!(
                "a member of the archive could not be decompressed: {e}"
            ))
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(crate::check::hex_of(&hasher.finalize()))
}

/// Read a run of bytes from one place, without disturbing the caller's idea of
/// where it was.
fn read_at(file: &mut (impl Read + Seek), at: u64, length: usize) -> Result<Vec<u8>, Error> {
    seek(file, at)?;
    let mut buffer = vec![0u8; length];
    file.read_exact(&mut buffer).map_err(|e| {
        Error::Malformed(format!(
            "the archive ends before {at}, where its index says something is: {e}"
        ))
    })?;
    Ok(buffer)
}

/// Move to a byte offset, with the offset in the error.
fn seek(file: &mut impl Seek, at: u64) -> Result<(), Error> {
    file.seek(SeekFrom::Start(at))
        .map(|_| ())
        .map_err(|e| Error::Io(format!("cannot read the archive at {at}: {e}")))
}

/// A little-endian `u16` at an offset, or zero past the end.
///
/// Out of range reads zero rather than panicking. Every field read here is
/// bounds-checked by the `read_at` that fetched the block it is in, so a zero
/// can only come from a block that was short, and the signature check on that
/// block refuses it.
fn half(bytes: &[u8], at: usize) -> u16 {
    let Some(slice) = bytes.get(at..at + 2) else {
        return 0;
    };
    u16::from_le_bytes([slice[0], slice[1]])
}

/// A little-endian `u32` at an offset, or zero past the end.
fn word(bytes: &[u8], at: usize) -> u32 {
    let Some(slice) = bytes.get(at..at + 4) else {
        return 0;
    };
    u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
}

/// A little-endian `u64` at an offset, or zero past the end.
fn long(bytes: &[u8], at: usize) -> u64 {
    let Some(slice) = bytes.get(at..at + 8) else {
        return 0;
    };
    let mut eight = [0u8; 8];
    eight.copy_from_slice(slice);
    u64::from_le_bytes(eight)
}

/// A member path, as the manifest records it, or `None` for a directory.
///
/// The backslash is turned into a forward slash because a zip written on
/// Windows may hold either and `tools/release/contents.py` normalises the same
/// way. Nothing else is rewritten: see the note on [`super::tar`]'s `tidy`.
fn tidy(raw: &str) -> Option<String> {
    let name = raw.replace('\\', "/");
    let name = name.strip_prefix("./").unwrap_or(&name);
    if name.is_empty() || name.ends_with('/') {
        return None;
    }
    Some(name.to_string())
}
