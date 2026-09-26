// SPDX-License-Identifier: GPL-3.0-or-later
//! The moving picture an export draws, and the templates it starts from.
//!
//! **Roadmap item 175.** The Browser's video used to be a black frame with the
//! sound under it, which is enough for somewhere that only takes video and is
//! nothing anybody would choose to watch. This is the picture a recording tool
//! draws: a wave that answers the sound as it plays, moving across the frame,
//! in colours chosen to look right in motion.
//!
//! # The whole look is a setting
//!
//! [`Motion`] is every part of it:
//!
//! * **the lettering**: the built-in face, heavier, or none at all
//!   ([`Face`]);
//! * **the colours**: the background and the wave, each a flat colour or a
//!   gradient ([`Fill`]);
//! * **the shape**: bars from a baseline, bars mirrored about the middle, or a
//!   line ([`Style`]);
//! * **how strongly the wave answers the sound** ([`Motion::response`]);
//! * **how many seconds of it are on screen at once**
//!   ([`Motion::window_secs`]);
//! * **how quickly it settles** after a loud moment ([`Motion::settle_secs`]).
//!
//! # Templates, so nobody has to open the advanced half
//!
//! [`templates`] is one for every theme the application already has, built
//! from that theme's own colours, and several more chosen because they look
//! right moving rather than because they match a panel. The default is the
//! application's default theme, and its settings are the ones that looked best
//! at sixty frames a second: high enough quality and smooth enough that the
//! defaults are the answer for most people.
//!
//! # Why a timeline of levels rather than the samples
//!
//! Drawing a frame from the samples means reading every sample on screen, for
//! every frame: six seconds of 48 kHz audio is nearly three hundred thousand
//! samples, sixty times a second. So the recording is reduced once, to a level
//! every five milliseconds with the settling already applied ([`levels`]), and
//! a frame reads a few hundred of those. The smoothing is computed in time
//! order along the whole recording, which is what makes it a property of the
//! sound rather than of the frame rate: the same recording settles the same
//! way at twenty-four frames a second and at sixty.
//!
//! # The same picture on every machine
//!
//! As for everything [`crate::raster`] draws: no system fonts and no
//! dependency, so two people exporting one recording with one template get the
//! same frames.
//!
//! # In plain words
//!
//! This draws the video: a wave that moves with the voice, in a colour scheme
//! you pick from a list or make yourself. The defaults are chosen so that most
//! people never need to change anything.

use crate::font;
use crate::palette::{self, Palette};
use crate::raster::{self, Canvas, Rgb};
use crate::size::Size;
use crate::Error;

/// How the title and the times are lettered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Face {
    /// The built-in five-by-seven face. The default.
    #[default]
    Plain,
    /// The same face, drawn heavier.
    Bold,
    /// No lettering at all: the wave and nothing else.
    None,
}

impl Face {
    /// Every face, in the order they are offered.
    pub const ALL: [Face; 3] = [Face::Plain, Face::Bold, Face::None];

    /// What this is called where it is chosen.
    pub fn label(self) -> &'static str {
        match self {
            Face::Plain => "Plain",
            Face::Bold => "Bold",
            Face::None => "No lettering",
        }
    }
}

/// Which way a gradient runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    /// Left to right. The default for the wave, so its colour follows time.
    #[default]
    Across,
    /// Top to bottom.
    Down,
    /// Top left to bottom right.
    Diagonal,
}

impl Direction {
    /// Every direction, in the order they are offered.
    pub const ALL: [Direction; 3] = [Direction::Across, Direction::Down, Direction::Diagonal];

    /// What this is called where it is chosen.
    pub fn label(self) -> &'static str {
        match self {
            Direction::Across => "Across",
            Direction::Down => "Down",
            Direction::Diagonal => "Diagonal",
        }
    }
}

/// A colour, or two with a gradient between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// One colour.
    Solid(Rgb),
    /// A gradient from one colour to another.
    Gradient {
        /// Where it starts.
        from: Rgb,
        /// Where it ends.
        to: Rgb,
        /// Which way it runs.
        direction: Direction,
    },
}

impl Fill {
    /// The colour at a point, given as a fraction of the width and height.
    pub fn at(&self, x: f32, y: f32) -> Rgb {
        match *self {
            Fill::Solid(colour) => colour,
            Fill::Gradient {
                from,
                to,
                direction,
            } => {
                let t = match direction {
                    Direction::Across => x,
                    Direction::Down => y,
                    Direction::Diagonal => (x + y) / 2.0,
                }
                .clamp(0.0, 1.0);
                mix(from, to, t)
            }
        }
    }

