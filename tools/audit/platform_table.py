#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
The table of what runs where says what the release workflow actually builds.

# The defect this exists to stop

`docs/USER_GUIDE.md` has a table under *What runs where*: one row per kind of
platform, and for each whether the archive has the command line, the desktop
application and live microphone scrambling. It is the first place a reader
looks to find out whether the download in front of them does what they want.

Two of its rows were wrong, and had been for as long as the table existed. The
statically linked Linux builds and the Raspberry Pi build both said **yes** to
the desktop application and **yes** to the live microphone. Both are built
with `cli_only: true` in `.github/workflows/release.yml`: the command line
alone, with the `live` feature off, because `cpal` cannot be linked into a
static binary and the cross toolchain has no ALSA for armv7. The release page
got this right, because `tools/site/releases.py` labels those archives
"command line only"; the manual got it wrong, and the two were never compared.

Somebody reading the manual on a Raspberry Pi downloaded an archive with no
window in it, having been told there was one.

# What is checked

Every row of the table is one this guard knows, by its first cell, and knows
which archive labels it describes. Every label the release matrix builds is
described by at least one row. And for each row:

* if every archive it describes is `cli_only`, the desktop column must say
  `not shipped` and the live column must say `no`;
* if none of them is, the desktop column must begin with `yes`.

The BSD archives are built outside the matrix and are not asserted on here:
since roadmap item 162 the FreeBSD job may carry the window, and whether it did
is a fact about one release rather than about the workflow.

A row this guard does not know fails, rather than being skipped, so a platform
added to the table has to be added here with the labels it means.
"""

import re
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

#: Each row of the table, by its first cell, and the archive labels it means.
#: An empty list is a row about something no matrix archive is: the BSDs.
ROWS = {
    "Windows 10 and 11, x86-64": ["windows-x86_64"],
    "macOS on Intel": ["macos-x86_64"],
    "macOS on Apple Silicon": ["macos-arm64"],
    "Linux, x86-64 and arm64": ["linux-x86_64", "linux-arm64"],
    "Linux, statically linked (musl)": ["linux-x86_64-musl-static", "linux-arm64-musl-static"],
    "Raspberry Pi and other armv7": ["linux-armv7-pi"],
    "WSL on Windows": ["linux-x86_64"],
    "FreeBSD, OpenBSD, NetBSD": [],
}


def matrix(text):
    """Every label in the build matrix, and whether it is `cli_only`."""
    rows = {}
    for block in re.split(r"\n\s*- os:", text):
        label = re.search(r"^\s*label:\s*(\S+)", block, re.M)
        if not label:
            continue
        cli_only = re.search(r"^\s*cli_only:\s*true\s*$", block, re.M) is not None
        rows[label.group(1)] = cli_only
    return rows


def table(text):
    """The rows under *What runs where*, as lists of cells."""
    heading = text.find("### What runs where")
    if heading < 0:
        return None
    rows = []
    started = False
    for line in text[heading:].splitlines()[1:]:
        if line.startswith("|"):
            started = True
            cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
            if cells[0] in ("Platform",) or set(cells[0]) <= set("-: "):
                continue
            rows.append(cells)
        elif started:
            break
    return rows


def audit(root):
    problems = []
    workflow = (root / ".github/workflows/release.yml").read_text(encoding="utf-8")
    guide = (root / "docs/USER_GUIDE.md").read_text(encoding="utf-8")

    built = matrix(workflow)
    if not built:
        return ["  no labels were found in the release workflow's matrix, so this "
                "guard can no longer read it."], 0
    rows = table(guide)
    if not rows:
        return ["  docs/USER_GUIDE.md has no table under `### What runs where`, or "
                "this guard can no longer read it."], 0

    described = set()
    for cells in rows:
        if len(cells) < 4:
            problems.append("  the row `%s` does not have four cells" % " | ".join(cells))
            continue
        name, _, desktop, live = cells[:4]
        if name not in ROWS:
            problems.append(
                "  the row `%s` is not one this guard knows. Add it to ROWS in "
                "tools/audit/platform_table.py with the archive labels it describes."
                % name)
            continue
        labels = ROWS[name]
        described.update(labels)
        missing = [label for label in labels if label not in built]
        if missing:
            problems.append("  the row `%s` describes %s, which the release matrix does "
                            "not build" % (name, ", ".join(missing)))
            continue
        if not labels:
            continue
        flags = [built[label] for label in labels]
        if all(flags):
            if desktop != "not shipped":
                problems.append(
                    "  `%s` says the desktop app is `%s`, and every archive it describes "
                    "is built command line only (cli_only in release.yml). It must say "
                    "`not shipped`." % (name, desktop))
            if live != "no":
                problems.append(
                    "  `%s` says live microphone is `%s`, and its archives are built with "
                    "the live feature off. It must say `no`." % (name, live))
        elif not any(flags):
            if not desktop.startswith("yes"):
                problems.append(
                    "  `%s` says the desktop app is `%s`, and its archives carry the "
                    "window." % (name, desktop))
        else:
            problems.append(
                "  `%s` describes archives some of which have the window and some of "
                "which do not. Split the row." % name)

    for label in sorted(set(built) - described):
        problems.append(
            "  %s is built by the release matrix and no row of the table describes it."
            % label)

    return problems, len(rows)


def self_test():
    """A row claiming a window a cli_only archive lacks is caught; a true table is not."""
    failures = 0
    where = Path(tempfile.mkdtemp(prefix="veilvoice-platform-table-"))
    try:
        def build(desktop, live, cli_only):
            (where / ".github/workflows").mkdir(parents=True, exist_ok=True)
            (where / "docs").mkdir(parents=True, exist_ok=True)
            (where / ".github/workflows/release.yml").write_text(
                "matrix:\n  include:\n"
                "    - os: ubuntu-latest\n      target: a\n      label: linux-armv7-pi\n"
                + ("      cli_only: true\n" if cli_only else ""),
                encoding="utf-8")
            (where / "docs/USER_GUIDE.md").write_text(
                "### What runs where\n\n"
                "| Platform | Command line | Desktop app | Live microphone |\n"
                "|---|---|---|---|\n"
                "| Raspberry Pi and other armv7 | yes | %s | %s |\n\nAfter.\n" % (desktop, live),
                encoding="utf-8")

        for name, case, must_catch in [
            ("a window claimed for a command-line-only archive",
             ("yes", "no", True), "not shipped"),
            ("a live microphone claimed with the feature off",
             ("not shipped", "yes", True), "must say `no`"),
            ("a window denied to an archive that carries one",
             ("not shipped", "no", False), "carry the window"),
        ]:
            build(*case)
            problems, _ = audit(where)
            if any(must_catch in line for line in problems):
                print("    caught: %s" % name)
            else:
                failures += 1
                print("    MISSED: %s" % name)

        build("not shipped", "no", True)
        problems, _ = audit(where)
        if problems:
            failures += 1
            print("    MISSED: a true table was reported as faulty")
            print("\n".join("      " + line for line in problems))
        else:
            print("    clean: a table that agrees with the workflow")
    finally:
        shutil.rmtree(where, ignore_errors=True)

    if failures:
        print("  %d self-test(s) failed: this guard does not catch what it claims to"
              % failures)
        return 1
    print("  every fault this guard exists for is caught by it")
    return 0


def main():
    if "--self-test" in sys.argv:
        return self_test()
    problems, counted = audit(ROOT)
    if problems:
        print("\n".join(problems))
        return 1
    print("  %d row(s) of the platform table, every one of them what the workflow builds"
          % counted)
    return 0


if __name__ == "__main__":
    sys.exit(main())
