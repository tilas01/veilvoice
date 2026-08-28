#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Trim the black border the window capture leaves around each screenshot.

    python tools/shots/crop.py            # trim, in place
    python tools/shots/crop.py --check    # verify there is nothing left to trim

# What this is for

`tools/shots/gui.ps1` captures the window's DWM frame, which is the right
rectangle to ask for: `GetWindowRect` includes the invisible resize border and
the drop shadow, and using it put a strip of desktop down two edges of every
picture. The frame is closer, and it is still not exact. Measured on the
committed captures: **eleven columns of pure black down each side, one row along
the top and two along the bottom.**

Eleven pixels of nothing is not a disaster and it is not nothing either. It is
the difference between a picture that sits in a page and a picture that has a
ragged black margin somebody has to look past, and on a rounded container it is
the part that stops the rounding from meeting the content.

So the border comes off here rather than in the capture script. Two reasons.
The capture runs on Windows, where the window is; this runs everywhere,
including in CI, so the check that it stayed off runs everywhere too. And the
exact border depends on the display, the theme and the compositor, so a number
typed into the capture script would be right on one machine.

# Why this is a crop and not a filter

Nothing is redrawn, recoloured or resampled. Rows and columns are removed, and
only ones that are **entirely a single opaque colour that is also the colour of
the corner**, which is the shape a capture border has and is not the shape
anything the application draws has. The picture that remains is the same pixels
the application drew, which is what a screenshot has to be for it to be worth
committing at all.

# It has to be safe to run twice

Every generator in this repository is checked by running it again and comparing,
so this one has to reach a fixed point. It does: once the border is gone, the
first row is no longer uniform, so a second run removes nothing. `--check` is
exactly that second run, and it fails if anything would still come off.

A guard on top of that: nothing is cropped if it would take more than
`MAX_TRIM` from a side, or leave an image smaller than `MIN_SIZE`. A capture
that went wrong should be looked at rather than quietly shaved down to a strip.

# In plain words

The pictures of the application had a thin black edge around them, left over
from how a window is photographed. This takes it off, and refuses to take off
anything that is not obviously that edge.

