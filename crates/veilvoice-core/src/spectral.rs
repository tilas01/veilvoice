// SPDX-License-Identifier: GPL-3.0-or-later
//! Frequency-domain de-identification transform.
//!
//! For every STFT frame we:
//!   1. take the magnitude spectrum and **discard the measured phase** — this is
//!      the irreversible step, it permanently erases the speaker's waveform /
//!      micro-timing;
//!   2. estimate a smooth spectral **envelope** (the vocal-tract / formant
//!      structure, i.e. the biometric identity) and the **excitation** residual
//!      (glottal source + phonetic detail that carries the words);
//!   3. shift the excitation by a cryptographically-modulated *pitch* ratio and
//!      warp the envelope by an independent *formant* ratio, so the identity is
//!      moved somewhere it never was while the phonemes stay legible;
//!   4. resynthesise a fresh phase, plus a fixed random per-bin offset.
//!
//! ## Voiced frames: an explicit harmonic comb
//!
//! Step 4 has two modes. On **unvoiced** frames each bin accumulates its own
//! centre frequency — the classic channel-vocoder phase, exactly right for
//! fricatives and noise.
//!
//! On **voiced** frames that alone is not enough, and it is audible. Bin centres
//! are multiples of `sample_rate / n` (46.875 Hz at the default settings), and a
//! harmonic peak spans several bins, so a plain channel vocoder turns each
//! partial into a cluster of independent grid-frequency sinusoids with unrelated
//! phases. A 210 Hz voice comes out beating around 187.5 and 234.4 Hz: metallic,
//! and with a pitch that cannot be steered, which would make the canonical
//! register [`crate::accent`] aims for unreachable.
//!
//! So when the frame is voiced and accent neutralisation is active, the
//! excitation is not resampled at all — it is **replaced** by an ideal harmonic
//! comb at the canonical fundamental, quantised to the nearest whole bin. This
//! is the textbook source-filter model of voiced speech (an impulse train
//! through the vocal-tract filter), and because every comb line then sits
//! exactly on a bin centre, the existing per-bin phase advance is precisely the
//! right advance for it: successive frames overlap-add coherently and each
//! harmonic emerges as one clean partial. The envelope still supplies the
//! formants, so the vowels are untouched.
//!
//! Snapping to the bin grid is what buys that coherence, and it costs pitch
//! resolution — the grid step is coarse. That is not a problem for the default
//! configuration, which maps every speaker onto a *single constant* register
//! that need only be snapped once; it does mean any residual intonation
//! (`prosody_flatten` below 1.0) is quantised to the same grid. Lifting that
//! restriction needs window-kernel synthesis, noted as future work in the
//! project roadmap.
//!
//! None of this weakens irreversibility. The measured phase is still discarded
//! in full, and pinning the output to one canonical fundamental destroys *more*
//! pitch information than randomising it would — a constant carries nothing.
//!
//! Between steps 2 and 4 the optional [`crate::accent`] neutraliser folds in its
//! long-term corrections: it reads the unwarped envelope to measure the
//! speaker's vocal-tract scale, contributes extra pitch and formant ratios, and
//! rotates the warped envelope toward a canonical spectral tilt.
//!
//! The measured phase is never reused, so no amount of downstream processing can
//! reconstruct the original excitation phase — the transform is one-way.

use crate::accent::AccentNeutralizer;
use crate::pitch::PitchEstimate;
use realfft::num_complex::Complex;
use std::f32::consts::{PI, TAU};

/// Persistent per-instance state for the spectral transform.
pub struct SpectralState {
    half: usize, // n/2 + 1 bins
    // scratch, reused every frame to avoid real-time allocation
    mag: Vec<f32>,
    env: Vec<f32>,
    exc: Vec<f32>,
    exc_shift: Vec<f32>,
    env_shift: Vec<f32>,
    tmp: Vec<f32>,
    // unvoiced synthesis phase state (per bin)
    phase_acc: Vec<f32>,
    phase_adv: Vec<f32>,           // per-bin centre-frequency advance per hop
    phase_offset: Vec<f32>,        // random per-bin offset (decorrelation)
    phase_offset_target: Vec<f32>, // where it is gliding to after a reseed
    offset_glide: f32,             // one-pole coefficient toward the target
    // envelope smoothing radius in bins
    env_radius: usize,
    // frequency mapping
    bin_hz: f32,
}

