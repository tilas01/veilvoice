// SPDX-License-Identifier: GPL-3.0-or-later
//! Walking a `tar` stream and hashing every regular file in it.
//!
//! # Why this is written here rather than taken from a crate
//!
//! A `tar` reader is a loop over 512-byte headers, and the whole of what this
//! one needs to do is name each regular file and hash its bytes. It never
//! creates a file, never follows a link, never sets a mode and never writes a
//! path to disk, so the entire class of defect that makes archive extraction
//! dangerous -- a member called `../../etc/profile`, a symbolic link pointing
//! out of the tree, a mode with the setuid bit on -- is absent by
//! construction rather than defended against. A member path here is a string
//! that gets compared against a signed manifest and then thrown away.
//!
//! The one part that is genuinely hard, and the one part not written here, is
//! the decompression. That is `flate2`, which is already in this binary
//! underneath `pgp` and is read by a great many people.
//!
//! # What it accepts
//!
//! Regular files, in the three ways `tar` spells a long name: the plain
//! `ustar` `prefix`/`name` pair, GNU's `L` record, and a `pax` extended header
//! carrying `path=`. GNU `tar` writes the second and `bsdtar` the third, and
//! this project's releases are built on five machines, so all three turn up.
//!
//! Anything else -- a directory, a link, a device, a member whose header
//! checksum does not add up -- is reported as what it is rather than skipped.
//! A verifier that silently ignores what it does not understand is a verifier
//! that passes an archive it never read.
//!
//! # In plain words
//!
//! Opens a `.tar.gz` and works out the fingerprint of every file inside it,
//! without unpacking anything.

use std::io::Read;

use crate::check::Error;

/// One member of a tar stream.
pub enum Member {
    /// An ordinary file, with its path and the SHA-256 of its contents.
    File {
        /// Where it sits inside the archive.
        path: String,
        /// Its SHA-256, lowercase hex.
        digest: String,
    },
    /// Something at a path that is not an ordinary file.
    ///
    /// Reported rather than dropped. The contents list published by a release
    /// holds only regular files, so a link or a device at a published path is
    /// a difference the reader has to be told about: see `Verdict::NotAFile`
    /// in [`crate::check::contents`], which is the same finding on disk.
    NotAFile {
        /// Where it sits inside the archive.
        path: String,
        /// What it is, in words: "a symbolic link", "a hard link".
        what: &'static str,
    },
}

/// A tar header block, before any of it is believed.
const BLOCK: usize = 512;

/// Read every member of a tar stream.
///
/// The stream is read once, start to end, and nothing is seeked: a `.tar.gz`
/// is decompressed as it goes, and a decompressor cannot be rewound cheaply.
/// Members come back in the order the archive holds them.
pub fn members(mut stream: impl Read) -> Result<Vec<Member>, Error> {
    let mut out = Vec::new();
    // A long name read from a `L` record or a `pax` header applies to the
    // member that follows it, and to that one only.
    let mut pending_name: Option<String> = None;
    let mut zero_blocks = 0usize;

    loop {
        let mut header = [0u8; BLOCK];
        match fill(&mut stream, &mut header)? {
            // The stream ended without the two zero blocks that finish a tar.
            // Truncation, in other words, and an archive that stops early is
            // an archive whose remaining members were never seen.
            Filled::Eof => {
                if zero_blocks > 0 {
                    // One zero block and then the end. Sloppy, and every
                    // reader accepts it, because the padding after it is what
                    // is missing rather than any data.
                    return Ok(out);
                }
                return Err(Error::Malformed(
                    "the archive ends in the middle of it: the last member is cut short"
                        .to_string(),
                ));
            }
            Filled::Short => {
                return Err(Error::Malformed(
                    "the archive ends in the middle of a header".to_string(),
                ))
            }
            Filled::Whole => {}
        }

        if header.iter().all(|byte| *byte == 0) {
            zero_blocks += 1;
            if zero_blocks == 2 {
                return Ok(out);
            }
            continue;
        }
        // A zero block followed by data is not the end marker. Reset rather
        // than remember it, so a stray run of zeroes cannot end the walk early.
        zero_blocks = 0;

        if !checksum_agrees(&header) {
            return Err(Error::Malformed(
                "a header inside the archive does not add up to its own checksum: \
                 this is not a tar file, or it has been damaged"
                    .to_string(),
            ));
        }

        let size = octal(&header[124..136]).ok_or_else(|| {
            Error::Malformed("a member inside the archive has no readable size".to_string())
        })?;
        let flag = header[156];

        // GNU's long name, and its long link name. The member's *data* is the
        // name of the next member, so it is read and kept rather than hashed.
        if flag == b'L' || flag == b'K' {
            let data = read_member(&mut stream, size)?;
            if flag == b'L' {
                pending_name = Some(trim_nul(&data));
            }
            continue;
        }

        // A pax extended header: length-prefixed `key=value` records, of which
        // `path` is the only one that concerns a reader who is not unpacking.
        if flag == b'x' || flag == b'X' {
            let data = read_member(&mut stream, size)?;
            if let Some(path) = pax_path(&data) {
                pending_name = Some(path);
            }
            continue;
        }
        // A global pax header applies to every member after it. This project's
        // archives carry none, and honouring a global `path` would be wrong
        // anyway: it would rename every file in the archive to one name.
        if flag == b'g' {
            let _ = read_member(&mut stream, size)?;
            continue;
        }

        let name = pending_name.take().unwrap_or_else(|| name_from(&header));

        match flag {
            // A regular file, in both of the two spellings. `\0` is the
            // original one and `0` the ustar one, and both are still written.
            b'0' | b'\0' => {
                let digest = hash_member(&mut stream, size)?;
                if let Some(path) = tidy(&name) {
                    out.push(Member::File { path, digest });
                }
            }
            // Directories carry no bytes, so there is nothing to compare and
            // the manifest lists none. Skipped, not reported.
            b'5' => skip(&mut stream, size + padding(size))?,
            b'1' | b'2' => {
                skip(&mut stream, size + padding(size))?;
                if let Some(path) = tidy(&name) {
                    out.push(Member::NotAFile {
                        path,
                        what: if flag == b'1' {
                            "a hard link"
                        } else {
                            "a symbolic link"
                        },
                    });
                }
            }
            _ => {
                skip(&mut stream, size + padding(size))?;
                if let Some(path) = tidy(&name) {
                    out.push(Member::NotAFile {
                        path,
                        what: "not an ordinary file",
                    });
                }
            }
        }
    }
}

