// SPDX-License-Identifier: GPL-3.0-or-later
//! Who is in the recording, and who is speaking when.
//!
//! # VeilVoice does not work out who is talking, and will not guess
//!
//! Deciding which person is speaking at each moment is *speaker diarisation*,
//! and doing it from the audio alone needs a trained model. This project ships
//! no model, talks to no server, and is not about to start doing either — so
//! the turns come from the user, and there are exactly two honest ways to get
//! them:
//!
//! * **One microphone each.** If the recording has a channel per person, the
//!   split is already there and is exact. [`Conversation::from_channels`]
//!   builds the plan from that.
//! * **A list of turns.** Times and speakers, in a text file, written by
//!   whoever was there or produced by whatever tool they already use for
//!   transcripts.
//!
//! What would be worse than either is guessing. A wrong guess maps two people
//! onto one voice — which is a privacy *improvement* and a usability disaster —
//! or splits one person across two voices, which invites a listener to believe
//! there was somebody in the room who was not. Neither failure would be visible
//! in the output, and both would be blamed on the recording rather than on the
//! tool.
//!
//! # Format
//!
//! Text, one record per line, for the same reason everything else here is text:
//! a file describing who said what is worth more if it can be read, checked and
//! edited without this program.
//!
//! ```text
//! VEILCONV1
//! title  Two people, one microphone
//! speaker  0  Alex
//! speaker  1  Sam  portrait.png
//! turn  0.000  4.200  0  Hello -- how did it go?
//! turn  4.100  9.050  1
//! ```
//!
//! Times are seconds with a decimal point. The text on a turn is optional: with
//! it, subtitles carry the words; without it they carry the speaker's name and
//! nothing else, which is still enough to follow a conversation whose voices
//! have all been replaced.
//!
//! Overlapping turns are allowed, because people talk over each other, and
//! [`crate::render`] mixes them rather than picking a winner.
//!
//! # In plain words
//!
//! A list of who is in a recording and when each of them speaks.
//!
//! VeilVoice does not work this out for itself. Deciding who is talking at any
//! moment is a hard problem that needs a trained model, and this project does not
//! ship one, so it asks instead. You either write the times down, or you record
//! each person on their own microphone.
//!
//! That is less convenient and it is honest. A program that guessed would
//! sometimes put one person's words in another person's voice, and you would not
//! find out by listening, because the result would sound perfectly fine.

use crate::Error;
use std::path::PathBuf;
use veilvoice_core::voices::{Voice, MAX_VOICES};

/// Magic first line. The digit is a format version.
const MAGIC: &str = "VEILCONV1";

/// One person in the recording.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Speaker {
    /// What to call them in subtitles and in an interface.
    ///
    /// A label for a human, and **not** part of the de-identification: writing
    /// somebody's real name here puts their real name in the subtitle file. The
    /// audio is veiled; a name typed into a caption is not.
    pub name: String,
    /// A picture to show for them in a rendered video, if there is one.
    ///
    /// `None` means a plain filled circle in their colour, which is what most
    /// people will use — and which reveals nothing, unlike a photograph.
    pub picture: Option<PathBuf>,
}

impl Speaker {
    /// A speaker with a name and no picture.
    pub fn named(name: &str) -> Self {
        Self {
            name: name.trim().to_string(),
            picture: None,
        }
    }
}

/// A span of the recording belonging to one speaker.
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    /// When it starts, in seconds from the beginning of the recording.
    pub start: f64,
    /// When it ends, in seconds.
    pub end: f64,
    /// Which speaker, as an index into [`Conversation::speakers`].
    pub speaker: usize,
    /// What was said, if anybody wrote it down.
    ///
    /// VeilVoice does not transcribe: there is no model here and there is no
    /// server to ask. This is carried through to the subtitles when it is
    /// supplied and left out when it is not.
    pub text: Option<String>,
}

