// SPDX-License-Identifier: GPL-3.0-or-later
//! A monospace face, five pixels by seven, drawn here.
//!
//! # Why this exists rather than a font file
//!
//! [`crate::frames`] draws the video's pictures as actual pixels, and names are
//! text. Text needs a face and a rasteriser, and the two usual ways to get
//! those are both wrong for this project.
//!
//! Loading one of the system's fonts makes the output depend on which machine
//! drew it: the same recording rendered on two computers would produce
//! different files, and this project publishes reproducible builds and checks
//! them. Pulling in a font rasteriser means a large dependency to draw eight
//! names, in a program whose front page invites the reader to run `cargo tree`
//! and find nothing large.
//!
//! So the face is here, it is ninety-five glyphs, and it is the same everywhere.
//!
//! # What it can and cannot draw
//!
//! Printable ASCII, from space to `~`. **Anything else is drawn as an open
//! box**, which is the honest way to render a character this cannot: a name in
//! Cyrillic or Japanese comes out as boxes rather than as nothing, and
//! [`crate::frames`] says so in its notes rather than letting somebody find out
//! by watching the finished video.
//!
//! The preview page does not have this limit, because it is markup and uses
//! whatever the reader's machine has. That is a real difference between the two
//! outputs and it is written down rather than glossed over.
//!
//! # Five by seven
//!
//! Small enough to write out and check by eye, large enough to stay legible
//! when scaled up by whole numbers, which is the only way it is ever scaled: a
//! bitmap glyph drawn at a fractional size is a blurred glyph, so
//! [`Face::scale_for`] picks a whole-number multiple and the text is crisp at
//! any frame size.
//!
//! # In plain words
//!
//! The letters used in the video, drawn dot by dot inside this program.
//!
//! It is here rather than taken from the computer so that the same recording
//! makes the same video on every machine, and so that this program does not have
//! to carry a large piece of somebody else's code to write eight names.

/// The first character the face has a glyph for.
pub const FIRST: char = ' ';
/// The last character the face has a glyph for.
pub const LAST: char = '~';

/// Width of one glyph, in pixels, before scaling.
pub const WIDTH: usize = 5;
/// Height of one glyph, in pixels, before scaling.
pub const HEIGHT: usize = 7;

/// The gap between two glyphs, in unscaled pixels.
///
/// One column. A monospace face with no gap runs its letters together, and two
/// makes eight names wider than the picture they sit in.
pub const GAP: usize = 1;

