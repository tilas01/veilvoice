// SPDX-License-Identifier: GPL-3.0-or-later
//! Cryptographically-seeded modulation of the effect parameters.
//!
//! The pitch and formant ratios are never constant: a ChaCha20 CSPRNG picks a
//! new random target every `frames_per_target` STFT frames, and a one-pole
//! filter glides continuously toward it. Because the transform is therefore
//! non-stationary and unpredictable, an attacker cannot "undo" it by assuming a
//! single fixed shift — there is no single shift to undo, and the target
//! sequence is unknowable without the seed (which never leaves the process and
//! is zeroized on drop).

use rand::Rng;
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

    /// Build from the operating-system CSPRNG (fresh, unpredictable per run).
    pub fn from_os_rng(
        pitch_bounds: (f32, f32),
        formant_bounds: (f32, f32),
        frames_per_target: u32,
        smooth: f32,
    ) -> Self {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).expect("OS CSPRNG unavailable");
        let m = Self::from_seed(
            seed,
            pitch_bounds,
            formant_bounds,
            frames_per_target,
            smooth,
        );
        // The seed lives inside the Modulator (zeroized on drop); wipe our copy.
        seed.zeroize();
        m
    }

    /// The 32 fixed per-bin phase offsets consumer needs are derived from the
    /// same stream; expose a helper that fills `out` with values in [0, 2π).
    pub fn fill_phase_offsets(&mut self, out: &mut [f32]) {
        for v in out.iter_mut() {
            *v = self.rng.gen_range(0.0..std::f32::consts::TAU);
        }
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
