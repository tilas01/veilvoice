// SPDX-License-Identifier: GPL-3.0-or-later
//! Destination voices: several canonical registers instead of one.
//!
//! # What this is for
//!
//! By default every speaker VeilVoice processes comes out as **the same
//! voice** — one pitch register, one vocal-tract scale, one long-term spectrum.
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
//! voice 3, whoever they were. The mapping is still many-to-one — there are
//! still infinitely many inputs per output — so there is still no inverse to
//! compute. What changes is the number of buckets, from one to at most ten.
//!
//! This is why a voice is **never derived from the speaker**. Choosing a
//! destination by measuring the input — the obvious implementation, and the one
//! that would sound most natural — would make the output voice a function of the
//! input voice, which is precisely the linkage the project exists to destroy.
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
//!   on purpose — it is the thing that makes the result usable — and turn
//!   structure is information about a conversation. Overlaps, interruptions,
//!   the length of each answer and the rhythm of the exchange all survive.
//! * **Nothing about who they were.** The voiceprint of each speaker is
//!   destroyed exactly as thoroughly as in single-speaker mode: the same phase
//!   discard, the same many-to-one normalisation onto the slot's canonical
//!   values.
//!
//! A single-speaker recording leaks the first two facts too, and they are
//! trivial there. Here they are not, and that is worth a sentence in any front
//! end that offers this.
//!
//! # Ten, and not twenty
//!
//! Twenty was asked for. Ten is what can honestly be delivered, and the
//! arithmetic is short.
//!
//! A destination voice is distinguishable from another by its fundamental and
//! by its vocal-tract scale. The fundamental has to stay inside roughly
//! 100–210 Hz: below that the resynthesised comb has too few harmonics in the
//! band that carries the vowels, and above it the register stops being a
//! plausible speaking voice. The just-noticeable difference for the
//! fundamental of speech is around five to eight per cent, so about 10 Hz at
//! this register — and "just noticeable" is far too close together for somebody
//! to *keep track of* over an hour of conversation. Five registers across that
//! range gives steps of about 26 Hz, which is three times the threshold.
//!
//! Two vocal-tract scales, 660 Hz and 840 Hz, are about 25 % apart: a clear
//! difference in apparent speaker size, and both still inside the range where
//! the vowels stay natural.
//!
//! Five registers times two tracts is [`MAX_VOICES`]. Twenty would need either
//! 13 Hz steps in the fundamental — at the edge of audibility and well past the
//! edge of memorability — or vocal-tract scales close enough to be heard as the
//! same person on a different day. The honest number is ten, so the table has
//! ten and this paragraph says why.

use crate::AccentConfig;

/// How many distinct destination voices this engine will hand out.
///
/// See the module documentation for why this is ten and not twenty.
pub const MAX_VOICES: usize = 10;

/// One destination voice: the canonical values every speaker in this slot is
/// mapped onto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voice {
    /// Pitch register, in hertz.
    pub target_f0_hz: f32,
    /// Canonical long-term envelope centroid, in hertz — the vocal-tract scale.
    pub target_centroid_hz: f32,
    /// Slope of the canonical long-term spectrum, in dB per octave.
    pub target_tilt_db_oct: f32,
}