    /// The colour a thumbnail or a picker shows for this fill: its middle.
    pub fn middle(&self) -> Rgb {
        self.at(0.5, 0.5)
    }
}

/// The shape the wave is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Style {
    /// Bars mirrored about the middle, which is what a voice looks like.
    #[default]
    Mirror,
    /// Bars standing on a baseline, like a level meter.
    Bars,
    /// A line above and below the middle, with nothing filled.
    Line,
}

impl Style {
    /// Every style, in the order they are offered.
    pub const ALL: [Style; 3] = [Style::Mirror, Style::Bars, Style::Line];

    /// What this is called where it is chosen.
    pub fn label(self) -> &'static str {
        match self {
            Style::Mirror => "Mirrored bars",
            Style::Bars => "Bars",
            Style::Line => "Line",
        }
    }
}

/// The smallest and largest [`Motion::response`].
pub const RESPONSE_RANGE: (f32, f32) = (0.25, 4.0);
/// The shortest and longest [`Motion::window_secs`].
pub const WINDOW_RANGE: (f32, f32) = (1.0, 30.0);
/// The shortest and longest [`Motion::settle_secs`].
pub const SETTLE_RANGE: (f32, f32) = (0.0, 1.0);

/// The whole look of a video.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Behind everything.
    pub background: Fill,
    /// The wave.
    pub wave: Fill,
    /// The lettering.
    pub ink: Rgb,
    /// How the lettering is drawn, or whether it is.
    pub face: Face,
    /// The shape of the wave.
    pub style: Style,
    /// How strongly the wave answers the sound. One is as recorded; two makes
    /// a quiet voice fill the frame; a half keeps a loud one inside it.
    pub response: f32,
    /// How many seconds of sound are on screen at once.
    pub window_secs: f32,
    /// How long the wave takes to fall back after a loud moment, in seconds.
    /// Zero follows every peak exactly and flickers; a second glides.
    pub settle_secs: f32,
}

impl Default for Motion {
    fn default() -> Self {
        from_palette(palette::default_palette())
    }
}

impl Motion {
    /// Refuse settings outside the ranges the controls offer.
    ///
    /// A front end clamps its sliders to the same ranges, so this fires only
    /// for a caller that built a [`Motion`] by hand, and says which number.
    pub fn checked(&self) -> Result<(), Error> {
        let within = |value: f32, (low, high): (f32, f32), what: &str, unit: &str| {
            if value.is_finite() && (low..=high).contains(&value) {
                Ok(())
            } else {
                Err(Error::Malformed(format!(
                    "{what} of {value}{unit} is outside {low}{unit} to {high}{unit}"
                )))
            }
        };
        within(self.response, RESPONSE_RANGE, "a response", "")?;
        within(self.window_secs, WINDOW_RANGE, "a window", " seconds")?;
        within(
            self.settle_secs,
            SETTLE_RANGE,
            "a settling time",
            " seconds",
        )?;
        Ok(())
    }
}

/// A named starting point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Template {
    /// Stable identifier. A theme's template has the theme's identifier.
    pub id: &'static str,
    /// What a picker shows.
    pub name: &'static str,
    /// The look itself.
    pub motion: Motion,
}

/// The template a theme makes: its own background, its two accents as the
/// wave, its own text colour.
fn from_palette(palette: &Palette) -> Motion {
    let hex = |value: &str| raster::colour(value).unwrap_or([128, 128, 128]);
    Motion {
        background: Fill::Gradient {
            from: hex(palette.bg),
            to: hex(palette.bg_soft),
            direction: Direction::Down,
        },
        wave: Fill::Gradient {
            from: hex(palette.accent),
            to: hex(palette.accent_2),
            direction: Direction::Across,
        },
        ink: hex(palette.fg),
        face: Face::Plain,
        style: Style::Mirror,
        response: 1.0,
        window_secs: 6.0,
        settle_secs: 0.15,
    }
}

