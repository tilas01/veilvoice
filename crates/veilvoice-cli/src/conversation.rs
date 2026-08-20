// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice conversation` -- several speakers, a voice each, and subtitles.
//!
//! The command-line front end to [`veilvoice_conversation`]. That crate holds
//! the plan, the renderer and the honest account of what a conversation keeps;
//! this file reads the audio, writes the results and prints what happened.
//!
//! # What comes out
//!
//! Three files beside the output you name:
//!
//! ```text
//! out.veiled.wav   the audio, one destination voice per speaker
//! out.veiled.vtt   subtitles for a browser
//! out.veiled.srt   subtitles for everything else
//! ```
//!
//! The subtitles are written whether or not anybody wrote down the words. With
//! every voice replaced, a caption track saying *who* is talking is often the
//! only way to follow a recording at all.
//!
//! # The one warning this command will not let you miss
//!
//! Audio no turn claims is **silenced**, never passed through. A gap in the
//! plan is a span nobody assigned to a speaker, so it has not been veiled, and
//! putting it into the result would place a real voice inside a file whose
//! whole purpose is that it contains none. The amount is printed, loudly,
//! because a plan with a hole in it is something to fix rather than something
//! to discover later.
//!
//! # This does not encrypt what it writes
//!
//! Unlike `anonymise`, which seals its output at rest by default. A conversation
//! render produces a set of files -- audio and two subtitle tracks -- and the
//! container this project uses seals one thing. Rather than invent a
//! half-answer, the files are written in the clear and the command says so:
//! `veilvoice encrypt` seals the audio afterwards, and the subtitles hold
//! whatever names were typed and are not veiled by anything.

use crate::theme::{colour, err, field, heading, ok, paint, warn};
use std::path::{Path, PathBuf};
use veilvoice_audio::io as audio_io;
use veilvoice_conversation::render::{self, Settings};
use veilvoice_conversation::subtitles::{self, Format};
use veilvoice_conversation::Conversation;
use veilvoice_core::DeidConfig;

/// Show a plan without rendering anything.
pub fn inspect(plan_path: &Path) -> Result<(), String> {
    let plan = load_plan(plan_path)?;
    println!("{}", heading("Conversation"));
    if let Some(title) = &plan.title {
        println!("{}", field("title", title));
    }
    println!("{}", field("speakers", &plan.len().to_string()));
    println!("{}", field("turns", &plan.turns().len().to_string()));
    println!("{}", field("length", &format!("{:.2} s", plan.duration())));
    println!();

    for (index, speaker) in plan.speakers().iter().enumerate() {
        let voice = plan.voice(index);
        println!("{}", paint(colour::BLUE, &speaker.name));
        println!("{}", field("voice", &voice.describe()));
        let spoken: f64 = plan
            .turns()
            .iter()
            .filter(|turn| turn.speaker == index)
            .map(|turn| turn.duration())
            .sum();
        println!("{}", field("speaks for", &format!("{spoken:.2} s")));
        if let Some(picture) = &speaker.picture {
            println!("{}", field("picture", &picture.display().to_string()));
        }
        println!();
    }

    let overlaps = plan.overlaps();
    if !overlaps.is_empty() {
        println!(
            "{}",
            field(
                "interruptions",
                &format!("{} -- these will be mixed, not resolved", overlaps.len())
            )
        );
    }
    let self_overlaps = plan.self_overlaps();
    if !self_overlaps.is_empty() {
        println!(
            "{}",
            warn(&format!(
                "{} place(s) where one speaker overlaps themselves. One person cannot \
                 be in two places in their own recording, so this is usually a typed \
                 time.",
                self_overlaps.len()
            ))
        );
    }

    println!();
    println!("{}", paint(colour::YELLOW, "WHAT A CONVERSATION KEEPS"));
    for line in crate::sentry::wrap(veilvoice_conversation::SCOPE, 72) {
        println!("  {line}");
    }
    Ok(())
}

