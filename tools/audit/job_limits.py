#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Every job in every workflow says how long it is allowed to take.

# Why this is worth a check

A job with no `timeout-minutes` gets GitHub's default, which is **six hours**.
Nothing in this repository takes six hours. So the only job that will ever
reach that limit is one that has stopped making progress, and six hours is the
worst way to find out: the run sits there green-pending, the queue fills up
behind it, and when it finally dies it reads as a fault of the runner rather
than of the code. An infrastructure fault gets retried. A failure gets read.

This is not hypothetical here. `website/js/markdown.js` had four documents it
could not finish rendering, one of them `<details><summary>x</summary>` on a
single line, which is a shape a person writing a collapsed section reaches for.
`tools/site/wiki_site.py` renders every wiki page through that same renderer in
CI. Whichever of the two arrived first, the job would have hung, and no
workflow in this repository set a limit of any kind. Both are fixed, and a
limit is for the one nobody has found yet.

# What a good limit is

Generously above what the job takes and far below the default. It is not a
performance budget: a limit tight enough to fail on a slow runner teaches
people to raise it without reading why it fired, and then it is decoration. A
job that normally takes four minutes is fine at thirty.

# What is checked

Every `*.yml` and `*.yaml` under `.github/workflows`, every job in it, must
carry a `timeout-minutes` of its own. A limit above `CEILING` is reported too,
because a limit that is nearly the default is the default wearing a hat.

A reusable workflow called with `uses:` at the job level runs under the called
workflow's own limits and cannot be given one here, so such a job is skipped
and counted separately.