/// The templates that are not a theme, chosen for how they look moving.
const EXTRAS: &[(&str, &str, Motion)] = &[
    (
        "neon",
        "Neon",
        Motion {
            background: Fill::Solid([11, 11, 18]),
            wave: Fill::Gradient {
                from: [0, 245, 212],
                to: [241, 91, 181],
                direction: Direction::Across,
            },
            ink: [240, 240, 255],
            face: Face::Bold,
            style: Style::Bars,
            response: 1.3,
            window_secs: 4.0,
            settle_secs: 0.08,
        },
    ),
    (
        "sunset",
        "Sunset",
        Motion {
            background: Fill::Gradient {
                from: [43, 16, 85],
                to: [117, 30, 90],
                direction: Direction::Down,
            },
            wave: Fill::Gradient {
                from: [255, 209, 102],
                to: [239, 71, 111],
                direction: Direction::Down,
            },
            ink: [255, 236, 214],
            face: Face::Plain,
            style: Style::Mirror,
            response: 1.1,
            window_secs: 6.0,
            settle_secs: 0.18,
        },
    ),
    (
        "ocean",
        "Ocean",
        Motion {
            background: Fill::Gradient {
                from: [3, 27, 52],
                to: [10, 77, 104],
                direction: Direction::Diagonal,
            },
            wave: Fill::Gradient {
                from: [94, 231, 223],
                to: [180, 144, 202],
                direction: Direction::Across,
            },
            ink: [224, 247, 250],
            face: Face::Plain,
            style: Style::Line,
            response: 1.2,
            window_secs: 8.0,
            settle_secs: 0.25,
        },
    ),
    (
        "ember",
        "Ember",
        Motion {
            background: Fill::Solid([18, 11, 8]),
            wave: Fill::Gradient {
                from: [236, 159, 5],
                to: [255, 78, 0],
                direction: Direction::Down,
            },
            ink: [255, 228, 196],
            face: Face::Bold,
            style: Style::Bars,
            response: 1.2,
            window_secs: 5.0,
            settle_secs: 0.12,
        },
    ),
    (
        "aurora",
        "Aurora",
        Motion {
            background: Fill::Gradient {
                from: [15, 32, 39],
                to: [32, 58, 67],
                direction: Direction::Down,
            },
            wave: Fill::Gradient {
                from: [168, 255, 120],
                to: [120, 255, 214],
                direction: Direction::Across,
            },
            ink: [220, 255, 240],
            face: Face::Plain,
            style: Style::Mirror,
            response: 1.0,
            window_secs: 10.0,
            settle_secs: 0.3,
        },
    ),
    (
        "print",
        "Print (light)",
        Motion {
            background: Fill::Solid([247, 245, 240]),
            wave: Fill::Solid([27, 27, 27]),
            ink: [27, 27, 27],
            face: Face::Plain,
            style: Style::Mirror,
            response: 0.9,
            window_secs: 6.0,
            settle_secs: 0.2,
        },
    ),
    (
        "mono",
        "Mono",
        Motion {
            background: Fill::Solid([0, 0, 0]),
            wave: Fill::Solid([255, 255, 255]),
            ink: [255, 255, 255],
            face: Face::None,
            style: Style::Bars,
            response: 1.0,
            window_secs: 6.0,
            settle_secs: 0.1,
        },
    ),
];

/// Every template: one per theme, in the themes' own order, then the extras.
pub fn templates() -> Vec<Template> {
    let mut out: Vec<Template> = palette::PALETTES
        .iter()
        .map(|palette| Template {
            id: palette.id,
            name: palette.name,
            motion: from_palette(palette),
        })
        .collect();
    out.extend(
        EXTRAS
            .iter()
            .map(|&(id, name, motion)| Template { id, name, motion }),
    );
    out
}

/// The template with this identifier.
pub fn template(id: &str) -> Option<Template> {
    templates().into_iter().find(|template| template.id == id)
}

/// Seconds between two entries of a [`Levels`] timeline.
pub const HOP_SECS: f64 = 0.005;

/// How loud the recording is, every five milliseconds, already settled.
#[derive(Clone, Debug, PartialEq)]
pub struct Levels {
    values: Vec<f32>,
    duration_secs: f64,
}

impl Levels {
    /// How long the recording is.
    pub fn duration_secs(&self) -> f64 {
        self.duration_secs
    }

