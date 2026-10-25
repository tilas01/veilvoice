#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Every row of a Markdown table has as many cells as its heading says.

# The defect this exists to stop coming back

`wiki/Crate-veilvoice-conversation.md` listed the crate's six source files in a
three-column table, and every row of it reached a reader like this:

    [[edit.rs    File-veilvoice-conversation-edit]]    649

The generator wrote `| [[`edit.rs`|File-veilvoice-conversation-edit]] | 649 |
... |`. A GitHub wiki link is `[[text|Page]]`, and in a table the pipe is the
column separator, so that is not one cell holding a link. It is a cell holding
`[[`edit.rs`` and a cell holding `File-veilvoice-conversation-edit]]`, and the
rest of the row shifted one column left. Fifteen crate pages, a hundred and
eighty-four rows, every one of them wrong, on the pages whose whole job is to
show what the crate is made of.

The two generators that write that table, `tools/docs/generate.py` and
`tools/docs/sources.py`, both escape the pipes in the summary cell beside it,
and have done since the table was written. So the escaping was there; it was
put on the cell whose text comes from a doc comment, and not on the cell the
generator writes itself. That is the whole of the defect, and it is why this is
a check rather than one more `replace`: the next person to write a table will
escape the part they are thinking about.

# Why counting cells rather than looking for brackets

A check for a visible `[[` would have found this one and nothing else. Counting
cells finds the fault itself, which is a row that does not fit its heading,
whatever produced it: a pipe in a path, a pipe in a regular expression quoted
in a cell, a hand-drawn table with one separator missing. It also says
something a reader can act on, that this row has one cell too many, rather than
naming a symptom and leaving the cause to be worked out.

# What is checked

**Every table in every Markdown file in the repository.** The first row is the
heading and fixes the width; the alignment row under it is skipped; every row
after it must have that many cells. A blank line or any non-table line ends the
table, so two tables in one section are two tables.

A pipe escaped as `\\|` is text and not a separator, which is what the summary
cells rely on. A fenced code block is skipped whole: a table drawn inside one is
an illustration, and this file's own documentation contains a broken row on
purpose.

# What this deliberately does not do

It does not check that a table is well formed in any other way: an alignment row
with the wrong number of dashes, a heading with an empty cell, or a table whose
heading repeats a name are all left alone. They are visible in the rendering.
A row that has lost a column is not, because it renders as a table.

It does not read HTML tables. The site's are generated from these, and
`tools/site-tests/source.test.js` reads them there.
"""

from __future__ import annotations

import io
import os
import re
import shutil
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))

# A pipe that separates columns. One escaped as `\|` is a literal and is left
# in the cell, which is how a summary holding a pipe is written.
SEPARATOR = re.compile(r"(?<!\\)\|")

# `|---|---:|---|`, the row that sets each column's alignment. It is part of the
# table's frame rather than a row of it, and its cell count is not checked
# because a reader can see at a glance when it is wrong.
ALIGNMENT = re.compile(r"^\|[\s:|-]+\|?$")

FENCE = re.compile(r"^\s*```")

# Where to look. Every Markdown file the repository carries, wherever it lives,
# because the defect was in generated output and the generator is not the only
# thing that writes a table.
SKIP_DIRS = {".git", "target", "node_modules", "__pycache__"}


def cells(line):
    """How many cells a table row has."""
    parts = SEPARATOR.split(line.strip())
    # The outer pipes produce an empty field at each end. A row written without
    # them is legal GFM, so only an empty one is dropped.
    if parts and not parts[0].strip():
        parts = parts[1:]
    if parts and not parts[-1].strip():
        parts = parts[:-1]
    return len(parts)


def problems_in(text, rel):
    """Every row of `text` whose cell count differs from its heading's."""
    found = []
    width = None
    fenced = False
    for number, line in enumerate(text.split("\n"), 1):
        if FENCE.match(line):
            fenced = not fenced
            width = None
            continue
        if fenced:
            continue
        stripped = line.strip()
        if not stripped.startswith("|"):
            width = None
            continue
        if width is not None and ALIGNMENT.match(stripped):
            continue
        if width is None:
            width = cells(line)
            continue
        here = cells(line)
        if here != width:
            found.append(
                "%s:%d has %d cell(s) where its heading has %d: %s"
                % (rel, number, here, width, re.sub(r"\s+", " ", stripped)[:72]))
    return found


