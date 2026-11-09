// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-audio
//!
//! Everything between the sound hardware and
//! [`veilvoice_core`](../veilvoice_core/index.html): device enumeration, file
//! import and export, and the real-time capture → de-identify → playback path.
//!
//! - [`io`], to decode any common audio file to mono `f32`, write 16-bit WAV, or
//!   encode one in memory so it can be encrypted without ever landing on disk
//!   in the clear.
//! - `devices`, to enumerate inputs and outputs and spot a virtual audio cable.
//! - `live`, to run the engine live between two devices.
//!
//! ## The `live` feature
//!
//! `devices` and `live` sit behind the default-on `live` feature. They are the
//! only part of this crate that needs `cpal`, and `cpal` has no backend for the
//! BSDs. Everything else, meaning decoding, encoding and running the engine over
//! a buffer, is pure Rust and builds anywhere, so turning the feature off keeps
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
pub mod kinds;
#[cfg(feature = "live")]
pub mod live;
#[cfg(feature = "live")]
pub mod playback;
// F-169. This attribute belongs to `record` and has been taken from it once
// already, by a `playback` declaration inserted directly above it: an
// attribute attaches to the item that follows, so the insertion left `record`
// bare and the crate stopped compiling with the feature off. `record` needs
// the gate because it uses `ringbuf`, which the feature brings in, and two
// `Error` variants that are gated themselves. `tools/audit/features.py` builds
// that configuration now, so the next insertion here fails a build rather than
// nine release jobs.
#[cfg(feature = "live")]
pub mod record;
/// Several microphones at once, a guest each. **Roadmap item 147.**
#[cfg(feature = "live")]
pub mod room;
// Not behind the `live` feature. The scale is arithmetic over a number, and a
// front end that only processes files still has a level to draw -- and on the
// BSDs, where `cpal` has no backend and `live` is off, the alternative would be
// a second copy of it in whichever crate still wanted one.
pub mod meter;

// Roadmap item 162. With `live` off, the same five modules exist and refuse to
// start anything, so the window builds on the BSDs without a `cfg` on every
// line of the tabs that would have used them. See `absent`.
#[cfg(not(feature = "live"))]
pub mod absent;
#[cfg(not(feature = "live"))]
pub use absent::{devices, live, playback, record, room};

/// Whether this build can capture and play sound.
///
/// **Roadmap item 162.** A front end asks this to say, before anybody presses
/// a button, that this platform cannot capture, rather than letting the
/// refusal be the first they hear of it.
pub const CAN_CAPTURE: bool = cfg!(feature = "live");

/// Why this build cannot capture, in the words a person reads, or `None` when
/// it can.
///
/// **Roadmap item 162.** The same sentence every constructor refuses with, so
/// the notice a front end shows before anything is pressed and the refusal it
/// would get afterwards cannot say different things.
#[cfg(feature = "live")]
pub const WHY_NO_CAPTURE: Option<&str> = None;
/// Why this build cannot capture, in the words a person reads, or `None` when
/// it can.
///
/// **Roadmap item 162.** The same sentence every constructor refuses with, so
/// the notice a front end shows before anything is pressed and the refusal it
/// would get afterwards cannot say different things.
#[cfg(not(feature = "live"))]
pub const WHY_NO_CAPTURE: Option<&str> = Some(absent::REASON);

