// SPDX-License-Identifier: GPL-3.0-or-later
//! The Browser's export, the way a recording tool does it, and its thumbnails.
//!
//! **Roadmap item 175.** The questions and the picture live in
//! `veilvoice_video::export` and `veilvoice_video::motion`, where the command
//! line can reach them too. This is the window's half: what the person has
//! chosen, the worker that renders it, and a small picture of every recording
//! in the list.
//!
//! # Nothing here runs on the thread that draws
//!
//! A render is minutes and a thumbnail is a decryption. Both are started from
//! the Browser and both finish on a worker ([`run`] and [`Thumbs::ask`]), which
//! is the rule roadmap item 167 set for the whole window and a guard in
//! `crate::draw_path_tests` holds it to.
//!
//! # The key is held for one recording at a time
//!
//! The render takes the shared vault, loads one recording and lets go of it
//! before anything else happens, exactly as the older export does (F-219). The
//! thumbnails hold a [`std::sync::Weak`] rather than the vault itself: each
//! recording is loaded through a strong reference that is dropped straight
//! after, so shutting the vault while thumbnails are being drawn stops them at
//! the next recording rather than keeping the key alive until the list ends.
//!
//! # Thumbnails are never written anywhere
//!
//! They are pictures of the veiled audio, and they are still drawn from what is
//! in the vault. They exist as textures while the vault is open and nowhere
//! else: no cache on the disk, nothing that survives [`Thumbs::clear`].
//!
//! # In plain words
//!
//! The export button in the Browser: which kind of file, how good, how it
//! looks, and then the work done in the background with a bar that counts the
//! frames. And a little picture of each recording in the list, drawn in the
//! same style the video will be.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Weak};

use veilvoice_crypto::studio::Studio as Vault;
use veilvoice_video::export::{Content, Export};
use veilvoice_video::motion::{self, Motion};

/// Width of a thumbnail, in pixels.
pub const THUMB_WIDTH: usize = 120;
/// Height of a thumbnail, in pixels.
pub const THUMB_HEIGHT: usize = 32;

/// Everything the person has chosen about an export.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// The file: what goes in it, the container, the codec, the quality, the
    /// size and the rate.
    pub export: Export,
    /// The template the look started from.
    pub template: &'static str,
    /// The look, which the advanced half changes from the template's.
    pub motion: Motion,
    /// Whether the advanced half is open.
    pub advanced: bool,
}

impl Default for Choice {
    fn default() -> Self {
        let template = veilvoice_video::palette::DEFAULT_ID;
        Self {
            export: Export::default(),
            template,
            motion: motion::template(template)
                .map(|t| t.motion)
                .unwrap_or_default(),
            advanced: false,
        }
    }
}

impl Choice {
    /// Start again from a template, keeping the file settings.
    pub fn use_template(&mut self, id: &'static str) {
        if let Some(template) = motion::template(id) {
            self.template = template.id;
            self.motion = template.motion;
        }
    }
}

/// How a finished export reads: good, worth a second look, or failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Written.
    Good,
    /// Something the person has to do first.
    Warn,
    /// Nothing, or not all of it, was written.
    Bad,
}

/// Everything one export needs, taken at the moment the folder was chosen.
pub struct Job {
    /// The vault, let go of as soon as the recording is out of it.
    pub vault: Arc<Vault>,
    /// Which recording.
    pub id: String,
    /// What it is called, which names the file and titles the picture.
    pub name: String,
    /// What was chosen.
    pub choice: Choice,
    /// The folder.
    pub into: PathBuf,
    /// Counted in frames while the picture is drawn.
    pub reach: Arc<crate::progress::Reach>,
}

/// Why no bar can be drawn for an export that has no picture.
pub const NO_FRAMES: &str = "a file of sound alone is written in one piece, with no frames \
     in it to count";

/// How far an export with this choice can say it has got.
///
/// A picture is drawn one frame at a time, so it has a total and a bar. Sound
/// alone has neither, and says why in [`NO_FRAMES`] rather than showing a bar
/// that would have to be invented.
pub fn reach_for(choice: &Choice) -> crate::progress::Reach {
    if choice.export.content == Content::Audio {
        crate::progress::Reach::unmeasurable(NO_FRAMES)
    } else {
        crate::progress::Reach::counting(0)
    }
}

/// A file name built from what somebody called a recording.
///
/// A name reaches a path here, so anything but a letter, a digit, a dash or an
/// underscore becomes a dash. A name Windows reserves for a device (`CON`,
/// `NUL`, `COM1` and the rest) gets `take-` in front: on older Windows,
/// writing `con.wav` writes to the console and leaves no file.
pub fn safe_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        return "take".to_string();
    }
    let stem: String = trimmed.chars().take(60).collect();
    if reserved_on_windows(&stem) {
        format!("take-{stem}")
    } else {
        stem
    }
}

