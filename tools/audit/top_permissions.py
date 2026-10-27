#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
No workflow grants a write permission to every job at once.

# Why the top level is the wrong place for a write

A `permissions:` block at the top of a workflow is the default for every job in
it, including a job added next year by somebody who never read the block. A
write granted there is granted to all of them. Granted on the job that needs
it, it goes to that job and stops there, and a second job has to ask for it in
writing where a reviewer will see the request.

The OpenSSF Scorecard reads it the same way: its Token-Permissions check marks
a workflow down for any top-level write, whatever the jobs under it do. Two of
ours had one, `rewrite.yml` and `signing.yml`, each with a single job that
needed it, so moving the grant changed nothing those jobs could do.

# What is checked

Every `*.yml` and `*.yaml` under `.github/workflows` must have a top-level
`permissions:` block, so nothing falls back to the repository's default token,
and every permission in it must be `read` or `none`. `read-all` is accepted;
`write-all` is not.

Pure standard library. `--self-test` checks the reader against known shapes.
"""

from __future__ import annotations

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORKFLOWS = os.path.join(ROOT, ".github", "workflows")

TOP = re.compile(r"^permissions:\s*(?P<inline>[^#\s][^#]*?)?\s*(#.*)?$")
ENTRY = re.compile(r"^\s+(?P<scope>[a-z-]+):\s*(?P<level>[a-z-]+)\s*(#.*)?$")


def problems(text):
    """What is wrong with one workflow's top-level permissions, in words."""
    lines = text.splitlines()
    for index, line in enumerate(lines):
        found = TOP.match(line)
        if not found:
            continue
        inline = (found.group("inline") or "").strip()
        if inline:
            if inline in ("read-all", "{}"):
                return []
            return ["the top level grants %s to every job" % inline]
        out = []
        for entry in lines[index + 1:]:
            if not entry.strip() or entry.lstrip().startswith("#"):
                continue
            if not entry.startswith(" "):
                break
            pair = ENTRY.match(entry)
            if pair and pair.group("level") not in ("read", "none"):
                out.append("the top level grants %s: %s to every job"
                           % (pair.group("scope"), pair.group("level")))
        return out
    return ["there is no top-level permissions block, so every job gets the "
            "repository's default token"]


def self_test():
    good = "on: push\npermissions:\n  contents: read\njobs:\n  a:\n    permissions:\n      contents: write\n"
    assert problems(good) == [], problems(good)
    bad = "on: push\npermissions:\n  contents: write # for the push\n  actions: read\njobs:\n"
    assert problems(bad) == ["the top level grants contents: write to every job"], problems(bad)
    assert problems("on: push\npermissions: read-all\n") == []
    assert problems("on: push\npermissions: {}\n") == []
    assert problems("on: push\npermissions: write-all\n") == [
        "the top level grants write-all to every job"]
    assert len(problems("on: push\njobs:\n  a:\n    permissions:\n      contents: read\n")) == 1
    print("top_permissions: self-test passed")


def main(argv):
    if argv[1:] == ["--self-test"]:
        self_test()
        return 0
    failures = []
    names = sorted(n for n in os.listdir(WORKFLOWS) if n.endswith((".yml", ".yaml")))
    for name in names:
        with open(os.path.join(WORKFLOWS, name), encoding="utf-8") as handle:
            for problem in problems(handle.read()):
                failures.append("%s: %s" % (name, problem))
    if failures:
        print("A write permission belongs on the job that needs it:")
        for failure in failures:
            print("  " + failure)
        return 1
    print("  %d workflow(s), none granting a write to every job" % len(names))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
