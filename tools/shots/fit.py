#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Make every window capture the same height: the tallest one's content.

    python tools/shots/fit.py           # fit, in place
    python tools/shots/fit.py --check   # verify every picture is that height

# One height for all of them, and which fault that chooses

A window capture is wrong if it cuts a sentence in half at the bottom edge, and
it is wrong if two thirds of it are empty background. The tabs make both happen
at once: the monitor tab draws a couple of hundred pixels of content and the
group tab draws well over a thousand.

This used to trim each picture to its own content, with a floor. Nothing was
cut off and nothing was padding, and it produced eight pictures 1000 tall, one
1095 and one 1315.

**That is wrong for where they are actually shown.** The README and the website
put them in a grid, and a grid with three heights in it steps: the row holding
the group tab sits lower than the rows either side, and the eye reads the
inconsistency as the pictures being wrong rather than the panels being
different lengths.

So one height, and it is **the tallest content among them**. That is the only
shared height that crops nothing:

* trimming everything to the shortest would cut the bottom off the group panel,
  which is publishing the first fault on purpose;
* scaling them to match would make the text in one picture a different size
  from the text in the next, which is worse than either fault;
* padding the short ones with their own background is the remaining option, and
  the cost is real and is exactly this: the monitor tab has empty space below
  its content now.

That cost is paid in the one place it is cheap. Empty background at the bottom
of a picture in a grid reads as the window having room; a stepped grid reads as
a mistake.

# Measured, not written down

No table of per-tab heights, and no constant naming the answer. The tallest
content is measured across the whole set on every run, so a panel that grows a
paragraph moves every picture together rather than becoming the one exception
again. `--check` proves the committed pictures are what this would produce.

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

# No picture is shorter than this, even if every tab's content fits in less. It
# is the height the capture scripts ask the window to open at, so it is the size
# of the thing being photographed, and a picture of a window has no business
# being shorter than the window.
#
# It is a floor rather than the answer: the answer is the tallest content in the
# set, and that is nearly always well above this.
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


def wanted_height(path):
    """How tall this picture needs to be to hold its own content."""
    width, height, channels, rows = read_png(path)
    bottom = content_bottom(width, height, channels, rows)
    return max(FLOOR, bottom + 1 + PADDING)


def shared_height(paths):
    """The one height every picture is given: the tallest content among them.

    Measured across the set rather than written down, so a panel that grows
    moves all of them together instead of becoming the one that sticks out.
    """
    return max((wanted_height(path) for path in paths), default=FLOOR)


def fitted(path, target):
    """`(channels, rows, was, now)` if this picture is not `target` tall.

    Taller is trimmed. Shorter is **padded with its own background**, which is
    the cost this tool now pays on purpose: see the note at the top for why a
    stepped grid is the worse of the two faults.
    """
    width, height, channels, rows = read_png(path)
    if height == target:
        return None
    if height > target:
        return channels, rows[:target], (width, height), (width, target)

    # Padded with the row the background was read from, so the added space is
    # the same colour as the space above it, including on a light palette.
    # Copied rather than synthesised: a row of the picture's own pixels cannot
    # be the wrong colour, and `round.py` runs afterwards and only touches
    # alpha in the corners.
    # `bytearray`, matching what `read_png` hands back and what `write_png`
    # will accept: a plain list of ints looks equivalent and fails on the way
    # out, in a function two files away.
    filler = bytes(rows[height - 2])
    grown = list(rows) + [bytearray(filler) for _ in range(target - height)]
    return channels, grown, (width, height), (width, target)


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

    paths = captures()
    if not paths:
        print("  no window captures to fit")
        return 0
    target = shared_height(paths)

    pending = []
    for path in paths:
        result = fitted(path, target)
        if result is None:
            continue
        channels, rows, was, now = result
        rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
        pending.append((path, channels, rows, was, now, rel))

    if args.check:
        if pending:
            for _, _, _, was, now, rel in pending:
                print("  %s is %dx%d and every capture should be %dx%d"
                      % (rel, was[0], was[1], *now))
            print("\n  Run: python tools/shots/fit.py")
            return 1
        print("  all %d captures are %d tall, which is the tallest one's content"
              % (len(paths), target))
        return 0

    if not pending:
        print("  all %d captures are already %d tall" % (len(paths), target))
        return 0
    for path, channels, rows, was, now, rel in pending:
        write_png(path, channels, rows)
        print("  %-46s %dx%d -> %dx%d" % (rel, was[0], was[1], *now))
    print("  fitted %d capture(s) to %d tall" % (len(pending), target))
    return 0


if __name__ == "__main__":
    sys.exit(main())