/// Render a recording according to a plan.
pub fn run(
    plan_path: &Path,
    input: &Path,
    output: Option<PathBuf>,
    tuning: DeidConfig,
) -> Result<(), String> {
    let plan = load_plan(plan_path)?;
    if plan.is_empty() {
        return Err("the plan names no speakers, so there is nothing to render into".into());
    }

    let out_path = output.unwrap_or_else(|| {
        let mut path = input.to_path_buf();
        path.set_extension("veiled.wav");
        path
    });

    let audio = audio_io::load(input).map_err(|error| error.to_string())?;
    println!("{}", heading("Input"));
    println!("{}", field("file", &input.display().to_string()));
    println!(
        "{}",
        field("duration", &format!("{:.2} s", audio.duration_secs()))
    );
    println!(
        "{}",
        field("sample rate", &format!("{} Hz", audio.sample_rate))
    );
    println!("{}", field("speakers", &plan.len().to_string()));
    println!();

    // The engine settings come from the same flags `anonymise` uses, with the
    // sample rate taken from the file rather than from a default that would
    // silently resample nothing and mistime everything.
    let mut settings = Settings {
        config: tuning,
        ..Settings::default()
    };
    settings.config.sample_rate = audio.sample_rate as f32;

    let started = std::time::Instant::now();
    let rendered = render::render(&plan, &audio.samples, &settings, None)
        .map_err(|error| error.to_string())?;
    let elapsed = started.elapsed().as_secs_f32();

    println!("{}", heading("Rendered"));
    for (index, speaker) in plan.speakers().iter().enumerate() {
        println!(
            "{}",
            field(
                &speaker.name,
                &format!(
                    "{:.2} s as {}",
                    rendered.per_speaker_secs[index],
                    plan.voice(index).describe()
                )
            )
        );
    }
    println!("{}", field("took", &format!("{elapsed:.2} s")));

    // Printed at error weight rather than as a note. A gap in the plan is the
    // one mistake here that costs content, and it is invisible in a waveform
    // somebody skims.
    if rendered.has_unassigned() {
        println!();
        println!(
            "{}",
            err(&format!(
                "{:.2} s of the recording is not claimed by any turn",
                rendered.unassigned_secs
            ))
        );
        println!("  That audio was SILENCED, not passed through: nobody assigned it to");
        println!("  a speaker, so it has not been veiled, and putting it into the");
        println!("  result would place a real voice in a file whose whole purpose is");
        println!("  that it contains none. Add a turn covering it and render again.");
    }
    for note in &rendered.notes {
        // The unassigned note is already printed above, in stronger words.
        if note.contains("was silenced") {
            continue;
        }
        println!("{}", warn(note));
    }

    let veiled = veilvoice_audio::io::Audio {
        samples: rendered.samples,
        sample_rate: audio.sample_rate,
    };
    audio_io::save_wav(&out_path, &veiled).map_err(|error| error.to_string())?;

    let vtt = with_extension(&out_path, Format::WebVtt.extension());
    let srt = with_extension(&out_path, Format::SubRip.extension());
    std::fs::write(&vtt, subtitles::write(&plan, Format::WebVtt))
        .map_err(|error| format!("{}: {error}", vtt.display()))?;
    std::fs::write(&srt, subtitles::write(&plan, Format::SubRip))
        .map_err(|error| format!("{}: {error}", srt.display()))?;

    println!();
    println!("{}", ok(&format!("wrote {}", out_path.display())));
    println!("{}", ok(&format!("wrote {}", vtt.display())));
    println!("{}", ok(&format!("wrote {}", srt.display())));
    println!();
    println!(
        "{}",
        warn(
            "These files are not encrypted. `veilvoice encrypt` seals the audio. The \
             subtitles hold whatever names you typed, and nothing veils a name."
        )
    );
    Ok(())
}

/// Read a plan, and say where it went wrong rather than only that it did.
fn load_plan(path: &Path) -> Result<Conversation, String> {
    Conversation::load(path).map_err(|error| format!("{}: {error}", path.display()))
}

