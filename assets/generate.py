#!/usr/bin/env python3
# SPDX-License-Identifier: CC-BY-NC-SA-4.0
"""Generate VeilVoice's icon and banner.

The artwork is *generated*, not committed as opaque binaries, so anyone can see
exactly how it was made, tweak the palette, and reproduce byte-identical output.
That matters for a project whose whole pitch is verifiability: a binary blob in
the repository is one more thing a reader has to take on trust.

Pure standard library — no Pillow, no build step:
    python assets/generate.py

Outputs icon.png, icon.ico and banner.png next to this script.
"""

import math
import os
import struct
import sys
import zlib

# --- Tokyo Night ------------------------------------------------------------
BG        = (0x1a, 0x1b, 0x26, 255)   # editor background
BG_DARK   = (0x16, 0x16, 0x1e, 255)   # deeper panel
BORDER    = (0x41, 0x48, 0x68, 255)   # subtle outline
FG        = (0xc0, 0xca, 0xf5, 255)   # foreground text
BLUE      = (0x7a, 0xa2, 0xf7, 255)   # the "clean voice" side
CYAN      = (0x7d, 0xcf, 0xff, 255)
PURPLE    = (0xbb, 0x9a, 0xf7, 255)   # the "veiled voice" side
GREEN     = (0x9e, 0xce, 0x6a, 255)
COMMENT   = (0x56, 0x5f, 0x89, 255)   # muted text
NONE      = (0, 0, 0, 0)


# --- PNG encoding -----------------------------------------------------------
PNG_SIGNATURE = bytes((0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))

def write_png(path, pixels):
    """Write 8-bit RGBA rows (list of list of 4-tuples) as a PNG."""
    height = len(pixels)
    width = len(pixels[0])
    raw = bytearray()
    for row in pixels:
        raw.append(0)  # filter type 0 (None): keeps the output deterministic
        for r, g, b, a in row:
            raw += bytes((r, g, b, a))

    def chunk(kind, data):
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    blob = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )
    with open(path, "wb") as f:
        f.write(blob)
    return blob


def blank(width, height, colour=NONE):
    return [[colour for _ in range(width)] for _ in range(height)]


def scale(pixels, factor):
    """Nearest-neighbour upscale — pixel art must stay crisp, never blurred."""
    out = []
    for row in pixels:
        big = []
        for px in row:
            big.extend([px] * factor)
        out.extend([list(big)] * factor)
    return [list(r) for r in out]


def blit(dst, src, x0, y0):
    for y, row in enumerate(src):
        for x, px in enumerate(row):
            if px[3] == 0:
                continue
            ty, tx = y0 + y, x0 + x
            if 0 <= ty < len(dst) and 0 <= tx < len(dst[0]):
                dst[ty][tx] = px


def rect(dst, x0, y0, w, h, colour):
    for y in range(y0, y0 + h):
        for x in range(x0, x0 + w):
            if 0 <= y < len(dst) and 0 <= x < len(dst[0]):
                dst[y][x] = colour


# --- The mark ---------------------------------------------------------------
# A 32x32 pixel-art badge. A voice enters as an even, solid waveform on the left
# and leaves fragmented and recoloured on the right: the whole product in one
# picture. Bar heights are hand-tuned rather than generated, because a formula
# produces something that reads as noise at 16 pixels.
BARS = [
    #  x, half-height, veiled?
    (5, 3, False),
    (8, 6, False),
    (11, 9, False),
    (14, 5, False),
    (17, 8, True),
    (20, 4, True),
    (23, 9, True),
    (26, 6, True),
]

# Rows knocked out of the veiled bars, so they look dissolved rather than short.
GAPS = {17: (1,), 20: (), 23: (2, 5), 26: (3,)}


def icon_32():
    px = blank(32, 32, NONE)

    # Rounded-square body: corners cut by one pixel, which is all the rounding
    # that reads at this size.
    for y in range(32):
        for x in range(32):
            corner = (x < 2 and y < 2) or (x > 29 and y < 2) or (x < 2 and y > 29) or (x > 29 and y > 29)
            if corner:
                continue
            edge = x in (0, 1, 30, 31) or y in (0, 1, 30, 31)
            px[y][x] = BORDER if edge else BG_DARK

    centre = 16
    for x, half, veiled in BARS:
        colour = PURPLE if veiled else BLUE
        gaps = GAPS.get(x, ())
        for dy in range(-half, half):
            y = centre + dy
            if veiled and (abs(dy) in gaps):
                continue
            rect(px, x, y, 2, 1, colour)

    # A single cyan pixel pair at the centre line: the "signal still there".
    rect(px, 14, centre - 1, 2, 2, CYAN)
    return px


