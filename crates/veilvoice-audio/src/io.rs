// SPDX-License-Identifier: GPL-3.0-or-later
//! Reading and writing audio files.
//!
//! Decoding goes through `symphonia`, which is pure Rust and covers WAV, MP3,
//! FLAC, OGG/Vorbis, MP4/AAC and friends without shelling out to a codec
//! library. Writing is WAV only, on purpose: VeilVoice's job is to hand back
//! audio that has not been degraded, and re-encoding to a lossy format after
//! de-identification would throw away quality for no benefit. Callers who want
//! MP3 can transcode with whatever they already trust.

use crate::Error;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

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
    let file = std::fs::File::open(path)?;
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
