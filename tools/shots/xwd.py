#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Turn an X window dump into a PNG, and fingerprint a PNG.

    python tools/shots/xwd.py capture.xwd out.png
    python tools/shots/xwd.py --fingerprint out.png

Used by `tools/shots/gui.sh`, which captures the screen with `xwd` because
that is the one screen-grabbing tool that is reliably present beside Xvfb.

# Why this exists rather than a call to ImageMagick

The same reason the rest of the image tooling here is written out: `convert`
is not installed everywhere, it is a large dependency to add for a format
conversion, and the format in question is a header and a block of pixels.
Pure standard library, like `tools/shots/round.py` and `assets/generate.py`.

# The fingerprint

`gui.sh` photographs nine tabs by starting the application nine times, and the
failure it has to catch is two of them coming out identical, which means a tab
did not open and the previous picture was taken again under a new name. That
is invisible in a directory listing and obvious in a hash. It samples on a
grid and masks the low bits of each channel, so a difference of one level in
software rendering does not read as a different tab.
"""

import struct
import sys
import zlib

# The X11 window dump header: 25 big-endian 32-bit words, then the colour map,
# then the pixels. These are the words this needs, by their position in that
# structure (`XWDFileHeader` in <X11/XWDFile.h>).
HEADER_WORDS = 25
COLOUR_MAP_ENTRY = 12
(HEADER_SIZE, WIDTH, HEIGHT, BYTE_ORDER, BITS_PER_PIXEL, BYTES_PER_LINE,
 RED_MASK, GREEN_MASK, BLUE_MASK, NCOLORS) = (0, 4, 5, 7, 11, 12, 14, 15, 16, 19)


def channel_offset(mask, depth_bytes, msb_first):
    """Which byte of a stored pixel holds the channel this mask selects.

    Read from the header rather than assumed. The first version of this file
    assumed 32 bits per pixel in BGRX order, which is what one machine
    happened to write; the Xvfb here writes 24-bit pixels and every capture
    was refused. A dump says what it contains, so this asks.
    """
    if mask == 0:
        raise SystemExit("the capture declares no mask for one of its channels")
    shift = (mask & -mask).bit_length() - 1
    index = shift // 8
    if index >= depth_bytes:
        raise SystemExit("a channel mask falls outside the pixel")
    return depth_bytes - 1 - index if msb_first else index


def decode(path):
    """(width, height, rows of RGB bytes) from an xwd file."""
    with open(path, "rb") as handle:
        data = handle.read()
    if len(data) < HEADER_WORDS * 4:
        raise SystemExit("%s: too short to be an xwd capture" % path)
    field = struct.unpack(">%dI" % HEADER_WORDS, data[:HEADER_WORDS * 4])
    width, height = field[WIDTH], field[HEIGHT]
    bits, line_bytes = field[BITS_PER_PIXEL], field[BYTES_PER_LINE]
    if bits not in (24, 32):
        raise SystemExit("%s: %d bits per pixel; this reads 24 and 32"
                         % (path, bits))
    depth_bytes = bits // 8
    msb_first = field[BYTE_ORDER] == 1
    red = channel_offset(field[RED_MASK], depth_bytes, msb_first)
    green = channel_offset(field[GREEN_MASK], depth_bytes, msb_first)
    blue = channel_offset(field[BLUE_MASK], depth_bytes, msb_first)

    start = field[HEADER_SIZE] + field[NCOLORS] * COLOUR_MAP_ENTRY
    pixels = data[start:]
    if len(pixels) < height * line_bytes:
        raise SystemExit("%s: %d bytes of pixels, expected %d"
                         % (path, len(pixels), height * line_bytes))

    # Sliced rather than looped over. A 1400x1000 capture is 1.4 million
    # pixels, and a Python loop over them takes long enough that nine of them
    # is a coffee break; three strided slices per row is the same work inside
    # the interpreter.
    span = width * depth_bytes
    rows = []
    for y in range(height):
        base = y * line_bytes
        row = pixels[base:base + span]
        out = bytearray(width * 3)
        out[0::3] = row[red::depth_bytes]
        out[1::3] = row[green::depth_bytes]
        out[2::3] = row[blue::depth_bytes]
        rows.append(bytes(out))
    return width, height, rows


def chunk(kind, body):
    return (struct.pack(">I", len(body)) + kind + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xffffffff))


def write_png(path, width, height, rows):
    raw = bytearray()
    for row in rows:
        raw.append(0)          # filter: none
        raw += row
    head = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    with open(path, "wb") as handle:
        handle.write(b"\x89PNG\r\n\x1a\n"
                     + chunk(b"IHDR", head)
                     + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
                     + chunk(b"IEND", b""))


def read_png(path):
    """Enough of a PNG reader for the files this tool wrote."""
    with open(path, "rb") as handle:
        data = handle.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit("%s: not a PNG" % path)
    at = 8
    width = height = None
    body = b""
    while at + 8 <= len(data):
        length = struct.unpack(">I", data[at:at + 4])[0]
        kind = data[at + 4:at + 8]
        payload = data[at + 8:at + 8 + length]
        if kind == b"IHDR":
            width, height, depth, colour = struct.unpack(">IIBB", payload[:10])
            if (depth, colour) not in ((8, 2), (8, 6)):
                raise SystemExit("%s: not 8-bit RGB or RGBA" % path)
            channels = 3 if colour == 2 else 4
        elif kind == b"IDAT":
            body += payload
        elif kind == b"IEND":
            break
        at += 12 + length

    stride = width * channels
    flat = zlib.decompress(body)
    rows = []
    previous = bytearray(stride)
    at = 0
    for _ in range(height):
        filter_kind = flat[at]
        line = bytearray(flat[at + 1:at + 1 + stride])
        at += 1 + stride
        for i in range(stride):
            left = line[i - channels] if i >= channels else 0
            up = previous[i]
            if filter_kind == 1:
                line[i] = (line[i] + left) & 0xff
            elif filter_kind == 2:
                line[i] = (line[i] + up) & 0xff
            elif filter_kind == 3:
                line[i] = (line[i] + ((left + up) >> 1)) & 0xff
            elif filter_kind == 4:
                upper_left = previous[i - channels] if i >= channels else 0
                estimate = left + up - upper_left
                da = abs(estimate - left)
                db = abs(estimate - up)
                dc = abs(estimate - upper_left)
                nearest = left if da <= db and da <= dc else (up if db <= dc else upper_left)
                line[i] = (line[i] + nearest) & 0xff
        rows.append(bytes(line))
        previous = line
    return width, height, channels, rows


def fingerprint(path):
    width, height, channels, rows = read_png(path)
    out = []
    for y in range(60, min(600, height), 17):
        row = rows[y]
        for x in range(20, min(900, width), 19):
            at = x * channels
            out.append(row[at] & 0xf0)
            out.append(row[at + 2] & 0xf0)
    return "%08x" % (zlib.crc32(bytes(out)) & 0xffffffff)


def main():
    args = sys.argv[1:]
    if args[:1] == ["--fingerprint"]:
        if len(args) != 2:
            raise SystemExit("usage: xwd.py --fingerprint <png>")
        print(fingerprint(args[1]))
        return 0
    if len(args) != 2:
        raise SystemExit(__doc__.split("\n\n")[0])
    width, height, rows = decode(args[0])
    write_png(args[1], width, height, rows)
    return 0


if __name__ == "__main__":
    sys.exit(main())