    /// The loudest settled level between two moments, from 0.0 to 1.0.
    ///
    /// Before the start and after the end is silence, which is what makes the
    /// wave enter from the right at the beginning rather than starting full.
    pub fn peak(&self, from_secs: f64, to_secs: f64) -> f32 {
        if self.values.is_empty() || to_secs <= 0.0 || from_secs >= self.duration_secs {
            return 0.0;
        }
        let first = (from_secs.max(0.0) / HOP_SECS).floor() as usize;
        let last = ((to_secs / HOP_SECS).ceil() as usize).min(self.values.len());
        self.values
            .get(first..last.max(first + 1).min(self.values.len()))
            .map(|run| run.iter().copied().fold(0.0f32, f32::max))
            .unwrap_or(0.0)
    }
}

/// Reduce a recording to the timeline a picture is drawn from.
///
/// Each entry is the loudest sample in its five milliseconds, scaled by the
/// response and put through a curve so that a quiet voice still moves the
/// picture, then settled: it rises at once and falls back over
/// [`Motion::settle_secs`].
pub fn levels(samples: &[f32], sample_rate: u32, motion: &Motion) -> Levels {
    let rate = sample_rate.max(1) as f64;
    let duration_secs = samples.len() as f64 / rate;
    let hop = ((rate * HOP_SECS).round() as usize).max(1);
    let fall = if motion.settle_secs > 0.0 {
        (-(HOP_SECS as f32) / motion.settle_secs).exp()
    } else {
        0.0
    };
    let mut values = Vec::with_capacity(samples.len() / hop + 1);
    let mut held = 0.0f32;
    for chunk in samples.chunks(hop) {
        let peak = chunk
            .iter()
            .filter(|sample| sample.is_finite())
            .fold(0.0f32, |most, sample| most.max(sample.abs()));
        // A square-root-ish curve: speech sits well below full scale, and a
        // straight line would leave most of it as a flat stripe.
        let shaped = (peak * motion.response).clamp(0.0, 1.0).powf(0.6);
        held = if shaped >= held {
            shaped
        } else {
            shaped + (held - shaped) * fall
        };
        values.push(held);
    }
    Levels {
        values,
        duration_secs,
    }
}

/// Where everything on a frame goes.
#[derive(Clone, Copy, Debug)]
struct Layout {
    left: f32,
    right: f32,
    middle: f32,
    half: f32,
    pitch: f32,
    bar: f32,
    title_top: i64,
    title_scale: usize,
    foot_top: i64,
    foot_scale: usize,
}

/// Where the wave, the title and the progress line go on a frame of this size.
fn layout(width: usize, height: usize, title: Option<&str>) -> Layout {
    let w = width as f32;
    let h = height as f32;
    let pad = (w * 0.06).round();
    let pitch = (w / 160.0).max(3.0);
    let foot_scale = ((h / 360.0).round() as usize).max(1);
    let title_scale = match title {
        Some(text) => {
            font::scale_for(text, (w - pad * 2.0) as usize).min(((h / 90.0) as usize).max(1))
        }
        None => 1,
    };
    Layout {
        left: pad,
        right: w - pad,
        middle: h * 0.54,
        half: h * 0.26,
        pitch,
        bar: (pitch * 0.62).max(2.0),
        title_top: (h * 0.1).round() as i64,
        title_scale,
        foot_top: (h - pad * 0.5 - (font::HEIGHT * foot_scale) as f32 * 1.5).round() as i64,
        foot_scale,
    }
}

/// Draws the frames of one export.
///
/// Built once, then asked for each frame in turn. The background, the title
/// and the wave's colours are painted once here and copied for each frame,
/// because a gradient is the same on every frame and repainting it sixty times
/// a second is most of the work of drawing one.
pub struct Renderer {
    motion: Motion,
    width: usize,
    height: usize,
    fps: u32,
    base: Canvas,
    colour: Canvas,
    levels: Levels,
    layout: Layout,
    undrawable: Vec<String>,
}

