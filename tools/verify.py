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
    ("artwork", [sys.executable, "assets/generate.py"]),
    ("documentation", [sys.executable, "tools/docs/generate.py"]),
    # Derived from website/index.html, so it must run after anything that could
    # edit that file and before the index walks the result.
    ("section pages", [sys.executable, "tools/site/split.py"]),
    ("search index", [sys.executable, "tools/search-index/generate.py"]),
]

CHECKS = [
    ("artwork matches its generator", [sys.executable, "assets/generate.py", "--check"]),
    ("documentation matches the source", [sys.executable, "tools/docs/generate.py", "--check"]),
    ("section pages match index.html", [sys.executable, "tools/site/split.py", "--check"]),
    ("search index matches the tree", [sys.executable, "tools/search-index/generate.py", "--check"]),
    ("website suites", ["node", "tools/site-tests/run.js"]),
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
