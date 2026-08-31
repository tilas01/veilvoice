// SPDX-License-Identifier: GPL-3.0-or-later
//! The assembled de-identification chain and its live performance statistics.
//!
//! Every other module in this crate does one job. This is the file that puts
//! them in order and decides what happens to a block of samples, so it is the
//! one to read first if you want to know what VeilVoice actually *does* to
//! audio.
//!
//! # The signal path
//!
//! [`Deidentifier::process`] takes a block of input and writes an equal-length
//! block of output. Everything below happens inside it, per STFT frame:
//!
//! 1. **Roll the modulation stream** if this frame is the one where the
//!    ratchet fires. See "forward secrecy" below.
//! 2. **Draw this frame's modulation** from the CSPRNG -- a pitch ratio and a
//!    formant ratio, glided toward fresh random targets rather than jumped, so
//!    the scrambling is inaudible as scrambling.
//! 3. **Track the fundamental** from the newest hop of *time-domain* samples.
//!    This cannot be done in the frequency domain: at any frame size with
//!    usable latency, the FFT's bin spacing cannot tell 100 Hz from 140 Hz.
//!    The tracker keeps its own longer history and is fed only what is new.
//! 4. **Let the accent neutraliser observe** that estimate, so its long-term
//!    picture of the speaker stays current.
//! 5. **Transform the spectrum** -- this is the irreversible step, and it lives
//!    in [`crate::spectral`]. Measured phase is discarded and resynthesised;
//!    pitch, vocal-tract scale and spectral tilt are mapped onto canonical
//!    values.
//!
//! Then, once per block rather than per frame, a short time-domain tail: soft
//! clip, chorus, reverb. Those are cosmetic. **They are not what makes the
//! output unlinkable** and nothing here should be read as though they were.
//!
//! # Why it is one-way, in one paragraph
//!
//! Two independent reasons, and both are needed:
//!
//! * **The mapping is many-to-one.** Every speaker is pushed toward the same
//!   pitch register, the same vocal-tract scale and the same long-term
//!   spectrum. Many different inputs produce the same output, so there is no
//!   inverse to compute -- not "an inverse that is hard to find", none.
//! * **The phase is gone.** The measured phase of every frame is discarded and
//!   replaced. Phase carries the precise waveform and a speaker's
//!   micro-timing; it is never stored, so nothing downstream can restore it.
//!
//! The CSPRNG modulation on top means there is not even one fixed transform to
//! characterise. That is a third reason, and it is the weakest of the three:
//! randomness alone would be reversible by anyone holding the seed. The seed
//! never leaves the process and is zeroized on drop, but the argument does not
//! rest on that.
//!
//! # Forward secrecy, and what `reseed_secs` is really for
//!
//! The modulation stream rolls onto a fresh seed every [`DeidConfig::reseed_secs`]
//! (two seconds by default), drawing the new seed from the stream it replaces.
//! ChaCha20 cannot be run backwards, so obtaining the current state tells an
//! adversary nothing about the modulation that drove any earlier segment: a
//! long recording is a chain of short independently-sealed streams rather than
//! one long one.
//!
//! **This is forward secrecy, not irreversibility.** Rolling more often does
//! not make the output harder to invert -- the phase discard and the
//! many-to-one mapping already did that, and they do not depend on the ratchet
//! at all. Setting `reseed_secs` to `0.0` keeps one stream for the session and
//! the output is exactly as unlinkable as before.
//!
//! # A roll cannot happen faster than a frame, and the interface must say so
//!
//! [`DeidConfig::reseed_range_ms`] asks for the interval to be drawn fresh from
//! a range at every roll, in milliseconds, rather than fixed. The gap is drawn
//! from the modulation stream itself, so it is unpredictable and costs neither
//! a syscall nor an allocation.
//!
//! It is **quantised to whole frames**, and the grain is coarser than people
//! expect. The engine produces one set of modulation parameters per STFT hop:
//! 256 samples at the default frame size, which is 5.33 ms at 48 kHz. There is
//! nothing between two frames to change, so a request for a 0.7 ms interval
//! does not roll seven times inside a frame -- it rolls once, at the frame
//! boundary, exactly as a request for 5 ms would.
//!
//! Making the frame short enough for a sub-millisecond roll would mean a
//! 128-point transform, which is 375 Hz per bin: too coarse to locate a
//! formant, and moving formants is the thing being done. The trade is not
//! available.
//!
//! So [`DeidConfig::effective_reseed_range_ms`] reports what a requested range
//! actually comes to on this configuration, and a front end shows that rather
//! than the number that was typed. Quietly accepting 0.7 ms and rolling at
//! 5.33 ms would be a setting that lies about itself.
//!
//! The roll is deliberately cheap: no syscall, no allocation, no lock. It has
//! to be, because it happens inside an audio callback.
//!
//! # Real-time constraints
//!
//! [`Deidentifier::process`] is allocation-free and safe to call from an audio
//! callback. That is a property of this file and it is easy to lose: a `Vec`
//! grown inside the per-frame closure, a lock taken, or a log line written
//! would each turn a working live path into audible dropouts on somebody
//! else's machine and not on yours.
//!
//! [`Deidentifier::process_vec`] is the convenience form that *does* allocate.
//! It is for offline processing; do not reach for it in a callback.
//!
//! [`ProcessStats`] records what each block cost -- last, worst, and an
//! exponential moving average -- so a front-end can show a real-time factor
//! instead of guessing. `worst_block_ms` is the one that matters for live use:
//! the average being comfortable says nothing about whether the worst block
//! missed its deadline.
//!
//! # Configuration is validated in one place
//!
//! [`DeidConfig::checked`] is the single funnel, and nothing should bypass it.
//! Two shipped defects are the reason it exists in that shape: a configuration
//! value once made every output sample silently `NaN` (F-10), and parameters
//! read from a file and handed to a library without a bound killed the process
//! (F-2, F-3). The engine keeps persistent state, so a bad value is not one bad
//! block -- it is every block from then on.
//!
//! # In plain words
//!
//! This is the file to read first if you want to know what VeilVoice actually does
//! to a voice.
//!
//! Every other file in the engine does one job. This one puts them in order and
//! decides what happens to each piece of sound: what is measured, what is thrown
//! away, what is replaced, and in which order.
//!
//! It also keeps count of how long the work is taking, which is what live mode
//! needs in order to tell you honestly if the computer is not keeping up.

use crate::accent::{AccentConfig, AccentNeutralizer, AccentStats};
use crate::effects::{Chorus, Reverb, SoftClip};
use crate::modulation::Modulator;
use crate::pitch::PitchTracker;
use crate::spectral::SpectralState;
use crate::stft::StftEngine;
use std::time::Instant;

