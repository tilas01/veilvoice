#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Every public item is named by something other than its own declaration.

    python tools/audit/reachable.py      # report, and fail on anything reached
                                         # by nothing at all

There is no `--check`, for the same reason `dependencies.py` has none: reading
and checking are one run, so there is no mode in which this passes quietly over
an item nobody calls.

# What this asks, and what it does not

It asks the narrowest version of the question: is this name written down
anywhere in the workspace apart from the line that declares it. Not "does a
released binary reach it", not "is it on a path a user can take". Just whether
anything at all, a caller or a test, ever says the name.

That is deliberately weak, and the weakness is the point. A public item reached
only by its own tests is a normal and often correct thing: a constructor a front
end has not been written for yet, a reader kept beside a writer so the format
has two sides. Twenty-six such items were counted in this tree the day this was
written and every one of them was fine. An item reached by **nothing** is a
different thing. It is code that is compiled, documented, published into the
generated reference and the wiki, carried by every build on every platform, and
answerable to nobody, because there is no caller whose behaviour would change if
it were wrong.

# Why the compiler cannot answer this

`dead_code` stops at the crate boundary. Anything `pub` in a library might be
called by a consumer the compiler cannot see, so it is never reported, and in a
workspace whose libraries have exactly two consumers, both in the workspace,
that is a whole class of defect nothing was looking at.

It is worse than silence. A public accessor keeps its private field alive: the
field is read, by the accessor, so `dead_code` says nothing about the field
either, and both survive together. `VaultStore::last_audit` and the `audit`
field behind it were found that way, together with a `clone` of the audit result
performed on every vault open for a value no line of code ever read.

# The shape of the check

A declaration is `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub const`,
`pub static`, `pub type` or `pub mod` at the start of a line, with or without a
restriction like `pub(crate)`. Declarations inside a `#[cfg(test)]` module are
not the subject: a test helper is reached by its test by construction.

A mention is the name appearing as a word anywhere in any `.rs` file in
`crates/`, on any line but the declaration's own. Method-call syntax, a trait
impl, a macro, a glob re-export and a doc link all contain the name, so all of
them count. Sibling declarations of the same name in other crates count too,
which makes this check slightly *more* forgiving than it looks and is the right
direction for a guard to err in.

Pure standard library, like everything in `tools/`.
"""

import collections
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CRATES = os.path.join(ROOT, "crates")

DECLARATION = re.compile(
    r'^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:unsafe\s+)?(?:extern\s+"[^"]*"\s+)?'
    r"(fn|struct|enum|trait|const|static|type|mod)\s+([A-Za-z_][A-Za-z0-9_]*)"
)
WORD = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
CFG_TEST = re.compile(r"\s*#\[cfg\(test\)\]")

# Names that are declared and named nowhere else on purpose. Empty, and meant to
# stay that way: an entry here is an argument that something unreachable should
# be kept, which is a thing to write out in full at the moment it is made rather
# than a line to add so a build goes green.
EXEMPT = {}


def sources():
    """Every Rust file under `crates/`, in a stable order."""
    found = []
    for base, _, names in os.walk(CRATES):
        for name in sorted(names):
            if name.endswith(".rs"):
                found.append(os.path.join(base, name))
    return sorted(found)


def test_lines(lines):
    """Mark the lines inside each `#[cfg(test)]` module.

    Brace counting from the attribute to the close of the module it guards.
    Crude, and sufficient: these modules are written one way in this tree, they
    are always at the end of a file, and being wrong here can only make the
    check more forgiving, never less.
    """
    inside = [False] * len(lines)
    index = 0
    while index < len(lines):
        if not CFG_TEST.match(lines[index]):
            index += 1
            continue
        opening = index
        while opening < len(lines) and "{" not in lines[opening]:
            opening += 1
        depth = 0
        started = False
        cursor = opening
        while cursor < len(lines):
            depth += lines[cursor].count("{") - lines[cursor].count("}")
            if "{" in lines[cursor]:
                started = True
            if started and depth <= 0:
                break
            cursor += 1
        for line in range(index, min(cursor + 1, len(lines))):
            inside[line] = True
        index = cursor + 1
    return inside


def read():
    """Return the declarations and the set of places every word is written."""
    declarations = []
    mentions = collections.defaultdict(list)
    for path in sources():
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            lines = handle.read().split("\n")
        inside = test_lines(lines)
        for number, line in enumerate(lines):
            if not inside[number]:
                found = DECLARATION.match(line)
                if found:
                    declarations.append((path, number, found.group(1), found.group(2)))
            for word in WORD.findall(line):
                mentions[word].append((path, number))
    return declarations, mentions


def unreached(declarations, mentions):
    """Declarations whose name appears nowhere but on their own line."""
    problems = []
    for path, number, kind, name in declarations:
        if name in EXEMPT:
            continue
        elsewhere = [
            place for place in mentions[name] if place != (path, number)
        ]
        if not elsewhere:
            relative = os.path.relpath(path, ROOT).replace(os.sep, "/")
            problems.append("%s:%d: pub %s %s" % (relative, number + 1, kind, name))
    return problems


def main():
    declarations, mentions = read()
    problems = unreached(declarations, mentions)
    if problems:
        print("these public items are named by nothing but their own declaration:")
        for problem in problems:
            print("  %s" % problem)
        print()
        print(
            "Each is compiled into every build, published into the generated "
            "reference and the wiki, and answerable to no caller. Give it one, "
            "or a test, or take it out. `dead_code` cannot see any of this: a "
            "public item might have a consumer outside the crate, and an "
            "accessor keeps its private field alive while it does."
        )
        return 1
    print(
        "  %d public items, every one of them named somewhere other than its "
        "own declaration" % len(declarations)
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