Pure standard library, like the rest of these: a check that needs a package
installed is a check that gets skipped.
"""

from __future__ import annotations

import io
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORKFLOWS = os.path.join(ROOT, ".github", "workflows")

# No job here should want more than this. Four hours is already well past
# anything the project does, and the point is to be clearly below the six-hour
# default rather than to be exact.
CEILING = 240

# A job's name sits two spaces in, under `jobs:`; its own keys sit four.
JOBS = re.compile(r"^jobs:\s*$")
JOB = re.compile(r"^  (?P<name>[A-Za-z0-9_-]+):\s*(?:#.*)?$")
KEY = re.compile(r"^    (?P<key>[A-Za-z0-9_-]+):\s*(?P<value>.*?)\s*(?:#.*)?$")
TOP = re.compile(r"^[A-Za-z0-9_-]+:")


def jobs_in(text):
    """Every job of one workflow, as (name, line number, its own top keys)."""
    found = []
    inside = False
    name = None
    number = 0
    keys = {}
    for index, line in enumerate(text.split("\n"), 1):
        if JOBS.match(line):
            inside = True
            continue
        if not inside:
            continue
        # Any other key at the left margin ends the jobs block. `on:` and
        # `permissions:` can legally follow it.
        if TOP.match(line):
            inside = False
            if name is not None:
                found.append((name, number, keys))
                name = None
            continue
        head = JOB.match(line)
        if head:
            if name is not None:
                found.append((name, number, keys))
            name, number, keys = head.group("name"), index, {}
            continue
        if name is None:
            continue
        key = KEY.match(line)
        if key:
            keys[key.group("key")] = key.group("value")
    if name is not None:
        found.append((name, number, keys))
    return found


def problems_in(text, rel):
    """Every job of `text` without a sane limit of its own."""
    found = []
    counted = 0
    delegated = 0
    for name, number, keys in jobs_in(text):
        if "uses" in keys:
            # A reusable workflow brings its own jobs and their limits.
            delegated += 1
            continue
        limit = keys.get("timeout-minutes")
        if limit is None:
            found.append(
                "%s:%d  job '%s' has no timeout-minutes, so it gets the "
                "six-hour default" % (rel, number, name))
            continue
        try:
            minutes = int(limit)
        except ValueError:
            # An expression is allowed, and its value is not knowable here.
            counted += 1
            continue
        if minutes <= 0:
            found.append("%s:%d  job '%s' has timeout-minutes: %s"
                         % (rel, number, name, limit))
        elif minutes > CEILING:
            found.append(
                "%s:%d  job '%s' allows %d minutes, which is past the %d this "
                "project has any use for" % (rel, number, name, minutes, CEILING))
        else:
            counted += 1
    return found, counted, delegated


def audit(root):
    directory = os.path.join(root, ".github", "workflows")
    if not os.path.isdir(directory):
        return [], 0, 0
    found, counted, delegated = [], 0, 0
    for name in sorted(os.listdir(directory)):
        if not name.endswith((".yml", ".yaml")):
            continue
        with io.open(os.path.join(directory, name), encoding="utf-8") as handle:
            text = handle.read()
        here, ok, passed_on = problems_in(text, ".github/workflows/%s" % name)
        found.extend(here)
        counted += ok
        delegated += passed_on
    return found, counted, delegated


def self_test():
    """The shapes this is for, and the shapes it must leave alone."""
    cases = [
        ("a job with no limit at all",
         "jobs:\n"
         "  test:\n"
         "    runs-on: ubuntu-latest\n"
         "    steps:\n"
         "      - run: true\n",
         True),
        ("the same job with one",
         "jobs:\n"
         "  test:\n"
         "    runs-on: ubuntu-latest\n"
         "    timeout-minutes: 30\n"
         "    steps:\n"
         "      - run: true\n",
         False),
        ("a limit that is nearly the default",
         "jobs:\n"
         "  test:\n"
         "    runs-on: ubuntu-latest\n"
         "    timeout-minutes: 350\n",
         True),
        ("a limit of zero",
         "jobs:\n"
         "  test:\n"
         "    runs-on: ubuntu-latest\n"
         "    timeout-minutes: 0\n",
         True),
        ("one job of three without a limit",
         "jobs:\n"
         "  a:\n"
         "    runs-on: ubuntu-latest\n"
         "    timeout-minutes: 10\n"
         "  b:\n"
         "    runs-on: ubuntu-latest\n"
         "  c:\n"
         "    runs-on: ubuntu-latest\n"
         "    timeout-minutes: 10\n",
         True),
        ("a job that calls a reusable workflow",
         "jobs:\n"
         "  call:\n"
         "    uses: ./.github/workflows/other.yml\n",
         False),
        ("a step-level timeout, which is not the job's",
         "jobs:\n"
         "  test:\n"
         "    runs-on: ubuntu-latest\n"
         "    steps:\n"
         "      - run: true\n"
         "        timeout-minutes: 5\n",
         True),
        ("keys at the left margin after the jobs block",
         "jobs:\n"
         "  test:\n"
         "    runs-on: ubuntu-latest\n"
         "    timeout-minutes: 15\n"
         "permissions:\n"
         "  contents: read\n",
         False),
        ("a limit written as an expression",
         "jobs:\n"
         "  test:\n"
         "    runs-on: ubuntu-latest\n"
         "    timeout-minutes: ${{ fromJSON(inputs.budget) }}\n",
         False),
    ]
    bad = []
    for what, text, should_fail in cases:
        found, _, _ = problems_in(text, "case.yml")
        if bool(found) != should_fail:
            bad.append("  %s was %s"
                       % (what, "not caught" if should_fail else "wrongly caught"))
    if bad:
        print("  this guard does not catch what it claims to:")
        print("\n".join(bad))
        return 1
    print("  every job without a sane limit is caught, and the three shapes "
          "that are not faults are left alone")
    return 0


def main():
    if "--self-test" in sys.argv:
        return self_test()

    found, counted, delegated = audit(ROOT)
    if found:
        print("  these jobs can run until GitHub's six-hour default kills them:")
        for line in found:
            print("    %s" % line)
        print()
        print("    Give each one a `timeout-minutes:` beside its `runs-on:`,")
        print("    generously above what the job takes and well under %d."
              % CEILING)
        print("    A job that hangs should fail in minutes and be read, not")
        print("    sit for six hours and be retried.")
        return 1
    print("  %d job(s) with a wall-clock limit%s"
          % (counted,
             ", and %d that run a reusable workflow" % delegated
             if delegated else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
