// SPDX-License-Identifier: GPL-3.0-or-later
//! Correcting a plan: who is speaking when, what they are called, and in what
//! colour.
//!
//! # Why a plan needs correcting at all
//!
//! A plan says which speaker each span of a recording belongs to, and something
//! had to decide that. Splitting a multi-track recording by channel is exact.
//! Everything else is a guess: a person listening and typing timestamps
//! mis-hears, a plan written from a meeting tool's own speaker labels inherits
//! that tool's mistakes, and two people talking over each other defeat both.
//!
//! A wrong span is the one mistake in this program that **cannot be heard in
//! the result**. Both voices are unfamiliar by construction, so a listener has
//! nothing to compare against: thirty seconds of Ada rendered in Grace's voice
//! sounds exactly like Grace talking. Nobody notices, which is why the
//! correction has to happen before the render and why it has to be easy.
//!
//! # What can be corrected
//!
//! Every operation here works on a plan somebody already has, and none of them
//! touches the audio:
//!
//! 1. **Reassign** a span to a different speaker, by index or by naming a
//!    moment inside it. This is the wrong-voice fix.
//! 2. **Split** a span at a moment, for when one span turns out to hold two
//!    people, which is what a missed hand-over looks like.
//! 3. **Merge** two neighbouring spans belonging to the same speaker, for when
//!    a split was wrong or a detector chopped one sentence into three.
//! 4. **Move** a span's edges, for a hand-over caught a second late.
//! 5. **Rename** a speaker, and give them **a colour**.
//!
//! # Nothing half-applies
//!
//! Every operation checks everything before it changes anything, so a refusal
//! leaves the plan exactly as it was. A half-applied edit to a plan is worse
//! than a refused one: it is a plan that looks fine and renders somebody in the
//! wrong voice, which is the failure this whole module exists to prevent.
//!
//! # In plain words
//!
//! Fixing a plan before you render it.
//!
//! If a stretch of the recording has been given to the wrong person, this is
//! how you move it. You can also cut one stretch in two where the hand-over was
//! missed, join two back together, nudge where one starts or ends, and change
//! what somebody is called and what colour they are.
//!
//! This matters more than it sounds. If the wrong person is on a stretch of
//! audio, the finished recording will not sound wrong to anybody, because every
//! voice in it is a voice nobody has heard before. There is nothing to notice.
//! So it has to be right before you render, and that means it has to be easy to
//! correct.

use crate::plan::{Conversation, Speaker, Turn};
use crate::Error;

/// A colour a speaker can be given, as `#rrggbb`.
///
/// Checked here rather than where it is drawn. A colour reaches an SVG, an HTML
/// page and a subtitle file, and a value that is not a colour would land in all
/// three as text: `fill="red; }"` closes an attribute somebody else opened.
pub fn check_colour(colour: &str) -> Result<String, Error> {
    let text = colour.trim();
    let body = text.strip_prefix('#').unwrap_or(text);
    if body.len() != 6 || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Malformed(format!(
            "{text:?} is not a colour. Write it as #rrggbb, such as #7aa2f7."
        )));
    }
    Ok(format!("#{}", body.to_ascii_lowercase()))
}

impl Conversation {
    /// Which span covers `secs`, if any.
    ///
    /// The **first** one when spans overlap, which is the earliest-starting
    /// speaker at that moment. Overlaps are ordinary here, so this is a stated
    /// rule rather than an assumption: a caller correcting an overlap works on
    /// one span at a time and can ask again after each.
    pub fn turn_at(&self, secs: f64) -> Option<usize> {
        self.turns()
            .iter()
            .position(|turn| secs >= turn.start && secs < turn.end)
    }

    /// Every span covering `secs`, earliest first.
    pub fn turns_at(&self, secs: f64) -> Vec<usize> {
        self.turns()
            .iter()
            .enumerate()
            .filter(|(_, turn)| secs >= turn.start && secs < turn.end)
            .map(|(index, _)| index)
            .collect()
    }