impl SpectralState {
    /// `n` = FFT size, `hop` = analysis/synthesis hop, `rand_phase` = fixed
    /// per-bin phase offsets in radians (length n/2+1) drawn from the CSPRNG.
    pub fn new(n: usize, hop: usize, sample_rate: f32, rand_phase: &[f32]) -> Self {
        let half = n / 2 + 1;
        assert_eq!(rand_phase.len(), half, "rand_phase must have n/2+1 entries");
        // Each bin k advances by its centre frequency over one hop.
        let phase_adv: Vec<f32> = (0..half)
            .map(|k| (TAU * k as f32 * hop as f32 / n as f32).rem_euclid(TAU))
            .collect();
        // Envelope smoothing wide enough to remove harmonic ripple (~formant
        // scale). Scales with FFT size; clamped to something sane.
        let env_radius = (n / 48).clamp(4, 64);
        Self {
            half,
            mag: vec![0.0; half],
            env: vec![0.0; half],
            exc: vec![0.0; half],
            exc_shift: vec![0.0; half],
            env_shift: vec![0.0; half],
            tmp: vec![0.0; half],
            phase_acc: rand_phase.to_vec(),
            phase_adv,
            phase_offset: rand_phase.to_vec(),
            phase_offset_target: rand_phase.to_vec(),
            // Roughly half a second to travel to a new offset. See
            // `retarget_phase_offsets` for why it is a glide and not a jump.
            offset_glide: (hop as f32 / (sample_rate * 0.5)).clamp(1e-4, 1.0),
            env_radius,
            bin_hz: sample_rate / n as f32,
        }
    }

    /// Aim the per-bin phase offsets at fresh values.
    ///
    /// Called when the modulation stream rolls onto a new seed. The offsets are
    /// **glided** to, never assigned: they are added directly to each bin's
    /// synthesis phase, so replacing them outright would step every partial's
    /// phase at once — an audible click, every couple of seconds, forever.
    ///
    /// Gliding turns that step into a brief, tiny detune instead. The move is
    /// taken the short way around the circle, so the worst case is half a turn
    /// spread over about half a second: under one hertz of momentary shift,
    /// which is inaudible.
    pub fn retarget_phase_offsets(&mut self, offsets: &[f32]) {
        debug_assert_eq!(offsets.len(), self.half);
        self.phase_offset_target.copy_from_slice(offsets);
    }

