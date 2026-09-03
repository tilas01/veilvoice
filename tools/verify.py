#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Regenerate everything generated, then run every check, in the right order.

    python tools/verify.py            # regenerate, then check
    python tools/verify.py --check    # check only, exactly as CI does

# Why this exists

Four generators write into this tree, and three of them read files the others
write. Run them in the wrong order, or edit a source file after running them,
and the committed output is one change behind -- which is not a visible fault.
It is a green local run and a red CI run ten minutes later.

That happened twice in one afternoon, both times the same way: edit a source
file, regenerate, edit another source file, commit. The search index is built
from *every tracked file*, so it goes stale when anything at all changes after
it is written.

The order below is the dependency order, and it is the whole point of the file:

  1. `assets/generate.py`      -- artwork, from nothing but itself
  2. `tools/docs/generate.py`  -- reads the Rust doc comments, writes 371 files
  3. `tools/search-index/generate.py` -- reads *everything*, so it goes last
  4. the checks, which must all see the same tree

Anything that regenerates has to be staged before the index runs, because the
index walks `git ls-files` and a file git has never heard of is not in it.
"""

import os
import subprocess
import sys


def repo_root():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(here, ".."))


def run(root, label, command, capture=True):
    """Run one step. Returns (ok, output).

    Cargo steps are run with `RUSTFLAGS=-D warnings`, because that is what CI
    sets and a local check that does not match CI is worse than no local check:
    it passes, and then the push fails ten minutes later for something that was
    on screen the whole time. Three CI failures in one session came through
    exactly that gap -- an unused `mut`, a `needless_return`, a dead enum
    variant -- each a warning locally and an error there.
    """
    environment = dict(os.environ)
    if command and str(command[0]).startswith("cargo"):
        existing = environment.get("RUSTFLAGS", "")
        if "-D warnings" not in existing:
            environment["RUSTFLAGS"] = (existing + " -D warnings").strip()
    try:
        result = subprocess.run(
            command, cwd=root, shell=isinstance(command, str), env=environment,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.STDOUT if capture else None,
        )
    except OSError as error:
        return False, "%s: could not run (%s)" % (label, error)
    output = (result.stdout or b"").decode("utf-8", "replace")
    return result.returncode == 0, output


def stage(root):
    """Stage everything, so the index walk sees newly written files.

    `git ls-files` lists *tracked* files. A generator that has just written a
    new page leaves it untracked, so the index built immediately afterwards
    does not contain it -- and CI, regenerating from the committed tree, finds
    one more file and fails with a message about drift that says nothing about
    why.
    """
    subprocess.run(["git", "add", "-A"], cwd=root,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


GENERATORS = [
    # First. It depends on nothing that is generated -- it runs the test suite
    # and reads Cargo.toml -- and everything after it may quote what it
    # measures. It was written in last, which put it after the search index:
    # the index walked docs/ before this file had been rewritten and then
    # disagreed with it, on the very first run.
    ("measured numbers", [sys.executable, "tools/measured/generate.py"]),
    ("artwork", [sys.executable, "assets/generate.py"]),
    # Drawn from the command output committed beside them. `--capture`, which
    # actually runs `veilvoice`, is a separate and manual step: it needs a
    # build, a machine and a person deciding the output is right. Everything
    # after that is a pure function of those text files, which is why this half
    # can be regenerated and checked here.
    # Before the drawings, which mirror the window captures into the website
    # and record their sizes: a crop after that copy would leave the two out of
    # step, and the size check in `terminal.py --check` is what would notice.
    ("screenshot borders", [sys.executable, "tools/shots/crop.py"]),
    # After the border comes off and before the corners go on. A capture is
    # taken taller than any tab needs, so that the longest one is not cut in
    # half, and this trims each picture back to what it actually contains.
    # Rounding first would round the corners of a picture that is about to
    # lose its bottom half.
    ("screenshot height", [sys.executable, "tools/shots/fit.py"]),
    # After the crop and the fit, never before them: rounding the corners of a
    # picture that still has a capture border rounds the border.
    ("screenshot corners", [sys.executable, "tools/shots/round.py"]),
    # After everything that can change a picture's size, because it copies
    # those sizes into the pages that show them.
    ("screenshot sizes on the pages", [sys.executable, "tools/shots/attrs.py"]),
    ("terminal drawings", [sys.executable, "tools/shots/terminal.py"]),
    ("documentation", [sys.executable, "tools/docs/generate.py"]),
    # After the documentation generator, which owns the wiki directory:
    # these are per-program views of the user guide and land in it too.
    ("per-program guides", [sys.executable, "tools/docs/guides.py"]),
    # The website's own source, which `generate.py` does not cover: it reads
    # Rust doc comments, and these are JavaScript and CSS. Imports the same
    # module for the palette and the drawing code, so it goes after it.
    ("website source pages", [sys.executable, "tools/docs/sources.py"]),
    # Derived from website/index.html, so it must run after anything that could
    # edit that file and before the index walks the result.
    ("section pages", [sys.executable, "tools/site/split.py"]),
    # After the split, because it borrows that tool's header, navigation and
    # footer from index.html, and before the index, which walks the result.
    ("roadmap page", [sys.executable, "tools/site/roadmap.py"]),
    # Before the source pages walk website/js, since this writes one of them.
    ("demonstration data", [sys.executable, "tools/site/demo.py"]),
    ("questions page", [sys.executable, "tools/site/faq.py"]),
    # After the split, whose header this borrows, and before the index walks it.
    ("releases page", [sys.executable, "tools/site/releases.py"]),
    ("search index", [sys.executable, "tools/search-index/generate.py"]),
]

CHECKS = [
    # First, because a release whose README tells people to download the
    # previous version is a failure nothing else here would notice: the
    # command works, the verifier passes, and the reader gets the wrong
    # program.
    ("every copy of the version agrees with Cargo.toml",
     [sys.executable, "tools/release/version.py", "--check"]),
    ("every package installs what the workspace builds",
     [sys.executable, "tools/release/packaging.py"]),
    ("no state file is written one place and read another",
     [sys.executable, "tools/audit/state_paths.py"]),
    ("artwork matches its generator", [sys.executable, "assets/generate.py", "--check"]),
    ("no screenshot has a capture border",
     [sys.executable, "tools/shots/crop.py", "--check"]),
    ("no screenshot has empty space below its content",
     [sys.executable, "tools/shots/fit.py", "--check"]),
    ("screenshots have rounded corners",
     [sys.executable, "tools/shots/round.py", "--check"]),
    ("every screenshot tag matches its file",
     [sys.executable, "tools/shots/attrs.py", "--check"]),
    ("terminal drawings match their output",
     [sys.executable, "tools/shots/terminal.py", "--check"]),
    # The recorded sessions, re-run and compared. This is the check that would
    # have caught the demonstration inventing the verifier's output, and it is
    # the only one here that runs the programs rather than reading about them.
    ("recorded sessions match the programs",
     [sys.executable, "tools/shots/sessions.py", "--check"]),
    ("documentation matches the source", [sys.executable, "tools/docs/generate.py", "--check"]),
    ("per-program guides match the user guide",
     [sys.executable, "tools/docs/guides.py", "--check"]),
    ("website source pages match their files",
     [sys.executable, "tools/docs/sources.py", "--check"]),
    ("section pages match index.html", [sys.executable, "tools/site/split.py", "--check"]),
    ("the roadmap page matches ROADMAP.md",
     [sys.executable, "tools/site/roadmap.py", "--check"]),
    ("the demonstration matches the source",
     [sys.executable, "tools/site/demo.py", "--check"]),
    ("the questions page matches docs/FAQ.md",
     [sys.executable, "tools/site/faq.py", "--check"]),
    ("the releases page matches CHANGELOG.md",
     [sys.executable, "tools/site/releases.py", "--check"]),
    ("search index matches the tree", [sys.executable, "tools/search-index/generate.py", "--check"]),
    ("measured numbers match the tree",
     [sys.executable, "tools/measured/generate.py", "--check"]),
    ("website suites", ["node", "tools/site-tests/run.js"]),
    ("the local site serves every page", [sys.executable, "tools/site/serve.py", "--check"]),
]

CARGO = [
    ("formatting", ["cargo", "fmt", "--all", "--check"]),
    ("clippy", ["cargo", "clippy", "--workspace", "--all-targets"]),
    ("tests", ["cargo", "test", "--workspace"]),
]


def main():
    root = repo_root()
    check_only = "--check" in sys.argv
    failed = []

    if not check_only:
        print("regenerating, in dependency order")
        for label, command in GENERATORS:
            stage(root)   # so the walk sees anything written by the step before
            ok, output = run(root, label, command)
            print("  %-16s %s" % (label, "ok" if ok else "FAILED"))
            if not ok:
                print(output)
                failed.append(label)
        stage(root)
        print()

    print("checking")
    for label, command in CHECKS + CARGO:
        ok, output = run(root, label, command)
        print("  %-34s %s" % (label, "ok" if ok else "FAILED"))
        if not ok:
            failed.append(label)
            for line in output.strip().splitlines()[-25:]:
                print("      " + line)

    print()
    if failed:
        print("%d check(s) failed: %s" % (len(failed), ", ".join(failed)))
        return 1
    print("everything regenerated and every check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
