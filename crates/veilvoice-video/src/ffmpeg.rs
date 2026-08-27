// SPDX-License-Identifier: GPL-3.0-or-later
//! The video file, which needs a codec this project does not ship.
//!
//! # Why there is no encoder here
//!
//! A conversation an hour long is about 108,000 frames at 30 per second. Turning
//! those into something a phone will play means H.264 or AV1, and writing
//! either is not a sensible thing for a voice de-identifier to do. Pulling one
//! in is worse: every usable encoder is a large C library, and this project's
//! dependency graph containing no such thing is a claim on its front page that
//! a reader can check with `cargo tree` in ten seconds.
//!
//! So the honest arrangement is the one the rest of the project already uses
//! for exactly this kind of problem — the download in `veilvoice-verify`, the
//! registry in `veilvoice-watch`, the driver list in `veilvoice-drivers`. Find
//! the tool the machine already has, prepare the exact command, and let the
//! person decide.
//!
//! # And VeilVoice will not run it for you
//!
//! [`command`] builds the argument list. Running it is the caller's, and a
//! front end should print it rather than execute it silently — the same rule
//! the companion installer follows, for the same reason.
//!
//! # If the machine has no ffmpeg, nothing has failed
//!
//! [`crate::page::player`] has already written a file that plays everywhere
//! and needs nothing installed. The video file is the extra, not the product.
//!
//! # In plain words
//!
//! This works out the command that would turn the pictures and the veiled audio
//! into a video file, and prints it for you to run.
//!
//! It does not run it, and VeilVoice does not contain a video encoder. Every
//! usable one is a large piece of C code, and adding one would mean this project
//! no longer being something you can read the whole of. So it writes out the
//! command for `ffmpeg`, which many people already have, and leaves running it to
//! you.

use std::path::{Path, PathBuf};

/// Where `ffmpeg` is, if this machine has one.
///
/// Looks along `PATH` rather than spawning `which` or `where`: the answer is a
/// string this process already holds, and spawning a program to ask where a
/// program is costs a subprocess to learn nothing new.
pub fn found() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["ffmpeg.exe"]
    } else {
        &["ffmpeg"]
    };
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for name in names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// How to render the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encoding {
    /// Frames per second.
    pub fps: u32,
    /// Constant rate factor: lower is better quality and a larger file.
    pub crf: u32,
    /// The video encoder to ask `ffmpeg` for.
    ///
    /// `None` means `libx264`, which is the software encoder and is always
    /// there. A hardware encoder goes here by its `ffmpeg` name, such as
    /// `h264_nvenc`; `veilvoice_accel` is what finds out which this machine
    /// has, and it is honest that finding a device is not proof it works.
    ///
    /// This changes **how long the video takes to write, and nothing else**.
    /// The audio is veiled by the same engine either way and the picture is
    /// drawn by the same code.
    pub encoder: Option<String>,
}

impl Default for Encoding {
    fn default() -> Self {
        Self {
            // Thirty is enough for a waveform and a circle that lights up.
            // Sixty would double the file for motion that is not there.
            fps: 30,
            // Visually lossless for flat colour and text, which is all this is.
            crf: 20,
            // The software encoder, which every copy of ffmpeg has. Choosing
            // hardware is a decision somebody makes, not a default that
            // silently depends on the machine: two people rendering the same
            // recording should get the same file unless one of them asked not
            // to.
            encoder: None,
        }
    }
}

/// The command that turns a directory of numbered frames and a WAV into a
/// video file.
///
/// Returned as an argument list rather than a shell string, because a shell
/// string has quoting rules and a path with a space in it is the ordinary case
/// on two of the three platforms here.
///
/// `frames` is a printf-style pattern such as `frame-%05d.png`.
pub fn command(
    frames: &Path,
    pattern: &str,
    audio: &Path,
    output: &Path,
    encoding: Encoding,
) -> Vec<String> {
    let input = frames.join(pattern);
    vec![
        "ffmpeg".to_string(),
        // Never overwrite without being asked. `-n` fails instead, which is the
        // right way round for a tool a person is running by hand over a
        // directory they may have put something else in.
        "-n".to_string(),
        "-framerate".to_string(),
        encoding.fps.to_string(),
        "-i".to_string(),
        input.display().to_string(),
        "-i".to_string(),
        audio.display().to_string(),
        "-c:v".to_string(),
        encoding
            .encoder
            .clone()
            .unwrap_or_else(|| "libx264".to_string()),
        // `-crf` is libx264's control. The hardware encoders do not have it and
        // use `-cq` instead, so asking for the wrong one is not a slower
        // render, it is ffmpeg refusing to start.
        if encoding.encoder.is_some() {
            "-cq".to_string()
        } else {
            "-crf".to_string()
        },
        encoding.crf.to_string(),
        // The pixel format every player and every phone accepts. Without it
        // ffmpeg picks yuv444p for RGB input, which a great many devices
        // silently refuse to play -- and "it produces a file nothing opens" is
        // a worse failure than an error.
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "192k".to_string(),
        // Stop at whichever of the two runs out first, so a rounding error in
        // the frame count cannot leave a second of frozen picture on the end.
        "-shortest".to_string(),
        output.display().to_string(),
    ]
}

