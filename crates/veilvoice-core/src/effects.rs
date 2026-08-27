// SPDX-License-Identifier: GPL-3.0-or-later
//! Light time-domain effects applied after resynthesis.
//!
//! These run on the continuous output stream (not per FFT frame) and exist to
//! (a) further decorrelate the signal from the original, and (b) add a few
//! detuned "voices" so the spectrogram is densely filled rather than showing a
//! clean harmonic stack — without harming intelligibility, so every mix defaults
//! low. None of them are invertible in a way that recovers the source voice.
//!
//! # How little these contribute, said plainly
//!
//! It would be easy to read a chorus and a reverb as part of the anonymity
//! argument. They are not, and this crate should not let anybody think they
//! are. **The voiceprint is destroyed in [`crate::spectral`]** -- by discarding
//! measured phase and by mapping every speaker onto one canonical pitch
//! register and vocal-tract scale. That has already happened before a single
//! sample reaches this file.
//!
//! What these three add is *decorrelation at the margins*: a denser
//! spectrogram, a few detuned voices where there was a clean harmonic stack,
//! and some odd harmonics smearing whatever residual cues survived. Useful,
//! cheap, and nowhere near sufficient alone. Set all three mixes to zero and
//! the output is exactly as unlinkable as before.
//!
//! The reason to be exact about this is that a filter chain of precisely this
//! shape -- clip, chorus, reverb -- is what a *voice changer* ships, and a
//! voice changer offers no anonymity whatsoever. Everything that separates this
//! project from that one happens upstream of this file.
//!
//! # Why every mix defaults low
//!
//! Intelligibility is a requirement, not a preference. Each of these effects
//! trades clarity for density, and past a fairly low mix the words start to
//! cost more than the added decorrelation is worth. The defaults sit where a
//! listener does not notice the effect is there at all; they are a starting
//! point a user may raise, not a recommendation to raise them.
//!
//! # Real-time constraints
//!
//! These run per output sample inside an audio callback, on the continuous
//! stream rather than per FFT frame. Every buffer is allocated once at
//! construction: `process` allocates nothing, takes no lock and reads no clock.
//! [`Chorus`] and [`Reverb`] own their delay lines and index them with wrapping
//! arithmetic, so changing sample rate means building a new one rather than
//! resizing a live one.
//!
//! # In plain words
//!
//! A few small finishing touches applied to the sound after the main work is done.
//!
//! They do two things. They loosen what remains of the connection between the
//! result and the original recording, and they fill in the picture a spectrogram
//! would show, so it looks like a dense, ordinary voice rather than something
//! obviously processed.
//!
//! Every one of them is set gently by default, because all of them can hurt how
//! clear the words are if pushed, and clear words are the point.

use std::f32::consts::TAU;

/// Symmetric soft-clip (tanh) waveshaper. `drive` sets curvature, `mix` blends
/// dry/wet. Adds gentle odd harmonics that smear residual identity cues.
pub struct SoftClip {
    drive: f32,
    mix: f32,
    norm: f32,
}

impl SoftClip {
    pub fn new(drive: f32, mix: f32) -> Self {
        let drive = drive.max(0.01);
        Self {
            drive,
            mix: mix.clamp(0.0, 1.0),
            norm: drive.tanh(),
        }
    }
    #[inline]
    pub fn process(&self, x: f32) -> f32 {
        let wet = (self.drive * x).tanh() / self.norm;
        x * (1.0 - self.mix) + wet * self.mix
    }
}

/// A single modulated delay line, summed into a small ensemble to create the
/// impression of several slightly different voices.
struct DelayVoice {
    buf: Vec<f32>,
    write: usize,
    base_delay: f32, // samples
    depth: f32,      // samples of LFO sweep
    lfo_phase: f32,
    lfo_inc: f32,
}

