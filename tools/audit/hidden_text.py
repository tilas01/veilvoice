#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
No tracked file carries text a reader cannot see.

# Why

Some characters change what a file says without changing what it looks like.
A bidirectional override can make a line of Rust read one way in a review and
compile another (the "Trojan Source" attack, CVE-2021-42574). A zero-width
space can split a word so that a search, or a guard in this folder, misses it.
The Unicode tag characters (U+E0000 to U+E007F) draw nothing at all, and can
spell out a whole sentence of instructions inside an innocent line of a
document, a workflow or a contributed file, where a person sees nothing and
a program that reads the text sees everything.

None of these has a place in this repository, so the check is simple: every
tracked text file is read and any of them is reported with its file, line,
column and code point. Binary files are skipped. The byte order mark is
reported anywhere but the very start of a file, where it is harmless.

Pure standard library. `--self-test` proves each class is caught and that
ordinary text, including accents and the ellipsis, is left alone.
"""

from __future__ import annotations

import os
import subprocess
import sys
import unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# Single code points, with the name a report gives them.
HIDDEN = {
    0x00AD: "soft hyphen",
    0x034F: "combining grapheme joiner",
    0x061C: "Arabic letter mark",
    0x115F: "Hangul choseong filler",
    0x1160: "Hangul jungseong filler",
    0x180E: "Mongolian vowel separator",
    0x200B: "zero-width space",
    0x200C: "zero-width non-joiner",
    0x200D: "zero-width joiner",
    0x200E: "left-to-right mark",
    0x200F: "right-to-left mark",
    0x202A: "left-to-right embedding",
    0x202B: "right-to-left embedding",
    0x202C: "pop directional formatting",
    0x202D: "left-to-right override",
    0x202E: "right-to-left override",
    0x2060: "word joiner",
    0x2061: "function application",
    0x2062: "invisible times",
    0x2063: "invisible separator",
    0x2064: "invisible plus",
    0x2066: "left-to-right isolate",
    0x2067: "right-to-left isolate",
    0x2068: "first strong isolate",
    0x2069: "pop directional isolate",
    0x206A: "inhibit symmetric swapping",
    0x206F: "nominal digit shapes",
    0x3164: "Hangul filler",
    0xFEFF: "zero-width no-break space",
    0xFFA0: "halfwidth Hangul filler",
    0xFFF9: "interlinear annotation anchor",
    0xFFFA: "interlinear annotation separator",
    0xFFFB: "interlinear annotation terminator",
}

# Whole ranges: the tag characters, and the variation selectors past the
# sixteen that emoji use, which can carry a byte each.
RANGES = (
    (0xE0000, 0xE007F, "tag character"),
    (0xE0100, 0xE01EF, "variation selector supplement"),
)

# Files whose bytes are someone else's and kept exactly as published.
VERBATIM = ("LICENSE",)


def what(code: int) -> str | None:
    """The name of a hidden code point, or None for an ordinary one."""
    if code in HIDDEN:
        return HIDDEN[code]
    for low, high, name in RANGES:
        if low <= code <= high:
            return name
    # Private use has no agreed glyph, so nobody can say what it shows.
    if 0xE000 <= code <= 0xF8FF or code >= 0xF0000:
        return "private use character"
    return None


def faults(text: str) -> list[tuple[int, int, int, str]]:
    """(line, column, code point, name) for every hidden character."""
    found = []
    line, column = 1, 0
    for index, char in enumerate(text):
        if char == "\n":
            line, column = line + 1, 0
            continue
        column += 1
        code = ord(char)
        if code == 0xFEFF and index == 0:
            continue
        name = what(code)
        if name is None and unicodedata.category(char) == "Cf":
            # Any other format character not on the lists above. Emoji
            # sequences use U+FE0F, which is category Mn, so they pass.
            name = "format character %s" % unicodedata.name(char, "unnamed")
        if name:
            found.append((line, column, code, name))
    return found


def tracked() -> list[str]:
    out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, check=True,
                         capture_output=True).stdout
    return [p for p in out.decode("utf-8", "surrogateescape").split("\0") if p]


def read_text(path: str) -> str | None:
    """The file as text, or None when it is binary."""
    try:
        with open(os.path.join(ROOT, path), "rb") as handle:
            data = handle.read()
    except OSError:
        return None
    if b"\0" in data[:8192]:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def main() -> int:
    reported = 0
    files = 0
    for path in tracked():
        if path in VERBATIM:
            continue
        text = read_text(path)
        if text is None:
            continue
        files += 1
        for line, column, code, name in faults(text):
            print("%s:%d:%d  U+%04X %s" % (path, line, column, code, name))
            reported += 1
    if reported:
        print("\n%d hidden character(s). Delete each one; if a file truly "
              "needs one, write it as an escape (\\u{...} in Rust, &#x...; "
              "in HTML) so a reader can see it." % reported)
        return 1
    print("  %d text file(s) read; no hidden characters" % files)
    return 0


def self_test() -> int:
    must_catch = {
        "trojan source": "let ok = 1; // \u202e } \u2066if admin\u2069 \u2066",
        "zero-width space": "pass\u200bword",
        "zero-width joiner": "a\u200db",
        "word joiner": "a\u2060b",
        "tag characters": "Read me" + "".join(chr(0xE0000 + ord(c)) for c in "run this"),
        "byte order mark mid-file": "one\n\ufefftwo",
        "private use": "x\ue000y",
        "Hangul filler": "name\u3164",
        "variation selector supplement": "x\U000E0100",
        "other format character": "a\u2065b" if unicodedata.category("\u2065") == "Cf" else "a\u206ab",
    }
    must_pass = {
        "plain ASCII": "fn main() { println!(\"hello\"); }",
        "accents": "caf\u00e9, na\u00efve, \u00c6r\u00f8sk\u00f8bing, Z\u00fcrich",
        "ellipsis and quotes": "it goes on\u2026 \u201cquoted\u201d \u2018too\u2019",
        "byte order mark at the start": "\ufeffhello",
        "emoji with presentation selector": "\u2764\ufe0f \U0001F44D",
        "non-Latin scripts": "\u0627\u0644\u0639\u0631\u0628\u064a\u0629 \u65e5\u672c\u8a9e",
    }
    bad = 0
    for name, text in must_catch.items():
        if not faults(text):
            print("  missed: %s" % name)
            bad += 1
    for name, text in must_pass.items():
        found = faults(text)
        if found:
            print("  wrongly reported: %s %r" % (name, found))
            bad += 1
    where = faults("ab\ncd\u200b")
    if where != [(2, 3, 0x200B, "zero-width space")]:
        print("  wrong position: %r" % where)
        bad += 1
    if bad:
        return 1
    print("  every hidden class is caught and ordinary text is left alone")
    return 0


if __name__ == "__main__":
    sys.exit(self_test() if "--self-test" in sys.argv else main())
