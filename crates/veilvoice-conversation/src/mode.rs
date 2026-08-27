// SPDX-License-Identifier: GPL-3.0-or-later
//! How many voices a group gets, and the trade between the two answers.
//!
//! # The limit is measured, not chosen
//!
//! The engine holds ten destination voices and all ten are different. Only
//! **eight** are far enough apart that somebody following a conversation can
//! tell which is which: adding the ninth brings the closest pair to 1.1842 --
//! two voices with exactly the same rendered pitch and vocal tracts 18 % apart
//! -- and that is under the three-semitone floor a listener needs when the two
//! voices are half a minute apart rather than side by side.
//!
//! [`veilvoice_core::voices::clear_voices`] computes it from the configuration
//! in force, because a coarser frame grid collapses registers onto each other
//! and eight stops being true.
//!
//! # Two ways to be told apart, and the second one is safer
//!
//! [`VoiceMode::Distinct`] gives each speaker their own voice. It is the
//! obvious arrangement and it is what most recordings want, and it is capped at
//! the measured limit, because handing two people voices nobody can separate
//! produces a recording in which two speakers sound like one — discovered only
//! after the recording exists.
//!
//! [`VoiceMode::Uniform`] gives **everybody the same voice**, and the speakers
//! are told apart by their names in the subtitles and by which circle lights up
//! in the picture. That has two consequences worth stating plainly, one of each
//! kind:
//!
//! * **It is more private.** In distinct mode the output carries one bit of
//!   structure the input had: *this is speaker three*. Anybody who obtains two
//!   recordings of the same group can align them by voice slot. Uniform mode
//!   does not have that structure to leak — every speaker is the same voice, so
//!   there is nothing to align.
//! * **It is harder to follow by ear alone.** A listener with no subtitles and
//!   no picture cannot tell who is speaking. That is the price, and it is why
//!   this is not the default.
//!
//! Uniform mode has **no speaker limit** from voices, because there is no
//! second voice to collide with. The plan's own ten-speaker limit still
//! applies, because ten names is already a great deal to follow.
//!
//! # In plain words
//!
//! How many people can be in one recording, and the choice between two ways of
//! handling them.
//!
//! Give everybody a different voice and a listener can follow the conversation by
//! ear, but only so many of the available voices are far enough apart to actually
//! be told apart. That number was measured rather than picked, and the limit is
//! real: past it, two people would sound like one person and you would only find
//! out by listening to the finished recording.
//!
//! Give everybody the *same* voice and there is no limit, and it is more private,
//! because the result no longer carries even the fact of who was speaker three.
//! The price is that names and pictures become the only way to tell who is talking.

use veilvoice_core::voices::{self, Voice};
use veilvoice_core::DeidConfig;

/// Whether speakers get different voices or one voice between them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VoiceMode {
    /// A different destination voice per speaker, capped at the measured
    /// number that are clearly separable.
    #[default]
    Distinct,
    /// One voice for everybody, told apart by name rather than by sound.
    Uniform,
}

impl VoiceMode {
    /// A short name, for a picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::Distinct => "a voice each",
            Self::Uniform => "one voice for everybody",
        }
    }

    /// The most speakers this mode can carry under `config`.
    ///
    /// For [`VoiceMode::Distinct`] this is the measured clear limit. For
    /// [`VoiceMode::Uniform`] it is the plan's own limit, because voices are no
    /// longer what bounds it.
    pub fn speaker_limit(self, config: &DeidConfig) -> usize {
        match self {
            Self::Distinct => voices::clear_voices(config),
            Self::Uniform => voices::MAX_VOICES,
        }
    }

    /// The voice a slot gets in this mode.
    ///
    /// Uniform mode returns slot 0's voice for everybody. Slot 0 rather than a
    /// new one: it is a voice the table already contains and the tests already
    /// cover, and inventing an eleventh just to be the shared one would be a
    /// voice nobody had measured.
    pub fn voice_for(self, slot: usize) -> Voice {
        match self {
            Self::Distinct => voices::voice(slot),
            Self::Uniform => voices::voice(0),
        }
    }

    /// What this mode costs and buys, in the words a front end should show.
    pub fn note(self) -> &'static str {
        match self {
            Self::Distinct => {
                "Each person gets a different voice, so a listener can follow the \
                 conversation by ear. The number of speakers is capped at how many \
                 voices are far enough apart to actually be told apart -- measured, \
                 not guessed."
            }
            Self::Uniform => {
                "Everybody gets the same voice. Nobody can be picked out by how they \
                 sound, not even as \"the third speaker\", so two recordings of the \
                 same group cannot be lined up by voice. The price is that the names \
                 in the subtitles and the picture are the only way to tell who is \
                 speaking -- by ear alone, you cannot."
            }
        }
    }
}

/// Why a group cannot be rendered as asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TooMany {
    /// More speakers than there are clearly separable voices.
    ForDistinct {
        /// How many were asked for.
        asked: usize,
        /// How many voices are clearly separable here.
        limit: usize,
    },
    /// More speakers than a plan can hold at all.
    ForAnyMode {
        /// How many were asked for.
        asked: usize,
        /// The plan's own limit.
        limit: usize,
    },
}