/// The command as one line, for printing.
///
/// Quoted where a part contains a space. For a person to read and paste, not
/// for a shell this program runs — nothing here runs it.
pub fn command_line(argv: &[String]) -> String {
    argv.iter()
        .map(|part| {
            if part.contains(' ') {
                format!("\"{part}\"")
            } else {
                part.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What to tell the user about their machine's `ffmpeg`.
pub fn describe() -> String {
    match found() {
        Some(path) => format!(
            "ffmpeg is on this machine, at {}. VeilVoice will not run it for you; the \
             command is printed so you can see what it does before it does it.",
            path.display()
        ),
        None => "ffmpeg is not on this machine, so no video file can be written. Nothing \
                 has failed: the page VeilVoice wrote plays everywhere and needs nothing \
                 installed. If you want a video file, install ffmpeg yourself -- VeilVoice \
                 will not download or install it."
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv() -> Vec<String> {
        command(
            Path::new("/tmp/frames"),
            "frame-%05d.png",
            Path::new("/tmp/out.wav"),
            Path::new("/tmp/out.mp4"),
            Encoding::default(),
        )
    }

    #[test]
    fn the_command_names_both_inputs_and_the_output() {
        let argv = argv();
        assert_eq!(argv[0], "ffmpeg");
        let line = command_line(&argv);
        assert!(line.contains("frame-%05d.png"), "{line}");
        assert!(line.contains("out.wav"), "{line}");
        assert!(line.ends_with("out.mp4"), "{line}");
    }

    /// Without this a great many phones silently refuse to play the result,
    /// which is a worse failure than an error.
    #[test]
    fn the_pixel_format_is_the_one_every_device_accepts() {
        let argv = argv();
        let at = argv.iter().position(|part| part == "-pix_fmt").unwrap();
        assert_eq!(argv[at + 1], "yuv420p");
    }

    /// A tool a person runs by hand over a directory they may have put
    /// something else in must not overwrite without being asked.
    #[test]
    fn the_command_refuses_to_overwrite() {
        assert!(argv().contains(&"-n".to_string()));
        assert!(!argv().contains(&"-y".to_string()));
    }

    /// A rounding error in the frame count must not leave frozen picture on
    /// the end.
    #[test]
    fn the_command_stops_at_the_shorter_stream() {
        assert!(argv().contains(&"-shortest".to_string()));
    }

    #[test]
    fn the_frame_rate_and_quality_are_the_documented_defaults() {
        let encoding = Encoding::default();
        assert_eq!(encoding.fps, 30);
        assert_eq!(encoding.crf, 20);
        let argv = argv();
        let at = argv.iter().position(|part| part == "-framerate").unwrap();
        assert_eq!(argv[at + 1], "30");
        let at = argv.iter().position(|part| part == "-crf").unwrap();
        assert_eq!(argv[at + 1], "20");
    }

    /// A path with a space in it is the ordinary case on two of the three
    /// platforms here, so the printed line has to survive one.
    ///
    /// The separator is whatever `Path::join` produced, which differs by
    /// platform -- so the test checks the **quoting**, which is the thing this
    /// function is responsible for, rather than a path spelling it is not.
    #[test]
    fn a_path_with_a_space_is_quoted_when_printed() {
        let argv = command(
            Path::new("/home/somebody/My Videos"),
            "f-%04d.png",
            Path::new("/home/somebody/My Videos/out.wav"),
            Path::new("/home/somebody/My Videos/out.mp4"),
            Encoding::default(),
        );
        for part in &argv {
            if part.contains(' ') {
                let quoted = format!("\"{part}\"");
                assert!(
                    command_line(&argv).contains(&quoted),
                    "{part:?} was not quoted"
                );
            }
        }
        // And nothing without a space is quoted, or the line is unreadable.
        let line = command_line(&argv);
        assert!(line.starts_with("ffmpeg -n -framerate 30"), "{line}");
        assert!(!line.contains("\"ffmpeg\""), "{line}");
    }

    /// Looking for ffmpeg must not panic whatever the machine has, and must
    /// give the same answer twice.
    #[test]
    fn looking_for_ffmpeg_is_free_of_side_effects() {
        assert_eq!(found(), found());
        let described = describe();
        assert!(!described.is_empty());
        if found().is_none() {
            assert!(described.contains("Nothing has failed"), "{described}");
            assert!(
                described.contains("will not download or install it"),
                "{described}"
            );
        } else {
            assert!(described.contains("will not run it for you"), "{described}");
        }
    }

    /// Whatever the machine has, the message must not promise to fetch it.
    #[test]
    fn the_description_never_offers_to_install_anything() {
        let described = describe().to_lowercase();
        for offer in [
            "downloading ffmpeg",
            "installing it for you",
            "we will install",
        ] {
            assert!(!described.contains(offer), "{described}");
        }
    }
}
