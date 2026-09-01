// SPDX-License-Identifier: GPL-3.0-or-later
//! Destination voices: several canonical registers instead of one.
//!
//! # What this is for
//!
//! By default every speaker VeilVoice processes comes out as **the same
//! voice**, with one pitch register, one vocal-tract scale and one long-term
//! spectrum.
//! That is the many-to-one mapping the whole project rests on, and for a single
//! speaker it is exactly right.
//!
//! It is wrong for a conversation. Two people veiled into one indistinguishable
//! voice produce a recording nobody can follow: the words survive and the
//! turn-taking does not, so a listener cannot tell a question from its answer.
//! This module hands out a small table of **distinct destination voices**, so a
//! recording with three people in it comes out with three voices in it.
//!
//! # The security property, and how it survives
//!
//! The property that matters is that **the output voice is a function of the
//! slot, not of the speaker**. Every input mapped onto slot 3 comes out as
//! voice 3, whoever they were. The mapping is still many-to-one, with
//! infinitely many inputs per output, so there is still no inverse to
//! compute. What changes is the number of buckets, from one to at most ten.
//!
//! This is why a voice is **never derived from the speaker**. Choosing a
//! destination by measuring the input is the obvious implementation, and the
//! one that would sound most natural. It would make the output voice a function
//! of the input voice, which is precisely the linkage the project exists to destroy.
//! Slots are assigned by turn order, and turn order is something the *user*
//! supplies.
//!
//! # What a conversation leaks that a monologue does not
//!
//! Stated plainly, because it is a real cost and nobody should have to work it
//! out for themselves:
//!
//! * **How many people were talking.** Ten voices in the output means ten
//!   speakers in the input.
//! * **Who spoke when, and for how long.** Turn-taking structure is preserved
//!   on purpose, because it is the thing that makes the result usable, and turn
//!   structure is information about a conversation. Overlaps, interruptions,
//!   the length of each answer and the rhythm of the exchange all survive.
//! * **Nothing about who they were.** The voiceprint of each speaker is
//!   destroyed exactly as thoroughly as in single-speaker mode: the same phase
//!   discard, the same many-to-one normalisation onto the slot's canonical
//!   values.
//!
//! # A register has to land on a bin, and that is the whole constraint
//!
//! The first version of this table picked five registers about 26 Hz apart on
//! the reasoning that the just-noticeable difference for the fundamental of
//! speech is around 8 Hz, so 26 Hz would be three times that. The reasoning was
//! sound and the table was wrong, because it measured the wrong thing.
//!
//! When accent neutralisation is on, [`crate::spectral`] does not resample the
//! excitation. It **replaces** it with a harmonic comb at the canonical
//! fundamental, quantised to the nearest whole FFT bin so that every comb line
//! sits on a bin centre and the frames overlap-add coherently. The rendered
//! fundamental is therefore not the number in the table. It is
//!
//! ```text
//! round(target_f0 / bin_hz) * bin_hz,   bin_hz = sample_rate / frame_size
//! ```
//!
//! At the default 1024-point frame and 48 kHz that spacing is **46.875 Hz**, so
//! the five registers 105, 131, 157, 183 and 209 Hz render as 93.75, 140.625,
//! 140.625, 187.5 and 187.5, which is three distinct pitches, not five. Two pairs of
//! speakers would have shared a register with nothing in the interface saying
//! so. It was found by measuring the fundamental of an actual rendered file,
//! not by reading the code, and the tests below now measure the same thing the
//! ear would.
//!
//! So the registers are **bin-exact by construction**: each is a whole number
//! of bins at the default configuration. Inside the range where a resynthesised
//! voice stays intelligible, roughly 90 to 240 Hz, below which the comb has too
//! few harmonics under the vowels and above which it stops being a speaking
//! register, there are exactly four:
//!
//! | bin | rendered |
//! |---:|---:|
//! | 2 | 93.75 Hz |
//! | 3 | 140.625 Hz |
//! | 4 | 187.5 Hz |
//! | 5 | 234.375 Hz |
//!
//! # Ten voices, from four registers and three vocal tracts
//!
//! The second axis is the canonical vocal-tract scale, which is a continuous
//! warp and is **not** quantised, so it is free to take values the ear can
//! separate: 620, 760 and 900 Hz, each about 22 % from its neighbour, all
//! inside the range where the vowels stay natural.
//!
//! Four registers times three tracts is twelve, and [`MAX_VOICES`] ships ten of
//! them. Twenty, which was asked for, is not available: it would need either
//! registers a single bin apart at a frame size four times longer, which
//! quadruples the latency, or vocal tracts close enough to be heard as the
//! same person on a different day.
//!
//! # If you change the frame size, check the table again
//!
//! The registers are exact at the *default* configuration. A caller who changes
//! [`crate::DeidConfig::frame_size`] or the sample rate moves the bin grid
//! underneath them, and two registers can collide again.
//! [`Voice::rendered_f0_hz`] reports what a given configuration will actually
//! produce, and [`distinct_voices`] counts how many of the ten survive it, so
//! a front end can say "this frame size gives you six distinguishable voices"
//! rather than handing out ten labels for six sounds.
//!
//! # In plain words
//!
//! The set of voices a recording can be turned into.
//!
//! By default everyone comes out as the same one. That is on purpose: if every
//! speaker sounds identical, there is nothing in the result that distinguishes one
//! from another, and nothing to trace back.
//!
//! When a recording has several people in it that becomes a problem, because a
//! listener cannot follow who is who. So there is a small set of destination
//! voices to hand out instead, chosen to be as far apart as the arithmetic allows,
//! and a measured limit on how many of them can genuinely be told apart by ear.