/// Whether Windows treats this name as a device rather than a file.
fn reserved_on_windows(stem: &str) -> bool {
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0')
}

/// The samples and the rate of a canonical 16-bit WAV, mixed to one channel.
pub fn samples_of(wav: &[u8]) -> Option<(Vec<f32>, u32)> {
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return None;
    }
    let channels = u16::from_le_bytes([wav[22], wav[23]]).max(1) as usize;
    let rate = u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]);
    let bits = u16::from_le_bytes([wav[34], wav[35]]);
    if bits != 16 || rate == 0 {
        return None;
    }
    let frames: Vec<f32> = wav[44..]
        .chunks_exact(2 * channels)
        .map(|frame| {
            frame
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32_768.0)
                .sum::<f32>()
                / channels as f32
        })
        .collect();
    Some((frames, rate))
}

/// Do the export. Runs on a worker; the Browser starts it and collects the
/// answer.
pub fn run(job: Job) -> (String, Outcome) {
    run_with(job, veilvoice_video::ffmpeg::found)
}

/// [`run`], with where `ffmpeg` is found passed in, so a test can hand it a
/// stand-in rather than depend on what the machine has installed.
///
/// # Nothing is written through a link, and nothing is written over
///
/// The folder is one somebody chose, and may be shared. Every file this
/// creates there is created new, by the kernel's one atomic answer
/// (`create_new`, `mkdir`, `link`), never by checking and then writing: a check
/// is false for a link that points nowhere, and a write follows it to wherever
/// it points. `ffmpeg` writes into a folder of this run's own, readable only by
/// its owner, and the finished file is linked into place only if nothing is
/// there. So a failure removes only what this run made.
fn run_with(job: Job, ffmpeg: impl FnOnce() -> Option<PathBuf>) -> (String, Outcome) {
    let Job {
        vault,
        id,
        name,
        choice,
        into,
        reach,
    } = job;
    let loaded = vault.load(&id);
    // F-219's rule: the key is not kept for the length of a render.
    drop(vault);
    let wav = match loaded {
        Ok(wav) => wav,
        Err(error) => return (error.to_string(), Outcome::Bad),
    };
    if let Err(why) = choice.export.checked() {
        return (why.to_string(), Outcome::Bad);
    }

    let stem = safe_stem(&name);
    let output = into.join(format!("{stem}.{}", choice.export.extension()));

    // A WAV of the sound alone needs nothing but the bytes already in hand.
    if choice.export.content == Content::Audio
        && choice.export.audio_file == veilvoice_video::export::AudioFile::Wav
    {
        return match veilvoice_crypto::privatefile::write_owner_only_new(&output, wav.expose()) {
            Ok(()) => (wrote(&output), Outcome::Good),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                already_there(&output)
            }
            Err(error) => (error.to_string(), Outcome::Bad),
        };
    }

    let Some(program) = ffmpeg() else {
        return (
            "Nothing was written: every file but a WAV is made by ffmpeg, and ffmpeg is \
             not on this machine. The Setup tab lists it under companion software, with \
             the install command for this system and a button to run it."
                .into(),
            Outcome::Warn,
        );
    };
    let Some((samples, rate)) = samples_of(wav.expose()) else {
        return (
            "That recording does not have a WAV header this can read, so nothing was \
             written."
                .into(),
            Outcome::Bad,
        );
    };
    // Said before minutes of rendering rather than after them. Not what keeps
    // a file from being written over: `place` is.
    if std::fs::symlink_metadata(&output).is_ok() {
        return already_there(&output);
    }

    let room = match Workroom::new(&into) {
        Ok(room) => room,
        Err(error) => return (error.to_string(), Outcome::Bad),
    };
    // ffmpeg reads the sound from a file and the picture from its standard
    // input, and there is only one standard input. So the sound goes into the
    // private folder, when the export has any sound in it.
    let sound = room.path().join("sound.wav");
    if choice.export.content.wants_sound() {
        if let Err(error) =
            veilvoice_crypto::privatefile::write_owner_only_new(&sound, wav.expose())
        {
            return (error.to_string(), Outcome::Bad);
        }
    }
    drop(wav);
    let made = room
        .path()
        .join(format!("{stem}.{}", choice.export.extension()));
    if let Err(why) = render(
        &program, &choice, &name, &samples, rate, &sound, &made, &reach,
    ) {
        return (why, Outcome::Bad);
    }
    // ffmpeg made the file under its own umask, inside a folder nobody else
    // can open; it is owner-only before it leaves that folder.
    if let Err(error) = veilvoice_crypto::privatefile::tighten(&made) {
        return (error.to_string(), Outcome::Bad);
    }
    match place(&made, &output) {
        Ok(()) => (wrote(&output), Outcome::Good),
        Err(said) => said,
    }
    // `room` goes here, and the sound and anything ffmpeg left with it.
}

