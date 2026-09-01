// SPDX-License-Identifier: GPL-3.0-or-later
//! Cryptographically-seeded modulation of the effect parameters.
//!
//! The pitch and formant ratios are never constant: a ChaCha20 CSPRNG picks a
//! new random target every `frames_per_target` STFT frames, and a one-pole
//! filter glides continuously toward it. Because the transform is therefore
//! non-stationary and unpredictable, an attacker cannot "undo" it by assuming a
//! single fixed shift: there is no single shift to undo, and the target
//! sequence is unknowable without the seed (which never leaves the process and
//! is zeroized on drop).
//!
//! The seed does not stay put either. It is rolled forward every couple of
//! seconds by default (see [`Modulator::reseed`]), so the stream driving any
//! given stretch of audio is closed off permanently once that stretch is past.
//!
//! # In plain words
//!
//! The amount by which the voice is altered is never held still. It drifts,
//! constantly and unpredictably.
//!
//! The drift comes from the same kind of random number generator used for
//! encryption, so it cannot be guessed, worked out from what came before, or
//! reproduced by somebody who has the recording. It slides between values rather
//! than jumping, so nothing about it can be heard.
//!
//! This is what stops the transform from being reversed by anybody who works out
//! the settings, because there is no single setting to work out.

use rand::Rng;
use rand::RngCore;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use zeroize::Zeroize;

/// One smoothly-varying parameter bounded to `[lo, hi]`.
struct Param {
    lo: f32,
    hi: f32,
    current: f32,
    target: f32,
    smooth: f32, // one-pole coefficient in (0,1]; larger = snappier
}

impl Param {
    fn new(lo: f32, hi: f32, smooth: f32) -> Self {
        let mid = 0.5 * (lo + hi);
        Self {
            lo,
            hi,
            current: mid,
            target: mid,
            smooth,
        }
    }
    fn retarget(&mut self, rng: &mut ChaCha20Rng) {
        self.target = rng.gen_range(self.lo..=self.hi);
    }
    fn step(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.smooth;
        self.current
    }
}

/// The values handed to the spectral transform for one frame.
#[derive(Clone, Copy, Debug)]
pub struct ModValues {
    /// Excitation pitch ratio (>1 raises pitch).
    pub pitch_ratio: f32,
    /// Envelope formant ratio (>1 moves formants up).
    pub formant_ratio: f32,
}

/// Non-stationary parameter generator.
pub struct Modulator {
    rng: ChaCha20Rng,
    pitch: Param,
    formant: Param,
    frames_per_target: u32,
    frame_in_seg: u32,
    seed: [u8; 32],
}

impl Modulator {
    /// Build from an explicit 32-byte seed (deterministic; used by tests and by
    /// session-key-derived seeding).
    pub fn from_seed(
        seed: [u8; 32],
        pitch_bounds: (f32, f32),
        formant_bounds: (f32, f32),
        frames_per_target: u32,
        smooth: f32,
    ) -> Self {
        Self {
            rng: ChaCha20Rng::from_seed(seed),
            pitch: Param::new(pitch_bounds.0, pitch_bounds.1, smooth),
            formant: Param::new(formant_bounds.0, formant_bounds.1, smooth),
            frames_per_target: frames_per_target.max(1),
            frame_in_seg: 0,
            seed,
        }
    }

    /// The 32 fixed per-bin phase offsets consumer needs are derived from the
    /// same stream; expose a helper that fills `out` with values in [0, 2π).
    pub fn fill_phase_offsets(&mut self, out: &mut [f32]) {
        for v in out.iter_mut() {
            *v = self.rng.gen_range(0.0..std::f32::consts::TAU);
        }
    }

    /// Roll onto a fresh seed, drawn from the current stream.
    ///
    /// # Why a ratchet rather than fresh OS entropy
    ///
    /// The new seed is 32 bytes of ChaCha20 output from the stream being
    /// replaced. That buys the property that matters, **forward secrecy**.
    /// ChaCha20 is not invertible, so an adversary who somehow learned the
    /// current seed could generate everything from this moment on but could not
    /// walk backwards to recover any earlier one. Each roll permanently closes
    /// off the segment before it.
    ///
    /// Reading fresh entropy from the OS instead would mean a syscall inside an
    /// audio callback every couple of seconds, which is exactly the kind of
    /// thing that causes a dropout, and it would make
    /// [`crate::Deidentifier::from_seed`] non-deterministic, and losing the
    /// reproducibility the test suite depends on. The chain is seeded from the
    /// OS CSPRNG once at construction; the ratchet carries that unpredictability
    /// forward without ever going back to the kernel.
    ///
    /// The smoothed parameter values are deliberately **not** reset. Only the
    /// source of future targets changes, so the glide continues through a roll
    /// and there is no discontinuity to hear.
    pub fn reseed(&mut self) {
        let mut next = [0u8; 32];
        self.rng.fill_bytes(&mut next);
        self.rng = ChaCha20Rng::from_seed(next);
        // Replace the retained copy, wiping the old one first.
        self.seed.zeroize();
        self.seed.copy_from_slice(&next);
        next.zeroize();
    }

