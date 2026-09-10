#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Every feature selection a release builds is built here too.

    python tools/audit/features.py            # print the selections, and check them
    python tools/audit/features.py --build    # and compile each one

# The failure this exists to catch

`veilvoice-audio` has a `live` feature, off on the BSDs and on the musl and
cross targets because `cpal` has no backend for them. That is nine of the
twelve jobs in the release workflow, and **no job in CI built that
configuration at all**: every `cargo` line in `ci.yml` takes the default
features, so a module that compiles only with `live` on passed every check on
every platform and then failed nine release jobs at once.

That is exactly what happened. `pub mod record;` lost its
`#[cfg(feature = "live")]` when a `playback` module was inserted above it and
took over the attribute line that had belonged to `record`; the crate then
built everywhere CI looked and nowhere it did not. It was found by dispatching
a release, two days and four commits later, which is the slowest and most
expensive way this repository has ever found a compile error.

# Why it reads the workflow

`CLAUDE.md` asks that a fact living in more than one place be derived rather
than repeated. The set of feature selections a release builds lives in
`release.yml`, and that is the copy that is true: a target added to the matrix
with a new selection must be built by this check without anybody remembering
to add it here. So the selections are read out of the workflow, and a workflow
rewritten into a shape this cannot read fails rather than quietly checking
nothing.

Pure standard library, like everything in `tools/`.
"""

import argparse
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORKFLOW = os.path.join(ROOT, ".github", "workflows", "release.yml")

# The selection every native target uses. Read from the workflow like the
# others, but named here because it is the one CI already builds and so is
# reported rather than compiled again.
ALREADY_IN_CI = "--workspace"


def selections():
    """The `cargo build` argument sets the release workflow uses.

    Each arrives as `CARGO_ARGS=<arguments>` in the step that decides what a
    target builds. Read as text rather than as YAML for the reason
    `dependabot.py` gives: this project ships no YAML parser and will not add
    one to read a file it wrote itself.
    """
    if not os.path.isfile(WORKFLOW):
        return None
    with open(WORKFLOW, "r", encoding="utf-8") as handle:
        body = handle.read()
    found = []
    for match in re.finditer(r'echo\s+"CARGO_ARGS=([^"]+)"', body):
        arguments = match.group(1).strip()
        if arguments not in found:
            found.append(arguments)
    return found


# Where these builds go, and why it is not `target/`.
#
# `-p veilvoice-cli --no-default-features` produces a binary called `veilvoice`
# with no live mode in it, and the ordinary build produces one with live mode in
# it under the same name. Sharing a directory means whichever ran last is the
# one every other check reads, and `tools/shots/terminal.py` reads it to ask
# what `veilvoice live --help` prints. Running this tool would then fail that
# check with a mismatch that is nothing to do with the drawings, which is
# exactly the confusion it happened to cause the day it was written.
#
# So this compiles somewhere of its own. The cost is that these selections do
# not share the main build's cache and are compiled from scratch the first time.
TARGET = os.path.join("target", "feature-audit")


def build(arguments):
    """Compile one selection, and say what happened."""
    command = ["cargo", "build", "--release", "--locked"] + arguments.split()
    print("  %s" % " ".join(command))
    where = dict(os.environ, CARGO_TARGET_DIR=os.path.join(ROOT, TARGET))
    finished = subprocess.run(command, cwd=ROOT, env=where)
    return finished.returncode == 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--build",
        action="store_true",
        help="compile each selection rather than only listing it",
    )
    options = parser.parse_args()

    found = selections()
    if found is None:
        print("there is no .github/workflows/release.yml, so nothing builds a release.")
        return 1

    # A workflow whose shape changed under this would otherwise report success
    # over an empty list, which is the way a check stops checking without
    # anybody noticing.
    if len(found) < 2:
        print(
            "only %d feature selection could be read out of release.yml, and a "
            "release builds at least two: the whole workspace on the native "
            "targets and the command line alone where cpal has no backend. The "
            "workflow has changed shape and this check is no longer reading it."
            % len(found)
        )
        return 1

    print("  %d feature selections in the release workflow" % len(found))
    for arguments in found:
        note = " (built by the test job already)" if arguments == ALREADY_IN_CI else ""
        print("    %s%s" % (arguments, note))

    if not options.build:
        return 0

    failed = []
    for arguments in found:
        if arguments == ALREADY_IN_CI:
            # Built by `cargo build --workspace --release` in the test job, on
            # every platform rather than only this one. Building it twice would
            # add four minutes to say the same thing.
            continue
        print()
        if not build(arguments):
            failed.append(arguments)

    if failed:
        print()
        print("these feature selections do not compile:")
        for arguments in failed:
            print("  cargo build --release %s" % arguments)
        print()
        print(
            "A release builds each of these, so this would have failed the "
            "release workflow instead. The usual cause is code reachable "
            "without a feature that needs it: check that every module, import "
            "and enum variant guarded by a `#[cfg(feature = ...)]` still "
            "carries its own attribute."
        )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
