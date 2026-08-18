// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Monophonic fundamental-frequency tracker (decimated YIN).
//!
//! Accent neutralisation needs to know the speaker's *current* f0 so the
//! intonation contour can be replaced with a canonical one (see [`crate::accent`]).
//!
//! Two constraints shape this implementation:
//!
//! * **The STFT frame is too short to resolve f0 directly.** At the default
//!   1024-point FFT / 48 kHz the bin spacing is ~47 Hz, so a spectral peak-pick
//!   cannot tell 100 Hz from 140 Hz. This tracker therefore works in the time
//!   domain over its own rolling history, which may be longer than one STFT
//!   frame without adding any output latency — the window still *ends* at the
//!   current frame, so it stays causal.
//! * **It must be cheap enough for an audio callback.** The signal is decimated
//!   to ~8 kHz first (pitch lives in the low harmonics), which cuts the
//!   difference-function cost by the square of the decimation factor. At the
//!   default settings it costs on the order of 8 M flops/s — well under 1 % of
//!   one core — and allocates nothing after construction.
//!
//! The algorithm is YIN's cumulative mean normalised difference function
//! (de Cheveigné & Kawahara, 2002) with parabolic interpolation, minus the
//! optimisations that only matter for offline accuracy.

/// Lowest fundamental the tracker will report, in hertz.
const F0_MIN_HZ: f32 = 60.0;
/// Highest fundamental the tracker will report, in hertz.
const F0_MAX_HZ: f32 = 400.0;
/// Target sample rate after decimation, in hertz.
const DECIMATED_HZ: f32 = 8_000.0;
/// Analysis window length in decimated samples (~40 ms at 8 kHz — at least two
/// periods of the lowest supported f0).
const WINDOW: usize = 320;
/// `d'(tau)` below this counts as a confident voiced period.
const YIN_THRESHOLD: f32 = 0.15;
/// Frames quieter than this (RMS) are treated as unvoiced regardless.
const SILENCE_RMS: f32 = 1e-4;

/// One f0 measurement.
#[derive(Clone, Copy, Debug, Default)]
pub struct PitchEstimate {
    /// Estimated fundamental in hertz, or 0.0 when unvoiced.
    pub f0_hz: f32,
    /// Periodicity confidence in `[0, 1]`; 0 when unvoiced.
    pub confidence: f32,
    /// Whether the frame was judged voiced.
    pub voiced: bool,
}

/// Rolling, allocation-free f0 tracker.
pub struct PitchTracker {
    decim: usize,
    sr_d: f32,
    // box anti-alias accumulator feeding the decimator
    acc: f32,
    acc_n: usize,
    // decimated history; the newest `need` samples are always the tail
    buf: Vec<f32>,
    need: usize,
    lag_min: usize,
    lag_max: usize,
    // scratch
    cmnd: Vec<f32>,
    last: PitchEstimate,
}

impl PitchTracker {
    /// Build a tracker for input at `sample_rate` hertz.
    pub fn new(sample_rate: f32) -> Self {
        let decim = (sample_rate / DECIMATED_HZ).round().max(1.0) as usize;
        let sr_d = sample_rate / decim as f32;
        let lag_min = (sr_d / F0_MAX_HZ).floor().max(2.0) as usize;
        let lag_max = (sr_d / F0_MIN_HZ).ceil() as usize;
        let need = WINDOW + lag_max;
        Self {
            decim,
            sr_d,
            acc: 0.0,
            acc_n: 0,
            // Two windows of headroom so compaction is amortised, not per-sample.
            buf: Vec::with_capacity(2 * need),
            need,
            lag_min,
            lag_max,
            cmnd: vec![0.0; lag_max + 1],
            last: PitchEstimate::default(),
        }
    }

    /// Feed new input samples (anti-aliased and decimated internally).
    pub fn push(&mut self, samples: &[f32]) {
        for &x in samples {
            self.acc += x;
            self.acc_n += 1;
            if self.acc_n == self.decim {
                let v = self.acc / self.decim as f32;
                self.acc = 0.0;
                self.acc_n = 0;
                if self.buf.len() == 2 * self.need {
                    // Keep only the newest `need` samples; amortised O(1).
                    self.buf.copy_within(self.need.., 0);
                    self.buf.truncate(self.need);
                }
                self.buf.push(v);
            }
        }
    }