Pure standard library: a small PNG reader and writer, because adding an image
library to this repository to remove eleven pixels would be a poor trade.
"""

import argparse
import os
import struct
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# Where the captures live, and where the website's copies have to match.
SOURCES = ["assets/screenshots", "website/assets/screenshots"]

# The most that may come off one side, and the smallest picture left behind.
# A capture that needs more than this is a capture that went wrong.
MAX_TRIM = 64
MIN_SIZE = 200

# How dark every channel has to be for a uniform edge to count as capture
# border rather than as something the application drew.
#
# This is load-bearing, and it was found by running the tool without it. With
# the black gone, the top row of the picture is the **title bar**, which is a
# uniform blue all the way across, so a rule that trims any uniform edge
# happily ate nine rows of it and would have kept going on the next run. The
# border a window capture leaves is the desktop showing through, and here that
# is pure black; a title bar is not, a panel is not, and the application's own
# background is never uniform to the edge because it has a border and a header
# in it.
#
# So: uniform, opaque, and dark on every channel. Sixteen rather than zero
# because a compositor may hand back 1 or 2 rather than a clean 0, and a rule
# that misses the border on one machine is a rule that does nothing.
DARKEST_BORDER = 16


# --- PNG ---------------------------------------------------------------------

def read_png(path):
    """An 8-bit, non-interlaced PNG as (width, height, channels, rows).

    Refuses anything else rather than guessing. Every file this reads is one
    written by the capture script or by this one.
    """
    with open(path, "rb") as handle:
        data = handle.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit("%s: not a PNG" % path)
    pos = 8
    idat = bytearray()
    width = height = channels = None
    while pos < len(data):
        length = struct.unpack(">I", data[pos:pos + 4])[0]
        kind = data[pos + 4:pos + 8]
        body = data[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or interlace != 0:
                raise SystemExit("%s: only 8-bit non-interlaced PNGs are handled" % path)
            channels = {0: 1, 2: 3, 4: 2, 6: 4}.get(colour)
            if channels is None:
                raise SystemExit("%s: unsupported colour type %d" % (path, colour))
        elif kind == b"IDAT":
            idat += body
        pos += 12 + length

    raw = zlib.decompress(bytes(idat))
    stride = width * channels
    rows = []
    previous = bytearray(stride)
    at = 0
    for _ in range(height):
        kind = raw[at]
        at += 1
        line = bytearray(raw[at:at + stride])
        at += stride
        # The five PNG filters, in the order the specification numbers them.
        if kind == 1:
            for x in range(channels, stride):
                line[x] = (line[x] + line[x - channels]) & 255
        elif kind == 2:
            for x in range(stride):
                line[x] = (line[x] + previous[x]) & 255
        elif kind == 3:
            for x in range(stride):
                left = line[x - channels] if x >= channels else 0
                line[x] = (line[x] + ((left + previous[x]) >> 1)) & 255
        elif kind == 4:
            for x in range(stride):
                left = line[x - channels] if x >= channels else 0
                up = previous[x]
                corner = previous[x - channels] if x >= channels else 0
                guess = left + up - corner
                da, db, dc = abs(guess - left), abs(guess - up), abs(guess - corner)
                if da <= db and da <= dc:
                    best = left
                elif db <= dc:
                    best = up
                else:
                    best = corner
                line[x] = (line[x] + best) & 255
        elif kind != 0:
            raise SystemExit("%s: unknown filter %d" % (path, kind))
        rows.append(bytes(line))
        previous = line
    return width, height, channels, rows


def write_png(path, channels, rows):
    """Write back, filter 0 on every line.

    Not the smallest possible file, and deliberately: a fixed filter means the
    bytes are a function of the pixels alone, so `--check` compares images
    rather than compression choices.
    """
    colour = {1: 0, 2: 4, 3: 2, 4: 6}[channels]
    width = len(rows[0]) // channels
    header = struct.pack(">IIBBBBB", width, len(rows), 8, colour, 0, 0, 0)

    def chunk(kind, body):
        return (struct.pack(">I", len(body)) + kind + body
                + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF))

    raw = bytearray()
    for line in rows:
        raw.append(0)
        raw += line
    blob = (b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
            + chunk(b"IEND", b""))
    with open(path, "wb") as handle:
        handle.write(blob)


# --- the crop ----------------------------------------------------------------

def pixel(rows, channels, x, y):
    at = x * channels
    return rows[y][at:at + channels]


def border(width, height, channels, rows):
    """How many rows and columns of capture border are on each side.

    A side is trimmed only while every pixel along it is the same colour as the
    corner, and that colour is opaque. Anything else is the application, and
    the application's own background is not uniform to the edge: it has a
    border, a header and a panel in it.
    """
    corner = pixel(rows, channels, 0, 0)
    if channels == 4 and corner[3] != 255:
        return 0, 0, 0, 0
    if any(value > DARKEST_BORDER for value in corner[:3]):
        # Not a capture border. See DARKEST_BORDER: without this the tool
        # trimmed the title bar, which is uniform and is not desktop.
        return 0, 0, 0, 0

    def row_is_border(y):
        return all(pixel(rows, channels, x, y) == corner for x in range(width))

    def column_is_border(x):
        return all(pixel(rows, channels, x, y) == corner for y in range(height))

    top = 0
    while top < height and row_is_border(top):
        top += 1
    bottom = 0
    while bottom < height - top and row_is_border(height - 1 - bottom):
        bottom += 1
    left = 0
    while left < width and column_is_border(left):
        left += 1
    right = 0
    while right < width - left and column_is_border(width - 1 - right):
        right += 1
    return top, bottom, left, right


def cropped(path):
    """The rows this picture should have, and what came off. `None` if nothing."""
    width, height, channels, rows = read_png(path)
    top, bottom, left, right = border(width, height, channels, rows)
    if not (top or bottom or left or right):
        return None
    if max(top, bottom, left, right) > MAX_TRIM:
        raise SystemExit(
            "%s: would trim %d pixels from one side, which is more than %d.\n"
            "  That is not a capture border. Look at the picture rather than\n"
            "  letting this shave it down." % (path, max(top, bottom, left, right), MAX_TRIM))
    new_width = width - left - right
    new_height = height - top - bottom
    if new_width < MIN_SIZE or new_height < MIN_SIZE:
        raise SystemExit(
            "%s: cropping would leave %dx%d, which is smaller than %d on a side."
            % (path, new_width, new_height, MIN_SIZE))
    kept = [row[left * channels:(width - right) * channels]
            for row in rows[top:height - bottom]]
    return channels, kept, (top, bottom, left, right), (width, height), (new_width, new_height)


def shots():
    """Every capture, in a fixed order."""
    found = []
    for folder in SOURCES:
        base = os.path.join(ROOT, folder.replace("/", os.sep))
        if not os.path.isdir(base):
            continue
        for name in sorted(os.listdir(base)):
            if name.endswith(".png"):
                found.append(os.path.join(base, name))
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if any picture still has a border to trim")
    args = parser.parse_args()

    pending = []
    for path in shots():
        result = cropped(path)
        if result is None:
            continue
        channels, rows, trim, was, now = result
        rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
        pending.append((path, channels, rows, trim, was, now, rel))

    if args.check:
        if pending:
            for _, _, _, trim, was, now, rel in pending:
                print("  %s still has a border: %dx%d would become %dx%d "
                      "(top %d, bottom %d, left %d, right %d)"
                      % (rel, was[0], was[1], now[0], now[1], *trim))
            print("\n  Run: python tools/shots/crop.py")
            return 1
        print("no screenshot has a capture border left on it")
        return 0

    if not pending:
        print("nothing to trim")
        return 0
    for path, channels, rows, trim, was, now, rel in pending:
        write_png(path, channels, rows)
        print("  %-46s %dx%d -> %dx%d" % (rel, was[0], was[1], now[0], now[1]))
    print("trimmed %d screenshot(s)" % len(pending))
    return 0


if __name__ == "__main__":
    sys.exit(main())
