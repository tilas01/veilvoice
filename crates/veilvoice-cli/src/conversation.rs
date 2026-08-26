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
//! out.veiled.html  with `--page`: a player that needs nothing installed
//! ```
//!
//! # The page, and why it is not a video
//!
//! `--page` writes a self-contained HTML player: the waveform, a circle per
//! speaker that lights when they speak, and the subtitles. It references the
//! audio and the WebVTT track by relative name rather than embedding them, so
//! the files move together and the page does not double the size of a recording
//! already sitting beside it.
//!
//! A *video* file needs an encoder, and this project ships no codec. `preview`
//! prints the `ffmpeg` command that would make one, and says whether `ffmpeg`
//! is on this machine -- it never runs it. Nothing here silently depends on a
//! program the user did not know they were running.
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
use veilvoice_conversation::mode::VoiceMode;
use veilvoice_conversation::render::{self, Settings};
use veilvoice_conversation::subtitles::{self, Format};
use veilvoice_conversation::Conversation;
use veilvoice_core::DeidConfig;
use veilvoice_video::page::{Background, Look};
use veilvoice_video::{ffmpeg, page, waveform};

/// How many columns of waveform to reduce a recording to.
///
/// One per two pixels of a 1280-wide picture, which is finer than any display
/// resolves and coarse enough that the path stays a few kilobytes. The page
/// scales it to whatever width was asked for.
const WAVE_COLUMNS: usize = 640;