use crate::{AccentConfig, DeidConfig};

/// How many distinct destination voices this engine hands out.
///
/// Ten of the twelve the table can express. See the module documentation for
/// why it is not twenty.
pub const MAX_VOICES: usize = 10;

/// One destination voice: the canonical values every speaker in this slot is
/// mapped onto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voice {
    /// Pitch register, in hertz.
    ///
    /// What is *asked for*. What is rendered is this quantised to the FFT bin
    /// grid. See [`Voice::rendered_f0_hz`], and the module documentation for
    /// why that distinction cost a wrong table once already.
    pub target_f0_hz: f32,
    /// Canonical long-term envelope centroid, in hertz, which is the vocal-tract scale.
    ///
    /// A continuous warp, so this one is rendered as asked.
    pub target_centroid_hz: f32,
    /// Slope of the canonical long-term spectrum, in dB per octave.
    pub target_tilt_db_oct: f32,
}

impl Voice {
    /// Apply this voice to an [`AccentConfig`].
    ///
    /// Only the three canonical targets are replaced. The *strengths*, meaning
    /// how hard the neutraliser pushes toward them, are left alone, because
    /// they are the user's setting and a slot is a destination, not a policy
    /// about how firmly to arrive at it.
    pub fn applied_to(&self, mut accent: AccentConfig) -> AccentConfig {
        accent.target_f0_hz = self.target_f0_hz;
        accent.target_centroid_hz = self.target_centroid_hz;
        accent.target_tilt_db_oct = self.target_tilt_db_oct;
        accent
    }

    /// The fundamental this voice will **actually** be rendered at, under
    /// `config`.
    ///
    /// The voiced excitation is a harmonic comb snapped to the FFT bin grid, so
    /// the rendered fundamental is the requested one rounded to the nearest
    /// whole bin. This is the number to compare two voices by, and the number
    /// to show anybody who asks what a slot sounds like.
    pub fn rendered_f0_hz(&self, config: &DeidConfig) -> f32 {
        let bin_hz = bin_hz(config);
        if bin_hz <= 0.0 || !self.target_f0_hz.is_finite() {
            return 0.0;
        }
        (self.target_f0_hz / bin_hz).round().max(1.0) * bin_hz
    }