/// User-facing configuration for the de-identifier.
#[derive(Clone, Copy, Debug)]
pub struct DeidConfig {
    /// Audio sample rate in Hz.
    pub sample_rate: f32,
    /// FFT size (power of two recommended). Larger = better frequency
    /// resolution but more latency.
    pub frame_size: usize,
    /// Overlap factor; hop = frame_size / overlap (4 = 75 % overlap).
    pub overlap: usize,
    /// Pitch ratio bounds (before intensity scaling).
    pub pitch_bounds: (f32, f32),
    /// Formant ratio bounds (before intensity scaling).
    pub formant_bounds: (f32, f32),
    /// Frames between fresh random modulation targets.
    pub frames_per_target: u32,
    /// One-pole glide coefficient toward each target (0,1].
    pub mod_smooth: f32,
    /// Soft-clip drive and dry/wet mix.
    pub distortion_drive: f32,
    /// Soft-clip dry/wet mix.
    pub distortion_mix: f32,
    /// Chorus dry/wet mix.
    pub chorus_mix: f32,
    /// Reverb dry/wet mix.
    pub reverb_mix: f32,
    /// 0..1 scales how far pitch/formant ratios deviate from 1.0.
    pub intensity: f32,
    /// Accent and speaker-trait neutralisation.
    pub accent: AccentConfig,
    /// How often the modulation stream rolls onto a fresh seed, in seconds.
    ///
    /// Each roll permanently closes off the stream that drove the audio before
    /// it: ChaCha20 cannot be run backwards, so an adversary who obtained the
    /// current state could not reconstruct the modulation of any earlier
    /// segment. A long recording therefore is not one key stream but a chain of
    /// short, independently-sealed ones.
    ///
    /// Two seconds by default, which is frequent enough to keep each segment
    /// small and far too slow to hear — the parameters glide across a roll and
    /// the phase offsets ease to their new values over about half a second.
    /// Set to `0.0` to keep a single stream for the whole session.
    ///
    /// Ignored when [`DeidConfig::reseed_range_ms`] is set.
    pub reseed_secs: f32,
    /// Draw the interval before each roll from this range, in **milliseconds**.
    ///
    /// `Some((lo, hi))` replaces the fixed [`DeidConfig::reseed_secs`] with a
    /// gap drawn fresh from the modulation stream at every roll, so the ratchet
    /// has no period to observe. `None` keeps the fixed interval.
    ///
    /// **Quantised to whole frames.** One frame is
    /// [`DeidConfig::frame_ms`] -- 5.33 ms at the default settings and 48 kHz
    /// -- and nothing can happen between two of them.
    /// [`DeidConfig::effective_reseed_range_ms`] is what the range comes to,
    /// and it is what an interface should display.
    ///
    /// Not part of [`DeidConfig::default`], which stays deterministic so the
    /// test suite does. The front ends call
    /// [`DeidConfig::with_random_reseed_range`] at launch, which is what makes
    /// the shipped interval something other than a number compiled in.
    ///
    /// F-73: that last sentence was written before anything did it. The method
    /// existed, was tested, and was called by **nothing but its own test** for
    /// two releases, so every shipped copy rolled on the same fixed two-second
    /// period -- a number compiled into the binary, which is precisely what it
    /// says here is not the case. A test now reads the front ends' source and
    /// fails the build if the call is not there.
    pub reseed_range_ms: Option<(f32, f32)>,
}

impl Default for DeidConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000.0,
            frame_size: 1024,
            overlap: 4,
            // Strong enough to erase identity, gentle enough to stay legible.
            pitch_bounds: (0.80, 1.28),
            formant_bounds: (0.78, 1.30),
            frames_per_target: 8,
            mod_smooth: 0.06,
            distortion_drive: 1.5,
            distortion_mix: 0.12,
            chorus_mix: 0.28,
            reverb_mix: 0.12,
            intensity: 1.0,
            accent: AccentConfig::default(),
            reseed_secs: 2.0,
            reseed_range_ms: None,
        }
    }
}

/// The narrowest randomised roll range this engine will accept, in
/// milliseconds. Below one frame the range has no room to vary in.
///
/// Public so that a front end refusing a typed value can name the limit. A
/// refusal that will not say what the bound is leaves somebody guessing, which
/// is only marginally better than the clamp it replaced.
pub const MIN_RESEED_MS: f32 = 0.05;

/// The widest, in milliseconds. Ten minutes is far past any use for a ratchet
/// and stops an absurd value producing a frame count that overflows.
pub const MAX_RESEED_MS: f32 = 600_000.0;

/// Why a ratchet range typed by a person was not accepted.
///
/// **Every one of these is a refusal, never a correction.** Clamping a typed
/// number to something legal is how somebody ends up running on a setting they
/// did not choose and cannot see: they typed a value, nothing complained, and
/// the program used a different one. For a control whose entire purpose is that
/// the interval should not be predictable, silently substituting a value would
/// be the worst available failure -- and the roadmap marker asks for exactly
/// this, in these words: *invalid input refused rather than clamped*.
#[derive(Clone, Debug, PartialEq)]
pub enum RangeError {
    /// The text was not two numbers.
    NotTwoNumbers(String),
    /// One of the two was not a number at all.
    NotANumber(String),
    /// A number was negative, zero, or not finite.
    NotPositive(f32),
    /// The low end was not below the high end.
    Backwards {
        /// What was given as the low end.
        lo: f32,
        /// What was given as the high end.
        hi: f32,
    },
    /// Below the narrowest range the engine can draw from.
    TooShort {
        /// The low end asked for.
        lo: f32,
        /// The least this engine accepts.
        least: f32,
    },
    /// Past the longest interval the engine accepts.
    TooLong {
        /// The high end asked for.
        hi: f32,
        /// The most this engine accepts.
        most: f32,
    },
}

impl std::fmt::Display for RangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotTwoNumbers(text) => write!(
                f,
                "a ratchet range is two numbers of milliseconds, low first, like \
                 250,1800 -- got \"{text}\""
            ),
            Self::NotANumber(part) => {
                write!(f, "\"{part}\" is not a number of milliseconds")
            }
            Self::NotPositive(value) => write!(
                f,
                "{value} is not a length of time; both ends must be above zero"
            ),
            Self::Backwards { lo, hi } => write!(
                f,
                "the range runs backwards: {lo} is not below {hi}, and the low end \
                 comes first"
            ),
            Self::TooShort { lo, least } => write!(
                f,
                "{lo} ms is below the {least} ms floor. The ratchet can only fire on \
                 a frame boundary, so anything shorter cannot be drawn"
            ),
            Self::TooLong { hi, most } => write!(
                f,
                "{hi} ms is past the {most} ms ceiling. A ratchet that slow is almost \
                 certainly a typo, and a long interval weakens forward secrecy without \
                 buying anything"
            ),
        }
    }
}

impl std::error::Error for RangeError {}

/// Read a `low,high` ratchet range in milliseconds, or say why not.
///
/// A comma or a dash may separate the two, because both are what people type.
/// Everything else is **refused with the reason** and nothing is ever adjusted
/// to fit -- see [`RangeError`].
///
/// The range is not quantised here. One frame is the smallest gap that can
/// happen at all, so a range narrower than a frame collapses to a single value
/// once it meets the engine; [`DeidConfig::effective_reseed_range_ms`] is what
/// a range really comes to and is what an interface should display beside it.
pub fn parse_reseed_range(text: &str) -> Result<(f32, f32), RangeError> {
    let cleaned = text.trim();
    let parts: Vec<&str> = if cleaned.contains(',') {
        cleaned.splitn(2, ',').collect()
    } else if cleaned.len() > 1 {
        // Searched from the second character so that a leading minus stays part
        // of the first number and is refused as not-positive, rather than read
        // as the separator and turning "-5-9" into a valid-looking pair.
        match cleaned[1..].find('-') {
            Some(at) => vec![&cleaned[..at + 1], &cleaned[at + 2..]],
            None => vec![cleaned],
        }
    } else {
        vec![cleaned]
    };
    if parts.len() != 2 {
        return Err(RangeError::NotTwoNumbers(cleaned.to_string()));
    }

    let mut ends = [0.0f32; 2];
    for (slot, part) in ends.iter_mut().zip(parts.iter()) {
        let part = part.trim();
        *slot = part
            .parse::<f32>()
            .map_err(|_| RangeError::NotANumber(part.to_string()))?;
        if !slot.is_finite() || *slot <= 0.0 {
            return Err(RangeError::NotPositive(*slot));
        }
    }
    let (lo, hi) = (ends[0], ends[1]);
    if lo >= hi {
        return Err(RangeError::Backwards { lo, hi });
    }
    if lo < MIN_RESEED_MS {
        return Err(RangeError::TooShort {
            lo,
            least: MIN_RESEED_MS,
        });
    }
    if hi > MAX_RESEED_MS {
        return Err(RangeError::TooLong {
            hi,
            most: MAX_RESEED_MS,
        });
    }
    Ok((lo, hi))
}

