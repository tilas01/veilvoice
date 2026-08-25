// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-conversation
//!
//! Several people in one recording: a plan of who spoke when, a distinct
//! destination voice for each of them, and subtitles that carry their names.
//!
//! ## Why this exists
//!
//! VeilVoice's whole argument is that every speaker is mapped onto **one**
//! canonical voice, so many inputs give one output and there is no inverse to
//! compute. Run an interview through it and both people come out as the same
//! voice — which is perfectly private and completely unusable, because a
//! listener cannot tell a question from its answer.
//!
//! This crate keeps the property and fixes the usability. Each speaker is
//! assigned a **slot**, each slot has its own canonical destination
//! ([`veilvoice_core::voices`]), and every speaker in a slot is normalised onto
//! that destination exactly as thoroughly as a lone speaker is normalised onto
//! the default one. There are ten buckets instead of one; each is still
//! many-to-one.
//!
//! ## What a conversation costs, said plainly
//!
//! * **The number of speakers survives.** Three voices in the output means
//!   three people were in the room.
//! * **The turn-taking survives.** Who spoke when, for how long, who
//!   interrupted whom, the rhythm of the exchange. That is preserved on
//!   purpose — it is what makes the result worth listening to — and it is
//!   information about the conversation.
//! * **Names are whatever you type.** A subtitle saying "Alex" contains the
//!   string "Alex". The audio is veiled; a caption is not, and this crate
//!   cannot veil a name for you.
//! * **The voiceprints do not survive.** Each speaker is destroyed as
//!   thoroughly as in single-speaker mode.
//!
//! ## VeilVoice does not decide who is talking
//!
//! Working that out from audio alone is speaker diarisation and needs a trained
//! model. There is no model here, there is no server to ask, and guessing would
//! be worse than not offering it: a wrong guess either merges two people or
//! invents a third, and neither would be visible in the output. So the plan
//! comes from the user — a channel per person, or a list of turns. See
//! [`plan`].
//!
//! ## The modules
//!
//! | Module | What it owns |
//! |---|---|
//! | [`plan`] | Who is in the recording, when they speak, and the text format |
//! | [`render`] | One engine per speaker, spliced back onto the timeline |
//! | [`subtitles`] | WebVTT and SubRip, from the same plan |
//!
//! # In plain words
//!
//! This is for a recording with more than one person in it.
//!
//! Given a note of who speaks when, it gives each person a different voice --
//! every one of them just as thoroughly disguised as a single speaker would be --
//! and writes subtitles saying who said what.
//!
//! It will not guess who is talking. Working that out needs a trained model, and
//! this project ships none, so it is told: either one microphone per person, or a
//! list of turns. Any part of the recording nobody claims is silenced rather than
//! passed through, because audio nobody claimed has not been disguised.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod plan;
pub mod render;
pub mod subtitles;

pub use plan::{Conversation, Speaker, Turn};

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What this crate does to a recording, in the words a front end should show.
///
/// Single-sourced and asserted by the tests, exactly as every other scope note
/// in this project is, so it cannot quietly turn into a promise.
pub const SCOPE: &str =
    "Each speaker is given a different destination voice, and each voiceprint is \
     destroyed just as thoroughly as it would be on its own. What a conversation keeps \
     is the shape of the conversation: how many people were talking, who spoke when, and \
     for how long. That is kept on purpose, because it is what makes the result worth \
     listening to, and it is information about the conversation. Names in subtitles are \
     whatever you type -- the audio is veiled and a caption is not. VeilVoice cannot work \
     out who is speaking; you tell it, with one microphone each or a list of turns.";

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    Io(std::io::Error),
    /// A plan is not in a form this build understands.
    Malformed(String),
    /// More speakers than there are distinct voices to give them.
    ///
    /// Refused rather than wrapped: two people sharing one output voice is
    /// exactly the failure this crate exists to prevent, and it would be
    /// invisible in the result.
    TooManySpeakers(usize),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "input/output error: {error}"),
            Self::Malformed(what) => write!(f, "malformed plan: {what}"),
            Self::TooManySpeakers(most) => write!(
                f,
                "this build has {most} distinct voices, so it can carry {most} speakers. \
                 An eleventh would have to share a voice with somebody, and nothing in \
                 the output would show that it had happened."
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The claim must keep stating what a conversation costs. If somebody edits
    /// this into a promise, this is what stops it shipping.
    #[test]
    fn the_scope_note_states_what_is_kept_as_well_as_what_is_destroyed() {
        let scope = SCOPE.to_lowercase();
        assert!(scope.contains("destroyed just as thoroughly"));
        assert!(
            scope.contains("who spoke when"),
            "the turn structure survives and the note must say so"
        );
        assert!(
            scope.contains("a caption is not"),
            "a name typed into a subtitle is not veiled and the note must say so"
        );
        assert!(scope.contains("cannot work out who is speaking"));
        for boast in ["anonymous", "untraceable", "guarantee", "impossible to"] {
            assert!(!scope.contains(boast), "overclaim: {boast}");
        }
    }

    #[test]
    fn too_many_speakers_explains_why_it_is_refused() {
        let error = Error::TooManySpeakers(10);
        let text = error.to_string();
        assert!(text.contains("10 distinct voices"), "{text}");
        assert!(
            text.contains("nothing in the output would show"),
            "the invisibility of the failure is the reason: {text}"
        );
    }

    #[test]
    fn an_io_error_displays_and_keeps_its_source() {
        let error = Error::from(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert!(error.to_string().contains("gone"));
        assert!(std::error::Error::source(&error).is_some());
        assert!(Error::Malformed("x".into()).to_string().contains("x"));
    }
}
