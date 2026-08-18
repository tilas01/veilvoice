// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Analysis/synthesis windowing helpers.

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
