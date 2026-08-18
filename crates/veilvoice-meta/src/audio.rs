// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Audio tag removal and replacement.
//!
//! Tags are handled through `lofty`, which understands ID3v1/ID3v2, Vorbis
//! comments, MP4 atoms and APE, so one code path covers every format VeilVoice
//! imports. Only the tag blocks are rewritten — the audio stream is never
//! re-encoded, so cleaning a file is lossless.

use crate::{Error, Policy, Report};
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::prelude::{ItemKey, TagExt};
use lofty::probe::Probe;
use lofty::tag::{Tag, TagType};
use std::io::Read;
use std::path::Path;

/// Read just enough of a file to identify its container.
fn read_head(path: &Path) -> Result<Vec<u8>, Error> {
    let mut f = std::fs::File::open(path)?;
    let mut head = vec![0u8; 12];
    let n = f.read(&mut head)?;
    head.truncate(n);
    Ok(head)
}

/// Tags written in [`Policy::Realistic`] mode.
///
/// Deliberately bland: the point is a file that looks ordinary, not one that
/// tells a story. Nothing here names a device, a person, a place or a date, and
/// the encoder string is one of the most common in the world, so it blends in
/// rather than fingerprinting VeilVoice itself.
const REALISTIC: &[(ItemKey, &str)] = &[
    (ItemKey::TrackTitle, "Audio"),
    (ItemKey::TrackArtist, "Unknown Artist"),
    (ItemKey::AlbumTitle, "Recordings"),
    (ItemKey::EncoderSoftware, "Lavf58.76.100"),
];

/// Strip or replace the tags of an audio file, in place.
///
/// The file is rewritten only if something actually changed.
pub fn clean_audio_file(path: &Path, policy: Policy) -> Result<Report, Error> {
    // WAV is handled at the chunk level: `lofty` cannot remove an ID3v2 block
    // from a RIFF file, and quietly leaving the tag behind is the one outcome
    // this crate must never produce. See `crate::wav`.
    let head = read_head(path)?;
    if crate::wav::is_wav(&head) {
        let bytes = std::fs::read(path)?;
        let (cleaned, report) = crate::wav::clean_wav_bytes(&bytes, policy)?;
        if report.changed {
            std::fs::write(path, cleaned)?;
        }
        return Ok(report);
    }

    let tagged = Probe::open(path)
        .map_err(|e| Error::Malformed(e.to_string()))?
        .read()
        .map_err(|e| Error::Malformed(e.to_string()))?;

    let mut report = Report::default();
    let present: Vec<TagType> = tagged.tags().iter().map(|t| t.tag_type()).collect();
    let primary = tagged.primary_tag_type();
    // Drop the parsed view before touching the file on disk.
    drop(tagged);

    // Each tag block has to be deleted from the file itself. Removing it from
    // the in-memory `TaggedFile` and saving is not enough: saving writes the
    // tags the value holds, it does not erase the ones already in the file.
    for tag_type in &present {
        report.note(format!("{tag_type:?}"));
        tag_type
            .remove_from_path(path, WriteOptions::default())
            .map_err(|e| Error::Malformed(e.to_string()))?;
    }

    if policy == Policy::Realistic {
        // Write into the format's own preferred tag type, so the result looks
        // like anything else that might have touched the file.
        let mut tag = Tag::new(primary);
        for (key, value) in REALISTIC {
            tag.insert_text(*key, (*value).to_string());
        }
        tag.save_to_path(path, WriteOptions::default())
            .map_err(|e| Error::Malformed(e.to_string()))?;
        report.changed = true;
    }
    Ok(report)
}

