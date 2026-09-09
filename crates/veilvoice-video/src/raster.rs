// SPDX-License-Identifier: GPL-3.0-or-later
//! Pixels, and a PNG to put them in.
//!
//! # Why the video's pictures are drawn here rather than converted
//!
//! Everything else this crate draws is SVG, which is right for a page: the
//! reader's browser has a renderer and a font library and neither is this
//! project's problem. A video file is not a page. `ffmpeg` reads a directory of
//! raster images, and **no build of ffmpeg can be assumed to read SVG**: that
//! needs librsvg, which most builds do not have, and a picture that fails to
//! encode on somebody's machine is worse than one that was never offered.
//!
//! The other way is to convert the SVG here, which means an SVG rasteriser.
//! Every usable one is large, and [`crate::ffmpeg`] already spends a page
//! explaining why this project will not pull in a large library to make a video
//! file. Doing it anyway, one module over, would make that argument a thing
//! this project says rather than a thing it does.
//!
//! So the picture is a few hundred lines: rectangles, circles, a face
//! ([`crate::font`]) and a PNG writer. That is the whole of what the drawing
//! needs, and it is small enough to read.
//!
//! # The same picture on every machine
//!
//! No system fonts, no floating-point that varies by platform in a way that
//! reaches a pixel, and no dependency that could quietly change its output. Two
//! people rendering the same recording get identical files, which is the same
//! property the reproducible builds have and is checked the same way: by
//! comparing bytes.
//!
//! # One dependency, and what it is for
//!
//! `miniz_oxide` deflates the pixel data, because PNG is deflate and a PNG
//! written with stored blocks is roughly six megabytes a frame. It is pure Rust
//! and it is **already in this tree**, underneath `flate2`, which `lofty` and
//! `pgp` both pull in, so naming it here adds nothing to the dependency graph
//! that was not being compiled already.
//!
//! # In plain words
//!
//! The part that draws the video's pictures, dot by dot, and writes them as PNG
//! files.
//!
//! It is written here rather than borrowed because the video tool this hands its
//! pictures to cannot be relied on to read drawings, and converting them would
//! mean carrying a large piece of somebody else's code, which is the thing this
//! program is careful not to do.

use crate::font;

/// A colour, as the three bytes a PNG stores.
pub type Rgb = [u8; 3];

