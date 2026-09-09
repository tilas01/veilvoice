// SPDX-License-Identifier: GPL-3.0-or-later
//! The video's pictures, and how many of them there really are.
//!
//! # What this finishes
//!
//! [`crate::ffmpeg::command`] has always known how to turn a directory of
//! pictures into a video file. Nothing filled the directory, so what a render
//! produced was veiled audio over a black picture, and the roadmap row said so
//! rather than letting somebody find out by playing the file.
//!
//! This fills it, with [`crate::raster`] for the pixels and [`crate::font`] for
//! the names.
//!
//! # A frame is written when the picture changes, not thirty times a second
//!
//! An hour at thirty frames a second is 108,000 pictures. Written out at 1080p
//! that is gigabytes of intermediate files to make one video, and almost all of
//! them are identical to the one before.
//!
//! So the frame rate decides when the picture is *looked at*, and a new file is
//! written only when what it would contain has actually changed. Three things
//! can change it, and [`Signature`] is exactly those three:
//!
//! * the playhead, which moves one pixel at a time and not one frame at a time,
//! * the level bars, which move when the envelope column changes,
//! * who is lit, which changes at a turn boundary.
//!
//! On a 1920-wide waveform over an hour the playhead moves a pixel about every
//! two seconds, so runs of fifty-odd identical frames collapse into one file
//! held for the length of the run. That is not a guess: [`plan`] reports how
//! many pictures it actually produced against how many frames the video has,
//! and a caller can show the ratio.
//!
//! **A short recording saves nothing, and should not.** The playhead crosses
//! the whole waveform however long the recording is, so under about forty
//! seconds it moves more than a pixel per frame and every frame is genuinely a
//! different picture. The saving arrives with length, which is exactly where it
//! was needed.
//!
//! **This is why the ffmpeg command is a concat list rather than a numbered
//! sequence.** `image2` gives every file the same duration; a held frame needs
//! its own. See [`crate::ffmpeg::concat_command`].
//!
//! # What the video cannot draw that the page can
//!
//! Names outside printable ASCII. The page is markup and uses whatever face the
//! reader's machine has; this has one face, written here, for the reasons
//! [`crate::font`] gives. A name it cannot draw comes out as open boxes and is
//! **named in the notes**, because a person who typed a name in Cyrillic should
//! be told before they render an hour of video rather than after.
//!
//! # In plain words
//!
//! This draws the pictures the video is made of.
//!
//! It only draws a new one when something on screen has actually moved, which
//! for a long recording is a tiny fraction of the frames the video has, so a
//! render writes hundreds of pictures rather than hundreds of thousands.

use std::path::Path;

use veilvoice_conversation::Conversation;

use crate::page::{self, Background, Look};
use crate::raster::{self, Canvas, Rgb};
use crate::size;
use crate::waveform::{self, Envelope};
use crate::Error;

/// What decides whether two moments look the same.
///
/// Compared rather than the pixels themselves: drawing a 1080p frame to find
/// out it matched the last one costs more than the frame it saves. These three
/// are everything on the picture that moves.
#[derive(Clone, PartialEq, Eq)]
struct Signature {
    /// Where the playhead sits, rounded to the pixel it is drawn on.
    playhead: i64,
    /// Which envelope column the level bars are reading.
    column: usize,
    /// Who is lit, in slot order.
    speaking: Vec<usize>,
}

impl Signature {
    fn at(plan: &Conversation, envelope: &Envelope, look: &Look, at_secs: f64) -> Self {
        let layout = page::layout(look, plan.len());
        let duration = plan.duration().max(1e-9);
        let progress = (at_secs / duration).clamp(0.0, 1.0);
        Self {
            playhead: (layout.wave_x + layout.wave_width * progress as f32).round() as i64,
            column: if envelope.is_empty() {
                0
            } else {
                ((progress * (envelope.len() - 1) as f64).round() as usize).min(envelope.len() - 1)
            },
            speaking: plan
                .turns()
                .iter()
                .filter(|turn| at_secs >= turn.start && at_secs < turn.end)
                .map(|turn| turn.speaker)
                .collect(),
        }
    }
}