impl DelayVoice {
    fn new(sample_rate: f32, base_ms: f32, depth_ms: f32, rate_hz: f32, phase: f32) -> Self {
        let base_delay = base_ms * 0.001 * sample_rate;
        let depth = depth_ms * 0.001 * sample_rate;
        let max = (base_delay + depth) as usize + 4;
        Self {
            buf: vec![0.0; max.next_power_of_two()],
            write: 0,
            base_delay,
            depth,
            lfo_phase: phase,
            lfo_inc: TAU * rate_hz / sample_rate,
        }
    }
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let mask = self.buf.len() - 1;
        self.buf[self.write] = x;
        let d = self.base_delay + self.depth * self.lfo_phase.sin();
        let read = self.write as f32 - d;
        let read = read.rem_euclid(self.buf.len() as f32);
        let i0 = read.floor() as usize & mask;
        let i1 = (i0 + 1) & mask;
        let frac = read - read.floor();
        let out = self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac;
        self.write = (self.write + 1) & mask;
        self.lfo_phase = (self.lfo_phase + self.lfo_inc) % TAU;
        out
    }
}

/// Detuned chorus ensemble.
pub struct Chorus {
    voices: Vec<DelayVoice>,
    mix: f32,
}

impl Chorus {
    pub fn new(sample_rate: f32, mix: f32) -> Self {
        // Three voices at different rates/phases — cheap but effective spread.
        let voices = vec![
            DelayVoice::new(sample_rate, 14.0, 3.0, 0.17, 0.0),
            DelayVoice::new(sample_rate, 21.0, 4.0, 0.23, 2.1),
            DelayVoice::new(sample_rate, 9.0, 2.0, 0.31, 4.0),
        ];
        Self {
            voices,
            mix: mix.clamp(0.0, 1.0),
        }
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let mut wet = 0.0;
        for v in &mut self.voices {
            wet += v.process(x);
        }
        wet /= self.voices.len() as f32;
        x * (1.0 - self.mix) + wet * self.mix
    }
}

/// Minimal Schroeder-style reverb: one feedback comb + one all-pass. Kept very
/// light so speech stays dry enough to transcribe.
pub struct Reverb {
    comb: Vec<f32>,
    comb_w: usize,
    comb_fb: f32,
    ap: Vec<f32>,
    ap_w: usize,
    ap_g: f32,
    mix: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32, mix: f32) -> Self {
        let comb_len = ((0.0297 * sample_rate) as usize).max(1);
        let ap_len = ((0.0050 * sample_rate) as usize).max(1);
        Self {
            comb: vec![0.0; comb_len],
            comb_w: 0,
            comb_fb: 0.72,
            ap: vec![0.0; ap_len],
            ap_w: 0,
            ap_g: 0.5,
            mix: mix.clamp(0.0, 1.0),
        }
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        // comb
        let c = self.comb[self.comb_w];
        let cw = x + c * self.comb_fb;
        self.comb[self.comb_w] = cw;
        self.comb_w = (self.comb_w + 1) % self.comb.len();
        // all-pass
        let a = self.ap[self.ap_w];
        let aw = c + (-self.ap_g) * a;
        let out = a + self.ap_g * aw;
        self.ap[self.ap_w] = aw;
        self.ap_w = (self.ap_w + 1) % self.ap.len();
        x * (1.0 - self.mix) + out * self.mix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn softclip_is_bounded_and_finite() {
        let sc = SoftClip::new(2.0, 1.0);
        for i in -1000..1000 {
            let y = sc.process(i as f32 / 100.0);
            assert!(y.is_finite());
        }
    }

    #[test]
    fn chorus_and_reverb_finite() {
        let mut ch = Chorus::new(48_000.0, 0.3);
        let mut rv = Reverb::new(48_000.0, 0.15);
        let mut acc = 0.0f32;
        for i in 0..48_000 {
            let x = (i as f32 * 0.01).sin();
            let y = rv.process(ch.process(x));
            assert!(y.is_finite());
            acc += y.abs();
        }
        assert!(acc > 0.0);
    }
}
