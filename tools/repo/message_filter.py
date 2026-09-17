#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Rewrite one commit message: roadmap "markers" become "roadmap items".

    git filter-branch -f --msg-filter 'python3 tools/repo/message_filter.py' \
        --tag-name-filter cat -- --all

# What this is for

The roadmap called its entries "markers" for a long time and now calls them
roadmap items. The documents were changed; the commit messages describing that
work still said marker, and so did the tags pointing at them.

It also drops the `Co-Authored-By:` and `Claude-Session:` trailers that survive
on the pre-rewrite history a few release tags still point at. This repository's
commits have one author, and those trailers make GitHub show a second.

# What it deliberately does not touch

"marker" is a real word and this repository uses it for real things: the
session marker written in `app::new`, the hidden-volume marker, and the markers
a picture drops. Renaming those would be a wrong answer that looks like a right
one, so the test is narrow. A number has to follow, or the phrase has to be one
of the counted forms the roadmap commits used ("Nine markers for the recording
studio"). Everything else is left exactly as it was.

# Why this is a file rather than a one-liner

Because it has to be run twice and produce the same thing both times: once in
the container where the branches were rewritten, and once on a machine that can
push tags, since a proxy that refuses to move a ref refuses to move a tag. Two
subtly different sed expressions would produce two different histories.

Pure standard library.
"""

import re, sys

COUNTS = r"(?:One|Two|Three|Four|Five|Six|Seven|Eight|Nine|Ten|Eleven|Twelve|\d+)"

def fix(text):
    # `ROADMAP markers 30 and 31` reads wrong as `ROADMAP roadmap items`.
    text = re.sub(r"\bROADMAP markers(?=\s+\d)", "ROADMAP items", text)
    text = re.sub(r"\bROADMAP marker(?=\s+\d)", "ROADMAP item", text)
    # A number follows: unambiguously a roadmap entry.
    text = re.sub(r"\bMarkers(?=\s+\d)", "Roadmap items", text)
    text = re.sub(r"\bmarkers(?=\s+\d)", "roadmap items", text)
    text = re.sub(r"\bMarker(?=\s+\d)", "Roadmap item", text)
    text = re.sub(r"\bmarker(?=\s+\d)", "roadmap item", text)
    # `Nine markers for the recording studio`: counted, so a roadmap entry too.
    text = re.sub(r"(\b%s)\s+markers\b" % COUNTS, r"\1 roadmap items", text)
    text = re.sub(r"(\b%s)\s+Markers\b" % COUNTS, r"\1 Roadmap items", text)
    # Attribution trailers that survive only on the pre-rewrite tagged history.
    out = []
    for line in text.split("\n"):
        s = line.strip()
        if s.lower().startswith(("co-authored-by:", "claude-session:")):
            continue
        if s.startswith("\U0001F916 Generated with") or s.startswith("Generated with [Claude Code]"):
            continue
        if s.startswith("https://claude.ai/code/session_"):
            continue
        out.append(line)
    text = "\n".join(out)
    # Collapse the blank lines a stripped trailer leaves at the end.
    return re.sub(r"\n{3,}$", "\n", text.rstrip("\n")) + "\n"

if __name__ == "__main__":
    sys.stdout.write(fix(sys.stdin.read()))