impl DeidConfig {
    fn hop(&self) -> usize {
        (self.frame_size / self.overlap.max(1)).max(1)
    }

    /// How long one analysis frame is, in milliseconds.
    ///
    /// The grain of everything the modulation does. No parameter can change
    /// more often than this, because only one set of them exists per frame.
    pub fn frame_ms(&self) -> f32 {
        if self.sample_rate > 0.0 {
            self.hop() as f32 * 1000.0 / self.sample_rate
        } else {
            0.0
        }
    }

    /// The number of frames a millisecond interval comes to, at least one.
    fn frames_for_ms(&self, ms: f32) -> u32 {
        let frame = self.frame_ms();
        if frame <= 0.0 {
            return 1;
        }
        ((ms / frame).round() as i64).clamp(1, u32::MAX as i64) as u32
    }

    /// What [`DeidConfig::reseed_range_ms`] actually comes to on this
    /// configuration, after quantising to whole frames.
    ///
    /// Show this, not the number the user typed. A request for 0.7 ms to
    /// 2.7 ms comes back as 5.33 ms to 5.33 ms at the default frame size,
    /// because a frame is the grain and the whole requested range is finer than
    /// one. That is not a failure -- it is the fastest this can honestly roll --
    /// but an interface that displayed "0.7 ms" would be claiming something
    /// that is not happening.
    pub fn effective_reseed_range_ms(&self) -> Option<(f32, f32)> {
        let (lo, hi) = self.reseed_range_ms?;
        let frame = self.frame_ms();
        Some((
            self.frames_for_ms(lo) as f32 * frame,
            self.frames_for_ms(hi) as f32 * frame,
        ))
    }

    /// Whether the requested range is finer than one frame, so the whole of it
    /// collapses onto a single interval.
    ///
    /// A front end should say so where the control is, rather than leaving
    /// somebody to wonder why moving the slider changes nothing.
    pub fn reseed_range_is_finer_than_a_frame(&self) -> bool {
        match self.reseed_range_ms {
            Some((lo, hi)) => self.frames_for_ms(lo) == self.frames_for_ms(hi),
            None => false,
        }
    }

    /// This configuration with a roll range drawn from the OS CSPRNG.
    ///
    /// The shipped interval is then a property of this launch rather than a
    /// number compiled into the binary -- which is the point: a fixed ratchet
    /// period is a fixed thing to observe, and every copy of VeilVoice having
    /// the same one makes it a property of the *program* rather than of the
    /// session.
    ///
    /// The range is centred somewhere between one frame and about two seconds,
    /// and both ends are drawn, so neither the period nor the spread is the
    /// same twice. Falls back to leaving the configuration alone if the OS
    /// CSPRNG cannot be read, because a de-identifier that refuses to start
    /// over the *ratchet* -- which is forward secrecy, not irreversibility --
    /// would be trading the whole feature for a nicety.
    pub fn with_random_reseed_range(mut self) -> Self {
        let mut bytes = [0u8; 4];
        if getrandom::getrandom(&mut bytes).is_err() {
            return self;
        }
        let a = u16::from_le_bytes([bytes[0], bytes[1]]) as f32 / u16::MAX as f32;
        let b = u16::from_le_bytes([bytes[2], bytes[3]]) as f32 / u16::MAX as f32;
        let frame = self.frame_ms().max(MIN_RESEED_MS);
        // One frame at the fast end, two seconds at the slow end, and the two
        // draws sorted so the range is never reversed.
        let span = (2000.0f32 - frame).max(frame);
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        self.reseed_range_ms = Some((frame + lo * span, frame + hi * span));
        self
    }

    /// Scale a `(lo, hi)` ratio range toward 1.0 by `intensity`.
    fn scaled(&self, bounds: (f32, f32)) -> (f32, f32) {
        let s = self.intensity.clamp(0.0, 1.0);
        (1.0 + (bounds.0 - 1.0) * s, 1.0 + (bounds.1 - 1.0) * s)
    }

    /// The largest sample rate this engine will build for, in Hz.
    ///
    /// Every value in here is reachable from a file: a WAV's `fmt ` chunk
    /// carries a **`u32`** sample rate, and `symphonia` passes whatever it
    /// finds straight through. That number then sizes the delay lines in
    /// `effects.rs` — `Reverb`'s comb is `0.0297 × sample_rate` samples and the
    /// chorus voices are similar — so a four-kilobyte file declaring
    /// `u32::MAX` asks for roughly two gigabytes of buffers before a single
    /// sample is processed. A failed allocation in Rust aborts the process,
    /// which is the same shape as F-3: opening a hostile file kills the
    /// program.
    ///
    /// 768 kHz is chosen well above anything real. Professional converters top
    /// out at 384 kHz and DSD-rate PCM at 705.6 kHz; nothing legitimate asks
    /// for more, and the largest buffer this permits is a few megabytes.
    pub const MAX_SAMPLE_RATE: f32 = 768_000.0;

    /// The largest FFT size this engine will build for.
    ///
    /// Bounded for the same reason as the sample rate: `frame_size` sizes every
    /// internal buffer and the FFT plan, and there was previously no upper
    /// limit at all, so a caller could ask for a `usize::MAX / 2` transform.
    /// 65536 is eight times the largest size anyone uses for speech.
    pub const MAX_FRAME_SIZE: usize = 1 << 16;