/// One picture, and how long the video shows it for.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The moment it was drawn at.
    pub at_secs: f64,
    /// How long it stays on screen, in seconds.
    pub hold_secs: f64,
}

/// The pictures a render will write, worked out without drawing any of them.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The moments, in order.
    pub frames: Vec<Frame>,
    /// How many frames the finished video has.
    ///
    /// Always the frame rate times the duration. The video is not shorter for
    /// having been drawn fewer times; the pictures are held.
    pub video_frames: u64,
}

impl Plan {
    /// How many pictures were saved by holding the ones that did not change.
    ///
    /// `1.0` means every frame was distinct and nothing was saved. Above that
    /// is how many video frames each written picture covers.
    pub fn saving(&self) -> f64 {
        if self.frames.is_empty() {
            return 1.0;
        }
        self.video_frames as f64 / self.frames.len() as f64
    }
}

/// Work out which moments need a picture.
///
/// The frame rate says when to look; the [`Signature`] says whether what would
/// be drawn has changed since the last look. Nothing is drawn here, so a front
/// end can ask what a render will cost before starting one.
pub fn plan(conversation: &Conversation, envelope: &Envelope, look: &Look, fps: u32) -> Plan {
    let duration = conversation.duration().max(0.0);
    let fps = fps.max(1);
    // Ceiling, so the last partial frame is still shown: a recording of 1.02
    // seconds at thirty is 31 frames, not 30, and the missing one is the end.
    let video_frames = ((duration * fps as f64).ceil() as u64).max(1);

    let mut frames: Vec<Frame> = Vec::new();
    let mut last: Option<Signature> = None;
    for frame in 0..video_frames {
        let at_secs = frame as f64 / fps as f64;
        let now = Signature::at(conversation, envelope, look, at_secs);
        if last.as_ref() == Some(&now) {
            continue;
        }
        frames.push(Frame {
            at_secs,
            hold_secs: 0.0,
        });
        last = Some(now);
    }

    // Each picture is held until the next one starts, and the last until the
    // end. Filled in afterwards because a frame does not know its own length
    // until the following one exists.
    for at in 0..frames.len() {
        let until = frames
            .get(at + 1)
            .map(|next| next.at_secs)
            .unwrap_or(duration.max(frames[at].at_secs));
        frames[at].hold_secs = (until - frames[at].at_secs).max(1.0 / fps as f64);
    }

    Plan {
        frames,
        video_frames,
    }
}

/// What a drawn frame carried with it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notes {
    /// Names with characters the built-in face cannot draw.
    ///
    /// Reported rather than silently boxed: somebody who typed a name in an
    /// alphabet this face does not have should hear it before rendering an
    /// hour of video, not after.
    pub undrawable: Vec<String>,
}