    /// Rewrite `spec` (length n/2+1) in place, given the current modulation.
    ///
    /// * `pitch_ratio`   > 1 raises the voice (excitation shifted up)
    /// * `formant_ratio` > 1 shrinks the apparent vocal tract (formants up)
    /// * `accent`        optional neutraliser, contributing its own ratios and
    ///   long-term envelope shaping on top of the random modulation
    /// * `pitch`         current f0 estimate; when voiced (and `accent` is
    ///   active) the excitation is replaced by a harmonic comb instead of being
    ///   resampled
    pub fn transform(
        &mut self,
        spec: &mut [Complex<f32>],
        pitch_ratio: f32,
        formant_ratio: f32,
        accent: Option<&mut AccentNeutralizer>,
        pitch: PitchEstimate,
    ) {
        debug_assert_eq!(spec.len(), self.half);

        // 1. magnitude only (phase discarded here)
        for (k, c) in spec.iter().enumerate() {
            self.mag[k] = c.norm();
        }

        // 2. envelope via double box-smoothing (≈ triangular) of the magnitude
        box_smooth(&self.mag, self.env_radius, &mut self.tmp);
        box_smooth(&self.tmp, self.env_radius, &mut self.env);

        // excitation = magnitude / envelope
        for k in 0..self.half {
            self.exc[k] = self.mag[k] / (self.env[k] + 1e-9);
        }

        // 3. accent neutralisation reads the *unwarped* envelope (that is where
        //    the speaker's real vocal-tract scale is visible) and contributes
        //    ratios that compose with the cryptographic modulation.
        let mut accent = accent;
        let (prosody_ratio, formant_ratio, accent_on) = match &mut accent {
            Some(a) if a.enabled() => {
                a.measure_envelope(&self.env);
                (a.prosody_ratio(), formant_ratio * a.vtln_ratio(), true)
            }
            _ => (1.0, formant_ratio, false),
        };

        // Decide the synthesis mode for this frame. The comb spacing is the
        // canonical fundamental rounded to whole bins; the random modulation is
        // deliberately *not* applied to it, because a pitch normalised to a
        // constant already carries no speaker information for randomisation to
        // hide, and a varying target would jitter across the grid.
        let comb_period = if accent_on && pitch.voiced && pitch.f0_hz > 0.0 {
            let f0 = pitch.f0_hz * prosody_ratio;
            let p = (f0 / self.bin_hz).round() as usize;
            (p >= 2 && p < self.half / 2).then_some(p)
        } else {
            None
        };

        // 4. formant (envelope) resampling always; excitation resampling only
        //    for the channel-vocoder path, since the comb replaces it outright.
        resample_linear(&self.env, formant_ratio, &mut self.env_shift);
        if comb_period.is_none() {
            resample_linear(&self.exc, pitch_ratio * prosody_ratio, &mut self.exc_shift);
        }

        // 5. shape the warped envelope toward the canonical long-term tilt
        if let Some(a) = &mut accent {
            a.shape(&mut self.env_shift);
        }

        // 6. recombine and assign a fresh, coherent synthesis phase.
        //
        // The per-bin accumulator is advanced every frame regardless of mode, so
        // that switching between voiced and unvoiced never lands on a stale
        // phase and clicks.
        for k in 0..self.half {
            self.phase_acc[k] = (self.phase_acc[k] + self.phase_adv[k]).rem_euclid(TAU);

            // Ease toward any newly drawn offset, the short way round.
            let mut delta = self.phase_offset_target[k] - self.phase_offset[k];
            if delta > PI {
                delta -= TAU;
            } else if delta < -PI {
                delta += TAU;
            }
            if delta != 0.0 {
                self.phase_offset[k] =
                    (self.phase_offset[k] + delta * self.offset_glide).rem_euclid(TAU);
            }
        }

        match comb_period {
            // Voiced: an ideal harmonic comb through the formant envelope. The
            // comb is scaled to carry the frame's original magnitude energy, so
            // replacing the excitation does not change the level.
            Some(p) => {
                let target_energy: f32 = self.mag.iter().map(|m| m * m).sum();
                let mut env_energy = 0.0f32;
                let mut k = p;
                while k < self.half {
                    env_energy += self.env_shift[k] * self.env_shift[k];
                    k += p;
                }
                let amp = if env_energy > 1e-20 {
                    (target_energy / env_energy).sqrt()
                } else {
                    0.0
                };

                for c in spec.iter_mut() {
                    *c = Complex::new(0.0, 0.0);
                }
                let mut k = p;
                while k < self.half {
                    let m = amp * self.env_shift[k];
                    let phase = self.phase_acc[k] + self.phase_offset[k];
                    spec[k] = Complex::new(m * phase.cos(), m * phase.sin());
                    k += p;
                }
            }
            // Unvoiced: channel-vocoder phase across the full spectrum.
            None => {
                for (k, c) in spec.iter_mut().enumerate() {
                    let new_mag = self.exc_shift[k] * self.env_shift[k];
                    let phase = self.phase_acc[k] + self.phase_offset[k];
                    *c = Complex::new(new_mag * phase.cos(), new_mag * phase.sin());
                }
            }
        }

        // DC and Nyquist bins must be purely real for the inverse real FFT.
        spec[0].im = 0.0;
        if let Some(last) = spec.last_mut() {
            last.im = 0.0;
        }
    }
}

/// Linear resampling of a non-negative spectral function.
///
/// `dst[k] = src[k / ratio]` with linear interpolation. Destination bins whose
/// source position falls outside `src` roll off to zero (band edges), which is
/// exactly what we want when shifting energy up or down the spectrum.
fn resample_linear(src: &[f32], ratio: f32, dst: &mut [f32]) {
    let len = src.len();
    let r = if ratio.is_finite() && ratio > 1e-4 {
        ratio
    } else {
        1.0
    };
    for (k, out) in dst.iter_mut().enumerate() {
        let pos = k as f32 / r;
        if pos < 0.0 || pos >= (len - 1) as f32 {
            *out = 0.0;
            continue;
        }
        let i0 = pos.floor() as usize;
        let frac = pos - i0 as f32;
        *out = src[i0] * (1.0 - frac) + src[i0 + 1] * frac;
    }
}

