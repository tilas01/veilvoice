#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
A picture of what changed, one per release.

# Why this exists

A release is described in words in two places, the notes on GitHub and the
releases page on this site, and the only image either of them carries is the
project's banner. That is the same picture for every version and says nothing
about this one. Anywhere a release is passed along as a link, that banner is
what appears.

So each release gets a card: the version, and the changes that release led
with, drawn in the site's own palette with the same hand-drawn pixel font and
the same PNG writer the banner and the mark already use. No dependency is
added and no second copy of the notes exists, because the text on the card is
read out of `CHANGELOG.md` by the functions `tools/site/releases.py` already
uses to build the page.

# Where the words come from

An entry written since v0.1.13 names each change on a line of its own in bold
and puts the detail in bullets under it. Those bold lines are the headline
changes and they are what the card carries, taken with
`releases.headlines`.

Two entries use bold for a label rather than for a change, `**In short**` and
`**In full**`, and a card reading "IN SHORT, IN FULL" describes nothing. They
are named in `SUMMARY_INSTEAD` below, with the reason, rather than guessed at
by a rule about how long a bold line is: "The terminal drawings share one
width" is 36 characters and is a real change, and any rule short enough to
catch a label catches that too.

An entry with no bold lines at all needs no declaration. There is nothing
there to misread, and the card carries the sentence the releases page already
shows beside that version, which `releases.summary` computes and the site
suite already checks.

# What this refuses to do

Draw a character it has no glyph for. `assets/generate.py`'s `text` raises
rather than advancing the cursor over a gap, which is finding F-37's shape:
artwork that is wrong about the project, invisible to every test, obvious the
moment somebody looks. A release whose headline uses a character the font
does not have fails this script rather than publishing a card with a hole in
it.

Invent a date. `CHANGELOG.md` carries none, the site shows none, and this
clone has no tags, so there is no date in the tree to draw and a date taken
from the clock would differ between a build and its check.

# Checking

`--check` redraws every card into memory and compares it against the committed
file, which is how every other generated thing here is held. A card that does
not match what `CHANGELOG.md` says fails the build, because a picture of the
wrong release is worse than no picture.
"""

from __future__ import annotations

import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

sys.path.insert(0, os.path.join(ROOT, "tools", "site"))
import releases as site  # noqa: E402

# `assets/generate.py` is a script rather than a package, so it is loaded by
# path. Importing it is already established: it holds the PNG writer, the
# palette and the font, and a second copy of any of those is a second thing to
# drift.
_spec = importlib.util.spec_from_file_location(
    "veilvoice_artwork", os.path.join(ROOT, "assets", "generate.py"))
art = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(art)

OUT_DIR = os.path.join("assets", "changelog")
WEB_DIR = os.path.join("website", "assets", "changelog")

# The same shape as the banner, which is what a link preview is cropped to.
WIDTH, HEIGHT = 1280, 640

MARGIN = 80
HEADLINE_SCALE = 3
LINE_HEIGHT = 7 * HEADLINE_SCALE + 14
BULLET = 10

# Entries whose bold lines label a section rather than name a change.
SUMMARY_INSTEAD = {
    "0.1.17": "the entry is one narrative under **In short** and **In full**",
    "0.1.16": "the same shape, with one bold sentence of measurement under it",
}


def columns(scale_factor=HEADLINE_SCALE):
    """How many characters fit across the card at this size."""
    room = WIDTH - 2 * MARGIN - BULLET * 3
    return room // ((5 + 1) * scale_factor)


def plain(text):
    """A line of Markdown as the font will draw it.

    Backticks, asterisks and brackets around a link are markup rather than
    words. They are taken out here rather than given glyphs, because a card
    setting `` `deps` `` in backticks is showing the reader punctuation that
    exists to tell a Markdown renderer something.
    """
    for mark in ("`", "*", "_", "[", "]"):
        text = text.replace(mark, "")
    return " ".join(text.split()).upper()


def wrapped(text, width):
    """`text` broken on spaces at `width` characters. A long word is cut."""
    lines, line = [], ""
    for word in text.split(" "):
        while len(word) > width:
            if line:
                lines.append(line)
                line = ""
            lines.append(word[:width])
            word = word[width:]
        if not line:
            line = word
        elif len(line) + 1 + len(word) <= width:
            line += " " + word
        else:
            lines.append(line)
            line = word
    if line:
        lines.append(line)
    return lines


def content(version, body):
    """What this release's card says, as (lines, how many were left out)."""
    new, _technical = site.split_entry(body)
    width = columns()

    if version in SUMMARY_INSTEAD:
        found = []
    else:
        found = site.headlines(new)

    if not found:
        sentence = plain(site.summary(body))
        return ([(line, False) for line in wrapped(sentence, width)], 0)

    # The room left under the version, in whole lines.
    room = (HEIGHT - 300 - MARGIN) // LINE_HEIGHT
    lines, used = [], 0
    for headline in found:
        block = wrapped(plain(headline), width)
        if len(lines) + len(block) > room - 1:
            break
        lines.extend((line, at == 0) for at, line in enumerate(block))
        used += 1
    return (lines, len(found) - used)