/// What an export says when its file is already there.
fn already_there(output: &Path) -> (String, Outcome) {
    (
        format!(
            "{} is already there, and nothing here writes over a file. Move it or \
             choose another folder.",
            output.display()
        ),
        Outcome::Warn,
    )
}

/// Put the finished file where it was asked for, if nothing is there.
///
/// A hard link is created new or not at all, and does not follow a link that
/// is already at the destination. A filesystem without hard links (FAT and
/// exFAT, which most memory sticks are) gets a rename after a look, which is
/// the one place a check comes before a write; such a filesystem has no
/// symbolic links to plant either.
fn place(made: &Path, output: &Path) -> Result<(), (String, Outcome)> {
    match std::fs::hard_link(made, output) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(already_there(output))
        }
        Err(_) => {
            if std::fs::symlink_metadata(output).is_ok() {
                return Err(already_there(output));
            }
            std::fs::rename(made, output).map_err(|error| (error.to_string(), Outcome::Bad))
        }
    }
}

/// A folder of one export's own, beside where its file is going, removed with
/// everything in it when the export is done, however it ended.
///
/// Beside rather than in the system's temporary folder, so the finished file
/// can be linked into place on the same filesystem. Created by `mkdir`, which
/// fails rather than follows anything already at the name, and readable only
/// by its owner. A crash in the middle of a render leaves it behind, under a
/// name that says what it is.
struct Workroom(PathBuf);

impl Workroom {
    /// Makes a folder only this user can enter, beside `beside`, under a name
    /// no other folder there has.
    fn new(beside: &Path) -> std::io::Result<Self> {
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        for attempt in 0..100u32 {
            let path = beside.join(format!(
                ".veilvoice-export-{}-{attempt}",
                std::process::id()
            ));
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "a hundred folders from earlier exports are in the way",
        ))
    }

    /// Where the folder is, to write the job's files into.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Workroom {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a successful export says.
fn wrote(output: &Path) -> String {
    format!(
        "Wrote {}. It is not sealed: what leaves the vault is an ordinary file, and the \
         voice in it is still a voice nobody owns.",
        output.display()
    )
}

/// Run ffmpeg, drawing and piping the frames when there is a picture.
#[allow(clippy::too_many_arguments)]
fn render(
    program: &Path,
    choice: &Choice,
    title: &str,
    samples: &[f32],
    rate: u32,
    sound: &Path,
    output: &Path,
    reach: &crate::progress::Reach,
) -> Result<(), String> {
    let argv = choice
        .export
        .command(sound, output)
        .map_err(|error| error.to_string())?;
    let mut child = crate::command(program)
        .args(argv.iter().skip(1))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("ffmpeg could not be started: {error}"))?;

    // Drained on a thread of its own, so a complaint longer than a pipe holds
    // cannot stop ffmpeg while this is still feeding it frames.
    let mut stderr = child.stderr.take();
    let complaints = std::thread::spawn(move || {
        let mut said = String::new();
        if let Some(stream) = stderr.as_mut() {
            let _ = stream.read_to_string(&mut said);
        }
        said
    });

    let mut stdin = child.stdin.take();
    if choice.export.content.wants_picture() {
        let levels = motion::levels(samples, rate, &choice.motion);
        let renderer = motion::Renderer::new(
            &choice.motion,
            choice.export.plan.size,
            choice.export.plan.fps.get(),
            levels,
            Some(title),
        )
        .map_err(|error| error.to_string())?;
        reach.set_total(renderer.frames());
        let mut frame = Vec::with_capacity(renderer.frame_bytes());
        if let Some(pipe) = stdin.as_mut() {
            for index in 0..renderer.frames() {
                renderer.frame(index, &mut frame);
                if pipe.write_all(&frame).is_err() {
                    // ffmpeg stopped reading, which means it stopped. Its own
                    // words, collected below, say why.
                    break;
                }
                reach.advance(1);
            }
        }
    }
    drop(stdin);

    let status = child.wait().map_err(|error| error.to_string())?;
    let said = complaints.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let last = said.lines().rev().find(|line| !line.trim().is_empty());
    Err(format!(
        "ffmpeg refused: {}",
        last.unwrap_or("it gave no reason").trim()
    ))
}

