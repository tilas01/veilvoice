#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
A tool a workflow installs with cargo is named at a version.

# Why `--locked` is not enough

`cargo install some-tool --locked` pins the tool's dependencies to the lockfile
it was published with. It does not pin the tool. Without `--version`, cargo
asks crates.io for the newest release on the morning the job runs and builds
that, so the weekly mutation campaign and the fuzz run were each executing
whatever had been published most recently, by whoever could publish it, on a
runner holding a checkout of this repository.

That is the same hole `tools/audit/actions.py` closes for actions, one layer
down. A release of the tool that changes its output, its exit code or its
behaviour also changes what the job means, and nothing in the repository
records that it happened. With the version in the workflow, a new release
reaches CI only as a change somebody reads.

# What counts as pinned

`--version X.Y.Z` or the `tool@X.Y.Z` spelling, with an exact version rather
than a requirement such as `^1` or `1.*`, and `--locked` alongside it. Lines
continued with a backslash are read as one command.

Pure standard library. `--self-test` checks the parser against known shapes.
"""

from __future__ import annotations

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORKFLOWS = os.path.join(ROOT, ".github", "workflows")

INSTALL = re.compile(r"\bcargo\s+(?:\+\S+\s+)?install\s")
EXACT = r"=?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?"
VERSION_FLAG = re.compile(r"\s--version(?:\s+|=)" + EXACT + r"(?=\s|$)")
AT_VERSION = re.compile(r"\s[A-Za-z0-9_-]+@" + EXACT + r"(?=\s|$)")
LOCKED = re.compile(r"\s--locked(?=\s|$)")


def commands(text):
    """(line number, command) for each shell line, with continuations joined."""
    out, start, held = [], None, []
    for number, line in enumerate(text.splitlines(), 1):
        if not held and line.lstrip().startswith("#"):
            continue
        if not held:
            start = number
        if line.rstrip().endswith("\\"):
            held.append(line.rstrip()[:-1])
            continue
        held.append(line)
        out.append((start, " ".join(held)))
        held = []
    if held:
        out.append((start, " ".join(held)))
    return out


def loose_installs(text):
    """(line number, problem) for each `cargo install` that is not pinned."""
    loose = []
    for number, command in commands(text):
        if not INSTALL.search(command):
            continue
        command = " " + command.split("#", 1)[0] + " "
        if not (VERSION_FLAG.search(command) or AT_VERSION.search(command)):
            loose.append((number, "no exact --version"))
        elif not LOCKED.search(command):
            loose.append((number, "no --locked"))
    return loose


def self_test():
    cases = [
        ("bare", "        run: cargo install cargo-mutants --locked\n",
         [(1, "no exact --version")]),
        ("pinned", "        run: cargo install cargo-mutants --version 27.1.0 --locked\n", []),
        ("equals form", "        run: cargo install foo --version=1.2.3 --locked\n", []),
        ("at form", "          cargo +nightly install cargo-fuzz@0.13.2 --locked\n", []),
        ("requirement is not a pin",
         "        run: cargo install foo --version ^1.2 --locked\n",
         [(1, "no exact --version")]),
        ("pinned but unlocked", "        run: cargo install foo --version 1.2.3\n",
         [(1, "no --locked")]),
        ("continued",
         "        run: |\n          cargo install foo \\\n            --version 1.2.3 --locked\n", []),
        ("continued and loose",
         "        run: |\n          cargo install foo \\\n            --locked\n",
         [(2, "no exact --version")]),
        ("a comment is not a command", "      # cargo install foo --locked\n", []),
        ("version in a trailing comment does not count",
         "        run: cargo install foo --locked # --version 1.2.3\n",
         [(1, "no exact --version")]),
    ]
    failed = 0
    for name, text, expected in cases:
        got = loose_installs(text)
        if got != expected:
            print("  self-test %r: expected %r, got %r" % (name, expected, got))
            failed += 1
    if failed:
        return 1
    print("  %d parser cases, all as expected" % len(cases))
    return 0


def main():
    if "--self-test" in sys.argv[1:]:
        return self_test()
    if not os.path.isdir(WORKFLOWS):
        print("  no workflows to check")
        return 0

    problems, checked = [], 0
    for name in sorted(os.listdir(WORKFLOWS)):
        if not name.endswith((".yml", ".yaml")):
            continue
        with open(os.path.join(WORKFLOWS, name), encoding="utf-8") as handle:
            text = handle.read()
        checked += sum(1 for _, command in commands(text) if INSTALL.search(command))
        for number, problem in loose_installs(text):
            problems.append("%s:%d: %s" % (name, number, problem))

    if problems:
        print("  cargo installs that build whatever was published last:")
        for problem in problems:
            print("    %s" % problem)
        print()
        print("  Name the release and keep the lockfile it shipped with:")
        print("    cargo install <tool> --version <x.y.z> --locked")
        return 1

    print("  %d cargo install(s), every one at an exact version and locked" % checked)
    return 0


if __name__ == "__main__":
    sys.exit(main())
