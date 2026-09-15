#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""No build output lives inside this repository except where it is expected.

    python tools/audit/build_output.py    # report, and fail on a stray one

There is no `--check`, for the same reason `dependencies.py` has none: reading
and checking are one run.

# What this is for

Cargo marks every directory it builds into with a `CACHEDIR.TAG` carrying a
fixed signature, so build output announces itself and does not have to be
guessed at by name. Two such directories are expected here: `target/` at the
root, and `fuzz/target/` for the fuzzing package, which is a separate Cargo
project on purpose. Anything else is a build nobody asked for.

# Why nothing was watching, and why nothing would have

`.gitignore` carries a bare `target/`, which git matches at **any** depth. That
is the right rule, because a stray build directory is certainly not source. It
also means such a directory never appears in `git status`, never appears in a
diff, and is never cleaned, so the only way to notice it is to go looking with
`du`.

One was found this way: `tools/measured/generate.py` redirected the test build
to `%LOCALAPPDATA%/veilvoice/target` on Windows and, through a fallback nobody
had thought about, to `<repo>/veilvoice/target` on every other machine. It was
a second complete copy of the workspace build, inside the repository, rebuilt
every time the measured numbers were regenerated. It was fifteen gigabytes when
it was first found, and what found it was a mutation-testing campaign dying for
want of disk, several steps removed from the cause. F-185.

So this is not a tidiness check. A stray build directory costs the disk twice
over, doubles the compile in any pass that regenerates, and hides from every
tool that would otherwise report it.

Pure standard library, like everything in `tools/`.
"""

import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Cargo writes this first line into the `CACHEDIR.TAG` of every directory it
# builds into. Matching on it rather than on the name `target` means a
# directory called `target` that holds something else is left alone, and a
# build directory under any other name is still found.
SIGNATURE = "Signature: 8a477f597d28d172789f06886806bc55"

# The two that belong here, as paths relative to the root.
EXPECTED = {"target", os.path.join("fuzz", "target")}

# Never descended into: git's own storage is large, is not ours, and holds no
# build output of this project's making.
SKIP = {".git"}


def build_directories():
    """Every directory in the tree that Cargo has marked as build output."""
    found = []
    for base, names, files in os.walk(ROOT):
        names[:] = [n for n in names if n not in SKIP]
        if "CACHEDIR.TAG" not in files:
            continue
        tag = os.path.join(base, "CACHEDIR.TAG")
        try:
            with open(tag, "r", encoding="utf-8", errors="replace") as handle:
                first = handle.readline().strip()
        except OSError:
            continue
        if first != SIGNATURE:
            continue
        found.append(os.path.relpath(base, ROOT))
        # Nothing below a build directory is worth walking, and some of it is
        # very deep.
        names[:] = []
    return sorted(found)


def size_of(relative):
    """Bytes under `relative`, for saying how much a stray one is costing."""
    total = 0
    for base, _, files in os.walk(os.path.join(ROOT, relative)):
        for name in files:
            try:
                total += os.path.getsize(os.path.join(base, name))
            except OSError:
                pass
    return total


def readable(count):
    """A size somebody can read, which is the point of reporting it at all."""
    for unit in ("B", "KiB", "MiB", "GiB"):
        if count < 1024 or unit == "GiB":
            return "%.1f %s" % (count, unit)
        count /= 1024.0
    return "%.1f GiB" % count


def main():
    found = build_directories()
    strays = [d for d in found if d.replace("/", os.sep) not in EXPECTED]
    if strays:
        print("these directories hold build output and are not where it belongs:")
        for stray in strays:
            print("  %s (%s)" % (stray.replace(os.sep, "/"), readable(size_of(stray))))
        print()
        print(
            "`.gitignore` matches `target/` at any depth, so none of this shows "
            "in `git status` and none of it is ever cleaned. Find whatever set "
            "CARGO_TARGET_DIR to a path inside the repository and point it at "
            "the root `target/` instead, then delete the directory above."
        )
        return 1
    if len(found) == 1:
        print("  1 build directory, and it is where it belongs")
    else:
        print("  %d build directories, every one where it belongs" % len(found))
    return 0


if __name__ == "__main__":
    sys.exit(main())