/// The export form, drawn under the recording it is for.
///
/// Returns true on the frame the button that asks for a folder is pressed,
/// and only when what is chosen can be written: a combination `ffmpeg` would
/// refuse is named in red and the button is not offered, rather than being
/// found out a render later.
///
/// Reads and writes nothing but `choice`. The folder, the vault and the work
/// are the caller's, on a worker.
pub fn form(ui: &mut egui::Ui, choice: &mut Choice) -> bool {
    use crate::layout::{chip, chip_value};
    use crate::theme::palette as p;
    use egui::RichText;
    use veilvoice_video::export::{AudioFile, Container, Quality};
    use veilvoice_video::size::{FrameRate, Preset};

    let heading = |ui: &mut egui::Ui, text: &str| {
        ui.label(RichText::new(text).color(p::muted()).small());
    };

    heading(ui, "what goes in it");
    ui.horizontal_wrapped(|ui| {
        for content in Content::ALL {
            chip_value(ui, &mut choice.export.content, content, content.label());
        }
    });

    if choice.export.content == Content::Audio {
        heading(ui, "the file");
        ui.horizontal_wrapped(|ui| {
            for file in [AudioFile::Flac, AudioFile::Wav] {
                chip_value(ui, &mut choice.export.audio_file, file, file.label());
            }
        });
    } else {
        heading(ui, "container");
        ui.horizontal_wrapped(|ui| {
            for container in Container::ALL {
                if chip_value(
                    ui,
                    &mut choice.export.container,
                    container,
                    container.label(),
                )
                .changed()
                    && !container.video_codecs().contains(&choice.export.codec)
                {
                    // The codec follows the container rather than leaving a
                    // combination on screen that the button would refuse.
                    choice.export.codec = container.video_codecs()[0];
                }
            }
        });
        heading(ui, "codec");
        ui.horizontal_wrapped(|ui| {
            for codec in choice.export.container.video_codecs() {
                chip_value(ui, &mut choice.export.codec, *codec, codec.label());
            }
        });
        heading(ui, "quality");
        ui.horizontal_wrapped(|ui| {
            for quality in Quality::ALL {
                chip_value(ui, &mut choice.export.quality, quality, quality.label());
            }
        });
        heading(ui, "size and smoothness");
        ui.horizontal_wrapped(|ui| {
            for preset in Preset::ALL {
                if ui
                    .add(chip(
                        choice.export.plan.size == preset.size(),
                        preset.name(),
                    ))
                    .clicked()
                {
                    choice.export.plan.size = preset.size();
                }
            }
            for fps in [24, 30, 60] {
                if let Ok(rate) = FrameRate::new(fps) {
                    if ui
                        .add(chip(choice.export.plan.fps == rate, format!("{fps} fps")))
                        .clicked()
                    {
                        choice.export.plan.fps = rate;
                    }
                }
            }
        });
        heading(ui, "the look");
        ui.horizontal_wrapped(|ui| {
            for template in motion::templates() {
                if ui
                    .add(chip(choice.template == template.id, template.name))
                    .clicked()
                {
                    choice.use_template(template.id);
                }
            }
        });
        if ui
            .add(chip(choice.advanced, "the look, in detail"))
            .clicked()
        {
            choice.advanced = !choice.advanced;
        }
        if choice.advanced {
            look(ui, &mut choice.motion);
        }
    }

    ui.add_space(4.0);
    ui.label(
        RichText::new(choice.export.describe())
            .color(p::fg())
            .small(),
    );
    let refused = choice
        .export
        .checked()
        .err()
        .map(|why| why.to_string())
        .or_else(|| choice.motion.checked().err().map(|why| why.to_string()));
    match refused {
        Some(why) => {
            ui.label(RichText::new(why).color(p::red()).small());
            false
        }
        None => ui.button("choose a folder and export").clicked(),
    }
}

