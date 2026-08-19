// SPDX-License-Identifier: GPL-3.0-or-later
//! Accent and speaker-trait neutralisation.
//!
//! # What an accent is made of, and what a signal-level transform can remove
//!
//! Accent is carried by two very different kinds of cue, and they have opposite
//! answers here:
//!
//! * **Suprasegmental cues** — intonation contour and pitch range, long-term
//!   voice quality and spectral tilt, and the fixed vocal-tract scale behind a
//!   speaker's vowel space. These are *properties of the signal*, they are
//!   strongly speaker- and region-identifying, and this module removes them by
//!   mapping every speaker onto one canonical target.
//! * **Segmental cues** — *which phonemes the speaker actually produced*:
//!   rhoticity, vowel mergers, dental-fricative substitution, aspiration
//!   patterns. These are not a colouration laid over the words; at this level
//!   they **are** the words. Removing them means deciding a different phoneme
//!   was said, which no filter can do — it requires recognising the speech and
//!   re-synthesising it (see the planned text-to-speech mode, which sidesteps
//!   this entirely by never carrying the original signal at all).
//!
//! So this module makes every speaker land on the same pitch register, the same
//! apparent vocal-tract length and the same long-term spectral tilt, which
//! removes the accent's melody and colour and a large part of its perceived
//! origin. It does not, and cannot, re-articulate phonemes.
//! `docs/WHITEPAPER.md` must state that limit plainly rather than claim accent
//! removal is total.
//!
//! # Why this also strengthens de-identification
//!
//! Every step here is *many-to-one*: a whole population of input f0 contours,
//! spectral tilts and vocal-tract lengths is collapsed onto a single canonical
//! value. That destroys information rather than displacing it, so it composes
//! with the phase discard in [`crate::spectral`] — the two are independent
//! one-way steps, and normalising the speaker's *mean* pitch and vocal-tract
//! length removes two of the strongest biometric features there are.
//!
//! # Preserving intelligibility
//!
//! The critical design rule is that every correction is derived from a
//! **long-term** average, never from the current frame. Per-frame spectral shape
//! is what distinguishes /i/ from /u/; normalising it frame-by-frame would erase
//! the vowels along with the accent. Vocal-tract and tilt corrections therefore
//! use multi-second time constants, so they track the speaker and leave the
//! phonemes moving freely underneath.

use crate::pitch::PitchEstimate;

/// Lower edge of the band used to measure vocal-tract scale, in hertz.
const CENTROID_LO_HZ: f32 = 200.0;
/// Upper edge of the band used to measure vocal-tract scale, in hertz.
const CENTROID_HI_HZ: f32 = 3_500.0;
/// Lower edge of the band whose long-term tilt is normalised.
const LTAS_LO_HZ: f32 = 100.0;
/// Upper edge of the band whose long-term tilt is normalised.
const LTAS_HI_HZ: f32 = 7_500.0;
/// Reference frequency of the canonical spectral-tilt line, in hertz.
const TILT_REF_HZ: f32 = 500.0;
/// Maximum long-term shaping applied to any bin, in decibels.
const MAX_SHAPE_DB: f32 = 12.0;
/// Time constant for the intonation correction, in seconds.
const TAU_PROSODY_S: f32 = 0.025;
/// Time constant for the vocal-tract estimate, in seconds. Deliberately long:
/// it must track the *speaker*, not the current vowel.
const TAU_VTLN_S: f32 = 3.0;
/// Time constant for the long-term average spectrum, in seconds.
const TAU_LTAS_S: f32 = 2.0;
/// Seconds of voiced audio over which corrections fade in from nothing.
const WARMUP_S: f32 = 0.75;