/// Draw the picture at `at_secs`.
///
/// The same layout the page uses, from [`page::layout`], so the video and the
/// preview cannot drift into being two different pictures of one recording.
pub fn draw(
    conversation: &Conversation,
    envelope: &Envelope,
    look: &Look,
    at_secs: f64,
) -> Result<(Canvas, Notes), Error> {
    look.checked()?;
    let width = look.width as usize;
    let height = look.height as usize;
    let layout = page::layout(look, conversation.len());
    let mut notes = Notes::default();

    let ink = raster::colour(look.palette.fg).unwrap_or([230, 230, 230]);
    let muted = raster::colour(look.palette.muted).unwrap_or([140, 140, 140]);
    let inset = raster::colour(look.palette.bg_inset).unwrap_or([30, 30, 30]);
    let border = raster::colour(look.palette.border).unwrap_or([60, 60, 60]);
    // An image background is a page feature: the video's frames are drawn, and
    // a picture that is a data URI in markup is not a thing this rasteriser
    // reads. Flat colour either way, which is what the palette says.
    let background = match &look.background {
        Background::Colour(hex) => raster::colour(hex),
        Background::Image { .. } => None,
    }
    .or_else(|| raster::colour(look.palette.bg))
    .unwrap_or([16, 16, 16]);

    let mut canvas = Canvas::new(width, height, background);

    if let Some(title) = &conversation.title {
        let room = width.saturating_sub(look.padding as usize * 2);
        let scale = crate::font::scale_for(title, room);
        // The page puts the title's baseline at `title_y`; a bitmap glyph is
        // positioned by its top, so the seven rows are lifted off the baseline
        // to put the two in the same place.
        let top = layout.title_y as i64 - (crate::font::HEIGHT * scale) as i64;
        if canvas.text_centred(width as f32 / 2.0, top, title, scale, ink) > 0 {
            notes.undrawable.push(title.clone());
        }
    }

    let duration = conversation.duration().max(1e-9);
    let progress = (at_secs / duration).clamp(0.0, 1.0);
    let level = waveform::level_at(envelope, at_secs / duration);
    let speaking: Vec<usize> = conversation
        .turns()
        .iter()
        .filter(|turn| at_secs >= turn.start && at_secs < turn.end)
        .map(|turn| turn.speaker)
        .collect();

    let count = conversation.len().max(1);
    let step = layout.wave_width / count as f32;
    for slot in 0..conversation.len() {
        let centre_x = layout.wave_x + step * (slot as f32 + 0.5);
        let talking = speaking.contains(&slot);
        let speaker = &conversation.speakers()[slot];
        let colour = raster::colour(&conversation.colour_of(slot, crate::palette::speaker(slot)))
            .unwrap_or([120, 160, 240]);

        // Dimmed by mixing towards the background rather than by an opacity the
        // PNG has no channel for. The same 0.35 the page uses, so the two
        // pictures match.
        let shown = if talking {
            colour
        } else {
            dim(colour, background, 0.35)
        };
        canvas.circle(centre_x, layout.circles_y, layout.radius, shown);

        let label_y = layout.circles_y + layout.radius + (layout.radius * 0.55).max(18.0);
        let room = (step * 0.95) as usize;
        let scale = crate::font::scale_for(&speaker.name, room.max(1));
        let name_ink = if talking {
            ink
        } else {
            dim(ink, background, 0.35)
        };
        if canvas.text_centred(centre_x, label_y as i64, &speaker.name, scale, name_ink) > 0 {
            notes.undrawable.push(speaker.name.clone());
        }

        // The level, on the same rule as the page: the track is always drawn
        // and only the filled part moves, and somebody whose turn it is not
        // shows nothing rather than a small amount.
        let track_width = (layout.radius * 1.9).max(24.0);
        let track_height = (layout.radius * 0.14).clamp(3.0, 10.0);
        let track_x = centre_x - track_width / 2.0;
        let track_y = label_y + (crate::font::HEIGHT * scale) as f32 * 0.55;
        canvas.rounded_rect(track_x, track_y, track_width, track_height, inset);
        if talking && level > 0.0 {
            canvas.rounded_rect(
                track_x,
                track_y,
                track_width * level.clamp(0.0, 1.0),
                track_height,
                colour,
            );
        }
    }

    // The waveform's box, the wave, then the playhead over it.
    canvas.rect(
        layout.wave_x as i64,
        layout.wave_y as i64,
        layout.wave_width as i64,
        layout.wave_height as i64,
        inset,
    );
    canvas.rect(
        layout.wave_x as i64,
        layout.wave_y as i64,
        layout.wave_width as i64,
        1,
        border,
    );
    draw_wave(&mut canvas, envelope, &layout, muted);

    let head_x = layout.wave_x + layout.wave_width * progress as f32;
    canvas.rect(
        head_x.round() as i64 - 1,
        layout.wave_y as i64,
        2,
        layout.wave_height as i64,
        raster::colour(crate::palette::SPEAKERS[0]).unwrap_or([122, 162, 247]),
    );

    notes.undrawable.sort();
    notes.undrawable.dedup();
    Ok((canvas, notes))
}

