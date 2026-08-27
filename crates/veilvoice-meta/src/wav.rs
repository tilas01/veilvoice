// SPDX-License-Identifier: GPL-3.0-or-later
//! Chunk-level RIFF/WAVE metadata removal.
//!
//! # Why WAV gets its own path
//!
//! `lofty` handles tags in every other container, but it cannot remove an
//! ID3v2 block from a WAV file — the attempt fails with an encoding error and
//! the tag stays put. Silently leaving metadata in place is exactly the failure
//! this crate exists to prevent, and WAV is the format VeilVoice writes itself,
//! so it gets a direct implementation rather than a caveat.
//!
//! # Whitelist, not blacklist
//!
//! A RIFF file is a flat list of chunks, and metadata hides in a lot of them:
//! `LIST`/`INFO` (artist, software, comments), `id3 ` and `ID3 `, `bext`
//! (broadcast extension — originator, date, even a coding history), `iXML`,
//! `_PMX` (XMP), `axml`, `cart`. Enumerating those would leave every chunk
//! nobody thought of, and new ones keep being invented.
//!
//! So this keeps only the chunks needed to decode the audio and drops
//! everything else. Anything unrecognised is discarded by default, which is the
//! right bias for a privacy tool: the worst case is a lost non-essential chunk,
//! not a leaked identity.
//!
//! # In plain words
//!
//! Strips the hidden information out of a WAV file specifically.
//!
//! WAV is built as a series of labelled sections, and the ones carrying the sound
//! sit alongside ones carrying text somebody or some program wrote. Those are
//! removed and the sound is copied through exactly, byte for byte.
//!
//! It gets its own path because WAV is what VeilVoice writes, so this is the one
//! that runs on nearly every file it produces, and doing it directly means not
//! handing VeilVoice's own output to a general-purpose parser.

use crate::{Error, Policy, Report};

/// Chunks required to interpret the audio. Everything else goes.
const KEEP: &[&[u8; 4]] = &[
    b"fmt ", // sample format — mandatory
    b"data", // the samples themselves — mandatory
    b"fact", // sample count, required for non-PCM encodings
];

/// Tags written in [`Policy::Realistic`] mode, as `LIST`/`INFO` sub-chunks.
const REALISTIC_INFO: &[(&[u8; 4], &str)] = &[
    (b"INAM", "Audio"),
    (b"IART", "Unknown Artist"),
    (b"ISFT", "Lavf58.76.100"),
];

/// Whether `bytes` looks like a RIFF/WAVE file.
pub fn is_wav(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE"
}

/// Rewrite a WAV, keeping only the chunks needed to decode it.
pub fn clean_wav_bytes(bytes: &[u8], policy: Policy) -> Result<(Vec<u8>, Report), Error> {
    if !is_wav(bytes) {
        return Err(Error::UnsupportedFormat);
    }
    let declared = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    // Trust the file's actual length over the header, which is routinely wrong
    // in streamed or truncated recordings.
    //
    // `saturating_add` rather than `+`: on a 32-bit target — and VeilVoice
    // ships an ARMv7 build — `declared` can be `u32::MAX`, where `declared + 8`
    // overflows `usize` and panics under overflow checks. A 64-bit host cannot
    // reach it, which is exactly why the fuzzer in `tests/wav_fuzz.rs` never
    // will either; this one had to be found by reading. Saturating is also the
    // right answer semantically, since the value is immediately clamped to the
    // real length anyway.
    let end = declared.saturating_add(8).min(bytes.len());

    let mut report = Report::default();
    let mut body: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut pos = 12;

    while pos + 8 <= end {
        let id: [u8; 4] = bytes[pos..pos + 4].try_into().expect("4 bytes");
        let size =
            u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().expect("4 bytes")) as usize;
        let data_start = pos + 8;
        // A malformed size must not run off the end or wrap around.
        let data_end = match data_start.checked_add(size) {
            Some(e) if e <= end => e,
            _ => {
                return Err(Error::Malformed(format!(
                    "chunk {} overruns the file",
                    show(&id)
                )))
            }
        };

        if KEEP.contains(&&id) {
            body.extend_from_slice(&bytes[pos..data_end]);
            // RIFF chunks are word aligned; preserve the pad byte.
            if size % 2 == 1 && data_end < end {
                body.push(bytes[data_end]);
            }
        } else {
            report.note(show(&id));
        }

        pos = data_end + (size % 2);
    }

    if body.is_empty() {
        return Err(Error::Malformed("no audio chunks found".into()));
    }

    if policy == Policy::Realistic {
        body.extend_from_slice(&info_chunk());
        report.changed = true;
    }

    // The RIFF size field is a `u32`, so a body that does not fit in one cannot
    // be described by the format at all. `as u32` would have wrapped and
    // written a size that does not match the file — a silently corrupt WAV
    // handed back as if it were clean, which for a *metadata cleaner* means
    // the user believes a file is safe when it will not even open. Refuse
    // instead. Only reachable for a body at or above 4 GiB, which is past what
    // RIFF can express in the first place.
    let riff_size = match u32::try_from(body.len() + 4) {
        Ok(size) => size,
        Err(_) => {
            return Err(Error::Malformed(
                "the cleaned audio is larger than the RIFF format can describe (4 GiB)".into(),
            ))
        }
    };

    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(&body);
    Ok((out, report))
}

