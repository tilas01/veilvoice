// SPDX-License-Identifier: GPL-3.0-or-later
//! Subtitles, from the same plan the audio is rendered from.
//!
//! # Two formats, both written out here
//!
//! **WebVTT** is what a browser plays alongside a `<video>`, and it is the one
//! to use with anything this project renders. **SubRip** (`.srt`) is what every
//! other player on earth reads. They differ in three small ways — a header, a
//! cue counter, and a comma instead of a full stop in the timestamp — so both
//! come from one function with a flag rather than from two that drift.
//!
//! No library. The workspace carries no subtitle crate and this is forty lines;
//! adding a dependency to the graph for it would cost more than it saves, and
//! the `offline` CI job that checks what is in that graph is one of the things
//! this project is worth trusting for.
//!
//! # What goes in a cue when nobody wrote down the words
//!
//! VeilVoice does not transcribe. Where a turn has no text, the cue carries the
//! **speaker's name and nothing else** — which is still worth having: after
//! every voice has been replaced, a caption track saying who is talking is
//! often the only way to follow a recording at all.
//!
//! # A name in a caption is not veiled
//!
//! Worth saying twice, because it is the mistake this feature invites. The
//! audio has had its voiceprints destroyed. The subtitle file is a text file
//! containing whatever names were typed into it, sitting next to the recording.
//! If the names matter, use labels rather than names — the plan does not care
//! which, and [`crate::SCOPE`] says so where a user will read it.

use crate::Conversation;

/// Which subtitle format to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// WebVTT, for a browser. Header, no cue numbers, `.` in timestamps.
    WebVtt,
    /// SubRip, for everything else. No header, numbered cues, `,` in
    /// timestamps.
    SubRip,
}

impl Format {
    /// The conventional file extension, without the dot.
    pub fn extension(&self) -> &'static str {
        match self {
            Format::WebVtt => "vtt",
            Format::SubRip => "srt",
        }
    }
}