    /// Whether this voice is inside the range the engine can render usefully.
    ///
    /// Checked rather than clamped: a caller who built a voice out of range
    /// meant something, and silently moving it would give them a different
    /// speaker from the one they asked for without saying so.
    pub fn checked(self) -> Result<Self, String> {
        for (name, value) in [
            ("target_f0_hz", self.target_f0_hz),
            ("target_centroid_hz", self.target_centroid_hz),
            ("target_tilt_db_oct", self.target_tilt_db_oct),
        ] {
            if !value.is_finite() {
                return Err(format!("{name} must be a real number"));
            }
        }
        if !(F0_MIN_HZ..=F0_MAX_HZ).contains(&self.target_f0_hz) {
            return Err(format!(
                "target_f0_hz {} is outside {F0_MIN_HZ}-{F0_MAX_HZ} Hz, where a \
                 resynthesised voice stays intelligible",
                self.target_f0_hz
            ));
        }
        if !(CENTROID_MIN_HZ..=CENTROID_MAX_HZ).contains(&self.target_centroid_hz) {
            return Err(format!(
                "target_centroid_hz {} is outside {CENTROID_MIN_HZ}-{CENTROID_MAX_HZ} Hz, \
                 where the vowels stay natural",
                self.target_centroid_hz
            ));
        }
        if !(TILT_MIN_DB_OCT..=TILT_MAX_DB_OCT).contains(&self.target_tilt_db_oct) {
            return Err(format!(
                "target_tilt_db_oct {} is outside {TILT_MIN_DB_OCT} to \
                 {TILT_MAX_DB_OCT} dB per octave",
                self.target_tilt_db_oct
            ));
        }
        Ok(self)
    }

    /// A short label for an interface: "low register, narrow tract".
    ///
    /// Describes the *destination*, never the speaker. There is deliberately no
    /// vocabulary here for who somebody was: "man", "woman", "child", "older"
    /// are all statements about an input this crate has just finished
    /// destroying, and a label that reintroduced one would undo the point.
    ///
    /// The hertz figure quoted is the one that will be **rendered** at the
    /// default configuration, not the one requested, so the label and the ear
    /// agree.
    pub fn describe(&self) -> String {
        let rendered = self.rendered_f0_hz(&DeidConfig::default());
        let register = if rendered < 115.0 {
            "low"
        } else if rendered < 165.0 {
            "low-mid"
        } else if rendered < 210.0 {
            "mid-high"
        } else {
            "high"
        };
        let tract = if self.target_centroid_hz < 690.0 {
            "narrow"
        } else if self.target_centroid_hz < 830.0 {
            "medium"
        } else {
            "wide"
        };
        format!(
            "{register} register, {tract} tract ({rendered:.0} Hz, {:.0} Hz)",
            self.target_centroid_hz
        )
    }
}

/// The lowest fundamental a resynthesised voice stays intelligible at.
pub const F0_MIN_HZ: f32 = 90.0;
/// The highest fundamental that still reads as a speaking register.
pub const F0_MAX_HZ: f32 = 240.0;
/// The narrowest canonical vocal tract offered.
pub const CENTROID_MIN_HZ: f32 = 550.0;
/// The widest canonical vocal tract offered.
pub const CENTROID_MAX_HZ: f32 = 1000.0;
/// The steepest permitted long-term slope, in dB per octave.
pub const TILT_MIN_DB_OCT: f32 = -12.0;
/// The flattest permitted long-term slope, in dB per octave.
pub const TILT_MAX_DB_OCT: f32 = 0.0;

/// The FFT bin spacing of a configuration, in hertz.
///
/// The grid every canonical register is snapped to. Public because a front end
/// that lets somebody change the frame size needs to be able to explain what
/// changed about the voices.
pub fn bin_hz(config: &DeidConfig) -> f32 {
    if config.frame_size == 0 || !config.sample_rate.is_finite() {
        return 0.0;
    }
    config.sample_rate / config.frame_size as f32
}

/// The four registers, each a whole number of bins at the default
/// configuration: bins 2, 3, 4 and 5 of a 1024-point frame at 48 kHz.
///
/// Written out rather than computed from the default, so that changing the
/// default frame size makes a **test** fail rather than silently moving every
/// voice in every recording anybody has already made.
const REGISTERS_HZ: [f32; 4] = [93.75, 140.625, 187.5, 234.375];

/// The three vocal-tract scales, about 22 % apart. Not quantised, because the
/// warp is continuous, so these render as asked.
const TRACTS: [(f32, f32); 3] = [
    // (centroid Hz, tilt dB per octave)
    (620.0, -5.0),
    (760.0, -6.0),
    (900.0, -7.0),
];