    /// Give a span to a different speaker.
    ///
    /// The timing is untouched: this is the fix for "the right stretch of
    /// audio, the wrong person", which is the mistake that cannot be heard.
    pub fn reassign(&mut self, turn: usize, speaker: usize) -> Result<(), Error> {
        let count = self.speakers().len();
        if speaker >= count {
            return Err(Error::Malformed(format!(
                "there is no speaker {speaker}; this plan has {count}"
            )));
        }
        let turns = self.turns_mut();
        let entry = turns
            .get_mut(turn)
            .ok_or_else(|| Error::Malformed(format!("there is no span {turn} in this plan")))?;
        entry.speaker = speaker;
        Ok(())
    }

    /// Give the span covering `secs` to a different speaker.
    ///
    /// The form a person actually wants: they heard the wrong voice at a
    /// timestamp and can say when, not which numbered span.
    pub fn reassign_at(&mut self, secs: f64, speaker: usize) -> Result<usize, Error> {
        let turn = self.turn_at(secs).ok_or_else(|| {
            Error::Malformed(format!(
                "nothing is assigned at {secs} s, so there is nothing to move. \
                 {}",
                self.nearest_hint(secs)
            ))
        })?;
        self.reassign(turn, speaker)?;
        Ok(turn)
    }

    /// Cut the span covering `secs` in two at that moment.
    ///
    /// Both halves keep the speaker; the caller reassigns whichever half was
    /// wrong, which is what a missed hand-over needs. Returns the indices of
    /// the two halves, earliest first.
    ///
    /// **The text stays with the first half** and the second gets none. There
    /// is no way to know where in a sentence a moment falls, and splitting the
    /// words at a guess would put half a sentence under the wrong speaker's
    /// name in the subtitles. Losing it from one half is visible; splitting it
    /// wrongly is not.
    pub fn split_at(&mut self, secs: f64) -> Result<(usize, usize), Error> {
        let turn = self.turn_at(secs).ok_or_else(|| {
            Error::Malformed(format!(
                "nothing is assigned at {secs} s, so there is nothing to cut. {}",
                self.nearest_hint(secs)
            ))
        })?;
        let existing = self.turns()[turn].clone();
        // A cut exactly on an edge would make a zero-length span, which
        // `add_turn` refuses for good reasons. Refused here with the reason,
        // rather than allowed to fail further in with a message about typos.
        if secs <= existing.start || secs >= existing.end {
            return Err(Error::Malformed(format!(
                "{secs} s is the edge of that span, not inside it; a cut there \
                 would make a span of no length"
            )));
        }

        let second = Turn {
            start: secs,
            end: existing.end,
            speaker: existing.speaker,
            text: None,
        };
        // The first half is shortened in place and the second added, so the
        // plan is never in a state where the audio between `secs` and the old
        // end belongs to nobody.
        self.turns_mut()[turn].end = secs;
        self.add_turn(second)?;

        let first = self
            .turn_at(existing.start)
            .expect("the shortened first half still covers its own start");
        let second = self
            .turn_at(secs)
            .expect("the second half was just added and covers its own start");
        Ok((first, second))
    }

    /// Join two spans of the same speaker into one.
    ///
    /// They have to belong to the same speaker and to touch or overlap.
    /// Anything else would silently hand somebody else's audio to whichever
    /// speaker was named first, which is the mistake this module exists to fix
    /// rather than to cause.
    ///
    /// The text of both is joined with a space where both have one.
    pub fn merge(&mut self, a: usize, b: usize) -> Result<usize, Error> {
        if a == b {
            return Err(Error::Malformed("a span cannot be joined to itself".into()));
        }
        let turns = self.turns();
        let (first, second) = match (turns.get(a), turns.get(b)) {
            (Some(one), Some(two)) => (one.clone(), two.clone()),
            _ => {
                return Err(Error::Malformed(format!(
                    "this plan has {} spans, so {a} and {b} are not both in it",
                    turns.len()
                )))
            }
        };
        if first.speaker != second.speaker {
            return Err(Error::Malformed(format!(
                "span {a} belongs to {} and span {b} to {}. Joining them would \
                 give one of them the other's audio; reassign one first.",
                self.speakers()[first.speaker].name,
                self.speakers()[second.speaker].name
            )));
        }
        let (early, late) = if first.start <= second.start {
            (first, second)
        } else {
            (second, first)
        };
        if late.start > early.end {
            return Err(Error::Malformed(format!(
                "those two spans have {} s of silence between them, which \
                 joining would hand to {}",
                late.start - early.end,
                self.speakers()[early.speaker].name
            )));
        }

        let text = match (early.text.clone(), late.text.clone()) {
            (Some(one), Some(two)) => Some(format!("{one} {two}")),
            (Some(one), None) => Some(one),
            (None, two) => two,
        };
        let joined = Turn {
            start: early.start,
            end: early.end.max(late.end),
            speaker: early.speaker,
            text,
        };

        // Removed by index, largest first, so removing one does not move the
        // other. The classic way to get this wrong is to remove the smaller
        // index and then use the larger one, which now names a different span.
        let (high, low) = if a > b { (a, b) } else { (b, a) };
        self.turns_mut().remove(high);
        self.turns_mut().remove(low);
        self.add_turn(joined)?;
        self.turn_at(early.start)
            .ok_or_else(|| Error::Malformed("the joined span went missing".into()))
    }