/// Build a bland `LIST`/`INFO` chunk.
fn info_chunk() -> Vec<u8> {
    let mut info = Vec::from(*b"INFO");
    for (id, value) in REALISTIC_INFO {
        // INFO strings are NUL-terminated and word-aligned.
        let mut text = value.as_bytes().to_vec();
        text.push(0);
        if text.len() % 2 == 1 {
            text.push(0);
        }
        info.extend_from_slice(*id);
        info.extend_from_slice(&(text.len() as u32).to_le_bytes());
        info.extend_from_slice(&text);
    }
    let mut chunk = Vec::from(*b"LIST");
    chunk.extend_from_slice(&(info.len() as u32).to_le_bytes());
    chunk.extend_from_slice(&info);
    chunk
}

fn show(id: &[u8; 4]) -> String {
    String::from_utf8_lossy(id).trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut c = Vec::from(*id);
        c.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        c.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            c.push(0);
        }
        c
    }

    /// A WAV carrying audio plus several places metadata likes to hide.
    fn dirty_wav() -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(&chunk(
            b"fmt ",
            &[1, 0, 1, 0, 128, 187, 0, 0, 0, 119, 1, 0, 2, 0, 16, 0],
        ));
        body.extend_from_slice(&chunk(b"LIST", b"INFOIART\x0f\x00\x00\x00Jane Real Name\0"));
        body.extend_from_slice(&chunk(
            b"id3 ",
            b"ID3\x03\x00\x00\x00\x00\x00\x00TPE1 Jane Real Name",
        ));
        body.extend_from_slice(&chunk(b"bext", b"Originator: SomePhone SN#12345"));
        body.extend_from_slice(&chunk(b"data", &[1u8, 2, 3, 4, 5, 6, 7, 8]));
        body.extend_from_slice(&chunk(
            b"iXML",
            b"<BWFXML><PROJECT>Secret</PROJECT></BWFXML>",
        ));

        let mut out = Vec::from(*b"RIFF");
        out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(&body);
        out
    }

    fn find(bytes: &[u8], needle: &[u8]) -> bool {
        bytes.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn recognises_wav() {
        assert!(is_wav(&dirty_wav()));
        assert!(!is_wav(b"not a wav file at all"));
        assert!(!is_wav(b"RIFF____AVI "));
    }

    #[test]
    fn every_metadata_chunk_is_dropped() {
        let (clean, report) = clean_wav_bytes(&dirty_wav(), Policy::Strip).unwrap();
        assert!(report.changed);
        for id in ["LIST", "id3", "bext", "iXML"] {
            assert!(report.removed.iter().any(|r| r == id), "{id} not reported");
            assert!(!find(&clean, id.as_bytes()), "{id} chunk survived");
        }
    }

    #[test]
    fn identifying_strings_are_gone() {
        let (clean, _) = clean_wav_bytes(&dirty_wav(), Policy::Strip).unwrap();
        for needle in [&b"Jane Real Name"[..], b"SomePhone", b"SN#12345", b"Secret"] {
            assert!(
                !find(&clean, needle),
                "{} survived",
                String::from_utf8_lossy(needle)
            );
        }
    }

    #[test]
    fn audio_and_format_survive_intact() {
        let (clean, _) = clean_wav_bytes(&dirty_wav(), Policy::Strip).unwrap();
        assert!(is_wav(&clean));
        assert!(find(&clean, b"fmt "), "format chunk lost");
        assert!(find(&clean, b"data"), "audio chunk lost");
        assert!(find(&clean, &[1u8, 2, 3, 4, 5, 6, 7, 8]), "samples lost");
    }

    #[test]
    fn the_riff_size_header_is_corrected() {
        let (clean, _) = clean_wav_bytes(&dirty_wav(), Policy::Strip).unwrap();
        let declared = u32::from_le_bytes(clean[4..8].try_into().unwrap()) as usize;
        assert_eq!(
            declared + 8,
            clean.len(),
            "RIFF size must match the new length"
        );
    }

    #[test]
    fn unknown_chunks_are_dropped_by_default() {
        // The whitelist bias: a chunk nobody has heard of must not survive.
        let mut body = Vec::new();
        body.extend_from_slice(&chunk(b"fmt ", &[0u8; 16]));
        body.extend_from_slice(&chunk(b"data", &[9u8; 4]));
        body.extend_from_slice(&chunk(b"zZz9", b"invented chunk with a name inside"));
        let mut wav = Vec::from(*b"RIFF");
        wav.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(&body);

        let (clean, report) = clean_wav_bytes(&wav, Policy::Strip).unwrap();
        assert!(report.removed.iter().any(|r| r == "zZz9"));
        assert!(!find(&clean, b"invented chunk"));
    }

    #[test]
    fn odd_sized_chunks_keep_their_alignment() {
        let mut body = Vec::new();
        body.extend_from_slice(&chunk(b"fmt ", &[0u8; 16]));
        body.extend_from_slice(&chunk(b"data", &[7u8; 5])); // odd, needs a pad
        body.extend_from_slice(&chunk(b"LIST", b"INFOsomething"));
        let mut wav = Vec::from(*b"RIFF");
        wav.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(&body);

        let (clean, _) = clean_wav_bytes(&wav, Policy::Strip).unwrap();
        // Walk the result: every chunk must parse and land on an even boundary.
        let mut pos = 12;
        while pos + 8 <= clean.len() {
            let size = u32::from_le_bytes(clean[pos + 4..pos + 8].try_into().unwrap()) as usize;
            pos += 8 + size + (size % 2);
        }
        assert_eq!(
            pos,
            clean.len(),
            "chunk walk did not land exactly at the end"
        );
    }

    #[test]
    fn realistic_mode_leaves_bland_tags_only() {
        let (clean, _) = clean_wav_bytes(&dirty_wav(), Policy::Realistic).unwrap();
        assert!(find(&clean, b"Unknown Artist"), "expected placeholder tags");
        assert!(!find(&clean, b"Jane Real Name"), "real name survived");
    }

    #[test]
    fn cleaning_twice_is_stable() {
        let (once, _) = clean_wav_bytes(&dirty_wav(), Policy::Strip).unwrap();
        let (twice, report) = clean_wav_bytes(&once, Policy::Strip).unwrap();
        assert!(!report.changed, "a clean WAV needs no second pass");
        assert_eq!(once, twice);
    }

    #[test]
    fn a_lying_chunk_size_is_rejected_not_panicked() {
        let mut wav = Vec::from(*b"RIFF");
        let body_len = 8 + 8 + 4;
        wav.extend_from_slice(&((body_len + 4) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // absurd size
        wav.extend_from_slice(&[0u8; 4]);
        assert!(matches!(
            clean_wav_bytes(&wav, Policy::Strip),
            Err(Error::Malformed(_))
        ));
    }

    #[test]
    fn a_wav_with_no_audio_is_rejected() {
        let mut body = Vec::new();
        body.extend_from_slice(&chunk(b"LIST", b"INFOnothing useful"));
        let mut wav = Vec::from(*b"RIFF");
        wav.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(&body);
        assert!(matches!(
            clean_wav_bytes(&wav, Policy::Strip),
            Err(Error::Malformed(_))
        ));
    }
}