/// The ten destination voices, in the order they are handed out.
///
/// The order is chosen, not incidental. Slot 0 and slot 1 are the two furthest
/// apart in both dimensions, because a **two-person conversation is the common
/// case** and the two people in it should be the easiest pair in the table to
/// tell apart. The table then works inward, so it degrades gracefully as more
/// speakers are added rather than saving its clearest contrasts for a tenth
/// speaker who is usually not there.
///
/// Two of the twelve combinations are unused, which is slack rather than a
/// stretch: nothing here is reaching for a tenth voice it cannot really make.
const TABLE: [(usize, usize); MAX_VOICES] = [
    // (register index, tract index)
    (0, 0), // 0: lowest register, narrowest tract
    (3, 2), // 1: highest register, widest tract -- furthest from slot 0
    (1, 2), // 2
    (2, 0), // 3
    (0, 2), // 4
    (3, 0), // 5
    (1, 0), // 6
    (2, 2), // 7
    (0, 1), // 8
    (3, 1), // 9
];

/// The destination voice for slot `index`.
///
/// Wraps rather than failing past [`MAX_VOICES`]: an eleventh speaker gets the
/// first voice again. That is a real collision, with two people sharing one
/// output voice, and it is why [`MAX_VOICES`] is stated and why a front end should
/// refuse rather than rely on this. Wrapping is here so the function is total,
/// not because reusing a voice is acceptable.
pub fn voice(index: usize) -> Voice {
    let (register, tract) = TABLE[index % MAX_VOICES];
    let (centroid, tilt) = TRACTS[tract];
    Voice {
        target_f0_hz: REGISTERS_HZ[register],
        target_centroid_hz: centroid,
        target_tilt_db_oct: tilt,
    }
}

/// Every destination voice, in the order they are handed out.
pub fn all() -> Vec<Voice> {
    (0..MAX_VOICES).map(voice).collect()
}

/// How far apart two voices are, as the **larger** of their two separations.
///
/// Both axes are expressed as a ratio, because hearing is ratio-based on both:
/// a 20 Hz pitch difference is enormous at 90 Hz and inaudible at 400, and the
/// same is true of a vocal-tract scale.
///
/// # Why the larger and not the smaller
///
/// The first version of this took the *smaller*, reasoning that two voices are
/// only as separable as their closest resemblance. Measuring it showed that to
/// be backwards. Slots 0 and 4 have exactly the same rendered pitch and vocal
/// tracts 45 % apart -- one sounds like a much larger person than the other,
/// and nobody would confuse them -- and the minimum called them **identical**,
/// because one axis matched. Taking the minimum reported that three voices were
/// already indistinguishable, which is plainly false if you listen.
///
/// A listener separates two voices by whichever cue is strongest. Two voices
/// are confusable only when they are close on *both* axes, which is what the
/// maximum expresses.
///
/// `1.0` means identical on both axes. `1.19` means the stronger axis differs
/// by 19 %, which is three semitones of pitch.
pub fn separation(a: &Voice, b: &Voice, config: &DeidConfig) -> f32 {
    fn ratio(x: f32, y: f32) -> f32 {
        if x <= 0.0 || y <= 0.0 || !x.is_finite() || !y.is_finite() {
            return 1.0;
        }
        if x > y {
            x / y
        } else {
            y / x
        }
    }
    let pitch = ratio(a.rendered_f0_hz(config), b.rendered_f0_hz(config));
    let tract = ratio(a.target_centroid_hz, b.target_centroid_hz);
    pitch.max(tract)
}

/// The separation below which two voices should not be handed to two people.
///
/// **Three semitones, a ratio of 1.19.**
///
/// A semitone is about 6 % and is audible when two sounds are played back to
/// back for comparison. That is not the task here. The task is following a
/// conversation: hearing one voice, then a different one thirty seconds later,
/// and knowing without being told that the speaker changed. That needs a
/// margin, not a threshold, and three semitones is the smallest interval that
/// is unmistakable rather than merely detectable.
///
/// Deliberately conservative, because being wrong in the other direction is
/// worse. A group set up with two voices the listener cannot separate produces
/// a recording in which two people sound like one, which is not a privacy
/// failure but is a failure of the thing the feature is *for*, and it is only
/// discovered after the recording exists.
pub const CLEAR_SEPARATION: f32 = 1.19;

