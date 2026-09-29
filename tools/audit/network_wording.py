#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""No sentence a reader sees may claim more about the network than is true.

    python tools/audit/network_wording.py            # report, and fail on any
    python tools/audit/network_wording.py --self-test

No `--check`, for the reason the other tools here have none: this only reads,
so reporting and checking are one run.

# What this is for

The front page's claim is that there is **no HTTP client anywhere in the
dependency graph**, that a reader can check that in ten seconds with `cargo
tree`, and that CI fails the build if `reqwest`, `hyper`, `curl`, `ureq`,
`tungstenite`, `isahc` or `surf` ever appears. That claim is true, it is
enforced by a job in `ci.yml`, and it is a large part of why this project is
worth trusting.

Three other sentences say something that sounds the same and is not: that
VeilVoice reaches the network never, at all, under any circumstances. Those are
false. `veilvoice verify release` has downloaded three files from `github.com`
since v0.1.11, `veilvoice update` fetches and applies a release, and the
desktop application has a check-for-updates button. Every one of those is
something a person asked for by name, which is the property actually worth
having; none of them is an HTTP client in the graph.

That is F-213, and the reason it needs a guard rather than an edit is F-256.
F-213 corrected the two sentences its own write-up quoted, `veilvoice --help`
and the install message, and left four copies of the same claim standing: the
crash log a person reads after a crash, the crash-report panel shown on the
next launch, and `veilvoice-verify`'s own module documentation, which denied
the download that crate performs and was published to the website through the
generated reference. A sentence in four places is not fixed by correcting the
two somebody had open.

**And the shape is worth naming, because this is the fourth instance of it.**
F-210 and F-212 were guards that read the file they lived in; this crate's
`SCOPE` guard is a third, and it is the closest relative: it refuses the words
"prevent", "unbreakable", "guarantee" and "protects against", and it refuses
them in exactly one string, its own. The vocabulary rule was right and its
reach was one constant. This asks the same question of everything a reader
sees.

# What is checked, and where

Each rule is a phrase that is false on its own and true with a qualifier, so
the qualifier is what is looked for rather than the phrase being banned
outright. The window is 80 characters after the match, because these sentences
are written to wrap and the qualifier is usually on the next line.

Whitespace is flattened before matching, with Rust's `//!` and `//` markers and
a string literal's trailing backslash removed, so a claim split across three
lines reads as one sentence. It is the sentence a person sees, not the source
line, that is being checked.

Read: the workspace's Rust sources, the hand-written documents, `README.md`,
and the hand-written pages of the website.

Not read, and each for a reason rather than for convenience:

  * **Generated output.** `docs/files/`, `website/reference/`, `website/wiki/`,
    `website/nojs/`, `wiki/` and the crate `README.md` files carry their
    source's words, so a claim in one of them is a claim in the source, where
    it is read and where the fix belongs. Reporting both would name the same
    sentence twice and send a reader to the copy they must not edit.
  * **The documents that record what was said.** `docs/AUDIT.md`,
    `CHANGELOG.md`, `ROADMAP.md` and `website/releases.html` quote these
    sentences in order to say they were wrong and were changed. A guard that
    fires on the write-up of a defect is a guard that makes recording defects
    expensive.
  * **`tools/`.** The comments here argue about the claims rather than making
    them, and nobody installs VeilVoice and reads them.

