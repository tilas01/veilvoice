// SPDX-License-Identifier: GPL-3.0-or-later
//! The assembled de-identification chain and its live performance statistics.

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
    pub reseed_secs: f32,
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
        }
    }
}

impl DeidConfig {
    fn hop(&self) -> usize {
        (self.frame_size / self.overlap.max(1)).max(1)
    }

    /// Scale a `(lo, hi)` ratio range toward 1.0 by `intensity`.
    fn scaled(&self, bounds: (f32, f32)) -> (f32, f32) {
        let s = self.intensity.clamp(0.0, 1.0);
        (1.0 + (bounds.0 - 1.0) * s, 1.0 + (bounds.1 - 1.0) * s)
    }

    /// Validate and normalise; returns an error string on impossible values.
    pub fn checked(mut self) -> Result<Self, String> {
        if self.frame_size < 64 || !self.frame_size.is_multiple_of(2) {
            return Err("frame_size must be even and >= 64".into());
        }
        if !(2..=16).contains(&self.overlap) {
            return Err("overlap must be in 2..=16".into());
        }
        if !self.frame_size.is_multiple_of(self.overlap) {
            return Err("frame_size must be divisible by overlap".into());
        }
        if self.sample_rate < 8_000.0 {
            return Err("sample_rate too low".into());
        }
        if !self.reseed_secs.is_finite() || self.reseed_secs < 0.0 {
            return Err("reseed_secs must be zero or a positive number of seconds".into());
        }
        self.intensity = self.intensity.clamp(0.0, 1.0);
        Ok(self)
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
    /// Frames between seed rolls; 0 disables rolling entirely.
    reseed_frames: u32,
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
        let reseed_frames = if config.reseed_secs > 0.0 {
            ((config.reseed_secs * config.sample_rate) / hop as f32)
                .round()
                .max(1.0) as u32
        } else {
            0
        };

        let stats = ProcessStats {
            sample_rate: config.sample_rate,
            algorithmic_latency_ms: latency_samples as f64 / config.sample_rate as f64 * 1000.0,
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
            reseed_frames,
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
        let reseed_frames = self.reseed_frames;
        let countdown = &mut self.frames_until_reseed;
        let phase_scratch = &mut self.phase_scratch;
        self.stft.process(input, output, |spec, frame| {
            // Roll the stream forward. Cheap, allocation-free and syscall-free:
            // the new seed is drawn from the stream it replaces.
            if reseed_frames > 0 {
                *countdown = countdown.saturating_sub(1);
                if *countdown == 0 {
                    modulator.reseed();
                    modulator.fill_phase_offsets(phase_scratch);
                    spectral.retarget_phase_offsets(phase_scratch);
                    *countdown = reseed_frames;
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

    /// Accent tracking must not cost the real-time budget. Release builds are
    /// far faster; this only guards against an accidental order-of-magnitude
    /// regression such as un-decimated pitch search.
    #[test]
    fn stays_comfortably_realtime_with_accent_on() {
        let mut d = Deidentifier::from_seed(DeidConfig::default(), [8u8; 32]).unwrap();
        let input = speaker(150.0, 1.0, 1.0);
        for block in input.chunks(1024) {
            d.process(block, &mut vec![0.0; block.len()]);
        }
        let rtf = d.stats().last_realtime_factor();
        println!("realtime factor with accent tracking on: {rtf:.4}");
        assert!(
            rtf < 0.5,
            "realtime factor {rtf:.3} leaves too little headroom"
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