/// The advanced half of the form: the typeface, the colours and the motion.
///
/// One kind of control to a line, so no row mixes a slider with a colour
/// button of a different height; `row_tests` in `lib.rs` measures it.
fn look(ui: &mut egui::Ui, chosen: &mut Motion) {
    use crate::layout::chip_value;
    use crate::theme::palette as p;
    use egui::RichText;
    use veilvoice_video::motion::{Face, Style, RESPONSE_RANGE, SETTLE_RANGE, WINDOW_RANGE};

    ui.horizontal_wrapped(|ui| {
        for face in Face::ALL {
            chip_value(ui, &mut chosen.face, face, face.label());
        }
    });
    ui.horizontal_wrapped(|ui| {
        for style in Style::ALL {
            chip_value(ui, &mut chosen.style, style, style.label());
        }
    });
    // The value is written into the label rather than shown in the box a
    // slider puts beside itself, which is a control of another height on the
    // same row.
    let response = chosen.response;
    ui.add(
        egui::Slider::new(&mut chosen.response, RESPONSE_RANGE.0..=RESPONSE_RANGE.1)
            .show_value(false)
            .text(format!(
                "how strongly the wave answers the sound, {response:.2}"
            )),
    );
    let window = chosen.window_secs;
    ui.add(
        egui::Slider::new(&mut chosen.window_secs, WINDOW_RANGE.0..=WINDOW_RANGE.1)
            .show_value(false)
            .text(format!("seconds shown at once, {window:.1}")),
    );
    let settle = chosen.settle_secs;
    ui.add(
        egui::Slider::new(&mut chosen.settle_secs, SETTLE_RANGE.0..=SETTLE_RANGE.1)
            .show_value(false)
            .text(format!("seconds it takes to settle, {settle:.2}")),
    );
    fill(ui, "background", &mut chosen.background);
    fill(ui, "wave", &mut chosen.wave);
    ui.horizontal(|ui| {
        ui.color_edit_button_srgb(&mut chosen.ink);
        ui.label(RichText::new("lettering").color(p::muted()).small());
    });
}

/// One colour, or two with a gradient between them.
fn fill(ui: &mut egui::Ui, what: &str, fill: &mut motion::Fill) {
    use crate::layout::chip;
    use crate::theme::palette as p;
    use egui::RichText;
    use motion::{Direction, Fill};

    ui.horizontal(|ui| {
        match fill {
            Fill::Solid(colour) => {
                ui.color_edit_button_srgb(colour);
            }
            Fill::Gradient { from, to, .. } => {
                ui.color_edit_button_srgb(from);
                ui.color_edit_button_srgb(to);
            }
        }
        ui.label(RichText::new(what).color(p::muted()).small());
    });
    ui.horizontal_wrapped(|ui| {
        let solid = matches!(fill, Fill::Solid(_));
        if ui.add(chip(solid, "one colour")).clicked() && !solid {
            *fill = Fill::Solid(fill.middle());
        }
        for direction in Direction::ALL {
            let chosen = matches!(fill, Fill::Gradient { direction: d, .. } if *d == direction);
            if ui
                .add(chip(
                    chosen,
                    format!("gradient {}", direction.label().to_lowercase()),
                ))
                .clicked()
            {
                *fill = match *fill {
                    Fill::Solid(colour) => Fill::Gradient {
                        from: colour,
                        to: [255 - colour[0], 255 - colour[1], 255 - colour[2]],
                        direction,
                    },
                    Fill::Gradient { from, to, .. } => Fill::Gradient {
                        from,
                        to,
                        direction,
                    },
                };
            }
        }
    });
}

/// A small picture of every recording in the list.
#[derive(Default)]
pub struct Thumbs {
    made: HashMap<String, egui::TextureHandle>,
    asked: HashSet<String>,
    arriving: Option<mpsc::Receiver<(String, Vec<u8>)>>,
    drawn_with: Option<Motion>,
}

impl Thumbs {
    /// The picture for a recording, once it has been drawn.
    pub fn get(&self, id: &str) -> Option<&egui::TextureHandle> {
        self.made.get(id)
    }

    /// Forget every picture. Called when the vault shuts, and when the
    /// template they were drawn with changes.
    pub fn clear(&mut self) {
        self.made.clear();
        self.asked.clear();
        self.arriving = None;
        self.drawn_with = None;
    }