pub use devices::{DeviceInfo, Direction};
pub use io::Audio;
pub use live::{Interference, Keeping, Kept, LiveSession, LiveStats, Side};
pub use record::{Recorder, Sink};
pub use room::{Guest, GuestStats, KeptRoom, RoomSession, RoomStats, MAX_GUESTS};

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
    /// Protected memory for a recording could not be prepared or filled.
    #[cfg(feature = "live")]
    Crypto(veilvoice_crypto::Error),
    /// This build has no live audio at all. **Roadmap item 162.**
    ///
    /// Not behind the feature, because it is the one error a build without it
    /// has to be able to give; the words are `absent::REASON`, which only that build has.
    NoLiveAudio(&'static str),
    /// The recording is longer than a WAV file can describe. The payload is
    /// how many bytes of audio it holds.
    #[cfg(feature = "live")]
    TooLong(usize),
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
            Self::NoLiveAudio(why) => write!(f, "{why}"),
            #[cfg(feature = "live")]
            Self::Crypto(e) => write!(f, "protected memory for the recording: {e}"),
            #[cfg(feature = "live")]
            Self::TooLong(bytes) => write!(
                f,
                "this recording holds {bytes} bytes of audio, and a WAV file states \
                 its size in 32-bit fields, so it cannot describe more than {}. \
                 Record in more than one part.",
                u32::MAX as usize - 36
            ),
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

    /// The public items of one module, as `kind name` plus a normalised
    /// signature for a function.
    ///
    /// Read from the source, because only one of the two builds is compiled
    /// at a time, and the one CI runs tests in is the one with live audio.
    fn surface(source: &str) -> std::collections::BTreeSet<String> {
        let mut found = std::collections::BTreeSet::new();
        let mut rest = source;
        while let Some(at) = rest.find("pub ") {
            let after = &rest[at..];
            let line_start = rest[..at].rfind('\n').map_or(0, |n| n + 1);
            let indent_ok = rest[line_start..at].trim().is_empty();
            rest = &rest[at + 4..];
            if !indent_ok {
                continue;
            }
            let item = after.split(['{', ';']).next().unwrap_or("");
            let item: String = item.split_whitespace().collect::<Vec<_>>().join(" ");
            // Argument names differ (`_samples` where nothing is read), and
            // the device type is reached by a different path in each build.
            let item = item
                .replace("( ", "(")
                .replace(", )", ")")
                .replace(",)", ")")
                .replace("(_", "(")
                .replace(", _", ", ")
                .replace("super::devices::Device", "Device")
                .replace("crate::devices::Device", "Device")
                .replace("crate::live::", "")
                .replace("crate::record::", "")
                .replace("crate::Error", "Error")
                .replace("(mut ", "(")
                .replace(", mut ", ", ");
            // A stand-in holds `Infallible` where the live type holds its
            // fields, and the live build names cpal's device by an alias, so
            // a type is compared by its name alone.
            let item = match item.split_once('(') {
                Some((head, _)) if head.starts_with("pub struct ") => head.to_string(),
                _ => item,
            };
            let item = match item.strip_prefix("pub type ") {
                Some(name) => format!("pub struct {name}"),
                None => item,
            };
            if item.starts_with("pub fn ")
                || item.starts_with("pub struct ")
                || item.starts_with("pub const ")
                || item.starts_with("pub type ")
                || item.starts_with("pub use ")
            {
                let item = match item.find(" -> ") {
                    Some(_) if item.starts_with("pub fn ") => item,
                    _ => item.split(" =").next().unwrap_or(&item).to_string(),
                };
                found.insert(item);
            }
        }
        found
    }

    /// The body of `pub mod name { ... }` in `absent.rs`.
    fn absent_module<'a>(source: &'a str, name: &str) -> &'a str {
        let at = source
            .find(&format!("pub mod {name} {{"))
            .unwrap_or_else(|| panic!("absent.rs has no `{name}` module"));
        let body = &source[at..];
        let mut depth = 0usize;
        for (i, c) in body.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &body[..i];
                    }
                }
                _ => {}
            }
        }
        body
    }

    /// **Roadmap item 162.** The build without live audio offers what the
    /// live build offers, by the same names and signatures, so the window
    /// compiles against either without a `cfg`.
    ///
    /// Checked here, in the build CI tests, because the build without live
    /// audio is only ever compiled, and a compiler checks only what the
    /// window happens to call. Two differences are deliberate and named:
    /// `record::start` makes a recorder, which no build without live audio
    /// can hold, and only a live session calls it; and the live build's
    /// private helpers do not exist to be offered.
    #[test]
    fn the_build_without_live_audio_offers_what_the_live_build_offers() {
        let absent = include_str!("absent.rs").replace("\r\n", "\n");
        let modules = [
            ("devices", include_str!("devices.rs")),
            ("record", include_str!("record.rs")),
            ("playback", include_str!("playback.rs")),
            ("live", include_str!("live.rs")),
            ("room", include_str!("room.rs")),
        ];
        let not_offered = ["pub fn start(sample_rate: u32) -> (Recorder, Sink)"];
        let mut wrong = Vec::new();
        for (name, live) in modules {
            let live = live.replace("\r\n", "\n");
            let live = live.split("\n#[cfg(test)]").next().unwrap_or(&live);
            let live = surface(live);
            let stand_in = surface(absent_module(&absent, name));
            for item in live.difference(&stand_in) {
                if !not_offered.contains(&item.as_str()) {
                    wrong.push(format!(
                        "{name}: the live build has `{item}` and absent.rs does not"
                    ));
                }
            }
            for item in stand_in.difference(&live) {
                if !item.starts_with("pub mod ") {
                    wrong.push(format!(
                        "{name}: absent.rs has `{item}` and the live build does not"
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    }

    /// The two answers to "can this build capture?" never disagree.
    #[test]
    fn a_build_that_can_capture_gives_no_reason_why_not() {
        assert_eq!(WHY_NO_CAPTURE.is_none(), CAN_CAPTURE);
    }

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