# --- 5x7 pixel font ---------------------------------------------------------
# Hand-drawn so the wordmark is genuinely pixel art rather than an anti-aliased
# system font shrunk down. '#' is ink, '.' is empty.
GLYPHS = {
    "A": ["01110", "10001", "10001", "11111", "10001", "10001", "10001"],
    "B": ["11110", "10001", "10001", "11110", "10001", "10001", "11110"],
    "C": ["01111", "10000", "10000", "10000", "10000", "10000", "01111"],
    "D": ["11110", "10001", "10001", "10001", "10001", "10001", "11110"],
    "E": ["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
    "F": ["11111", "10000", "10000", "11110", "10000", "10000", "10000"],
    "G": ["01111", "10000", "10000", "10111", "10001", "10001", "01111"],
    "H": ["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
    "I": ["11111", "00100", "00100", "00100", "00100", "00100", "11111"],
    "J": ["00111", "00010", "00010", "00010", "00010", "10010", "01100"],
    "K": ["10001", "10010", "10100", "11000", "10100", "10010", "10001"],
    "L": ["10000", "10000", "10000", "10000", "10000", "10000", "11111"],
    "M": ["10001", "11011", "10101", "10101", "10001", "10001", "10001"],
    "N": ["10001", "11001", "10101", "10011", "10001", "10001", "10001"],
    "O": ["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
    "P": ["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
    "Q": ["01110", "10001", "10001", "10001", "10101", "10010", "01101"],
    "R": ["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
    "S": ["01111", "10000", "10000", "01110", "00001", "00001", "11110"],
    "T": ["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
    "U": ["10001", "10001", "10001", "10001", "10001", "10001", "01110"],
    "V": ["10001", "10001", "10001", "10001", "10001", "01010", "00100"],
    "W": ["10001", "10001", "10001", "10101", "10101", "11011", "10001"],
    "X": ["10001", "10001", "01010", "00100", "01010", "10001", "10001"],
    "Y": ["10001", "10001", "01010", "00100", "00100", "00100", "00100"],
    "Z": ["11111", "00001", "00010", "00100", "01000", "10000", "11111"],
    "0": ["01110", "10001", "10011", "10101", "11001", "10001", "01110"],
    "1": ["00100", "01100", "00100", "00100", "00100", "00100", "01110"],
    "3": ["11110", "00001", "00001", "01110", "00001", "00001", "11110"],
    # The digit set used to be exactly 0, 1 and 3, because those were the only
    # ones the two strings on the banner needed. Changing the licence line to
    # "CC BY-NC-SA 4.0" asked for a `4`, and `text()` silently draws nothing for
    # a character it does not have -- so the banner would have shipped reading
    # "CC BY-NC-SA .0", on the social preview card, wrong about its own licence
    # and with every test passing. That is finding F-37 exactly: a banner
    # illegible about the project, invisible to the suite, obvious on sight.
    #
    # The whole set is defined now rather than the one digit that was wanted,
    # so the next string to go on the banner cannot reintroduce the same hole.
    # `check_glyphs()` below refuses to generate if one is ever missing again.
    "2": ["01110", "10001", "00001", "00010", "00100", "01000", "11111"],
    "4": ["00010", "00110", "01010", "10010", "11111", "00010", "00010"],
    "5": ["11111", "10000", "11110", "00001", "00001", "10001", "01110"],
    "6": ["00110", "01000", "10000", "11110", "10001", "10001", "01110"],
    "7": ["11111", "00001", "00010", "00100", "01000", "01000", "01000"],
    "8": ["01110", "10001", "10001", "01110", "10001", "10001", "01110"],
    "9": ["01110", "10001", "10001", "01111", "00001", "00010", "01100"],
    "-": ["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
    ".": ["00000", "00000", "00000", "00000", "00000", "01100", "01100"],
    ",": ["00000", "00000", "00000", "00000", "01100", "01100", "01000"],
    "'": ["00100", "00100", "00000", "00000", "00000", "00000", "00000"],
    ":": ["00000", "01100", "01100", "00000", "01100", "01100", "00000"],
    "/": ["00001", "00010", "00010", "00100", "01000", "01000", "10000"],
    "+": ["00000", "00100", "00100", "11111", "00100", "00100", "00000"],
    "*": ["00000", "10101", "01110", "11111", "01110", "10101", "00000"],
    " ": ["00000"] * 7,
}


def text(pixels, string, x0, y0, colour, scale_factor=1, spacing=1):
    """Draw `string` in the 5x7 font.

    A character with no glyph is a **hard error**, not a silent gap.

    It used to advance the cursor and draw nothing, which meant a string could
    contain a character this font had never defined and the only symptom was a
    blank space in the finished artwork. Changing the licence line to
    "CC BY-NC-SA 4.0" hit exactly that -- there was no `4` -- and the banner
    would have gone out reading "CC BY-NC-SA .0" on GitHub's social preview
    card, stating the wrong licence, with `--check` passing because the
    generator and the committed file agreed with each other about the same
    mistake.

    That is finding F-37's shape a second time: artwork that is wrong about the
    project, invisible to every test, obvious the moment somebody looks. A
    generator that cannot draw what it was asked to draw should say so.
    """
    missing = sorted({ch for ch in string.upper() if ch not in GLYPHS})
    if missing:
        raise SystemExit(
            ("assets/generate.py: no glyph for %s in %r." + chr(10) +
             "Add it to GLYPHS rather than letting the banner render a gap.")
            % (", ".join(repr(ch) for ch in missing), string))
    cursor = x0
    for ch in string.upper():
        glyph = GLYPHS.get(ch)
        if glyph is None:
            cursor += (5 + spacing) * scale_factor
            continue
        for gy, row in enumerate(glyph):
            for gx, cell in enumerate(row):
                if cell != "1":
                    continue
                rect(
                    pixels,
                    cursor + gx * scale_factor,
                    y0 + gy * scale_factor,
                    scale_factor,
                    scale_factor,
                    colour,
                )
        cursor += (5 + spacing) * scale_factor
    return cursor


def text_width(string, scale_factor=1, spacing=1):
    return len(string) * (5 + spacing) * scale_factor - spacing * scale_factor


# --- Banner -----------------------------------------------------------------
# --- the waveform band ------------------------------------------------------
#
# The band sits below the tagline; overlapping the two turns the hyphen in
# "DE-IDENTIFICATION" into a plus sign and makes both harder to read.
#
# # What the picture is saying, and why no bar is missing any more
#
# The left half is a voice: a smooth travelling wave, every bar in step with
# its neighbours. The right half is the same voice after VeilVoice: the same
# bars, the same energy, but the phase relationship between them is gone.
#
# An earlier version expressed that by *deleting* one bar in five. It read as a
# broken image rather than as a design -- the first thing anyone said about it
# was that bars were missing -- and it was also the wrong idea. VeilVoice does
# not remove parts of the signal; it keeps the words and destroys the structure
# that identifies the speaker. Gaps say "data lost". Incoherence says
# "voiceprint destroyed, words intact", which is the actual claim.
#
# Animated, this reads immediately: the left marches, the right seethes.

BAND_MID = 336          # vertical centre of the waveform

# The animated rectangle, cropped to what actually moves.
#
# An APNG frame may declare a sub-rectangle, and every byte outside it is not
# in the file at all. The first version declared a generous 1280x156 band "so
# no frame can clip a tall bar" -- which was true and cost about a quarter of
# the file for rows and columns that never change. The bars reach BAR_MAX above
# and below BAND_MID and span BAR_LEFT to width-BAR_LEFT, so the box below is
# that, plus two pixels of margin for the anti-aliased ends.
#
# The margin is not decoration: a bar end is blended into the row *outside* its
# whole-pixel height, so a rectangle cropped exactly to BAR_MAX would clip the
# very edge this animation exists to make smooth.
BAND_TOP = 336 - 62 - 2
BAND_HEIGHT = (62 + 2) * 2
BAND_X = 78
BAND_WIDTH = 1128
BAR_STEP = 12
BAR_WIDTH = 6
BAR_LEFT = 80
BAR_MAX = 62

# How many levels of partial coverage an anti-aliased bar end may take. See the
# note in `draw_bar`: this trades an invisible amount of precision for a large
# amount of compressibility.
AA_STEPS = 16


# Peak of the harmonic sums below, measured rather than assumed.
#
# Three sinusoids whose amplitudes sum to 1.0 only reach 1.0 if they all peak
# together, and the phase offsets are there precisely so they do not. Without
# this the band quietly lost a fifth of its height when the harmonics were
# added -- the animation still worked, still looped, and simply looked smaller,
# which is the kind of change no test notices.
#
# Sampled from the same expressions the function uses, at import, so editing an
# amplitude re-measures instead of leaving a stale constant behind.
def _wave_peak():
    coherent = 0.0
    incoherent = 0.0
    steps = 720
    for step in range(steps):
        angle = 2.0 * math.pi * step / steps
        coherent = max(coherent, abs(
            0.62 * math.sin(angle)
            + 0.26 * math.sin(2.0 * angle + 0.9)
            + 0.12 * math.sin(3.0 * angle + 2.1)))
        for extra in (1.0, 2.0, 3.0):
            incoherent = max(incoherent, abs(
                0.70 * math.sin(angle)
                + 0.30 * math.sin(angle * (extra + 1.0) / extra)))
    return coherent, incoherent


WAVE_PEAK_COHERENT, WAVE_PEAK_INCOHERENT = _wave_peak()


def _wave(i, phase, coherent):
    """Half-height of bar `i` at animation `phase` (0.0 to 1.0), in *fractional*
    pixels.

    Deterministic in both arguments, so every frame is reproducible and the loop
    closes exactly: `phase` enters only through `sin`, always at a whole-number
    multiple of the base frequency, and the integer hash below does not depend
    on it.

    Returns a float on purpose. Rounding each height to a whole pixel is what
    made the first animation look stepped rather than fluid: a bar near the top
    of its travel changes by a fraction of a pixel per frame, so it sat still
    for several frames and then jumped. `draw_bar` renders the fractional part
    as partial coverage instead, which is what the browser does for the CSS bars
    this is meant to match.

    # Why three harmonics rather than one

    A single sinusoid is the shape of a test tone. Speech is a fundamental plus
    harmonics whose amplitudes fall off, and its energy breathes rather than
    holding steady -- so the bars are summed from three components at 1x, 2x and
    3x the base rate, with the amplitudes falling roughly as 1/k, plus a slow
    envelope at 1x.

    **Every multiplier is a whole number, and that is load-bearing.**
    `sin(k * 2*pi*phase + c)` has period exactly 1 in `phase` for integer `k`,
    so the frame at phase 1.0 is byte-identical to the frame at 0.0 no matter
    how many terms are added. A rate of, say, 1.7 would look perfectly good in
    any single frame and tear once per loop, which is the sort of defect that
    survives review because nobody watches an animation for a whole cycle.

    The amplitudes are chosen to sum to 1.0 so the result stays in [-1, 1] and
    the band cannot overflow its own height.
    """
    if coherent:
        # A travelling wave: neighbouring bars differ by a fixed phase step, so
        # the crest walks along the band. The harmonics inherit that step
        # multiplied, which is what gives a real waveform its shorter ripples
        # riding on the fundamental.
        angle = 2.0 * math.pi * (phase - i * 0.085)
        wave = (
            0.62 * math.sin(angle)
            + 0.26 * math.sin(2.0 * angle + 0.9)
            + 0.12 * math.sin(3.0 * angle + 2.1)
        )
        shape = 0.5 + 0.5 * (wave / WAVE_PEAK_COHERENT)
        # A gentle standing envelope so the band is not a rectangle of equal
        # peaks, and a slow breath so the whole thing does not pulse in lockstep.
        envelope = 0.62 + 0.38 * abs(((i * 7) % 23) / 23.0 - 0.5) * 2.0
        breath = 0.88 + 0.12 * math.sin(angle * 1.0 - i * 0.31)
        return BAR_MAX * (0.20 + 0.80 * shape) * envelope * breath

    # Incoherent: each bar keeps its own pseudo-random phase and rate, so the
    # bars never line up. Same energy, no shared structure -- which is the
    # picture of what VeilVoice does to the phase relationships in a voice.
    h = (i * 2654435761) & 0xFFFFFFFF
    offset = ((h >> 7) % 1000) / 1000.0
    # Whole-number rates only, for the same loop-closing reason as above.
    rate = 1.0 + float((h >> 17) % 3)
    angle = 2.0 * math.pi * (phase * rate + offset)
    second = 2.0 * math.pi * (phase * (rate + 1.0) + offset * 1.7)
    wave = 0.70 * math.sin(angle) + 0.30 * math.sin(second)
    shape = 0.5 + 0.5 * (wave / WAVE_PEAK_INCOHERENT)
    envelope = 0.55 + 0.45 * (((h >> 3) % 100) / 100.0)
    breath = 0.86 + 0.14 * math.sin(2.0 * math.pi * phase + offset * 6.283)
    return BAR_MAX * (0.18 + 0.82 * shape) * envelope * breath


def _blend(fg, bg, alpha):
    """`fg` over `bg` at `alpha`, rounded to whole channel values.

    Both are opaque here, so this is a plain linear interpolation. Rounding
    with `int(v + 0.5)` rather than truncating keeps the two ends symmetric --
    truncation biases every edge pixel one step towards the background, which
    over a whole band reads as bars that are subtly too short.
    """
    return (
        int(fg[0] * alpha + bg[0] * (1.0 - alpha) + 0.5),
        int(fg[1] * alpha + bg[1] * (1.0 - alpha) + 0.5),
        int(fg[2] * alpha + bg[2] * (1.0 - alpha) + 0.5),
        255,
    )


def draw_bar(dst, x, centre, half, w, colour):
    """One bar, centred on `centre`, with anti-aliased ends.

    `half` is fractional. The rows fully inside the bar are painted flat; the
    single row at each end is blended against whatever is already there by the
    fraction it actually covers. That is the whole of the smoothness fix: the
    bar's apparent height now changes continuously instead of in whole pixels,
    so at 60 frames a second the crest glides rather than clicking from one row
    to the next.
    """
    top = centre - half
    bottom = centre + half

    first = int(math.floor(top))
    last = int(math.ceil(bottom))

    for y in range(first, last):
        if y < 0 or y >= len(dst):
            continue
        # How much of this one-pixel row the bar covers, in [0, 1].
        covered = min(bottom, y + 1.0) - max(top, float(y))
        if covered <= 0.0:
            continue

        # Quantise coverage to sixteen steps.
        #
        # Continuous coverage produces a different edge colour on almost every
        # frame, and a PNG's compression works on repeated bytes -- so the file
        # grew from 300 KB to 405 KB for a difference no eye can see at 1/60 s.
        # Sixteen steps is finer than the eye can separate at this size and
        # gives the compressor something to find: the animation stays fluid and
        # the download stops being larger than the rest of the site put
        # together.
        covered = round(covered * AA_STEPS) / AA_STEPS
        if covered <= 0.0:
            continue
        for px_x in range(x, x + w):
            if px_x < 0 or px_x >= len(dst[0]):
                continue
            if covered >= 0.999:
                dst[y][px_x] = colour
            else:
                dst[y][px_x] = _blend(colour, dst[y][px_x], covered)


def draw_band(px, phase, width=None, y_offset=0, x_offset=0):
    """Draw the waveform band into `px` at the given animation phase.

    `y_offset` shifts the drawing up by that many rows, so the same routine can
    fill the whole banner (offset 0) or just the strip an APNG frame replaces
    (offset BAND_TOP). One drawing routine, one coordinate convention: the
    alternative is two, and the second one is where the bug goes.
    """
    if width is None:
        width = len(px[0])
    for i, x in enumerate(range(BAR_LEFT, width - BAR_LEFT, BAR_STEP)):
        progress = (x - BAR_LEFT) / float(width - 2 * BAR_LEFT)
        coherent = progress < 0.45
        half = max(2.0, _wave(i, phase, coherent))
        colour = BLUE if coherent else PURPLE
        draw_bar(px, x - x_offset, BAND_MID - y_offset, half, BAR_WIDTH, colour)


def band_strip(phase, background):
    """Just the animated rectangle, for one APNG frame.

    Drawn onto a copy of the *static banner's* own pixels for that region, so
    the faint grid behind the bars is preserved and each frame replaces the
    region outright. That is why the frames declare `blend_op = SOURCE`: there
    is nothing to blend against, the strip is already complete.
    """
    rows = [
        list(background[y][BAND_X:BAND_X + BAND_WIDTH])
        for y in range(BAND_TOP, BAND_TOP + BAND_HEIGHT)
    ]
    draw_band(
        rows,
        phase,
        width=len(background[0]),
        y_offset=BAND_TOP,
        x_offset=BAND_X,
    )
    return rows


def banner():
    """GitHub's social-preview card is 1280x640."""
    w, h = 1280, 640
    px = blank(w, h, BG)

    # A faint 16-pixel grid: texture without competing with the wordmark.
    grid = (0x1f, 0x21, 0x30, 255)
    for y in range(0, h, 16):
        for x in range(w):
            px[y][x] = grid
    for x in range(0, w, 16):
        for y in range(h):
            px[y][x] = grid

    draw_band(px, 0.0)

    # Icon badge, top left.
    blit(px, scale(icon_32(), 4), 80, 80)

    # Wordmark.
    word_scale = 12
    text(px, "VEILVOICE", 240, 96, FG, word_scale)

    # Tagline and footer.
    text(px, "IRREVERSIBLE VOICE DE-IDENTIFICATION", 242, 204, BLUE, 4)
    text(px, "THE VOICEPRINT IS DESTROYED. THE WORDS STAY READABLE.", 80, 430, COMMENT, 3)
    text(px, "FULLY OFFLINE", 80, 478, GREEN, 3)
    text(px, "SECURE AUDITED RUST CODE", 80, 512, CYAN, 3)
    text(px, "CC BY-NC-SA 4.0", 80, 546, COMMENT, 3)
    # Attribution, in the same green as the offline claim.
    text(px, "BY TILAS01 ON GITHUB", 80, 580, GREEN, 3)

    # A hairline accent along the bottom edge.
    rect(px, 0, h - 8, w, 8, BLUE)
    return px


# --- Windows ICO ------------------------------------------------------------
def write_ico(path, sizes, source_32):
    """An ICO whose entries are embedded PNGs (supported since Windows Vista)."""
    images = []
    for size in sizes:
        factor = max(1, size // 32)
        images.append((size, write_png_bytes(scale(source_32, factor))))

    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries, blobs = b"", b""
    for size, blob in images:
        # 0 in the width/height byte means 256.
        dim = 0 if size >= 256 else size
        entries += struct.pack(
            "<BBBBHHII", dim, dim, 0, 0, 1, 32, len(blob), offset
        )
        blobs += blob
        offset += len(blob)

    with open(path, "wb") as f:
        f.write(header + entries + blobs)


def write_png_bytes(pixels):
    height = len(pixels)
    width = len(pixels[0])
    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for r, g, b, a in row:
            raw += bytes((r, g, b, a))

    def chunk(kind, data):
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


# --- APNG -------------------------------------------------------------------
#
# An animated banner, in the same spirit as everything else here: generated
# from source, not a committed blob somebody has to trust.
#
# APNG rather than GIF, for three reasons that all matter to this project:
#
#   * It is a PNG. The encoder above already writes PNG chunks, so this is an
#     extension of code that is already here and already reviewed, not a second
#     image format with its own quantiser.
#   * GIF is limited to 256 colours, so the palette would have to be quantised
#     -- a lossy step whose output depends on the quantiser, which is exactly
#     the kind of thing that stops a build being reproducible.
#   * A browser that does not understand APNG shows the first frame, which is
#     the static banner. The failure mode is "no animation", never "no image".
#
# Only the waveform band is animated. Frame 0 is the whole banner; every later
# frame declares a sub-rectangle covering the band alone, which is why the file
# is a fraction of the size of a full-frame animation.

APNG_DISPOSE_NONE = 0
APNG_BLEND_SOURCE = 0


def _chunk(kind, data):
    return (
        struct.pack(">I", len(data))
        + kind
        + data
        + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
    )


def _raw_rows(pixels):
    raw = bytearray()
    for row in pixels:
        raw.append(0)  # filter 0 (None), for deterministic output
        for r, g, b, a in row:
            raw += bytes((r, g, b, a))
    return bytes(raw)


def write_apng(path, frames, delay_num, delay_den):
    """Write an APNG.

    `frames` is a list of `(x, y, pixels)`. The first must be the full image and
    sit at the origin; the rest are sub-rectangles that replace what is under
    them. The delay is a *rational* number of seconds, `delay_num/delay_den`,
    because that is what the format stores and because 60 frames per second is
    1/60 -- a value milliseconds cannot express without rounding (16 ms is
    62.5 fps, 17 ms is 58.8) and rounding a frame interval is what makes an
    animation visibly stutter.
    """
    first_x, first_y, first = frames[0]
    if (first_x, first_y) != (0, 0):
        raise ValueError("the first APNG frame must be the whole image")
    width = len(first[0])
    height = len(first)

    out = [PNG_SIGNATURE]
    out.append(_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)))
    # num_plays 0 means loop for ever.
    out.append(_chunk(b"acTL", struct.pack(">II", len(frames), 0)))

    def fctl(sequence, x, y, pixels):
        return _chunk(b"fcTL", struct.pack(
            ">IIIIIHHBB",
            sequence,
            len(pixels[0]), len(pixels),
            x, y,
            delay_num, delay_den,    # delay as an exact fraction of a second
            APNG_DISPOSE_NONE, APNG_BLEND_SOURCE,
        ))

    sequence = 0
    out.append(fctl(sequence, 0, 0, first))
    sequence += 1
    out.append(_chunk(b"IDAT", zlib.compress(_raw_rows(first), 9)))

    for x, y, pixels in frames[1:]:
        out.append(fctl(sequence, x, y, pixels))
        sequence += 1
        out.append(_chunk(
            b"fdAT",
            struct.pack(">I", sequence) + zlib.compress(_raw_rows(pixels), 9),
        ))
        sequence += 1

    out.append(_chunk(b"IEND", b""))
    blob = b"".join(out)
    with open(path, "wb") as handle:
        handle.write(blob)
    return blob


def decode_apng(blob):
    """Decode our own APNG back to `(x, y, pixels)` frames.

    Exists for `--check`, and for the same reason `decode_png` does: zlib's
    output differs between Python builds, so comparing compressed bytes would
    fail spuriously while saying nothing about whether the picture changed.
    Frames are compared as pixels.
    """
    if blob[:8] != PNG_SIGNATURE:
        raise ValueError("not a PNG")

    pos = 8
    width = height = None
    frames = []
    pending = None      # (x, y, w, h) from the most recent fcTL
    data = b""

    def flush():
        if pending is None:
            return
        x, y, w, h = pending
        raw = zlib.decompress(data)
        stride = w * 4
        rows = []
        for row_index in range(h):
            start = row_index * (stride + 1)
            if raw[start] != 0:
                raise ValueError("expected filter type 0")
            line = raw[start + 1:start + 1 + stride]
            rows.append([tuple(line[i * 4:i * 4 + 4]) for i in range(w)])
        frames.append((x, y, rows))

    while pos < len(blob):
        length = struct.unpack(">I", blob[pos:pos + 4])[0]
        kind = blob[pos + 4:pos + 8]
        payload = blob[pos + 8:pos + 8 + length]

        if kind == b"IHDR":
            width, height, depth, colour = struct.unpack(">IIBB", payload[:10])
            if (depth, colour) != (8, 6):
                raise ValueError("expected 8-bit RGBA")
        elif kind == b"fcTL":
            flush()
            _, w, h, x, y = struct.unpack(">IIIII", payload[:20])
            pending = (x, y, w, h)
            data = b""
        elif kind == b"IDAT":
            data += payload
        elif kind == b"fdAT":
            data += payload[4:]     # skip the sequence number
        elif kind == b"IEND":
            flush()
            break
        pos += 12 + length

    return frames


# How the animation is shaped.
#
# **Sixty frames per second, one full cycle per second.** The first version ran
# 24 frames at 70 ms -- about 14 fps -- which is fine for a blinking cursor and
# far too coarse for a travelling wave: the crest visibly jumped from bar to
# bar instead of moving along them.
#
# The delay is exactly 1/60 s rather than a rounded 16 or 17 ms. At 16 ms the
# loop runs 0.96 s and at 17 ms it runs 1.02 s, and either way the frame
# interval no longer divides the display's own 60 Hz refresh evenly, which is
# what produces a stutter that is hard to name but easy to see.
#
# The phase of frame `i` is `i / FRAMES`, so the last frame lands exactly one
# cycle from the first: the loop closes with no seam and no repeated frame.
BANNER_FRAMES = 60
BANNER_DELAY_NUM = 1
BANNER_DELAY_DEN = 60


def banner_frames():
    """The animated banner: frame 0 whole, the rest just the waveform band."""
    base = banner()
    frames = [(0, 0, base)]
    for index in range(1, BANNER_FRAMES):
        phase = index / float(BANNER_FRAMES)
        frames.append((BAND_X, BAND_TOP, band_strip(phase, base)))
    return frames


def decode_png(blob):
    """Decode the narrow PNG subset this script writes: 8-bit RGBA, filter 0.

    Only needs to read our own output, so it skips the general cases. It exists
    for `--check`: comparing decoded pixels rather than compressed bytes makes
    the reproducibility test independent of the zlib version, which differs
    between Python builds and would otherwise cause spurious CI failures.
    """
    if blob[:8] != PNG_SIGNATURE:
        raise ValueError("not a PNG")
    pos, width, height, idat = 8, None, None, b""
    while pos < len(blob):
        length = struct.unpack(">I", blob[pos:pos + 4])[0]
        kind = blob[pos + 4:pos + 8]
        data = blob[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            width, height, depth, colour = struct.unpack(">IIBB", data[:10])
            if (depth, colour) != (8, 6):
                raise ValueError("expected 8-bit RGBA")
        elif kind == b"IDAT":
            idat += data
        elif kind == b"IEND":
            break
        pos += 12 + length

    raw = zlib.decompress(idat)
    stride = width * 4
    rows = []
    for y in range(height):
        start = y * (stride + 1)
        if raw[start] != 0:
            raise ValueError("expected filter type 0")
        line = raw[start + 1:start + 1 + stride]
        rows.append([tuple(line[x * 4:x * 4 + 4]) for x in range(width)])
    return rows


# The website serves its own copies of the artwork rather than reaching up out
# of `website/`, which keeps that directory exactly what GitHub Pages publishes.
# The copies were previously kept in step by hand, and `--check` only ever
# looked at `assets/` -- so `website/assets/banner.png` could drift from its
# generator and nothing would notice. It had. The generator now writes both
# and checks both, which is the only version of this that stays true.
WEB_COPIES = ("icon.png", "icon-32.png", "banner.png", "banner-animated.png")


def website_assets(here):
    return os.path.join(os.path.dirname(here), "website", "assets")


def check(here):
    """Verify the committed artwork still matches what this script produces."""
    mark = icon_32()
    expected = {
        "icon.png": scale(mark, 8),
        "icon-32.png": mark,
        "banner.png": banner(),
    }
    web = website_assets(here)
    problems = []

    def check_png(path, label, pixels):
        try:
            with open(path, "rb") as handle:
                actual = decode_png(handle.read())
        except (OSError, ValueError) as exc:
            problems.append(f"{label}: cannot read ({exc})")
            return
        if actual != pixels:
            problems.append(f"{label}: pixels differ from the generator output")

    def check_apng(path, label, want_frames):
        try:
            with open(path, "rb") as handle:
                actual_frames = decode_apng(handle.read())
        except (OSError, ValueError) as exc:
            problems.append(f"{label}: cannot read ({exc})")
            return
        if len(actual_frames) != len(want_frames):
            problems.append("%s: %d frames, generator produces %d"
                            % (label, len(actual_frames), len(want_frames)))
            return
        for index, (want, got) in enumerate(zip(want_frames, actual_frames)):
            if (want[0], want[1]) != (got[0], got[1]) or want[2] != got[2]:
                problems.append("%s: frame %d differs from the generator"
                                % (label, index))
                return

    for name, pixels in expected.items():
        check_png(os.path.join(here, name), name, pixels)
        if name in WEB_COPIES:
            check_png(os.path.join(web, name), "website/assets/" + name, pixels)

    frames = banner_frames()
    check_apng(os.path.join(here, "banner-animated.png"),
               "banner-animated.png", frames)
    check_apng(os.path.join(web, "banner-animated.png"),
               "website/assets/banner-animated.png", frames)

    raw_path = os.path.join(here, "icon-32.rgba")
    want = bytes(b for row in mark for px in row for b in px)
    try:
        with open(raw_path, "rb") as f:
            if f.read() != want:
                problems.append("icon-32.rgba: bytes differ")
    except OSError as exc:
        problems.append(f"icon-32.rgba: cannot read ({exc})")

    if problems:
        for line in problems:
            print(f"  MISMATCH {line}")
        print()
        print("Run 'python assets/generate.py' and commit the result.")
        return 1
    print("  all generated assets match the generator")
    return 0


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    if "--check" in sys.argv:
        return check(here)
    mark = icon_32()

    write_png(os.path.join(here, "icon.png"), scale(mark, 8))       # 256x256
    write_png(os.path.join(here, "icon-32.png"), mark)              # 1:1 source

    # Raw RGBA for the window icon. The GUI embeds this with `include_bytes!`,
    # so the application needs no PNG decoder just to draw its own title bar.
    with open(os.path.join(here, "icon-32.rgba"), "wb") as f:
        for row in mark:
            for r, g, b, a in row:
                f.write(bytes((r, g, b, a)))
    write_ico(os.path.join(here, "icon.ico"), [16, 32, 48, 64, 128, 256], mark)
    write_png(os.path.join(here, "banner.png"), banner())

    # The animated banner. Frame 0 is byte-for-byte the picture above, so a
    # viewer with no APNG support shows the static banner rather than nothing.
    write_apng(
        os.path.join(here, "banner-animated.png"),
        banner_frames(),
        BANNER_DELAY_NUM,
        BANNER_DELAY_DEN,
    )

    # And the website's own copies, from the same run rather than by hand.
    web = website_assets(here)
    os.makedirs(web, exist_ok=True)
    for name in WEB_COPIES:
        with open(os.path.join(here, name), "rb") as source:
            blob = source.read()
        with open(os.path.join(web, name), "wb") as target:
            target.write(blob)

    for name in ("icon.png", "icon-32.png", "icon-32.rgba", "icon.ico",
                 "banner.png", "banner-animated.png"):
        size = os.path.getsize(os.path.join(here, name))
        print(f"  {name:<20} {size:>9,} bytes")
    print(f"  copied {len(WEB_COPIES)} of them into website/assets/")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
