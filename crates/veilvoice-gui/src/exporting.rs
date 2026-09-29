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
pub const NO_FRAMES: &str = "the sound is handed to ffmpeg whole, and it reports nothing \
     back that this window can read";

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
/// The same rule the older export follows: a name reaches a path here, so
/// anything but a letter, a digit, a dash or an underscore becomes a dash.
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
        "take".to_string()
    } else {
        trimmed.chars().take(60).collect()
    }
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
    if output.exists() {
        return (
            format!(
                "{} is already there, and nothing here writes over a file. Move it or \
                 choose another folder.",
                output.display()
            ),
            Outcome::Warn,
        );
    }

    // A WAV of the sound alone needs nothing but the bytes already in hand.
    if choice.export.content == Content::Audio
        && choice.export.audio_file == veilvoice_video::export::AudioFile::Wav
    {
        return match veilvoice_crypto::privatefile::write_owner_only(&output, wav.expose()) {
            Ok(()) => (wrote(&output), Outcome::Good),
            Err(error) => (error.to_string(), Outcome::Bad),
        };
    }

    let Some(program) = veilvoice_video::ffmpeg::found() else {
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

    // ffmpeg reads the sound from a file and the picture from its standard
    // input, and there is only one standard input. So the sound goes beside
    // the output for the length of the render, owner-only, and is removed
    // after, whatever happened.
    let sound = into.join(format!(".{stem}.veiled-sound.wav"));
    if let Err(error) = veilvoice_crypto::privatefile::write_owner_only(&sound, wav.expose()) {
        return (error.to_string(), Outcome::Bad);
    }
    drop(wav);
    let result = render(
        &program, &choice, &name, &samples, rate, &sound, &output, &reach,
    );
    let _ = std::fs::remove_file(&sound);
    match result {
        Ok(()) => (wrote(&output), Outcome::Good),
        Err(why) => {
            // A half-written file is worse than none: it plays, and stops.
            let _ = std::fs::remove_file(&output);
            (why, Outcome::Bad)
        }
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

/// A small picture of every recording in the list.
#[derive(Default)]
pub struct Thumbs {
    made: HashMap<String, egui::TextureHandle>,
    asked: HashSet<String>,
    arriving: Option<mpsc::Receiver<(String, Vec<u8>)>>,
    drawn_with: Option<&'static str>,
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
    pub fn ask(
        &mut self,
        ctx: &egui::Context,
        vault: &Arc<Vault>,
        ids: &[String],
        template: &'static str,
        look: &Motion,
    ) {
        if self.drawn_with.is_some_and(|drawn| drawn != template) {
            self.clear();
        }
        if self.arriving.is_some() {
            return;
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
        self.drawn_with = Some(template);
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