    /// Validate and normalise; returns an error string on impossible values.
    ///
    /// Every float is checked for finiteness, not merely for range. A `NaN`
    /// compares false against every bound, so a bare `self.sample_rate <
    /// 8_000.0` test *passes* `NaN` — and an engine built at a `NaN` sample
    /// rate produced `NaN` for every output sample, for the whole session,
    /// with nothing reported. That is F-5 arriving through a second door: F-5
    /// sanitised the samples, and nothing sanitised the configuration they were
    /// processed under.
    pub fn checked(mut self) -> Result<Self, String> {
        if self.frame_size < 64 || !self.frame_size.is_multiple_of(2) {
            return Err("frame_size must be even and >= 64".into());
        }
        if self.frame_size > Self::MAX_FRAME_SIZE {
            return Err(format!(
                "frame_size must be at most {}",
                Self::MAX_FRAME_SIZE
            ));
        }
        if !(2..=16).contains(&self.overlap) {
            return Err("overlap must be in 2..=16".into());
        }
        if !self.frame_size.is_multiple_of(self.overlap) {
            return Err("frame_size must be divisible by overlap".into());
        }
        // `is_finite` first: `NaN < 8_000.0` is false, so a range test alone
        // lets it through.
        if !self.sample_rate.is_finite() {
            return Err("sample_rate must be a real number".into());
        }
        if self.sample_rate < 8_000.0 {
            return Err("sample_rate too low".into());
        }
        if self.sample_rate > Self::MAX_SAMPLE_RATE {
            return Err(format!(
                "sample_rate {} Hz is above the {} Hz this engine will build for",
                self.sample_rate,
                Self::MAX_SAMPLE_RATE
            ));
        }
        if !self.reseed_secs.is_finite() || self.reseed_secs < 0.0 {
            return Err("reseed_secs must be zero or a positive number of seconds".into());
        }
        // Refused rather than clamped, and refused rather than silently
        // reordered. A reversed range is a typo, and a caller who wrote one
        // believes something about what their recording is doing.
        if let Some((lo, hi)) = self.reseed_range_ms {
            if !lo.is_finite() || !hi.is_finite() {
                return Err("reseed_range_ms must be two real numbers".into());
            }
            if lo > hi {
                return Err(format!(
                    "reseed_range_ms is {lo} to {hi} ms, which is backwards. Give the \
                     shorter interval first."
                ));
            }
            if lo < MIN_RESEED_MS || hi > MAX_RESEED_MS {
                return Err(format!(
                    "reseed_range_ms {lo} to {hi} ms is outside {MIN_RESEED_MS} to \
                     {MAX_RESEED_MS} ms"
                ));
            }
        }
        // The remaining floats are all clamped rather than refused, because
        // every one of them has a meaningful nearest legal value — but a `NaN`
        // does not clamp, it propagates, so it is refused by name.
        for (name, value) in [
            ("intensity", self.intensity),
            ("mod_smooth", self.mod_smooth),
            ("distortion_drive", self.distortion_drive),
            ("distortion_mix", self.distortion_mix),
            ("chorus_mix", self.chorus_mix),
            ("reverb_mix", self.reverb_mix),
            ("pitch_bounds.0", self.pitch_bounds.0),
            ("pitch_bounds.1", self.pitch_bounds.1),
            ("formant_bounds.0", self.formant_bounds.0),
            ("formant_bounds.1", self.formant_bounds.1),
        ] {
            if !value.is_finite() {
                return Err(format!("{name} must be a real number"));
            }
        }
        self.intensity = self.intensity.clamp(0.0, 1.0);
        self.mod_smooth = self.mod_smooth.clamp(1e-6, 1.0);
        self.distortion_mix = self.distortion_mix.clamp(0.0, 1.0);
        self.chorus_mix = self.chorus_mix.clamp(0.0, 1.0);
        self.reverb_mix = self.reverb_mix.clamp(0.0, 1.0);
        self.distortion_drive = self.distortion_drive.clamp(0.01, 64.0);
        // Ratios outside this are not a transform, they are a resampler with a
        // pathological factor; `resample_linear` already substitutes 1.0 for
        // anything non-finite, and this stops the merely absurd as well.
        self.pitch_bounds = clamp_ratio_bounds(self.pitch_bounds);
        self.formant_bounds = clamp_ratio_bounds(self.formant_bounds);
        Ok(self)
    }
}

/// Keep a `(lo, hi)` ratio pair inside a range a resampler can act on, and in
/// the right order.
fn clamp_ratio_bounds((lo, hi): (f32, f32)) -> (f32, f32) {
    const MIN: f32 = 0.05;
    const MAX: f32 = 20.0;
    let lo = lo.clamp(MIN, MAX);
    let hi = hi.clamp(MIN, MAX);
    if lo <= hi {
        (lo, hi)
    } else {
        (hi, lo)
    }
}

/// Rolling performance statistics, surfaced live to the UI.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessStats {
    /// Total blocks processed.
    pub blocks: u64,
    /// Total input samples processed.
    pub samples: u64,
    /// Wall-clock microseconds for the most recent block.
    pub last_block_us: f64,
    /// Worst (maximum) block time observed, microseconds.
    pub worst_block_us: f64,
    /// Exponential moving average of block time, microseconds.
    pub ema_block_us: f64,
    /// Sample count of the most recent block.
    pub last_block_samples: usize,
    /// Sample rate (for realtime-factor computation).
    pub sample_rate: f32,
    /// Fixed algorithmic latency of the STFT, milliseconds.
    pub algorithmic_latency_ms: f64,
    /// Frames until the next seed roll, as last drawn.
    ///
    /// With a randomised range this changes at every roll; with a fixed
    /// interval it is constant, and it is zero when rolling is off.
    pub reseed_frames: u32,
    /// The same figure in milliseconds -- the interval **actually** in force.
    ///
    /// This is the number to show a user, not the one they typed: the request
    /// is quantised to whole frames, and at the default frame size a frame is
    /// 5.33 ms.
    pub reseed_interval_ms: f64,
}

impl ProcessStats {
    /// Most recent block processing time in milliseconds.
    pub fn last_block_ms(&self) -> f64 {
        self.last_block_us / 1000.0
    }
    /// Worst block processing time in milliseconds.
    pub fn worst_block_ms(&self) -> f64 {
        self.worst_block_us / 1000.0
    }
    /// Smoothed block processing time in milliseconds.
    pub fn ema_block_ms(&self) -> f64 {
        self.ema_block_us / 1000.0
    }
    /// Processing time divided by the block's real-time duration. < 1.0 means
    /// the machine keeps up with real time; the headroom is `1 - factor`.
    pub fn last_realtime_factor(&self) -> f64 {
        let audio_us = self.last_block_samples as f64 / self.sample_rate as f64 * 1e6;
        if audio_us > 0.0 {
            self.last_block_us / audio_us
        } else {
            0.0
        }
    }
}

/// The complete, irreversible voice de-identification chain.
///
/// Feed it mono `f32` samples; it returns mono `f32` samples of equal length,
/// delayed by [`Deidentifier::latency_samples`]. Not real-time-thread cheap to
/// *construct* (allocates FFT plans), but `process` performs no heap
/// allocation and is safe to run inside an audio callback.
pub struct Deidentifier {
    stft: StftEngine,
    spectral: SpectralState,
    modulator: Modulator,
    accent: AccentNeutralizer,
    pitch: PitchTracker,
    softclip: SoftClip,
    chorus: Chorus,
    reverb: Reverb,
    stats: ProcessStats,
    latency_samples: usize,
    hop: usize,
    /// One frame in milliseconds, kept so the per-block statistics need no
    /// division by a sample rate inside the hot path.
    frame_ms: f64,
    /// Frames between seed rolls; 0 disables rolling entirely.
    ///
    /// With a randomised range this is the *last drawn* interval rather than a
    /// constant, and `reseed_span` is the range each new one is drawn from.
    reseed_frames: u32,
    /// The inclusive frame range each interval is drawn from, or `None` for a
    /// fixed interval.
    reseed_span: Option<(u32, u32)>,
    frames_until_reseed: u32,
    /// Pre-allocated, so a roll never allocates inside an audio callback.
    phase_scratch: Vec<f32>,
}

