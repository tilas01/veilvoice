// SPDX-License-Identifier: GPL-3.0-or-later
//! Streaming short-time Fourier transform with overlap-add resynthesis.
//!
//! Structure follows the classic FIFO/overlap-add pipeline (as popularised by
//! Bernsee's SMB pitch shifter): samples flow in and out one-for-one with a
//! fixed latency of `n - hop` samples, and a full frame is analysed/synthesised
//! every `hop` input samples. The caller supplies a closure that rewrites the
//! complex spectrum in place, keeping the FFT plumbing and the de-identification
//! maths cleanly separated.
//!
//! The closure also receives the raw (unwindowed) analysis frame. Accent
//! neutralisation needs a time-domain view to track f0 — the FFT resolution at
//! useful frame sizes is far too coarse for that — and handing over the frame
//! that produced the spectrum keeps the two perfectly aligned. Its newest `hop`
//! samples are the tail.
//!
//! # In plain words
//!
//! Sound arrives as a long stream of numbers. To change a voice you have to look
//! at it in terms of pitch and tone rather than raw numbers, and this is the part
//! that converts back and forth.
//!
//! It takes a short slice of sound, works out which frequencies are in it, hands
//! that picture to the code that alters it, and turns the result back into sound.
//! The slices overlap and are faded together, so the joins cannot be heard.
//!
//! Everything else in the engine is written in terms of those pictures. This file
//! is the door between the two ways of looking at the same thing.

use crate::window::{hann, ola_gain};
use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::sync::Arc;

/// Reusable streaming STFT engine (single channel).
pub struct StftEngine {
    n: usize,
    hop: usize,
    latency: usize, // n - hop
    window: Vec<f32>,
    norm: f32, // overlap-add gain

    r2c: Arc<dyn RealToComplex<f32>>,
    c2r: Arc<dyn ComplexToReal<f32>>,
    fwd_scratch: Vec<Complex<f32>>,
    inv_scratch: Vec<Complex<f32>>,

    in_fifo: Vec<f32>,           // len n
    out_fifo: Vec<f32>,          // len n
    out_accum: Vec<f32>,         // len 2n
    frame_in: Vec<f32>,          // len n (windowed analysis frame)
    frame_out: Vec<f32>,         // len n (inverse output)
    spectrum: Vec<Complex<f32>>, // len n/2+1
    rover: usize,
}

impl StftEngine {
    /// `n` must be even; `hop` must divide evenly for constant overlap-add
    /// (typical: hop = n/4).
    pub fn new(n: usize, hop: usize) -> Self {
        assert!(n >= 2 && n.is_multiple_of(2), "FFT size must be even");
        assert!(hop > 0 && hop < n, "hop must be in (0, n)");
        let window = hann(n);
        let norm = ola_gain(&window, hop);

        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(n);
        let c2r = planner.plan_fft_inverse(n);
        let fwd_scratch = r2c.make_scratch_vec();
        let inv_scratch = c2r.make_scratch_vec();

        Self {
            n,
            hop,
            latency: n - hop,
            window,
            norm,
            fwd_scratch,
            inv_scratch,
            in_fifo: vec![0.0; n],
            out_fifo: vec![0.0; n],
            out_accum: vec![0.0; 2 * n],
            frame_in: vec![0.0; n],
            frame_out: vec![0.0; n],
            spectrum: vec![Complex::new(0.0, 0.0); n / 2 + 1],
            r2c,
            c2r,
            rover: 0,
        }
    }

    /// End-to-end algorithmic latency (group delay) in samples.
    ///
    /// Empirically — and as the identity-reconstruction test asserts — the
    /// FIFO/overlap-add path delays the signal by exactly one frame (`n`), which
    /// is what the UI reports to the user. (`self.latency = n - hop` is the
    /// separate *internal* FIFO offset used for indexing.)
    pub fn latency_samples(&self) -> usize {
        self.n
    }

