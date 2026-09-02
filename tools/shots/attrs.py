#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Make the width and height on each screenshot tag match the file.

    python tools/shots/attrs.py           # correct them
    python tools/shots/attrs.py --check   # fail if any disagrees

# What the attributes are for, and what happens when they are wrong

`<img width height>` tells the browser the shape of a picture before its bytes
arrive, so the page reserves the right amount of room and does not jump under
the reader as each one loads. That is the whole job, and it only works while
the numbers are true.

They were `1371x988` on every screenshot tag, hand-typed once, and the
captures are no longer that size: eight are 1400x1000 and the group tab is
taller because its panel is. Wrong numbers are worse than none, because the
browser reserves the wrong space and then jumps anyway.

So they are read out of the files. Nothing here is typed twice.

Pure standard library: the PNG header is thirty-three bytes and the width and
height are two of them.
"""

import io
import os
import re
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# Every page that shows a capture, and where it looks for one.
PAGES = ["website/index.html", "website/what.html", "website/guide.html"]

TAG = re.compile(
    r'<img([^>]*?)src="(?P<src>[^"]*assets/screenshots/gui-[^"]+\.png)"([^>]*?)>')


def size_of(path):
    with open(path, "rb") as handle:
        head = handle.read(33)
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit("%s: not a PNG" % path)
    return struct.unpack(">II", head[16:24])


def fixed(html, page):
    """The page with every screenshot's width and height matching its file."""
    problems = []

    def one(match):
        before, src, after = match.group(1), match.group("src"), match.group(3)
        # The page lives in `website/`, and so do the pictures it names.
        path = os.path.join(ROOT, "website", src.replace("/", os.sep))
        if not os.path.isfile(path):
            problems.append("%s: %s does not exist" % (page, src))
            return match.group(0)
        width, height = size_of(path)
        whole = before + after
        was = (re.search(r'width="(\d+)"', whole), re.search(r'height="(\d+)"', whole))
        if was[0] and was[1] and (int(was[0].group(1)), int(was[1].group(1))) != (width, height):
            problems.append("%s: %s says %sx%s and is %dx%d"
                            % (page, os.path.basename(src), was[0].group(1),
                               was[1].group(1), width, height))
        fix = lambda text: re.sub(  # noqa: E731
            r'width="\d+"', 'width="%d"' % width,
            re.sub(r'height="\d+"', 'height="%d"' % height, text))
        return '<img%ssrc="%s"%s>' % (fix(before), src, fix(after))

    return TAG.sub(one, html), problems


def main():
    check = "--check" in sys.argv
    problems = []
    changed = 0
    for page in PAGES:
        path = os.path.join(ROOT, page.replace("/", os.sep))
        if not os.path.isfile(path):
            continue
        with io.open(path, encoding="utf-8", newline="") as handle:
            html = handle.read()
        fresh, found = fixed(html.replace("\r\n", "\n"), page)
        problems.extend(found)
        if not check and fresh != html.replace("\r\n", "\n"):
            with io.open(path, "w", encoding="utf-8", newline="\n") as handle:
                handle.write(fresh)
            changed += 1

    if check:
        if problems:
            for line in problems:
                print("  %s" % line)
            print()
            print("Run: python tools/shots/attrs.py")
            return 1
        print("  every screenshot tag matches the file it names")
        return 0
    print("  corrected %d page(s)" % changed if changed else
          "  every screenshot tag already matches its file")
    return 0


if __name__ == "__main__":
    sys.exit(main())
