// SPDX-License-Identifier: GPL-3.0-or-later
//! The picture: one still for a preview, and one page that plays.
//!
//! # Why a page rather than a video file
//!
//! It needs nothing installed, it contacts nothing, and it opens on every
//! browser and every phone. A video file needs an encoder this project does not
//! ship — see [`crate::ffmpeg`] — so the page is the thing that always exists
//! and the file is the extra.
//!
//! It is also the honest artefact for this particular job. What is being drawn
//! is a waveform, some circles and a title: flat colour, hard edges and text.
//! That is what vector graphics are for, and it stays sharp at any size on any
//! screen rather than being resampled from a fixed grid of pixels.
//!
//! # The layout, and why the padding is a setting
//!
//! A title across the top, a row of circles under it — one per speaker, in
//! their colour, with their name beneath — and the waveform along the bottom
//! with a line that moves through it. [`Look::padding`] is a setting because
//! the same picture is wanted at very different sizes: a thumbnail wants little
//! and a full-screen render wants a lot, and a fixed margin looks wrong at one
//! end or the other.
//!
//! # Everything user-supplied is escaped
//!
//! Names and titles are typed by a person and end up inside markup. They are
//! escaped on the way in, every time, through one function. A name containing
//! `</text>` would otherwise end the element it is in and the rest of the file
//! would be whatever that person wrote — which is a nuisance in a local file
//! and a real problem in one that gets sent to somebody.
//!
//! # The script, and what happens without it
//!
//! Lighting the circle of whoever is speaking means knowing where the audio has
//! got to, and only the audio element knows that. There is a small inline
//! script — no file, no library, no network — and a `<noscript>` saying what it
//! does. Without it the audio plays, the subtitles appear, the waveform is
//! drawn and the circles simply stay dim.
//!
//! # In plain words
//!
//! Draws the picture: a waveform, a circle for each person that lights up when it
//! is their turn, and the names.
//!
//! It produces two things. A still, so you can see what you will get before
//! anything is rendered, and a self-contained web page that plays the veiled audio
//! with the picture moving along beside it.
//!
//! A page rather than a video file, because a page needs nothing installed to
//! watch and can be opened by anybody. If you want an actual video file, the
//! command to make one is printed for you.

use crate::palette;
use crate::waveform::{self, Envelope};
use crate::Error;
use std::path::{Path, PathBuf};
use veilvoice_conversation::Conversation;

/// What the picture looks like.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Look {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The margin around everything, in pixels.
    pub padding: u32,
    /// What is behind it all.
    pub background: Background,
    /// The colour scheme, from the same nine the website and the desktop
    /// application offer.
    ///
    /// A reference rather than an identifier, so a `Look` that exists is a
    /// `Look` whose palette exists: the name is resolved once, where the user
    /// typed it, and every drawing after that is looking at real colours.
    pub palette: &'static palette::Palette,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            padding: 48,
            background: Background::Colour(palette::BG.to_string()),
            palette: palette::default_palette(),
        }
    }
}

/// What sits behind the picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Background {
    /// A flat colour, as `#rrggbb`.
    Colour(String),
    /// An image file, embedded so the page stays self-contained.
    Image(PathBuf),
}

impl Look {
    /// Black, for somebody who wants the plainest possible picture.
    pub fn black(self) -> Self {
        Self {
            background: Background::Colour("#000000".to_string()),
            ..self
        }
    }

    /// The same look in another palette.
    ///
    /// The background follows unless it was set to something specific. A
    /// reader who asked for Gruvbox and got a Gruvbox picture on a Tokyo Night
    /// page would reasonably call that a bug; a reader who asked for Gruvbox
    /// *and* `--background #123456` asked for two things and gets both.
    pub fn themed(self, palette: &'static palette::Palette) -> Self {
        let followed = matches!(
            &self.background,
            Background::Colour(colour) if colour == self.palette.bg
        );
        Self {
            background: if followed {
                Background::Colour(palette.bg.to_string())
            } else {
                self.background
            },
            palette,
            ..self
        }
    }