Pure standard library, like everything in `tools/`.
"""

import os
import re
import sys

#: Each rule: the phrase, and the qualifier that makes it true. A rule with no
#: qualifier is false in every form.
RULES = [
    (
        r"has\s+no\s+network\s+code",
        r"dependency graph",
        "VeilVoice has no HTTP client in its dependency graph; it does reach "
        "the network when asked, so say which of the two is meant",
    ),
    (
        r"no\s+network\s+code\s+at\s+all",
        r"dependency graph",
        "same claim, stronger wording, same correction",
    ),
    (
        r"talks\s+to\s+no\s+servers",
        None,
        "`veilvoice verify release` and `veilvoice update` both fetch from the "
        "releases page",
    ),
    (
        r"never\s+checks\s+for\s+updates",
        r"on its own",
        "there is no automatic check, and there is a check you can ask for",
    ),
]

WINDOW = 80

SKIP_TREES = (
    "docs/files",
    "website/reference",
    "website/wiki",
    "website/nojs",
    "wiki",
    "tools",
    "target",
    ".git",
    "assets",
    "install",
)

SKIP_FILES = (
    "docs/AUDIT.md",
    "CHANGELOG.md",
    "ROADMAP.md",
    "website/releases.html",
)

READ = (".rs", ".md", ".html")


def repo_root():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.dirname(os.path.dirname(here))


def flatten(text: str) -> str:
    """One long line, as a reader would see the sentence rather than the source.

    A doc comment carries `//!` on every line and a wrapped Rust string literal
    ends each line with a backslash. Both are punctuation of the source, not of
    the sentence, so they go before the match.
    """
    text = text.replace("\\n", " ")
    text = re.sub(r"\\\s*\n\s*", " ", text)
    text = re.sub(r"\n\s*(?://!|///|//|\*)?\s*", " ", text)
    return re.sub(r"\s+", " ", text)


def offences(text: str) -> list:
    """Every claim in `text` that is not qualified, as (phrase, reason)."""
    flat = flatten(text)
    found = []
    for pattern, qualifier, reason in RULES:
        for match in re.finditer(pattern, flat, re.IGNORECASE):
            if qualifier and re.search(
                qualifier, flat[match.start(): match.end() + WINDOW], re.IGNORECASE
            ):
                continue
            found.append((match.group(0), reason))
    return found


def files(root: str):
    for base, dirs, names in os.walk(root):
        rel = os.path.relpath(base, root).replace(os.sep, "/")
        rel = "" if rel == "." else rel
        if any(rel == d or rel.startswith(d + "/") for d in SKIP_TREES):
            dirs[:] = []
            continue
        dirs[:] = [d for d in dirs if not d.startswith(".")]
        for name in sorted(names):
            path = (rel + "/" + name) if rel else name
            if path in SKIP_FILES or not name.endswith(READ):
                continue
            # A crate's README.md is written by tools/docs/generate.py from the
            # crate's own module documentation, so it is generated output that
            # happens not to live under one of the directories above.
            if name == "README.md" and rel.startswith("crates/"):
                continue
            yield path


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()

    root = repo_root()
    problems = []
    read = 0
    for path in files(root):
        try:
            with open(os.path.join(root, path), encoding="utf-8") as handle:
                text = handle.read()
        except (OSError, UnicodeDecodeError):
            continue
        read += 1
        for phrase, reason in offences(text):
            problems.append("  %s: %r -- %s" % (path, phrase, reason))

    if problems:
        print("%d unqualified network claim(s):" % len(problems))
        for line in problems:
            print(line)
        print()
        print("  Each is true of the dependency graph and false of the program.")
        print("  Say which is meant, as tools/audit/network_wording.py explains.")
        return 1
    print("  %d file(s) read, and every network claim in them is qualified" % read)
    return 0


def self_test() -> int:
    """The rules, against text each must and must not fire on.

    A guard for a form of words is only as good as its reach, and this one's
    reach is a regular expression over flattened text. Both halves are the
    part that can rot quietly.
    """
    failures = []

    def fires(label, text, expected):
        got = len(offences(text)) > 0
        if got != expected:
            failures.append(
                "%s: expected %s, got %s"
                % (label, "a complaint" if expected else "silence",
                   "a complaint" if got else "silence")
            )

    # The four sentences that shipped.
    fires("the crash log's old wording",
          "This file was written on your machine and sent nowhere. VeilVoice has\n"
          "no network code at all. Delete it whenever you like.", True)
    fires("the verifier's old module documentation",
          "//! It does not download anything -- this project has no network code and\n"
          "//! this binary is not the exception.", True)
    fires("the old --help paragraph",
          "This command line talks to no servers, ever.", True)
    fires("the old install message",
          "VeilVoice never checks for updates and cannot tell you when one exists.",
          True)

    # And the true forms, which must pass, including across a line break and
    # across a wrapped string literal, which is how all of them are written.
    fires("the qualified claim", "VeilVoice has no network code in its dependency graph.", False)
    fires("the qualified claim, wrapped",
          '"This tool does not contain an HTTP client -- VeilVoice has no network \\\n'
          '         code in its dependency graph at all, which you can check."', False)
    fires("the qualified claim in a doc comment",
          "//! refusing to be, and there is no network code in the dependency graph\n"
          "//! to do it with.", False)
    fires("no automatic check", "VeilVoice never checks for updates on its own.", False)
    fires("a test naming the phrase it asserts on",
          'assert!(text.contains("no network code"), "say why there is no client");',
          False)

    # And the reach: a file the walk must read, and one it must not.
    listed = set(files(repo_root()))
    for must in ("crates/veilvoice-gui/src/crashlog.rs", "docs/COMMANDS.md"):
        if must not in listed:
            failures.append("the walk does not read %s" % must)
    for must_not in ("docs/AUDIT.md", "CHANGELOG.md",
                     "crates/veilvoice-verify/README.md"):
        if must_not in listed:
            failures.append("the walk reads %s, which it must not" % must_not)

    for line in failures:
        print("  " + line)
    print("%d failure(s)" % len(failures) if failures else "self-test passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
