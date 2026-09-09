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
//! produces a recording in which two speakers sound like one, discovered only
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
//!   does not have that structure to leak, because every speaker is the same voice, so
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

/// What the finished recording says about **who** was speaking.
///
/// # The question this answers, and the one it does not
///
/// It does **not** measure how well any one person's voice is disguised. That
/// is what the engine does, every speaker is mapped onto a canonical
/// destination, and it does not get weaker because somebody else joined the
/// call. A recording of eight people hides each of their voiceprints exactly as
/// well as a recording of one.
///
/// What it measures is the *other* leak, the one that does grow with the group:
/// **how much of the conversation's structure a listener gets for free.** Give
/// eight people eight tellable-apart voices and anybody who hears the output can
/// count the participants, follow who said what, and line two recordings of the
/// same group up against each other by voice. Give them all one voice and none
/// of that is there to find.
///
/// # It is not cryptography, and this does not pretend it is
///
/// There is no key here, no work factor and no adversary bounded by
/// computation. Nothing about this number gets better with a longer key or
/// worse with a faster machine. It is an information count about one specific
/// thing: which of the speakers a given turn belongs to. Calling that
/// "cryptographic strength" would be the kind of sentence this project spends
/// most of its documentation refusing to write.
///
/// # Where the bits come from
///
/// A listener who cannot tell the voices apart has to guess which of `classes`
/// speakers produced a turn, and that guess costs `log2(classes)` bits. When
/// the voices *are* tellable apart the output hands those bits over. So the
/// leak is `log2(classes)` bits per turn: zero when everybody shares one voice,
/// one bit for two, three bits for eight.
///
/// `classes` is not simply the number of speakers. Two speakers whose voices
/// fall within [`voices::CLEAR_SEPARATION`] of each other cannot reliably be
/// separated by ear, so they count as one class. That is the part of this that
/// runs the other way from intuition: crowding the table makes the recording
/// *leak less* and *follow worse* at the same time, which is the trade the two
/// modes exist to let somebody choose between.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exposure {
    /// How many voice classes a listener can separate in the output.
    ///
    /// One in [`VoiceMode::Uniform`], whatever the group size. In
    /// [`VoiceMode::Distinct`] it is the number of speakers, less any that were
    /// given voices too close together to tell apart.
    pub classes: usize,
    /// Bits about the speaker of a turn that the output gives away, per turn.
    ///
    /// `log2(classes)`. Zero means the output says nothing about which of them
    /// was talking.
    pub bits: f32,
    /// The same thing as a score out of a hundred, where a hundred is a
    /// recording that says nothing about who is who.
    ///
    /// Measured against the widest the engine can go, which is
    /// [`voices::MAX_VOICES`] classes, so the number means the same thing
    /// whatever configuration it was asked about.
    pub score: u8,
    /// The closest pair among the voices actually handed out, as a ratio.
    ///
    /// The *usability* side, and it moves opposite to the score: below
    /// [`voices::CLEAR_SEPARATION`] two people start sounding like one. Carried
    /// here so a front end can show both halves of the trade rather than a
    /// number that looks like it only goes one way.
    pub crowding: f32,
}

impl Exposure {
    /// What this recording gives away, for `speakers` people under `config`.
    pub fn of(mode: VoiceMode, speakers: usize, config: &DeidConfig) -> Self {
        let classes = match mode {
            // One voice for everybody is one class however many people there
            // are. There is no structure in the output to count.
            VoiceMode::Uniform => 1,
            VoiceMode::Distinct => separable(speakers, config),
        };
        let bits = if classes <= 1 {
            0.0
        } else {
            (classes as f32).log2()
        };
        let widest = (voices::MAX_VOICES as f32).log2();
        let score = if widest <= 0.0 {
            100.0
        } else {
            100.0 * (1.0 - bits / widest)
        };
        Self {
            classes,
            bits,
            score: score.clamp(0.0, 100.0).round() as u8,
            crowding: voices::closest_pair(
                match mode {
                    VoiceMode::Uniform => 1,
                    VoiceMode::Distinct => speakers,
                },
                config,
            ),
        }
    }

    /// Whether the voices handed out are too close to be told apart.
    ///
    /// The usability failure, not the privacy one. A recording in this state
    /// leaks *less* and is harder to follow, which is why it is reported rather
    /// than folded into the score.
    pub fn crowded(&self) -> bool {
        // One voice has nothing to be confused with. `closest_pair` answers
        // 1.0 for a single voice, which is below the floor and is not a warning
        // about anything: without this guard every solo recording carried a
        // "two of these sound alike" notice about the one voice in it.
        self.classes > 1 && self.crowding < voices::CLEAR_SEPARATION
    }

    /// One sentence for the interface, saying what the number means here.
    pub fn note(&self) -> String {
        if self.classes <= 1 {
            return "Everybody is the same voice, so the recording does not say which \
                    of them was speaking. A listener cannot pick anyone out, not even \
                    as \"the third speaker\", and two recordings of this group cannot \
                    be lined up against each other by voice."
                .to_string();
        }
        format!(
            "A listener can separate {} voices, which tells them which of the speakers \
             each turn belongs to: {:.1} bits per turn. They can count the people in \
             the room and follow who said what. This is about the shape of the \
             conversation and not about the voices themselves, which are disguised the \
             same either way.",
            self.classes, self.bits
        )
    }
}