/// Mix `colour` towards `background`, keeping `amount` of it.
///
/// The page dims a speaker with an opacity. A PNG frame is opaque, so the same
/// effect is the same mix done here, against the colour that would have shown
/// through.
fn dim(colour: Rgb, background: Rgb, amount: f32) -> Rgb {
    let mut out = [0u8; 3];
    for channel in 0..3 {
        let front = colour[channel] as f32;
        let behind = background[channel] as f32;
        out[channel] = (behind + (front - behind) * amount)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    out
}

/// The envelope as filled columns inside the waveform's box.
fn draw_wave(canvas: &mut Canvas, envelope: &Envelope, layout: &page::Layout, colour: Rgb) {
    if envelope.is_empty() || layout.wave_width <= 0.0 {
        return;
    }
    let middle = layout.wave_y + layout.wave_height / 2.0;
    let half = layout.wave_height / 2.0;
    let step = layout.wave_width / envelope.len() as f32;
    for (at, (low, high)) in envelope.min.iter().zip(&envelope.max).enumerate() {
        let x = layout.wave_x + step * at as f32;
        let top = middle - high.clamp(0.0, 1.0) * half;
        let bottom = middle - low.clamp(-1.0, 0.0) * half;
        // At least one pixel: a column of silence is a line on the centre,
        // which is what the page draws, rather than nothing at all.
        let height = (bottom - top).max(1.0);
        canvas.rect(
            x.round() as i64,
            top.round() as i64,
            step.ceil() as i64,
            height.round() as i64,
            colour,
        );
    }
}

/// What a written sequence produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    /// How many picture files were written.
    pub files: usize,
    /// How many frames the video has.
    pub video_frames: u64,
    /// The concat list ffmpeg is given.
    pub list: std::path::PathBuf,
    /// Anything the caller should be told.
    pub notes: Notes,
}