    /// Draw a whole number of frames uniformly from `lo..=hi`.
    ///
    /// Used for the randomised roll interval: the gap before the next ratchet
    /// is itself drawn from the stream, so it is as unpredictable as everything
    /// else here and costs no syscall and no allocation -- which it cannot,
    /// because it is drawn inside an audio callback.
    ///
    /// `hi` below `lo` is treated as `lo`. That is not input validation --
    /// [`crate::DeidConfig::checked`] refuses a reversed range long before this
    /// is reached -- it is this function being total so that a caller cannot
    /// produce a panic in an audio thread by arithmetic.
    pub fn draw_frames(&mut self, lo: u32, hi: u32) -> u32 {
        let lo = lo.max(1);
        let hi = hi.max(lo);
        if hi == lo {
            return lo;
        }
        self.rng.gen_range(lo..=hi)
    }

    /// Advance one STFT frame and return the parameters to apply.
    pub fn next_frame(&mut self) -> ModValues {
        if self.frame_in_seg == 0 {
            self.pitch.retarget(&mut self.rng);
            self.formant.retarget(&mut self.rng);
        }
        self.frame_in_seg = (self.frame_in_seg + 1) % self.frames_per_target;
        ModValues {
            pitch_ratio: self.pitch.step(),
            formant_ratio: self.formant.step(),
        }
    }
}

impl Drop for Modulator {
    fn drop(&mut self) {
        // Best-effort wipe of the retained seed (the ChaCha state itself is not
        // exposed; the seed is the sensitive input).
        self.seed.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(seed: u8) -> Modulator {
        Modulator::from_seed([seed; 32], (0.8, 1.25), (0.85, 1.3), 8, 0.06)
    }

    #[test]
    fn values_stay_in_bounds() {
        let mut m = mk(1);
        for _ in 0..10_000 {
            let v = m.next_frame();
            assert!((0.79..=1.26).contains(&v.pitch_ratio), "{}", v.pitch_ratio);
            assert!(
                (0.84..=1.31).contains(&v.formant_ratio),
                "{}",
                v.formant_ratio
            );
        }
    }

    #[test]
    fn same_seed_is_deterministic() {
        let mut a = mk(7);
        let mut b = mk(7);
        for _ in 0..500 {
            let (x, y) = (a.next_frame(), b.next_frame());
            assert_eq!(x.pitch_ratio.to_bits(), y.pitch_ratio.to_bits());
            assert_eq!(x.formant_ratio.to_bits(), y.formant_ratio.to_bits());
        }
    }

    #[test]
    fn different_seed_diverges() {
        let mut a = mk(1);
        let mut b = mk(2);
        let mut differ = false;
        for _ in 0..500 {
            let (x, y) = (a.next_frame(), b.next_frame());
            if (x.pitch_ratio - y.pitch_ratio).abs() > 1e-6 {
                differ = true;
                break;
            }
        }
        assert!(differ, "distinct seeds should produce distinct streams");
    }

    #[test]
    fn reseeding_changes_the_stream() {
        let mut rolled = mk(11);
        let mut steady = mk(11);
        for _ in 0..50 {
            rolled.next_frame();
            steady.next_frame();
        }
        rolled.reseed();
        let mut diverged = false;
        for _ in 0..500 {
            if (rolled.next_frame().formant_ratio - steady.next_frame().formant_ratio).abs() > 1e-6
            {
                diverged = true;
                break;
            }
        }
        assert!(diverged, "a roll should change what comes next");
    }

    /// The ratchet must stay deterministic, or `from_seed` stops being
    /// reproducible and the reproducible-build story goes with it.
    #[test]
    fn the_ratchet_is_deterministic() {
        let mut a = mk(5);
        let mut b = mk(5);
        for round in 0..4 {
            for _ in 0..30 {
                let (x, y) = (a.next_frame(), b.next_frame());
                assert_eq!(
                    x.formant_ratio.to_bits(),
                    y.formant_ratio.to_bits(),
                    "round {round}"
                );
            }
            a.reseed();
            b.reseed();
        }
    }

    /// A roll must not jolt the smoothed values: the glide is what keeps the
    /// transform inaudible at the seam.
    #[test]
    fn reseeding_does_not_jump_the_parameters() {
        let mut m = mk(9);
        for _ in 0..200 {
            m.next_frame();
        }
        let before = m.next_frame();
        m.reseed();
        let after = m.next_frame();
        assert!(
            (after.formant_ratio - before.formant_ratio).abs() < 0.02,
            "parameters jumped across a roll: {} -> {}",
            before.formant_ratio,
            after.formant_ratio
        );
    }

    #[test]
    fn a_roll_replaces_the_retained_seed() {
        let mut m = mk(3);
        let original = m.seed;
        m.reseed();
        assert_ne!(m.seed, original, "the old seed must not be kept around");
    }

    #[test]
    fn parameters_actually_move() {
        let mut m = mk(3);
        let first = m.next_frame().pitch_ratio;
        let mut moved = false;
        for _ in 0..200 {
            if (m.next_frame().pitch_ratio - first).abs() > 1e-3 {
                moved = true;
                break;
            }
        }
        assert!(moved, "pitch ratio should vary over time");
    }
}