impl std::fmt::Display for TooMany {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForDistinct { asked, limit } => write!(
                f,
                "{asked} speakers, and only {limit} destination voices are far enough \
                 apart to be told apart by ear. Two of them would sound like the same \
                 person. Either use {limit} or fewer, or switch to one voice for \
                 everybody -- which has no such limit, is more private, and leaves the \
                 names and the picture to say who is speaking."
            ),
            Self::ForAnyMode { asked, limit } => write!(
                f,
                "{asked} speakers is past the {limit} a plan can hold. Ten names is \
                 already a great deal for a listener to follow."
            ),
        }
    }
}

impl std::error::Error for TooMany {}

/// Whether this many speakers can be rendered in this mode.
pub fn check(count: usize, mode: VoiceMode, config: &DeidConfig) -> Result<(), TooMany> {
    if count > voices::MAX_VOICES {
        return Err(TooMany::ForAnyMode {
            asked: count,
            limit: voices::MAX_VOICES,
        });
    }
    let limit = mode.speaker_limit(config);
    if count > limit {
        return Err(TooMany::ForDistinct {
            asked: count,
            limit,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_is_the_default_because_most_recordings_want_it() {
        assert_eq!(VoiceMode::default(), VoiceMode::Distinct);
    }

    /// The measured number, and the reason this module exists.
    #[test]
    fn distinct_mode_stops_at_the_measured_clear_limit() {
        let config = DeidConfig::default();
        assert_eq!(VoiceMode::Distinct.speaker_limit(&config), 8);
        assert!(check(8, VoiceMode::Distinct, &config).is_ok());
        let error = check(9, VoiceMode::Distinct, &config).expect_err("nine is too many");
        assert!(matches!(error, TooMany::ForDistinct { asked: 9, limit: 8 }));
    }

    /// The refusal has to point at the way out, or it is a dead end.
    #[test]
    fn the_refusal_names_the_alternative() {
        let config = DeidConfig::default();
        let error = check(9, VoiceMode::Distinct, &config).unwrap_err();
        let words = error.to_string();
        assert!(words.contains("one voice for everybody"), "{words}");
        assert!(words.contains("more private"), "{words}");
        assert!(words.contains('8'), "{words}");
    }

    /// Uniform mode has no voice-based limit, because there is no second voice.
    #[test]
    fn uniform_mode_carries_everybody_a_plan_can_hold() {
        let config = DeidConfig::default();
        assert_eq!(
            VoiceMode::Uniform.speaker_limit(&config),
            voices::MAX_VOICES
        );
        for count in 1..=voices::MAX_VOICES {
            assert!(check(count, VoiceMode::Uniform, &config).is_ok(), "{count}");
        }
    }

    /// And the plan's own limit still applies to both.
    #[test]
    fn neither_mode_goes_past_what_a_plan_can_hold() {
        let config = DeidConfig::default();
        for mode in [VoiceMode::Distinct, VoiceMode::Uniform] {
            let error = check(11, mode, &config).expect_err("eleven is past the plan");
            assert!(matches!(error, TooMany::ForAnyMode { asked: 11, .. }));
        }
    }

    /// Uniform means uniform: every slot is the same voice, and it is one the
    /// table already contains rather than an eleventh nobody measured.
    #[test]
    fn uniform_gives_every_slot_the_same_measured_voice() {
        let first = VoiceMode::Uniform.voice_for(0);
        for slot in 0..voices::MAX_VOICES {
            assert_eq!(VoiceMode::Uniform.voice_for(slot), first, "slot {slot}");
        }
        assert_eq!(first, voices::voice(0));
    }

    #[test]
    fn distinct_gives_every_slot_its_own() {
        for slot in 0..voices::MAX_VOICES {
            assert_eq!(VoiceMode::Distinct.voice_for(slot), voices::voice(slot));
        }
    }

    /// A coarser frame grid collapses registers, so the distinct limit has to
    /// fall with it rather than keep promising eight.
    #[test]
    fn a_coarser_grid_lowers_the_distinct_limit_and_not_the_uniform_one() {
        let coarse = DeidConfig {
            frame_size: 128,
            ..DeidConfig::default()
        };
        assert!(
            VoiceMode::Distinct.speaker_limit(&coarse)
                <= VoiceMode::Distinct.speaker_limit(&DeidConfig::default())
        );
        assert_eq!(
            VoiceMode::Uniform.speaker_limit(&coarse),
            voices::MAX_VOICES,
            "one voice cannot collide with itself"
        );
    }

    /// Both notes have to say what the mode costs, not only what it gives.
    #[test]
    fn every_note_states_the_price_as_well_as_the_benefit() {
        let distinct = VoiceMode::Distinct.note().to_lowercase();
        assert!(distinct.contains("capped"), "{distinct}");
        assert!(distinct.contains("measured"), "{distinct}");

        let uniform = VoiceMode::Uniform.note().to_lowercase();
        assert!(uniform.contains("the price is"), "{uniform}");
        assert!(uniform.contains("by ear alone, you cannot"), "{uniform}");
    }
}
