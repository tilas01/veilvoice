#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate VeilVoice's icon and banner.

The artwork is *generated*, not committed as opaque binaries, so anyone can see
exactly how it was made, tweak the palette, and reproduce byte-identical output.
That matters for a project whose whole pitch is verifiability: a binary blob in
the repository is one more thing a reader has to take on trust.

Pure standard library — no Pillow, no build step:
    python assets/generate.py

Outputs icon.png, icon.ico and banner.png next to this script.
"""

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
    """Draw `string` in the 5x7 font. Unknown characters are skipped."""
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

    # A full-width waveform band, clean on the left and veiled on the right,
    # echoing the icon at banner scale.
    # The band sits below the tagline; overlapping the two turns the hyphen in
    # "DE-IDENTIFICATION" into a plus sign and makes both harder to read.
    mid = 336
    seed = 0x5EED
    for i, x in enumerate(range(80, w - 80, 12)):
        seed = (seed * 1103515245 + 12345) & 0x7FFFFFFF
        progress = (x - 80) / (w - 160)
        base = 14 + int(48 * abs(((i * 7) % 23) / 23 - 0.5) * 2)
        if progress < 0.45:
            height, colour = base, BLUE
        else:
            wobble = (seed >> 7) % 44
            height, colour = max(5, base - 22 + wobble), PURPLE
            if (seed >> 3) % 5 == 0:
                continue  # a dropped bar: the signal coming apart
        rect(px, x, mid - height, 6, height * 2, colour)

    # Icon badge, top left.
    blit(px, scale(icon_32(), 4), 80, 80)

    # Wordmark.
    word_scale = 12
    text(px, "VEILVOICE", 240, 96, FG, word_scale)

    # Tagline and footer.
    text(px, "IRREVERSIBLE VOICE DE-IDENTIFICATION", 242, 204, BLUE, 4)
    text(px, "THE VOICEPRINT IS DESTROYED. THE WORDS STAY READABLE.", 80, 442, COMMENT, 3)
    text(px, "FULLY OFFLINE", 80, 498, GREEN, 3)
    text(px, "NO UNSAFE CODE", 80, 534, CYAN, 3)
    text(px, "GPL-3.0-OR-LATER", 80, 570, COMMENT, 3)

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


def check(here):
    """Verify the committed artwork still matches what this script produces."""
    mark = icon_32()
    expected = {
        "icon.png": scale(mark, 8),
        "icon-32.png": mark,
        "banner.png": banner(),
    }
    problems = []
    for name, pixels in expected.items():
        path = os.path.join(here, name)
        try:
            with open(path, "rb") as f:
                actual = decode_png(f.read())
        except (OSError, ValueError) as exc:
            problems.append(f"{name}: cannot read ({exc})")
            continue
        if actual != pixels:
            problems.append(f"{name}: pixels differ from the generator output")

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

    for name in ("icon.png", "icon-32.png", "icon-32.rgba", "icon.ico", "banner.png"):
        size = os.path.getsize(os.path.join(here, name))
        print(f"  {name:<16} {size:>8,} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