impl Turn {
    /// How long this turn lasts, in seconds.
    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// The whole plan: who is in the recording, and when each of them speaks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Conversation {
    /// A title for the recording, shown on a rendered video.
    pub title: Option<String>,
    speakers: Vec<Speaker>,
    turns: Vec<Turn>,
    /// Whether each speaker gets their own voice, or one between them.
    ///
    /// Private, and **not written to the plan file**. A plan says who is in the
    /// recording and when they speak; how they are rendered is decided at
    /// render time by whoever is rendering. A mode stored in a shared file
    /// would silently change what somebody else's render sounds like.
    mode: crate::mode::VoiceMode,
}

impl Conversation {
    /// An empty plan.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a speaker, and return the index they were given.
    ///
    /// The index is also the destination-voice slot, so the first speaker added
    /// gets [`veilvoice_core::voices::voice`] 0 and the second gets voice 1 —
    /// which are the two furthest apart in the table, because two people is the
    /// common case.
    pub fn add_speaker(&mut self, speaker: Speaker) -> Result<usize, Error> {
        if self.speakers.len() >= MAX_VOICES {
            return Err(Error::TooManySpeakers(MAX_VOICES));
        }
        if speaker.name.trim().is_empty() {
            return Err(Error::Malformed("a speaker needs a name".into()));
        }
        if speaker.name.contains('\n') || speaker.name.contains('\r') {
            return Err(Error::Malformed(
                "a speaker's name may not contain a line break: it would be able to \
                 forge a record"
                    .into(),
            ));
        }
        self.speakers.push(Speaker {
            name: speaker.name.trim().to_string(),
            picture: speaker.picture,
        });
        Ok(self.speakers.len() - 1)
    }