/// A timestamp as the subtitle formats want it: `HH:MM:SS.mmm`.
///
/// Negative and non-finite inputs become zero rather than producing a cue no
/// player will accept. A plan cannot contain either — [`Conversation::add_turn`]
/// refuses both — so this is the belt to that braces, and it is here because
/// the alternative failure is a subtitle file that silently does not load.
fn timestamp(seconds: f64, comma: bool) -> String {
    let seconds = if seconds.is_finite() && seconds > 0.0 {
        seconds
    } else {
        0.0
    };
    let total_ms = (seconds * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_secs = total_ms / 1000;
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = total_secs / 3600;
    let separator = if comma { ',' } else { '.' };
    format!("{h:02}:{m:02}:{s:02}{separator}{ms:03}")
}

/// Render the plan as subtitles.
pub fn write(conversation: &Conversation, format: Format) -> String {
    let comma = format == Format::SubRip;
    let mut out = String::new();
    if format == Format::WebVtt {
        out.push_str("WEBVTT\n");
        if let Some(title) = &conversation.title {
            // A NOTE block, which every player ignores and every text editor
            // shows. The title belongs in the file it describes.
            out.push_str(&format!("\nNOTE {}\n", one_line(title)));
        }
        out.push('\n');
    }

    for (index, turn) in conversation.turns().iter().enumerate() {
        if format == Format::SubRip {
            out.push_str(&format!("{}\n", index + 1));
        }
        out.push_str(&format!(
            "{} --> {}\n",
            timestamp(turn.start, comma),
            timestamp(turn.end, comma)
        ));
        let name = conversation
            .speakers()
            .get(turn.speaker)
            .map(|speaker| speaker.name.as_str())
            .unwrap_or("unknown speaker");
        match &turn.text {
            Some(text) => out.push_str(&format!("{}: {}\n", one_line(name), one_line(text))),
            // No transcript, so the cue is the one fact there is. After every
            // voice has been replaced this is often the only way to follow a
            // recording.
            None => out.push_str(&format!("{}\n", one_line(name))),
        }
        out.push('\n');
    }
    out
}

/// Flatten anything that would break a cue into one line.
///
/// A cue ends at a blank line and a timestamp line is found by its arrow, so a
/// name or a line of text containing either could end one cue early and start
/// something a player would try to read as a timestamp. The plan already
/// refuses line breaks; the arrow is caught here.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ").replace("-->", "->")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Speaker, Turn};

    fn two_people() -> Conversation {
        let mut conversation = Conversation::new();
        conversation.title = Some("Two people".into());
        conversation.add_speaker(Speaker::named("Alex")).unwrap();
        conversation.add_speaker(Speaker::named("Sam")).unwrap();
        conversation
            .add_turn(Turn {
                start: 0.0,
                end: 4.2,
                speaker: 0,
                text: Some("Hello, how did it go?".into()),
            })
            .unwrap();
        conversation
            .add_turn(Turn {
                start: 4.5,
                end: 3671.5,
                speaker: 1,
                text: None,
            })
            .unwrap();
        conversation
    }

    #[test]
    fn webvtt_opens_with_its_header_and_subrip_does_not() {
        assert!(write(&two_people(), Format::WebVtt).starts_with("WEBVTT\n"));
        assert!(!write(&two_people(), Format::SubRip).starts_with("WEBVTT"));
    }

    #[test]
    fn subrip_numbers_its_cues_and_webvtt_does_not() {
        let srt = write(&two_people(), Format::SubRip);
        assert!(srt.starts_with("1\n"), "{srt}");
        assert!(srt.contains("\n2\n"), "{srt}");
        let vtt = write(&two_people(), Format::WebVtt);
        assert!(!vtt.contains("\n1\n00:"), "{vtt}");
    }

    #[test]
    fn the_timestamp_separator_differs_between_the_two() {
        assert!(write(&two_people(), Format::WebVtt).contains("00:00:00.000"));
        assert!(write(&two_people(), Format::SubRip).contains("00:00:00,000"));
    }

    /// Over an hour, so the hour field is exercised rather than assumed.
    #[test]
    fn hours_minutes_seconds_and_milliseconds_are_all_right() {
        assert_eq!(timestamp(0.0, false), "00:00:00.000");
        assert_eq!(timestamp(4.2, false), "00:00:04.200");
        assert_eq!(timestamp(61.5, false), "00:01:01.500");
        assert_eq!(timestamp(3671.5, false), "01:01:11.500");
        assert_eq!(timestamp(3671.5, true), "01:01:11,500");
        assert!(write(&two_people(), Format::WebVtt).contains("01:01:11.500"));
    }

    /// A cue no player accepts is worse than a wrong one, because it fails
    /// silently on somebody else's machine.
    #[test]
    fn an_impossible_time_becomes_zero_rather_than_a_broken_cue() {
        assert_eq!(timestamp(-5.0, false), "00:00:00.000");
        assert_eq!(timestamp(f64::NAN, false), "00:00:00.000");
        assert_eq!(timestamp(f64::INFINITY, false), "00:00:00.000");
    }

    /// The whole point of a subtitle track when every voice has been replaced.
    #[test]
    fn a_turn_with_no_words_still_says_who_was_speaking() {
        let vtt = write(&two_people(), Format::WebVtt);
        assert!(vtt.contains("Sam\n"), "{vtt}");
        assert!(vtt.contains("Alex: Hello, how did it go?"), "{vtt}");
    }

    /// An arrow inside a name or a line of text would start something a player
    /// reads as a timestamp.
    #[test]
    fn an_arrow_in_the_text_cannot_forge_a_timestamp_line() {
        let mut conversation = Conversation::new();
        conversation
            .add_speaker(Speaker::named("00:00:00.000 --> 00:00:09.000"))
            .unwrap();
        conversation
            .add_turn(Turn {
                start: 0.0,
                end: 1.0,
                speaker: 0,
                text: Some("and then --> that happened".into()),
            })
            .unwrap();
        let vtt = write(&conversation, Format::WebVtt);
        assert_eq!(
            vtt.matches(" --> ").count(),
            1,
            "exactly one real timestamp line: {vtt}"
        );
    }

    #[test]
    fn the_title_is_carried_into_webvtt_as_a_note() {
        let vtt = write(&two_people(), Format::WebVtt);
        assert!(vtt.contains("NOTE Two people"), "{vtt}");
        // And SubRip has nowhere to put one, so it does not pretend to.
        assert!(!write(&two_people(), Format::SubRip).contains("Two people"));
    }

    #[test]
    fn an_empty_plan_produces_a_valid_empty_file() {
        let empty = Conversation::new();
        let vtt = write(&empty, Format::WebVtt);
        assert_eq!(vtt.trim(), "WEBVTT");
        assert!(write(&empty, Format::SubRip).is_empty());
    }

    #[test]
    fn the_extensions_are_the_conventional_ones() {
        assert_eq!(Format::WebVtt.extension(), "vtt");
        assert_eq!(Format::SubRip.extension(), "srt");
    }

    /// Every cue must be separated by a blank line, or players merge them.
    #[test]
    fn every_cue_is_terminated() {
        for format in [Format::WebVtt, Format::SubRip] {
            let text = write(&two_people(), format);
            assert!(text.ends_with("\n\n"), "{format:?}: {text:?}");
            assert_eq!(
                text.matches(" --> ").count(),
                2,
                "{format:?} lost a cue: {text}"
            );
        }
    }
}
