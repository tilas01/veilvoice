#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Every picture the Windows packages and the Store listing need, drawn from the mark.

    python tools/package/icons.py msix  OUT   # Assets\\ for an MSIX package
    python tools/package/icons.py store OUT   # the images a Store listing asks for
    python tools/package/icons.py --self-test

Nothing here is committed output. The mark is the 32 by 32 pixel drawing in
`assets/generate.py`, and every size is made from it at build time by whole
number scaling, so the edges stay sharp and the result is the same bytes on
every machine. A size that is not a multiple of 32 gets the largest whole
multiple that fits, centred, and the rest is left transparent or filled with
the window's background colour, as the slot calls for.

# Which sizes, and why so many

An MSIX names a handful of logos in its manifest and Windows picks a file for
each from a family of variants: a scale for each display density, and for the
small one a family of exact pixel sizes for the taskbar and the Start menu
list. A package that ships only the base files works and looks blurred on
every display above 100 per cent. The list below is Microsoft's recommended
set for a desktop application; `makepri` indexes it when the package is built.

# In plain words

Makes all the little pictures of the VeilVoice logo that Windows and the
Microsoft Store want, in every size they ask for, from the one drawing the
project already has.
"""

import importlib.util
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))


def _generator():
    """`assets/generate.py`, loaded as a module: its main() is guarded."""
    spec = importlib.util.spec_from_file_location(
        "veilvoice_assets", os.path.join(ROOT, "assets", "generate.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


G = _generator()

# The plate behind a tile, when a slot asks for one. The window's own
# background, so the tile and the application read as one thing.
PLATE = G.BG


def mark_at(limit):
    """The mark at the largest whole size that fits in `limit` pixels."""
    mark = G.icon_32()
    if limit >= 32:
        return G.scale(mark, limit // 32)
    # Below 32 the only honest reduction is every other pixel, which is the
    # same 16 by 16 the Windows icon and the hicolor theme already use.
    return [row[::2] for row in mark[::2]]


def picture(width, height, fill, plate=None):
    """A canvas with the mark centred, filling `fill` of the shorter side."""
    canvas = G.blank(width, height, plate or G.NONE)
    art = mark_at(int(min(width, height) * fill))
    x = (width - len(art[0])) // 2
    y = (height - len(art)) // 2
    G.blit(canvas, art, x, y)
    return canvas


# (file name, width, height, fill, plated). Fill is how much of the shorter
# side the mark may take: a tile has a margin, a taskbar icon does not.
MSIX = []
for scale_pct in (100, 125, 150, 200, 400):
    side = 44 * scale_pct // 100
    MSIX.append((f"Square44x44Logo.scale-{scale_pct}.png", side, side, 1.0, False))
for size in (16, 24, 32, 48, 256):
    MSIX.append((f"Square44x44Logo.targetsize-{size}.png", size, size, 1.0, False))
    MSIX.append((f"Square44x44Logo.targetsize-{size}_altform-unplated.png",
                 size, size, 1.0, False))
for scale_pct in (100, 200, 400):
    MSIX.append((f"Square150x150Logo.scale-{scale_pct}.png",
                 150 * scale_pct // 100, 150 * scale_pct // 100, 0.66, True))
    MSIX.append((f"Wide310x150Logo.scale-{scale_pct}.png",
                 310 * scale_pct // 100, 150 * scale_pct // 100, 0.66, True))
    MSIX.append((f"SmallTile.scale-{scale_pct}.png",
                 71 * scale_pct // 100, 71 * scale_pct // 100, 0.66, True))
    MSIX.append((f"LargeTile.scale-{scale_pct}.png",
                 310 * scale_pct // 100, 310 * scale_pct // 100, 0.66, True))
    MSIX.append((f"StoreLogo.scale-{scale_pct}.png",
                 50 * scale_pct // 100, 50 * scale_pct // 100, 1.0, False))

# What Partner Center asks for on the listing page, beyond the package's own.
# Only the 1:1 300 by 300 logo is required; the others are optional and make
# the listing look finished in search results and on the Xbox-style shelves.
STORE = [
    ("store-logo-300x300.png", 300, 300, 0.85, True),
    ("store-box-art-1080x1080.png", 1080, 1080, 0.6, True),
    ("store-poster-720x1080.png", 720, 1080, 0.6, True),
    ("winget-icon-256x256.png", 256, 256, 1.0, False),
]


def write_set(entries, out):
    os.makedirs(out, exist_ok=True)
    written = []
    for name, width, height, fill, plated in entries:
        pixels = picture(width, height, fill, PLATE if plated else None)
        path = os.path.join(out, name)
        G.write_png(path, pixels)
        written.append(path)
    return written


def self_test():
    with tempfile.TemporaryDirectory() as scratch:
        first = write_set(MSIX + STORE, os.path.join(scratch, "a"))
        second = write_set(MSIX + STORE, os.path.join(scratch, "b"))
        for a, b in zip(first, second):
            with open(a, "rb") as x, open(b, "rb") as y:
                if x.read() != y.read():
                    print(f"FAIL {os.path.basename(a)} differs between two runs")
                    return 1
        for path, (name, width, height, _, plated) in zip(first, MSIX + STORE):
            with open(path, "rb") as handle:
                head = handle.read(33)
            got = int.from_bytes(head[16:20], "big"), int.from_bytes(head[20:24], "big")
            if got != (width, height):
                print(f"FAIL {name} is {got}, the slot is {(width, height)}")
                return 1
        # A plated tile must have no transparent pixel; an unplated icon must
        # have at least one, or the taskbar draws a square.
        tile = picture(150, 150, 0.66, PLATE)
        if any(px[3] == 0 for row in tile for px in row):
            print("FAIL a plated tile has a transparent pixel")
            return 1
        icon = picture(44, 44, 1.0)
        if not any(px[3] == 0 for row in icon for px in row):
            print("FAIL an unplated icon has no transparent pixel")
            return 1
    print(f"ok   {len(MSIX)} package images and {len(STORE)} listing images, "
          "the right sizes and the same bytes twice")
    return 0


def main(argv):
    if argv[1:] == ["--self-test"]:
        return self_test()
    if len(argv) != 3 or argv[1] not in ("msix", "store"):
        print("usage: icons.py msix|store OUT, or icons.py --self-test")
        return 2
    entries = MSIX if argv[1] == "msix" else STORE
    written = write_set(entries, argv[2])
    print(f"wrote {len(written)} images to {argv[2]}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