/// How many voices can be handed out before two of them are too alike.
///
/// Slots are given out in table order, so this asks: taking them one at a time,
/// at what point does a new voice come within [`CLEAR_SEPARATION`] of one
/// already given out? Everything up to that point is safe to use.
///
/// This is a stricter question than [`distinct_voices`], which only asks
/// whether two voices are *different*. Different is not the same as tellable
/// apart, and a table of ten technically-different voices can still contain a
/// pair nobody can separate by ear.
pub fn clear_voices(config: &DeidConfig) -> usize {
    let voices = all();
    let mut given: Vec<Voice> = Vec::with_capacity(voices.len());
    for candidate in voices {
        if given
            .iter()
            .any(|taken| separation(taken, &candidate, config) < CLEAR_SEPARATION)
        {
            return given.len();
        }
        given.push(candidate);
    }
    given.len()
}

/// The closest pair among the first `count` voices, as a ratio.
///
/// For a front end that wants to say *how* clear a given group size is rather
/// than only whether it passed. `1.0` for fewer than two voices, since one
/// voice has nothing to be confused with.
pub fn closest_pair(count: usize, config: &DeidConfig) -> f32 {
    let voices = all();
    let taken = &voices[..count.min(voices.len())];
    let mut closest = f32::INFINITY;
    for (index, a) in taken.iter().enumerate() {
        for b in taken.iter().skip(index + 1) {
            closest = closest.min(separation(a, b, config));
        }
    }
    if closest.is_finite() {
        closest
    } else {
        1.0
    }
}

