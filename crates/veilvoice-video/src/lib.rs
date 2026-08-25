// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-video
//!
//! A watchable version of a veiled conversation: the waveform, a circle per
//! speaker, the title, the subtitles, and a background.
//!
//! ## What this produces, and what needs something else
//!
//! **A self-contained page, always.** [`page::player`] writes one HTML file
//! that plays the veiled audio, draws its waveform, lights each speaker's
//! circle as they talk and shows the subtitles. It needs nothing installed, it
//! contacts nothing, it opens in any browser on any device, and it is what this
//! crate is for.
//!
//! **A video file, only if `ffmpeg` is there.** Turning a few thousand frames
//! into an `.mp4` needs a codec, and this project ships none: writing an H.264
//! encoder is not a sensible thing for a voice de-identifier to do, and pulling
//! one in would put a large C dependency into a graph whose emptiness is a
//! front-page claim. So [`ffmpeg`] finds the tool if the machine has it and
//! prepares the exact command; if the machine does not, the page is still
//! there and nothing has failed.
//!
//! VeilVoice never downloads or runs `ffmpeg` on your behalf, exactly as it
//! never installs any other companion.
//!
//! ## The page needs a little JavaScript, and says so
//!
//! Lighting the right circle means knowing where the audio has got to, and only
//! the audio element knows that. There is a small inline script — no file, no
//! network, no library — and a `<noscript>` that says what it does. Without it
//! the audio still plays, the subtitles still appear and the waveform is still
//! drawn; the circles simply do not light up.
//!
//! ## A picture is not veiled
//!
//! A speaker may have a portrait, and it is drawn exactly as supplied. Nothing
//! here anonymises an image: a photograph of somebody's face beside their
//! veiled voice identifies them completely. The default is a plain filled
//! circle for that reason, and [`SCOPE`] says it where a user will read it.
//!
//! # In plain words
//!
//! This draws the picture.
//!
//! A waveform, a circle for each person in their own colour with their name under
//! it, a title, and a page that plays all of it together in a browser and needs
//! nothing installed.
//!
//! It does not make a video file. That needs an encoder, and this project ships
//! none -- so it prints the command that would do it with `ffmpeg`, if you have
//! `ffmpeg`, and leaves running it to you.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod ffmpeg;
pub mod page;
pub mod palette;
pub mod waveform;

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What a rendered video is worth, in the words a front end should show.
///
/// Single-sourced and asserted by the tests, exactly as every other scope note
/// in this project is, so it cannot quietly turn into a promise.
pub const SCOPE: &str =
    "The picture is drawn from the veiled audio, not from the original. A waveform carries \
     no formants and no phase, so it is not a voiceprint -- but it does show silences, \
     rhythm and loudness, which is the same turn-taking structure the audio already \
     keeps. Names and portraits are not veiled by anything: a photograph beside a veiled \
     voice identifies the person completely, which is why the default is a plain coloured \
     circle. A video file needs ffmpeg, which VeilVoice will not download or run for you; \
     the page it writes needs nothing installed.";

/// Everything that can go wrong in this crate.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A file could not be read or written.
    Io(std::io::Error),
    /// Something about the request does not make sense.
    Malformed(String),
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
            Self::Malformed(what) => write!(f, "{what}"),
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

    /// The claim must keep stating what a picture does and does not hide.
    #[test]
    fn the_scope_note_states_the_limits_rather_than_a_guarantee() {
        let scope = SCOPE.to_lowercase();
        assert!(
            scope.contains("not from the original"),
            "which audio the picture is of is the first thing to say"
        );
        assert!(scope.contains("it is not a voiceprint"));
        assert!(
            scope.contains("photograph beside a veiled voice identifies the person"),
            "the portrait trap must be stated plainly"
        );
        assert!(scope.contains("will not download or run for you"));
        for boast in ["anonymous", "guarantee", "impossible to", "untraceable"] {
            assert!(!scope.contains(boast), "overclaim: {boast}");
        }
    }

    #[test]
    fn an_io_error_displays_and_keeps_its_source() {
        let error = Error::from(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert!(error.to_string().contains("gone"));
        assert!(std::error::Error::source(&error).is_some());
        assert!(Error::Malformed("x".into()).to_string().contains("x"));
    }
}
