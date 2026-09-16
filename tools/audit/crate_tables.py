#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Every crate in the workspace appears in every table that lists the crates.

    python tools/audit/crate_tables.py

There is no `--check`: reading and checking are one run.

# What went wrong

Three documents list the crates: the README's Layout table, the README's
"use it as a library" table, and `docs/USING_THE_CRATES.md`. They listed
thirteen, twelve and thirteen crates. The workspace has twenty-seven.

The front page of the website renders the README, so the twelve-crate table
was what the site said the project was made of, and it had been wrong for
fifteen crates' worth of work. Nobody had to do anything careless for that to
happen: a crate is added, and the tables are somewhere else.

# What is checked

That each table names every crate it is supposed to name, and names no crate
that does not exist. The descriptions are left alone: a sentence assembled
from a crate name is padding, and every one of these is written by hand.

The library tables cover the crates with a `src/lib.rs`, because a
binary-only crate is not something anybody can depend on. The Layout table
covers every workspace member, because it describes the repository.
"""

import io
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

# Each table, by the document it is in and the heading it sits under.
TABLES = [
    ("README.md", "the README's \"use it as a library\" table", "libraries"),
    ("README.md", "the README's Layout table", "all"),
    ("docs/USING_THE_CRATES.md", "docs/USING_THE_CRATES.md's crate table", "libraries"),
]

ROW = re.compile(r"^\|\s*`(?P<crate>veilvoice-[a-z]+)`\s*\|", re.M)


def read(relative):
    with io.open(os.path.join(ROOT, relative), encoding="utf-8") as handle:
        return handle.read()


def crates():
    """Every workspace member, and which of them are libraries."""
    everything, libraries = set(), set()
    directory = os.path.join(ROOT, "crates")
    for name in sorted(os.listdir(directory)):
        if not os.path.isfile(os.path.join(directory, name, "Cargo.toml")):
            continue
        everything.add(name)
        if os.path.isfile(os.path.join(directory, name, "src", "lib.rs")):
            libraries.add(name)
    return everything, libraries


def tables(text):
    """The crate rows of each table in a document, in the order they appear.

    A table ends at the first line that is not one of its rows, so two tables
    in one document do not run together.
    """
    found, current = [], []
    for line in text.splitlines():
        match = ROW.match(line)
        if match:
            current.append(match.group("crate"))
        elif current:
            found.append(current)
            current = []
    if current:
        found.append(current)
    return found


def main():
    everything, libraries = crates()
    problems = []

    by_document = {}
    for relative, _, _ in TABLES:
        if relative not in by_document:
            by_document[relative] = tables(read(relative))

    seen = {}
    for relative, description, scope in TABLES:
        index = seen.get(relative, 0)
        seen[relative] = index + 1
        found = by_document[relative]
        if index >= len(found):
            problems.append("%s no longer exists" % description)
            continue

        listed = set(found[index])
        wanted = libraries if scope == "libraries" else everything

        for crate in sorted(wanted - listed):
            problems.append("%s does not list %s" % (description, crate))
        for crate in sorted(listed - wanted):
            reason = ("is not a crate in this workspace" if crate not in everything
                      else "has no src/lib.rs, so nothing can depend on it")
            problems.append("%s lists %s, which %s" % (description, crate, reason))

    if problems:
        print("the crate tables disagree with the workspace:")
        for problem in problems:
            print("  %s" % problem)
        print()
        print(
            "The website's front page renders the README, so a table that is "
            "short is the site telling a reader the project is smaller than it "
            "is. Add the row, with a sentence somebody wrote."
        )
        return 1

    print("  %d crates, %d of them libraries, in all %d tables"
          % (len(everything), len(libraries), len(TABLES)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