/// How aggressively accent and speaker traits are normalised.
///
/// Each strength is in `[0, 1]`, where 0 leaves that trait untouched and 1 maps
/// every speaker fully onto the canonical target.
#[derive(Clone, Copy, Debug)]
pub struct AccentConfig {
    /// Master switch. When false the neutraliser is bypassed entirely.
    pub enabled: bool,
    /// How much of the speaker's intonation contour is replaced by a canonical
    /// one. At 1.0 — the default — the output sits at a steady
    /// [`AccentConfig::target_f0_hz`], which removes the intonation pattern
    /// entirely; the melody of an accent is one of its strongest cues, and a
    /// constant carries no speaker information at all.
    ///
    /// Values below 1.0 keep that fraction of the speaker's excursion, but note
    /// that voiced resynthesis quantises the fundamental to the STFT bin grid
    /// (see [`crate::spectral`]), so partial flattening steps rather than
    /// glides. Full flattening needs only one snap and is unaffected.
    pub prosody_flatten: f32,
    /// Pitch register every speaker is mapped onto, in hertz.
    pub target_f0_hz: f32,
    /// How much of the speaker's vocal-tract scale is normalised away.
    pub vtln_strength: f32,
    /// Canonical long-term envelope centroid, in hertz — the vocal-tract scale
    /// every speaker is mapped onto.
    pub target_centroid_hz: f32,
    /// How much of the speaker's long-term spectral tilt is rotated onto the
    /// canonical slope. Deliberately a broad tilt only, never a bin-by-bin
    /// match — see [`AccentNeutralizer::shape`].
    pub ltas_strength: f32,
    /// Slope of the canonical long-term spectrum, in dB per octave.
    pub target_tilt_db_oct: f32,
}

impl Default for AccentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prosody_flatten: 1.0,
            // Between typical male and female registers, so neither is pushed
            // far enough to lose intelligibility.
            target_f0_hz: 155.0,
            vtln_strength: 0.85,
            target_centroid_hz: 750.0,
            ltas_strength: 0.7,
            // A gently falling spectrum, close to natural voiced speech.
            target_tilt_db_oct: -6.0,
        }
    }
}

/// Live read-out of what the neutraliser is currently doing, for the UI.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccentStats {
    /// Most recent detected fundamental, in hertz (0 when unvoiced).
    pub detected_f0_hz: f32,
    /// Whether the most recent frame was voiced.
    pub voiced: bool,
    /// Periodicity confidence of the most recent estimate, in `[0, 1]`.
    pub voicing_confidence: f32,
    /// Pitch ratio currently applied to reach the canonical register.
    pub prosody_ratio: f32,
    /// Formant ratio currently applied to reach the canonical vocal tract.
    pub vtln_ratio: f32,
    /// Long-term envelope centroid measured for this speaker, in hertz.
    pub speaker_centroid_hz: f32,
    /// Warm-up progress in `[0, 1]`; corrections are scaled by this.
    pub warmup: f32,
}

/// Maps any speaker onto one canonical pitch register, vocal-tract scale and
/// long-term spectrum.
pub struct AccentNeutralizer {
    cfg: AccentConfig,
    bin_hz: f32,
    // measurement bands, in bins
    cent_lo: usize,
    cent_hi: usize,
    ltas_lo: usize,
    ltas_hi: usize,
    // smoothing coefficients, per frame
    a_prosody: f32,
    a_vtln: f32,
    a_ltas: f32,
    warm_step: f32,
    // running state
    prosody_ratio: f32,
    centroid_hz: f32,
    vtln_ratio: f32,
    ltas_db: Vec<f32>,
    shape_db: Vec<f32>,
    started: bool,
    warmup: f32,
    last_pitch: PitchEstimate,
}

