#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""The manual names every command that reaches the network.

    python tools/audit/network_claims.py
    python tools/audit/network_claims.py --self-test

# The defect this exists to stop coming back

`veilvoice --help` opens by saying that nothing here reaches the network except
when it is asked to by name, and it names the commands that do. That sentence is
the program's own claim about itself, and it is compiled in: the `offline` CI job
is what makes it true rather than asserted, by refusing a network crate in the
dependency graph.

`docs/USER_GUIDE.md` has a section explaining the same thing at length, for a
reader who wants to know how a program with no HTTP client reaches the network at
all. Two documents naming the same set of commands is two places to be wrong, and
both were:

  * **F-222.** `veilvoice update` landed, and the manual went on saying "the
    command line has no such feature at all" for a week, in the manual, in the
    three per-program guides cut from it, in the wiki and on the website.
  * **F-228.** The manual never named `veilvoice verify release` either. Running
    that command to find out what to write about it is what found F-228: asking
    it for help opened a connection.

So the second of those was found by hand, while doing the first by hand, and
neither was found by anything. This is the check.

# What it compares, and why that is the honest direction

The program's list is the authority, so this asks only one question: is every
command the program says reaches the network named in the manual's section about
reaching the network?

Not the other way round. That section legitimately mentions commands that do not
touch the network, `veilvoice info` among them, because it explains which release
stream a copy belongs to and that is where a reader is standing when they ask.
A check that insisted the two lists were equal would fail on a paragraph that is
correct, and a check that fails on correct prose gets deleted.

# Where each list comes from

The program's list is read out of `assets/screenshots/cli-help.txt`, which
`tools/shots/terminal.py` captures from the built program and compares against it
on every run. So this is reading what the program prints, at one remove, rather
than a copy of it: a sentence edited in `main.rs` without recapturing fails that
check before it reaches this one.

The manual's section is found by its heading rather than by line number, and it
ends at the next heading. A heading renamed out of recognition fails here, which
is the right outcome: this check is about a claim, and a claim with no section to
live in is a claim that has been deleted.

Inside that section a command counts as named when it opens a backticked span,
because the manual writes invocations rather than bare names: `veilvoice update
--check`, `veilvoice verify release <tag>`. Whitespace is collapsed first, so a
command that wraps across a line is still the command being named.

