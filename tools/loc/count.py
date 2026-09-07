#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Functional lines of Rust: lines that hold code, per crate and in total.

    python tools/loc/count.py              # print the table
    python tools/loc/count.py --json       # the same numbers, for a generator
    python tools/loc/count.py --self-test  # check the scanner against known cases

# What is counted, and why the definition is stated wherever the number is

A **functional line** is a line with code on it. Blank lines are not counted,
and neither are lines that hold only a comment. A line with code *and* a
trailing comment counts once, because there is code on it.

That definition is written next to every number this produces. A line count
without one is not a measurement, it is a number: "80,000 lines" means something
different depending on whether the counter agreed with you about comments, and
this project is written with a very high comment-to-code ratio, so the two
answers are far apart here in particular.

# This is a new number, not a redefinition of an existing one

The generated per-file pages, the artwork and the reference links already carry
a line count, and it is the plain length of the file. That measure is not
touched. Changing what an existing number means, everywhere it appears, in order
to match a new definition would silently alter every page and link carrying one,
and somebody comparing two of them would have no way to tell which definition
they were reading.

So there are two numbers with two definitions, each stated where it is used,
rather than one number that quietly changed meaning.

# Why a string-aware scan rather than a regular expression

`//` inside a string literal is not a comment:

```rust
let url = "https://example.invalid/";   // this half is
```

A counter that treats the first `//` as the start of a comment reads that line
as code (correct here, by luck) and reads a line holding *only* a string
containing `//` as a comment (wrong). Raw strings make it worse: `r"...//..."`
and `r#"..."#` have their own rules about what ends them.

So this walks each line character by character, tracking whether it is inside a
string, a raw string, or a block comment. It is not a Rust parser and does not
need to be: it needs to know whether any character on a line is code, which is a
much smaller question than what that code means.

# In plain words

Counts how much actual code this project is, ignoring blank lines and comments,
and says what it counted so the number means something.