impl AccentNeutralizer {
    /// Build for a given spectrum size and sample rate. `half` is `n/2 + 1`.
    pub fn new(cfg: AccentConfig, sample_rate: f32, n: usize, hop: usize, half: usize) -> Self {
        let bin_hz = sample_rate / n as f32;
        let per_frame = hop as f32 / sample_rate;
        let coeff = |tau: f32| 1.0 - (-per_frame / tau).exp();
        let bin_of = |hz: f32| ((hz / bin_hz).round() as usize).clamp(1, half - 1);

        Self {
            bin_hz,
            cent_lo: bin_of(CENTROID_LO_HZ),
            cent_hi: bin_of(CENTROID_HI_HZ.min(sample_rate * 0.45)),
            ltas_lo: bin_of(LTAS_LO_HZ),
            ltas_hi: bin_of(LTAS_HI_HZ.min(sample_rate * 0.45)),
            a_prosody: coeff(TAU_PROSODY_S),
            a_vtln: coeff(TAU_VTLN_S),
            a_ltas: coeff(TAU_LTAS_S),
            warm_step: per_frame / WARMUP_S,
            prosody_ratio: 1.0,
            centroid_hz: 0.0,
            vtln_ratio: 1.0,
            ltas_db: vec![0.0; half],
            shape_db: vec![0.0; half],
            started: false,
            warmup: 0.0,
            last_pitch: PitchEstimate::default(),
            cfg,
        }
    }

    /// Whether neutralisation is active.
    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    /// Live read-out for the UI.
    pub fn stats(&self) -> AccentStats {
        AccentStats {
            detected_f0_hz: self.last_pitch.f0_hz,
            voiced: self.last_pitch.voiced,
            voicing_confidence: self.last_pitch.confidence,
            prosody_ratio: self.prosody_ratio,
            vtln_ratio: self.vtln_ratio,
            speaker_centroid_hz: self.centroid_hz,
            warmup: self.warmup,
        }
    }

    /// Feed this frame's f0 estimate and update the intonation correction.
    ///
    /// The estimate is produced by the chain rather than here, because the
    /// harmonic-locked resynthesis in [`crate::spectral`] needs the same reading
    /// even when accent neutralisation is switched off.
    pub fn observe(&mut self, est: PitchEstimate) {
        if !self.cfg.enabled {
            return;
        }
        self.last_pitch = est;

        if est.voiced && est.f0_hz > 0.0 {
            self.warmup = (self.warmup + self.warm_step).min(1.0);
            // Map f0 onto the canonical register, keeping (1 - flatten) of the
            // speaker's log-domain excursion.
            let s = self.cfg.prosody_flatten.clamp(0.0, 1.0) * self.warmup;
            let want = (self.cfg.target_f0_hz / est.f0_hz).powf(s).clamp(0.5, 2.0);
            self.prosody_ratio += (want - self.prosody_ratio) * self.a_prosody;
        }
        // Unvoiced frames hold the last ratio: snapping back to 1.0 during
        // consonants would produce an audible warble at every stop.
    }

    /// Pitch ratio to apply to the excitation this frame.
    pub fn prosody_ratio(&self) -> f32 {
        if self.cfg.enabled {
            self.prosody_ratio
        } else {
            1.0
        }
    }

    /// Measure the speaker's vocal-tract scale from the *unwarped* envelope and
    /// update the VTLN ratio. Uses a multi-second average, so it tracks the
    /// speaker rather than the current vowel.
    pub fn measure_envelope(&mut self, env: &[f32]) {
        if !self.cfg.enabled || !self.last_pitch.voiced {
            return;
        }
        let Some(c) = log_centroid(env, self.cent_lo, self.cent_hi, self.bin_hz) else {
            return;
        };
        if self.centroid_hz <= 0.0 {
            self.centroid_hz = c;
        } else {
            self.centroid_hz += (c - self.centroid_hz) * self.a_vtln;
        }
        let s = self.cfg.vtln_strength.clamp(0.0, 1.0) * self.warmup;
        self.vtln_ratio = (self.cfg.target_centroid_hz / self.centroid_hz)
            .powf(s)
            .clamp(0.72, 1.40);
    }

