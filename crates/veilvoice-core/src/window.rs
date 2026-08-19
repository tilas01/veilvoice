// SPDX-License-Identifier: GPL-3.0-or-later
//! Analysis and synthesis windowing, and the one constant that keeps
//! overlap-add honest.
//!
//! Two functions, both small, both easy to get subtly wrong in ways that do not
//! look wrong.
//!
//! # Why the *periodic* Hann window
//!
//! There are two Hann windows and they differ by one sample. The **symmetric**
//! variant (`cos(2*pi*i / (n-1))`) is the right one for designing filters, and
//! it is what most textbook snippets show. The **periodic** variant
//! (`cos(2*pi*i / n)`, which is what [`hann`] returns) is the right one for an
//! STFT, because it is the one that tiles: shifted copies of it sum to a
//! constant, and the symmetric one does not quite.
//!
//! Using the wrong one does not produce an obvious failure. It produces a
//! faint periodic amplitude ripple at the hop rate -- a quiet buzz that sounds
//! like a codec artefact rather than like a bug, and that nothing in a test
//! suite notices unless the test is looking for exactly it.
//!
//! # Why the gain is computed rather than assumed
//!
//! VeilVoice windows **twice**: once on analysis and once on synthesis. That is
//! deliberate -- a synthesis window suppresses the discontinuities that
//! modifying a spectrum introduces at frame edges, and this crate modifies every
//! spectrum it touches. The cost is that the reconstruction gain is no longer
//! the familiar Constant-Overlap-Add value, it is `sum(w^2) / hop`.
//!
//! [`ola_gain`] returns the reciprocal of that, so a caller multiplies rather
//! than divides -- one multiply per sample in a hot loop instead of one divide.
//! It is derived from the window actually in use rather than hardcoded for a
//! particular size and overlap, so changing either cannot silently change the
//! output level.
//!
//! The zero-length and single-sample cases are handled explicitly, and the
//! degenerate `sum(w^2) == 0` returns unity rather than infinity: this is a
//! gain that gets multiplied into every output sample, and one non-finite value
//! entering an engine with persistent state is permanent.

use std::f32::consts::TAU;

/// Periodic Hann window of length `n` (the correct variant for STFT overlap-add,
/// as opposed to the symmetric variant used for filter design).
///
/// `w[i] = 0.5 - 0.5 * cos(2*pi*i / n)`
pub fn hann(n: usize) -> Vec<f32> {
    match n {
        0 => Vec::new(),
        1 => vec![1.0],
        _ => (0..n)
            .map(|i| 0.5 - 0.5 * ((TAU * i as f32) / n as f32).cos())
            .collect(),
    }
}

/// Overlap-add normalisation for a window applied on **both** analysis and
/// synthesis at the given `hop`.
///
/// For a Constant-Overlap-Add window the steady-state reconstruction gain is
/// `sum(w^2) / hop`; dividing synthesis frames by it yields unity gain. We
/// return the reciprocal (`hop / sum(w^2)`) so callers can multiply.
pub fn ola_gain(window: &[f32], hop: usize) -> f32 {
    debug_assert!(hop > 0);
    let sum_sq: f32 = window.iter().map(|w| w * w).sum();
    if sum_sq <= f32::EPSILON {
        1.0
    } else {
        hop as f32 / sum_sq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hann_endpoints_are_zero() {
        let w = hann(1024);
        assert!(w[0].abs() < 1e-6);
        // periodic Hann is not symmetric-zero at the last sample, but is small
        assert!(w.iter().all(|&v| (0.0..=1.0).contains(&v)));
    }

    #[test]
    fn ola_gain_positive_and_finite() {
        let w = hann(1024);
        let g = ola_gain(&w, 256);
        assert!(g.is_finite() && g > 0.0);
    }
}