impl Renderer {
    /// A renderer for `levels` at `size`, `fps` frames a second, with `title`
    /// lettered at the top unless the face is [`Face::None`].
    pub fn new(
        motion: &Motion,
        size: Size,
        fps: u32,
        levels: Levels,
        title: Option<&str>,
    ) -> Result<Self, Error> {
        motion.checked()?;
        let width = size.width() as usize;
        let height = size.height() as usize;
        let title = title.filter(|text| !text.trim().is_empty() && motion.face != Face::None);
        let layout = layout(width, height, title);

        let mut base = Canvas::new(width, height, motion.background.middle());
        let mut colour = Canvas::new(width, height, motion.wave.middle());
        let (wf, hf) = ((width.max(2) - 1) as f32, (height.max(2) - 1) as f32);
        for y in 0..height {
            for x in 0..width {
                let (fx, fy) = (x as f32 / wf, y as f32 / hf);
                base.set(x, y, motion.background.at(fx, fy));
                colour.set(x, y, motion.wave.at(fx, fy));
            }
        }

        let mut undrawable = Vec::new();
        if let Some(text) = title {
            let missing = letter(
                &mut base,
                motion.face,
                width as f32 / 2.0,
                layout.title_top,
                text,
                layout.title_scale,
                motion.ink,
            );
            if missing > 0 {
                undrawable.push(text.to_string());
            }
        }

        Ok(Self {
            motion: *motion,
            width,
            height,
            fps: fps.max(1),
            base,
            colour,
            levels,
            layout,
            undrawable,
        })
    }

    /// How many frames the video has: the length times the rate, rounded up
    /// so the last partial frame is still shown.
    pub fn frames(&self) -> u64 {
        ((self.levels.duration_secs() * self.fps as f64).ceil() as u64).max(1)
    }

    /// Text that came out as boxes, so a front end can say so before the
    /// render rather than after.
    pub fn undrawable(&self) -> &[String] {
        &self.undrawable
    }

    /// The size of one frame in bytes, as `rgb24`.
    pub fn frame_bytes(&self) -> usize {
        self.width * self.height * 3
    }

    /// Draw frame `index` into `out`, which is resized to fit.
    pub fn frame(&self, index: u64, out: &mut Vec<u8>) {
        let canvas = self.still(index);
        out.clear();
        out.extend_from_slice(canvas.rgb());
    }

    /// Draw frame `index` as a canvas, for a still or a test.
    pub fn still(&self, index: u64) -> Canvas {
        let at_secs = index as f64 / self.fps as f64;
        let mut canvas = self.base.clone();
        self.draw_wave(&mut canvas, at_secs);
        self.draw_foot(&mut canvas, at_secs);
        canvas
    }

    /// The wave as it stands at `at_secs`, newest sound at the right.
    fn draw_wave(&self, canvas: &mut Canvas, at_secs: f64) {
        let l = self.layout;
        let span = (l.right - l.left).max(1.0);
        let window = self.motion.window_secs as f64;
        let slots = (span / l.pitch).ceil() as i64 + 1;
        let slot_secs = window / (span / l.pitch) as f64;
        // Slots are fixed in time, so a bar keeps its height as it moves and
        // slides by fractions of a pixel between frames: that is what makes the
        // motion smooth rather than a picture redrawn in steps.
        let newest = (at_secs / slot_secs).floor() as i64;
        // The reach of the slot to the right of this one, so a line can be
        // joined to it rather than drawn as a row of separate dashes.
        let mut newer: Option<f32> = None;
        for back in 0..slots {
            let slot = newest - back;
            let start = slot as f64 * slot_secs;
            let value = self.levels.peak(start, start + slot_secs);
            let age = (at_secs - start) / slot_secs;
            let x_right = l.right - (age as f32 - 1.0) * l.pitch;
            if x_right <= l.left {
                break;
            }
            match self.motion.style {
                Style::Mirror => {
                    let (x0, x1) = (x_right - l.bar, x_right);
                    let reach = (value * l.half).max(1.0);
                    self.paint(canvas, x0, x1, l.middle - reach, l.middle + reach);
                }
                Style::Bars => {
                    let (x0, x1) = (x_right - l.bar, x_right);
                    let base = l.middle + l.half;
                    let reach = (value * l.half * 2.0).max(1.5);
                    self.paint(canvas, x0, x1, base - reach, base);
                }
                Style::Line => {
                    let reach = value * l.half;
                    let thick = (self.height as f32 / 360.0).max(1.5);
                    let (x0, x1) = (x_right - l.pitch, x_right);
                    for sign in [-1.0f32, 1.0] {
                        let y = l.middle + sign * reach;
                        self.paint(canvas, x0, x1, y - thick, y + thick);
                        if let Some(next) = newer {
                            // The step up or down to the next slot, at the
                            // boundary between the two.
                            let other = l.middle + sign * next;
                            let (top, bottom) = (y.min(other), y.max(other));
                            self.paint(canvas, x1 - thick, x1 + thick, top - thick, bottom + thick);
                        }
                    }
                    newer = Some(reach);
                }
            }
        }
    }