impl Deidentifier {
    /// Build with a fresh, unpredictable seed from the OS CSPRNG.
    pub fn new(config: DeidConfig) -> Result<Self, String> {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).map_err(|e| format!("OS CSPRNG: {e}"))?;
        Self::from_seed(config, seed)
    }

    /// Build with an explicit seed (deterministic; for tests or seed-from-key).
    pub fn from_seed(config: DeidConfig, seed: [u8; 32]) -> Result<Self, String> {
        let config = config.checked()?;
        let n = config.frame_size;
        let hop = config.hop();
        let half = n / 2 + 1;

        let mut modulator = Modulator::from_seed(
            seed,
            config.scaled(config.pitch_bounds),
            config.scaled(config.formant_bounds),
            config.frames_per_target,
            config.mod_smooth,
        );

        // Draw the fixed per-bin phase offsets from the same CSPRNG stream.
        let mut phase = vec![0.0f32; half];
        modulator.fill_phase_offsets(&mut phase);

        let stft = StftEngine::new(n, hop);
        let latency_samples = stft.latency_samples();
        let spectral = SpectralState::new(n, hop, config.sample_rate, &phase);
        let accent = AccentNeutralizer::new(config.accent, config.sample_rate, n, hop, half);
        let pitch = PitchTracker::new(config.sample_rate);

        // Rolls are counted in frames so the audio thread never touches a clock.
        let reseed_span = config
            .reseed_range_ms
            .map(|(lo, hi)| (config.frames_for_ms(lo), config.frames_for_ms(hi)));
        let reseed_frames = match reseed_span {
            // The first interval is drawn like every other one, so the very
            // first roll is no more predictable than the rest.
            Some((lo, hi)) => modulator.draw_frames(lo, hi),
            None if config.reseed_secs > 0.0 => ((config.reseed_secs * config.sample_rate)
                / hop as f32)
                .round()
                .max(1.0) as u32,
            None => 0,
        };

        let stats = ProcessStats {
            sample_rate: config.sample_rate,
            algorithmic_latency_ms: latency_samples as f64 / config.sample_rate as f64 * 1000.0,
            reseed_frames,
            reseed_interval_ms: reseed_frames as f64 * config.frame_ms() as f64,
            ..Default::default()
        };

        Ok(Self {
            stft,
            spectral,
            modulator,
            accent,
            pitch,
            softclip: SoftClip::new(config.distortion_drive, config.distortion_mix),
            chorus: Chorus::new(config.sample_rate, config.chorus_mix),
            reverb: Reverb::new(config.sample_rate, config.reverb_mix),
            stats,
            latency_samples,
            hop,
            frame_ms: config.frame_ms() as f64,
            reseed_frames,
            reseed_span,
            frames_until_reseed: reseed_frames,
            phase_scratch: phase,
        })
    }

    /// Fixed algorithmic latency in samples.
    pub fn latency_samples(&self) -> usize {
        self.latency_samples
    }

    /// Live performance statistics (copy).
    pub fn stats(&self) -> ProcessStats {
        self.stats
    }

    /// Live accent-neutralisation read-out (detected f0, applied ratios).
    pub fn accent_stats(&self) -> AccentStats {
        self.accent.stats()
    }

    /// Process `input` into `output` (equal length). Allocation-free; safe for
    /// an audio callback. Updates [`Deidentifier::stats`].
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), output.len());
        let start = Instant::now();

        // Disjoint field borrows so the per-frame closure can drive the
        // modulator + spectral transform while the STFT owns the FFT plumbing.
        let spectral = &mut self.spectral;
        let modulator = &mut self.modulator;
        let accent = &mut self.accent;
        let tracker = &mut self.pitch;
        let hop = self.hop;
        let reseed_span = self.reseed_span;
        let interval = &mut self.reseed_frames;
        let countdown = &mut self.frames_until_reseed;
        let phase_scratch = &mut self.phase_scratch;
        self.stft.process(input, output, |spec, frame| {
            // Roll the stream forward. Cheap, allocation-free and syscall-free:
            // the new seed is drawn from the stream it replaces, and so is the
            // gap before the next roll.
            if *interval > 0 {
                *countdown = countdown.saturating_sub(1);
                if *countdown == 0 {
                    modulator.reseed();
                    modulator.fill_phase_offsets(phase_scratch);
                    spectral.retarget_phase_offsets(phase_scratch);
                    // Drawn *after* the roll, so the next gap comes from the new
                    // stream. Drawing it before would leave the timing of every
                    // future roll recoverable from a seed that the roll was
                    // supposed to have closed off.
                    *interval = match reseed_span {
                        Some((lo, hi)) => modulator.draw_frames(lo, hi),
                        None => *interval,
                    };
                    *countdown = *interval;
                }
            }

            let m = modulator.next_frame();
            // f0 has to come from the time domain: the FFT resolution at usable
            // frame sizes cannot tell 100 Hz from 140 Hz. Only the newest `hop`
            // samples are new; the tracker keeps its own longer history.
            tracker.push(&frame[frame.len() - hop..]);
            let est = tracker.estimate();
            accent.observe(est);
            spectral.transform(spec, m.pitch_ratio, m.formant_ratio, Some(accent), est);
        });

        // Time-domain effect tail.
        for s in output.iter_mut() {
            let mut y = self.softclip.process(*s);
            y = self.chorus.process(y);
            y = self.reverb.process(y);
            *s = y;
        }

        self.stats.reseed_frames = self.reseed_frames;
        self.stats.reseed_interval_ms = self.reseed_frames as f64 * self.frame_ms;

        let us = start.elapsed().as_nanos() as f64 / 1000.0;
        self.stats.blocks += 1;
        self.stats.samples += input.len() as u64;
        self.stats.last_block_us = us;
        self.stats.last_block_samples = input.len();
        self.stats.worst_block_us = self.stats.worst_block_us.max(us);
        self.stats.ema_block_us = if self.stats.blocks == 1 {
            us
        } else {
            0.05 * us + 0.95 * self.stats.ema_block_us
        };
    }

    /// Convenience: process a whole buffer and return a new `Vec`.
    pub fn process_vec(&mut self, input: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0; input.len()];
        self.process(input, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        if x.is_empty() {
            return 0.0;
        }
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// One frame is the grain of everything the modulation does, and the
    /// documented figure -- 5.33 ms at 48 kHz and the default frame size -- is
    /// the number every claim about roll intervals rests on.
    #[test]
    fn a_frame_is_the_documented_length() {
        let config = DeidConfig::default();
        assert_eq!(config.frame_size, 1024);
        assert_eq!(config.overlap, 4);
        assert!(
            (config.frame_ms() - 5.333).abs() < 0.01,
            "a frame is {} ms, not the 5.33 ms the documentation claims",
            config.frame_ms()
        );
    }

    /// The request that started this: 0.7 ms to 2.7 ms. The whole range is
    /// finer than one frame, so it collapses onto exactly one interval -- and
    /// the engine must say so rather than displaying the number typed.
    #[test]
    fn a_range_finer_than_a_frame_collapses_and_admits_it() {
        let config = DeidConfig {
            reseed_range_ms: Some((0.7, 2.7)),
            ..DeidConfig::default()
        };
        let (lo, hi) = config
            .effective_reseed_range_ms()
            .expect("a range was asked for");
        assert!((lo - config.frame_ms()).abs() < 1e-3, "{lo}");
        assert!((hi - config.frame_ms()).abs() < 1e-3, "{hi}");
        assert!(
            config.reseed_range_is_finer_than_a_frame(),
            "the collapse must be reportable, or the control lies about itself"
        );
    }

    /// A range wide enough to hold several frames keeps its width.
    #[test]
    fn a_range_wider_than_a_frame_survives_quantisation() {
        let config = DeidConfig {
            reseed_range_ms: Some((20.0, 200.0)),
            ..DeidConfig::default()
        };
        let (lo, hi) = config.effective_reseed_range_ms().unwrap();
        assert!(hi > lo * 5.0, "{lo} to {hi} lost its width");
        assert!(!config.reseed_range_is_finer_than_a_frame());
        // And both ends are whole frames.
        for value in [lo, hi] {
            let frames = value / config.frame_ms();
            assert!(
                (frames - frames.round()).abs() < 1e-3,
                "{value} ms is not a whole number of frames"
            );
        }
    }

    /// A reversed range is a typo about what a recording is doing, so it is
    /// refused rather than quietly sorted.
    #[test]
    fn a_backwards_range_is_refused_rather_than_reordered() {
        let error = DeidConfig {
            reseed_range_ms: Some((200.0, 20.0)),
            ..DeidConfig::default()
        }
        .checked()
        .expect_err("backwards must be refused");
        assert!(error.contains("backwards"), "{error}");

        assert!(DeidConfig {
            reseed_range_ms: Some((f32::NAN, 20.0)),
            ..DeidConfig::default()
        }
        .checked()
        .is_err());
        assert!(
            DeidConfig {
                reseed_range_ms: Some((0.0, 20.0)),
                ..DeidConfig::default()
            }
            .checked()
            .is_err(),
            "zero is below the floor and must be refused"
        );
        assert!(DeidConfig {
            reseed_range_ms: Some((1.0, 1e9)),
            ..DeidConfig::default()
        }
        .checked()
        .is_err());
    }

    /// The interval in force is reported, and with a randomised range it
    /// actually changes as the recording runs. Without that, the feature is a
    /// setting that does nothing.
    #[test]
    fn a_randomised_interval_changes_as_the_audio_runs() {
        let config = DeidConfig {
            // Wide enough in frames that two consecutive draws being equal by
            // chance is unlikely, and short enough that a second of audio
            // contains many rolls.
            reseed_range_ms: Some((10.0, 120.0)),
            ..DeidConfig::default()
        };
        let mut deid = Deidentifier::from_seed(config, [7u8; 32]).unwrap();
        let block = vec![0.05f32; 4096];
        let mut out = vec![0.0f32; 4096];
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..40 {
            deid.process(&block, &mut out);
            seen.insert(deid.stats().reseed_frames);
        }
        assert!(
            seen.len() > 1,
            "the interval never changed: {seen:?} -- the range is not being drawn from"
        );
        for frames in &seen {
            let ms = *frames as f64 * DeidConfig::default().frame_ms() as f64;
            assert!(
                (9.0..=125.0).contains(&ms),
                "{ms} ms is outside the range that was asked for"
            );
        }
    }

    /// A fixed interval must still report itself, and must not wander.
    #[test]
    fn a_fixed_interval_is_reported_and_stays_put() {
        let config = DeidConfig {
            reseed_secs: 0.1,
            ..DeidConfig::default()
        };
        let mut deid = Deidentifier::from_seed(config, [9u8; 32]).unwrap();
        let block = vec![0.05f32; 4096];
        let mut out = vec![0.0f32; 4096];
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..20 {
            deid.process(&block, &mut out);
            seen.insert(deid.stats().reseed_frames);
        }
        assert_eq!(seen.len(), 1, "a fixed interval moved: {seen:?}");
        let frames = *seen.iter().next().unwrap();
        assert!(
            (frames as f32 * DeidConfig::default().frame_ms() - 100.0).abs() < 6.0,
            "{frames} frames is not about 100 ms"
        );
    }

    /// Rolling off means no interval at all, not an interval of zero length.
    #[test]
    fn rolling_off_reports_no_interval() {
        let config = DeidConfig {
            reseed_secs: 0.0,
            ..DeidConfig::default()
        };
        let mut deid = Deidentifier::from_seed(config, [3u8; 32]).unwrap();
        let block = vec![0.05f32; 2048];
        let mut out = vec![0.0f32; 2048];
        deid.process(&block, &mut out);
        assert_eq!(deid.stats().reseed_frames, 0);
        assert_eq!(deid.stats().reseed_interval_ms, 0.0);
    }

    /// A randomised range is still deterministic from a seed, or the test
    /// suite could not hold anything about it.
    #[test]
    fn the_same_seed_draws_the_same_intervals() {
        let config = DeidConfig {
            reseed_range_ms: Some((10.0, 120.0)),
            ..DeidConfig::default()
        };
        let run = || {
            let mut deid = Deidentifier::from_seed(config, [42u8; 32]).unwrap();
            let block = vec![0.05f32; 4096];
            let mut out = vec![0.0f32; 4096];
            let mut intervals = Vec::new();
            for _ in 0..20 {
                deid.process(&block, &mut out);
                intervals.push(deid.stats().reseed_frames);
            }
            intervals
        };
        assert_eq!(run(), run());
    }

    /// The launch-time randomiser must produce something the engine accepts,
    /// every time, and something that is not the same on two calls.
    #[test]
    fn a_randomised_launch_range_is_valid_and_not_a_constant() {
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..12 {
            let config = DeidConfig::default().with_random_reseed_range();
            let checked = config
                .checked()
                .expect("the launch randomiser must produce a legal range");
            let (lo, hi) = checked.reseed_range_ms.expect("a range was set");
            assert!(lo <= hi, "{lo} to {hi}");
            assert!(lo >= checked.frame_ms() - 1e-3, "{lo} is below one frame");
            Deidentifier::from_seed(checked, [1u8; 32]).expect("and one the engine can build from");
            seen.insert(format!("{lo:.3}-{hi:.3}"));
        }
        assert!(
            seen.len() > 1,
            "the launch randomiser returned the same range every time"
        );
    }

    #[test]
    fn config_rejects_impossible_values() {
        assert!(DeidConfig {
            overlap: 1,
            ..Default::default()
        }
        .checked()
        .is_err());
        assert!(DeidConfig {
            frame_size: 1000,
            overlap: 3,
            ..Default::default()
        }
        .checked()
        .is_err());
        assert!(DeidConfig::default().checked().is_ok());
    }

    #[test]
    fn output_finite_and_length_preserved() {
        let mut d = Deidentifier::from_seed(DeidConfig::default(), [42u8; 32]).unwrap();
        let input: Vec<f32> = (0..48_000)
            .map(|i| (i as f32 * 220.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.3)
            .collect();
        let out = d.process_vec(&input);
        assert_eq!(out.len(), input.len());
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn loudness_roughly_preserved_no_runaway() {
        // De-identified speech must remain audible: not silent, not exploding.
        let mut d = Deidentifier::from_seed(DeidConfig::default(), [7u8; 32]).unwrap();
        // voiced-like input: fundamental + a few harmonics
        let sr = 48_000.0;
        let input: Vec<f32> = (0..sr as usize)
            .map(|i| {
                let t = i as f32 / sr;
                0.3 * (2.0 * std::f32::consts::PI * 140.0 * t).sin()
                    + 0.15 * (2.0 * std::f32::consts::PI * 280.0 * t).sin()
                    + 0.1 * (2.0 * std::f32::consts::PI * 420.0 * t).sin()
            })
            .collect();
        let out = d.process_vec(&input);
        let (ri, ro) = (rms(&input), rms(&out[sr as usize / 4..])); // skip warm-up
        assert!(ro > ri * 0.15, "output too quiet: in={ri} out={ro}");
        assert!(ro < ri * 6.0, "output runaway: in={ri} out={ro}");
    }

    #[test]
    fn different_seeds_produce_different_output() {
        let input: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let a = Deidentifier::from_seed(DeidConfig::default(), [1u8; 32])
            .unwrap()
            .process_vec(&input);
        let b = Deidentifier::from_seed(DeidConfig::default(), [2u8; 32])
            .unwrap()
            .process_vec(&input);
        let diff: f32 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum();
        assert!(
            diff > 1.0,
            "distinct seeds must yield distinct audio (diff={diff})"
        );
    }

    /// Harmonically rich voiced speech from a speaker with a given pitch and
    /// vocal-tract scale (`vtl` > 1 = longer tract = lower formants).
    fn speaker(f0: f32, vtl: f32, secs: f32) -> Vec<f32> {
        let sr = 48_000.0f32;
        let n = (sr * secs) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                let mut s = 0.0;
                for h in 1..=24 {
                    let f = f0 * h as f32;
                    if f > sr * 0.45 {
                        break;
                    }
                    let mut g = 1.0 / h as f32;
                    for &cf in &[700.0f32, 1220.0, 2600.0] {
                        g += 0.9 / (1.0 + ((f - cf / vtl) / 110.0).powi(2)) / h as f32;
                    }
                    s += g * (std::f32::consts::TAU * f * t).sin();
                }
                s * 0.1
            })
            .collect()
    }

    /// Isolate the accent path: no random modulation, no time-domain effects.
    fn accent_only(accent: AccentConfig) -> DeidConfig {
        DeidConfig {
            intensity: 0.0,
            distortion_mix: 0.0,
            chorus_mix: 0.0,
            reverb_mix: 0.0,
            accent,
            ..Default::default()
        }
    }

    fn measure_f0(signal: &[f32]) -> f32 {
        let mut t = crate::pitch::PitchTracker::new(48_000.0);
        t.push(signal);
        t.estimate().f0_hz
    }

    /// The end-to-end claim: two speakers who differ sharply in register go in,
    /// and come out sharing one canonical register.
    #[test]
    fn accent_neutralisation_converges_speakers_end_to_end() {
        let cfg = accent_only(AccentConfig::default());
        let (lo_f0, hi_f0) = (105.0f32, 230.0f32);

        let run = |f0: f32, vtl: f32| {
            let mut d = Deidentifier::from_seed(cfg, [11u8; 32]).unwrap();
            let out = d.process_vec(&speaker(f0, vtl, 3.0));
            // Skip warm-up; measure the settled tail.
            measure_f0(&out[out.len() * 2 / 3..])
        };
        let out_lo = run(lo_f0, 1.15);
        let out_hi = run(hi_f0, 0.87);

        assert!(out_lo > 0.0 && out_hi > 0.0, "output should be voiced");
        let before = (hi_f0 / lo_f0).log2().abs();
        let after = (out_hi / out_lo).log2().abs();
        assert!(
            after < before * 0.35,
            "registers should converge: {before:.2} octaves apart before, \
             {after:.2} after ({out_lo:.0} Hz vs {out_hi:.0} Hz)"
        );
    }

    #[test]
    fn accent_neutralisation_can_be_switched_off() {
        let input = speaker(210.0, 0.9, 2.0);
        let off = accent_only(AccentConfig {
            enabled: false,
            ..Default::default()
        });
        let out_off = Deidentifier::from_seed(off, [3u8; 32])
            .unwrap()
            .process_vec(&input);
        let out_on = Deidentifier::from_seed(accent_only(AccentConfig::default()), [3u8; 32])
            .unwrap()
            .process_vec(&input);

        let tail = input.len() * 2 / 3;
        let f_on = measure_f0(&out_on[tail..]);
        let f_off = measure_f0(&out_off[tail..]);

        // Enabled, the output is a clean comb at the canonical register.
        let target = AccentConfig::default().target_f0_hz;
        assert!(
            (f_on - target).abs() < 25.0,
            "expected ~{target} Hz, got {f_on}"
        );
        // Disabled, the legacy channel-vocoder path runs instead, which does not
        // produce that register.
        assert!(
            (f_off - target).abs() > 25.0,
            "bypass should not land on the canonical register: {f_off}"
        );
    }

    #[test]
    fn accent_stats_are_populated() {
        let mut d = Deidentifier::from_seed(DeidConfig::default(), [5u8; 32]).unwrap();
        d.process_vec(&speaker(160.0, 1.0, 1.5));
        let a = d.accent_stats();
        assert!(a.voiced, "synthetic speech should register as voiced");
        assert!(
            (a.detected_f0_hz - 160.0).abs() < 12.0,
            "f0={}",
            a.detected_f0_hz
        );
        assert!(a.warmup > 0.99, "warm-up did not complete");
        assert!(a.speaker_centroid_hz > 0.0);
    }

    /// Accent tracking must not cost the real-time budget: it is an addition to
    /// the spectral work, not a multiple of it.
    ///
    /// # Why this is a ratio and not a number
    ///
    /// This asserted an absolute real-time factor under 0.5 until it failed,
    /// and what it was measuring was the machine. A debug build under QEMU on
    /// the armv7 job reported 0.557 while the same commit passed on every
    /// native target: an emulated 32-bit target is far slower than the runner
    /// hosting it, and there is no single number that is generous enough there
    /// and tight enough to catch anything here.
    ///
    /// The claim worth defending does not depend on the machine. The regression
    /// this exists to catch, an un-decimated pitch search, is an order of
    /// magnitude, and an order of magnitude is still an order of magnitude on a
    /// slow processor. So the same audio is run twice on the same machine in
    /// the same test, once with the neutraliser bypassed, and the two are
    /// compared. Both runs are preceded by an unmeasured pass so that neither
    /// is paying for a cold cache.
    ///
    /// The bound is deliberately loose. This is a timing measurement on a
    /// shared build machine, and a test that fails when somebody else's job
    /// gets busy teaches people to re-run it rather than read it.
    #[test]
    fn accent_tracking_is_a_small_part_of_what_the_chain_costs() {
        let input = speaker(150.0, 1.0, 1.0);
        let cost = |enabled: bool| {
            let config = DeidConfig {
                accent: AccentConfig {
                    enabled,
                    ..AccentConfig::default()
                },
                ..DeidConfig::default()
            };
            let mut d = Deidentifier::from_seed(config, [8u8; 32]).unwrap();
            let mut out = vec![0.0; 1024];
            let mut run = |d: &mut Deidentifier| {
                for block in input.chunks(1024) {
                    d.process(block, &mut out[..block.len()]);
                }
            };
            run(&mut d);
            let start = std::time::Instant::now();
            run(&mut d);
            start.elapsed().as_secs_f64()
        };
        let off = cost(false);
        let on = cost(true);
        println!("one second of audio: {off:.4}s bypassed, {on:.4}s with accent tracking");
        assert!(off > 0.0, "the bypassed run took no measurable time");
        assert!(
            on < off * 4.0,
            "accent tracking cost {:.1} times the rest of the chain ({on:.4}s against {off:.4}s), \
             which is the shape of a search that stopped being decimated",
            on / off
        );
    }

    /// The property that makes rolling usable at all: it must be inaudible.
    /// A discontinuity in the phase offsets would show up as a sample-to-sample
    /// jump far larger than the signal ever produces on its own.
    #[test]
    fn rolling_the_seed_introduces_no_clicks() {
        let input = speaker(150.0, 1.0, 6.0);
        let worst_jump = |cfg: DeidConfig| {
            let out = Deidentifier::from_seed(cfg, [21u8; 32])
                .unwrap()
                .process_vec(&input);
            out.windows(2)
                .skip(4_800) // past the engine's warm-up
                .fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()))
        };

        let steady = worst_jump(DeidConfig {
            reseed_secs: 0.0,
            ..Default::default()
        });
        // Fast enough to roll several times inside the test signal.
        let rolling = worst_jump(DeidConfig {
            reseed_secs: 0.25,
            ..Default::default()
        });

        assert!(
            rolling <= steady * 1.5 + 1e-3,
            "rolling produced a discontinuity: worst jump {rolling:.4} vs {steady:.4} without"
        );
    }

    #[test]
    fn rolling_changes_the_audio_but_keeps_it_sane() {
        let input = speaker(150.0, 1.0, 5.0);
        let steady = Deidentifier::from_seed(
            DeidConfig {
                reseed_secs: 0.0,
                ..Default::default()
            },
            [4u8; 32],
        )
        .unwrap()
        .process_vec(&input);
        let rolling = Deidentifier::from_seed(
            DeidConfig {
                reseed_secs: 0.5,
                ..Default::default()
            },
            [4u8; 32],
        )
        .unwrap()
        .process_vec(&input);

        assert!(rolling.iter().all(|v| v.is_finite()));
        let diff: f32 = steady
            .iter()
            .zip(&rolling)
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            diff > 1.0,
            "rolling should change the modulation (diff={diff})"
        );

        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let ratio = rms(&rolling) / rms(&steady);
        assert!(
            (0.5..2.0).contains(&ratio),
            "rolling should not change the level ({ratio:.2}x)"
        );
    }

    /// Rolling must not cost determinism — reproducible builds and the whole
    /// test suite depend on `from_seed` being repeatable.
    #[test]
    fn rolling_stays_deterministic_for_a_given_seed() {
        let input = speaker(180.0, 1.0, 3.0);
        let cfg = DeidConfig {
            reseed_secs: 0.3,
            ..Default::default()
        };
        let a = Deidentifier::from_seed(cfg, [77u8; 32])
            .unwrap()
            .process_vec(&input);
        let b = Deidentifier::from_seed(cfg, [77u8; 32])
            .unwrap()
            .process_vec(&input);
        assert_eq!(a, b, "same seed must give the same audio, rolling or not");
    }

    #[test]
    fn reseed_interval_is_validated() {
        assert!(DeidConfig {
            reseed_secs: -1.0,
            ..Default::default()
        }
        .checked()
        .is_err());
        assert!(DeidConfig {
            reseed_secs: f32::NAN,
            ..Default::default()
        }
        .checked()
        .is_err());
        assert!(DeidConfig {
            reseed_secs: 0.0,
            ..Default::default()
        }
        .checked()
        .is_ok());
        assert!(DeidConfig {
            reseed_secs: 2.0,
            ..Default::default()
        }
        .checked()
        .is_ok());
    }

    #[test]
    fn stats_are_populated() {
        let mut d = Deidentifier::from_seed(DeidConfig::default(), [9u8; 32]).unwrap();
        let input = vec![0.1f32; 8192];
        d.process(&input, &mut vec![0.0; 8192]);
        let s = d.stats();
        assert_eq!(s.blocks, 1);
        assert!(s.last_block_us > 0.0);
        assert!(s.algorithmic_latency_ms > 0.0);
    }
}