    /// Add a turn.
    pub fn add_turn(&mut self, turn: Turn) -> Result<(), Error> {
        if !turn.start.is_finite() || !turn.end.is_finite() {
            return Err(Error::Malformed(
                "a turn needs real start and end times".into(),
            ));
        }
        if turn.start < 0.0 {
            return Err(Error::Malformed(format!(
                "a turn cannot start at {} seconds",
                turn.start
            )));
        }
        if turn.end <= turn.start {
            return Err(Error::Malformed(format!(
                "a turn from {} to {} seconds has no length. A zero-length turn is \
                 almost always a typo, and silently dropping it would leave that \
                 speech in whichever voice happened to be next.",
                turn.start, turn.end
            )));
        }
        if turn.speaker >= self.speakers.len() {
            return Err(Error::Malformed(format!(
                "turn at {} s names speaker {}, and only {} are declared",
                turn.start,
                turn.speaker,
                self.speakers.len()
            )));
        }
        if let Some(text) = &turn.text {
            if text.contains('\n') || text.contains('\r') {
                return Err(Error::Malformed(
                    "a turn's text may not contain a line break".into(),
                ));
            }
        }
        self.turns.push(turn);
        // Kept in time order so rendering, subtitles and every report agree
        // about what comes first without each having to sort for itself.
        self.turns.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.speaker.cmp(&b.speaker))
        });
        Ok(())
    }

    /// Rename everybody, in slot order, keeping every turn where it is.
    ///
    /// For a front end that holds the names and reads the turns from a plan
    /// file somebody wrote earlier. The names are the ones just typed; the
    /// turns are the plan's, and nothing here touches them.
    ///
    /// # Refused rather than reconciled
    ///
    /// The count has to match exactly. A plan naming three speakers renamed
    /// from a list of two would either leave one person with a stale name or
    /// silently drop a slot, and a dropped slot means somebody's audio comes
    /// out in another person's voice — the one mistake here that cannot be
    /// heard in the result, because both voices are unfamiliar.
    ///
    /// Every name is validated exactly as [`Conversation::add_speaker`]
    /// validates one, and for the same reasons: an empty name labels nobody,
    /// and a name containing a line break can forge a record in the plan file.
    /// Nothing is changed unless every name passes, so a refusal leaves the
    /// plan exactly as it was rather than half-renamed.
    pub fn rename_speakers(&mut self, names: &[String]) -> Result<(), Error> {
        if names.len() != self.speakers.len() {
            return Err(Error::Malformed(format!(
                "this plan has {} speaker(s) and {} name(s) were given",
                self.speakers.len(),
                names.len()
            )));
        }
        let mut checked = Vec::with_capacity(names.len());
        for name in names {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err(Error::Malformed("a speaker needs a name".into()));
            }
            if trimmed.contains('\n') || trimmed.contains('\r') {
                return Err(Error::Malformed(
                    "a speaker's name may not contain a line break: it would be able to \
                     forge a record"
                        .into(),
                ));
            }
            checked.push(trimmed.to_string());
        }
        for (speaker, name) in self.speakers.iter_mut().zip(checked) {
            speaker.name = name;
        }
        Ok(())
    }

    /// The speakers, in the order they were added.
    pub fn speakers(&self) -> &[Speaker] {
        &self.speakers
    }

    /// The turns, in time order.
    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    /// How many speakers there are.
    pub fn len(&self) -> usize {
        self.speakers.len()
    }

    /// Whether there is nobody in the plan.
    pub fn is_empty(&self) -> bool {
        self.speakers.is_empty()
    }

    /// The destination voice for a speaker.
    pub fn voice(&self, speaker: usize) -> Voice {
        self.mode.voice_for(speaker)
    }

    /// Whether every speaker gets their own voice, or one between them.
    ///
    /// Not persisted in the plan file, and deliberately so. A plan says *who is
    /// in the recording and when they speak*; how they are rendered is a
    /// decision made at render time, by whoever is doing the rendering. Writing
    /// it into the file would mean a plan somebody shared could silently change
    /// what a later render sounds like.
    pub fn mode(&self) -> crate::mode::VoiceMode {
        self.mode
    }

    /// Render every speaker as the same voice, or as their own.
    ///
    /// Refuses when this plan holds more speakers than the mode can carry --
    /// which for [`crate::mode::VoiceMode::Distinct`] is how many voices are
    /// far enough apart to be told apart, measured under `config`.
    pub fn set_mode(
        &mut self,
        mode: crate::mode::VoiceMode,
        config: &veilvoice_core::DeidConfig,
    ) -> Result<(), crate::mode::TooMany> {
        crate::mode::check(self.speakers.len(), mode, config)?;
        self.mode = mode;
        Ok(())
    }

    /// When the last turn ends, in seconds.
    pub fn duration(&self) -> f64 {
        self.turns.iter().map(|turn| turn.end).fold(0.0, f64::max)
    }

    /// Turns where two people are speaking at once.
    ///
    /// Reported rather than refused: people talk over each other, and a plan
    /// that forbade it would be a plan that cannot describe a real
    /// conversation. [`crate::render`] mixes the overlap.
    pub fn overlaps(&self) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        for (i, a) in self.turns.iter().enumerate() {
            for (j, b) in self.turns.iter().enumerate().skip(i + 1) {
                if b.start >= a.end {
                    // Sorted by start, so nothing later can overlap this one.
                    break;
                }
                if a.speaker != b.speaker {
                    found.push((i, j));
                }
            }
        }
        found
    }

    /// Spans where a speaker's turns overlap **their own** other turns.
    ///
    /// Distinct from [`Conversation::overlaps`] and much more likely to be a
    /// mistake: one person cannot be in two places in their own recording, so
    /// this is usually a typed time rather than an interruption.
    pub fn self_overlaps(&self) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        for (i, a) in self.turns.iter().enumerate() {
            for (j, b) in self.turns.iter().enumerate().skip(i + 1) {
                if b.start >= a.end {
                    break;
                }
                if a.speaker == b.speaker {
                    found.push((i, j));
                }
            }
        }
        found
    }

    /// A plan for a recording with one microphone per person.
    ///
    /// The only split VeilVoice can make on its own, because it is not a guess:
    /// if each person had their own channel then each channel *is* one person,
    /// and the whole recording is one turn per channel.
    ///
    /// The names are the ones supplied; there must be one per channel.
    pub fn from_channels(names: &[&str], duration_secs: f64) -> Result<Self, Error> {
        if names.is_empty() {
            return Err(Error::Malformed("no channels were named".into()));
        }
        if !duration_secs.is_finite() || duration_secs <= 0.0 {
            return Err(Error::Malformed(format!(
                "a recording cannot be {duration_secs} seconds long"
            )));
        }
        let mut conversation = Self::new();
        for name in names {
            conversation.add_speaker(Speaker::named(name))?;
        }
        for speaker in 0..conversation.speakers.len() {
            conversation.add_turn(Turn {
                start: 0.0,
                end: duration_secs,
                speaker,
                text: None,
            })?;
        }
        Ok(conversation)
    }

    /// Serialise to the text format described at the top of this module.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        if let Some(title) = &self.title {
            out.push_str(&format!("title  {title}\n"));
        }
        for (index, speaker) in self.speakers.iter().enumerate() {
            match &speaker.picture {
                Some(picture) => out.push_str(&format!(
                    "speaker  {index}  {}  {}\n",
                    speaker.name,
                    picture.display().to_string().replace('\\', "/")
                )),
                None => out.push_str(&format!("speaker  {index}  {}\n", speaker.name)),
            }
        }
        for turn in &self.turns {
            match &turn.text {
                Some(text) => out.push_str(&format!(
                    "turn  {:.3}  {:.3}  {}  {}\n",
                    turn.start, turn.end, turn.speaker, text
                )),
                None => out.push_str(&format!(
                    "turn  {:.3}  {:.3}  {}\n",
                    turn.start, turn.end, turn.speaker
                )),
            }
        }
        out
    }

    /// Parse the text format.
    ///
    /// An unknown keyword is refused rather than skipped. A plan is a statement
    /// about who is in a recording, and honouring half of one written by a
    /// newer build would put somebody's speech in the wrong voice without
    /// saying anything.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.trim() == MAGIC => {}
            Some(other) => {
                return Err(Error::Malformed(format!(
                    "expected {MAGIC} on the first line, found {other:?}"
                )))
            }
            None => return Err(Error::Malformed("the plan is empty".into())),
        }

        let mut conversation = Self::new();
        let mut pending: Vec<(usize, Turn)> = Vec::new();
        for (index, line) in lines.enumerate() {
            let number = index + 2;
            if line.trim().is_empty() {
                continue;
            }
            let Some((keyword, rest)) = line.split_once("  ") else {
                return Err(Error::Malformed(format!(
                    "line {number}: no keyword, found {line:?}"
                )));
            };
            match keyword {
                "title" => conversation.title = Some(rest.trim().to_string()),
                "speaker" => {
                    let mut parts = rest.splitn(3, "  ");
                    let declared = parts.next().unwrap_or_default().trim();
                    let name = parts.next().unwrap_or_default().trim();
                    let picture = parts.next().map(|p| PathBuf::from(p.trim()));
                    let declared: usize = declared.parse().map_err(|_| {
                        Error::Malformed(format!("line {number}: bad index {declared:?}"))
                    })?;
                    // The index is written down and checked rather than
                    // implied by position: a hand-edited file with two
                    // speakers numbered 0 would otherwise silently give one of
                    // them the other's voice.
                    if declared != conversation.speakers.len() {
                        return Err(Error::Malformed(format!(
                            "line {number}: speaker {declared} appears where speaker {} \
                             was expected. Speakers are numbered from zero, in order.",
                            conversation.speakers.len()
                        )));
                    }
                    conversation.add_speaker(Speaker {
                        name: name.to_string(),
                        picture,
                    })?;
                }
                "turn" => {
                    let mut parts = rest.splitn(4, "  ");
                    let start = parts.next().unwrap_or_default().trim();
                    let end = parts.next().unwrap_or_default().trim();
                    let speaker = parts.next().unwrap_or_default().trim();
                    let text = parts.next().map(|t| t.trim().to_string());
                    let start: f64 = start.parse().map_err(|_| {
                        Error::Malformed(format!("line {number}: bad start {start:?}"))
                    })?;
                    let end: f64 = end
                        .parse()
                        .map_err(|_| Error::Malformed(format!("line {number}: bad end {end:?}")))?;
                    let speaker: usize = speaker.parse().map_err(|_| {
                        Error::Malformed(format!("line {number}: bad speaker {speaker:?}"))
                    })?;
                    pending.push((
                        number,
                        Turn {
                            start,
                            end,
                            speaker,
                            text: text.filter(|t| !t.is_empty()),
                        },
                    ));
                }
                other => {
                    return Err(Error::Malformed(format!(
                        "line {number}: unknown keyword {other:?}"
                    )))
                }
            }
        }

        // Turns are added after every speaker is known, so a file that lists
        // its turns first is still readable and a turn naming an undeclared
        // speaker is still refused.
        for (number, turn) in pending {
            conversation
                .add_turn(turn)
                .map_err(|error| Error::Malformed(format!("line {number}: {error}")))?;
        }
        Ok(conversation)
    }

    /// Write the plan to `path`.
    pub fn save(&self, path: &std::path::Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.to_text())?;
        Ok(())
    }

    /// Read a plan written by [`Conversation::save`].
    pub fn load(path: &std::path::Path) -> Result<Self, Error> {
        Self::parse(&std::fs::read_to_string(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_people() -> Conversation {
        let mut conversation = Conversation::new();
        conversation.add_speaker(Speaker::named("Alex")).unwrap();
        conversation.add_speaker(Speaker::named("Sam")).unwrap();
        conversation
            .add_turn(Turn {
                start: 0.0,
                end: 4.2,
                speaker: 0,
                text: Some("Hello -- how did it go?".into()),
            })
            .unwrap();
        conversation
            .add_turn(Turn {
                start: 4.5,
                end: 9.05,
                speaker: 1,
                text: None,
            })
            .unwrap();
        conversation
    }

    #[test]
    fn speakers_are_numbered_in_the_order_they_are_added() {
        let mut conversation = Conversation::new();
        assert_eq!(conversation.add_speaker(Speaker::named("Alex")).unwrap(), 0);
        assert_eq!(conversation.add_speaker(Speaker::named("Sam")).unwrap(), 1);
        assert_eq!(conversation.len(), 2);
        assert!(!conversation.is_empty());
    }

    /// The first two speakers must get the two voices furthest apart in the
    /// table, because two people is the common case.
    #[test]
    fn the_first_two_speakers_get_the_two_most_distinct_voices() {
        let conversation = two_people();
        assert_eq!(conversation.voice(0), veilvoice_core::voices::voice(0));
        assert_eq!(conversation.voice(1), veilvoice_core::voices::voice(1));
        assert_ne!(conversation.voice(0), conversation.voice(1));
    }

    #[test]
    fn there_can_be_no_more_speakers_than_there_are_voices() {
        let mut conversation = Conversation::new();
        for index in 0..MAX_VOICES {
            conversation
                .add_speaker(Speaker::named(&format!("Person {index}")))
                .unwrap();
        }
        let error = conversation
            .add_speaker(Speaker::named("One too many"))
            .expect_err("the eleventh must be refused, not given a reused voice");
        assert!(matches!(error, Error::TooManySpeakers(MAX_VOICES)));
        assert!(error.to_string().contains("10"), "{error}");
    }

    #[test]
    fn a_nameless_speaker_is_refused() {
        let mut conversation = Conversation::new();
        assert!(conversation.add_speaker(Speaker::named("   ")).is_err());
        assert!(conversation.add_speaker(Speaker::named("a\nb")).is_err());
        assert!(conversation.is_empty());
    }

    #[test]
    fn a_turn_naming_an_undeclared_speaker_is_refused() {
        let mut conversation = Conversation::new();
        conversation.add_speaker(Speaker::named("Alex")).unwrap();
        let error = conversation
            .add_turn(Turn {
                start: 0.0,
                end: 1.0,
                speaker: 4,
                text: None,
            })
            .expect_err("speaker 4 does not exist");
        assert!(error.to_string().contains("only 1"), "{error}");
    }

    /// A zero-length turn would leave that speech in whichever voice was next,
    /// silently. Refused.
    #[test]
    fn an_impossible_turn_is_refused() {
        let mut conversation = Conversation::new();
        conversation.add_speaker(Speaker::named("Alex")).unwrap();
        for (start, end) in [(1.0, 1.0), (2.0, 1.0), (-1.0, 1.0)] {
            assert!(
                conversation
                    .add_turn(Turn {
                        start,
                        end,
                        speaker: 0,
                        text: None,
                    })
                    .is_err(),
                "{start} to {end} should be refused"
            );
        }
        assert!(conversation
            .add_turn(Turn {
                start: f64::NAN,
                end: 1.0,
                speaker: 0,
                text: None,
            })
            .is_err());
        assert!(conversation.turns().is_empty());
    }

    #[test]
    fn turns_are_kept_in_time_order_however_they_arrive() {
        let mut conversation = Conversation::new();
        conversation.add_speaker(Speaker::named("Alex")).unwrap();
        for start in [8.0, 2.0, 5.0, 0.0] {
            conversation
                .add_turn(Turn {
                    start,
                    end: start + 1.0,
                    speaker: 0,
                    text: None,
                })
                .unwrap();
        }
        let starts: Vec<f64> = conversation.turns().iter().map(|t| t.start).collect();
        assert_eq!(starts, vec![0.0, 2.0, 5.0, 8.0]);
        assert_eq!(conversation.duration(), 9.0);
    }

    /// People talk over each other, so an overlap is reported and not refused.
    #[test]
    fn an_overlap_between_two_people_is_reported_rather_than_refused() {
        let mut conversation = two_people();
        // Sam cuts in while Alex is still talking, and well before Sam's own
        // later turn -- an interruption, and not Sam overlapping Sam.
        conversation
            .add_turn(Turn {
                start: 2.0,
                end: 3.0,
                speaker: 1,
                text: None,
            })
            .expect("an interruption is a real thing that happens");
        let overlaps = conversation.overlaps();
        assert!(!overlaps.is_empty(), "the interruption should be reported");
        assert!(conversation.self_overlaps().is_empty());
    }

    /// One person cannot be in two places in their own recording, so this is
    /// almost always a typed time -- reported separately, and more loudly.
    #[test]
    fn a_speaker_overlapping_themselves_is_reported_separately() {
        let mut conversation = Conversation::new();
        conversation.add_speaker(Speaker::named("Alex")).unwrap();
        conversation
            .add_turn(Turn {
                start: 0.0,
                end: 5.0,
                speaker: 0,
                text: None,
            })
            .unwrap();
        conversation
            .add_turn(Turn {
                start: 3.0,
                end: 8.0,
                speaker: 0,
                text: None,
            })
            .unwrap();
        assert_eq!(conversation.self_overlaps().len(), 1);
        assert!(conversation.overlaps().is_empty());
    }

    /// The one split VeilVoice can make on its own, because it is not a guess.
    #[test]
    fn a_channel_per_person_needs_no_diarisation() {
        let conversation = Conversation::from_channels(&["Alex", "Sam"], 30.0).unwrap();
        assert_eq!(conversation.len(), 2);
        assert_eq!(conversation.turns().len(), 2);
        assert_eq!(conversation.duration(), 30.0);
        // Both talk for the whole recording, which is what a channel each
        // means -- and which is an overlap by construction.
        assert!(!conversation.overlaps().is_empty());

        assert!(Conversation::from_channels(&[], 30.0).is_err());
        assert!(Conversation::from_channels(&["Alex"], 0.0).is_err());
        assert!(Conversation::from_channels(&["Alex"], f64::NAN).is_err());
    }

    #[test]
    fn a_plan_survives_a_round_trip_through_text() {
        let mut conversation = two_people();
        conversation.title = Some("Two people, one microphone".into());
        let text = conversation.to_text();
        let read_back = Conversation::parse(&text).expect("its own output must parse");
        assert_eq!(read_back.title, conversation.title);
        assert_eq!(read_back.speakers(), conversation.speakers());
        assert_eq!(read_back.turns().len(), conversation.turns().len());
        assert_eq!(read_back.to_text(), text, "and byte for byte");
    }

    #[test]
    fn a_plan_survives_a_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let conversation = two_people();
        let path = dir.path().join("deeper").join("plan.txt");
        conversation.save(&path).unwrap();
        assert_eq!(Conversation::load(&path).unwrap(), conversation);
    }

    #[test]
    fn a_picture_survives_the_round_trip() {
        let mut conversation = Conversation::new();
        conversation
            .add_speaker(Speaker {
                name: "Sam".into(),
                picture: Some(PathBuf::from("portraits/sam.png")),
            })
            .unwrap();
        let read_back = Conversation::parse(&conversation.to_text()).unwrap();
        assert_eq!(
            read_back.speakers()[0].picture,
            Some(PathBuf::from("portraits/sam.png"))
        );
    }

    /// A file that lists its turns before its speakers must still read.
    #[test]
    fn turns_before_speakers_still_parse() {
        let text = format!("{MAGIC}\nturn  0.000  1.000  0\nspeaker  0  Alex\n");
        let conversation = Conversation::parse(&text).unwrap();
        assert_eq!(conversation.len(), 1);
        assert_eq!(conversation.turns().len(), 1);
    }

    /// A hand-edited file with two speakers numbered 0 would otherwise give
    /// one of them the other's voice, silently.
    #[test]
    fn a_repeated_speaker_number_is_refused() {
        let text = format!("{MAGIC}\nspeaker  0  Alex\nspeaker  0  Sam\n");
        let error = Conversation::parse(&text).expect_err("must refuse");
        assert!(error.to_string().contains("numbered from zero"), "{error}");
    }

    #[test]
    fn a_malformed_plan_is_refused_rather_than_half_read() {
        assert!(Conversation::parse("").is_err(), "empty");
        assert!(
            Conversation::parse("NOT-THE-MAGIC\n").is_err(),
            "wrong magic"
        );
        for bad in [
            "speaker  x  Alex",
            "speaker  0  Alex\nturn  x  1.0  0",
            "speaker  0  Alex\nturn  0.0  x  0",
            "speaker  0  Alex\nturn  0.0  1.0  x",
            "speaker  0  Alex\nturn  0.0  1.0  9",
            "whatever  1",
            "nokeyword",
        ] {
            let text = format!("{MAGIC}\n{bad}\n");
            assert!(
                Conversation::parse(&text).is_err(),
                "should have been refused: {bad:?}"
            );
        }
    }

    #[test]
    fn blank_lines_are_tolerated() {
        let conversation = two_people();
        let padded = conversation.to_text().replace('\n', "\n\n");
        assert_eq!(Conversation::parse(&padded).unwrap(), conversation);
    }

    /// Renaming keeps the turns exactly where they were.
    #[test]
    fn renaming_leaves_every_turn_alone() {
        let mut plan = two_people();
        let before = plan.turns().to_vec();
        plan.rename_speakers(&["Robin".to_string(), "Jules".to_string()])
            .unwrap();
        assert_eq!(plan.speakers()[0].name, "Robin");
        assert_eq!(plan.speakers()[1].name, "Jules");
        assert_eq!(plan.turns(), before.as_slice(), "the turns must not move");
    }

    /// The one mistake here that cannot be heard in the result: a mismatch
    /// would put somebody's audio in another person's voice, and both voices
    /// are unfamiliar, so nobody would notice.
    #[test]
    fn a_different_number_of_names_is_refused() {
        let mut plan = two_people();
        let error = plan
            .rename_speakers(&["Only one".to_string()])
            .expect_err("two speakers, one name");
        assert!(format!("{error}").contains('2'), "{error}");
        assert_eq!(plan.speakers()[0].name, "Alex", "nothing may have changed");
    }

    /// Every name is checked the way `add_speaker` checks one, and nothing is
    /// changed unless all of them pass.
    #[test]
    fn a_bad_name_leaves_the_plan_exactly_as_it_was() {
        let mut plan = two_people();
        for bad in ["", "   ", "Robin\nspeaker  9  Mallory"] {
            let error = plan
                .rename_speakers(&["Robin".to_string(), bad.to_string()])
                .expect_err("{bad:?} should be refused");
            let _ = error;
            assert_eq!(
                plan.speakers()[0].name,
                "Alex",
                "a refusal must not half-rename"
            );
            assert_eq!(plan.speakers()[1].name, "Sam");
        }
    }

    #[test]
    fn names_are_trimmed_the_same_way_they_are_when_added() {
        let mut plan = two_people();
        plan.rename_speakers(&["  Robin  ".to_string(), "Jules".to_string()])
            .unwrap();
        assert_eq!(plan.speakers()[0].name, "Robin");
    }

    /// Uniform mode gives every speaker the same voice. This is the whole of
    /// what the mode does to the sound, so it is asserted directly.
    #[test]
    fn uniform_mode_gives_every_speaker_one_voice() {
        use crate::mode::VoiceMode;
        let config = veilvoice_core::DeidConfig::default();
        let mut plan = two_people();
        assert_ne!(plan.voice(0), plan.voice(1), "distinct by default");

        plan.set_mode(VoiceMode::Uniform, &config).unwrap();
        assert_eq!(plan.voice(0), plan.voice(1), "one voice between them");
        assert_eq!(plan.mode(), VoiceMode::Uniform);

        plan.set_mode(VoiceMode::Distinct, &config).unwrap();
        assert_ne!(plan.voice(0), plan.voice(1), "and back again");
    }

    /// A plan with more speakers than there are separable voices cannot be put
    /// into distinct mode, and the refusal says what to do instead.
    #[test]
    fn distinct_mode_is_refused_past_the_measured_limit() {
        use crate::mode::VoiceMode;
        let config = veilvoice_core::DeidConfig::default();
        let mut plan = Conversation::new();
        for index in 0..9 {
            plan.add_speaker(Speaker::named(&format!("P{index}")))
                .unwrap();
        }
        // Nine is past the eight that are clearly separable...
        let error = plan
            .set_mode(VoiceMode::Distinct, &config)
            .expect_err("nine speakers, eight clear voices");
        assert!(error.to_string().contains("one voice for everybody"));
        // ...and uniform mode carries them, which is the point of the refusal
        // naming it.
        plan.set_mode(VoiceMode::Uniform, &config).unwrap();
        assert_eq!(plan.mode(), VoiceMode::Uniform);
        for slot in 0..9 {
            assert_eq!(plan.voice(slot), plan.voice(0));
        }
    }

    /// The mode is not written to the plan file. A plan says who speaks when;
    /// how it is rendered is the renderer's decision, and a mode hidden in a
    /// shared file would change what somebody else's render sounds like.
    #[test]
    fn the_mode_is_not_carried_in_the_file() {
        use crate::mode::VoiceMode;
        let config = veilvoice_core::DeidConfig::default();
        let mut plan = two_people();
        plan.set_mode(VoiceMode::Uniform, &config).unwrap();

        let text = plan.to_text();
        assert!(!text.to_lowercase().contains("uniform"), "{text}");
        assert!(!text.to_lowercase().contains("mode"), "{text}");

        let read_back = Conversation::parse(&text).unwrap();
        assert_eq!(
            read_back.mode(),
            VoiceMode::Distinct,
            "a plan read from disk renders distinct until told otherwise"
        );
    }

    #[test]
    fn a_turn_reports_its_own_length() {
        let turn = Turn {
            start: 1.5,
            end: 4.0,
            speaker: 0,
            text: None,
        };
        assert!((turn.duration() - 2.5).abs() < 1e-9);
    }
}
