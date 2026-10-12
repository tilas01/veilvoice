#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""A surviving mutant is a diff somebody reads, not a number that moved.

    python tools/mutants/check.py mutants.out/missed.txt
    python tools/mutants/check.py --lint
    python tools/mutants/check.py --self-test

# Why a committed list rather than a count

`cargo mutants` changes the code a line at a time and reports how many changes
the test suite failed to object to. Reporting that as a number has two failure
modes and they are both quiet. A number that goes up says something survived
but not what, so somebody has to re-run the half-hour campaign to find out. A
number that stays the same while one survivor is fixed and another appears says
nothing at all.

So the known survivors are committed, one per entry, each with the argument for
why it cannot be killed written above it. A campaign is compared against that
list. A new survivor fails the build and is named. A survivor that has been
killed also fails, because the list is then claiming something untrue about the
code, and this repository does not let a file say something that is no longer
so.

# What an entry is keyed on, and why it is not the line number

cargo-mutants names a mutant by where it is: `<path>:<line>:<column>`, plus a
sentence saying what the mutation does. The list was copied from that, and
comparing the two was a string comparison.

A line number is an address rather than an identity. Insert a paragraph of doc
comment three hundred lines higher and every entry below it is wrong, while
nothing about any of the mutants has changed. The comparison then reports the
same untouched mutant twice over: once as a survivor nobody has argued for, and
once as an argument that no longer holds. Both messages are false and both ask
for exactly the wrong thing to be done. F-250 found nine of the twenty entries
in that state, pointing at a closing brace, at a doc comment, and at functions
hundreds of lines from the ones named beside them.

So an entry is keyed on the mutation instead: the file, what the mutation does,
and the text of the line it does it to. None of the three moves when code above
it moves, and all of them change when the mutated line is edited, which is
exactly when the argument above the entry needs reading again. `missed.txt`
still arrives in cargo-mutants' own notation, so the line number in it is read
once, to look the source line up, and then discarded.

Two mutants can share all three parts of that key. The six-bit encoder has one
line with two `|` in it and both survive. Entries are therefore counted rather
than merely matched, so killing one of a pair still fails the comparison.

# What belongs in the list

Only a mutant that is **unkillable by construction**, with the reason written
out. F-180's campaign left two, both mutations of Argon2's ceiling on
parallelism, which the memory ceiling always reaches first because eight KiB
are wanted per lane: no input can distinguish the mutated code from the
original, so no test can. That is a real entry.

A mutant that merely has no test yet is not an entry. It is a missing test.