#[cfg(test)]
mod reseed_range_tests {
    use super::*;

    /// **F-73.** The front ends must actually draw a range at launch.
    ///
    /// [`DeidConfig::reseed_range_ms`]'s own documentation said "the front ends
    /// call [`DeidConfig::with_random_reseed_range`] at launch, which is what
    /// makes the shipped interval something other than a number compiled in".
    /// Nothing called it. It was written, documented, tested in isolation, and
    /// reached by no code path for two releases, so every shipped copy rolled
    /// on the same fixed two-second period -- exactly the thing the sentence
    /// said was not happening.
    ///
    /// A comment cannot be tested, so this tests the code the comment is about.
    /// It reads both front ends and fails the build if the call is gone.
    #[test]
    fn both_front_ends_draw_a_random_range_at_launch() {
        let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let crates = here.parent().expect("crates/");
        for (crate_name, file) in [
            ("veilvoice-cli", "src/main.rs"),
            ("veilvoice-gui", "src/app.rs"),
        ] {
            let path = crates.join(crate_name).join(file);
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                .replace("\r\n", "\n");
            let code: String = source
                .lines()
                .filter(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.starts_with("//") && !trimmed.starts_with("///")
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                code.contains("with_random_reseed_range()"),
                "{crate_name} does not draw a ratchet range at launch, so every copy \
                 of it ships the same fixed period"
            );
        }
    }