/// How many of the ten are still distinguishable under `config`.
///
/// Two voices count as the same when they would be **rendered** with the same
/// fundamental and the same vocal tract. At the default configuration the
/// answer is [`MAX_VOICES`]; at a shorter frame size it is fewer, because the
/// bin grid coarsens and registers collapse onto each other.
///
/// A front end that lets somebody change the frame size should call this and
/// say what it returns. Handing out ten labels for six sounds is the failure
/// this function exists to make visible.
pub fn distinct_voices(config: &DeidConfig) -> usize {
    let mut seen: Vec<(i64, i64)> = Vec::with_capacity(MAX_VOICES);
    for voice in all() {
        // Rounded to a hundredth of a hertz before comparing: these are
        // computed floats, and two that differ in the last bit are the same
        // sound.
        let key = (
            (voice.rendered_f0_hz(config) * 100.0).round() as i64,
            (voice.target_centroid_hz * 100.0).round() as i64,
        );
        if !seen.contains(&key) {
            seen.push(key);
        }
    }
    seen.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> DeidConfig {
        DeidConfig::default()
    }

    /// **Eight**, at the default configuration. Measured, not chosen.
    ///
    /// The table holds ten and all ten are *different*; eight is how many are
    /// far enough apart that a listener following a conversation can tell which
    /// is which. Adding the ninth brings the closest pair to 1.1842 -- slots 4
    /// and 8, which have exactly the same rendered pitch and vocal tracts only
    /// 18 % apart -- and that is under the three-semitone floor.
    ///
    /// This number is the one a front end should cap a group at. If it moves,
    /// something about the table or the frame size moved with it, and the front
    /// end's limit has to move too.
    #[test]
    fn eight_voices_are_clearly_separable_and_the_ninth_is_not() {
        let config = default_config();
        assert_eq!(clear_voices(&config), 8, "the measured clear limit");
        assert!(
            closest_pair(8, &config) >= CLEAR_SEPARATION,
            "eight: closest pair {:.4}",
            closest_pair(8, &config)
        );
        assert!(
            closest_pair(9, &config) < CLEAR_SEPARATION,
            "nine: closest pair {:.4} should be under the floor",
            closest_pair(9, &config)
        );
        // The exact figures, so a change to the table is visible in the diff of
        // this test rather than only in a number nobody looks at.
        assert!((closest_pair(8, &config) - 1.25).abs() < 0.001);
        assert!((closest_pair(9, &config) - 1.1842).abs() < 0.001);
    }

    /// Being *different* and being *tellable apart* are different questions,
    /// and this is the gap between them: ten against eight.
    #[test]
    fn distinct_is_a_weaker_test_than_clear() {
        let config = default_config();
        assert_eq!(distinct_voices(&config), MAX_VOICES);
        assert!(clear_voices(&config) < distinct_voices(&config));
    }

    /// The separation of a voice with itself is 1.0, and the measure is
    /// symmetric. Both are obvious and both would be silently wrong if the
    /// ratio helper picked up a sign.
    #[test]
    fn separation_is_symmetric_and_one_against_itself() {
        let config = default_config();
        for (index, a) in all().iter().enumerate() {
            assert!(
                (separation(a, a, &config) - 1.0).abs() < 1e-6,
                "slot {index}"
            );
            for b in all().iter() {
                assert!((separation(a, b, &config) - separation(b, a, &config)).abs() < 1e-6);
            }
            assert!(separation(a, a, &config) >= 1.0);
        }
    }

    /// The first version of this took the smaller of the two axes, which
    /// reported three voices as already indistinguishable. Slots 0 and 4 are
    /// why that was wrong: identical pitch, vocal tracts 45 % apart -- one
    /// sounds like a much larger person, and nobody would confuse them.
    #[test]
    fn two_voices_differing_on_one_axis_only_are_still_separable() {
        let config = default_config();
        let voices = all();
        let (a, b) = (&voices[0], &voices[4]);
        assert!(
            (a.rendered_f0_hz(&config) - b.rendered_f0_hz(&config)).abs() < 0.01,
            "slots 0 and 4 should share a pitch"
        );
        assert!(
            separation(a, b, &config) > CLEAR_SEPARATION,
            "same pitch, 45 % apart in tract, and separable on that alone"
        );
    }

    /// One voice has nothing to be confused with, and no voices is not an
    /// error. Both are reachable from a front end with an empty group.
    #[test]
    fn a_group_too_small_to_confuse_reports_no_confusion() {
        let config = default_config();
        assert_eq!(closest_pair(0, &config), 1.0);
        assert_eq!(closest_pair(1, &config), 1.0);
        // And asking for more than the table holds does not index past it.
        assert_eq!(
            closest_pair(MAX_VOICES + 5, &config),
            closest_pair(MAX_VOICES, &config)
        );
    }

    /// A coarser frame grid collapses registers onto each other, and the clear
    /// count has to fall with it rather than keep promising eight.
    #[test]
    fn a_coarser_frame_grid_reduces_the_clear_count() {
        let coarse = DeidConfig {
            frame_size: 128,
            ..default_config()
        };
        let fine = default_config();
        assert!(
            clear_voices(&coarse) <= clear_voices(&fine),
            "coarse {} should not beat fine {}",
            clear_voices(&coarse),
            clear_voices(&fine)
        );
    }

    #[test]
    fn there_are_exactly_ten_and_they_are_all_different_as_written() {
        let voices = all();
        assert_eq!(voices.len(), MAX_VOICES);
        for (i, a) in voices.iter().enumerate() {
            for (j, b) in voices.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "slots {i} and {j} are the same voice");
                }
            }
        }
    }

    /// **The test the first version of this table did not have.**
    ///
    /// Being different as written is not enough: the voiced excitation is a
    /// comb snapped to the FFT bin grid, so two registers a few hertz apart can
    /// render as the same pitch. The first table had five registers that
    /// rendered as three, and two pairs of speakers would have shared a voice
    /// with nothing saying so. This compares what comes *out*.
    #[test]
    fn all_ten_are_still_distinct_after_the_bin_grid_has_had_them() {
        let config = default_config();
        assert_eq!(
            distinct_voices(&config),
            MAX_VOICES,
            "the table collapses to {} distinguishable voices at the default \
             configuration",
            distinct_voices(&config)
        );

        let voices = all();
        for (i, a) in voices.iter().enumerate() {
            for (j, b) in voices.iter().enumerate().skip(i + 1) {
                let same_pitch =
                    (a.rendered_f0_hz(&config) - b.rendered_f0_hz(&config)).abs() < 0.01;
                let same_tract = (a.target_centroid_hz - b.target_centroid_hz).abs() < 0.01;
                assert!(
                    !(same_pitch && same_tract),
                    "slots {i} and {j} both render at {:.3} Hz with a {:.0} Hz tract",
                    a.rendered_f0_hz(&config),
                    a.target_centroid_hz
                );
            }
        }
    }

    /// Each register must be a whole number of bins at the default
    /// configuration, so what is asked for is what is rendered. If somebody
    /// changes the default frame size, this is what tells them the voice table
    /// needs choosing again.
    #[test]
    fn every_register_is_bin_exact_at_the_default_configuration() {
        let config = default_config();
        let spacing = bin_hz(&config);
        assert!(
            (spacing - 46.875).abs() < 1e-4,
            "the bin spacing is {spacing} Hz, and the register table was chosen for \
             46.875 Hz. Choose it again."
        );
        for register in REGISTERS_HZ {
            let bins = register / spacing;
            assert!(
                (bins - bins.round()).abs() < 1e-4,
                "{register} Hz is {bins} bins, and a register must be a whole number"
            );
            let voice = Voice {
                target_f0_hz: register,
                ..voice(0)
            };
            assert!(
                (voice.rendered_f0_hz(&config) - register).abs() < 1e-3,
                "{register} Hz renders as {} Hz",
                voice.rendered_f0_hz(&config)
            );
        }
    }

    /// The measurement that found the bug, kept as a test. These are the
    /// registers the first table shipped, and what they actually rendered as.
    #[test]
    fn the_registers_that_were_wrong_are_still_wrong_for_the_same_reason() {
        let config = default_config();
        let rendered = |hz: f32| {
            Voice {
                target_f0_hz: hz,
                ..voice(0)
            }
            .rendered_f0_hz(&config)
        };
        assert!((rendered(131.0) - rendered(157.0)).abs() < 0.01);
        assert!((rendered(183.0) - rendered(209.0)).abs() < 0.01);
        assert!((rendered(105.0) - 93.75).abs() < 0.01);
    }

    /// A coarser grid must be reported as fewer voices rather than silently
    /// handing out ten labels for a smaller number of sounds.
    #[test]
    fn a_shorter_frame_reports_fewer_distinguishable_voices() {
        let coarse = DeidConfig {
            frame_size: 256,
            ..DeidConfig::default()
        };
        let fewer = distinct_voices(&coarse);
        assert!(
            fewer < MAX_VOICES,
            "a 256-point frame is 187.5 Hz per bin and must collapse the table, got \
             {fewer}"
        );
        assert!(fewer >= 1);
    }

    /// The vocal tract is a continuous warp, not a quantised one, so its three
    /// values must survive any frame size.
    #[test]
    fn the_vocal_tracts_are_far_apart_and_are_not_quantised() {
        for pair in TRACTS.windows(2) {
            let ratio = pair[1].0 / pair[0].0;
            assert!(
                ratio > 1.18,
                "{} Hz and {} Hz are only {:.0}% apart",
                pair[0].0,
                pair[1].0,
                (ratio - 1.0) * 100.0
            );
        }
    }

    /// A two-person conversation is the common case, so slots 0 and 1 must be
    /// the furthest apart in the table.
    #[test]
    fn the_first_two_slots_are_the_easiest_pair_to_tell_apart() {
        let config = default_config();
        let voices = all();
        let separation = |a: &Voice, b: &Voice| {
            (a.rendered_f0_hz(&config) - b.rendered_f0_hz(&config)).abs() / 46.875
                + (a.target_centroid_hz - b.target_centroid_hz).abs() / 140.0
        };
        let first_pair = separation(&voices[0], &voices[1]);
        for (i, a) in voices.iter().enumerate() {
            for b in voices.iter().skip(i + 1) {
                assert!(
                    separation(a, b) <= first_pair + 1e-3,
                    "a further-apart pair than slots 0 and 1 exists"
                );
            }
        }
    }

    /// Every voice in the table must be one the engine will accept.
    #[test]
    fn every_shipped_voice_is_within_range() {
        for (index, voice) in all().into_iter().enumerate() {
            voice
                .checked()
                .unwrap_or_else(|error| panic!("slot {index}: {error}"));
        }
    }

    #[test]
    fn a_voice_out_of_range_is_refused_rather_than_moved() {
        let bad = Voice {
            target_f0_hz: 5.0,
            ..voice(0)
        };
        let error = bad.checked().expect_err("5 Hz is not a speaking register");
        assert!(error.contains("target_f0_hz"), "{error}");
        assert!(error.contains("intelligible"), "{error}");

        assert!(Voice {
            target_centroid_hz: 50.0,
            ..voice(0)
        }
        .checked()
        .is_err());
        assert!(Voice {
            target_tilt_db_oct: 40.0,
            ..voice(0)
        }
        .checked()
        .is_err());
        assert!(Voice {
            target_f0_hz: f32::NAN,
            ..voice(0)
        }
        .checked()
        .is_err());
    }

    /// Applying a voice replaces the destination and nothing else. The
    /// strengths are the user's setting.
    #[test]
    fn applying_a_voice_changes_only_the_destination() {
        let accent = AccentConfig {
            enabled: true,
            prosody_flatten: 0.5,
            vtln_strength: 0.4,
            ltas_strength: 0.3,
            ..AccentConfig::default()
        };
        let applied = voice(3).applied_to(accent);
        assert_eq!(applied.target_f0_hz, voice(3).target_f0_hz);
        assert_eq!(applied.target_centroid_hz, voice(3).target_centroid_hz);
        assert_eq!(applied.target_tilt_db_oct, voice(3).target_tilt_db_oct);
        assert_eq!(applied.prosody_flatten, 0.5, "a strength must survive");
        assert_eq!(applied.vtln_strength, 0.4);
        assert_eq!(applied.ltas_strength, 0.3);
        assert!(applied.enabled);
    }

    /// The slot is a function of the index and of nothing else. If this ever
    /// takes anything derived from the input, the output voice becomes a
    /// function of the input voice and the whole exercise is undone.
    #[test]
    fn a_slot_is_the_same_voice_every_time() {
        for index in 0..MAX_VOICES {
            assert_eq!(voice(index), voice(index));
        }
    }

    /// Past the table it wraps rather than panicking, and the collision is
    /// real: slot 10 is slot 0 again.
    #[test]
    fn asking_past_the_table_wraps_onto_a_voice_already_in_use() {
        assert_eq!(voice(MAX_VOICES), voice(0));
        assert_eq!(voice(MAX_VOICES * 3 + 4), voice(4));
        let _ = voice(usize::MAX);
    }

    /// A label describes where the voice arrived, never who was speaking.
    #[test]
    fn no_label_says_anything_about_the_original_speaker() {
        for voice in all() {
            let label = voice.describe().to_lowercase();
            assert!(!label.is_empty());
            for forbidden in [
                "man", "woman", "male", "female", "boy", "girl", "child", "old", "young",
            ] {
                assert!(
                    !label
                        .split(|c: char| !c.is_alphanumeric())
                        .any(|word| word == forbidden),
                    "{label} describes a person, not a destination"
                );
            }
        }
    }

    /// A label must quote the fundamental that will be heard, not the one that
    /// was asked for -- otherwise the interface and the ear disagree.
    #[test]
    fn a_label_quotes_the_rendered_fundamental() {
        let config = default_config();
        for voice in all() {
            let label = voice.describe();
            let rendered = format!("{:.0} Hz", voice.rendered_f0_hz(&config));
            assert!(
                label.contains(&rendered),
                "{label} does not quote its rendered {rendered}"
            );
        }
    }

    #[test]
    fn every_label_is_distinct() {
        let mut labels: Vec<String> = all().iter().map(Voice::describe).collect();
        labels.sort();
        let before = labels.len();
        labels.dedup();
        assert_eq!(before, labels.len(), "two slots describe themselves alike");
    }

    #[test]
    fn an_impossible_configuration_reports_no_bin_spacing_rather_than_dividing_by_zero() {
        let broken = DeidConfig {
            frame_size: 0,
            ..DeidConfig::default()
        };
        assert_eq!(bin_hz(&broken), 0.0);
        assert_eq!(voice(0).rendered_f0_hz(&broken), 0.0);
    }
}
