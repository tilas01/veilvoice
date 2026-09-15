#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""A surviving mutant is a diff somebody reads, not a number that moved.

    python tools/mutants/check.py mutants.out/missed.txt

# Why a committed list rather than a count

`cargo mutants` changes the code a line at a time and reports how many changes
the test suite failed to object to. Reporting that as a number has two failure
modes and they are both quiet. A number that goes up says something survived
but not what, so somebody has to re-run the half-hour campaign to find out. A
number that stays the same while one survivor is fixed and another appears says
nothing at all.

So the known survivors are committed, one per line, each with the argument for
why it cannot be killed written above it. A campaign is compared against that
list. A new survivor fails the build and is named. A survivor that has been
killed also fails, because the list is then claiming something untrue about the
code, and this repository does not let a file say something that is no longer
so.

# What belongs in the list

Only a mutant that is **unkillable by construction**, with the reason written
out. F-180's campaign left two, both mutations of Argon2's ceiling on
parallelism, which the memory ceiling always reaches first because eight KiB
are wanted per lane: no input can distinguish the mutated code from the
original, so no test can. That is a real entry.

A mutant that merely has no test yet is not an entry. It is a missing test.

Pure standard library, like everything in `tools/`.
"""

import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SURVIVORS = os.path.join(ROOT, "tools", "mutants", "survivors.txt")


def listed(path):
    """The mutants argued for in the committed list, in order."""
    if not os.path.isfile(path):
        return None
    out = []
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line and not line.startswith("#"):
                out.append(line)
    return out


def reported(path):
    """The mutants a campaign says survived.

    `missed.txt` is one mutant per line in cargo-mutants' own notation, and an
    absent file means the campaign found none, which is a result rather than an
    error: a run over a file with no surviving mutants writes nothing.
    """
    if not os.path.isfile(path):
        return []
    out = []
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line:
                out.append(line)
    return out


def main(argv):
    if len(argv) != 2:
        print("usage: check.py <path to mutants.out/missed.txt>", file=sys.stderr)
        return 2

    known = listed(SURVIVORS)
    if known is None:
        print(
            "tools/mutants/survivors.txt is missing, so there is nothing to "
            "compare a campaign against.",
            file=sys.stderr,
        )
        return 1

    found = reported(argv[1])
    new = [m for m in found if m not in known]
    gone = [m for m in known if m not in found]

    if new:
        print("these mutants survived and are not argued for:")
        for mutant in new:
            print("  %s" % mutant)
        print()
        print(
            "A surviving mutant is a claim about the tests, not about the code: "
            "the suite accepted a change to a line and would accept it in a "
            "release. Write the test that objects. Add it to "
            "tools/mutants/survivors.txt only if no input can tell the mutated "
            "code from the original, and write that argument above the line."
        )

    if gone:
        print("these mutants are argued for and no longer survive:")
        for mutant in gone:
            print("  %s" % mutant)
        print()
        print(
            "A test now kills them, so the argument above each in "
            "tools/mutants/survivors.txt is no longer true. Take the line out "
            "with its argument. A list that over-claims is how a real survivor "
            "hides in it."
        )

    if new or gone:
        return 1

    print(
        "  %d surviving mutant%s, every one of them argued for"
        % (len(found), "" if len(found) == 1 else "s")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