/// Turn the picture flags into a [`Look`], or explain why they do not describe
/// a picture that can be drawn.
///
/// `checked()` is the crate's own refusal, and it refuses rather than clamps:
/// somebody who asked for a 200-pixel render of nine speakers meant something,
/// and quietly drawing an illegible one answers a question they did not ask.
pub fn look_from(
    width: u32,
    height: u32,
    padding: u32,
    background: Option<String>,
    black: bool,
    theme: Option<String>,
) -> Result<Look, String> {
    let mut look = Look {
        width,
        height,
        padding,
        ..Look::default()
    };

    // Resolved here, where the user typed it, so that every drawing after this
    // is looking at colours that exist. An unknown name is refused and the
    // refusal lists what it could have been: a picture silently drawn in a
    // different scheme than the one asked for is worse than an error.
    if let Some(theme) = theme {
        let palette = veilvoice_video::palette::by_id(&theme).ok_or_else(|| {
            format!(
                "{theme:?} is not a theme. It is one of: {}",
                veilvoice_video::palette::ids().join(", ")
            )
        })?;
        look = look.themed(palette);
    }
    if let Some(background) = background {
        // A value that names a file that exists is an image; anything else is
        // read as a colour, and `checked()` rejects it if it is not one. Tried
        // in that order so `--background red.png` cannot be mistaken for a
        // colour called "red.png" on a machine where the file is missing.
        let as_path = PathBuf::from(&background);
        look.background = if as_path.is_file() {
            Background::Image(as_path)
        } else {
            Background::Colour(background)
        };
    }
    if black {
        look = look.black();
    }
    look.checked().map_err(|error| error.to_string())?;
    Ok(look)
}

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
///
/// `look` is `Some` when `--page` was asked for, and it decides what the page
/// looks like. `None` writes the audio and the subtitles and nothing else.
pub fn run(
    plan_path: &Path,
    input: &Path,
    output: Option<PathBuf>,
    tuning: DeidConfig,
    look: Option<Look>,
    one_voice: bool,
) -> Result<(), String> {
    let mut plan = load_plan(plan_path)?;
    if plan.is_empty() {
        return Err("the plan names no speakers, so there is nothing to render into".into());
    }

    // Set before anything is read or rendered. The refusal here is the useful
    // one -- it names the alternative -- and it costs nothing to hit it before
    // a long file has been decoded.
    let mode = if one_voice {
        VoiceMode::Uniform
    } else {
        VoiceMode::Distinct
    };
    plan.set_mode(mode, &tuning)
        .map_err(|error| error.to_string())?;

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
    println!("{}", field("voices", mode.label()));
    if mode == VoiceMode::Uniform {
        println!();
        for line in crate::sentry::wrap(mode.note(), 72) {
            println!("  {line}");
        }
    }
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

    // The page is written from the *veiled* audio, so its waveform is the
    // waveform of what somebody will actually hear. Drawing the input's would
    // put a picture of the original signal beside a file whose whole point is
    // that the original is gone.
    let mut page_path = None;
    if let Some(look) = &look {
        let envelope = waveform::envelope(&veiled.samples, WAVE_COLUMNS);
        let drawn = page::player(
            &plan,
            &envelope,
            look,
            &file_name(&out_path),
            &file_name(&vtt),
        )
        .map_err(|error| error.to_string())?;
        let html = with_extension(&out_path, "html");
        std::fs::write(&html, drawn.markup)
            .map_err(|error| format!("{}: {error}", html.display()))?;
        for note in &drawn.notes {
            println!("{}", warn(note));
        }
        page_path = Some(html);
    }

    println!();
    println!("{}", ok(&format!("wrote {}", out_path.display())));
    println!("{}", ok(&format!("wrote {}", vtt.display())));
    println!("{}", ok(&format!("wrote {}", srt.display())));
    if let Some(html) = &page_path {
        println!("{}", ok(&format!("wrote {}", html.display())));
        println!("  The page reads the audio and the WebVTT track by name from the same");
        println!("  directory. Move all of them, or none.");
    }
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

/// A still of what the page will look like, and the command that would make a
/// video of it.
///
/// Nothing is rendered and nothing is encoded. This exists so that the answer
/// to "what will I get" costs a second rather than the length of the recording.
pub fn preview(
    plan_path: &Path,
    audio: Option<PathBuf>,
    at_secs: f64,
    look: Look,
    output: Option<PathBuf>,
    show_ffmpeg: bool,
    one_voice: bool,
) -> Result<(), String> {
    let mut plan = load_plan(plan_path)?;
    if plan.is_empty() {
        return Err("the plan names no speakers, so there is nothing to draw".into());
    }
    let mode = if one_voice {
        VoiceMode::Uniform
    } else {
        VoiceMode::Distinct
    };
    plan.set_mode(mode, &DeidConfig::default())
        .map_err(|error| error.to_string())?;

    // With no audio the waveform is flat rather than absent: the picture is
    // about the layout, and an empty band is honest about there being nothing
    // measured yet. With audio it is that file's real envelope.
    let envelope = match &audio {
        Some(path) => {
            let loaded = audio_io::load(path).map_err(|error| error.to_string())?;
            waveform::envelope(&loaded.samples, WAVE_COLUMNS)
        }
        None => waveform::envelope(&vec![0.0; WAVE_COLUMNS], WAVE_COLUMNS),
    };

    let drawn = page::still(&plan, &envelope, &look, at_secs).map_err(|e| e.to_string())?;
    let out_path = output.unwrap_or_else(|| with_extension(plan_path, "preview.svg"));
    std::fs::write(&out_path, drawn.markup)
        .map_err(|error| format!("{}: {error}", out_path.display()))?;

    println!("{}", heading("Preview"));
    println!("{}", field("plan", &plan_path.display().to_string()));
    println!("{}", field("speakers", &plan.len().to_string()));
    println!(
        "{}",
        field("picture", &format!("{}x{}", look.width, look.height))
    );
    println!("{}", field("theme", look.palette.name));
    println!("{}", field("at", &format!("{at_secs:.2} s")));
    println!("{}", field("voices", mode.label()));
    if audio.is_none() {
        println!("{}", field("waveform", "flat -- no recording was given"));
    }
    for note in &drawn.notes {
        println!("{}", warn(note));
    }
    println!();
    println!("{}", ok(&format!("wrote {}", out_path.display())));

    println!();
    println!("{}", heading("Which voice each speaker becomes"));
    if mode == VoiceMode::Uniform {
        for line in crate::sentry::wrap(mode.note(), 72) {
            println!("  {line}");
        }
        println!();
    }
    for (index, speaker) in plan.speakers().iter().enumerate() {
        println!(
            "{}",
            field(
                &speaker.name,
                &format!(
                    "{} -- {}",
                    plan.voice(index).describe(),
                    veilvoice_video::palette::speaker(index)
                )
            )
        );
    }

    if show_ffmpeg {
        println!();
        println!("{}", heading("Making a video of it"));
        let argv = ffmpeg::command(
            Path::new("frames"),
            "frame-%05d.png",
            Path::new("out.veiled.wav"),
            Path::new("out.mp4"),
            ffmpeg::Encoding::default(),
        );
        println!("  {}", ffmpeg::command_line(&argv));
        println!();
        match ffmpeg::found() {
            Some(path) => println!("{}", ok(&format!("ffmpeg is here: {}", path.display()))),
            None => println!(
                "{}",
                warn("ffmpeg was not found on PATH. The page above needs nothing installed.")
            ),
        }
        println!();
        for line in crate::sentry::wrap(&ffmpeg::describe(), 72) {
            println!("  {line}");
        }
        println!();
        println!(
            "{}",
            warn("VeilVoice never runs this for you. Copy it, read it, and run it yourself.")
        );
    }
    Ok(())
}

/// The last component of a path, for writing into a page as a relative link.
///
/// A page that referenced the absolute path would break the moment the three
/// files were moved together, which is the one thing they are meant to survive.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
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

        run(
            &plan,
            &input,
            Some(output.clone()),
            DeidConfig::default(),
            None,
            false,
        )
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
        run(&plan, &input, None, DeidConfig::default(), None, false).expect("should succeed");
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
            None,
            false,
        )
        .expect_err("an empty plan is refused");
        assert!(error.contains("no speakers"), "{error}");
    }

    /// `--page` writes a fourth file, and it points at the other three by name.
    #[test]
    fn a_page_is_written_beside_the_audio_and_links_to_it_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_file(dir.path());
        let input = wav_file(dir.path(), 1.0);
        run(
            &plan,
            &input,
            None,
            DeidConfig::default(),
            Some(Look::default()),
            false,
        )
        .expect("should succeed");

        let html = dir.path().join("input.veiled.html");
        let markup = std::fs::read_to_string(&html).expect("the page should be written");
        // Relative names, so the four files move together. An absolute path
        // here would break the moment somebody copied the directory, which is
        // the one thing this arrangement exists to survive.
        assert!(markup.contains("input.veiled.wav"), "{markup:.400}");
        assert!(markup.contains("input.veiled.vtt"), "{markup:.400}");
        assert!(
            !markup.contains(&dir.path().display().to_string()),
            "the page must not carry an absolute path"
        );
    }

    /// No `--page`, no page. The default is three files, as it always was.
    #[test]
    fn without_the_flag_no_page_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_file(dir.path());
        let input = wav_file(dir.path(), 1.0);
        run(&plan, &input, None, DeidConfig::default(), None, false).expect("should succeed");
        assert!(!dir.path().join("input.veiled.html").exists());
    }

    /// A picture nobody could read is refused rather than drawn.
    #[test]
    fn a_look_that_cannot_be_drawn_is_refused_with_the_numbers_in_it() {
        let error = look_from(200, 100, 8, None, false, None).expect_err("too small to draw");
        assert!(error.contains("200x100"), "{error}");
    }

    /// `--background` takes a colour or a file, and says so when it is neither.
    #[test]
    fn a_background_that_is_neither_a_colour_nor_a_file_is_named_in_the_error() {
        let error = look_from(1280, 720, 48, Some("chartreuse".into()), false, None)
            .expect_err("not a colour");
        assert!(error.contains("chartreuse"), "{error}");
    }

    /// `--black` wins over `--background`, as its help says.
    #[test]
    fn black_overrides_a_background_colour() {
        let look = look_from(1280, 720, 48, Some("#ff0000".into()), true, None).unwrap();
        assert_eq!(
            look.background,
            Background::Colour("#000000".to_string()),
            "--black must override --background"
        );
    }

    /// The default is Tokyo Night, and it is what a picture with no `--theme`
    /// is drawn in.
    #[test]
    fn no_theme_means_tokyo_night() {
        let look = look_from(1280, 720, 48, None, false, None).unwrap();
        assert_eq!(look.palette.id, "tokyo-night");
    }

    /// A named theme reaches the drawing, and takes the background with it.
    /// A Gruvbox picture on a Tokyo Night page is not what anybody asked for.
    #[test]
    fn a_named_theme_takes_the_background_with_it() {
        let look = look_from(1280, 720, 48, None, false, Some("gruvbox".into())).unwrap();
        assert_eq!(look.palette.id, "gruvbox");
        assert_eq!(
            look.background,
            Background::Colour(look.palette.bg.to_string()),
            "the page should follow the theme"
        );
    }

    /// Unless the background was asked for separately, in which case both
    /// requests are honoured.
    #[test]
    fn a_background_given_by_hand_survives_a_theme() {
        let look = look_from(
            1280,
            720,
            48,
            Some("#123456".into()),
            false,
            Some("nord".into()),
        )
        .unwrap();
        assert_eq!(look.palette.id, "nord");
        assert_eq!(look.background, Background::Colour("#123456".to_string()));
    }

    /// An unknown name is refused, and the refusal says what it could have
    /// been. An error that only says "no" leaves the reader guessing at a
    /// spelling.
    #[test]
    fn an_unknown_theme_is_refused_and_lists_the_real_ones() {
        let error = look_from(1280, 720, 48, None, false, Some("solarised".into()))
            .expect_err("not a theme this build has");
        assert!(error.contains("solarised"), "{error}");
        assert!(error.contains("tokyo-night"), "{error}");
        assert!(error.contains("gruvbox"), "{error}");
    }

    /// The colours actually reach the markup. Without this the flag could be
    /// plumbed all the way through and change nothing anybody can see.
    #[test]
    fn the_theme_changes_the_colours_in_the_drawing() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_file(dir.path());
        let gruvbox = dir.path().join("gruvbox.svg");
        preview(
            &plan,
            None,
            0.0,
            look_from(1280, 720, 48, None, false, Some("gruvbox".into())).unwrap(),
            Some(gruvbox.clone()),
            false,
            false,
        )
        .unwrap();
        let markup = std::fs::read_to_string(&gruvbox).unwrap();
        let wanted = veilvoice_video::palette::by_id("gruvbox").unwrap();
        assert!(
            markup.contains(wanted.bg),
            "the page should be {}",
            wanted.bg
        );
        assert!(
            !markup.contains(veilvoice_video::palette::BG),
            "no Tokyo Night background should survive a Gruvbox render"
        );
    }

    /// A preview needs no recording, and writes a still that is real SVG.
    #[test]
    fn a_preview_draws_without_any_audio_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let plan = plan_file(dir.path());
        let out = dir.path().join("still.svg");
        preview(
            &plan,
            None,
            0.25,
            Look::default(),
            Some(out.clone()),
            false,
            false,
        )
        .expect("a preview needs no audio");
        let markup = std::fs::read_to_string(&out).unwrap();
        assert!(markup.starts_with("<svg"), "{markup:.120}");
        assert!(markup.contains("Alex"), "the speakers should be named");
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
