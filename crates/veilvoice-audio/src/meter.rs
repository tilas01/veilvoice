// SPDX-License-Identifier: GPL-3.0-or-later
//! The scale a level meter is drawn on.
//!
//! # Why this is here rather than in a front end
//!
//! [`crate::live::LiveStats`] reports a peak, and both front ends draw it. They
//! drew it *differently*: both were linear, and the desktop one printed a
//! decibel number beside a bar filled linearly, so the number said -12 dB and
//! the bar showed a quarter. Two meters disagreeing about the same reading is
//! worse than one bad meter, because it makes the reader doubt the number.
//!
//! The scale belongs with the measurement. This module owns the arithmetic;
//! the front ends own how it looks.
//!
//! # Why not linear
//!
//! Loudness is not linear, and a meter that is has almost no useful range.
//! Ordinary speech recorded at a sensible level peaks around **-12 dBFS**,
//! which is 0.25 linear: a quarter of the bar, which reads as near-silence.
//! The only way to fill a linear bar is to be clipping. Every real meter is
//! logarithmic for exactly this reason.
//!
//! # What it measures, and what it does not
//!
//! **Sample peak**, since the last read. It is not a loudness meter: RMS, LUFS
//! and anything else that correlates with how loud a thing *sounds* needs a
//! window and a weighting curve, and answers a different question. This one
//! answers "am I being recorded, and am I clipping".
//!
//! It also cannot see an **inter-sample peak**, which is a waveform that passes above
//! full scale between two samples and clips in a converter without any single
//! sample exceeding 1.0. Catching those needs oversampling. A front end may say
//! `CLIP` when a sample reaches full scale, and must not imply it caught the
//! ones it cannot see.
//!
//! # In plain words
//!
//! This decides how a level meter is drawn, so that the bar and the number beside
//! it always agree.
//!
//! They did not, once. Both the window and the terminal drew the bar filling
//! evenly with the signal, while the number beside it was in decibels, which do
//! not rise evenly at all. So the number could read -12 dB while the bar looked a
//! quarter full, and a reader who noticed stopped trusting both.
//!
//! One piece of arithmetic, in one place, used by both. Now they cannot disagree.

/// The quietest level worth drawing. Below this is silence.
///
/// Sixty decibels is the range a person can usefully read off a short bar. A
/// meter that went to -90 would spend a third of itself on room tone.
pub const FLOOR_DB: f32 = -60.0;

/// At or above this, the level is called clipping.
///
/// -0.1 dBFS rather than exactly 0. A sample at full scale in a 16-bit file has
/// no larger neighbour to reach, so waiting for a mathematically perfect 1.0
/// means never saying so about a signal that is plainly clipped.
///
/// Decibels near the top of the scale are much finer than they look in linear
/// terms, and this is the number that shows it: a *linear* 0.99 is already
/// -0.087 dBFS, which is inside this threshold.
pub const CLIP_DB: f32 = -0.1;

/// Level in decibels relative to full scale.
///
/// Silence is [`FLOOR_DB`] rather than negative infinity: a caller wants a
/// number to place on a bar, and a meter is not the place to introduce an
/// infinity into arithmetic that has to keep running.
///
/// # The two ways a reading can be nonsense, answered differently
///
/// **NaN** is not a level at all, and is read as silence, because nothing can be
/// inferred from it.
///
/// **Positive infinity** is read as **full scale**. Both are wrong readings,
/// and a meter should be wrong in the direction that gets looked at: pinned at
/// the top it is noticed in a second, and pinned at the bottom it looks exactly
/// like a microphone that is not plugged in.
pub fn dbfs(peak: f32) -> f32 {
    if peak.is_nan() || peak <= 0.0 {
        return FLOOR_DB;
    }
    (20.0 * peak.clamp(0.0, 1.0).log10()).max(FLOOR_DB)
}

/// Where a level sits along a bar: 0.0 at the floor, 1.0 at full scale.
pub fn position(peak: f32) -> f32 {
    ((dbfs(peak) - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// Whether a reading counts as clipping.
pub fn clipping(peak: f32) -> bool {
    dbfs(peak) >= CLIP_DB
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_scale_is_zero_and_silence_is_the_floor() {
        assert!((dbfs(1.0) - 0.0).abs() < 0.001);
        assert_eq!(dbfs(0.0), FLOOR_DB);
        assert_eq!(dbfs(-1.0), FLOOR_DB, "a negative peak is not a level");
        assert_eq!(dbfs(f32::NAN), FLOOR_DB, "nothing can be read from NaN");
        assert_eq!(dbfs(f32::NEG_INFINITY), FLOOR_DB);
        // Wrong in the direction that gets looked at. See the note on `dbfs`.
        assert_eq!(dbfs(f32::INFINITY), 0.0, "a broken meter should pin high");
    }

    /// Halving the amplitude is six decibels. This is the check that the scale
    /// is a decibel scale rather than something that merely curves.
    #[test]
    fn halving_the_amplitude_is_six_decibels() {
        assert!((dbfs(1.0) - dbfs(0.5) - 6.0206).abs() < 0.01);
        assert!((dbfs(0.5) - dbfs(0.25) - 6.0206).abs() < 0.01);
    }

    /// The defect this module exists to fix, stated as a test. Speech at a
    /// sensible recording level peaks near -12 dBFS; on a linear meter that
    /// filled a quarter of the bar and read as near-silence.
    #[test]
    fn ordinary_speech_lands_in_the_middle_of_the_bar() {
        let speech = 0.251; // -12 dBFS
        let where_it_sits = position(speech);
        assert!(
            (0.7..0.85).contains(&where_it_sits),
            "-12 dBFS sits at {where_it_sits:.3} of the bar"
        );
        assert!(speech < 0.3, "the linear meter filled under a third");
    }

    #[test]
    fn the_position_is_bounded_whatever_it_is_given() {
        for peak in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -5.0,
            1e30,
            0.0,
            1.0,
        ] {
            let where_it_sits = position(peak);
            assert!(
                (0.0..=1.0).contains(&where_it_sits),
                "{peak} gave {where_it_sits}"
            );
        }
        assert_eq!(position(0.0), 0.0);
        assert_eq!(position(1.0), 1.0);
    }

    /// The numbers here were checked rather than assumed: a linear 0.99 is
    /// -0.087 dBFS, already inside a tenth of a decibel of full scale.
    #[test]
    fn the_clip_threshold_is_just_below_full_scale() {
        assert!(!clipping(0.9), "0.9 is -0.9 dBFS");
        assert!(!clipping(0.98), "0.98 is -0.18 dBFS");
        assert!(clipping(0.99), "0.99 is -0.087 dBFS, which counts");
        assert!(clipping(1.0));
        assert!(!clipping(0.0));
        assert!(!clipping(f32::NAN));
    }
}
