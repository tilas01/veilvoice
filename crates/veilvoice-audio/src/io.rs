// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Reading and writing audio files.
//!
//! Decoding goes through `symphonia`, which is pure Rust and covers WAV, MP3,
//! FLAC, OGG/Vorbis, MP4/AAC and friends without shelling out to a codec
//! library. Writing is WAV only, on purpose: VeilVoice's job is to hand back
//! audio that has not been degraded, and re-encoding to a lossy format after
//! de-identification would throw away quality for no benefit. Callers who want
//! MP3 can transcode with whatever they already trust.
//!
//! # A decoder is a parser, and this one reads files somebody else made
//!
//! `symphonia` is the largest attacker-facing surface in this crate: it is
//! handed whole files of a format VeilVoice does not itself define. It is pure
//! Rust, which removes the memory-corruption class outright, but it does not
//! remove panics -- and this workspace builds with `panic = "abort"`, so a
//! panic inside a decoder is not an error a caller can handle, it is the
//! process ending.
//!
//! That is why a pre-flight check runs before a file reaches the decoder, and
//! why the honest position is recorded in the audit rather than glossed:
//! decoding in a separate process is the only complete answer to "the next
//! malformed file in a format we do not parse ourselves", and it is not built.
//!
//! # Writing is WAV only, and that is a decision
//!
//! Re-encoding to a lossy format after de-identification would throw away
//! quality for no privacy benefit -- the voiceprint is already gone, and what
//! remains is the words, which is the part worth keeping intact.
//!
//! # In-memory encoding, so plaintext never reaches the disk
//!
//! [`wav_bytes`] exists so a recording can be encoded and then sealed without
//! ever being written in the clear. It has an awkward shape for a real reason:
//! `hound::WavWriter::finalize` consumes the writer, so the encode has to borrow
//! the cursor (`WavWriter::new(&mut cursor, spec)`) and read the bytes back
//! through `cursor.into_inner()` after the writer has been dropped.

use crate::Error;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// The most decoded audio [`load`] will hold, in mono `f32` samples.
///
/// Roughly twelve hours at 48 kHz, and about eight gigabytes of `f32`. The
/// ceiling exists because compressed formats expand: a mono MP3 at 32 kbit/s
/// decodes to 48 000 `f32` per second, which is a **forty-eight-fold**
/// expansion, so a hundred-megabyte download becomes some five gigabytes of
/// samples. Rust aborts the process when an allocation fails, so an unbounded
/// decode turns "someone sent me a recording" — the ordinary use this tool is
/// for — into a way to kill it.
///
/// Twelve hours is far past any interview or call anyone will veil, and the
/// refusal names the limit rather than truncating silently, because a recording
/// that quietly lost its second half is worse than one that would not open.
pub const MAX_DECODED_SAMPLES: usize = 48_000 * 60 * 60 * 12;

/// Reject a file whose own header carries a value that will crash the decoder,
/// before the decoder is given the file.
///
/// Public so that a caller holding bytes rather than a path can run the same
/// check [`load`] runs, and so the fuzz target in `fuzz/` can reach it. Pass as
/// much of the start of the file as is convenient; a few kilobytes is plenty,
/// and a short buffer is not an error.
///
/// This exists for one confirmed case and is deliberately narrow. A WAV whose
/// `fmt ` chunk declares a **sample rate of zero** makes `symphonia` panic
/// inside `Probe::format`, at `TimeBase::new`, before this crate is handed
/// anything it could check. VeilVoice's release profile sets `panic = "abort"`,
/// so that panic is not an error a caller can handle — it is the process
/// ending. `veilvoice anonymise` on a four-kilobyte file somebody sent you was
/// enough.
///
/// Every other malformed value tried during the audit — zero channels, 65535
/// channels, zero or 65535 bits per sample, a mismatched format tag — is
/// already refused cleanly by `symphonia` itself, so nothing else is duplicated
/// here. Checking what the decoder already checks would be a second parser to
/// keep in step, which is its own bug source.
///
/// **The residual is stated rather than engineered around:** this cannot
/// protect against a panic in the decoder for a format whose header VeilVoice
/// does not parse. Under `panic = "abort"` no wrapper can, short of decoding in
/// a separate process. The mitigations are this check, keeping `symphonia`
/// current, and the fact that it is a widely used pure-Rust decoder rather than
/// a C library.
pub fn preflight(head: &[u8]) -> Result<(), Error> {
    // Only RIFF/WAVE is inspected; see the note above.
    if head.len() < 12 || &head[..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Ok(());
    }
    let mut pos = 12usize;
    while pos + 8 <= head.len() {
        let id = &head[pos..pos + 4];
        let size = u32::from_le_bytes([head[pos + 4], head[pos + 5], head[pos + 6], head[pos + 7]])
            as usize;
        if id == b"fmt " {
            // The sample rate is a little-endian `u32` at offset 4 of the
            // chunk's data. A `fmt ` chunk too short to hold one is malformed;
            // leave that judgement to the decoder, which reports it properly.
            let rate_at = pos + 8 + 4;
            if size >= 8 && rate_at + 4 <= head.len() {
                let rate = u32::from_le_bytes([
                    head[rate_at],
                    head[rate_at + 1],
                    head[rate_at + 2],
                    head[rate_at + 3],
                ]);
                if rate == 0 {
                    return Err(Error::Decode(
                        "this WAV declares a sample rate of zero, which is not a rate; \
                         refusing it here because the decoder crashes on it"
                            .into(),
                    ));
                }
            }
            return Ok(());
        }
        // Chunks are word aligned. `saturating_add` for the same reason as the
        // RIFF walker in `veilvoice-meta` (F-4): `size` is a `u32` read from
        // the file and `pos + 8 + size` overflows `usize` on a 32-bit target.
        let Some(next) = pos.checked_add(8).and_then(|p| p.checked_add(size)) else {
            return Ok(());
        };
        pos = next.saturating_add(size % 2);
    }
    Ok(())
}

