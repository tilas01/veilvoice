#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
A checkout that never pushes does not keep the token.

# Why the default is the wrong answer here

`actions/checkout` leaves the workflow's token behind in the checkout's git
configuration, so that a later `git push` works without being told how. That
is convenient for the few jobs that push and a gift to everything else: every
step after the checkout can read it, and those steps compile third-party
crates and run their build scripts, run our own Python, and upload artefacts.
A dependency with a hostile `build.rs` does not need an exploit to reach the
token. It needs `cat .git/config`.

`persist-credentials: false` takes it away. Nothing that only reads the tree
notices, because this repository is public and an unauthenticated fetch works.
The jobs that call GitHub's API pass the token explicitly as `GH_TOKEN` or
`GITHUB_TOKEN`, where it is visible in the workflow and nowhere else.

# The exceptions

A workflow that pushes through `origin` needs the token where git looks for it.
Those are named in `KEEPS_TOKEN` with the reason, and this check fails if an
entry stops being needed, so an exception cannot outlive what it was for.

Pure standard library. `--self-test` checks the parser against known shapes.
"""

from __future__ import annotations

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORKFLOWS = os.path.join(ROOT, ".github", "workflows")

# Workflows allowed to leave the token in a checkout, and why.
KEEPS_TOKEN = {
    "rewrite.yml": "pushes the signed history it builds to a handover branch through origin",
    "signing.yml": "pushes a signed test commit through origin to ask GitHub whether it verifies",
}

CHECKOUT = re.compile(r"^(?P<indent>\s*)-\s*uses:\s*actions/checkout@")
PERSIST_OFF = re.compile(r"^\s*persist-credentials:\s*false\s*(#.*)?$")


def loose_checkouts(text):
    """Line numbers of checkout steps that do not set `persist-credentials: false`."""
    lines = text.splitlines()
    loose = []
    for number, line in enumerate(lines, 1):
        found = CHECKOUT.match(line)
        if not found:
            continue
        step = len(found.group("indent"))
        safe = False
        for after in lines[number:]:
            if not after.strip() or after.lstrip().startswith("#"):
                continue
            indent = len(after) - len(after.lstrip())
            if indent <= step:
                break
            if PERSIST_OFF.match(after):
                safe = True
                break
        if not safe:
            loose.append(number)
    return loose


def self_test():
    cases = [
        ("bare", "    steps:\n      - uses: actions/checkout@abc # v7\n      - run: make\n", [2]),
        ("off", "      - uses: actions/checkout@abc\n        with:\n          persist-credentials: false\n", []),
        ("off after other inputs",
         "      - uses: actions/checkout@abc\n        with:\n          ref: main\n\n"
         "          # why\n          persist-credentials: false\n", []),
        ("true is not off",
         "      - uses: actions/checkout@abc\n        with:\n          persist-credentials: true\n", [1]),
        ("next step does not count",
         "      - uses: actions/checkout@abc\n      - uses: other@def\n        with:\n"
         "          persist-credentials: false\n", [1]),
    ]
    failed = 0
    for name, text, expected in cases:
        got = loose_checkouts(text)
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
        checked += sum(1 for line in text.splitlines() if CHECKOUT.match(line))
        loose = loose_checkouts(text)
        if name in KEEPS_TOKEN:
            if not loose:
                problems.append(
                    "%s is listed in KEEPS_TOKEN but no checkout in it keeps the "
                    "token any more: remove the entry" % name)
            continue
        for number in loose:
            problems.append("%s:%d: checkout keeps the token" % (name, number))

    for name in sorted(KEEPS_TOKEN):
        if not os.path.exists(os.path.join(WORKFLOWS, name)):
            problems.append("KEEPS_TOKEN names %s, which does not exist: remove the entry" % name)

    if problems:
        print("  checkouts that leave the workflow token behind:")
        for problem in problems:
            print("    %s" % problem)
        print()
        print("  Add `persist-credentials: false` under the checkout's `with:`. If the")
        print("  job really pushes through origin, name the workflow in KEEPS_TOKEN in")
        print("  tools/audit/checkout_tokens.py and say why.")
        return 1

    print("  %d checkout(s); only %s keep the token, each for a stated reason"
          % (checked, ", ".join(sorted(KEEPS_TOKEN))))
    return 0


if __name__ == "__main__":
    sys.exit(main())