Pure standard library.
"""

from __future__ import annotations

import io
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

HELP_REL = "assets/screenshots/cli-help.txt"

# Every document that makes this claim to a reader, and how much of it the claim
# covers. "section" means the whole section under the heading that contains
# SUBJECT, which is the manual's long explanation. "paragraph" means the one
# paragraph that makes the claim, which is how a page whose subject is something
# else carries it. The difference matters: docs/COMMANDS.md is a table of every
# command in backticks, so a whole-page search would find every name in it and
# report that everything was fine.
CLAIMANTS = [
    ("docs/USER_GUIDE.md", "section"),
    ("docs/COMMANDS.md", "paragraph"),
]

# The sentence in the program's own description, and the heading in the manual.
# Both are matched on the same words, because both are about the same thing and
# a check that looked for two different phrases could pass while they drifted.
SUBJECT = "reaches the network"

# `veilvoice update`, `veilvoice verify release`. Backticked, because that is how
# both documents write a command, and bounded to lower-case words so a sentence
# reading "`veilvoice update` and" cannot swallow what follows.
COMMAND = re.compile(r"`(veilvoice(?: [a-z][a-z-]*)+)`")


def read(rel):
    with io.open(os.path.join(ROOT, rel), encoding="utf-8") as handle:
        return handle.read()


def paragraph(text):
    """The blank-line-delimited block containing the claim, or None.

    The claim lives in the program's opening description, and the rest of the
    help output is a table of every command there is. Narrowing to the block
    first means the splitting below never has to be right about that table.
    """
    for block in re.split(r"\n\s*\n", text):
        if SUBJECT in block:
            return flat(block)
    return None


def claimed(help_text):
    """The commands the program says reach the network, in the order named.

    Sentences are split on full stops and not on colons, because the claim is
    written with one: "except when you ask it to by name: `veilvoice update`
    and `veilvoice verify release` fetch from the releases page". Splitting on
    the colon too cuts the sentence off immediately before the only part of it
    this check is looking for, and reports a program that makes no claim. That
    is how the first draft of this file behaved, and its own self-test is what
    said so.
    """
    block = paragraph(help_text)
    if block is None:
        return []
    found = []
    for sentence in re.split(r"(?<=\.)\s+", block):
        if SUBJECT not in sentence:
            continue
        for name in COMMAND.findall(sentence):
            if name not in found:
                found.append(name)
    return found


def section(guide_text):
    """The manual's section about reaching the network, or None.

    Ends at the next heading of the same depth or shallower, so a subsection
    inside it is part of it.
    """
    lines = guide_text.split("\n")
    start = None
    depth = 0
    for at, line in enumerate(lines):
        if line.startswith("#") and SUBJECT in line:
            start = at
            depth = len(line) - len(line.lstrip("#"))
            break
    if start is None:
        return None
    for at in range(start + 1, len(lines)):
        line = lines[at]
        if not line.startswith("#"):
            continue
        here = len(line) - len(line.lstrip("#"))
        if here <= depth:
            return "\n".join(lines[start:at])
    return "\n".join(lines[start:])


def flat(text):
    """`text` with every run of whitespace collapsed to one space.

    Prose wraps, and a command wrapped across a line break is still the command
    being named. Matching the raw text would quietly miss one.
    """
    return re.sub(r"\s+", " ", text)


def names(text, command):
    """Whether `text` shows `command` being run.

    The manual writes an invocation, not a bare name: `veilvoice update
    --check`, `veilvoice verify release <tag>`. So the command counts as named
    when it opens a backticked span, whatever flags or arguments follow it. The
    lookahead is what stops `veilvoice update` being satisfied by a longer
    subcommand that merely starts with the same letters.
    """
    return re.search("`" + re.escape(command) + "(?![a-z-])", flat(text)) is not None


def scope(text, how, rel):
    """The part of `text` the claim covers, or a sentence saying it is gone."""
    if how == "paragraph":
        body = paragraph(text)
        if body is None:
            return None, ("%s has no paragraph about what %s. That page told a "
                          "reader nothing reached the network at all, so its "
                          "losing the claim entirely is a change to look at "
                          "rather than to pass." % (rel, SUBJECT))
        return body, None
    body = section(text)
    if body is None:
        return None, ("%s has no heading about what %s. It explained this at "
                      "length and the section is gone, which is a bigger "
                      "change than this check can decide about."
                      % (rel, SUBJECT))
    return body, None


def problems(help_text, claimants):
    """`claimants` maps a relative path to its text."""
    found = []
    commands = claimed(help_text)
    if not commands:
        return ["%s has no sentence about what %s, so there is nothing to hold "
                "these pages to. If that sentence has been reworded, reword "
                "this check with it." % (HELP_REL, SUBJECT)]

    for rel, how in CLAIMANTS:
        body, gone = scope(claimants[rel], how, rel)
        if gone:
            found.append(gone)
            continue
        for name in commands:
            if not names(body, name):
                found.append(
                    "the program says `%s` reaches the network, and %s does not "
                    "name it where it makes that claim" % (name, rel))
    return found


def main():
    if "--self-test" in sys.argv:
        return self_test()

    claimants = dict((rel, read(rel)) for rel, _ in CLAIMANTS)
    found = problems(read(HELP_REL), claimants)
    if found:
        print("  the manual and the program disagree about what reaches the "
              "network:")
        for line in found:
            print("    %s" % line)
        print()
        print("    Name it where the claim is made, and say what it does")
        print("    there. A command that reaches the network and is not")
        print("    explained where a reader looks for the explanation is the")
        print("    one thing this project cannot leave to somebody's `--help`.")
        print("    docs/COMMANDS.md is generated: fix tools/docs/commands.py.")
        return 1

    commands = claimed(read(HELP_REL))
    print("  %d command(s) reach the network, and all %d page(s) that say so "
          "name each: %s"
          % (len(commands), len(CLAIMANTS), ", ".join(commands)))
    return 0


# ------------------------------------------------------------- the self-test


HELP_SAMPLE = (
    "VeilVoice destroys the biometric voiceprint of a speaker. Nothing here "
    "reaches the network except when you ask it to by name: `veilvoice update` "
    "and `veilvoice verify release` fetch from the releases page, and no other "
    "command opens a connection for any reason.\n"
)

GUIDE_SAMPLE = """## 2. Something else

Text that mentions `veilvoice anonymise` and nothing about connections.

### How anything reaches the network, given that nothing here is a network client

`veilvoice update --check` looks for a newer version. `veilvoice verify release
<tag>` fetches a release to check. `veilvoice info` says which stream this is,
and reaches nothing.

## 3. The next thing

`veilvoice update` is mentioned here too, outside the section.
"""


COMMANDS_SAMPLE = """# Every command, and the same job in the window

VeilVoice is two programs over one engine.

**Nothing here reaches the network except two commands, and only when you ask
for them by name.** `veilvoice update` and `veilvoice verify release` are the
two, and nothing else opens a connection for any reason.

## The whole list