    /// Formant ratio to apply to the envelope this frame.
    pub fn vtln_ratio(&self) -> f32 {
        if self.cfg.enabled {
            self.vtln_ratio
        } else {
            1.0
        }
    }

    /// Rotate the already-warped envelope toward the canonical spectral tilt,
    /// then fold the result back into the running average.
    ///
    /// Measuring *after* the correction closes a slow feedback loop: the stored
    /// curve converges on whatever is needed to put the output's long-term tilt
    /// on the canonical slope, which automatically accounts for the pitch and
    /// formant warping applied upstream.
    ///
    /// The correction is renormalised to preserve the frame's energy exactly.
    /// Centring the ramp only makes it *approximately* level-neutral — the
    /// centroid is an energy-weighted quantity and a dB-symmetric curve is not
    /// energy-symmetric — so the level is pinned explicitly rather than assumed.
    pub fn shape(&mut self, env: &mut [f32]) {
        if !self.cfg.enabled {
            return;
        }
        // Apply the curve computed from previous frames, preserving energy.
        if self.started {
            let before: f64 = env.iter().map(|v| (*v as f64) * (*v as f64)).sum();
            for (e, &db) in env.iter_mut().zip(self.shape_db.iter()) {
                *e *= db_to_gain(db);
            }
            let after: f64 = env.iter().map(|v| (*v as f64) * (*v as f64)).sum();
            if after > 1e-30 && before > 0.0 {
                let g = (before / after).sqrt() as f32;
                for e in env.iter_mut() {
                    *e *= g;
                }
            }
        }
        if !self.last_pitch.voiced {
            return; // silence and consonant noise must not steer the average
        }

        // Fold the corrected envelope into the long-term average.
        let a = if self.started { self.a_ltas } else { 1.0 };
        for (avg, &e) in self.ltas_db.iter_mut().zip(env.iter()) {
            *avg += (gain_to_db(e) - *avg) * a;
        }
        self.started = true;
        self.recompute_shape();
    }

    /// Rebuild the correction curve from the current long-term average.
    ///
    /// The curve is deliberately **a straight line in log-frequency and nothing
    /// more**: the measured long-term spectrum is reduced to a single slope, and
    /// the correction is the rotation that carries that slope onto the canonical
    /// one. This is the strongest form of long-term colour correction that is
    /// *structurally incapable* of damaging intelligibility — a smooth monotone
    /// ramp adds the same offset to every speaker's vowels, so formant-scale
    /// contrast passes through untouched. Matching the long-term spectrum
    /// bin-by-bin would remove more speaker colour, but it also flattens the
    /// per-frame differences that distinguish one vowel from another, which is
    /// not a trade this project is willing to make.
    fn recompute_shape(&mut self) {
        let (lo, hi) = (self.ltas_lo, self.ltas_hi);
        if hi <= lo {
            return;
        }

        // Weighted least-squares line fit of measured dB against log-frequency.
        // Weighting by 1/k makes every octave count equally; FFT bins are linear
        // in frequency, so an unweighted fit would be dominated by the top
        // octave alone.
        let (mut sw, mut su, mut suu, mut sy, mut suy) = (0.0f32, 0.0, 0.0, 0.0, 0.0);
        for k in lo..=hi {
            let u = ((k as f32 * self.bin_hz).max(1.0) / TILT_REF_HZ).log2();
            let y = self.ltas_db[k];
            let w = 1.0 / k as f32;
            sw += w;
            su += w * u;
            suu += w * u * u;
            sy += w * y;
            suy += w * u * y;
        }
        let denom = sw * suu - su * su;
        if denom.abs() < 1e-12 {
            return;
        }
        let measured_tilt = (sw * suy - su * sy) / denom;
        let u_mid = su / sw;

        // Rotate about the band's log-frequency centroid by the slope error.
        let s = self.cfg.ltas_strength.clamp(0.0, 1.0) * self.warmup;
        let delta = (self.cfg.target_tilt_db_oct - measured_tilt) * s;
        let (u_lo, u_hi) = (
            ((lo as f32 * self.bin_hz).max(1.0) / TILT_REF_HZ).log2(),
            ((hi as f32 * self.bin_hz).max(1.0) / TILT_REF_HZ).log2(),
        );
        for (k, db) in self.shape_db.iter_mut().enumerate() {
            // Outside the band the ramp is held at its edge value rather than
            // dropped to zero, which would put a step at the band edge.
            let u = ((k as f32 * self.bin_hz).max(1.0) / TILT_REF_HZ)
                .log2()
                .clamp(u_lo, u_hi);
            *db = (delta * (u - u_mid)).clamp(-MAX_SHAPE_DB, MAX_SHAPE_DB);
        }
    }
}