/// How much of a block was there.
enum Filled {
    /// All of it.
    Whole,
    /// Nothing: the stream ended cleanly on a block boundary.
    Eof,
    /// Some of it, which means a truncated archive.
    Short,
}

/// Read exactly one block, reporting how the stream ended if it did.
fn fill(stream: &mut impl Read, block: &mut [u8; BLOCK]) -> Result<Filled, Error> {
    let mut filled = 0usize;
    while filled < BLOCK {
        let read = stream
            .read(&mut block[filled..])
            .map_err(|e| Error::Io(format!("cannot read the archive: {e}")))?;
        if read == 0 {
            return Ok(if filled == 0 {
                Filled::Eof
            } else {
                Filled::Short
            });
        }
        filled += read;
    }
    Ok(Filled::Whole)
}

/// The header's own checksum, which is the sum of its bytes with the checksum
/// field read as spaces.
///
/// Both spellings are accepted. The field is written as signed bytes by a few
/// very old implementations and unsigned by everything since, and a reader
/// that insists on one of them rejects real archives.
fn checksum_agrees(header: &[u8; BLOCK]) -> bool {
    let Some(stated) = octal(&header[148..156]) else {
        return false;
    };
    let mut unsigned = 0u64;
    let mut signed = 0i64;
    for (at, byte) in header.iter().enumerate() {
        let value = if (148..156).contains(&at) {
            b' '
        } else {
            *byte
        };
        unsigned += u64::from(value);
        signed += i64::from(value as i8);
    }
    stated == unsigned || i64::try_from(stated).map(|s| s == signed).unwrap_or(false)
}

/// An octal field, NUL- or space-terminated.
///
/// Returns `None` rather than zero for a field that cannot be read, so a
/// malformed size is a refusal rather than a member silently treated as empty.
fn octal(field: &[u8]) -> Option<u64> {
    // Base-256, for a size a twelve-character octal field cannot hold. This
    // project ships nothing near 8 GB, so it is refused plainly rather than
    // decoded: a reader that guesses here would be guessing about how many
    // bytes to skip, and every member after it would be misread.
    if field.first().is_some_and(|first| first & 0x80 != 0) {
        return None;
    }
    let text: String = field
        .iter()
        .take_while(|byte| **byte != 0 && **byte != b' ')
        .map(|byte| char::from(*byte))
        .collect();
    let text = text.trim();
    if text.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(text, 8).ok()
}