Pure standard library, like everything in `tools/`.
"""

import collections
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SURVIVORS = os.path.join(ROOT, "tools", "mutants", "survivors.txt")
WORKFLOW = os.path.join(ROOT, ".github", "workflows", "mutants.yml")

# cargo-mutants writes `<path>:<line>:<column>: <what the mutation does>`.
REPORTED = re.compile(r"^(?P<path>[^:]+):(?P<line>\d+):(?P<column>\d+): (?P<what>.+)$")
# An entry in the committed list, and the source line it is about.
ENTRY = re.compile(r"^(?P<path>[^:\s][^:]*): (?P<what>.+)$")
ON = re.compile(r"^\s+on: (?P<source>.+)$")


def listed(path):
    """Parse the committed list into entries, with whatever is malformed.

    An entry is two lines: the file and what the mutation does, then an
    indented `on:` line carrying the source line it does it to. Both are
    needed, because the pair of them is the key.
    """
    if not os.path.isfile(path):
        return None, [
            "tools/mutants/survivors.txt is missing, so there is nothing to "
            "compare a campaign against."
        ]

    entries = []
    problems = []
    with open(path, "r", encoding="utf-8") as handle:
        for number, raw in enumerate(handle, 1):
            if not raw.strip() or raw.lstrip().startswith("#"):
                continue
            on = ON.match(raw.rstrip("\n"))
            if on:
                if not entries:
                    problems.append(
                        "line %d: an `on:` line before any entry it could "
                        "belong to" % number
                    )
                elif entries[-1]["source"] is not None:
                    problems.append(
                        "line %d: a second `on:` line for the same entry"
                        % number
                    )
                else:
                    entries[-1]["source"] = on.group("source").strip()
                continue
            if REPORTED.match(raw.strip()):
                problems.append(
                    "line %d: still in cargo-mutants' `path:line:column` form. "
                    "Drop the line and column and add an `on:` line with the "
                    "source line instead: %s" % (number, raw.strip())
                )
                continue
            entry = ENTRY.match(raw.strip())
            if not entry:
                problems.append(
                    "line %d: neither `<path>: <what the mutation does>` nor an "
                    "indented `on:` line: %s" % (number, raw.strip())
                )
                continue
            entries.append(
                {
                    "path": entry.group("path"),
                    "what": entry.group("what").strip(),
                    "source": None,
                    "line": number,
                }
            )

    for entry in entries:
        if entry["source"] is None:
            problems.append(
                "line %d: no `on:` line, so there is nothing saying which line "
                "this mutation is of: %s: %s"
                % (entry["line"], entry["path"], entry["what"])
            )

    return [e for e in entries if e["source"] is not None], problems


def key(path, source, what):
    """What identifies a mutant across code moving around it."""
    return (path.replace(os.sep, "/"), source.strip(), what.strip())


def source_line(root, path, number):
    """The text of one line of one file, or None if it is not there."""
    full = os.path.join(root, path.replace("/", os.sep))
    if not os.path.isfile(full):
        return None
    with open(full, "r", encoding="utf-8", errors="replace") as handle:
        for index, line in enumerate(handle, 1):
            if index == number:
                return line.strip()
    return None


def reported(root, path):
    """The mutants a campaign says survived, keyed on the mutation.

    `missed.txt` is one mutant per line in cargo-mutants' own notation, and an
    absent file means the campaign found none, which is a result rather than an
    error: a run over a file with no surviving mutants writes nothing.

    The line number in it is used once, to read the line the mutation is of,
    and then plays no part: the key is the mutation.
    """
    found = []
    problems = []
    if not os.path.isfile(path):
        return found, problems
    with open(path, "r", encoding="utf-8") as handle:
        for raw in handle:
            raw = raw.strip()
            if not raw:
                continue
            match = REPORTED.match(raw)
            if not match:
                problems.append("%s is not a line cargo-mutants writes" % raw)
                continue
            source = source_line(root, match.group("path"), int(match.group("line")))
            if source is None:
                problems.append(
                    "%s names a line that is not in the tree being checked, so "
                    "the campaign and the working tree are not the same commit"
                    % raw
                )
                continue
            found.append(
                {
                    "key": key(match.group("path"), source, match.group("what")),
                    "reported": raw,
                    "source": source,
                }
            )
    return found, problems


def campaigned(path):
    """The files `.github/workflows/mutants.yml` changes a line at a time.

    The list has to answer for every one of them or a weekly run fails on a
    survivor in a file nobody has ever argued about, which is F-227. An entry
    for a file the campaign does not run is the same drift in the other
    direction, and this is what notices it.
    """
    if not os.path.isfile(path):
        return None
    with open(path, "r", encoding="utf-8") as handle:
        return set(re.findall(r"--file\s+(\S+)", handle.read()))


def lint(entries, root, workflow):
    """Check the list itself, without needing a campaign.

    Two things can be established from the tree alone, and `tools/verify.py`
    can afford both on every push.

    The first is that each entry is still about a line that exists. This is
    what replaces the old check that a line number was inside the file: a
    number in range said almost nothing, and passed happily while pointing at a
    closing brace. The `on:` text either is in the file or it is not.

    The second is that the file is one the campaign runs. A list and a campaign
    that name different files is the shape of F-227, where four of ten files
    were covered and the weekly run could not pass.
    """
    problems = []
    lines = {}
    for entry in entries:
        path = entry["path"]
        if path not in lines:
            full = os.path.join(root, path.replace("/", os.sep))
            if not os.path.isfile(full):
                lines[path] = None
            else:
                with open(full, "r", encoding="utf-8", errors="replace") as handle:
                    lines[path] = set(line.strip() for line in handle)
        if lines[path] is None:
            problems.append("%s: no such file" % path)
        elif entry["source"] not in lines[path]:
            problems.append(
                "%s: no line reads `%s` any more, so the argument above this "
                "entry is about code that has changed" % (path, entry["source"])
            )

    runs = campaigned(workflow)
    if runs is None:
        problems.append(
            "%s is missing, so there is no saying which files the campaign runs"
            % os.path.relpath(workflow, root)
        )
    else:
        for path in sorted(set(e["path"] for e in entries)):
            if path not in runs:
                problems.append(
                    "%s: the campaign does not run this file, so nothing here "
                    "is ever compared against it" % path
                )
    return problems


def compare(entries, found):
    """Name every mutant the list and the campaign disagree about."""
    problems = []

    want = collections.Counter(
        key(e["path"], e["source"], e["what"]) for e in entries
    )
    have = collections.Counter(f["key"] for f in found)

    shown = {}
    for item in found:
        shown.setdefault(item["key"], item)

    new = have - want
    gone = want - have

    if new:
        problems.append("these mutants survived and are not argued for:")
        for mutant, count in sorted(new.items()):
            item = shown[mutant]
            problems.append("  %s" % item["reported"])
            problems.append("    on: %s" % item["source"])
            if count > 1 or want.get(mutant):
                problems.append(
                    "    %d of these survived and %d are argued for"
                    % (have[mutant], want.get(mutant, 0))
                )
        problems.append("")
        problems.append(
            "A surviving mutant is a claim about the tests, not about the code: "
            "the suite accepted a change to a line and would accept it in a "
            "release. Write the test that objects. Add it to "
            "tools/mutants/survivors.txt only if no input can tell the mutated "
            "code from the original, and write that argument above the entry. "
            "The two lines above are the entry's shape."
        )

    if gone:
        if problems:
            problems.append("")
        problems.append("these mutants are argued for and no longer survive:")
        for mutant, count in sorted(gone.items()):
            problems.append("  %s: %s" % (mutant[0], mutant[2]))
            problems.append("    on: %s" % mutant[1])
            if have.get(mutant):
                problems.append(
                    "    %d of these are argued for and %d survived"
                    % (want[mutant], have[mutant])
                )
        problems.append("")
        problems.append(
            "A test now kills them, so the argument above each in "
            "tools/mutants/survivors.txt is no longer true. Take the entry out "
            "with its argument. A list that over-claims is how a real survivor "
            "hides in it. If instead the line itself was edited, the entry is "
            "about code that no longer exists: re-read the argument before "
            "writing the new line into it."
        )

    return problems


def self_test():
    """Prove each check fires, against lists and campaigns built to break them."""
    import shutil
    import tempfile

    SOURCE = [
        "fn pack(a: u32, b: u32) -> u32 {",
        "    let n = (a << 8) | b;",
        "    n",
        "}",
        "",
        "fn wide(m: u32, p: u32) -> bool {",
        "    if p > MAX_P {",
        "        return false;",
        "    }",
        "    if m > MAX_M {",
        "        return false;",
        "    }",
        "    true",
        "}",
    ]
    PACK = 2
    P_COST = 7
    M_COST = 10

    def build(where, source=None, entries=None, missed=None, files=None):
        os.makedirs(os.path.join(where, "src"), exist_ok=True)
        with open(os.path.join(where, "src", "thing.rs"), "w", encoding="utf-8") as h:
            h.write("\n".join(SOURCE if source is None else source) + "\n")
        with open(os.path.join(where, "survivors.txt"), "w", encoding="utf-8") as h:
            h.write("# a list\n" + (entries or ""))
        os.makedirs(os.path.join(where, ".github", "workflows"), exist_ok=True)
        with open(os.path.join(where, "mutants.yml"), "w", encoding="utf-8") as h:
            for name in files if files is not None else ["src/thing.rs"]:
                h.write("            --file %s \\\n" % name)
        with open(os.path.join(where, "missed.txt"), "w", encoding="utf-8") as h:
            h.write(missed or "")

    PAIR = (
        "src/thing.rs: replace | with ^ in pack\n"
        "  on: let n = (a << 8) | b;\n"
        "src/thing.rs: replace | with ^ in pack\n"
        "  on: let n = (a << 8) | b;\n"
    )
    LANES = (
        "src/thing.rs: replace > with == in wide\n"
        "  on: if p > MAX_P {\n"
    )

    def run(where, mode):
        entries, problems = listed(os.path.join(where, "survivors.txt"))
        if problems:
            return problems
        if mode == "lint":
            return lint(entries, where, os.path.join(where, "mutants.yml"))
        found, problems = reported(where, os.path.join(where, "missed.txt"))
        return problems + compare(entries, found)

    cases = []

    # The defect this design exists to stop. Ten lines of comment above the
    # mutated line move it, and nothing about the mutant has changed.
    def moved(where):
        shifted = ["// a paragraph"] * 10 + SOURCE
        build(
            where,
            source=shifted,
            entries=LANES,
            missed="src/thing.rs:%d:10: replace > with == in wide\n" % (P_COST + 10),
        )
    cases.append(("a survivor ten lines further down the file", moved, None, "full"))

    # A survivor in the campaign that the list says nothing about.
    def unargued(where):
        build(
            where,
            entries=LANES,
            missed=(
                "src/thing.rs:%d:10: replace > with == in wide\n"
                "src/thing.rs:%d:5: replace pack -> u32 with 0\n" % (P_COST, PACK)
            ),
        )
    cases.append(
        ("a survivor nobody has argued for", unargued, "are not argued for", "full")
    )

    # An entry the tests now kill.
    def killed(where):
        build(where, entries=LANES, missed="")
    cases.append(
        ("an argument for a mutant that no longer survives", killed,
         "no longer survive", "full")
    )

    # Two mutants on one line, one of them killed. A key alone matches; the
    # count is what notices.
    def half(where):
        build(
            where,
            entries=PAIR,
            missed="src/thing.rs:%d:20: replace | with ^ in pack\n" % PACK,
        )
    cases.append(
        ("one of a pair on the same line killed", half, "no longer survive", "full")
    )

    # Two mutations of the same function that cargo-mutants describes
    # identically, told apart by the line each is of.
    def sibling(where):
        build(
            where,
            entries=LANES,
            missed="src/thing.rs:%d:10: replace > with == in wide\n" % M_COST,
        )
    cases.append(
        ("the same mutation of a sibling line", sibling, "are not argued for", "full")
    )

    # The mutated line edited away under an argument that still claims it.
    def edited(where):
        changed = list(SOURCE)
        changed[P_COST - 1] = "    if p >= MAX_P {"
        build(where, source=changed, entries=LANES)
    cases.append(
        ("an argument about a line that has been edited", edited,
         "no line reads", "lint")
    )

    # An entry for a file the weekly campaign does not run, which is F-227's
    # shape from the other side.
    def uncampaigned(where):
        build(where, entries=LANES, files=["src/other.rs"])
    cases.append(
        ("an entry for a file the campaign does not run", uncampaigned,
         "does not run this file", "lint")
    )

    # An entry with nothing saying which line it is of.
    def anchorless(where):
        build(where, entries="src/thing.rs: replace > with == in wide\n")
    cases.append(
        ("an entry with no `on:` line", anchorless, "no `on:` line", "lint")
    )

    # An entry left in cargo-mutants' own notation, line number and all.
    def unconverted(where):
        build(
            where,
            entries="src/thing.rs:%d:10: replace > with == in wide\n" % P_COST,
        )
    cases.append(
        ("an entry still carrying a line number", unconverted,
         "cargo-mutants' `path:line:column` form", "lint")
    )

    failures = 0
    for name, prepare, expected, mode in cases:
        where = tempfile.mkdtemp()
        try:
            prepare(where)
            found = run(where, mode)
            text = "\n".join(found)
            if expected is None:
                if found:
                    failures += 1
                    print("    OBJECTED TO: %s" % name)
                    print("\n".join("      " + line for line in found))
                else:
                    print("    accepted: %s" % name)
            elif expected in text:
                print("    caught: %s" % name)
            else:
                failures += 1
                print("    MISSED: %s" % name)
                print("      expected %r in:" % expected)
                print("\n".join("      " + line for line in found) or "      (clean)")
        finally:
            shutil.rmtree(where, ignore_errors=True)

    if failures:
        print("  %d of the checks did not do what it says they do" % failures)
        return 1
    print("  %d checks, every one of them proved against a planted fault"
          % len(cases))
    return 0


def main(argv):
    if len(argv) == 2 and argv[1] == "--self-test":
        return self_test()

    if len(argv) != 2 or (argv[1] != "--lint" and argv[1].startswith("--")):
        print(
            "usage: check.py <path to mutants.out/missed.txt>\n"
            "       check.py --lint\n"
            "       check.py --self-test",
            file=sys.stderr,
        )
        return 2

    entries, problems = listed(SURVIVORS)
    if problems:
        print("tools/mutants/survivors.txt does not parse:")
        for problem in problems:
            print("  %s" % problem)
        print()
        print(
            "Every entry is `<path>: <what the mutation does>` followed by an "
            "indented `on:` line carrying the source line it is of. The pair "
            "is the key a campaign is compared against."
        )
        return 1

    if argv[1] == "--lint":
        problems = lint(entries, ROOT, WORKFLOW)
        if problems:
            print("these entries in tools/mutants/survivors.txt are not about "
                  "the code any more:")
            for problem in problems:
                print("  %s" % problem)
            print()
            print(
                "An entry names the line it is of rather than a line number, so "
                "this fires when that line is edited or its file leaves the "
                "campaign, not when code above it moves. Re-read the argument, "
                "then write the new line into the entry, or take the entry out "
                "if the mutation is now killable."
            )
            return 1
        print(
            "  %d argued-for survivors, every one about a line that is still "
            "there" % len(entries)
        )
        return 0

    found, problems = reported(ROOT, argv[1])
    problems += compare(entries, found)
    if problems:
        for problem in problems:
            print(problem)
        return 1

    print(
        "  %d surviving mutant%s, every one of them argued for"
        % (len(found), "" if len(found) == 1 else "s")
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