    /// The elapsed time, the length, and a progress line under them.
    fn draw_foot(&self, canvas: &mut Canvas, at_secs: f64) {
        let l = self.layout;
        let duration = self.levels.duration_secs().max(1e-9);
        let progress = (at_secs / duration).clamp(0.0, 1.0) as f32;
        let track = (self.height as f32 / 540.0).max(1.0).round();
        let y = l.foot_top as f32 - track * 4.0;
        let faint = mix(self.motion.background.middle(), self.motion.ink, 0.2);
        canvas.rect(
            l.left as i64,
            y as i64,
            (l.right - l.left) as i64,
            track as i64,
            faint,
        );
        self.paint(
            canvas,
            l.left,
            l.left + (l.right - l.left) * progress,
            y,
            y + track,
        );
        if self.motion.face == Face::None {
            return;
        }
        let text = format!("{} / {}", clock(at_secs), clock(duration));
        let width = font::width_of(&text, l.foot_scale) as f32;
        letter(
            canvas,
            self.motion.face,
            l.right - width / 2.0,
            l.foot_top,
            &text,
            l.foot_scale,
            mix(self.motion.background.middle(), self.motion.ink, 0.7),
        );
    }

    /// Fill a rectangle with the wave's colours, with soft edges where it
    /// falls between pixels.
    fn paint(&self, canvas: &mut Canvas, x0: f32, x1: f32, y0: f32, y1: f32) {
        let x0 = x0.max(self.layout.left);
        let x1 = x1.min(self.layout.right);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let cols = (x0.floor().max(0.0) as usize)..(x1.ceil().min(self.width as f32) as usize);
        let rows = (y0.floor().max(0.0) as usize)..(y1.ceil().min(self.height as f32) as usize);
        for y in rows {
            let cover_y = (y1.min(y as f32 + 1.0) - y0.max(y as f32)).clamp(0.0, 1.0);
            for x in cols.clone() {
                let cover_x = (x1.min(x as f32 + 1.0) - x0.max(x as f32)).clamp(0.0, 1.0);
                let alpha = cover_x * cover_y;
                if alpha <= 0.0 {
                    continue;
                }
                let (Some(want), Some(had)) = (self.colour.at(x, y), canvas.at(x, y)) else {
                    continue;
                };
                canvas.set(x, y, mix(had, want, alpha));
            }
        }
    }
}

/// Letter `text` centred on `centre_x`, heavier for [`Face::Bold`].
fn letter(
    canvas: &mut Canvas,
    face: Face,
    centre_x: f32,
    top: i64,
    text: &str,
    scale: usize,
    ink: Rgb,
) -> usize {
    match face {
        Face::None => 0,
        Face::Plain => canvas.text_centred(centre_x, top, text, scale, ink),
        Face::Bold => {
            // The same glyphs drawn twice, the second a fraction of a glyph
            // pixel to the right: heavier strokes, the same shapes, and still
            // no font file.
            let nudge = (scale as f32 / 2.0).max(1.0);
            canvas.text_centred(centre_x + nudge, top, text, scale, ink);
            canvas.text_centred(centre_x, top, text, scale, ink)
        }
    }
}

/// A picture of a whole recording, for a list of them.
///
/// **Roadmap item 175's thumbnails.** The whole recording rather than a moment
/// of it, drawn with the same template the export will use, so the list shows
/// what each recording is shaped like and what its video will look like in one
/// picture. Nothing moves, so there is no settling and no lettering.
pub fn thumbnail(samples: &[f32], motion: &Motion, width: usize, height: usize) -> Canvas {
    let width = width.max(8);
    let height = height.max(8);
    let mut canvas = Canvas::new(width, height, motion.background.middle());
    let (wf, hf) = ((width - 1) as f32, (height - 1) as f32);
    for y in 0..height {
        for x in 0..width {
            canvas.set(x, y, motion.background.at(x as f32 / wf, y as f32 / hf));
        }
    }
    let pitch = 3usize;
    let columns = (width / pitch).max(1);
    let envelope = crate::waveform::envelope(samples, columns);
    let middle = height as f32 / 2.0;
    let half = height as f32 * 0.4;
    for column in 0..envelope.len() {
        let peak = envelope.max[column].max(-envelope.min[column]);
        let value = (peak * motion.response).clamp(0.0, 1.0).powf(0.6);
        let reach = (value * half).max(0.5);
        let x = column * pitch;
        let (top, bottom) = match motion.style {
            Style::Bars => (middle + half - reach * 2.0, middle + half),
            Style::Mirror | Style::Line => (middle - reach, middle + reach),
        };
        for y in (top.floor().max(0.0) as usize)..(bottom.ceil().min(height as f32) as usize) {
            for dx in 0..(pitch - 1) {
                let colour = motion.wave.at((x + dx) as f32 / wf, y as f32 / hf);
                canvas.set(x + dx, y, colour);
            }
        }
    }
    canvas
}