def draw(version, body):
    """One card, as pixels."""
    px = art.blank(WIDTH, HEIGHT, art.BG)

    grid = (0x1f, 0x21, 0x30, 255)
    for y in range(0, HEIGHT, 16):
        for x in range(WIDTH):
            px[y][x] = grid
    for x in range(0, WIDTH, 16):
        for y in range(HEIGHT):
            px[y][x] = grid

    art.blit(px, art.scale(art.icon_32(), 3), MARGIN, 64)
    art.text(px, "VEILVOICE", MARGIN + 128, 74, art.FG, 5)
    art.text(px, "WHAT CHANGED", MARGIN + 128, 128, art.COMMENT, 2)

    art.text(px, "V" + version, MARGIN, 190, art.BLUE, 9)
    art.rect(px, MARGIN, 290, WIDTH - 2 * MARGIN, 2, art.BORDER)

    lines, rest = content(version, body)
    y = 320
    for line, first in lines:
        if first:
            art.rect(px, MARGIN, y + 8, BULLET, BULLET, art.PURPLE)
        art.text(px, line, MARGIN + BULLET * 3, y, art.FG, HEADLINE_SCALE)
        y += LINE_HEIGHT

    if rest:
        art.text(px, "AND %d MORE IN THE RELEASE NOTES" % rest,
                 MARGIN + BULLET * 3, y + 6, art.COMMENT, 2)

    art.rect(px, 0, HEIGHT - 8, WIDTH, 8, art.BLUE)
    return px


def cards():
    """Every release and the card it should have, newest first."""
    text = site.read(site.SOURCE)
    out = []
    for entry in site.releases(text):
        version = entry["version"]
        # `## v0.1.5 and earlier` is a heading over several versions at once
        # and is not a release with notes of its own to draw.
        if version in site.earlier(text):
            continue
        out.append((version, entry["body"]))
    return out


def name(version):
    return "v%s.png" % version


def check():
    problems = []
    wanted = {}
    for version, body in cards():
        wanted[name(version)] = draw(version, body)

    for where in (OUT_DIR, WEB_DIR):
        full = os.path.join(ROOT, where)
        present = set()
        if os.path.isdir(full):
            present = {f for f in os.listdir(full) if f.endswith(".png")}
        for extra in sorted(present - set(wanted)):
            problems.append("%s/%s is a card for a release CHANGELOG.md does "
                            "not have a section for" % (where, extra))
        for filename, pixels in sorted(wanted.items()):
            path = os.path.join(full, filename)
            if not os.path.exists(path):
                problems.append("%s/%s has not been drawn" % (where, filename))
                continue
            with open(path, "rb") as handle:
                blob = handle.read()
            try:
                actual = art.decode_png(blob)
            except ValueError as exc:
                problems.append("%s/%s: cannot read (%s)" % (where, filename, exc))
                continue
            if actual != pixels:
                problems.append("%s/%s does not match what CHANGELOG.md says "
                                "about that release" % (where, filename))

    if problems:
        print("  the release cards are out of date:")
        for line in problems:
            print("    %s" % line)
        print()
        print("    Run tools/release/card.py to redraw them. A picture of the")
        print("    wrong release is worse than no picture at all.")
        return 1

    print("  %d release card(s) match CHANGELOG.md" % len(wanted))
    return 0


def main():
    if "--check" in sys.argv:
        return check()

    drawn = 0
    for where in (OUT_DIR, WEB_DIR):
        os.makedirs(os.path.join(ROOT, where), exist_ok=True)
    known = set()
    for version, body in cards():
        pixels = draw(version, body)
        known.add(name(version))
        for where in (OUT_DIR, WEB_DIR):
            art.write_png(os.path.join(ROOT, where, name(version)), pixels)
        drawn += 1

    # A release removed from CHANGELOG.md leaves a card behind otherwise, and
    # a picture of a release that is no longer described is exactly the kind
    # of thing that survives because nothing walks the directory.
    removed = 0
    for where in (OUT_DIR, WEB_DIR):
        full = os.path.join(ROOT, where)
        for filename in sorted(os.listdir(full)):
            if filename.endswith(".png") and filename not in known:
                os.remove(os.path.join(full, filename))
                removed += 1

    print("  %d release card(s) drawn into %s and %s%s"
          % (drawn, OUT_DIR, WEB_DIR,
             ", %d removed" % removed if removed else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