def markdown(root):
    """Every Markdown file under `root`, repository order."""
    out = []
    for dirpath, names, files in os.walk(root):
        names[:] = sorted(n for n in names if n not in SKIP_DIRS)
        for name in sorted(files):
            if name.endswith(".md"):
                full = os.path.join(dirpath, name)
                out.append((full, os.path.relpath(full, root).replace(os.sep, "/")))
    return out


def audit(root):
    found = []
    tables = 0
    for full, rel in markdown(root):
        with io.open(full, encoding="utf-8") as handle:
            text = handle.read()
        tables += len(re.findall(r"(?m)^\s*\|[\s:|-]+\|?$", text))
        found.extend(problems_in(text, rel))
    return found, tables


def self_test():
    """Six trees: the defect as it was, and the shapes that are not faults."""
    tree = tempfile.mkdtemp()
    try:
        cases = [
            # The row that was published, pipe and all.
            ("broken.md",
             "| File | Lines | What it is |\n"
             "|---|---:|---|\n"
             "| [[`edit.rs`|File-veilvoice-conversation-edit]] | 649 | A plan. |\n",
             True, "a wiki link's pipe splitting a cell"),
            # The same table written the way the fix writes it.
            ("fixed.md",
             "| File | Lines | What it is |\n"
             "|---|---:|---|\n"
             "| [`edit.rs`](File-veilvoice-conversation-edit) | 649 | A plan. |\n",
             False, "an ordinary link in a cell"),
            # An escaped pipe is text, which is what a summary relies on.
            ("escaped.md",
             "| Name | What it is |\n"
             "|---|---|\n"
             "| `kind` | either `a` \\| `b`, and nothing else |\n",
             False, "an escaped pipe"),
            # A row short of a column, which renders as a table and so is the
            # case a reader cannot see.
            ("short.md",
             "| Name | Lines | What it is |\n"
             "|---|---:|---|\n"
             "| `lib.rs` | 182 |\n",
             True, "a row missing its last cell"),
            # A broken row inside a fence is an illustration, as in this file.
            ("fenced.md",
             "Here is the defect:\n\n"
             "```\n"
             "| File | Lines |\n"
             "|---|---|\n"
             "| [[`a.rs`|Page-a]] | 1 | 2 |\n"
             "```\n",
             False, "a broken row quoted inside a fence"),
            # Two tables in one file, of different widths: a blank line ends a
            # table, so the second does not inherit the first's width.
            ("two.md",
             "| A | B |\n|---|---|\n| 1 | 2 |\n\n"
             "| A | B | C |\n|---|---|---|\n| 1 | 2 | 3 |\n",
             False, "two tables of different widths"),
        ]
        bad = []
        for name, text, should_fail, what in cases:
            with io.open(os.path.join(tree, name), "w", encoding="utf-8") as handle:
                handle.write(text)
            found = problems_in(text, name)
            if bool(found) != should_fail:
                bad.append("  %s: %s was %s"
                           % (name, what,
                              "not caught" if should_fail else "wrongly caught"))
        if bad:
            print("  this guard does not catch what it claims to:")
            print("\n".join(bad))
            return 1
        print("  every fault this guard exists for is caught by it, and the "
              "four shapes that are not faults are left alone")
        return 0
    finally:
        shutil.rmtree(tree)


def main():
    if "--self-test" in sys.argv:
        return self_test()

    found, tables = audit(ROOT)
    if found:
        print("  these table rows do not fit their heading, so every cell "
              "after the fault is in the wrong column:")
        for line in found[:20]:
            print("    %s" % line)
        if len(found) > 20:
            print("    ... and %d more" % (len(found) - 20))
        print()
        print("    A pipe inside a cell is a column separator. Escape it as")
        print("    \\| , or write the cell so it has none: a table cell takes")
        print("    an ordinary [name](Page) link rather than [[name|Page]].")
        return 1
    print("  %d Markdown table(s), every row fitting its heading" % tables)
    return 0


if __name__ == "__main__":
    sys.exit(main())
