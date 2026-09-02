#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Trim the empty background below a window capture's content.

    python tools/shots/fit.py           # trim, in place
    python tools/shots/fit.py --check   # verify there is nothing left to trim

# The two ways a screenshot goes wrong, and why one size cannot fix both

A window capture is wrong if it cuts a sentence in half at the bottom edge,
and it is wrong if two thirds of it are empty background. The nine tabs of
this application make both happen at once, because their panels are nowhere
near the same length. Measured, at 1400 wide:

    monitor   174      lock      778      install   939
    settings  430      about     840      group    1288
    live      473      verify    896      file      729

A window 1000 tall cuts the group panel in half. A window 1320 tall shows the
monitor tab as a strip of content above nine hundred pixels of nothing. There
is no single window size that is right for all nine, and picking one means
choosing which of the two faults to publish.

So the capture is taken generously tall, and each picture is then trimmed to
what it actually contains. Nothing is cut off, and nothing is padding.

# The floor, and why eight of the nine still come out identical

Trimming to content alone would make the monitor tab 198 pixels tall: a wide
thin strip that, in a three-column gallery, is a sliver showing nothing. So
nothing is trimmed below `FLOOR`, which is the height the capture scripts ask
the window to open at. That is the size the application actually is, and a
picture of a window has no business being shorter than the window.

The result is that eight of the nine come out at exactly the same size, and
the ninth is taller because its panel is genuinely longer. That is as close to
one size as the application allows, and the exception is a fact about the
program rather than an accident of the tooling.

# What is read, and what is ignored

The background colour is taken from the bottom-left corner, inside the
padding, where no tab draws. Only the red, green and blue are compared:
`round.py` runs after this and writes transparency into the four corners
without touching their colour, so a rounded picture still reports its own
background correctly and this stays idempotent across runs.

The rightmost columns are ignored. The scroll bar runs the full height of the
panel, so a sweep that included it would report every tab as full to the
bottom and would measure nothing at all.

Pure standard library, and a crop rather than a filter: rows are removed, and
no pixel that survives is altered.
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from crop import read_png, write_png, shots as all_shots  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# No picture is trimmed below this. It is the height the capture scripts ask
# the window to open at, so it is the size of the thing being photographed.
FLOOR = 1000

# Kept below the content so the picture does not end flush against the last
# line of text.
PADDING = 24

# The scroll bar lives in the last of these columns and runs the whole height
# of the panel.
IGNORE_RIGHT = 40

# A capture is 8-bit and the background is flat, so anything the application
# drew differs by far more than this. Loose enough to survive the one-level
# differences software rendering produces.
TOLERANCE = 8


def content_bottom(width, height, channels, rows):
    """The last row that has anything on it, and the background it stands on."""
    # Bottom-left, inside the padding: no tab draws there, and `round.py` only
    # changes alpha, so the colour is the background whether or not the
    # corners have been rounded already.
    edge = rows[height - 2]
    background = (edge[0], edge[1], edge[2])

    last = 0
    limit = max(1, width - IGNORE_RIGHT)
    for y in range(height):
        row = rows[y]
        for x in range(limit):
            at = x * channels
            if (abs(row[at] - background[0]) > TOLERANCE
                    or abs(row[at + 1] - background[1]) > TOLERANCE
                    or abs(row[at + 2] - background[2]) > TOLERANCE):
                last = y
                break
    return last


def fitted(path):
    """`(channels, rows, was, now)` if this picture has empty space to lose."""
    width, height, channels, rows = read_png(path)
    bottom = content_bottom(width, height, channels, rows)
    wanted = max(FLOOR, bottom + 1 + PADDING)
    if wanted >= height:
        return None
    return channels, rows[:wanted], (width, height), (width, wanted)


def captures():
    """The window captures, and only those.

    The same restriction `round.py` makes, for the same reason: "the empty
    part of this is background below the content" is true of a picture of a
    window and is not true of a diagram, which would be quietly cut instead.
    """
    return [path for path in all_shots()
            if os.path.basename(path).startswith("gui-")]


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if any capture still has empty space below "
                             "its content")
    args = parser.parse_args()

    pending = []
    for path in captures():
        result = fitted(path)
        if result is None:
            continue
        channels, rows, was, now = result
        rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
        pending.append((path, channels, rows, was, now, rel))

    if args.check:
        if pending:
            for _, _, _, was, now, rel in pending:
                print("  %s has empty background below its content: "
                      "%dx%d would become %dx%d" % (rel, was[0], was[1], *now))
            print("\n  Run: python tools/shots/fit.py")
            return 1
        print("  no capture has empty space left below its content")
        return 0

    if not pending:
        print("  nothing to fit")
        return 0
    for path, channels, rows, was, now, rel in pending:
        write_png(path, channels, rows)
        print("  %-46s %dx%d -> %dx%d" % (rel, was[0], was[1], *now))
    print("  fitted %d capture(s)" % len(pending))
    return 0


if __name__ == "__main__":
    sys.exit(main())