    /// Estimate f0 over the newest history. Returns the previous estimate
    /// unchanged until enough samples have accumulated.
    pub fn estimate(&mut self) -> PitchEstimate {
        if self.buf.len() < self.need {
            return self.last;
        }
        let x = &self.buf[self.buf.len() - self.need..];

        let energy: f32 = x[..WINDOW].iter().map(|v| v * v).sum();
        if (energy / WINDOW as f32).sqrt() < SILENCE_RMS {
            self.last = PitchEstimate::default();
            return self.last;
        }

        // Cumulative mean normalised difference function.
        self.cmnd[0] = 1.0;
        let mut running = 0.0f32;
        for tau in 1..=self.lag_max {
            let mut d = 0.0f32;
            for j in 0..WINDOW {
                let diff = x[j] - x[j + tau];
                d += diff * diff;
            }
            running += d;
            self.cmnd[tau] = if running > 0.0 {
                d * tau as f32 / running
            } else {
                1.0
            };
        }

        // First local minimum under the threshold; otherwise the global minimum.
        let mut best = self.lag_min;
        let mut found = false;
        for tau in self.lag_min..self.lag_max {
            if self.cmnd[tau] < YIN_THRESHOLD && self.cmnd[tau] <= self.cmnd[tau + 1] {
                best = tau;
                found = true;
                break;
            }
        }
        if !found {
            for tau in self.lag_min..=self.lag_max {
                if self.cmnd[tau] < self.cmnd[best] {
                    best = tau;
                }
            }
        }

        let confidence = (1.0 - self.cmnd[best]).clamp(0.0, 1.0);
        let tau = self.parabolic(best);
        let f0 = if tau > 0.0 { self.sr_d / tau } else { 0.0 };
        let voiced = found && confidence >= 0.35 && (F0_MIN_HZ..=F0_MAX_HZ).contains(&f0);

        self.last = PitchEstimate {
            f0_hz: if voiced { f0 } else { 0.0 },
            confidence: if voiced { confidence } else { 0.0 },
            voiced,
        };
        self.last
    }

    /// Sub-sample refinement of the minimum at `tau` by fitting a parabola
    /// through its two neighbours.
    fn parabolic(&self, tau: usize) -> f32 {
        if tau == 0 || tau + 1 > self.lag_max {
            return tau as f32;
        }
        let (a, b, c) = (self.cmnd[tau - 1], self.cmnd[tau], self.cmnd[tau + 1]);
        let denom = a - 2.0 * b + c;
        if denom.abs() < 1e-12 {
            return tau as f32;
        }
        tau as f32 + 0.5 * (a - c) / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sawtooth is richly harmonic, like voiced speech excitation.
    fn saw(f0: f32, sr: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let ph = (i as f32 * f0 / sr).fract();
                0.5 * (2.0 * ph - 1.0)
            })
            .collect()
    }

    fn track(f0: f32, sr: f32) -> PitchEstimate {
        let mut t = PitchTracker::new(sr);
        t.push(&saw(f0, sr, sr as usize / 2));
        t.estimate()
    }

    #[test]
    fn tracks_male_and_female_range() {
        for &f0 in &[85.0f32, 110.0, 155.0, 210.0, 260.0] {
            let e = track(f0, 48_000.0);
            assert!(e.voiced, "{f0} Hz should be voiced");
            let err = (e.f0_hz - f0).abs() / f0;
            assert!(err < 0.05, "f0={f0} estimated={} (err {err})", e.f0_hz);
        }
    }

    #[test]
    fn resolves_pitches_a_single_fft_bin_cannot() {
        // 100 vs 140 Hz sit inside one 46.9 Hz bin of the default 1024-pt FFT;
        // the whole reason this tracker works in the time domain.
        let a = track(100.0, 48_000.0);
        let b = track(140.0, 48_000.0);
        assert!(a.voiced && b.voiced);
        assert!((a.f0_hz - 100.0).abs() < 5.0, "{}", a.f0_hz);
        assert!((b.f0_hz - 140.0).abs() < 7.0, "{}", b.f0_hz);
    }

    #[test]
    fn silence_is_unvoiced() {
        let mut t = PitchTracker::new(48_000.0);
        t.push(&vec![0.0f32; 24_000]);
        let e = t.estimate();
        assert!(!e.voiced);
        assert_eq!(e.f0_hz, 0.0);
    }

    #[test]
    fn white_noise_is_not_confidently_voiced() {
        let mut t = PitchTracker::new(48_000.0);
        // Deterministic pseudo-noise; no periodicity in the speech f0 range.
        let mut s = 0x1234_5678u32;
        let noise: Vec<f32> = (0..24_000)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s >> 8) as f32 / 8_388_608.0 - 1.0
            })
            .collect();
        t.push(&noise);
        let e = t.estimate();
        assert!(!e.voiced, "noise reported voiced at {} Hz", e.f0_hz);
    }

    #[test]
    fn works_at_other_sample_rates() {
        for &sr in &[16_000.0f32, 44_100.0, 48_000.0] {
            let e = track(150.0, sr);
            assert!(e.voiced, "sr={sr}");
            assert!(
                (e.f0_hz - 150.0).abs() / 150.0 < 0.05,
                "sr={sr} f0={}",
                e.f0_hz
            );
        }
    }

    #[test]
    fn history_stays_bounded() {
        let mut t = PitchTracker::new(48_000.0);
        for _ in 0..200 {
            t.push(&saw(120.0, 48_000.0, 4800));
            t.estimate();
        }
        assert!(t.buf.len() <= 2 * t.need, "history buffer grew unbounded");
    }
}