    /// A drawn range is different from run to run. If it were not, it would be
    /// a compiled-in number wearing a random-looking coat.
    #[test]
    fn a_drawn_range_is_not_the_same_twice() {
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..8 {
            let range = DeidConfig::default()
                .with_random_reseed_range()
                .reseed_range_ms
                .expect("a range");
            seen.insert(format!("{:.3},{:.3}", range.0, range.1));
        }
        assert!(seen.len() > 1, "eight draws produced one range: {seen:?}");
    }

    /// A drawn range is usable: the right way round, inside the bounds, and
    /// wide enough to survive quantisation.
    #[test]
    fn a_drawn_range_is_always_valid() {
        for _ in 0..64 {
            let config = DeidConfig::default().with_random_reseed_range();
            let (lo, hi) = config.reseed_range_ms.expect("a range");
            assert!(lo < hi, "{lo} {hi}");
            assert!(lo >= MIN_RESEED_MS, "{lo}");
            assert!(hi <= MAX_RESEED_MS, "{hi}");
            assert!(config.checked().is_ok(), "{lo} {hi}");
        }
    }

    /// Everything a person can type that is not a range is **refused**, and
    /// the refusal says which thing was wrong. Nothing is adjusted to fit:
    /// that is the whole of marker 28's wording.
    #[test]
    fn bad_input_is_refused_with_a_reason_and_never_corrected() {
        use RangeError::*;
        for (text, expected) in [
            ("", NotTwoNumbers(String::new())),
            ("5", NotTwoNumbers("5".into())),
            ("abc,def", NotANumber("abc".into())),
            ("100,zzz", NotANumber("zzz".into())),
            ("0,100", NotPositive(0.0)),
            ("-5,100", NotPositive(-5.0)),
            (
                "1800,250",
                Backwards {
                    lo: 1800.0,
                    hi: 250.0,
                },
            ),
            (
                "100,100",
                Backwards {
                    lo: 100.0,
                    hi: 100.0,
                },
            ),
            (
                "0.001,100",
                TooShort {
                    lo: 0.001,
                    least: MIN_RESEED_MS,
                },
            ),
            (
                "100,900000",
                TooLong {
                    hi: 900_000.0,
                    most: MAX_RESEED_MS,
                },
            ),
        ] {
            let got =
                parse_reseed_range(text).expect_err("a value that is not a range must be refused");
            assert_eq!(
                std::mem::discriminant(&got),
                std::mem::discriminant(&expected),
                "{text:?} gave {got:?}"
            );
            // Every refusal has to be a sentence somebody can act on.
            let words = got.to_string();
            assert!(words.len() > 20, "{text:?}: {words}");
        }
    }