/// Energy-weighted geometric-mean frequency of `env` over `[lo, hi]` bins.
///
/// The geometric (log-frequency) mean is the right measure here: vocal-tract
/// length scales the whole spectrum multiplicatively, so a log-domain centroid
/// moves by a constant offset when the tract scales, and the ratio of two
/// centroids is exactly the warp factor between two speakers.
fn log_centroid(env: &[f32], lo: usize, hi: usize, bin_hz: f32) -> Option<f32> {
    if hi <= lo || hi >= env.len() {
        return None;
    }
    let mut wsum = 0.0f32;
    let mut lsum = 0.0f32;
    for (i, &v) in env[lo..=hi].iter().enumerate() {
        let w = v.max(0.0);
        let f = ((lo + i) as f32 * bin_hz).max(1.0);
        lsum += w * f.ln();
        wsum += w;
    }
    if wsum <= 1e-12 {
        return None;
    }
    let c = (lsum / wsum).exp();
    c.is_finite().then_some(c)
}

fn gain_to_db(x: f32) -> f32 {
    20.0 * (x.abs() + 1e-9).log10()
}

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spectral::box_smooth;

    const SR: f32 = 48_000.0;
    const N: usize = 1024;
    const HOP: usize = 256;
    const HALF: usize = N / 2 + 1;

    /// The neutraliser plus the pitch tracker the chain normally drives it with,
    /// so tests can feed raw frames the way `Deidentifier` does.
    struct Rig {
        inner: AccentNeutralizer,
        tracker: crate::pitch::PitchTracker,
    }

    impl Rig {
        fn observe_frame(&mut self, frame: &[f32], hop: usize) {
            self.tracker.push(&frame[frame.len() - hop..]);
            let est = self.tracker.estimate();
            self.inner.observe(est);
        }
    }

    impl std::ops::Deref for Rig {
        type Target = AccentNeutralizer;
        fn deref(&self) -> &AccentNeutralizer {
            &self.inner
        }
    }

    impl std::ops::DerefMut for Rig {
        fn deref_mut(&mut self) -> &mut AccentNeutralizer {
            &mut self.inner
        }
    }

    fn neutralizer(cfg: AccentConfig) -> Rig {
        Rig {
            inner: AccentNeutralizer::new(cfg, SR, N, HOP, HALF),
            tracker: crate::pitch::PitchTracker::new(SR),
        }
    }

    /// A voiced frame: sawtooth excitation shaped by a vocal tract of a given
    /// scale (`vtl` > 1 = longer tract = lower formants).
    fn voiced_frame(f0: f32, vtl: f32, i0: usize) -> Vec<f32> {
        (0..N)
            .map(|i| {
                let t = (i0 + i) as f32 / SR;
                let mut s = 0.0;
                for h in 1..=24 {
                    let f = f0 * h as f32;
                    if f > SR * 0.45 {
                        break;
                    }
                    // Three formants, scaled by vocal-tract length.
                    let mut g = 1.0 / h as f32;
                    for &cf in &[700.0f32, 1220.0, 2600.0] {
                        let c = cf / vtl;
                        g += 0.9 / (1.0 + ((f - c) / 110.0).powi(2)) / h as f32;
                    }
                    s += g * (std::f32::consts::TAU * f * t).sin();
                }
                s * 0.1
            })
            .collect()
    }

    /// Smooth spectral envelope of a frame, matching what the chain feeds in.
    fn envelope_of(frame: &[f32]) -> Vec<f32> {
        use realfft::num_complex::Complex;
        use realfft::RealFftPlanner;
        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(N);
        let mut input: Vec<f32> = frame
            .iter()
            .enumerate()
            .map(|(i, x)| {
                let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos();
                x * w
            })
            .collect();
        let mut spec = vec![Complex::new(0.0, 0.0); HALF];
        r2c.process(&mut input, &mut spec).unwrap();
        let mag: Vec<f32> = spec.iter().map(|c| c.norm()).collect();
        let mut tmp = vec![0.0; HALF];
        let mut env = vec![0.0; HALF];
        box_smooth(&mag, 21, &mut tmp);
        box_smooth(&tmp, 21, &mut env);
        env
    }

    /// Run a synthetic speaker through the neutraliser and report the ratios it
    /// settles on, plus the centroid it measured.
    fn settle(f0: f32, vtl: f32, cfg: AccentConfig) -> (f32, f32, f32) {
        let mut a = neutralizer(cfg);
        for f in 0..400 {
            let frame = voiced_frame(f0, vtl, f * HOP);
            a.observe_frame(&frame, HOP);
            let env = envelope_of(&frame);
            a.measure_envelope(&env);
        }
        let s = a.stats();
        (s.prosody_ratio, s.vtln_ratio, s.speaker_centroid_hz)
    }

    #[test]
    fn two_speakers_converge_to_one_pitch_register() {
        let cfg = AccentConfig::default();
        let low = settle(105.0, 1.12, cfg); // deeper voice, longer tract
        let high = settle(225.0, 0.88, cfg); // higher voice, shorter tract

        let out_low = 105.0 * low.0;
        let out_high = 225.0 * high.0;
        let before = (225.0f32 / 105.0).log2().abs();
        let after = (out_high / out_low).log2().abs();

        assert!(
            after < before * 0.3,
            "pitch registers should converge: {before:.3} octaves before, \
             {after:.3} after ({out_low:.0} Hz vs {out_high:.0} Hz)"
        );
        for out in [out_low, out_high] {
            let err = (out / cfg.target_f0_hz).log2().abs();
            assert!(err < 0.35, "{out:.0} Hz is far from the canonical register");
        }
    }

    #[test]
    fn two_speakers_converge_to_one_vocal_tract() {
        let cfg = AccentConfig::default();
        let (_, r_long, c_long) = settle(105.0, 1.20, cfg);
        let (_, r_short, c_short) = settle(225.0, 0.85, cfg);

        let before = (c_short / c_long).log2().abs();
        let after = ((c_short * r_short) / (c_long * r_long)).log2().abs();
        assert!(before > 0.15, "test speakers should differ to begin with");
        assert!(
            after < before * 0.4,
            "vocal tracts should converge: {before:.3} octaves before, {after:.3} after"
        );
    }

    #[test]
    fn flatten_zero_leaves_intonation_alone() {
        let cfg = AccentConfig {
            prosody_flatten: 0.0,
            ..Default::default()
        };
        let (ratio, _, _) = settle(105.0, 1.0, cfg);
        assert!((ratio - 1.0).abs() < 0.02, "ratio={ratio}");
    }

    #[test]
    fn full_flatten_hits_the_canonical_register() {
        let cfg = AccentConfig {
            prosody_flatten: 1.0,
            ..Default::default()
        };
        for &f0 in &[105.0f32, 155.0, 225.0] {
            let (ratio, _, _) = settle(f0, 1.0, cfg);
            let out = f0 * ratio;
            assert!(
                (out / cfg.target_f0_hz).log2().abs() < 0.1,
                "f0={f0} -> {out:.0} Hz, want {}",
                cfg.target_f0_hz
            );
        }
    }

    #[test]
    fn disabled_is_a_true_bypass() {
        let cfg = AccentConfig {
            enabled: false,
            ..Default::default()
        };
        let mut a = neutralizer(cfg);
        let frame = voiced_frame(120.0, 1.0, 0);
        a.observe_frame(&frame, HOP);
        let env = envelope_of(&frame);
        a.measure_envelope(&env);
        assert_eq!(a.prosody_ratio(), 1.0);
        assert_eq!(a.vtln_ratio(), 1.0);

        let mut shaped = env.clone();
        a.shape(&mut shaped);
        assert_eq!(shaped, env, "shaping must not touch the envelope when off");
    }

    #[test]
    fn shaping_is_gain_neutral_and_finite() {
        let mut a = neutralizer(AccentConfig::default());
        let mut energy_in = 0.0f64;
        let mut energy_out = 0.0f64;
        for f in 0..300 {
            let frame = voiced_frame(140.0, 1.05, f * HOP);
            a.observe_frame(&frame, HOP);
            let env = envelope_of(&frame);
            a.measure_envelope(&env);
            let mut shaped = env.clone();
            a.shape(&mut shaped);
            assert!(shaped.iter().all(|v| v.is_finite()));
            if f > 200 {
                energy_in += env.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
                energy_out += shaped.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
            }
        }
        let ratio = energy_out / energy_in;
        assert!(
            (0.4..2.5).contains(&ratio),
            "long-term shaping changed the level by {ratio:.2}x"
        );
    }

    #[test]
    fn per_frame_vowel_contrast_survives() {
        // The whole intelligibility argument: normalisation is long-term, so two
        // different vowels from the same speaker must stay clearly different.
        let cfg = AccentConfig::default();
        let mut a = neutralizer(cfg);
        // Settle on the speaker using an /a/-like frame.
        for f in 0..300 {
            let frame = voiced_frame(140.0, 1.0, f * HOP);
            a.observe_frame(&frame, HOP);
            a.measure_envelope(&envelope_of(&frame));
        }
        // Now compare two distinct vowels shaped by the settled curve.
        let mut shaped = Vec::new();
        for &vtl in &[0.75f32, 1.35] {
            let frame = voiced_frame(140.0, vtl, 0);
            let mut env = envelope_of(&frame);
            a.shape(&mut env);
            shaped.push(log_centroid(&env, a.cent_lo, a.cent_hi, a.bin_hz).unwrap());
        }
        let sep = (shaped[0] / shaped[1]).log2().abs();
        assert!(sep > 0.15, "vowel contrast collapsed to {sep:.3} octaves");
    }

    #[test]
    fn warmup_ramps_corrections_in_from_nothing() {
        let mut a = neutralizer(AccentConfig::default());
        assert_eq!(a.stats().warmup, 0.0);
        assert_eq!(a.prosody_ratio(), 1.0);
        for f in 0..400 {
            let frame = voiced_frame(220.0, 1.0, f * HOP);
            a.observe_frame(&frame, HOP);
            a.measure_envelope(&envelope_of(&frame));
        }
        assert_eq!(a.stats().warmup, 1.0);
        assert!(a.prosody_ratio() < 0.95, "correction never engaged");
    }

    #[test]
    fn unvoiced_input_leaves_state_untouched() {
        let mut a = neutralizer(AccentConfig::default());
        let quiet = vec![0.0f32; N];
        for _ in 0..100 {
            a.observe_frame(&quiet, HOP);
            a.measure_envelope(&vec![0.0f32; HALF]);
        }
        assert_eq!(a.prosody_ratio(), 1.0);
        assert_eq!(a.vtln_ratio(), 1.0);
        assert_eq!(a.stats().warmup, 0.0);
    }
}