/// Read `#rrggbb` into three bytes.
///
/// The palettes are written as hex strings because that is what the website and
/// the SVG both want, so this is the one place they become numbers. Anything
/// that is not six hex digits after a `#` gives `None`: a colour that could not
/// be read is a caller's mistake, and guessing at it would paint a frame a
/// colour nobody chose.
pub fn colour(hex: &str) -> Option<Rgb> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let mut out = [0u8; 3];
    for (at, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(digits.get(at * 2..at * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// A picture being drawn, one byte per channel, three channels per pixel.
pub struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Canvas {
    /// A canvas of `width` by `height`, filled with `background`.
    pub fn new(width: usize, height: usize, background: Rgb) -> Self {
        let mut pixels = Vec::with_capacity(width * height * 3);
        for _ in 0..width * height {
            pixels.extend_from_slice(&background);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    /// How wide it is.
    pub fn width(&self) -> usize {
        self.width
    }

    /// How tall it is.
    pub fn height(&self) -> usize {
        self.height
    }

    /// The colour at a point, or `None` outside the canvas. For tests.
    pub fn at(&self, x: usize, y: usize) -> Option<Rgb> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let at = (y * self.width + x) * 3;
        Some([self.pixels[at], self.pixels[at + 1], self.pixels[at + 2]])
    }

    /// Mix `colour` into the pixel at `x`, `y` by `alpha`, from 0.0 to 1.0.
    ///
    /// Everything that draws a curve goes through this: a circle with a hard
    /// edge looks like a mistake at any size, and the fix is to let the edge
    /// pixels be part one colour and part the other.
    fn blend(&mut self, x: usize, y: usize, colour: Rgb, alpha: f32) {
        if x >= self.width || y >= self.height || alpha <= 0.0 {
            return;
        }
        let alpha = alpha.min(1.0);
        let at = (y * self.width + x) * 3;
        for (channel, want) in self.pixels[at..at + 3].iter_mut().zip(colour) {
            let had = *channel as f32;
            *channel = (had + (want as f32 - had) * alpha)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }

    /// A filled rectangle, clipped to the canvas.
    ///
    /// Takes signed coordinates because the layout is computed in floats and a
    /// box can legitimately start off the left edge; clipping here is one check
    /// rather than one at every call site.
    pub fn rect(&mut self, x: i64, y: i64, width: i64, height: i64, colour: Rgb) {
        if width <= 0 || height <= 0 {
            return;
        }
        let left = x.max(0) as usize;
        let top = y.max(0) as usize;
        let right = (x + width).clamp(0, self.width as i64) as usize;
        let bottom = (y + height).clamp(0, self.height as i64) as usize;
        for row in top..bottom {
            for column in left..right {
                let at = (row * self.width + column) * 3;
                self.pixels[at..at + 3].copy_from_slice(&colour);
            }
        }
    }

    /// A rectangle with rounded ends, which is how the level bars are drawn.
    ///
    /// The radius is capped at half the shorter side, because a corner rounder
    /// than the box it is on is not a shape.
    pub fn rounded_rect(&mut self, x: f32, y: f32, width: f32, height: f32, colour: Rgb) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let radius = (height / 2.0).min(width / 2.0);
        // The straight middle, then a disc at each end. Cheaper than testing
        // every pixel against four corner arcs, and the seams are exact
        // because the discs sit on the ends rather than near them.
        self.rect(
            (x + radius).round() as i64,
            y.round() as i64,
            (width - radius * 2.0).round() as i64,
            height.round() as i64,
            colour,
        );
        self.circle(x + radius, y + height / 2.0, radius, colour);
        self.circle(x + width - radius, y + height / 2.0, radius, colour);
    }

    /// A filled circle with a smooth edge.
    ///
    /// Coverage is sampled on a four-by-four grid inside each pixel that the
    /// edge crosses. Sixteen samples is enough that no step is visible at any
    /// frame size this renders, and pixels well inside or well outside are
    /// filled or skipped without sampling at all, so the cost is the outline
    /// rather than the area.
    pub fn circle(&mut self, centre_x: f32, centre_y: f32, radius: f32, colour: Rgb) {
        if radius <= 0.0 {
            return;
        }
        let left = ((centre_x - radius).floor() as i64).max(0) as usize;
        let top = ((centre_y - radius).floor() as i64).max(0) as usize;
        let right = ((centre_x + radius).ceil() as i64).clamp(0, self.width as i64) as usize;
        let bottom = ((centre_y + radius).ceil() as i64).clamp(0, self.height as i64) as usize;

        let inner = (radius - 0.75).max(0.0);
        let outer = radius + 0.75;
        for row in top..bottom {
            for column in left..right {
                let dx = column as f32 + 0.5 - centre_x;
                let dy = row as f32 + 0.5 - centre_y;
                let distance = (dx * dx + dy * dy).sqrt();
                if distance <= inner {
                    self.blend(column, row, colour, 1.0);
                    continue;
                }
                if distance >= outer {
                    continue;
                }
                let mut inside = 0u32;
                for sample_y in 0..4 {
                    for sample_x in 0..4 {
                        let sx = column as f32 + (sample_x as f32 + 0.5) / 4.0 - centre_x;
                        let sy = row as f32 + (sample_y as f32 + 0.5) / 4.0 - centre_y;
                        if sx * sx + sy * sy <= radius * radius {
                            inside += 1;
                        }
                    }
                }
                self.blend(column, row, colour, inside as f32 / 16.0);
            }
        }
    }

    /// A ring, drawn as a filled disc with the middle taken back out.
    pub fn ring(&mut self, centre_x: f32, centre_y: f32, radius: f32, thickness: f32, colour: Rgb) {
        let inner = (radius - thickness).max(0.0);
        let kept: Vec<(usize, usize, Rgb)> = {
            let left = ((centre_x - inner).floor() as i64).max(0) as usize;
            let top = ((centre_y - inner).floor() as i64).max(0) as usize;
            let right = ((centre_x + inner).ceil() as i64).clamp(0, self.width as i64) as usize;
            let bottom = ((centre_y + inner).ceil() as i64).clamp(0, self.height as i64) as usize;
            let mut held = Vec::new();
            for row in top..bottom {
                for column in left..right {
                    let dx = column as f32 + 0.5 - centre_x;
                    let dy = row as f32 + 0.5 - centre_y;
                    if dx * dx + dy * dy <= inner * inner {
                        if let Some(had) = self.at(column, row) {
                            held.push((column, row, had));
                        }
                    }
                }
            }
            held
        };
        self.circle(centre_x, centre_y, radius, colour);
        // Put back what was under the middle, so a ring over a drawn
        // background does not punch a hole in it.
        for (column, row, had) in kept {
            self.blend(column, row, had, 1.0);
        }
    }

    /// Draw `text` with its left edge at `x` and its top at `y`.
    ///
    /// Returns how many characters had no glyph, so a caller can say which
    /// names came out as boxes rather than leaving somebody to notice.
    pub fn text(&mut self, x: i64, y: i64, text: &str, scale: usize, colour: Rgb) -> usize {
        let mut missing = 0;
        let mut pen = x;
        for character in text.chars() {
            let (rows, known) = font::glyph(character);
            if !known {
                missing += 1;
            }
            for (row, bits) in rows.iter().enumerate() {
                for column in 0..font::WIDTH {
                    if bits & (1 << (font::WIDTH - 1 - column)) == 0 {
                        continue;
                    }
                    self.rect(
                        pen + (column * scale) as i64,
                        y + (row * scale) as i64,
                        scale as i64,
                        scale as i64,
                        colour,
                    );
                }
            }
            pen += ((font::WIDTH + font::GAP) * scale) as i64;
        }
        missing
    }

    /// Draw `text` centred on `centre_x`, with its top at `y`.
    pub fn text_centred(
        &mut self,
        centre_x: f32,
        y: i64,
        text: &str,
        scale: usize,
        colour: Rgb,
    ) -> usize {
        let width = font::width_of(text, scale) as f32;
        self.text(
            (centre_x - width / 2.0).round() as i64,
            y,
            text,
            scale,
            colour,
        )
    }

    /// The picture as a PNG file.
    ///
    /// Truecolour, eight bits a channel, no interlacing and no alpha: the
    /// frames are opaque, and an alpha channel nothing uses is a third more
    /// bytes through the encoder for every frame of the video.
    pub fn png(&self) -> Vec<u8> {
        // Each row is prefixed with its filter type. Zero, meaning none: these
        // are flat colour and large runs, which deflate handles directly, and
        // the adaptive filters cost a pass over every row to save little.
        let mut raw = Vec::with_capacity(self.height * (1 + self.width * 3));
        for row in 0..self.height {
            raw.push(0);
            let at = row * self.width * 3;
            raw.extend_from_slice(&self.pixels[at..at + self.width * 3]);
        }
        // Six: the default. Nine costs noticeably more time for a frame of flat
        // colour that is already almost all runs.
        let squeezed = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);

        let mut out = Vec::with_capacity(squeezed.len() + 64);
        out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

        let mut header = Vec::with_capacity(13);
        header.extend_from_slice(&(self.width as u32).to_be_bytes());
        header.extend_from_slice(&(self.height as u32).to_be_bytes());
        header.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &header);
        chunk(&mut out, b"IDAT", &squeezed);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

/// Append one PNG chunk: length, type, data, and the checksum over both.
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = Crc::new();
    crc.eat(kind);
    crc.eat(data);
    out.extend_from_slice(&crc.done().to_be_bytes());
}

/// The CRC-32 PNG puts on every chunk.
///
/// Fifteen lines rather than a dependency: this is the one checksum PNG needs
/// and the polynomial is in the specification.
struct Crc(u32);

impl Crc {
    fn new() -> Self {
        Self(0xffff_ffff)
    }

    fn eat(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= *byte as u32;
            for _ in 0..8 {
                // The reflected polynomial, which is what PNG specifies.
                self.0 = if self.0 & 1 != 0 {
                    (self.0 >> 1) ^ 0xedb8_8320
                } else {
                    self.0 >> 1
                };
            }
        }
    }

    fn done(self) -> u32 {
        self.0 ^ 0xffff_ffff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: Rgb = [0, 0, 0];
    const WHITE: Rgb = [255, 255, 255];

    #[test]
    fn a_colour_is_read_or_refused() {
        assert_eq!(colour("#1a1b26"), Some([0x1a, 0x1b, 0x26]));
        assert_eq!(colour("#FFFFFF"), Some([255, 255, 255]));
        for wrong in ["1a1b26", "#1a1b2", "#1a1b26f", "", "#zzzzzz", "#"] {
            assert_eq!(colour(wrong), None, "{wrong:?} was read as a colour");
        }
    }

    #[test]
    fn a_new_canvas_is_its_background_everywhere() {
        let canvas = Canvas::new(4, 3, [1, 2, 3]);
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(canvas.at(x, y), Some([1, 2, 3]));
            }
        }
        assert_eq!(canvas.at(4, 0), None, "outside is outside");
        assert_eq!(canvas.at(0, 3), None);
    }

    /// Drawing off the edge clips rather than wrapping or panicking.
    ///
    /// The layout is computed in floats and a box can legitimately start left
    /// of zero. Wrapping would put a bar on the wrong side of the picture.
    #[test]
    fn drawing_outside_the_canvas_is_clipped() {
        let mut canvas = Canvas::new(4, 4, BLACK);
        canvas.rect(-10, -10, 12, 12, WHITE);
        assert_eq!(canvas.at(1, 1), Some(WHITE), "the part that is inside");
        assert_eq!(canvas.at(3, 3), Some(BLACK), "the part that is outside");

        canvas.rect(100, 100, 10, 10, WHITE);
        canvas.circle(-50.0, -50.0, 3.0, WHITE);
        canvas.circle(1000.0, 1000.0, 3.0, WHITE);
        assert_eq!(canvas.at(3, 3), Some(BLACK), "nothing wrapped round");
    }

    /// A circle is filled in the middle, empty at the corners, and soft at the
    /// edge.
    #[test]
    fn a_circle_is_round_and_its_edge_is_not_a_staircase() {
        let mut canvas = Canvas::new(41, 41, BLACK);
        canvas.circle(20.5, 20.5, 15.0, WHITE);

        assert_eq!(canvas.at(20, 20), Some(WHITE), "the middle");
        assert_eq!(canvas.at(0, 0), Some(BLACK), "a corner");

        // Somewhere on the rim there is a pixel that is neither, which is what
        // a smooth edge means and what a hard-edged circle would not have.
        let mut partial = 0;
        for y in 0..41 {
            for x in 0..41 {
                let shade = canvas.at(x, y).unwrap()[0];
                if shade > 0 && shade < 255 {
                    partial += 1;
                }
            }
        }
        assert!(
            partial > 20,
            "only {partial} pixels are part-covered, so the edge is a staircase"
        );
    }

    /// A ring is hollow.
    #[test]
    fn a_ring_leaves_what_was_under_its_middle() {
        let mut canvas = Canvas::new(41, 41, BLACK);
        canvas.rect(0, 0, 41, 41, [7, 7, 7]);
        canvas.ring(20.5, 20.5, 15.0, 3.0, WHITE);
        assert_eq!(
            canvas.at(20, 20),
            Some([7, 7, 7]),
            "the middle is untouched"
        );
        assert_eq!(canvas.at(20, 6), Some(WHITE), "the rim is drawn");
    }

    /// Text lands where it was put and reports what it could not draw.
    #[test]
    fn text_is_drawn_and_says_what_it_could_not() {
        let mut canvas = Canvas::new(80, 20, BLACK);
        let missing = canvas.text(2, 2, "Hi", 2, WHITE);
        assert_eq!(missing, 0);
        let lit = (0..20)
            .flat_map(|y| (0..80).map(move |x| (x, y)))
            .filter(|(x, y)| canvas.at(*x, *y) == Some(WHITE))
            .count();
        assert!(lit > 0, "nothing was drawn");

        let mut other = Canvas::new(80, 20, BLACK);
        assert_eq!(other.text(2, 2, "Zoë", 2, WHITE), 1, "one box");
    }

    /// Centred text is centred.
    #[test]
    fn centred_text_sits_on_its_centre() {
        let mut canvas = Canvas::new(101, 20, BLACK);
        canvas.text_centred(50.0, 2, "ii", 2, WHITE);
        let columns: Vec<usize> = (0..101)
            .filter(|x| (0..20).any(|y| canvas.at(*x, y) == Some(WHITE)))
            .collect();
        let left = *columns.first().expect("something was drawn");
        let right = *columns.last().unwrap();
        let middle = (left + right) as f32 / 2.0;
        assert!(
            (middle - 50.0).abs() <= 2.0,
            "drawn from {left} to {right}, centred on {middle} rather than 50"
        );
    }

    /// The PNG is a PNG: signature, the three chunks, and correct checksums.
    ///
    /// Checked by reading the bytes rather than by opening the file with
    /// something, because "a decoder accepted it" and "it is what the format
    /// says" are different claims and this writer makes the second.
    #[test]
    fn the_png_is_shaped_the_way_the_format_says() {
        let mut canvas = Canvas::new(7, 5, [10, 20, 30]);
        canvas.circle(3.5, 2.5, 2.0, [200, 100, 50]);
        let png = canvas.png();

        assert_eq!(
            &png[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
            "signature"
        );

        // Walk the chunks, checking each length and checksum as it goes.
        let mut at = 8;
        let mut seen = Vec::new();
        while at + 12 <= png.len() {
            let length = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let kind = String::from_utf8_lossy(&png[at + 4..at + 8]).into_owned();
            let body = &png[at + 8..at + 8 + length];
            let stated =
                u32::from_be_bytes(png[at + 8 + length..at + 12 + length].try_into().unwrap());
            let mut crc = Crc::new();
            crc.eat(&png[at + 4..at + 8]);
            crc.eat(body);
            assert_eq!(crc.done(), stated, "{kind} checksum");
            if kind == "IHDR" {
                assert_eq!(u32::from_be_bytes(body[0..4].try_into().unwrap()), 7);
                assert_eq!(u32::from_be_bytes(body[4..8].try_into().unwrap()), 5);
                assert_eq!(
                    &body[8..],
                    &[8, 2, 0, 0, 0],
                    "8-bit truecolour, no interlace"
                );
            }
            seen.push(kind);
            at += 12 + length;
        }
        assert_eq!(at, png.len(), "trailing bytes after the last chunk");
        assert_eq!(seen, vec!["IHDR", "IDAT", "IEND"]);
    }

    /// The pixels survive the round trip.
    ///
    /// Deflated and inflated again, and compared against what was drawn. A
    /// writer that produced a well-formed file of the wrong pixels would pass
    /// every structural check above.
    #[test]
    fn the_pixels_come_back_out_as_they_went_in() {
        let mut canvas = Canvas::new(9, 6, [10, 20, 30]);
        canvas.rect(2, 1, 4, 3, [200, 100, 50]);
        let png = canvas.png();

        // The IDAT body, found by walking rather than by a fixed offset.
        let mut at = 8;
        let mut squeezed = Vec::new();
        while at + 12 <= png.len() {
            let length = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            if &png[at + 4..at + 8] == b"IDAT" {
                squeezed = png[at + 8..at + 8 + length].to_vec();
            }
            at += 12 + length;
        }
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&squeezed)
            .expect("the deflate stream is readable");

        assert_eq!(raw.len(), 6 * (1 + 9 * 3), "one filter byte per row");
        for y in 0..6 {
            let row = y * (1 + 9 * 3);
            assert_eq!(raw[row], 0, "row {y} filter");
            for x in 0..9 {
                let at = row + 1 + x * 3;
                assert_eq!(
                    [raw[at], raw[at + 1], raw[at + 2]],
                    canvas.at(x, y).unwrap(),
                    "pixel {x},{y}"
                );
            }
        }
    }

    /// The same drawing gives the same bytes.
    ///
    /// Two people rendering one recording should get one file. Nothing here may
    /// depend on a hash order, a system font or a clock.
    #[test]
    fn the_same_picture_encodes_to_the_same_bytes() {
        let draw = || {
            let mut canvas = Canvas::new(64, 40, [26, 27, 38]);
            canvas.circle(20.0, 20.0, 11.0, [122, 162, 247]);
            canvas.rounded_rect(30.0, 30.0, 28.0, 6.0, [158, 206, 106]);
            canvas.text(2, 2, "Alex", 2, [192, 202, 245]);
            canvas.png()
        };
        assert_eq!(draw(), draw());
    }
}