/// Report which tag blocks a file carries, without modifying it.
///
/// Useful for showing the user what is about to be removed, and for verifying
/// afterwards that nothing was left behind.
pub fn clean_audio_tags(path: &Path) -> Result<Vec<String>, Error> {
    let head = read_head(path)?;
    if crate::wav::is_wav(&head) {
        let bytes = std::fs::read(path)?;
        let (_, report) = crate::wav::clean_wav_bytes(&bytes, Policy::Strip)?;
        return Ok(report.removed);
    }
    let tagged = Probe::open(path)
        .map_err(|e| Error::Malformed(e.to_string()))?
        .read()
        .map_err(|e| Error::Malformed(e.to_string()))?;
    Ok(tagged
        .tags()
        .iter()
        .map(|t| format!("{:?}", t.tag_type()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::prelude::Accessor;

    /// A tiny but valid WAV, then tagged with something identifying.
    fn tagged_wav(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("sample.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..4_800 {
            w.write_sample((i % 100) as i16 * 50).unwrap();
        }
        w.finalize().unwrap();

        let tagged = Probe::open(&path).unwrap().read().unwrap();
        let mut tag = Tag::new(tagged.primary_tag_type());
        tag.set_artist("Jane Real Name".to_string());
        tag.set_title("Recorded on My Phone 2026-08-15".to_string());
        tag.insert_text(
            ItemKey::EncoderSoftware,
            "SomePhone Recorder 4.2".to_string(),
        );
        drop(tagged);
        tag.save_to_path(&path, WriteOptions::default()).unwrap();
        path
    }

    fn audio_bytes(path: &Path) -> Vec<i16> {
        hound::WavReader::open(path)
            .unwrap()
            .into_samples::<i16>()
            .map(|s| s.unwrap())
            .collect()
    }

    #[test]
    fn strip_removes_every_tag() {
        let dir = tempfile::tempdir().unwrap();
        let path = tagged_wav(dir.path());
        assert!(
            !clean_audio_tags(&path).unwrap().is_empty(),
            "fixture should be tagged"
        );

        let report = clean_audio_file(&path, Policy::Strip).unwrap();
        assert!(report.changed);
        assert!(
            clean_audio_tags(&path).unwrap().is_empty(),
            "tags survived stripping"
        );
    }

    #[test]
    fn identifying_strings_are_gone_from_the_bytes() {
        // The strongest check: the name must not appear anywhere in the file,
        // not merely be absent from the parsed tag view.
        let dir = tempfile::tempdir().unwrap();
        let path = tagged_wav(dir.path());
        clean_audio_file(&path, Policy::Strip).unwrap();
        let raw = std::fs::read(&path).unwrap();
        for needle in [
            b"Jane Real Name".as_slice(),
            b"SomePhone Recorder".as_slice(),
        ] {
            assert!(
                !raw.windows(needle.len()).any(|w| w == needle),
                "identifying string survived: {}",
                String::from_utf8_lossy(needle)
            );
        }
    }

    #[test]
    fn audio_samples_are_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = tagged_wav(dir.path());
        let before = audio_bytes(&path);
        clean_audio_file(&path, Policy::Strip).unwrap();
        assert_eq!(before, audio_bytes(&path), "cleaning must be lossless");
    }

    #[test]
    fn realistic_leaves_plausible_tags_not_real_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = tagged_wav(dir.path());
        clean_audio_file(&path, Policy::Realistic).unwrap();

        // WAV goes through the chunk-level cleaner, which writes LIST/INFO;
        // lofty surfaces that as a RIFF INFO tag, not the ID3v2 it prefers.
        let tagged = Probe::open(&path).unwrap().read().unwrap();
        let tag = tagged
            .tag(TagType::RiffInfo)
            .expect("realistic mode should leave a readable tag");
        assert_eq!(tag.artist().as_deref(), Some("Unknown Artist"));

        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(14).any(|w| w == b"Jane Real Name"));
    }

    #[test]
    fn cleaning_twice_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let path = tagged_wav(dir.path());
        clean_audio_file(&path, Policy::Strip).unwrap();
        let once = std::fs::read(&path).unwrap();
        let second = clean_audio_file(&path, Policy::Strip).unwrap();
        assert!(
            !second.changed,
            "a clean file should need no second rewrite"
        );
        assert_eq!(once, std::fs::read(&path).unwrap());
    }

    #[test]
    fn a_non_media_file_is_rejected_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, b"this is not audio").unwrap();
        assert!(clean_audio_file(&path, Policy::Strip).is_err());
    }
}