impl Voice {
    /// Apply this voice to an [`AccentConfig`].
    ///
    /// Only the three canonical targets are replaced. The *strengths* —
    /// how hard the neutraliser pushes toward them — are left alone, because
    /// they are the user's setting and a slot is a destination, not a policy
    /// about how firmly to arrive at it.
    pub fn applied_to(&self, mut accent: AccentConfig) -> AccentConfig {
        accent.target_f0_hz = self.target_f0_hz;
        accent.target_centroid_hz = self.target_centroid_hz;
        accent.target_tilt_db_oct = self.target_tilt_db_oct;
        accent
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

    /// A short label for an interface: "low voice, small tract" and so on.
    ///
    /// Describes the *destination*, never the speaker. There is deliberately no
    /// vocabulary here for who somebody was: "man", "woman", "child", "older"
    /// are all statements about an input this crate has just finished
    /// destroying, and a label that reintroduced one would undo the point.
    pub fn describe(&self) -> String {
        let register = if self.target_f0_hz < 120.0 {
            "low"
        } else if self.target_f0_hz < 145.0 {
            "low-mid"
        } else if self.target_f0_hz < 170.0 {
            "mid"
        } else if self.target_f0_hz < 196.0 {
            "mid-high"
        } else {
            "high"
        };
        let tract = if self.target_centroid_hz < 750.0 {
            "narrow"
        } else {
            "wide"
        };
        format!(
            "{register} register, {tract} tract ({:.0} Hz, {:.0} Hz)",
            self.target_f0_hz, self.target_centroid_hz
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

/// The five registers, spaced about 26 Hz apart — three times the
/// just-noticeable difference for the fundamental of speech at this register.
const REGISTERS_HZ: [f32; 5] = [105.0, 131.0, 157.0, 183.0, 209.0];

/// The ten destination voices, in the order they are handed out.
///
/// The order is chosen, not incidental. Slot 0 and slot 1 are the two furthest
/// apart in both dimensions, because a **two-person conversation is the common
/// case** and the two people in it should be the easiest pair in the table to
/// tell apart. Slot 2 is then the furthest remaining from both, and so on: the
/// table degrades gracefully as more speakers are added rather than saving its
/// clearest contrasts for a tenth speaker who is usually not there.
///
/// Every one of the five registers appears exactly twice and every one of the
/// two tracts exactly five times, so the set is the full five-by-two grid; only
/// the order is arranged.
const TABLE: [(usize, f32, f32); MAX_VOICES] = [
    // (register index, centroid Hz, tilt dB/octave)
    (0, 660.0, -5.0), // 0: lowest register, narrow tract
    (4, 840.0, -7.0), // 1: highest register, wide tract
    (2, 840.0, -7.0), // 2: middle register, the other tract
    (1, 660.0, -5.0), // 3
    (3, 660.0, -5.0), // 4
    (0, 840.0, -7.0), // 5
    (4, 660.0, -5.0), // 6
    (2, 660.0, -5.0), // 7
    (1, 840.0, -7.0), // 8
    (3, 840.0, -7.0), // 9
];

/// The destination voice for slot `index`.
///
/// Wraps rather than failing past [`MAX_VOICES`]: an eleventh speaker gets the
/// first voice again. That is a real collision — two people sharing one output
/// voice — and it is why [`MAX_VOICES`] is stated and why a front end should
/// refuse rather than rely on this. Wrapping is here so the function is total,
/// not because reusing a voice is acceptable.
pub fn voice(index: usize) -> Voice {
    let (register, centroid, tilt) = TABLE[index % MAX_VOICES];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_exactly_ten_and_they_are_all_different() {
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

    /// The table must be the full grid: every register twice, every tract five
    /// times. A typo that repeated one pair and dropped another would still
    /// pass the "all different" test above by accident of the tilt.
    #[test]
    fn the_table_is_the_whole_five_by_two_grid() {
        let voices = all();
        for register in REGISTERS_HZ {
            let count = voices
                .iter()
                .filter(|v| (v.target_f0_hz - register).abs() < 1e-3)
                .count();
            assert_eq!(count, 2, "{register} Hz appears {count} times, not twice");
        }
        for centroid in [660.0f32, 840.0] {
            let count = voices
                .iter()
                .filter(|v| (v.target_centroid_hz - centroid).abs() < 1e-3)
                .count();
            assert_eq!(count, 5, "{centroid} Hz appears {count} times, not five");
        }
    }

    /// The claim in the module documentation, checked: about 26 Hz between
    /// neighbouring registers, three times the roughly 8 Hz just-noticeable
    /// difference at this fundamental.
    #[test]
    fn the_registers_are_far_enough_apart_to_keep_track_of() {
        for pair in REGISTERS_HZ.windows(2) {
            let step = pair[1] - pair[0];
            assert!(
                step >= 24.0,
                "{} Hz to {} Hz is only {step} Hz apart",
                pair[0],
                pair[1]
            );
        }
    }

    /// A two-person conversation is the common case, so slots 0 and 1 must be
    /// the furthest apart in the table.
    #[test]
    fn the_first_two_slots_are_the_easiest_pair_to_tell_apart() {
        let voices = all();
        let separation = |a: &Voice, b: &Voice| {
            (a.target_f0_hz - b.target_f0_hz).abs() / 26.0
                + (a.target_centroid_hz - b.target_centroid_hz).abs() / 180.0
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
        // And no index panics.
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
                        .any(|w| w == forbidden),
                    "{label} describes a person, not a destination"
                );
            }
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
}
