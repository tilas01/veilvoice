// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-audio
//!
//! Everything between the sound hardware and
//! [`veilvoice_core`](../veilvoice_core/index.html): device enumeration, file
//! import and export, and the real-time capture → de-identify → playback path.
//!
//! - [`io`] — decode any common audio file to mono `f32`, write 16-bit WAV, or
//!   encode one in memory so it can be encrypted without ever landing on disk
//!   in the clear.
//! - `devices` — enumerate inputs and outputs, and spot a virtual audio cable.
//! - `live` — run the engine live between two devices.
//!
//! ## The `live` feature
//!
//! `devices` and `live` sit behind the default-on `live` feature. They are the
//! only part of this crate that needs `cpal`, and `cpal` has no backend for the
//! BSDs. Everything else — decoding, encoding, and running the engine over a
//! buffer — is pure Rust and builds anywhere, so turning the feature off keeps
//! file processing working on platforms that cannot do live capture rather than
//! failing to build at all.
//!
//! ## Routing, and why a virtual cable matters
//!
//! Scrambling a microphone is only useful if other applications can hear the
//! result. Selecting a virtual audio cable as the output makes the veiled voice
//! appear as an ordinary microphone to any call, stream or recorder on the
//! machine, with no per-application setup. [`devices::find_virtual_cable`]
//! detects an installed one so the UI can offer it directly.
//!
//! # In plain words
//!
//! This is the plumbing between your microphone, your speakers and the part that
//! changes the voice.
//!
//! It finds the sound devices you have, opens the recording you point at whatever
//! kind of file it is, and writes the result back out. For live use it does the
//! whole loop while you talk -- in from the microphone, through the engine, out to
//! whatever else is listening -- fast enough that a conversation still works.
//!
//! It also reports how loud things are, which is what the level bars in the
//! program are drawing.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "live")]
pub mod devices;
pub mod io;
#[cfg(feature = "live")]
pub mod live;
// Not behind the `live` feature. The scale is arithmetic over a number, and a
// front end that only processes files still has a level to draw -- and on the
// BSDs, where `cpal` has no backend and `live` is off, the alternative would be
// a second copy of it in whichever crate still wanted one.
pub mod meter;

#[cfg(feature = "live")]
pub use devices::{DeviceInfo, Direction};
pub use io::Audio;
#[cfg(feature = "live")]
pub use live::{LiveSession, LiveStats};

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    Io(std::io::Error),
    /// A file could not be decoded.
    Decode(String),
    /// A WAV file could not be written.
    Wav(hound::Error),
    /// A device could not be enumerated or opened.
    #[cfg(feature = "live")]
    Device(String),
    /// An audio stream could not be built or started.
    #[cfg(feature = "live")]
    Stream(String),
    /// The de-identification engine rejected its configuration.
    Engine(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<hound::Error> for Error {
    fn from(e: hound::Error) -> Self {
        Self::Wav(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "input/output error: {e}"),
            Self::Decode(m) => write!(f, "could not decode audio: {m}"),
            Self::Wav(e) => write!(f, "could not write WAV: {e}"),
            #[cfg(feature = "live")]
            Self::Device(m) => write!(f, "audio device error: {m}"),
            #[cfg(feature = "live")]
            Self::Stream(m) => write!(f, "audio stream error: {m}"),
            Self::Engine(m) => write!(f, "de-identification engine error: {m}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Wav(e) => Some(e),
            _ => None,
        }
    }
}

/// De-identify a whole buffer of audio in one call.
///
/// Convenience for file processing: it builds an engine at the buffer's own
/// sample rate, runs it, and trims the engine's start-up delay so the output
/// lines up with the input rather than beginning with a frame of silence.
pub fn deidentify(audio: &Audio, config: veilvoice_core::DeidConfig) -> Result<Audio, Error> {
    let mut config = config;
    config.sample_rate = audio.sample_rate as f32;
    let mut engine = veilvoice_core::Deidentifier::new(config).map_err(Error::Engine)?;

    // Run past the end by the group delay so the tail is not cut off, then drop
    // the leading silence the STFT inevitably produces.
    let latency = engine.latency_samples();
    let mut padded = audio.samples.clone();
    padded.extend(std::iter::repeat_n(0.0, latency));

    let processed = engine.process_vec(&padded);
    let samples = processed[latency.min(processed.len())..].to_vec();
    Ok(Audio {
        samples,
        sample_rate: audio.sample_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speech_like(sample_rate: u32, secs: f32) -> Audio {
        let n = (sample_rate as f32 * secs) as usize;
        let samples = (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let mut s = 0.0;
                for h in 1..=12 {
                    s += (std::f32::consts::TAU * 150.0 * h as f32 * t).sin() / h as f32;
                }
                s * 0.1
            })
            .collect();
        Audio {
            samples,
            sample_rate,
        }
    }

    #[test]
    fn deidentify_preserves_length_and_rate() {
        let input = speech_like(48_000, 1.0);
        let out = deidentify(&input, Default::default()).unwrap();
        assert_eq!(out.sample_rate, input.sample_rate);
        assert_eq!(out.samples.len(), input.samples.len());
        assert!(out.samples.iter().all(|s| s.is_finite()));
    }

    /// Trimming the group delay matters: without it every processed file would
    /// start with a frame of silence and drift against the original.
    #[test]
    fn output_is_aligned_not_delayed_by_a_silent_frame() {
        let input = speech_like(48_000, 0.5);
        let out = deidentify(&input, Default::default()).unwrap();
        let head_energy: f32 = out.samples[..2_000].iter().map(|s| s * s).sum();
        assert!(
            head_energy > 1e-6,
            "output begins with silence: {head_energy:e}"
        );
    }

    #[test]
    fn output_is_audible_but_not_runaway() {
        let input = speech_like(48_000, 1.0);
        let out = deidentify(&input, Default::default()).unwrap();
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let (a, b) = (rms(&input.samples), rms(&out.samples));
        assert!(b > a * 0.1, "output too quiet: {a} -> {b}");
        assert!(b < a * 6.0, "output too loud: {a} -> {b}");
    }

    #[test]
    fn works_at_several_sample_rates() {
        for rate in [16_000u32, 44_100, 48_000] {
            let input = speech_like(rate, 0.4);
            let out = deidentify(&input, Default::default()).unwrap();
            assert_eq!(out.sample_rate, rate);
            assert!(out.samples.iter().all(|s| s.is_finite()), "rate {rate}");
        }
    }

    #[test]
    fn an_invalid_configuration_is_reported() {
        let bad = veilvoice_core::DeidConfig {
            overlap: 1,
            ..Default::default()
        };
        assert!(matches!(
            deidentify(&speech_like(48_000, 0.1), bad),
            Err(Error::Engine(_))
        ));
    }
}