/// Mono audio in memory.
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    /// Interleaved-free mono samples, nominally in `[-1, 1]`.
    pub samples: Vec<f32>,
    /// Sample rate in hertz.
    pub sample_rate: u32,
}

impl Audio {
    /// Duration in seconds.
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.samples.len() as f32 / self.sample_rate as f32
    }

    /// Peak absolute sample value.
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }
}

/// Decode any supported audio file to mono `f32`.
///
/// Multi-channel input is averaged down to mono. VeilVoice's engine is
/// single-channel by design: a stereo image is itself a recording-setup
/// fingerprint, and collapsing it removes one more way to match a file to the
/// room and hardware that produced it.
pub fn load(path: &Path) -> Result<Audio, Error> {
    let mut file = std::fs::File::open(path)?;

    // Read the head from the handle we are about to hand over, then rewind it,
    // rather than opening the path a second time. One open means the bytes
    // checked are the bytes decoded; two opens would be a race in which the
    // file could be swapped in between.
    let mut head = [0u8; 4096];
    let read = read_up_to(&mut file, &mut head)?;
    preflight(&head[..read])?;
    file.seek(SeekFrom::Start(0))?;

    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| Error::Decode(e.to_string()))?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| Error::Decode("no decodable audio track".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| Error::Decode(e.to_string()))?;

    let mut samples = Vec::new();
    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(48_000);
    let mut buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            // A clean end of stream, or a truncated file we have already read
            // the useful part of.
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(e) => return Err(Error::Decode(e.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                sample_rate = spec.rate;
                let channels = spec.channels.count().max(1);
                let buf = buffer.get_or_insert_with(|| {
                    SampleBuffer::<f32>::new(decoded.capacity() as u64, spec)
                });
                buf.copy_interleaved_ref(decoded);
                for frame in buf.samples().chunks(channels) {
                    // Bounded so a compressed file cannot expand into an
                    // allocation failure, which aborts rather than errors.
                    if samples.len() >= MAX_DECODED_SAMPLES {
                        return Err(Error::Decode(format!(
                            "this file decodes to more than {} samples \
                             (about twelve hours); refusing it rather than \
                             running the machine out of memory",
                            MAX_DECODED_SAMPLES
                        )));
                    }
                    samples.push(frame.iter().sum::<f32>() / channels as f32);
                }
            }
            // Decoders are allowed to report recoverable errors mid-stream;
            // dropping the bad packet is better than losing the whole file.
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(Error::Decode(e.to_string())),
        }
    }

    if samples.is_empty() {
        return Err(Error::Decode("file contained no audio".into()));
    }
    Ok(Audio {
        samples,
        sample_rate,
    })
}

/// Fill as much of `buf` as the file has, tolerating short reads.
///
/// A single `read` is allowed to return fewer bytes than asked for even when
/// more are available, and `read_exact` would fail outright on a file shorter
/// than the buffer — which most test fixtures are.
fn read_up_to(file: &mut std::fs::File, buf: &mut [u8]) -> Result<usize, Error> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