/// The glyphs, one row of five bits per line, seven lines per character.
///
/// Indexed by `character as usize - FIRST as usize`. Written out rather than
/// generated, so that what is in this file is what gets drawn.
#[rustfmt::skip]
const GLYPHS: [[u8; HEIGHT]; 95] = [
    // ' '      
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    // '!'      
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
    // '"'      
    [0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    // '#'      
    [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010],
    // '$'      
    [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100],
    // '%'      
    [0b11000, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b00011],
    // '&'      
    [0b01100, 0b10010, 0b10010, 0b01100, 0b10010, 0b10001, 0b01110],
    // "'"      
    [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    // '('      
    [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
    // ')'      
    [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
    // '*'      
    [0b00000, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0b00000],
    // '+'      
    [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
    // ','      
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000],
    // '-'      
    [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
    // '.'      
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110],
    // '/'      
    [0b00001, 0b00010, 0b00100, 0b00100, 0b00100, 0b01000, 0b10000],
    // '0'      
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
    // '1'      
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    // '2'      
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
    // '3'      
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
    // '4'      
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    // '5'      
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    // '6'      
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
    // '7'      
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    // '8'      
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    // '9'      
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
    // ':'      
    [0b00000, 0b00110, 0b00110, 0b00000, 0b00110, 0b00110, 0b00000],
    // ';'      
    [0b00000, 0b00110, 0b00110, 0b00000, 0b00110, 0b00100, 0b01000],
    // '<'      
    [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
    // '='      
    [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
    // '>'      
    [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
    // '?'      
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100],
    // '@'      
    [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01111],
    // 'A'      
    [0b00100, 0b01010, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001],
    // 'B'      
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
    // 'C'      
    [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
    // 'D'      
    [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
    // 'E'      
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
    // 'F'      
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
    // 'G'      
    [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
    // 'H'      
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    // 'I'      
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    // 'J'      
    [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
    // 'K'      
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
    // 'L'      
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
    // 'M'      
    [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
    // 'N'      
    [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001],
    // 'O'      
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    // 'P'      
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
    // 'Q'      
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
    // 'R'      
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
    // 'S'      
    [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
    // 'T'      
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    // 'U'      
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    // 'V'      
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    // 'W'      
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
    // 'X'      
    [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
    // 'Y'      
    [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
    // 'Z'      
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
    // '['      
    [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
    // '\\'     
    [0b10000, 0b01000, 0b01000, 0b00100, 0b00100, 0b00010, 0b00001],
    // ']'      
    [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
    // '^'      
    [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000],
    // '_'      
    [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
    // '`'      
    [0b01000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    // 'a'      
    [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
    // 'b'      
    [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
    // 'c'      
    [0b00000, 0b00000, 0b01111, 0b10000, 0b10000, 0b10000, 0b01111],
    // 'd'      
    [0b00001, 0b00001, 0b01111, 0b10001, 0b10001, 0b10001, 0b01111],
    // 'e'      
    [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
    // 'f'      
    [0b00110, 0b01001, 0b01000, 0b11110, 0b01000, 0b01000, 0b01000],
    // 'g'      
    [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
    // 'h'      
    [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
    // 'i'      
    [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
    // 'j'      
    [0b00010, 0b00000, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
    // 'k'      
    [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010],
    // 'l'      
    [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    // 'm'      
    [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10101, 0b10001],
    // 'n'      
    [0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001],
    // 'o'      
    [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
    // 'p'      
    [0b00000, 0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000],
    // 'q'      
    [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001],
    // 'r'      
    [0b00000, 0b00000, 0b10111, 0b11000, 0b10000, 0b10000, 0b10000],
    // 's'      
    [0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110],
    // 't'      
    [0b00100, 0b00100, 0b01111, 0b00100, 0b00100, 0b00101, 0b00010],
    // 'u'      
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10001, 0b01111],
    // 'v'      
    [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    // 'w'      
    [0b00000, 0b00000, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
    // 'x'      
    [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
    // 'y'      
    [0b00000, 0b10001, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
    // 'z'      
    [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
    // '{'      
    [0b00011, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00011],
    // '|'      
    [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    // '}'      
    [0b11000, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b11000],
    // '~'      
    [0b00000, 0b00000, 0b01001, 0b10110, 0b00000, 0b00000, 0b00000],
];

/// The box drawn for a character this face has no glyph for.
///
/// Open rather than solid: a filled block reads as a redaction, and nothing has
/// been redacted. This is "the video cannot draw this letter", which is a
/// different thing and should not look like the other one.
const UNKNOWN: [u8; HEIGHT] = [
    0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111,
];

/// The rows of `character`, and whether the face actually had it.
///
/// The second half of the answer is what lets a caller say "three of these
/// names cannot be drawn" instead of quietly producing boxes.
pub fn glyph(character: char) -> ([u8; HEIGHT], bool) {
    if character < FIRST || character > LAST {
        return (UNKNOWN, false);
    }
    let at = character as usize - FIRST as usize;
    match GLYPHS.get(at) {
        Some(rows) => (*rows, true),
        None => (UNKNOWN, false),
    }
}

/// Whether every character in `text` can be drawn.
pub fn can_draw(text: &str) -> bool {
    text.chars().all(|c| glyph(c).1)
}

/// How wide `text` is at `scale`, in pixels.
pub fn width_of(text: &str, scale: usize) -> usize {
    let count = text.chars().count();
    if count == 0 {
        return 0;
    }
    // Every glyph but the last carries a gap after it.
    (count * WIDTH + (count - 1) * GAP) * scale
}

/// The largest whole-number scale at which `text` fits inside `room` pixels.
///
/// **Whole numbers only.** A bitmap glyph drawn at 2.5 times its size has to
/// put half a pixel somewhere, and every way of doing that is a blurred letter.
/// Rounding down to a whole multiple keeps every edge on a pixel boundary, so
/// the text is as crisp at 4K as it is at 720p, and one size smaller is a much
/// better outcome than one that is soft.
///
/// Never returns zero: a scale of zero draws nothing, and a name too long for
/// the space it was given should be drawn small and overflow rather than vanish.
pub fn scale_for(text: &str, room: usize) -> usize {
    let mut scale = 1;
    while width_of(text, scale + 1) <= room {
        scale += 1;
        // A frame is at most 7680 pixels across and a glyph is five, so nothing
        // legitimate reaches this. It is here because a loop that depends on
        // arithmetic not overflowing should say what stops it.
        if scale > 512 {
            break;
        }
    }
    scale
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Every printable ASCII character has a glyph of its own.
    ///
    /// The face is written out by hand, so a missing row is a real possibility
    /// and would show up as a box in the middle of an ordinary name.
    #[test]
    fn every_printable_ascii_character_can_be_drawn() {
        for code in (FIRST as u32)..=(LAST as u32) {
            let character = char::from_u32(code).expect("ascii");
            let (_, known) = glyph(character);
            assert!(known, "{character:?} has no glyph");
        }
        assert_eq!(GLYPHS.len(), (LAST as usize - FIRST as usize) + 1);
    }

    /// A character the face does not have says so rather than drawing nothing.
    #[test]
    fn anything_else_is_a_box_and_reports_itself() {
        for character in ['é', 'ß', 'д', '日', '👤'] {
            let (rows, known) = glyph(character);
            assert!(!known, "{character:?} was claimed as drawable");
            assert_eq!(rows, UNKNOWN);
        }
        assert!(!can_draw("Zoë"));
        assert!(can_draw("Alex"));
        assert!(can_draw(""));
    }

    /// No two characters share a glyph.
    ///
    /// Two identical rows in a hand-written face means two letters a reader
    /// cannot tell apart, which in a list of speaker names is the same failure
    /// as two voices nobody can separate.
    #[test]
    fn no_two_characters_are_drawn_the_same() {
        let mut seen: Vec<([u8; HEIGHT], char)> = Vec::new();
        for code in (FIRST as u32)..=(LAST as u32) {
            let character = char::from_u32(code).expect("ascii");
            if character == ' ' {
                continue;
            }
            let (rows, _) = glyph(character);
            if let Some((_, other)) = seen.iter().find(|(held, _)| *held == rows) {
                panic!("{character:?} and {other:?} are drawn identically");
            }
            seen.push((rows, character));
        }
    }

    /// A glyph only uses the five columns it claims to.
    #[test]
    fn nothing_is_drawn_outside_the_five_columns() {
        for code in (FIRST as u32)..=(LAST as u32) {
            let character = char::from_u32(code).expect("ascii");
            for (row, bits) in glyph(character).0.iter().enumerate() {
                assert!(
                    bits >> WIDTH == 0,
                    "{character:?} row {row} sets a bit past column {WIDTH}: {bits:#07b}"
                );
            }
        }
    }

    /// The scale is a whole number and the text fits at it.
    #[test]
    fn text_is_scaled_by_whole_numbers_and_fits() {
        for text in ["Alex", "a", "a much longer name than usual"] {
            for room in [10, 60, 200, 1000] {
                let scale = scale_for(text, room);
                assert!(scale >= 1, "{text:?} in {room} got scale {scale}");
                if scale > 1 {
                    assert!(
                        width_of(text, scale) <= room,
                        "{text:?} at {scale} is {} wide in {room}",
                        width_of(text, scale)
                    );
                }
                assert!(
                    width_of(text, scale + 1) > room,
                    "{text:?} could have been drawn one size larger in {room}"
                );
            }
        }
    }

    /// The empty string is nothing wide, at any scale.
    #[test]
    fn nothing_is_no_pixels_wide() {
        for scale in [1, 3, 12] {
            assert_eq!(width_of("", scale), 0);
        }
    }
}