    /// The shapes people actually type are accepted, and accepted exactly --
    /// the numbers that come back are the numbers that went in.
    #[test]
    fn a_usable_range_survives_unchanged() {
        for (text, want) in [
            ("250,1800", (250.0, 1800.0)),
            (" 250 , 1800 ", (250.0, 1800.0)),
            ("250-1800", (250.0, 1800.0)),
            ("0.5,2", (0.5, 2.0)),
        ] {
            let got = parse_reseed_range(text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
            assert_eq!(got, want, "{text:?}");
        }
    }

    /// What an interface shows is what the engine will do, not what was asked
    /// for. The ratchet only fires on a frame boundary, so a range is
    /// quantised, and displaying the request would describe a spread that does
    /// not exist.
    #[test]
    fn the_effective_range_is_quantised_to_whole_frames() {
        let config = DeidConfig {
            reseed_range_ms: Some((250.0, 1800.0)),
            ..DeidConfig::default()
        };
        let (lo, hi) = config.effective_reseed_range_ms().expect("a range");
        let frame = config.frame_ms();
        for value in [lo, hi] {
            let frames = value / frame;
            assert!(
                (frames - frames.round()).abs() < 1e-3,
                "{value} ms is not a whole number of {frame} ms frames"
            );
        }
        assert!(lo >= 250.0 - frame && hi >= 1800.0 - frame, "{lo} {hi}");
    }
}