/// Replace the last extension, keeping any `.veiled` before it.
fn with_extension(path: &Path, extension: &str) -> PathBuf {
    let mut out = path.to_path_buf();
    out.set_extension(extension);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use veilvoice_conversation::{Speaker, Turn};

    fn plan_file(dir: &Path) -> PathBuf {
        let mut plan = Conversation::new();
        plan.title = Some("Two people".into());
        plan.add_speaker(Speaker::named("Alex")).unwrap();
        plan.add_speaker(Speaker::named("Sam")).unwrap();
        plan.add_turn(Turn {
            start: 0.0,
            end: 0.5,
            speaker: 0,
            text: Some("Hello".into()),
        })
        .unwrap();
        plan.add_turn(Turn {
            start: 0.5,
            end: 1.0,
            speaker: 1,
            text: None,
        })
        .unwrap();
        let path = dir.join("plan.txt");
        plan.save(&path).unwrap();
        path
    }

    fn wav_file(dir: &Path, seconds: f32) -> PathBuf {
        let rate = 48_000u32;
        let len = (seconds * rate as f32) as usize;
        let samples: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / rate as f32;
                0.3 * (2.0 * std::f32::consts::PI * 150.0 * t).sin()
            })
            .collect();
        let audio = veilvoice_audio::io::Audio {
            samples,
            sample_rate: rate,
        };
        let path = dir.join("input.wav");
        audio_io::save_wav(&path, &audio).unwrap();
        path
    }

    #[test]
    fn inspecting_a_plan_needs_no_audio() {
        let dir = tempfile::tempdir().unwrap();
        inspect(&plan_file(dir.path())).expect("a plan alone is enough to describe");
    }

    #[test]
    fn a_missing_plan_says_which_file() {
        let error = inspect(Path::new("definitely-not-here.txt")).expect_err("no such file");
        assert!(error.contains("definitely-not-here.txt"), "{error}");
    }

    /// The whole command, end to end: audio in, audio and two subtitle tracks
    /// out, and the output the same length as the input.
    #[test]
    fn rendering_writes_the_audio_and_both_subtitle_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_file(dir.path());
        let input = wav_file(dir.path(), 1.0);
        let output = dir.path().join("out.wav");

        run(&plan, &input, Some(output.clone()), DeidConfig::default())
            .expect("the render should succeed");

        assert!(output.exists(), "no audio was written");
        let vtt = dir.path().join("out.vtt");
        let srt = dir.path().join("out.srt");
        assert!(vtt.exists(), "no WebVTT was written");
        assert!(srt.exists(), "no SubRip was written");

        let written = std::fs::read_to_string(&vtt).unwrap();
        assert!(written.starts_with("WEBVTT"), "{written}");
        assert!(written.contains("Alex: Hello"), "{written}");
        assert!(
            written.contains("Sam"),
            "a turn with no words still names its speaker: {written}"
        );

        let veiled = audio_io::load(&output).unwrap();
        let original = audio_io::load(&input).unwrap();
        assert_eq!(veiled.sample_rate, original.sample_rate);
        assert_eq!(veiled.samples.len(), original.samples.len());
        assert!(veiled.samples.iter().all(|s| s.is_finite()));
    }

    /// The output name is derived from the input when none is given.
    #[test]
    fn the_default_output_sits_beside_the_input() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_file(dir.path());
        let input = wav_file(dir.path(), 1.0);
        run(&plan, &input, None, DeidConfig::default()).expect("should succeed");
        assert!(dir.path().join("input.veiled.wav").exists());
    }

    #[test]
    fn a_plan_with_no_speakers_is_refused_before_the_audio_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.txt");
        Conversation::new().save(&empty).unwrap();
        let error = run(
            &empty,
            Path::new("this-file-does-not-exist.wav"),
            None,
            DeidConfig::default(),
        )
        .expect_err("an empty plan is refused");
        assert!(error.contains("no speakers"), "{error}");
    }

    #[test]
    fn the_subtitle_names_follow_the_audio_extension() {
        let path = PathBuf::from("/somewhere/out.veiled.wav");
        assert_eq!(
            with_extension(&path, "vtt"),
            PathBuf::from("/somewhere/out.veiled.vtt")
        );
    }
}