    /// Whether these numbers describe a picture that can be drawn.
    ///
    /// Checked rather than clamped: a caller who asked for a 10-pixel-wide
    /// render of nine speakers meant something, and quietly producing an
    /// illegible one would be answering a question they did not ask.
    pub fn checked(&self) -> Result<(), Error> {
        if self.width < 320 || self.height < 180 {
            return Err(Error::Malformed(format!(
                "{}x{} is too small to draw a waveform and a row of speakers in; 320x180 \
                 is the floor",
                self.width, self.height
            )));
        }
        if self.width > 7680 || self.height > 4320 {
            return Err(Error::Malformed(format!(
                "{}x{} is larger than 8K, which is past anything that will play",
                self.width, self.height
            )));
        }
        // Two paddings plus something to draw in. A padding of half the width
        // leaves nothing, and the picture would silently be empty.
        if self.padding * 2 + 160 > self.width || self.padding * 2 + 120 > self.height {
            return Err(Error::Malformed(format!(
                "a padding of {} leaves no room inside {}x{}",
                self.padding, self.width, self.height
            )));
        }
        if let Background::Colour(colour) = &self.background {
            if palette::rgb(colour).is_none() {
                return Err(Error::Malformed(format!(
                    "{colour:?} is not a #rrggbb colour"
                )));
            }
        }
        Ok(())
    }
}

/// Where each part of the picture goes.
///
/// Worked out once and shared by the still and the player, so the preview shows
/// the layout the page will use rather than one that merely resembles it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Baseline of the title.
    pub title_y: f32,
    /// Centre height of the row of speaker circles.
    pub circles_y: f32,
    /// Radius of a speaker circle.
    pub radius: f32,
    /// Left edge of the waveform.
    pub wave_x: f32,
    /// Top edge of the waveform.
    pub wave_y: f32,
    /// Width of the waveform.
    pub wave_width: f32,
    /// Height of the waveform.
    pub wave_height: f32,
}

/// Work out the layout for a look and a number of speakers.
pub fn layout(look: &Look, speakers: usize) -> Layout {
    let padding = look.padding as f32;
    let width = look.width as f32;
    let height = look.height as f32;
    let inner_width = width - padding * 2.0;
    let inner_height = height - padding * 2.0;

    // The waveform takes the bottom quarter, the title the top eighth, and the
    // circles have what is left.
    let wave_height = (inner_height * 0.25).max(40.0);
    let title_y = padding + inner_height * 0.10;
    let wave_y = height - padding - wave_height;
    let circles_y = (title_y + wave_y) / 2.0;

    // A circle per speaker across the width, with a gap between each. Capped so
    // two speakers do not each get a circle taller than the space they are in.
    let count = speakers.max(1) as f32;
    let by_width = inner_width / (count * 2.6);
    let by_height = (wave_y - title_y) * 0.28;
    let radius = by_width.min(by_height).max(8.0);

    Layout {
        title_y,
        circles_y,
        radius,
        wave_x: padding,
        wave_y,
        wave_width: inner_width,
        wave_height,
    }
}