    /// Process `input` into `output` (equal length). `transform` is invoked once
    /// per analysed frame with the half-complex spectrum to rewrite in place and
    /// the raw analysis frame it came from (length `n`, newest samples last).
    pub fn process<F: FnMut(&mut [Complex<f32>], &[f32])>(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        mut transform: F,
    ) {
        assert_eq!(input.len(), output.len(), "input/output length mismatch");
        for (out, &x) in output.iter_mut().zip(input.iter()) {
            if self.rover == 0 {
                self.rover = self.latency;
            }
            // Non-finite input is replaced with silence *here*, at the one gate
            // every sample passes through, because the engine downstream keeps
            // persistent state and a single bad sample poisons it permanently.
            //
            // Found by the audit: one NaN — which a 32-bit-float WAV can
            // legally contain, and which `symphonia` faithfully decodes —
            // reached the accent neutraliser's long-term average, which is an
            // exponential moving average and therefore never recovers. Every
            // subsequent output sample was NaN, for the rest of the session,
            // with nothing reported. A file someone sent you is a realistic
            // source, and "the veiled recording came out silent and nobody said
            // why" is a bad way to find out.
            //
            // The magnitude bound is separate, and deliberately enormous. A
            // sample near `f32::MAX` produces an FFT bin near infinity, whose
            // square then *is* infinity, and the resulting NaN gets into the
            // same persistent averages by a different door. ±1e6 cannot
            // overflow the sums (1e6² × 1024 bins is ~1e15, against a float
            // ceiling of 3.4e38) while sitting six orders of magnitude above
            // any real audio, which is nominally ±1. It is a guard against
            // impossible values, not a limiter: nothing a microphone or a
            // decoder legitimately produces comes near it, and the engine's own
            // output is soft-clipped downstream regardless.
            self.in_fifo[self.rover] = if x.is_finite() {
                x.clamp(-1e6, 1e6)
            } else {
                0.0
            };
            *out = self.out_fifo[self.rover - self.latency];
            self.rover += 1;

            if self.rover >= self.n {
                self.rover = self.latency;
                self.process_frame(&mut transform);
            }
        }
    }

    fn process_frame<F: FnMut(&mut [Complex<f32>], &[f32])>(&mut self, transform: &mut F) {
        // analysis window
        for k in 0..self.n {
            self.frame_in[k] = self.in_fifo[k] * self.window[k];
        }
        self.r2c
            .process_with_scratch(
                &mut self.frame_in,
                &mut self.spectrum,
                &mut self.fwd_scratch,
            )
            .expect("forward FFT");

        // De-identification transform on the spectrum, alongside the raw frame
        // it was computed from (disjoint field borrows).
        transform(&mut self.spectrum, &self.in_fifo);

        // inverse FFT (destroys spectrum contents — fine, rebuilt each frame)
        self.c2r
            .process_with_scratch(
                &mut self.spectrum,
                &mut self.frame_out,
                &mut self.inv_scratch,
            )
            .expect("inverse FFT");

        // windowed overlap-add with FFT + OLA normalisation
        let scale = self.norm / self.n as f32;
        for k in 0..self.n {
            self.out_accum[k] += self.window[k] * self.frame_out[k] * scale;
        }

        // emit `hop` finished samples, then slide accumulators/fifo by `hop`
        self.out_fifo[..self.hop].copy_from_slice(&self.out_accum[..self.hop]);
        self.out_accum.copy_within(self.hop..self.hop + self.n, 0);
        for v in &mut self.out_accum[self.n..self.n + self.hop] {
            *v = 0.0;
        }
        self.in_fifo.copy_within(self.hop..self.n, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With an identity spectral transform the engine must reconstruct its input
    /// (delayed by the algorithmic latency) to high accuracy — this validates
    /// the windowing/overlap-add/normalisation maths.
    #[test]
    fn identity_reconstructs_input() {
        let n = 1024;
        let hop = n / 4;
        let mut eng = StftEngine::new(n, hop);
        let lat = eng.latency_samples();

        let total = 8192;
        let input: Vec<f32> = (0..total)
            .map(|i| (i as f32 * 0.05).sin() * 0.4 + (i as f32 * 0.011).sin() * 0.2)
            .collect();
        let mut output = vec![0.0; total];
        eng.process(&input, &mut output, |_spec, _frame| { /* identity */ });

        // The analysis/synthesis path delays the signal by exactly one frame.
        assert_eq!(lat, n, "reported latency should equal one frame");
        let mut max_err = 0.0f32;
        for i in (3 * n)..(total - n) {
            max_err = max_err.max((output[i] - input[i - lat]).abs());
        }
        assert!(max_err < 1e-3, "reconstruction error too high: {max_err}");
    }

    #[test]
    fn output_is_finite() {
        let mut eng = StftEngine::new(512, 128);
        let input: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut output = vec![0.0; 4096];
        eng.process(&input, &mut output, |spec, _frame| {
            for c in spec.iter_mut() {
                *c *= 0.9;
            }
        });
        assert!(output.iter().all(|v| v.is_finite()));
    }

    /// The frame handed to the closure must be the exact analysis window, so the
    /// pitch tracker stays aligned with the spectrum it accompanies.
    #[test]
    fn callback_frame_matches_the_analysed_window() {
        let (n, hop) = (256usize, 64usize);
        let mut eng = StftEngine::new(n, hop);
        let input: Vec<f32> = (0..2048).map(|i| i as f32).collect();
        let mut output = vec![0.0; 2048];
        let mut seen: Vec<f32> = Vec::new();
        eng.process(&input, &mut output, |_spec, frame| {
            assert_eq!(frame.len(), n);
            seen.push(frame[n - 1]);
        });
        // Each frame consumes exactly `hop` new samples, so the newest sample
        // advances by `hop` every call.
        assert!(seen.len() > 4);
        for w in seen.windows(2) {
            assert_eq!(w[1] - w[0], hop as f32, "frames must advance by one hop");
        }
    }
}
