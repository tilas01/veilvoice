#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Every GitHub Action a workflow runs is pinned to a commit, not to a tag.

# Why a tag is not a pin

`uses: some/action@v2` does not name a version. It names a *label*, and whoever
owns that repository can move it to any commit they like, at any time, without
telling anybody. Two of the labels this project was using were not even tags:
`rustsec/audit-check@v2` is a branch, which moves by design.

Whatever that label points at on the morning a workflow runs is downloaded and
executed on a runner that has a checkout of this repository and a token. So an
unpinned action is a standing invitation: compromise the action's repository,
or simply change your mind about what `v2` means, and you are running code
inside this project's builds. That is the shape of most of the supply-chain
attacks on CI that have actually happened.

A 40-character commit SHA cannot be moved. It is the only form of `uses:` that
says what will run.

# Why the version stays in a comment

`uses: actions/checkout@3d3c42e5... # v7` keeps the human-readable version where
a reader can see it, and Dependabot reads that comment: it updates the SHA and
the comment together, so pinning does not mean going stale. This check is here
because the pin is easy to lose, one paste at a time, and a rule nothing
enforces is a rule that decays.

Pure standard library.
"""

from __future__ import annotations

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORKFLOWS = os.path.join(ROOT, ".github", "workflows")

# `uses:` lines that name something to download. A local action (`./.github/...`)
# and a container (`docker://`) are not tag references and are left alone.
USES = re.compile(r"^\s*-?\s*uses:\s*(?P<ref>[^\s#]+)")
PINNED = re.compile(r"^[^@]+@[0-9a-f]{40}$")


def main():
    if not os.path.isdir(WORKFLOWS):
        print("  no workflows to check")
        return 0

    loose, pinned = [], 0
    for name in sorted(os.listdir(WORKFLOWS)):
        if not name.endswith((".yml", ".yaml")):
            continue
        path = os.path.join(WORKFLOWS, name)
        with open(path, encoding="utf-8") as handle:
            for number, line in enumerate(handle, 1):
                found = USES.match(line)
                if not found:
                    continue
                ref = found.group("ref")
                if ref.startswith(("./", "docker://")):
                    continue
                if PINNED.match(ref):
                    pinned += 1
                else:
                    loose.append((name, number, ref))

    if loose:
        print("  these actions are named by a label somebody else can move:")
        for name, number, ref in loose:
            print("    %s:%d  %s" % (name, number, ref))
        print()
        print("  Pin each to the commit the label points at now, keeping the")
        print("  version in a trailing comment so Dependabot can update both:")
        print("    git ls-remote https://github.com/<owner>/<repo> 'refs/tags/<tag>^{}'")
        print("    uses: <owner>/<repo>@<40-character sha> # <tag>")
        return 1

    print("  %d action reference(s), every one pinned to a commit" % pinned)
    return 0


if __name__ == "__main__":
    sys.exit(main())