/// In-place-ish box smoother using a running sum. `radius` is the half-width;
/// window length is `2*radius+1`. Edges use symmetric clamping.
pub(crate) fn box_smooth(src: &[f32], radius: usize, dst: &mut [f32]) {
    let len = src.len();
    debug_assert_eq!(dst.len(), len);
    if radius == 0 || len == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let win = (2 * radius + 1) as f32;
    // initial window sum centred at index 0 (clamped)
    let mut sum = 0.0f32;
    for &v in &src[..=radius.min(len - 1)] {
        sum += v;
    }
    // account for left clamp (indices below 0 clamp to src[0])
    sum += src[0] * radius as f32;
    for (i, d) in dst.iter_mut().enumerate() {
        *d = sum / win;
        // slide the window right by one: add (i+radius+1), remove (i-radius)
        let add_idx = (i + radius + 1).min(len - 1);
        let rem_idx = i.saturating_sub(radius);
        sum += src[add_idx];
        sum -= src[rem_idx];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_identity() {
        let src: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let mut dst = vec![0.0; 16];
        resample_linear(&src, 1.0, &mut dst);
        for i in 0..15 {
            assert!((dst[i] - src[i]).abs() < 1e-4, "i={i}");
        }
    }

    #[test]
    fn box_smooth_preserves_constant() {
        let src = vec![3.0f32; 64];
        let mut dst = vec![0.0; 64];
        box_smooth(&src, 5, &mut dst);
        for &v in &dst {
            assert!((v - 3.0).abs() < 1e-4);
        }
    }

    #[test]
    fn transform_is_finite_and_nonnegative_magnitude() {
        let n = 1024;
        let half = n / 2 + 1;
        let rand_phase = vec![0.0f32; half];
        let mut st = SpectralState::new(n, 256, 48_000.0, &rand_phase);
        // a synthetic spectrum
        let mut spec: Vec<Complex<f32>> = (0..half)
            .map(|k| Complex::new((k as f32).sin(), (k as f32 * 0.3).cos()))
            .collect();
        st.transform(&mut spec, 1.2, 1.1, None, PitchEstimate::default());
        assert!(spec.iter().all(|c| c.re.is_finite() && c.im.is_finite()));
        assert_eq!(spec[0].im, 0.0);
        assert_eq!(spec[half - 1].im, 0.0);
    }

    /// Voiced frames must come out as a sparse comb on the bin grid, which is
    /// what makes the partials coherent under overlap-add.
    #[test]
    fn voiced_frames_synthesise_a_grid_aligned_comb() {
        let (n, hop, sr) = (1024usize, 256usize, 48_000.0f32);
        let half = n / 2 + 1;
        let mut st = SpectralState::new(n, hop, sr, &vec![0.0f32; half]);
        let mut accent = crate::accent::AccentNeutralizer::new(
            crate::accent::AccentConfig::default(),
            sr,
            n,
            hop,
            half,
        );
        // Drive the prosody correction to its settled, fully warmed state.
        let voiced = PitchEstimate {
            f0_hz: 210.0,
            confidence: 0.9,
            voiced: true,
        };
        for _ in 0..2000 {
            accent.observe(voiced);
        }

        let mut spec = vec![Complex::new(1.0f32, 0.0); half];
        st.transform(&mut spec, 1.0, 1.0, Some(&mut accent), voiced);

        // Everything off the comb must be exactly zero.
        let live: Vec<usize> = (0..half).filter(|&k| spec[k].norm() > 1e-12).collect();
        assert!(live.len() > 4, "expected a comb, got {} lines", live.len());
        let p = live[0];
        assert!(p >= 2, "comb period {p} is implausible");
        for &k in &live {
            assert_eq!(k % p, 0, "bin {k} is not a multiple of the comb period {p}");
        }

        // And the comb must sit at the canonical register, not the speaker's.
        let f0_out = p as f32 * sr / n as f32;
        let target = crate::accent::AccentConfig::default().target_f0_hz;
        assert!(
            (f0_out - target).abs() <= sr / n as f32,
            "comb at {f0_out:.1} Hz, canonical register is {target} Hz"
        );
    }

    /// Unvoiced frames must keep the channel-vocoder behaviour, which is the
    /// right model for fricatives and noise.
    #[test]
    fn unvoiced_frames_keep_per_bin_phase() {
        let (n, hop, sr) = (1024usize, 256usize, 48_000.0f32);
        let half = n / 2 + 1;
        let offsets: Vec<f32> = (0..half).map(|k| (k as f32 * 0.7) % TAU).collect();
        let mut st = SpectralState::new(n, hop, sr, &offsets);
        let mut spec = vec![Complex::new(1.0f32, 0.0); half];
        st.transform(&mut spec, 1.0, 1.0, None, PitchEstimate::default());

        let ang = |c: Complex<f32>| c.im.atan2(c.re);
        let (a, b) = (ang(spec[100]), ang(spec[101]));
        assert!(
            (a - b).abs() > 1e-3,
            "neighbouring bins should not be locked"
        );
    }
}