    /// Move a span's edges, keeping its speaker and its text.
    ///
    /// For a hand-over caught a second late. Validated exactly as a new span
    /// is, so a move cannot produce something [`Conversation::add_turn`] would
    /// have refused.
    pub fn move_edges(&mut self, turn: usize, start: f64, end: f64) -> Result<(), Error> {
        let existing = self
            .turns()
            .get(turn)
            .cloned()
            .ok_or_else(|| Error::Malformed(format!("there is no span {turn} in this plan")))?;
        let moved = Turn {
            start,
            end,
            ..existing.clone()
        };
        // Round-tripped through `add_turn` so there is one set of rules about
        // what a span may be, rather than a second copy here that drifts from
        // it. Removed only after the new one is known to be acceptable.
        self.turns_mut().remove(turn);
        if let Err(error) = self.add_turn(moved) {
            // Put it back exactly as it was: a refused edit changes nothing.
            self.add_turn(existing)?;
            return Err(error);
        }
        Ok(())
    }

    /// Rename one speaker.
    ///
    /// The whole-list form is [`Conversation::rename_speakers`], which is for a
    /// front end holding every name. This is for correcting one.
    pub fn set_name(&mut self, speaker: usize, name: &str) -> Result<(), Error> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Malformed("a speaker needs a name".into()));
        }
        if name.contains('\n') || name.contains('\r') || name.contains('\t') {
            return Err(Error::Malformed(
                "a name may not contain a line break or a tab, because the plan \
                 file separates its fields with them"
                    .into(),
            ));
        }
        let count = self.speakers().len();
        self.speakers_mut()
            .get_mut(speaker)
            .ok_or_else(|| {
                Error::Malformed(format!(
                    "there is no speaker {speaker}; this plan has {count}"
                ))
            })?
            .name = name.to_string();
        Ok(())
    }

    /// Give a speaker a colour, or take it away again.
    ///
    /// `None` returns them to the colour their slot gets from the palette,
    /// which is what every speaker starts with.
    pub fn set_colour(&mut self, speaker: usize, colour: Option<&str>) -> Result<(), Error> {
        let colour = colour.map(check_colour).transpose()?;
        let count = self.speakers().len();
        self.speakers_mut()
            .get_mut(speaker)
            .ok_or_else(|| {
                Error::Malformed(format!(
                    "there is no speaker {speaker}; this plan has {count}"
                ))
            })?
            .colour = colour;
        Ok(())
    }

    /// Where to look, for a timestamp that landed in no span.
    ///
    /// An error saying only "nothing there" sends somebody to open the file and
    /// count, so this names the nearest span and when it runs.
    fn nearest_hint(&self, secs: f64) -> String {
        let nearest = self.turns().iter().min_by(|a, b| {
            let da = (a.start - secs).abs().min((a.end - secs).abs());
            let db = (b.start - secs).abs().min((b.end - secs).abs());
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        match nearest {
            Some(turn) => format!(
                "The nearest span is {} from {:.3} s to {:.3} s.",
                self.speakers()[turn.speaker].name,
                turn.start,
                turn.end
            ),
            None => "This plan has no spans at all.".to_string(),
        }
    }
}

/// A speaker with a colour, for building one in a front end.
pub fn speaker_with_colour(name: &str, colour: Option<&str>) -> Result<Speaker, Error> {
    Ok(Speaker {
        name: name.trim().to_string(),
        picture: None,
        colour: colour.map(check_colour).transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three speakers and four spans, the shape most corrections happen on.
    fn plan() -> Conversation {
        let mut plan = Conversation::new();
        for name in ["Ada", "Grace", "Alan"] {
            plan.add_speaker(Speaker::named(name)).unwrap();
        }
        for (start, end, speaker, text) in [
            (0.0, 10.0, 0, Some("Hello there.")),
            (10.0, 25.0, 1, Some("And hello back.")),
            (25.0, 30.0, 1, None),
            (30.0, 45.0, 2, Some("A third voice.")),
        ] {
            plan.add_turn(Turn {
                start,
                end,
                speaker,
                text: text.map(str::to_string),
            })
            .unwrap();
        }
        plan
    }

    /// Every span in order, as `(start, end, speaker)`, for comparing shapes.
    fn shape(plan: &Conversation) -> Vec<(f64, f64, usize)> {
        plan.turns()
            .iter()
            .map(|t| (t.start, t.end, t.speaker))
            .collect()
    }

    #[test]
    fn a_moment_finds_the_span_holding_it() {
        let plan = plan();
        assert_eq!(plan.turn_at(0.0), Some(0));
        assert_eq!(plan.turn_at(9.999), Some(0));
        // The end is exclusive, so a boundary belongs to the span starting
        // there and not to the one ending there. Without that rule a moment on
        // a boundary is in two spans and every operation has to pick one.
        assert_eq!(plan.turn_at(10.0), Some(1));
        assert_eq!(plan.turn_at(45.0), None);
        assert_eq!(plan.turn_at(-1.0), None);
    }

    #[test]
    fn the_wrong_voice_is_moved_without_touching_the_timing() {
        let mut plan = plan();
        let before = shape(&plan);
        let turn = plan.reassign_at(15.0, 2).unwrap();
        assert_eq!(turn, 1);
        assert_eq!(plan.turns()[1].speaker, 2);
        // Every start and end is exactly where it was.
        for (a, b) in before.iter().zip(shape(&plan).iter()) {
            assert_eq!((a.0, a.1), (b.0, b.1));
        }
        // And the words went with it, because the words are that person's.
        assert_eq!(plan.turns()[1].text.as_deref(), Some("And hello back."));
    }

    #[test]
    fn a_moment_in_no_span_says_where_the_nearest_one_is() {
        let mut plan = plan();
        let refused = plan.reassign_at(60.0, 0).unwrap_err().to_string();
        assert!(refused.contains("nothing is assigned"), "{refused}");
        assert!(refused.contains("Alan"), "{refused}");
        assert!(refused.contains("45.000"), "{refused}");
        // And nothing changed.
        assert_eq!(shape(&plan), shape(&super::tests::plan()));
    }

    #[test]
    fn reassigning_to_a_speaker_who_is_not_there_is_refused() {
        let mut plan = plan();
        let before = shape(&plan);
        let refused = plan.reassign(0, 9).unwrap_err().to_string();
        assert!(refused.contains("no speaker 9"), "{refused}");
        assert!(refused.contains("3"), "{refused}");
        assert_eq!(shape(&plan), before);
    }

    #[test]
    fn a_missed_handover_is_cut_in_two_and_half_is_moved() {
        let mut plan = plan();
        // Grace's 10-to-25 span is really Grace until 18 and Alan after.
        let (first, second) = plan.split_at(18.0).unwrap();
        assert_eq!(plan.turns()[first], plan.turns()[first].clone());
        assert_eq!(
            (plan.turns()[first].start, plan.turns()[first].end),
            (10.0, 18.0)
        );
        assert_eq!(
            (plan.turns()[second].start, plan.turns()[second].end),
            (18.0, 25.0)
        );
        assert_eq!(plan.turns()[first].speaker, plan.turns()[second].speaker);

        plan.reassign(second, 2).unwrap();
        assert_eq!(
            shape(&plan),
            vec![
                (0.0, 10.0, 0),
                (10.0, 18.0, 1),
                (18.0, 25.0, 2),
                (25.0, 30.0, 1),
                (30.0, 45.0, 2),
            ]
        );

        // No second of the recording gained or lost an owner.
        let covered: f64 = plan.turns().iter().map(|t| t.duration()).sum();
        assert!((covered - 45.0).abs() < 1e-9, "{covered}");
    }

    #[test]
    fn the_words_stay_with_the_first_half_rather_than_being_guessed_at() {
        let mut plan = plan();
        let (first, second) = plan.split_at(5.0).unwrap();
        assert_eq!(plan.turns()[first].text.as_deref(), Some("Hello there."));
        assert!(plan.turns()[second].text.is_none());
    }

    #[test]
    fn a_cut_on_an_edge_is_refused_rather_than_making_an_empty_span() {
        let mut plan = plan();
        let before = shape(&plan);
        for at in [10.0, 25.0] {
            let refused = plan.split_at(at).unwrap_err().to_string();
            assert!(refused.contains("edge"), "{refused}");
        }
        assert_eq!(shape(&plan), before);
    }

    #[test]
    fn two_spans_of_one_speaker_join_back_together() {
        let mut plan = plan();
        // Grace has 10-25 and 25-30, which a detector chopped in two.
        let joined = plan.merge(1, 2).unwrap();
        assert_eq!(
            (plan.turns()[joined].start, plan.turns()[joined].end),
            (10.0, 30.0)
        );
        assert_eq!(plan.turns().len(), 3);
        assert_eq!(
            shape(&plan),
            vec![(0.0, 10.0, 0), (10.0, 30.0, 1), (30.0, 45.0, 2)]
        );
    }

    #[test]
    fn joining_is_the_same_whichever_order_the_two_are_named() {
        let mut forwards = plan();
        let mut backwards = plan();
        forwards.merge(1, 2).unwrap();
        backwards.merge(2, 1).unwrap();
        assert_eq!(shape(&forwards), shape(&backwards));
    }

    #[test]
    fn joining_two_different_speakers_is_refused_by_name() {
        let mut plan = plan();
        let before = shape(&plan);
        let refused = plan.merge(0, 1).unwrap_err().to_string();
        assert!(refused.contains("Ada"), "{refused}");
        assert!(refused.contains("Grace"), "{refused}");
        assert!(refused.contains("reassign one first"), "{refused}");
        assert_eq!(shape(&plan), before);
    }

    #[test]
    fn joining_across_a_gap_says_how_much_silence_it_would_hand_over() {
        let mut plan = Conversation::new();
        plan.add_speaker(Speaker::named("Ada")).unwrap();
        for (start, end) in [(0.0, 5.0), (20.0, 25.0)] {
            plan.add_turn(Turn {
                start,
                end,
                speaker: 0,
                text: None,
            })
            .unwrap();
        }
        let refused = plan.merge(0, 1).unwrap_err().to_string();
        assert!(refused.contains("15"), "{refused}");
        assert!(refused.contains("silence"), "{refused}");
    }

    #[test]
    fn a_span_can_be_nudged_and_a_bad_nudge_puts_it_back() {
        let mut plan = plan();
        plan.move_edges(0, 0.0, 11.0).unwrap();
        assert_eq!((plan.turns()[0].start, plan.turns()[0].end), (0.0, 11.0));
        assert_eq!(plan.turns()[0].text.as_deref(), Some("Hello there."));

        // Backwards, which `add_turn` refuses. The span has to survive the
        // attempt exactly as it was: an edit that half-applies is worse than
        // one that fails, because the plan then looks fine.
        let before = shape(&plan);
        assert!(plan.move_edges(0, 11.0, 0.0).is_err());
        assert_eq!(shape(&plan), before);
        assert_eq!(plan.turns()[0].text.as_deref(), Some("Hello there."));
    }

    #[test]
    fn a_colour_is_checked_before_it_can_reach_a_drawing() {
        assert_eq!(check_colour("#7AA2F7").unwrap(), "#7aa2f7");
        assert_eq!(check_colour("7aa2f7").unwrap(), "#7aa2f7");
        assert_eq!(check_colour(" #7aa2f7 ").unwrap(), "#7aa2f7");
        // The shapes that would land in an SVG attribute as text.
        for bad in ["red", "#12345", "#1234567", "#gggggg", "", "#7aa2f7; }"] {
            let refused = check_colour(bad).unwrap_err().to_string();
            assert!(refused.contains("#rrggbb"), "{bad}: {refused}");
        }
    }

    #[test]
    fn a_name_and_a_colour_can_be_changed_and_taken_away() {
        let mut plan = plan();
        plan.set_name(0, "  Ada Lovelace  ").unwrap();
        assert_eq!(plan.speakers()[0].name, "Ada Lovelace");

        assert_eq!(plan.colour_of(0, "#fallback"), "#fallback");
        plan.set_colour(0, Some("#bb9af7")).unwrap();
        assert_eq!(plan.colour_of(0, "#fallback"), "#bb9af7");
        plan.set_colour(0, None).unwrap();
        assert_eq!(plan.colour_of(0, "#fallback"), "#fallback");
    }

    #[test]
    fn a_name_that_could_forge_a_record_is_refused() {
        let mut plan = plan();
        for bad in ["", "   ", "a\nspeaker  1  b", "a\ttab"] {
            assert!(plan.set_name(0, bad).is_err(), "{bad:?} was accepted");
        }
        assert_eq!(plan.speakers()[0].name, "Ada");
    }

    #[test]
    fn a_chosen_colour_survives_the_round_trip_through_a_plan_file() {
        let mut plan = plan();
        plan.set_colour(1, Some("#9ece6a")).unwrap();
        let read_back = Conversation::parse(&plan.to_text()).unwrap();
        assert_eq!(read_back.speakers()[1].colour.as_deref(), Some("#9ece6a"));
        assert!(read_back.speakers()[0].colour.is_none());
        assert_eq!(shape(&read_back), shape(&plan));
    }

    #[test]
    fn a_colour_for_a_speaker_who_has_not_been_declared_is_refused() {
        let text = "VEILCONV1\ncolour  0  #9ece6a\nspeaker  0  Ada\n";
        let refused = Conversation::parse(text).unwrap_err().to_string();
        assert!(refused.contains("has not been declared"), "{refused}");
    }

    #[test]
    fn a_colour_that_is_not_one_is_refused_by_the_parser_too() {
        let text = "VEILCONV1\nspeaker  0  Ada\ncolour  0  red\n";
        let refused = Conversation::parse(text).unwrap_err().to_string();
        assert!(refused.contains("#rrggbb"), "{refused}");
    }

    #[test]
    fn every_correction_leaves_the_spans_in_time_order() {
        // Rendering, the subtitles and every report read the list in order and
        // none of them sorts for itself, so an edit that leaves it unsorted is
        // a silent wrong answer three places away.
        let mut plan = plan();
        plan.split_at(5.0).unwrap();
        plan.split_at(35.0).unwrap();
        plan.move_edges(0, 0.0, 4.0).unwrap();
        plan.merge(0, 1).ok();
        let starts: Vec<f64> = plan.turns().iter().map(|t| t.start).collect();
        let mut sorted = starts.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(starts, sorted, "{:?}", shape(&plan));
    }

    #[test]
    fn an_overlap_reports_every_speaker_at_that_moment() {
        let mut plan = Conversation::new();
        plan.add_speaker(Speaker::named("Ada")).unwrap();
        plan.add_speaker(Speaker::named("Grace")).unwrap();
        plan.add_turn(Turn {
            start: 0.0,
            end: 10.0,
            speaker: 0,
            text: None,
        })
        .unwrap();
        plan.add_turn(Turn {
            start: 5.0,
            end: 15.0,
            speaker: 1,
            text: None,
        })
        .unwrap();
        assert_eq!(plan.turns_at(7.0).len(), 2);
        assert_eq!(plan.turns_at(2.0).len(), 1);
        assert_eq!(plan.turns_at(12.0).len(), 1);
    }
}
