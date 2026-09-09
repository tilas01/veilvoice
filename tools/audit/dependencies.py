#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Every dependency says what it is for, where it is declared.

    python tools/audit/dependencies.py           # report, and fail on any gap

There is no `--check`: this tool only reads. Reporting and checking are the
same run, so there is no mode in which it passes quietly over a dependency
nobody explained.

Marker 126. A dependency is a decision: it is code this project ships and does
not review, it is build time on every machine that compiles this, and on the
BSDs and the 32-bit targets it is one more thing that has to work. The decision
is worth one sentence at the moment it is made, and the moment it is made is
the line in the manifest.

Written after the pass that found `sha2` in `veilvoice-verify`, `hex` in its
tests and `hex-literal` in the crypto crate's, none of which any line of code
referred to. All three had been compiled by every build on every platform for
however long they had been there. None of them was noticed by reading the
manifests, because a manifest full of bare names reads as a list rather than as
a set of decisions; they were noticed by asking what each one was *for* and
finding that three had no answer.

**This does not check that a dependency is used.** That is `cargo udeps` and it
needs nightly. What it checks is that somebody said why, which is the part a
tool cannot do and the part that makes the unused ones visible.

# What counts as saying why

A comment line directly above the entry, or a trailing comment on it. Both
forms are already in this tree and both read fine in a manifest, so neither is
imposed on the other.

Sections named `dependencies`, `dev-dependencies` and `build-dependencies` are
covered, including the platform-specific ones under `target.'cfg(...)'`, and
`[workspace.dependencies]` at the root. Feature lists, profiles and metadata
are not dependencies and are left alone.

Pure standard library, like everything in `tools/`.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))


def manifests():
    """Every Cargo.toml this project owns, the fuzzing package included.

    `fuzz/` is excluded from the workspace and is still Rust written here, so
    its dependencies are decisions on the same terms. `tools/docs/generate.py`
    documents it beside the members for the same reason.
    """
    found = [os.path.join(ROOT, "Cargo.toml")]
    crates = os.path.join(ROOT, "crates")
    for name in sorted(os.listdir(crates)):
        path = os.path.join(crates, name, "Cargo.toml")
        if os.path.isfile(path):
            found.append(path)
    fuzz = os.path.join(ROOT, "fuzz", "Cargo.toml")
    if os.path.isfile(fuzz):
        found.append(fuzz)
    return found


def is_dependency_section(name):
    """Whether a `[...]` header opens a list of dependencies.

    `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`,
    `[workspace.dependencies]` and the `[target.'cfg(windows)'.dependencies]`
    family all end in one of those words. `[features]` and `[profile.release]`
    do not, and `[package.metadata]` does not, so nothing else is caught by
    matching on the last segment.
    """
    last = name.split(".")[-1]
    return last in ("dependencies", "dev-dependencies", "build-dependencies")


def unexplained(path):
    """The dependency entries in `path` with no reason beside them."""
    with open(path, "r", encoding="utf-8") as handle:
        lines = handle.read().replace("\r\n", "\n").split("\n")

    problems = []
    section = None
    for number, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("["):
            section = stripped.strip("[]")
            continue
        if not stripped or stripped.startswith("#"):
            continue
        if section is None or not is_dependency_section(section):
            continue
        if "=" not in stripped:
            continue

        name = stripped.split("=")[0].strip().split(".")[0]
        above = lines[number - 1].strip() if number else ""
        trailing = "#" in line.split("=", 1)[1]
        if above.startswith("#") or trailing:
            continue
        problems.append((number + 1, name, section))
    return problems


def counted(path):
    """How many dependency entries `path` declares."""
    with open(path, "r", encoding="utf-8") as handle:
        lines = handle.read().replace("\r\n", "\n").split("\n")
    section = None
    total = 0
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("["):
            section = stripped.strip("[]")
        elif (
            section is not None
            and is_dependency_section(section)
            and stripped
            and not stripped.startswith("#")
            and "=" in stripped
        ):
            total += 1
    return total


def main():
    problems = []
    total = 0
    for path in manifests():
        relative = os.path.relpath(path, ROOT).replace(os.sep, "/")
        total += counted(path)
        for number, name, section in unexplained(path):
            problems.append("%s:%d: %s (in [%s])" % (relative, number, name, section))

    if problems:
        print("these dependencies do not say what they are for:")
        for problem in problems:
            print("  %s" % problem)
        print()
        print(
            "Write one sentence above each, or after it on the same line, "
            "saying what this project calls in it. If there is no answer, "
            "that is the answer: take it out."
        )
        return 1

    print("  %d dependencies, every one of them explained where it is declared" % total)
    return 0


if __name__ == "__main__":
    sys.exit(main())