/// Encode mono `f32` audio as a 16-bit PCM WAV, in memory.
///
/// The in-memory form is what makes encrypt-at-rest honest: a recording that is
/// going to be sealed must never touch the disk in the clear first, because a
/// plaintext file that is written and then deleted is exactly the thing
/// [`veilvoice_crypto::shred`](../../veilvoice_crypto/shred/index.html) explains
/// cannot be reliably taken back on flash storage.
///
/// Samples are clamped rather than allowed to wrap: a sample past full scale
/// would otherwise flip sign and produce a loud click.
pub fn wav_bytes(audio: &Audio) -> Result<Vec<u8>, Error> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for &s in &audio.samples {
            let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            writer.write_sample(v)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

/// Write mono `f32` audio to a 16-bit PCM WAV file.
pub fn save_wav(path: &Path, audio: &Audio) -> Result<(), Error> {
    std::fs::write(path, wav_bytes(audio)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(sample_rate: u32, secs: f32, hz: f32) -> Audio {
        let n = (sample_rate as f32 * secs) as usize;
        let samples = (0..n)
            .map(|i| (i as f32 / sample_rate as f32 * hz * std::f32::consts::TAU).sin() * 0.5)
            .collect();
        Audio {
            samples,
            sample_rate,
        }
    }

    #[test]
    fn wav_round_trip_preserves_audio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        let original = tone(48_000, 0.25, 440.0);
        save_wav(&path, &original).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.sample_rate, 48_000);
        assert_eq!(loaded.samples.len(), original.samples.len());
        // 16-bit quantisation is the only permitted difference.
        let worst = original
            .samples
            .iter()
            .zip(&loaded.samples)
            .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(worst < 1e-3, "round trip error {worst}");
    }

    /// The in-memory encoder is what the encrypt-at-rest path uses, so it must
    /// produce exactly the file the on-disk one would.
    #[test]
    fn the_in_memory_encoder_matches_the_file_it_would_have_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        let original = tone(44_100, 0.2, 330.0);

        save_wav(&path, &original).unwrap();
        let on_disk = std::fs::read(&path).unwrap();
        let in_memory = wav_bytes(&original).unwrap();

        assert_eq!(&in_memory[..4], b"RIFF");
        assert_eq!(in_memory, on_disk);
    }

    #[test]
    fn out_of_range_samples_clamp_instead_of_wrapping() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hot.wav");
        let hot = Audio {
            samples: vec![3.0, -3.0, 0.0, 1.0, -1.0],
            sample_rate: 48_000,
        };
        save_wav(&path, &hot).unwrap();

        let loaded = load(&path).unwrap();
        assert!(
            loaded.samples[0] > 0.99,
            "positive overshoot wrapped: {}",
            loaded.samples[0]
        );
        assert!(
            loaded.samples[1] < -0.99,
            "negative overshoot wrapped: {}",
            loaded.samples[1]
        );
        assert!(loaded.samples.iter().all(|s| s.abs() <= 1.0));
    }

    #[test]
    fn stereo_is_averaged_to_mono() {
        // Write a stereo WAV by hand, then check both channels contribute.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stereo.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for _ in 0..1_000 {
            w.write_sample(16_000i16).unwrap(); // left
            w.write_sample(-16_000i16).unwrap(); // right, cancels
        }
        for _ in 0..1_000 {
            w.write_sample(16_000i16).unwrap();
            w.write_sample(16_000i16).unwrap(); // both, survives
        }
        w.finalize().unwrap();

        let a = load(&path).unwrap();
        assert_eq!(a.sample_rate, 44_100);
        assert_eq!(a.samples.len(), 2_000, "one mono sample per frame");
        assert!(a.samples[10].abs() < 0.01, "opposed channels should cancel");
        assert!(a.samples[1_500] > 0.4, "matched channels should survive");
    }

    #[test]
    fn metadata_helpers_are_sane() {
        let a = tone(48_000, 0.5, 100.0);
        assert!((a.duration_secs() - 0.5).abs() < 1e-6);
        assert!(a.peak() > 0.49 && a.peak() <= 0.5);
        assert_eq!(
            Audio {
                samples: vec![],
                sample_rate: 0
            }
            .duration_secs(),
            0.0
        );
    }

    #[test]
    fn a_missing_file_is_an_io_error() {
        assert!(matches!(
            load(Path::new("does-not-exist.wav")),
            Err(Error::Io(_))
        ));
    }

    /// Build a WAV by hand so the header can say things `hound` would refuse
    /// to write.
    fn handmade_wav(rate: u32, channels: u16, bits: u16) -> Vec<u8> {
        let data: Vec<u8> = (0..2_000u32)
            .flat_map(|i| ((i as i16).wrapping_mul(37)).to_le_bytes())
            .collect();
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&1u16.to_le_bytes()); // PCM
        fmt.extend_from_slice(&channels.to_le_bytes());
        fmt.extend_from_slice(&rate.to_le_bytes());
        fmt.extend_from_slice(&rate.wrapping_mul(2).to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());

        let mut body = Vec::new();
        body.extend_from_slice(b"fmt ");
        body.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        body.extend_from_slice(&fmt);
        body.extend_from_slice(b"data");
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(&data);

        let mut out = Vec::from(*b"RIFF");
        out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(&body);
        out
    }

    /// The regression, and the reason `preflight` exists.
    ///
    /// A WAV declaring a sample rate of zero made `symphonia` panic inside
    /// `Probe::format`, at `TimeBase::new` — before this crate saw anything it
    /// could check. Under the shipped `panic = "abort"` profile that is the
    /// process dying, not an error, so `veilvoice anonymise` on a file somebody
    /// sent you was enough to kill it. If this test ever *panics* rather than
    /// failing an assertion, the pre-flight has stopped running.
    #[test]
    fn a_wav_declaring_a_zero_sample_rate_is_refused_rather_than_crashing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hostile.wav");
        std::fs::write(&path, handmade_wav(0, 1, 16)).unwrap();

        match load(&path) {
            Err(Error::Decode(m)) => assert!(
                m.contains("sample rate of zero"),
                "refused for the wrong reason: {m}"
            ),
            other => panic!("expected a clean refusal, got {other:?}"),
        }
    }

    /// The pre-flight must not become a second, divergent WAV parser. Values
    /// `symphonia` already refuses properly are left to it, and ordinary files
    /// must pass straight through.
    #[test]
    fn the_preflight_passes_everything_it_is_not_for() {
        // Real headers, untouched.
        for rate in [8_000u32, 44_100, 48_000, 192_000] {
            assert!(preflight(&handmade_wav(rate, 1, 16)).is_ok(), "{rate} Hz");
        }
        // Not a WAV at all: not this function's business.
        assert!(preflight(b"").is_ok());
        assert!(preflight(b"\xff\xfb\x90\x00 an mp3 frame header").is_ok());
        assert!(preflight(b"RIFF____AVI LIST").is_ok());
        // Truncated inside the header, and a chunk size that would overflow a
        // 32-bit `usize` — neither may panic or loop.
        let mut truncated = handmade_wav(48_000, 1, 16);
        for n in 0..truncated.len().min(40) {
            assert!(preflight(&truncated[..n]).is_ok(), "truncated at {n}");
        }
        truncated[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(preflight(&truncated).is_ok());

        // A WAV whose `fmt ` chunk comes after another chunk still gets found.
        let mut with_junk = Vec::from(*b"RIFF");
        let mut body = Vec::new();
        body.extend_from_slice(b"JUNK");
        body.extend_from_slice(&4u32.to_le_bytes());
        body.extend_from_slice(&[0u8; 4]);
        body.extend_from_slice(&handmade_wav(0, 1, 16)[12..]);
        with_junk.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        with_junk.extend_from_slice(b"WAVE");
        with_junk.extend_from_slice(&body);
        assert!(
            preflight(&with_junk).is_err(),
            "a zero rate behind a JUNK chunk was missed"
        );
    }

    /// The decode ceiling has to sit far above anything anyone will really
    /// veil, or it stops being a guard against hostile expansion and starts
    /// being a limitation. Checked at compile time so it cannot drift.
    const _: () = assert!(
        MAX_DECODED_SAMPLES >= 48_000 * 60 * 60,
        "an hour at 48 kHz must fit comfortably inside the decode ceiling"
    );

    #[test]
    fn a_non_audio_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, b"absolutely not audio").unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn sample_rate_is_preserved_across_rates() {
        for rate in [16_000u32, 44_100, 48_000] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("r.wav");
            save_wav(&path, &tone(rate, 0.1, 220.0)).unwrap();
            assert_eq!(load(&path).unwrap().sample_rate, rate);
        }
    }
}