    /// Take whatever pictures have arrived. Never waits.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.arriving else { return };
        loop {
            match rx.try_recv() {
                Ok((id, rgb)) => {
                    let image = egui::ColorImage::from_rgb([THUMB_WIDTH, THUMB_HEIGHT], &rgb);
                    let texture = ctx.load_texture(
                        format!("thumb-{id}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.made.insert(id, texture);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.arriving = None;
                    break;
                }
            }
        }
    }

    /// Draw the pictures nobody has asked for yet, on a worker.
    ///
    /// Does nothing while a batch is still arriving, so a list drawn sixty
    /// times a second starts one worker rather than sixty.
    pub fn ask(&mut self, ctx: &egui::Context, vault: &Arc<Vault>, ids: &[String], look: &Motion) {
        // One batch at a time, so a slider being dragged starts one worker
        // and not one a frame. The look is compared whole, not by template:
        // the advanced half changes a look without changing its template.
        if self.arriving.is_some() {
            return;
        }
        if self.drawn_with.is_some_and(|drawn| drawn != *look) {
            // Asked again, and the old pictures stay until the new ones land,
            // so the list does not blink empty.
            self.asked.clear();
        }
        let wanted: Vec<String> = ids
            .iter()
            .filter(|id| !self.asked.contains(*id))
            .cloned()
            .collect();
        if wanted.is_empty() {
            return;
        }
        self.asked.extend(wanted.iter().cloned());
        self.drawn_with = Some(*look);
        let (tx, rx) = mpsc::channel();
        self.arriving = Some(rx);
        let weak: Weak<Vault> = Arc::downgrade(vault);
        let look = *look;
        let ctx = ctx.clone();
        std::thread::spawn(move || draw_thumbnails(weak, wanted, look, tx, ctx));
    }
}

/// The worker behind [`Thumbs::ask`].
fn draw_thumbnails(
    weak: Weak<Vault>,
    ids: Vec<String>,
    look: Motion,
    tx: mpsc::Sender<(String, Vec<u8>)>,
    ctx: egui::Context,
) {
    for id in ids {
        // A strong reference for one load and no longer. If the vault has been
        // shut this fails, and the rest of the list is not drawn.
        let Some(vault) = weak.upgrade() else { return };
        let loaded = vault.load(&id);
        drop(vault);
        let Ok(wav) = loaded else { continue };
        let Some((samples, _)) = samples_of(wav.expose()) else {
            continue;
        };
        drop(wav);
        let picture = motion::thumbnail(&samples, &look, THUMB_WIDTH, THUMB_HEIGHT);
        if tx.send((id, picture.rgb().to_vec())).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
        let data = (samples.len() * 2) as u32;
        let mut out = Vec::new();
        out.extend(b"RIFF");
        out.extend((36 + data).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(channels.to_le_bytes());
        out.extend(rate.to_le_bytes());
        out.extend((rate * 2 * channels as u32).to_le_bytes());
        out.extend((2 * channels).to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend(data.to_le_bytes());
        for s in samples {
            out.extend(s.to_le_bytes());
        }
        out
    }

    /// Sound alone admits it cannot be measured; a picture counts its frames.
    #[test]
    fn only_a_picture_draws_a_bar() {
        let mut choice = Choice::default();
        assert_eq!(reach_for(&choice).because(), None);
        choice.export.content = Content::Audio;
        assert_eq!(reach_for(&choice).because(), Some(NO_FRAMES));
    }

    #[test]
    fn a_stereo_recording_is_mixed_to_one_channel() {
        let (samples, rate) = samples_of(&wav(&[16_384, 0, -16_384, 0], 2, 44_100)).unwrap();
        assert_eq!(rate, 44_100);
        assert_eq!(samples, vec![0.25, -0.25]);
        assert!(samples_of(b"not a wav at all, and far too short").is_none());
    }

    #[test]
    fn a_name_cannot_reach_outside_the_folder() {
        assert_eq!(safe_stem("../../etc/passwd"), "etc-passwd");
        assert_eq!(safe_stem("///"), "take");
        assert_eq!(safe_stem("Interview 2"), "Interview-2");
    }

    /// A name Windows keeps for a device is not used as a file name.
    #[test]
    fn a_device_name_is_not_a_file_name() {
        for name in ["con", "CON", "nul", "Aux", "prn", "com1", "LPT9"] {
            assert_eq!(safe_stem(name), format!("take-{name}"));
        }
        for name in ["console", "com", "com0", "lpt10", "nulled"] {
            assert_eq!(safe_stem(name), name, "{name} is an ordinary name");
        }
    }

    /// A stand-in for ffmpeg: reads the frames, then writes its last argument,
    /// or fails after doing what `script` says.
    /// `$last` has the `file:` prefix every path is given (F-283) taken off.
    #[cfg(unix)]
    fn fake_ffmpeg(dir: &Path, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("ffmpeg");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\ncat > /dev/null\nfor last; do :; done\nlast=\"${{last#file:}}\"\n{script}\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[cfg(unix)]
    fn small_video() -> Choice {
        let mut choice = Choice::default();
        choice.export.plan.size = veilvoice_video::size::Preset::Hd720.size();
        choice
    }

    /// Only the export's own folder and the finished file are left behind.
    #[cfg(unix)]
    fn leftovers(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n != "vault" && n != "tools")
            .collect();
        names.sort();
        names
    }

    /// A render is placed, owner-only, the frames counted, and nothing else
    /// is left in the folder.
    #[cfg(unix)]
    #[test]
    fn a_render_lands_owner_only_and_leaves_nothing_else() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, vault, id) = vault_with(&[1000; 800]);
        let tools = dir.path().join("tools");
        std::fs::create_dir(&tools).unwrap();
        let ffmpeg = fake_ffmpeg(&tools, "umask 022; printf made > \"$last\"");
        let reach = Arc::new(crate::progress::Reach::counting(0));
        let mut job = job(&vault, &id, dir.path(), small_video());
        job.reach = Arc::clone(&reach);
        let (said, outcome) = run_with(job, || Some(ffmpeg));
        assert_eq!(outcome, Outcome::Good, "{said}");
        let output = dir.path().join("take-one.mp4");
        assert_eq!(std::fs::read(&output).unwrap(), b"made");
        let mode = std::fs::metadata(&output).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the export is readable by others: {mode:o}");
        assert!(reach.total() > 0 && reach.done() == reach.total());
        assert_eq!(leftovers(dir.path()), ["take-one.mp4"]);
    }

    /// A failed render removes only what this run made. A file that turned up
    /// at the destination while it rendered is somebody else's and stays.
    #[cfg(unix)]
    #[test]
    fn a_failed_render_removes_only_its_own_work() {
        let (dir, vault, id) = vault_with(&[1000; 800]);
        let tools = dir.path().join("tools");
        std::fs::create_dir(&tools).unwrap();
        let ffmpeg = fake_ffmpeg(
            &tools,
            "room=$(dirname \"$last\"); printf theirs > \"$(dirname \"$room\")/$(basename \"$last\")\"; \
             echo 'File exists' >&2; exit 1",
        );
        let (said, outcome) =
            run_with(job(&vault, &id, dir.path(), small_video()), || Some(ffmpeg));
        assert_eq!(outcome, Outcome::Bad, "{said}");
        assert!(said.contains("File exists"), "{said}");
        assert_eq!(
            std::fs::read(dir.path().join("take-one.mp4")).unwrap(),
            b"theirs",
            "the failed render deleted a file it did not make"
        );
        assert_eq!(leftovers(dir.path()), ["take-one.mp4"]);
    }

    /// A link planted where the file is going is not followed, whether it
    /// points at something or at nothing.
    #[cfg(unix)]
    #[test]
    fn a_link_at_the_destination_is_not_followed() {
        let (dir, vault, id) = vault_with(&[1000; 800]);
        let tools = dir.path().join("tools");
        std::fs::create_dir(&tools).unwrap();
        let victim = tools.join("victim");
        std::fs::write(&victim, b"precious").unwrap();
        let ffmpeg = fake_ffmpeg(&tools, "printf made > \"$last\"");

        for (choice, name) in [
            (wav_only(), "take-one.wav"),
            (small_video(), "take-one.mp4"),
        ] {
            for target in [victim.clone(), tools.join("nothing-here")] {
                let link = dir.path().join(name);
                std::os::unix::fs::symlink(&target, &link).unwrap();
                let program = ffmpeg.clone();
                let (said, outcome) =
                    run_with(job(&vault, &id, dir.path(), choice.clone()), || {
                        Some(program)
                    });
                assert_eq!(
                    outcome,
                    Outcome::Warn,
                    "{name} -> {}: {said}",
                    target.display()
                );
                assert_eq!(std::fs::read(&victim).unwrap(), b"precious");
                assert!(
                    !tools.join("nothing-here").exists(),
                    "a dangling link was followed"
                );
                std::fs::remove_file(&link).unwrap();
            }
        }
        assert!(
            leftovers(dir.path()).is_empty(),
            "{:?}",
            leftovers(dir.path())
        );
    }

    /// The thumbnails are drawn again when the look changes, not only when
    /// the template does, and one batch runs at a time.
    #[test]
    fn a_new_look_redraws_the_thumbnails() {
        let (_dir, vault, id) = vault_with(&[1000; 800]);
        let ctx = egui::Context::default();
        let mut thumbs = Thumbs::default();
        let mut look = Motion::default();
        let ids = vec![id.clone()];
        thumbs.ask(&ctx, &vault, &ids, &look);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while thumbs.arriving.is_some() {
            thumbs.poll(&ctx);
            assert!(
                std::time::Instant::now() < deadline,
                "the picture never came"
            );
        }
        assert!(thumbs.get(&id).is_some());

        thumbs.ask(&ctx, &vault, &ids, &look);
        assert!(thumbs.arriving.is_none(), "the same look was drawn twice");

        look.response *= 2.0;
        thumbs.ask(&ctx, &vault, &ids, &look);
        assert!(thumbs.arriving.is_some(), "a new look did not redraw");
        assert!(
            thumbs.get(&id).is_some(),
            "the old picture went before the new one came"
        );
        look.response *= 2.0;
        let before = thumbs.drawn_with;
        thumbs.ask(&ctx, &vault, &ids, &look);
        assert_eq!(
            thumbs.drawn_with, before,
            "a second batch started over the first"
        );
    }

    #[test]
    fn the_default_is_the_default_theme_and_a_valid_export() {
        let choice = Choice::default();
        assert_eq!(choice.template, veilvoice_video::palette::DEFAULT_ID);
        assert!(choice.export.checked().is_ok());
        let mut changed = choice.clone();
        changed.use_template("neon");
        assert_eq!(changed.template, "neon");
        assert_eq!(
            changed.export, choice.export,
            "a template changes the look only"
        );
        changed.use_template("no-such-template");
        assert_eq!(changed.template, "neon");
    }

    fn vault_with(samples: &[i16]) -> (tempfile::TempDir, Arc<Vault>, String) {
        use veilvoice_crypto::studio::StudioKey;
        let dir = tempfile::tempdir().unwrap();
        let secret = |b: &[u8]| {
            let mut copy = b.to_vec();
            veilvoice_crypto::Secret::new(&mut copy)
        };
        let key = StudioKey::derive(&secret(b"app-lock"), &secret(b"at-rest")).unwrap();
        let vault = Vault::open(dir.path().join("vault"), key).unwrap();
        let id = vault
            .store("take one", 1_700_000_000, &wav(samples, 1, 8_000))
            .unwrap()
            .id;
        (dir, Arc::new(vault), id)
    }

    fn job(vault: &Arc<Vault>, id: &str, into: &Path, choice: Choice) -> Job {
        Job {
            vault: Arc::clone(vault),
            id: id.to_string(),
            name: "take one".into(),
            choice,
            into: into.to_path_buf(),
            reach: Arc::new(crate::progress::Reach::counting(0)),
        }
    }

    fn wav_only() -> Choice {
        let mut choice = Choice::default();
        choice.export.content = Content::Audio;
        choice.export.audio_file = veilvoice_video::export::AudioFile::Wav;
        choice
    }

    /// The sound alone as a WAV needs no ffmpeg, and is the stored bytes.
    #[test]
    fn a_wav_on_its_own_is_the_recording_bit_for_bit() {
        let samples: Vec<i16> = (0..800).map(|i| (i * 37 % 2000) as i16).collect();
        let (dir, vault, id) = vault_with(&samples);
        let (said, outcome) = run(job(&vault, &id, dir.path(), wav_only()));
        assert_eq!(outcome, Outcome::Good, "{said}");
        let written = std::fs::read(dir.path().join("take-one.wav")).unwrap();
        assert_eq!(written, wav(&samples, 1, 8_000));
    }

    #[test]
    fn nothing_is_written_over() {
        let (dir, vault, id) = vault_with(&[0; 80]);
        std::fs::write(dir.path().join("take-one.wav"), b"somebody's file").unwrap();
        let (said, outcome) = run(job(&vault, &id, dir.path(), wav_only()));
        assert_eq!(outcome, Outcome::Warn, "{said}");
        assert_eq!(
            std::fs::read(dir.path().join("take-one.wav")).unwrap(),
            b"somebody's file"
        );
    }

    /// The render lets go of the vault before it does anything else.
    #[test]
    fn the_render_does_not_keep_the_vault() {
        let (dir, vault, id) = vault_with(&[0; 80]);
        let _ = run(job(&vault, &id, dir.path(), wav_only()));
        assert_eq!(Arc::strong_count(&vault), 1);
    }

    /// A shut vault stops the thumbnails: the worker holds a weak reference
    /// and nothing it draws outlives the vault.
    #[test]
    fn thumbnails_stop_when_the_vault_is_shut() {
        let (_dir, vault, id) = vault_with(&[1000; 800]);
        let weak = Arc::downgrade(&vault);
        drop(vault);
        let (tx, rx) = mpsc::channel();
        draw_thumbnails(
            weak,
            vec![id],
            Motion::default(),
            tx,
            egui::Context::default(),
        );
        assert!(rx.try_recv().is_err(), "nothing drawn from a shut vault");
    }

    #[test]
    fn a_thumbnail_is_drawn_for_an_open_vault_at_the_promised_size() {
        let (_dir, vault, id) = vault_with(&[1000; 800]);
        let (tx, rx) = mpsc::channel();
        draw_thumbnails(
            Arc::downgrade(&vault),
            vec![id.clone()],
            Motion::default(),
            tx,
            egui::Context::default(),
        );
        let (got, rgb) = rx.try_recv().unwrap();
        assert_eq!(got, id);
        assert_eq!(rgb.len(), THUMB_WIDTH * THUMB_HEIGHT * 3);
        assert_eq!(Arc::strong_count(&vault), 1);
    }
}