/// The member's path, from the `ustar` `prefix` and `name` fields.
fn name_from(header: &[u8; BLOCK]) -> String {
    let name = trim_nul(&header[0..100]);
    // The `prefix` field is only meaningful in a `ustar` archive. In the
    // original format those bytes are padding, and reading them as a directory
    // would prepend rubbish to every path.
    let ustar = &header[257..263] == b"ustar\0" || &header[257..262] == b"ustar";
    if !ustar {
        return name;
    }
    let prefix = trim_nul(&header[345..500]);
    if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    }
}

/// A NUL-padded field as text.
fn trim_nul(field: &[u8]) -> String {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

/// The `path=` record of a pax extended header.
///
/// The format is `<length> <key>=<value>\n`, where `<length>` counts the whole
/// record including itself. Anything that does not parse is passed over: a pax
/// header this cannot read is not a reason to refuse an archive, because the
/// member's ordinary name is still in the header block that follows.
fn pax_path(data: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(data);
    let mut rest = text.as_ref();
    while !rest.is_empty() {
        let (length, after) = rest.split_once(' ')?;
        let length: usize = length.trim().parse().ok()?;
        // The length counts from the start of this record, so the body is what
        // is left of it after the digits and the space.
        let digits = rest.len() - after.len();
        let body = after.get(..length.checked_sub(digits)?)?;
        if let Some(value) = body.strip_prefix("path=") {
            return Some(value.trim_end_matches('\n').to_string());
        }
        rest = &rest[length..];
    }
    None
}

/// Hash one member's bytes, and step over the padding after it.
fn hash_member(stream: &mut impl Read, size: u64) -> Result<String, Error> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    let mut left = size;
    let mut buffer = vec![0u8; 64 * 1024];
    while left > 0 {
        let want = usize::try_from(left.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = stream
            .read(&mut buffer[..want])
            .map_err(|e| Error::Io(format!("cannot read the archive: {e}")))?;
        if read == 0 {
            return Err(Error::Malformed(
                "a member inside the archive is shorter than its header says: \
                 the archive is truncated"
                    .to_string(),
            ));
        }
        hasher.update(&buffer[..read]);
        left -= read as u64;
    }
    skip(stream, padding(size))?;
    Ok(crate::check::hex_of(&hasher.finalize()))
}

/// Read one member's bytes into memory, for the two headers that carry text.
///
/// Bounded rather than trusted: a name is a name, and a header claiming a
/// gigabyte of it is a header this refuses rather than allocates for.
fn read_member(stream: &mut impl Read, size: u64) -> Result<Vec<u8>, Error> {
    const MOST: u64 = 64 * 1024;
    if size > MOST {
        return Err(Error::Malformed(format!(
            "the archive holds a {size}-byte file name, which is not a file name"
        )));
    }
    let mut data = vec![0u8; usize::try_from(size).unwrap_or(0)];
    stream
        .read_exact(&mut data)
        .map_err(|e| Error::Io(format!("cannot read the archive: {e}")))?;
    skip(stream, padding(size))?;
    Ok(data)
}

/// How many bytes of padding follow a member of this size.
fn padding(size: u64) -> u64 {
    (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64
}

/// Step over exactly this many bytes without keeping them.
///
/// The count is given rather than worked out here. A caller stepping over a
/// member passes its size **and** its padding; a caller who has just hashed a
/// member passes the padding alone. Deriving one from the other inside this
/// function would mean guessing which of the two a number is, and a tar reader
/// that misjudges how far to move lands in the middle of the next header and
/// misreads every member after it.
fn skip(stream: &mut impl Read, bytes: u64) -> Result<(), Error> {
    let mut left = bytes;
    let mut buffer = vec![0u8; 64 * 1024];
    while left > 0 {
        let want = usize::try_from(left.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = stream
            .read(&mut buffer[..want])
            .map_err(|e| Error::Io(format!("cannot read the archive: {e}")))?;
        if read == 0 {
            return Err(Error::Malformed(
                "the archive ends in the middle of a member: it is truncated".to_string(),
            ));
        }
        left -= read as u64;
    }
    Ok(())
}

/// A member path, as the manifest records it, or `None` for one with no name.
///
/// The same rule `tools/release/contents.py` writes with, stated here so the
/// two ends of the seam agree: exactly one leading `./` removed, and a
/// trailing `/` means a directory rather than a file. Nothing else is
/// rewritten. A path this cannot tidy is left exactly as it is and compared as
/// it is, because sanitising a path into one that matches is how a verifier
/// reports a pass on a file that is not there.
fn tidy(raw: &str) -> Option<String> {
    let name = raw.strip_prefix("./").unwrap_or(raw);
    if name.is_empty() || name.ends_with('/') {
        return None;
    }
    Some(name.to_string())
}