/// How many of the first `count` voices a listener can actually separate.
///
/// Voices are handed out in table order, so this walks them and puts each into
/// an existing class when it is within [`voices::CLEAR_SEPARATION`] of one
/// already there. Greedy and in table order deliberately: that is the order
/// slots are given out in, so this counts the classes that will actually exist
/// rather than the best packing of the same voices.
fn separable(count: usize, config: &DeidConfig) -> usize {
    if count == 0 {
        return 0;
    }
    let table = voices::all();
    let taken = &table[..count.min(table.len())];
    let mut classes: Vec<Voice> = Vec::with_capacity(taken.len());
    for voice in taken {
        let merged = classes
            .iter()
            .any(|held| voices::separation(held, voice, config) < voices::CLEAR_SEPARATION);
        if !merged {
            classes.push(*voice);
        }
    }
    classes.len().max(1)
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

#[cfg(test)]
mod exposure_tests {
    use super::*;
    #[test]
    fn a_bigger_group_gives_more_away() {
        let config = DeidConfig::default();
        let mut last = 101u8;
        for speakers in 1..=voices::MAX_VOICES {
            let seen = Exposure::of(VoiceMode::Distinct, speakers, &config);
            assert!(
                seen.score <= last,
                "{speakers} speakers scored {} after {last}, so the score went up as \
                 the group grew",
                seen.score
            );
            last = seen.score;
        }
    }

    /// One voice for everybody says nothing about who is who, at any size.
    #[test]
    fn one_voice_for_everybody_gives_nothing_away() {
        let config = DeidConfig::default();
        for speakers in [1, 2, 8, voices::MAX_VOICES, 40] {
            let seen = Exposure::of(VoiceMode::Uniform, speakers, &config);
            assert_eq!(seen.classes, 1, "{speakers} in uniform mode");
            assert_eq!(seen.bits, 0.0);
            assert_eq!(seen.score, 100, "{speakers} in uniform mode");
        }
    }

    /// The bits are the count they claim to be.
    ///
    /// A listener who cannot separate the voices guesses which of `classes`
    /// spoke, and that guess costs `log2(classes)`. Checked against the
    /// arithmetic rather than against a table somebody typed.
    #[test]
    fn the_bits_are_the_logarithm_of_the_classes() {
        let config = DeidConfig::default();
        for speakers in 1..=voices::MAX_VOICES {
            let seen = Exposure::of(VoiceMode::Distinct, speakers, &config);
            let expected = if seen.classes <= 1 {
                0.0
            } else {
                (seen.classes as f32).log2()
            };
            assert!(
                (seen.bits - expected).abs() < 1e-5,
                "{speakers} speakers: {} bits for {} classes",
                seen.bits,
                seen.classes
            );
        }
    }

    /// Two people given voices too close to separate count as one class.
    ///
    /// This is the part that runs the other way from intuition, so it is
    /// checked rather than asserted in a comment: crowding the table makes the
    /// recording leak *less* while making it harder to follow, which is the
    /// trade the two modes exist to let somebody choose between.
    #[test]
    fn voices_nobody_can_separate_are_one_class_and_leak_as_one() {
        // A frame size coarse enough to collapse registers onto each other.
        let coarse = DeidConfig {
            frame_size: 256,
            ..DeidConfig::default()
        };

        let asked = voices::MAX_VOICES;
        let seen = Exposure::of(VoiceMode::Distinct, asked, &coarse);
        let clear = voices::clear_voices(&coarse);

        if clear < asked {
            assert!(
                seen.classes < asked,
                "{asked} voices collapsed to {clear} clear ones, and this still counted \
                 {} classes",
                seen.classes
            );
            let fine = Exposure::of(VoiceMode::Distinct, asked, &DeidConfig::default());
            assert!(
                seen.score >= fine.score,
                "the crowded configuration leaks more than the clear one, which is \
                 backwards: {} against {}",
                seen.score,
                fine.score
            );
            assert!(seen.crowded(), "the crowding was not reported");
        }
    }

    /// One voice has nothing to be confused with.
    ///
    /// `closest_pair` answers 1.0 when given fewer than two voices, which is
    /// below the separation floor and means "nothing to compare" rather than
    /// "too close". Read without that in mind, every solo recording carried a
    /// warning that two of its voices sounded alike.
    #[test]
    fn one_speaker_is_never_reported_as_crowded() {
        let config = DeidConfig::default();
        for mode in [VoiceMode::Distinct, VoiceMode::Uniform] {
            let alone = Exposure::of(mode, 1, &config);
            assert!(
                !alone.crowded(),
                "{mode:?} with one speaker was called crowded, at {}",
                alone.crowding
            );
            assert_eq!(alone.score, 100, "one speaker gives nothing away");
        }
        // And a group whose voices really are too close still says so.
        let coarse = DeidConfig {
            frame_size: 256,
            ..DeidConfig::default()
        };
        if voices::clear_voices(&coarse) < voices::MAX_VOICES {
            let many = Exposure::of(VoiceMode::Distinct, voices::MAX_VOICES, &coarse);
            assert!(
                many.crowded(),
                "a genuinely crowded group stopped saying so"
            );
        }
    }

    /// The note says which question the number answers.
    ///
    /// The one thing this must never be read as is a claim about how well a
    /// voice is disguised, so the words that would invite that reading are
    /// checked for absence.
    #[test]
    fn the_note_does_not_claim_to_be_about_the_voices_themselves() {
        let config = DeidConfig::default();
        for mode in [VoiceMode::Distinct, VoiceMode::Uniform] {
            let note = Exposure::of(mode, 4, &config).note().to_lowercase();
            for wrong in ["encrypt", "cryptograph", "key", "unbreakable", "secure"] {
                assert!(
                    !note.contains(wrong),
                    "{mode:?} note says {wrong:?}, which invites reading an information \
                     count as a claim about strength"
                );
            }
        }
    }
}