/// Escape text for markup.
///
/// One function, used everywhere anything a person typed reaches the output. A
/// name containing `</text>` would otherwise end the element it sits in.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Base64, for embedding an image so the page stays one file.
///
/// Written out rather than depended on: it is twenty lines, and the dependency
/// graph containing nothing surprising is a claim this project makes on its
/// front page.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let packed = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(packed >> 18) as usize & 63] as char);
        out.push(ALPHABET[(packed >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(packed >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[packed as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The media type for an image, by extension.
fn media_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

/// Read an image and turn it into a `data:` URI.
///
/// An unknown extension is refused rather than guessed at: a browser handed the
/// wrong media type shows a broken image, and a broken image in a rendered
/// picture looks like the program failed rather than like the file was a `.bmp`.
pub fn data_uri(path: &Path) -> Result<String, Error> {
    let Some(media) = media_type(path) else {
        return Err(Error::Malformed(format!(
            "{} is not an image this can embed. PNG, JPEG, GIF, WebP and SVG are.",
            path.display()
        )));
    };
    let bytes = std::fs::read(path)?;
    Ok(format!("data:{media};base64,{}", base64(&bytes)))
}

/// The background element, and anything worth telling the user about it.
fn background_markup(look: &Look, width: u32, height: u32) -> (String, Vec<String>) {
    match &look.background {
        Background::Colour(colour) => (
            format!(
                "<rect width=\"{width}\" height=\"{height}\" fill=\"{}\"/>",
                escape(colour)
            ),
            Vec::new(),
        ),
        Background::Image(path) => match data_uri(path) {
            Ok(uri) => {
                let mut notes = Vec::new();
                // Roughly: base64 is four characters per three bytes.
                let kilobytes = uri.len() / 1024;
                if kilobytes > 2048 {
                    notes.push(format!(
                        "the background image adds about {kilobytes} kB to the page, \
                         because it is embedded so the file stays self-contained"
                    ));
                }
                (
                    format!(
                        "<rect width=\"{width}\" height=\"{height}\" fill=\"{}\"/>\
                         <image href=\"{}\" width=\"{width}\" height=\"{height}\" \
                         preserveAspectRatio=\"xMidYMid slice\"/>\
                         <rect width=\"{width}\" height=\"{height}\" fill=\"{}\" \
                         opacity=\"0.55\"/>",
                        look.palette.bg,
                        escape(&uri),
                        look.palette.bg
                    ),
                    notes,
                )
            }
            Err(error) => (
                format!(
                    "<rect width=\"{width}\" height=\"{height}\" fill=\"{}\"/>",
                    look.palette.bg
                ),
                vec![format!(
                    "the background image was not used: {error}. The picture was drawn on \
                     the plain background instead."
                )],
            ),
        },
    }
}

/// One speaker's circle and name, as SVG.
fn speaker_markup(
    plan: &Conversation,
    slot: usize,
    centre_x: f32,
    layout: &Layout,
    active: bool,
    ink: &str,
) -> (String, Vec<String>) {
    let speaker = &plan.speakers()[slot];
    let colour = palette::speaker(slot);
    let radius = layout.radius;
    let y = layout.circles_y;
    let mut notes = Vec::new();

    // Dim unless speaking. The lit state is a class the script toggles; the
    // still renders whichever state it was asked for.
    let opacity = if active { "1" } else { "0.35" };

    let body = match &speaker.picture {
        Some(picture) => match data_uri(picture) {
            Ok(uri) => format!(
                "<clipPath id=\"clip{slot}\"><circle cx=\"{centre_x:.1}\" cy=\"{y:.1}\" \
                 r=\"{r:.1}\"/></clipPath>\
                 <image href=\"{uri}\" x=\"{ix:.1}\" y=\"{iy:.1}\" width=\"{d:.1}\" \
                 height=\"{d:.1}\" preserveAspectRatio=\"xMidYMid slice\" \
                 clip-path=\"url(#clip{slot})\"/>\
                 <circle cx=\"{centre_x:.1}\" cy=\"{y:.1}\" r=\"{r:.1}\" fill=\"none\" \
                 stroke=\"{colour}\" stroke-width=\"{stroke:.1}\"/>",
                uri = escape(&uri),
                r = radius,
                ix = centre_x - radius,
                iy = y - radius,
                d = radius * 2.0,
                stroke = (radius * 0.14).max(2.0),
            ),
            Err(error) => {
                notes.push(format!(
                    "{}'s picture was not used: {error}. A plain circle was drawn instead.",
                    speaker.name
                ));
                format!(
                    "<circle cx=\"{centre_x:.1}\" cy=\"{y:.1}\" r=\"{radius:.1}\" \
                     fill=\"{colour}\"/>"
                )
            }
        },
        None => format!(
            "<circle cx=\"{centre_x:.1}\" cy=\"{y:.1}\" r=\"{radius:.1}\" fill=\"{colour}\"/>"
        ),
    };

    let label_y = y + radius + (radius * 0.55).max(18.0);
    let font = (radius * 0.36).clamp(11.0, 34.0);
    (
        format!(
            "<g class=\"speaker\" id=\"speaker{slot}\" opacity=\"{opacity}\">{body}\
             <text x=\"{centre_x:.1}\" y=\"{label_y:.1}\" text-anchor=\"middle\" \
             font-family=\"ui-monospace, SFMono-Regular, Menlo, Consolas, monospace\" \
             font-size=\"{font:.1}\" fill=\"{}\">{}</text></g>",
            ink,
            escape(&speaker.name)
        ),
        notes,
    )
}

/// What a render produced, and anything the user should know about it.
#[derive(Clone, Debug)]
pub struct Drawn {
    /// The markup.
    pub markup: String,
    /// Anything worth saying: a picture that could not be used, a page that
    /// grew large because an image was embedded in it.
    pub notes: Vec<String>,
}

/// A single frame, as standalone SVG.
///
/// `at_secs` decides which speaker is lit and where the playhead sits. This is
/// the preview: it is the same layout function and the same drawing code the
/// page uses, so what it shows is what will be produced rather than something
/// that resembles it.
pub fn still(
    plan: &Conversation,
    envelope: &Envelope,
    look: &Look,
    at_secs: f64,
) -> Result<Drawn, Error> {
    look.checked()?;
    let layout = layout(look, plan.len());
    let (width, height) = (look.width, look.height);
    let mut notes = Vec::new();

    let (background, background_notes) = background_markup(look, width, height);
    notes.extend(background_notes);

    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" \
         width=\"{width}\" height=\"{height}\" role=\"img\" aria-label=\"{}\">{background}",
        escape(plan.title.as_deref().unwrap_or("a veiled conversation"))
    );

    if let Some(title) = &plan.title {
        let font = (look.height as f32 * 0.055).clamp(14.0, 64.0);
        out.push_str(&format!(
            "<text x=\"{x:.1}\" y=\"{y:.1}\" text-anchor=\"middle\" \
             font-family=\"ui-monospace, SFMono-Regular, Menlo, Consolas, monospace\" \
             font-size=\"{font:.1}\" fill=\"{fg}\">{}</text>",
            escape(title),
            x = width as f32 / 2.0,
            y = layout.title_y,
            fg = look.palette.fg,
        ));
    }

    // Who is speaking at this moment. More than one is possible and all of them
    // are lit: an interruption is two people talking, and dimming one of them
    // would be choosing a winner the audio did not.
    let speaking: Vec<usize> = plan
        .turns()
        .iter()
        .filter(|turn| at_secs >= turn.start && at_secs < turn.end)
        .map(|turn| turn.speaker)
        .collect();

    let count = plan.len().max(1);
    let step = layout.wave_width / count as f32;
    for slot in 0..plan.len() {
        let centre_x = layout.wave_x + step * (slot as f32 + 0.5);
        let (markup, speaker_notes) = speaker_markup(
            plan,
            slot,
            centre_x,
            &layout,
            speaking.contains(&slot),
            look.palette.fg,
        );
        out.push_str(&markup);
        notes.extend(speaker_notes);
    }

    // The waveform, then the playhead over it.
    out.push_str(&format!(
        "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"4\" \
         fill=\"{}\" stroke=\"{}\"/>",
        layout.wave_x,
        layout.wave_y,
        layout.wave_width,
        layout.wave_height,
        look.palette.bg_inset,
        look.palette.border,
    ));
    let path = waveform::svg_path(
        envelope,
        layout.wave_x,
        layout.wave_y,
        layout.wave_width,
        layout.wave_height,
    );
    if !path.is_empty() {
        out.push_str(&format!(
            "<path d=\"{path}\" fill=\"{}\" opacity=\"0.85\"/>",
            look.palette.muted
        ));
    }
    let duration = plan.duration().max(1e-9);
    let progress = (at_secs / duration).clamp(0.0, 1.0) as f32;
    let head_x = layout.wave_x + layout.wave_width * progress;
    out.push_str(&format!(
        "<line id=\"playhead\" x1=\"{head_x:.1}\" y1=\"{:.1}\" x2=\"{head_x:.1}\" \
         y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"2\"/>",
        layout.wave_y,
        layout.wave_y + layout.wave_height,
        palette::SPEAKERS[0],
    ));

    out.push_str("</svg>");
    Ok(Drawn { markup: out, notes })
}

/// The self-contained page that plays.
///
/// `audio_href` and `subtitles_href` are written into the page as they are
/// given: relative names, so the three files can be moved together. Embedding
/// the audio would double the size of a recording somebody already has on disk
/// beside it.
pub fn player(
    plan: &Conversation,
    envelope: &Envelope,
    look: &Look,
    audio_href: &str,
    subtitles_href: &str,
) -> Result<Drawn, Error> {
    let drawn = still(plan, envelope, look, -1.0)?;

    // Turn boundaries for the script: slot, start, end. Numbers only -- the
    // names are already in the markup and there is no reason to put them in a
    // second place where they could disagree.
    let turns = plan
        .turns()
        .iter()
        .map(|turn| format!("[{},{:.3},{:.3}]", turn.speaker, turn.start, turn.end))
        .collect::<Vec<_>>()
        .join(",");

    let title = escape(plan.title.as_deref().unwrap_or("A veiled conversation"));
    let markup = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<style>\n\
         :root {{ color-scheme: dark; }}\n\
         html, body {{ margin: 0; background: {bg}; color: {fg};\n  \
           font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }}\n\
         main {{ max-width: 100%; margin: 0 auto; padding: 1rem; box-sizing: border-box; }}\n\
         svg {{ width: 100%; height: auto; display: block; }}\n\
         audio {{ width: 100%; margin-top: 1rem; }}\n\
         .speaker {{ transition: opacity 120ms linear; }}\n\
         .note {{ color: {muted}; font-size: 0.85rem; line-height: 1.5; }}\n\
         @media (prefers-reduced-motion: reduce) {{ .speaker {{ transition: none; }} }}\n\
         </style>\n</head>\n<body>\n<main>\n{svg}\n\
         <audio id=\"audio\" controls preload=\"metadata\" src=\"{audio}\">\n\
         <track kind=\"captions\" srclang=\"en\" label=\"Speakers\" src=\"{vtt}\" default>\n\
         </audio>\n\
         <noscript><p class=\"note\">This page uses a small script, held in the page \
         itself, only to light up whichever speaker is talking. Without it the audio \
         still plays, the captions still appear and the waveform is still drawn.</p>\
         </noscript>\n\
         <p class=\"note\">Every voice here has been replaced. The names are labels \
         somebody typed; nothing veils a name.</p>\n</main>\n\
         <script>\n(function () {{\n  \
         var turns = [{turns}];\n  \
         var audio = document.getElementById('audio');\n  \
         var head = document.getElementById('playhead');\n  \
         var groups = [];\n  \
         for (var i = 0; ; i++) {{\n    \
           var g = document.getElementById('speaker' + i);\n    \
           if (!g) break;\n    groups.push(g);\n  }}\n  \
         var box = head && head.getAttribute('x1');\n  \
         var left = {wave_x}, span = {wave_width}, total = {duration};\n  \
         function tick() {{\n    \
           var t = audio.currentTime;\n    \
           for (var i = 0; i < groups.length; i++) groups[i].setAttribute('opacity', '0.35');\n    \
           for (var j = 0; j < turns.length; j++) {{\n      \
             if (t >= turns[j][1] && t < turns[j][2] && groups[turns[j][0]]) {{\n        \
               groups[turns[j][0]].setAttribute('opacity', '1');\n      }}\n    }}\n    \
           if (head && total > 0) {{\n      \
             var x = left + span * Math.min(1, Math.max(0, t / total));\n      \
             head.setAttribute('x1', x); head.setAttribute('x2', x);\n    }}\n  }}\n  \
         audio.addEventListener('timeupdate', tick);\n  \
         audio.addEventListener('seeked', tick);\n  \
         void box;\n  tick();\n}})();\n</script>\n</body>\n</html>\n",
        bg = look.palette.bg,
        fg = look.palette.fg,
        muted = look.palette.muted,
        svg = drawn.markup,
        audio = escape(audio_href),
        vtt = escape(subtitles_href),
        wave_x = layout(look, plan.len()).wave_x,
        wave_width = layout(look, plan.len()).wave_width,
        duration = plan.duration(),
    );

    Ok(Drawn {
        markup,
        notes: drawn.notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use veilvoice_conversation::{Speaker, Turn};

    fn plan() -> Conversation {
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
        let samples: Vec<f32> = (0..48_000)
            .map(|i| (i as f32 / 200.0).sin() * 0.7)
            .collect();
        waveform::envelope(&samples, 400)
    }

    #[test]
    fn a_still_is_well_formed_svg_with_everything_in_it() {
        let drawn = still(&plan(), &envelope(), &Look::default(), 1.0).unwrap();
        assert!(drawn.markup.starts_with("<svg "));
        assert!(drawn.markup.ends_with("</svg>"));
        assert!(drawn.markup.contains("Two people"), "the title");
        assert!(drawn.markup.contains(">Alex<"), "a name");
        assert!(drawn.markup.contains(">Sam<"), "the other name");
        assert!(drawn.markup.contains("<path d=\"M"), "the waveform");
        assert!(drawn.markup.contains("id=\"playhead\""));
        assert!(drawn.notes.is_empty());
    }

    /// **The engine's settings are for the person operating it, not for the
    /// recording.**
    ///
    /// The desktop application shows each destination voice as "low register,
    /// narrow tract (94 Hz, 620 Hz)", which is exactly what somebody choosing
    /// between them needs. None of it belongs in the file that gets shared: it
    /// describes the *destination* rather than the speaker, so it leaks nothing
    /// about who was recorded, but it is noise to a viewer and it invites a
    /// reader to think the numbers say something about the people.
    ///
    /// What a viewer needs is who is talking, which is the name and the lit
    /// circle. This checks the picture carries the first and not the second.
    #[test]
    fn a_rendered_page_carries_names_and_no_engine_detail() {
        let drawn = still(&plan(), &envelope(), &Look::default(), 1.0).unwrap();
        let markup = drawn.markup.to_lowercase();

        // Who is talking: present.
        assert!(drawn.markup.contains(">Alex<"));
        assert!(drawn.markup.contains(">Sam<"));

        // The engine's own vocabulary: absent.
        for detail in [
            " hz",
            "register",
            "tract",
            "formant",
            "centroid",
            "intensity",
            "reseed",
            "ratchet",
        ] {
            assert!(
                !markup.contains(detail),
                "{detail:?} describes the engine and does not belong in a file                  somebody shares"
            );
        }
    }

    /// The circle that is lit must be the one whose turn it is.
    #[test]
    fn the_speaker_who_is_talking_is_the_one_that_is_lit() {
        let first = still(&plan(), &envelope(), &Look::default(), 1.0)
            .unwrap()
            .markup;
        let at = first.find("id=\"speaker0\"").unwrap();
        assert!(
            first[at..at + 60].contains("opacity=\"1\""),
            "slot 0 at 1 s"
        );
        let at = first.find("id=\"speaker1\"").unwrap();
        assert!(
            first[at..at + 60].contains("opacity=\"0.35\""),
            "slot 1 at 1 s"
        );

        let later = still(&plan(), &envelope(), &Look::default(), 3.0)
            .unwrap()
            .markup;
        let at = later.find("id=\"speaker1\"").unwrap();
        assert!(
            later[at..at + 60].contains("opacity=\"1\""),
            "slot 1 at 3 s"
        );
    }

    /// An interruption is two people talking, and dimming one would be
    /// choosing a winner the audio did not.
    #[test]
    fn two_people_talking_at_once_are_both_lit() {
        let mut plan = plan();
        plan.add_turn(Turn {
            start: 0.5,
            end: 1.5,
            speaker: 1,
            text: None,
        })
        .unwrap();
        let markup = still(&plan, &envelope(), &Look::default(), 1.0)
            .unwrap()
            .markup;
        for slot in 0..2 {
            let at = markup.find(&format!("id=\"speaker{slot}\"")).unwrap();
            assert!(
                markup[at..at + 60].contains("opacity=\"1\""),
                "slot {slot} should be lit during an interruption"
            );
        }
    }

    /// A name containing markup must not end the element it is in.
    #[test]
    fn a_name_cannot_break_out_of_the_markup() {
        let mut plan = Conversation::new();
        plan.title = Some("</text><script>alert(1)</script>".into());
        plan.add_speaker(Speaker::named("</text><g fill=\"red\">"))
            .unwrap();
        plan.add_turn(Turn {
            start: 0.0,
            end: 1.0,
            speaker: 0,
            text: None,
        })
        .unwrap();
        let markup = still(&plan, &envelope(), &Look::default(), 0.5)
            .unwrap()
            .markup;
        assert!(!markup.contains("<script>"), "a script element survived");
        assert!(!markup.contains("<g fill=\"red\">"), "an element survived");
        assert!(markup.contains("&lt;/text&gt;"), "it must be escaped");

        let page = player(&plan, &envelope(), &Look::default(), "a.wav", "a.vtt")
            .unwrap()
            .markup;
        assert!(
            !page.contains("<script>alert(1)</script>"),
            "the title escaped into the page"
        );
    }

    #[test]
    fn escaping_covers_every_character_that_matters() {
        assert_eq!(escape("a&b"), "a&amp;b");
        assert_eq!(escape("<b>"), "&lt;b&gt;");
        assert_eq!(escape("\"'"), "&quot;&#39;");
        assert_eq!(escape("plain"), "plain");
    }

    #[test]
    fn the_player_is_a_whole_page_that_needs_nothing_else() {
        let drawn = player(&plan(), &envelope(), &Look::default(), "out.wav", "out.vtt").unwrap();
        let page = &drawn.markup;
        assert!(page.starts_with("<!DOCTYPE html>"));
        assert!(page.trim_end().ends_with("</html>"));
        assert!(page.contains("<svg "), "the picture is inline");
        assert!(page.contains("src=\"out.wav\""));
        assert!(page.contains("src=\"out.vtt\""));
        assert!(
            page.contains("<noscript>"),
            "the script must explain itself"
        );
        assert!(page.contains("nothing veils a name"));
        // Nothing may be fetched from anywhere. The SVG namespace is a URI and
        // not a URL -- no browser resolves it, and every SVG document must
        // declare it -- so it is excluded by name rather than by pretending
        // the string is not there.
        let fetchable = page.replace("http://www.w3.org/2000/svg", "");
        for scheme in ["http://", "https://"] {
            assert!(
                !fetchable.contains(scheme),
                "the page reaches the network: {:?}",
                fetchable
                    .find(scheme)
                    .map(|at| &fetchable[at..(at + 60).min(fetchable.len())])
            );
        }
        assert!(
            page.contains("width=device-width"),
            "it must work on a phone"
        );
    }

    /// A look that cannot be drawn is refused rather than producing an
    /// illegible picture that looks like a design decision.
    #[test]
    fn an_impossible_look_is_refused() {
        for look in [
            Look {
                width: 100,
                ..Look::default()
            },
            Look {
                height: 100,
                ..Look::default()
            },
            Look {
                width: 9000,
                ..Look::default()
            },
            Look {
                padding: 600,
                ..Look::default()
            },
            Look {
                background: Background::Colour("nonsense".into()),
                ..Look::default()
            },
        ] {
            assert!(look.checked().is_err(), "{look:?} should be refused");
            assert!(still(&plan(), &envelope(), &look, 0.0).is_err());
        }
        Look::default()
            .checked()
            .expect("the default must be drawable");
        Look::default()
            .black()
            .checked()
            .expect("black must be drawable");
    }

    /// Padding is a setting because the same picture is wanted at very
    /// different sizes, so it has to actually move things.
    #[test]
    fn more_padding_leaves_less_room_for_the_waveform() {
        let tight = layout(
            &Look {
                padding: 8,
                ..Look::default()
            },
            2,
        );
        let loose = layout(
            &Look {
                padding: 120,
                ..Look::default()
            },
            2,
        );
        assert!(loose.wave_width < tight.wave_width);
        assert!(loose.wave_x > tight.wave_x);
    }

    /// Everything must stay inside the picture, at any speaker count.
    #[test]
    fn the_layout_stays_inside_the_frame_for_every_speaker_count() {
        let look = Look::default();
        for speakers in 1..=10 {
            let layout = layout(&look, speakers);
            assert!(layout.wave_x >= look.padding as f32 - 0.01);
            assert!(layout.wave_x + layout.wave_width <= (look.width - look.padding) as f32 + 0.01);
            assert!(
                layout.wave_y + layout.wave_height <= (look.height - look.padding) as f32 + 0.01
            );
            assert!(layout.circles_y - layout.radius > layout.title_y);
            assert!(
                layout.circles_y + layout.radius < layout.wave_y,
                "{speakers} speakers: the circles overlap the waveform"
            );
            assert!(layout.radius >= 8.0);
        }
    }

    #[test]
    fn base64_matches_the_standard_including_its_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn an_image_becomes_a_data_uri_and_an_unknown_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("portrait.png");
        std::fs::write(&png, b"not really a png, but the bytes go through").unwrap();
        let uri = data_uri(&png).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"), "{uri}");

        let bmp = dir.path().join("portrait.bmp");
        std::fs::write(&bmp, b"x").unwrap();
        let error = data_uri(&bmp).expect_err("an unknown type must be refused");
        assert!(error.to_string().contains("not an image this can embed"));
        assert!(data_uri(&dir.path().join("missing.png")).is_err());
    }

    /// A picture that cannot be used must draw the plain circle and say so,
    /// rather than leaving a broken image that looks like a crash.
    #[test]
    fn an_unusable_picture_falls_back_and_is_reported() {
        let mut plan = Conversation::new();
        plan.add_speaker(Speaker {
            name: "Sam".into(),
            picture: Some(PathBuf::from("no-such-file.png")),
        })
        .unwrap();
        plan.add_turn(Turn {
            start: 0.0,
            end: 1.0,
            speaker: 0,
            text: None,
        })
        .unwrap();
        let drawn = still(&plan, &envelope(), &Look::default(), 0.5).unwrap();
        assert!(drawn.markup.contains("<circle"), "the fallback circle");
        assert!(!drawn.markup.contains("<image "), "no broken image");
        let notes = drawn.notes.join(" ");
        assert!(notes.contains("Sam's picture was not used"), "{notes}");
    }

    /// A background image is embedded so the page stays one file, and a large
    /// one is reported rather than silently producing a huge page.
    #[test]
    fn a_background_image_is_embedded_and_a_large_one_is_mentioned() {
        let dir = tempfile::tempdir().unwrap();
        let small = dir.path().join("bg.png");
        std::fs::write(&small, vec![0u8; 64]).unwrap();
        let drawn = still(
            &plan(),
            &envelope(),
            &Look {
                background: Background::Image(small),
                ..Look::default()
            },
            0.0,
        )
        .unwrap();
        assert!(drawn.markup.contains("data:image/png;base64,"));
        assert!(drawn.notes.is_empty(), "a small image needs no comment");

        let big = dir.path().join("big.png");
        std::fs::write(&big, vec![0u8; 3 * 1024 * 1024]).unwrap();
        let drawn = still(
            &plan(),
            &envelope(),
            &Look {
                background: Background::Image(big),
                ..Look::default()
            },
            0.0,
        )
        .unwrap();
        assert!(
            drawn.notes.join(" ").contains("adds about"),
            "{:?}",
            drawn.notes
        );
    }

    /// A missing background must not stop the picture being drawn.
    #[test]
    fn a_missing_background_falls_back_and_says_so() {
        let drawn = still(
            &plan(),
            &envelope(),
            &Look {
                background: Background::Image(PathBuf::from("nowhere.png")),
                ..Look::default()
            },
            0.0,
        )
        .unwrap();
        assert!(drawn.markup.contains(palette::BG));
        assert!(drawn.notes.join(" ").contains("was not used"));
    }

    /// An empty plan still draws: a recording with nobody assigned is a
    /// picture of a waveform, which is better than an error at this stage.
    #[test]
    fn a_plan_with_no_speakers_still_draws_the_waveform() {
        let drawn = still(&Conversation::new(), &envelope(), &Look::default(), 0.0).unwrap();
        assert!(drawn.markup.contains("<path d=\"M"));
        assert!(!drawn.markup.contains("id=\"speaker0\""));
    }
}