Pure standard library.
"""

import argparse
import io
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# The definition, in one place, so the tool and every page it feeds say the
# same thing in the same words.
DEFINITION = (
    "a line holding code: blank lines and lines holding only a comment are "
    "not counted, and a line with code and a trailing comment counts once"
)


def functional_lines(source):
    """How many lines of `source` hold code.

    Walks the text tracking string, raw-string and block-comment state, because
    `//` inside a string is not a comment and a naive scan gets that wrong in
    both directions.
    """
    count = 0
    in_block = 0  # Rust block comments nest, so this is a depth rather than a flag.
    for line in source.splitlines():
        has_code = False
        i = 0
        n = len(line)
        in_str = False
        in_raw = False
        raw_hashes = 0
        while i < n:
            ch = line[i]

            if in_block:
                if line.startswith("*/", i):
                    in_block -= 1
                    i += 2
                    continue
                if line.startswith("/*", i):
                    in_block += 1
                    i += 2
                    continue
                i += 1
                continue

            if in_raw:
                if ch == '"':
                    # A raw string ends at a quote followed by its own number of
                    # hashes, so `r#"a"b"#` is one string rather than two.
                    if line.startswith("#" * raw_hashes, i + 1):
                        in_raw = False
                        i += 1 + raw_hashes
                        continue
                i += 1
                continue

            if in_str:
                if ch == "\\":
                    i += 2  # An escape, so the next character cannot close it.
                    continue
                if ch == '"':
                    in_str = False
                i += 1
                continue

            # Not inside anything: this is where comments and strings start.
            if line.startswith("//", i):
                break  # The rest of the line is a comment.
            if line.startswith("/*", i):
                in_block += 1
                i += 2
                continue
            if ch == "r" and i + 1 < n and line[i + 1] in '#"':
                hashes = 0
                j = i + 1
                while j < n and line[j] == "#":
                    hashes += 1
                    j += 1
                if j < n and line[j] == '"':
                    in_raw = True
                    raw_hashes = hashes
                    has_code = True
                    i = j + 1
                    continue
            if ch == '"':
                in_str = True
                has_code = True
                i += 1
                continue
            if not ch.isspace():
                has_code = True
            i += 1

        if has_code:
            count += 1
    return count


def read(path):
    with io.open(path, encoding="utf-8") as handle:
        return handle.read()


def count_tree(directory):
    """Functional lines in every `.rs` file under `directory`."""
    total = 0
    for base, _dirs, files in os.walk(directory):
        for file in sorted(files):
            if file.endswith(".rs"):
                total += functional_lines(read(os.path.join(base, file)))
    return total


def per_crate():
    """Every crate, with its functional line count, ordered by name.

    `fuzz` is included and is not under `crates/`: it is a separate Cargo
    project at the root, holding the harnesses, and it is Rust somebody in this
    repository wrote and maintains. `tools/docs/generate.py` documents it beside
    the workspace members for the same reason, so leaving it out here would give
    two different answers to "which crates are there" in two files that both
    claim to enumerate them.
    """
    crates = {}
    root = os.path.join(ROOT, "crates")
    for name in sorted(os.listdir(root)):
        src = os.path.join(root, name, "src")
        if os.path.isdir(src):
            crates[name] = count_tree(src)

    fuzz = os.path.join(ROOT, "fuzz")
    if os.path.isfile(os.path.join(fuzz, "Cargo.toml")):
        # Not `os.walk(fuzz)`: `corpus/`, `seeds/` and `artifacts/` hold inputs
        # rather than source, and a `.rs` file that arrived there as a fuzzing
        # input is not code this project wrote.
        total = 0
        for part in ("src", "fuzz_targets"):
            path = os.path.join(fuzz, part)
            if os.path.isdir(path):
                total += count_tree(path)
        if total:
            crates["fuzz"] = total

    return dict(sorted(crates.items()))


# The cases the scanner has to get right, and what each one is for. Written as
# data rather than as a list of asserts so the reason each case exists is beside
# it: a counter is a small thing that is easy to get subtly wrong, and a wrong
# number here would appear in the README and in 27 crate documents at once.
CASES = [
    ("let x = 1;", 1, "plain code"),
    ("", 0, "a blank line"),
    ("   ", 0, "a line of spaces"),
    ("// a comment", 0, "a whole-line comment"),
    ("    // an indented comment", 0, "a comment with code's indentation"),
    ("/// a doc comment", 0, "a doc comment is still a comment"),
    ("//! a module doc comment", 0, "so is a module one"),
    ("let x = 1; // why", 1, "code with a trailing comment counts once"),
    ("/* block */", 0, "a block comment on one line"),
    ("/* start\nmiddle\nend */", 0, "a block comment over three lines"),
    ("/* start\nend */ let x = 1;", 1, "code after a block comment ends"),
    ("let a = 1; /* mid */ let b = 2;", 1, "a block comment inside a line"),
    ("/* outer /* inner */ still outer */", 0, "Rust block comments nest"),
    ("/* outer /* inner */ still */ let x = 1;", 1, "and the nesting closes"),
    ('let u = "https://example.invalid/";', 1, "// inside a string is not a comment"),
    ('let s = "//";', 1, "a line whose only content is a string of slashes"),
    ('let s = "a\\"b"; // real', 1, "an escaped quote does not end the string"),
    ('let r = r"a//b";', 1, "a raw string with slashes in it"),
    ('let r = r#"a"b"#;', 1, "a raw string that contains a quote"),
    ('let r = r#"a"#; // real', 1, "a hashed raw string ends at the right place"),
    ("let s = \"start\n", 1, "an unterminated string still holds code"),
    ("}", 1, "a closing brace is code"),
]


def self_test():
    """Check the scanner against every case above. Returns the exit status."""
    wrong = []
    for source, expected, why in CASES:
        got = functional_lines(source)
        if got != expected:
            wrong.append("  %-46r expected %d, counted %d  (%s)"
                         % (source, expected, got, why))
    if wrong:
        print("the functional line scanner is wrong on %d case(s):" % len(wrong))
        print("\n".join(wrong))
        return 1
    print("  the scanner is right on all %d cases" % len(CASES))

    # And on the tree, because a scanner that passes its cases and reads no
    # files is a scanner nobody has run. Every crate must have some code in it,
    # and no crate can have more functional lines than it has lines.
    crates = per_crate()
    if not crates:
        print("no crates were read")
        return 1
    for name, count in crates.items():
        if count <= 0:
            print("%s counted %d functional lines, which cannot be right"
                  % (name, count))
            return 1
    print("  and reads %d crates, the smallest of which has %d lines"
          % (len(crates), min(crates.values())))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    crates = per_crate()
    total = sum(crates.values())
    if args.json:
        print(json.dumps({"definition": DEFINITION, "crates": crates,
                          "total": total}, indent=2, sort_keys=True))
        return 0

    width = max(len(n) for n in crates)
    for name, count in crates.items():
        print("  %-*s  %6d" % (width, name, count))
    print("  %-*s  %6d" % (width, "TOTAL", total))
    print()
    print("  Functional lines: %s." % DEFINITION)
    return 0


if __name__ == "__main__":
    sys.exit(main())
