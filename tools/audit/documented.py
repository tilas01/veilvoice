#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Every item on a generated page has something written under it.

    python tools/audit/documented.py            # fail if anything is missing
    python tools/audit/documented.py --list     # one path and name per line

# It was not a build guard, and now it is

For a long time this only reported, and said so here. The reason was sound: it
found eighty-one items with no comment, and a guard that fails a build for
eighty-one known things is a guard somebody turns off or a build nobody can
green. The note said it would become a build step in the same commit that took
that number to zero.

That happened on 2026-09-26, and the number was eighty-two by then. It is now run
by `tools/verify.py` and by `ci.yml`, so an item added without a sentence under
it fails before it is pushed rather than appearing as a blank row on a published
page.

The waiting was the right decision and the cost of it is worth naming: for as
long as this only reported, nothing ran it at all, so the eighty-two grew without
anybody being told. A guard held back until it can pass is a guard that has to be
finished, and the thing that finishes it is the last of the sentences rather than
the wiring.

# Why this exists at all

`tools/docs/generate.py` writes a page per source file from the doc comments in
it, and each page lists the items in that file with the first sentence of each
comment beside it. An item with no comment is a row on a published page with
nothing in it, and the page looks broken rather than the code looking
undocumented.

Nothing was counting them. Two earlier passes at this worked from a list of
*all* function names rather than from the missing ones and documented things
that were already documented, twice, while writing nothing where it was
actually needed. That is what a detector is for: the list comes from the same
parser the generator uses, so what this reports is exactly what a reader would
find blank.

# What does not need a comment, and why

**Trait implementations of standard methods.** `Display::fmt`, `From::from`,
`Drop::drop`, `Default::default` and the rest are named by their trait, and a
comment on one says either nothing ("formats this for display") or something
the trait already promises. They are skipped by name.

**Test functions and test modules.** A test's name is its documentation and is
written to be read in a failure report. The generated pages do not carry them.

**Examples and integration tests.** `examples/` and `tests/` hold programs
rather than library items, and `main` in an example is not a public surface.

Everything else is a thing somebody can reach and therefore a thing that needs
a sentence saying what it is for.

Pure standard library, and it imports the generator's own parser rather than
writing a second one, so the two cannot disagree about what an item is.
"""

from __future__ import annotations

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

sys.path.insert(0, os.path.join(ROOT, "tools", "docs"))
import generate as docs  # noqa: E402  (the path has to be set first)

# Methods named by the trait that requires them. A comment on one of these
# restates the trait, and a page that listed them as undocumented would send
# somebody to write forty of those.
TRAIT_METHODS = {
    "add", "as_mut", "as_ref", "borrow", "clone", "clone_from", "cmp", "default",
    "deref", "deref_mut", "div", "drop", "eq", "fill_bytes", "fmt", "from",
    "hash", "index", "into", "mul", "ne", "next", "next_u32", "next_u64",
    "partial_cmp", "provide", "size_hint", "source", "sub", "to_string",
    "try_fill_bytes", "try_from",
}

# Files that hold tests rather than the program. A test's name is what a
# failure report prints and is written to be read there.
TEST_FILES = ("tests.rs",)


def interesting(rel):
    """Whether a source file's items belong on a documented page at all."""
    if rel.startswith("fuzz/"):
        return False
    if "/examples/" in rel or "/tests/" in rel:
        return False
    return os.path.basename(rel) not in TEST_FILES


def undocumented():
    """Every production item with no doc comment, as (file, name, kind)."""
    found = []
    for crate in docs.workspace_crates(ROOT):
        try:
            model = docs.build(ROOT, crate)
        except Exception as error:  # noqa: BLE001  reported, not swallowed
            raise SystemExit("%s could not be read: %s" % (crate, error))
        for entry in model["files"]:
            rel = entry["rel"]
            if not interesting(rel):
                continue
            for item in entry.get("items", []):
                name = item.get("name") or ""
                if item.get("doc"):
                    continue
                if name in TRAIT_METHODS:
                    continue
                if name == "tests" or name.endswith("_tests"):
                    continue
                found.append((rel, name, item.get("kind", "item")))
    return sorted(found)


def main():
    missing = undocumented()
    if "--list" in sys.argv:
        for rel, name, kind in missing:
            print("%s\t%s\t%s" % (rel, name, kind))
        return 0
    if not missing:
        print("  every item on a generated page has a sentence under it")
        return 0
    print("%d item(s) appear on a generated page with nothing under them:"
          % len(missing))
    where = {}
    for rel, name, kind in missing:
        where.setdefault(rel, []).append("%s (%s)" % (name, kind))
    for rel in sorted(where):
        print("  %s" % rel)
        print("    %s" % ", ".join(where[rel]))
    print("  Write a sentence saying what each is for, in the source, and run")
    print("  python tools/docs/generate.py afterwards.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
