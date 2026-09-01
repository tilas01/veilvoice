#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Round the corners of every window capture, in the file rather than in CSS.

    python tools/shots/round.py            # round, in place
    python tools/shots/round.py --check    # verify every corner is rounded

# Why the corners are rounded in the picture

The application draws a rounded window and the capture is a rectangle, so
every committed screenshot had four square corners with a wedge of window
chrome in each. On a page that gives them a boxed-in look the running program
does not have.

CSS could round them on the website with one `border-radius`, and that is
where it would belong if the website were the only reader. It is not: the
README is rendered by GitHub, which strips styles from images, and the release
archives carry the same files. A picture that is only round in one of the three
places it appears is not rounded, so the alpha channel carries it and every
reader gets the same thing.

# What it does to the pixels

Only the corners, and only their alpha. A pixel wholly outside the radius
becomes transparent, a pixel wholly inside is untouched, and one the arc
crosses is given partial alpha from how much of it the arc covers, sampled on a
4x4 grid. Nothing is recoloured, nothing is moved, and nothing is resampled:
the picture is the same pixels the application drew, minus some corner.

That matters more than it sounds. `tools/shots/crop.py` says a screenshot has
to be the pixels the application drew for it to be worth committing, and this
is the same argument. Antialiasing the arc changes coverage, not colour.

# Transparent, not filled with the page colour

Filling the corners with the website's background would be a picture that only
looks right on one theme, and the website has nine. Transparency is the only
answer that is correct on all of them, on GitHub's light and dark renderings,
and on whatever a reader has set.

# It has to be safe to run twice

Every generator here is checked by running it again and comparing, so this has
to reach a fixed point. It does: rounding an already-rounded corner computes
the same alpha from the same geometry and writes the same bytes. `--check`
re-derives the corners and fails if any pixel differs, which also catches a
capture that was replaced without being rounded.

Pure standard library. `zlib` and `struct`, the same as everything else that
touches an image here.
"""

from __future__ import annotations

import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)

from crop import read_png, write_png, shots as all_shots  # noqa: E402

# The radius, in pixels, at the size these are captured.
#
# 14 is what the application asks its own window for. Matching it means the
# picture has the shape the program has, rather than a rounding somebody liked
# the look of.
RADIUS = 14

# How finely the arc is sampled inside one pixel. 4x4 is sixteen samples and
# seventeen possible alphas, which is past the point anyone can see a step on a
# 14-pixel arc, and it keeps the arithmetic in integers.
SAMPLES = 4


def coverage(px, py, radius):
    """How much of pixel (px, py) lies inside the rounded corner, 0.0 to 1.0.

    Coordinates are relative to the corner, with the arc centred at
    (radius, radius). Sampled rather than integrated: the exact area of a
    circle clipped to a square is a closed form nobody should have to read in a
    screenshot tool, and sixteen samples is indistinguishable at this size.
    """
    inside = 0
    for sy in range(SAMPLES):
        for sx in range(SAMPLES):
            x = px + (sx + 0.5) / SAMPLES
            y = py + (sy + 0.5) / SAMPLES
            dx = radius - x
            dy = radius - y
            if dx <= 0 or dy <= 0:
                # Past the centre on either axis: this pixel is in the straight
                # part of the edge, which the arc does not cut.
                inside += 1
                continue
            if dx * dx + dy * dy <= radius * radius:
                inside += 1
    return inside / float(SAMPLES * SAMPLES)


def with_alpha(channels, rows):
    """The same picture as RGBA rows, whatever it arrived as."""
    if channels == 4:
        return [bytearray(row) for row in rows]
    if channels == 3:
        out = []
        for row in rows:
            line = bytearray()
            for x in range(0, len(row), 3):
                line += row[x:x + 3] + b"\xff"
            out.append(line)
        return out
    raise SystemExit(
        "only RGB and RGBA captures are rounded; this one has %d channels"
        % channels)


def rounded(path):
    """The RGBA rows this picture should have once its corners are rounded."""
    width, height, channels, rows = read_png(path)
    if width < RADIUS * 2 or height < RADIUS * 2:
        raise SystemExit(
            "%s is %dx%d, which is too small for a %d-pixel radius"
            % (path, width, height, RADIUS))

    out = with_alpha(channels, rows)
    for cy in range(RADIUS):
        for cx in range(RADIUS):
            alpha = coverage(cx, cy, RADIUS)
            if alpha >= 1.0:
                continue
            value = int(alpha * 255 + 0.5)
            # The same corner, reflected into all four. `min` guards the case
            # where a picture is barely wider than two radii and the corners
            # would otherwise overlap and fight.
            for x, y in (
                (cx, cy),
                (width - 1 - cx, cy),
                (cx, height - 1 - cy),
                (width - 1 - cx, height - 1 - cy),
            ):
                at = x * 4 + 3
                out[y][at] = min(out[y][at], value)
    return [bytes(row) for row in out]


def captures():
    """The window captures, and only those.

    `crop.py` trims every PNG in the screenshot folders, which is right: a
    capture border is a capture border whatever the picture is of. Rounding is
    not like that. It says "this is a picture of a rounded window", which is
    true of the `gui-*.png` captures and is not true of anything else that
    might be put beside them. A diagram or a photograph rounded on the
    assumption it was a window would be quietly wrong, and nothing would say
    so, so the assumption is written down here instead of inherited.
    """
    return [
        path for path in all_shots()
        if os.path.basename(path).startswith("gui-")
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if any capture still has square corners")
    args = parser.parse_args()

    pending = []
    for path in captures():
        want = rounded(path)
        _, _, channels, have = read_png(path)
        if channels != 4 or have != want:
            pending.append((path, want))

    if args.check:
        if pending:
            for path, _ in pending:
                print("  %s does not have its corners rounded"
                      % os.path.relpath(path, ROOT).replace(os.sep, "/"))
            print("\n  Run: python tools/shots/round.py")
            return 1
        print("every window capture has rounded corners")
        return 0

    if not pending:
        print("nothing to round")
        return 0
    for path, want in pending:
        write_png(path, 4, want)
        print("  rounded %s" % os.path.relpath(path, ROOT).replace(os.sep, "/"))
    print("\n  rounded %d capture(s)" % len(pending))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