/// Draw and write the whole sequence into `directory`.
///
/// Writes `frame-00000.png` and up, and `frames.txt`, which is the concat list
/// naming each picture and how long it is held. See
/// [`crate::ffmpeg::concat_command`] for what is done with it.
///
/// `progress` is called with the number written and the total, so a front end
/// can show a bar without this module knowing what one is.
pub fn write(
    conversation: &Conversation,
    envelope: &Envelope,
    look: &Look,
    plan_for: &size::Plan,
    directory: &Path,
    mut progress: impl FnMut(usize, usize),
) -> Result<Written, Error> {
    look.checked()?;
    std::fs::create_dir_all(directory).map_err(|why| Error::Write {
        path: directory.to_path_buf(),
        why: why.to_string(),
    })?;

    let sequence = plan(conversation, envelope, look, plan_for.fps.get());
    let total = sequence.frames.len();
    let mut notes = Notes::default();
    // The list is built as the frames are written rather than afterwards, so a
    // run that is cancelled leaves a list describing exactly the files that
    // exist rather than one naming files it never got to.
    let mut list = String::new();

    for (at, frame) in sequence.frames.iter().enumerate() {
        let (canvas, frame_notes) = draw(conversation, envelope, look, frame.at_secs)?;
        notes.undrawable.extend(frame_notes.undrawable);
        let name = format!("frame-{at:05}.png");
        let path = directory.join(&name);
        std::fs::write(&path, canvas.png()).map_err(|why| Error::Write {
            path: path.clone(),
            why: why.to_string(),
        })?;
        // ffmpeg's concat demuxer wants the file, then how long it is shown.
        // Quoted because a directory somebody chose can contain a space.
        list.push_str(&format!("file '{name}'\nduration {:.6}\n", frame.hold_secs));
        progress(at + 1, total);
    }

    // The concat demuxer ignores the duration on the final entry, and drops the
    // last picture entirely without this repeat. It is in every worked example
    // of this format for that reason.
    if let Some(last) = sequence.frames.len().checked_sub(1) {
        list.push_str(&format!("file 'frame-{last:05}.png'\n"));
    }

    let list_path = directory.join("frames.txt");
    std::fs::write(&list_path, list).map_err(|why| Error::Write {
        path: list_path.clone(),
        why: why.to_string(),
    })?;

    notes.undrawable.sort();
    notes.undrawable.dedup();
    Ok(Written {
        files: total,
        video_frames: sequence.video_frames,
        list: list_path,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use veilvoice_conversation::{Speaker, Turn};

    fn conversation() -> Conversation {
        let mut plan = Conversation::new();
        plan.title = Some("Two people".into());
        plan.add_speaker(Speaker::named("Alex")).unwrap();
        plan.add_speaker(Speaker::named("Sam")).unwrap();
        plan.add_turn(Turn {
            start: 0.0,
            end: 2.0,
            speaker: 0,
            text: None,
        })
        .unwrap();
        plan.add_turn(Turn {
            start: 2.0,
            end: 4.0,
            speaker: 1,
            text: None,
        })
        .unwrap();
        plan
    }

    fn envelope() -> Envelope {
        let samples: Vec<f32> = (0..48_000 * 4)
            .map(|i| (i as f32 / 200.0).sin() * 0.7)
            .collect();
        waveform::envelope(&samples, 200)
    }

    /// A long recording writes far fewer pictures than it has frames.
    ///
    /// This is the whole reason the sequence is planned rather than drawn at
    /// the frame rate. Ten minutes at thirty is 18,000 frames; the playhead
    /// crosses a 1184-pixel waveform in that time, so it moves a pixel every
    /// fifteen frames and the other fourteen are the picture that was already
    /// there.
    #[test]
    fn a_long_recording_holds_most_of_its_frames() {
        let mut long = Conversation::new();
        long.add_speaker(Speaker::named("Alex")).unwrap();
        long.add_turn(Turn {
            start: 0.0,
            end: 600.0,
            speaker: 0,
            text: None,
        })
        .unwrap();

        let sequence = plan(&long, &envelope(), &Look::default(), 30);
        assert_eq!(sequence.video_frames, 18_000, "ten minutes at thirty");
        assert!(
            sequence.saving() > 5.0,
            "only {} pictures saved for {} frames, a saving of {:.1}",
            sequence.frames.len(),
            sequence.video_frames,
            sequence.saving()
        );
    }

    /// A short recording holds nothing, and that is correct.
    ///
    /// The playhead crosses the whole waveform in four seconds, which at
    /// thirty frames is ten pixels a frame, so every frame really is a
    /// different picture. Worth pinning down: a saving that appeared here
    /// would mean the playhead was being drawn in the wrong place.
    #[test]
    fn a_short_recording_draws_every_frame_because_every_frame_differs() {
        let sequence = plan(&conversation(), &envelope(), &Look::default(), 30);
        assert_eq!(sequence.video_frames, 120, "four seconds at thirty");
        assert_eq!(
            sequence.frames.len(),
            120,
            "a four second recording held a frame, so the playhead is not moving"
        );
    }

    /// The holds tile the whole recording with no gap and no overlap.
    ///
    /// A gap is a black flash in the finished video and an overlap is drift
    /// against the audio, and both are the kind of thing only noticed once the
    /// file is played.
    #[test]
    fn the_holds_cover_the_recording_exactly_once() {
        let plan_for = conversation();
        let sequence = plan(&plan_for, &envelope(), &Look::default(), 30);
        let mut at = 0.0f64;
        for frame in &sequence.frames {
            assert!(
                (frame.at_secs - at).abs() < 1e-6,
                "a gap or an overlap at {at}: the next picture starts at {}",
                frame.at_secs
            );
            assert!(frame.hold_secs > 0.0, "a picture held for no time");
            at += frame.hold_secs;
        }
        assert!(
            (at - plan_for.duration()).abs() < 0.05,
            "the pictures cover {at} of a {} second recording",
            plan_for.duration()
        );
    }

    /// Who is lit changes at a turn boundary, so a picture is written there.
    #[test]
    fn a_turn_boundary_always_gets_its_own_picture() {
        let sequence = plan(&conversation(), &envelope(), &Look::default(), 30);
        // The turn changes at two seconds. Some picture must start within one
        // frame of it, or the wrong person is lit for part of a second.
        assert!(
            sequence
                .frames
                .iter()
                .any(|frame| (frame.at_secs - 2.0).abs() <= 1.0 / 30.0 + 1e-6),
            "nothing was drawn at the turn boundary"
        );
    }

    /// The drawing is the size it was asked for, and deterministic.
    #[test]
    fn a_frame_is_the_size_asked_for_and_the_same_every_time() {
        let look = Look::default();
        let once = draw(&conversation(), &envelope(), &look, 1.0).unwrap().0;
        let twice = draw(&conversation(), &envelope(), &look, 1.0).unwrap().0;
        assert_eq!(once.width(), 1280);
        assert_eq!(once.height(), 720);
        assert_eq!(once.png(), twice.png(), "two draws differed");
    }

    /// The speaker with the turn is drawn brighter than the one without.
    #[test]
    fn the_speaker_with_the_turn_is_the_brighter_one() {
        let look = Look::default();
        let plan_for = conversation();
        let layout = page::layout(&look, plan_for.len());
        let step = layout.wave_width / 2.0;

        let canvas = draw(&plan_for, &envelope(), &look, 1.0).unwrap().0;
        let sample = |canvas: &Canvas, slot: usize| {
            let x = (layout.wave_x + step * (slot as f32 + 0.5)) as usize;
            let y = layout.circles_y as usize;
            let pixel = canvas.at(x, y).expect("inside the frame");
            pixel.iter().map(|c| *c as u32).sum::<u32>()
        };

        assert!(
            sample(&canvas, 0) > sample(&canvas, 1),
            "at one second slot 0 has the turn and should be the brighter"
        );

        let later = draw(&plan_for, &envelope(), &look, 3.0).unwrap().0;
        assert!(
            sample(&later, 1) > sample(&later, 0),
            "at three seconds it is the other way round"
        );
    }

    /// A name the face cannot draw is reported rather than quietly boxed.
    #[test]
    fn a_name_that_cannot_be_drawn_is_named() {
        let mut plan_for = Conversation::new();
        plan_for.add_speaker(Speaker::named("Zoë")).unwrap();
        plan_for
            .add_turn(Turn {
                start: 0.0,
                end: 1.0,
                speaker: 0,
                text: None,
            })
            .unwrap();
        let (_, notes) = draw(&plan_for, &envelope(), &Look::default(), 0.5).unwrap();
        assert_eq!(
            notes.undrawable,
            vec!["Zoë".to_string()],
            "the video would have shown a box and said nothing"
        );
    }

    /// A written sequence is files plus a list ffmpeg can read.
    #[test]
    fn writing_produces_the_pictures_and_a_list_that_names_them() {
        let directory =
            std::env::temp_dir().join(format!("veilvoice-frames-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);

        let mut seen = Vec::new();
        let written = write(
            &conversation(),
            &envelope(),
            &Look {
                width: 320,
                height: 180,
                padding: 12,
                ..Look::default()
            },
            &size::Plan::default(),
            &directory,
            |done, total| seen.push((done, total)),
        )
        .expect("the sequence was written");

        assert_eq!(written.files, seen.len(), "progress was reported per file");
        assert!(written.list.is_file());

        let list = std::fs::read_to_string(&written.list).unwrap();
        let named: Vec<&str> = list
            .lines()
            .filter(|line| line.starts_with("file "))
            .collect();
        // One line per picture, plus the repeat of the last that the concat
        // demuxer needs to show it at all.
        assert_eq!(named.len(), written.files + 1);
        for at in 0..written.files {
            let name = format!("frame-{at:05}.png");
            assert!(
                directory.join(&name).is_file(),
                "{name} is in the list and not on disk"
            );
            let png = std::fs::read(directory.join(&name)).unwrap();
            assert_eq!(&png[1..4], b"PNG", "{name} is not a PNG");
        }
        assert!(
            list.contains("duration "),
            "no durations, so every picture would be shown for the same time"
        );

        let _ = std::fs::remove_dir_all(&directory);
    }
}