/// `m:ss`, or `h:mm:ss` past an hour.
fn clock(seconds: f64) -> String {
    let whole = seconds.max(0.0) as u64;
    let (h, m, s) = (whole / 3600, whole / 60 % 60, whole % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Mix from `a` towards `b` by `t`.
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let mut out = [0u8; 3];
    for channel in 0..3 {
        let from = a[channel] as f32;
        let to = b[channel] as f32;
        out[channel] = (from + (to - from) * t).round().clamp(0.0, 255.0) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::Preset;

    fn tone(seconds: f32, rate: u32, loud: impl Fn(f32) -> f32) -> Vec<f32> {
        (0..(seconds * rate as f32) as usize)
            .map(|at| {
                let t = at as f32 / rate as f32;
                loud(t) * (t * 220.0 * std::f32::consts::TAU).sin()
            })
            .collect()
    }

    fn small() -> Size {
        Size::new(480, 270).unwrap()
    }

    #[test]
    fn there_is_a_template_for_every_theme_and_some_more() {
        let all = templates();
        for palette in palette::PALETTES {
            assert!(all.iter().any(|t| t.id == palette.id), "{}", palette.id);
        }
        assert!(all.len() >= palette::PALETTES.len() + 5);
        let mut ids: Vec<_> = all.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "identifiers must be unique");
        for template in &all {
            template.motion.checked().unwrap();
        }
        assert_eq!(
            Motion::default(),
            template(palette::DEFAULT_ID).unwrap().motion
        );
    }

    #[test]
    fn settings_outside_the_offered_ranges_are_refused() {
        for bad in [
            Motion {
                response: 0.0,
                ..Motion::default()
            },
            Motion {
                window_secs: 60.0,
                ..Motion::default()
            },
            Motion {
                settle_secs: f32::NAN,
                ..Motion::default()
            },
        ] {
            assert!(bad.checked().is_err());
            assert!(Renderer::new(&bad, small(), 30, levels(&[], 48_000, &bad), None).is_err());
        }
    }

    #[test]
    fn a_louder_response_draws_a_taller_wave() {
        let samples = tone(2.0, 8_000, |_| 0.2);
        let quiet = levels(&samples, 8_000, &Motion::default());
        let loud = levels(
            &samples,
            8_000,
            &Motion {
                response: 3.0,
                ..Motion::default()
            },
        );
        assert!(loud.peak(0.5, 1.5) > quiet.peak(0.5, 1.5));
    }

    #[test]
    fn settling_holds_a_peak_for_as_long_as_it_was_asked_to() {
        // Loud for half a second, then silence.
        let samples = tone(1.0, 8_000, |t| if t < 0.5 { 0.8 } else { 0.0 });
        let snap = levels(
            &samples,
            8_000,
            &Motion {
                settle_secs: 0.0,
                ..Motion::default()
            },
        );
        let glide = levels(
            &samples,
            8_000,
            &Motion {
                settle_secs: 0.5,
                ..Motion::default()
            },
        );
        assert!(snap.peak(0.6, 0.62) < 0.01, "no settling falls at once");
        assert!(glide.peak(0.6, 0.62) > 0.3, "a long settle is still high");
        assert!(glide.peak(0.98, 1.0) < glide.peak(0.6, 0.62));
    }

    #[test]
    fn settling_is_a_property_of_the_sound_not_the_frame_rate() {
        let samples = tone(1.0, 8_000, |t| if t < 0.3 { 0.9 } else { 0.1 });
        let motion = Motion::default();
        let a =
            Renderer::new(&motion, small(), 24, levels(&samples, 8_000, &motion), None).unwrap();
        let b =
            Renderer::new(&motion, small(), 60, levels(&samples, 8_000, &motion), None).unwrap();
        // The same moment, 0.5 seconds in, at two rates.
        assert_eq!(a.still(12), b.still(30));
    }

    #[test]
    fn the_wave_moves_between_frames_and_silence_draws_almost_nothing() {
        let motion = Motion::default();
        let samples = tone(3.0, 8_000, |t| 0.3 + 0.3 * (t * 3.0).sin().abs());
        let renderer = Renderer::new(
            &motion,
            small(),
            30,
            levels(&samples, 8_000, &motion),
            Some("Take"),
        )
        .unwrap();
        assert_eq!(renderer.frames(), 90);
        assert_ne!(
            renderer.still(40),
            renderer.still(41),
            "the picture must move"
        );

        let silent = vec![0.0f32; 8_000 * 2];
        let quiet =
            Renderer::new(&motion, small(), 30, levels(&silent, 8_000, &motion), None).unwrap();
        let loud = &renderer;
        let lit = |canvas: &Canvas| {
            let wave = motion.wave.middle();
            (0..canvas.height())
                .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let px = canvas.at(x, y).unwrap();
                    px.iter()
                        .zip(wave)
                        .map(|(a, b)| (*a as i32 - b as i32).abs())
                        .sum::<i32>()
                        < 90
                })
                .count()
        };
        assert!(lit(&loud.still(60)) > lit(&quiet.still(30)) * 5);
    }

    #[test]
    fn a_frame_is_exactly_the_bytes_ffmpeg_is_told_to_expect() {
        let motion = template("neon").unwrap().motion;
        let size = Preset::Hd720.size();
        let renderer =
            Renderer::new(&motion, size, 60, levels(&[0.1; 800], 8_000, &motion), None).unwrap();
        let mut out = Vec::new();
        renderer.frame(0, &mut out);
        assert_eq!(out.len(), 1280 * 720 * 3);
        assert_eq!(out.len(), renderer.frame_bytes());
    }

    #[test]
    fn a_title_the_face_cannot_draw_is_reported() {
        let motion = Motion::default();
        let renderer = Renderer::new(
            &motion,
            small(),
            30,
            levels(&[0.0; 80], 8_000, &motion),
            Some("Жанна"),
        )
        .unwrap();
        assert_eq!(renderer.undrawable(), ["Жанна"]);
        let none = Motion {
            face: Face::None,
            ..motion
        };
        let silent = Renderer::new(
            &none,
            small(),
            30,
            levels(&[0.0; 80], 8_000, &none),
            Some("Жанна"),
        )
        .unwrap();
        assert!(
            silent.undrawable().is_empty(),
            "nothing is lettered, nothing is boxed"
        );
    }

    #[test]
    fn every_style_and_face_draws_without_trouble() {
        let samples = tone(1.0, 8_000, |_| 0.5);
        for style in Style::ALL {
            for face in Face::ALL {
                let motion = Motion {
                    style,
                    face,
                    ..Motion::default()
                };
                let renderer = Renderer::new(
                    &motion,
                    small(),
                    30,
                    levels(&samples, 8_000, &motion),
                    Some("A"),
                )
                .unwrap();
                let _ = renderer.still(15);
            }
        }
    }

    #[test]
    fn a_gradient_runs_from_one_end_to_the_other() {
        let fill = Fill::Gradient {
            from: [0, 0, 0],
            to: [200, 100, 50],
            direction: Direction::Down,
        };
        assert_eq!(fill.at(0.9, 0.0), [0, 0, 0]);
        assert_eq!(fill.at(0.1, 1.0), [200, 100, 50]);
        assert_eq!(fill.at(0.0, 0.5), [100, 50, 25]);
    }

    #[test]
    fn a_thumbnail_shows_where_the_sound_is() {
        let motion = template("mono").unwrap().motion;
        // Sound in the first half, silence in the second.
        let samples = tone(2.0, 8_000, |t| if t < 1.0 { 0.8 } else { 0.0 });
        let thumb = thumbnail(&samples, &motion, 120, 40);
        let white_in = |from: usize, to: usize| {
            (from..to)
                .flat_map(|x| (0..40).map(move |y| (x, y)))
                .filter(|&(x, y)| thumb.at(x, y) == Some([255, 255, 255]))
                .count()
        };
        assert!(white_in(0, 55) > white_in(65, 120) * 10);
        assert_eq!(thumb.rgb().len(), 120 * 40 * 3);
    }

    #[test]
    fn the_clock_reads_like_a_player() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(75.4), "1:15");
        assert_eq!(clock(3_725.0), "1:02:05");
    }
}