| Command | What it does |
|---|---|
| `veilvoice anonymise` | De-identify an audio file |
| `veilvoice update` | Fetch the newest release |
| `veilvoice verify release` | Check a published release |
"""


def pages(**changes):
    files = {"docs/USER_GUIDE.md": GUIDE_SAMPLE,
             "docs/COMMANDS.md": COMMANDS_SAMPLE}
    files.update(changes)
    return files


def self_test():
    failures = []

    def expect(what, got, want):
        if got != want:
            failures.append("%s: got %r, wanted %r" % (what, got, want))

    expect("both commands are read out of the program's own sentence",
           claimed(HELP_SAMPLE),
           ["veilvoice update", "veilvoice verify release"])

    # The claim is written with a colon in the middle of it. A splitter that
    # treats the colon as the end of the sentence finds nothing, and says the
    # program makes no claim at all. That was the first draft of this file.
    expect("the colon in the middle of the claim does not cut it short",
           claimed("Nothing here reaches the network except when you ask it to "
                   "by name: `veilvoice update` fetches from the releases "
                   "page.\n"),
           ["veilvoice update"])

    # Only the sentence making the claim counts. The next one is about a
    # command that opens nothing, and the table below names every command there
    # is, which is why the block is narrowed before anything is split.
    expect("a neighbouring sentence is not part of the claim",
           claimed(HELP_SAMPLE +
                   " `veilvoice info` says which stream this is.\n"
                   "\nCommands:\n  anonymise  De-identify an audio file.\n"),
           ["veilvoice update", "veilvoice verify release"])

    expect("a sound set has nothing to report",
           problems(HELP_SAMPLE, pages()), [])

    # The F-222 shape: the manual names one of them and not the other.
    without = GUIDE_SAMPLE.replace("`veilvoice verify release\n<tag>`", "it")
    found = problems(HELP_SAMPLE, pages(**{"docs/USER_GUIDE.md": without}))
    expect("a command missing from the section is reported", len(found), 1)
    if found and "veilvoice verify release" not in found[0]:
        failures.append("and it must name the command: %r" % found[0])

    # A mention outside the section does not count, which is the whole point of
    # finding the section rather than searching the file.
    only_elsewhere = GUIDE_SAMPLE.replace(
        "`veilvoice update --check` looks for a newer version. ", "")
    found = problems(HELP_SAMPLE,
                     pages(**{"docs/USER_GUIDE.md": only_elsewhere}))
    expect("a mention outside the section does not satisfy it", len(found), 1)
    if found and "veilvoice update" not in found[0]:
        failures.append("and it must name that one: %r" % found[0])

    # The section ends at the next heading of the same depth.
    body = section(GUIDE_SAMPLE)
    if body is None or "The next thing" in body:
        failures.append("the section must end at the next heading of its depth")
    if body is None or "Something else" in body:
        failures.append("and must not start before its own heading")

    expect("a guide with no such section is reported rather than passed",
           len(problems(HELP_SAMPLE, pages(
               **{"docs/USER_GUIDE.md": "## Nothing relevant\n\nwords\n"}))), 1)
    expect("and so is a program that no longer makes the claim",
           len(problems("Some other description entirely.\n", pages())), 1)

    # The F-235 shape, and the reason this reads a paragraph rather than a page:
    # docs/COMMANDS.md is a table of every command, so a page-wide search finds
    # every name in it and reports that nothing is wrong.
    found = problems(HELP_SAMPLE, pages(**{"docs/COMMANDS.md": COMMANDS_SAMPLE.replace(
        "`veilvoice update` and `veilvoice verify release` are the\ntwo",
        "the window's check-for-updates button is the one, and it is not a\ncommand")}))
    expect("a claim beside a table of every command is read as a paragraph",
           len(found), 2)
    if found and "docs/COMMANDS.md" not in found[0]:
        failures.append("and names the page: %r" % found[0])

    expect("a page that loses the claim entirely is reported",
           len(problems(HELP_SAMPLE, pages(
               **{"docs/COMMANDS.md": "# Every command\n\nA table.\n"}))), 1)

    # The manual writes invocations. Each of these names `veilvoice update`.
    for how in ("`veilvoice update`", "`veilvoice update --check`",
                "`veilvoice update` and `veilvoice info`",
                "wrapped across\na line as `veilvoice\nupdate --check` is"):
        if not names("### reaches the network\n\n" + how, "veilvoice update"):
            failures.append("an invocation must count as naming it: %r" % how)

    # And none of these does, which is what the lookahead is for.
    for how in ("`veilvoice updates`", "`veilvoice update-all`",
                "the update command, in prose with no backticks"):
        if names("### reaches the network\n\n" + how, "veilvoice update"):
            failures.append("but this must not count: %r" % how)

    # A command name must not run past its backtick into the next word.
    expect("the pattern stops at the closing backtick",
           claimed("Nothing here reaches the network except `veilvoice update` "
                   "and nothing else at all.\n"),
           ["veilvoice update"])

    if failures:
        print("  %d self-test(s) failed: this guard does not catch what it "
              "claims to" % len(failures))
        for line in failures:
            print("    %s" % line)
        return 1
    print("  the network-claim check catches every case it claims to")
    return 0


if __name__ == "__main__":
    sys.exit(main())
