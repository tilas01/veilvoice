#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""The step that publishes a release says which commit it is publishing.

    python tools/audit/publishing.py     # report, and fail on any gap

No `--check`, for the reason the other tools here have none: this only reads,
so reporting and checking are one run.

# What this is for

A release is a claim that these binaries came from this source. Everything in
`release.yml` above the publishing step exists to support it: each binary is
built twice in different directories and compared, the hashes are signed, and
the notes tell a reader how to reproduce the build themselves from the tag.

The tag is the load-bearing part of that sentence, and creating a release for
a tag that does not exist yet makes GitHub create the tag. With no
`target_commitish` it creates it **at the default branch**, not at the commit
the run compiled. The reader then checks out the tag, rebuilds, gets different
bytes, and correctly concludes the release does not reproduce.

That is not hypothetical. It is F-170: v0.1.20 and v0.1.21 were both tagged at
whatever `main` happened to be, thirteen minutes and one branch away from what
they actually published.

# And the other half

`release.yml` reads a release's notes out of `CHANGELOG.md` by matching
`## v<version>` exactly. It used to print "No changelog section found" into the
notes and publish anyway, which is how v0.1.21 went out describing nothing at
all. The step must be able to fail, and this checks that the refusal is still
there, because a guard that is deleted in an edit is a guard nobody notices
the absence of.

`tools/release/version.py` asks the matching question of the changelog itself,
before a release is ever dispatched. This asks it of the workflow.

Pure standard library, like everything in `tools/`.
"""

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORKFLOW = os.path.join(ROOT, ".github", "workflows", "release.yml")

# The action that creates the release. Named rather than matched loosely, so
# swapping it for another one is a decision somebody makes here too, with this
# file's argument in front of them.
PUBLISHER = "softprops/action-gh-release"


def main():
    if not os.path.isfile(WORKFLOW):
        print("there is no .github/workflows/release.yml, so nothing publishes.")
        return 1

    with open(WORKFLOW, "r", encoding="utf-8") as handle:
        lines = handle.read().splitlines()

    gaps = []

    # ---- the publishing step names the commit it is publishing --------------
    steps = [i for i, line in enumerate(lines) if PUBLISHER in line and not line.strip().startswith("#")]
    if not steps:
        gaps.append(
            "no step uses %s, so this check cannot see how the release is "
            "created and is checking nothing" % PUBLISHER
        )
    for at in steps:
        # The step's `with:` block: everything more indented than the `- uses:`
        # line, up to the next thing at that indentation or less.
        indent = len(lines[at]) - len(lines[at].lstrip())
        body = []
        for line in lines[at + 1:]:
            if line.strip() and (len(line) - len(line.lstrip())) <= indent:
                break
            body.append(line)
        block = "\n".join(body)
        if "target_commitish:" not in block:
            gaps.append(
                "the %s step (line %d) sets no target_commitish, so GitHub "
                "would create the tag at the default branch rather than at the "
                "commit this run built" % (PUBLISHER, at + 1)
            )
        elif not re.search(r"target_commitish:\s*\$\{\{\s*github\.sha\s*\}\}", block):
            gaps.append(
                "the %s step (line %d) sets target_commitish to something "
                "other than github.sha, so the tag may not name the commit "
                "that was built" % (PUBLISHER, at + 1)
            )

    # ---- a release with no notes does not publish ---------------------------
    #
    # The workflow still *writes* "No changelog section found" into the notes,
    # and that is correct: a dry run publishes nothing, so it is allowed to
    # report the gap and carry on. What must exist is the refusal on the path
    # that does publish. That is what is checked, rather than the presence of
    # the sentence, because forbidding the sentence would fail the dry run's
    # honest report along with the defect.
    body = "\n".join(lines)
    if "has no '## $tag' section" not in body:
        gaps.append(
            "release.yml no longer refuses to publish when CHANGELOG.md has no "
            "section for the version, so a heading written in the wrong shape "
            "would silently produce an empty release"
        )

    if gaps:
        print("the release workflow would publish something it should not:")
        for gap in gaps:
            print("  %s" % gap)
        print()
        print(
            "A release is a claim that these binaries came from this source. "
            "A tag that does not name what was built, or notes that say the "
            "notes are missing, breaks that claim rather than weakening it."
        )
        return 1

    print("  the release step tags the commit it built, and refuses empty notes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
