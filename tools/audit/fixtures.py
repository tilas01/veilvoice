#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
A file the build needs is in the commit, not merely on the machine that wrote it.

# The defect this exists to stop coming back

`crates/veilvoice-verify/src/check/testdata/not-our-key.asc` was written,
referenced by an `include_str!`, checked, and pushed without ever being added
to the repository. `.gitignore` excludes `*.asc` so that a private key can
never be committed by accident, `git add -A` skips an excluded path in
silence, and `git status` does not list a file git has never heard of. So
every check passed on the machine that wrote it, and the crate did not compile
anywhere else. That took `cargo test --workspace`, clippy, and three of
`tools/verify.py`'s checks down with it, on every other clone at once.

The reason nothing here could have caught it is worth stating, because it
applies to every other check in this repository: **they all read the working
tree, and the working tree is not what anybody else gets.** A file that is
present and untracked reads exactly like a file that is present and committed.
The only way to tell them apart is to ask git, which is what this does.

The same file carried a second defect of the same shape. Four of its six
negation lines had never un-ignored anything:

    !*.pub                 nothing tracked here is a `.pub` file, and nothing
                           will be: they are a recipient's keys, not ours
    !*public*.asc          a guess at what the line below does exactly
    !**/public-key.asc     another one, and one that could never work for the
                           folder it was aimed at: a negation cannot re-include
                           a file inside an excluded directory
    !assets/** # comment   `.gitignore` has no inline comments, so the pattern
                           is the whole line, `#` and all, and it has never
                           matched a path

A rule that matches nothing is indistinguishable from a rule that works, right
up to the moment somebody relies on it. That is F-204.

# What is checked

**Every path an `include_str!` or an `include_bytes!` names is tracked.** Not
present: tracked. These are read at compile time, so a missing one is not a
failing test, it is a crate that does not build.

**`.gitignore` says what it means.** No pattern line may carry a `#`, because
git has no inline comments and will take it as part of the pattern. Every
negation must be the deciding rule for at least one tracked file, because a
negation nothing has ever exercised is a negation nobody has tested, and the
first file to need it is the worst moment to find out.

**`assets/` and every `testdata/` hold nothing git will not carry.** Those
directories exist for committed material and for nothing else, so a file in
one of them that is ignored, or simply untracked, is a file that will be
missing from the next clone.

# What this deliberately does not do

It does not read the index for files that are present and untracked anywhere
else in the tree. Working notes, editor droppings and build output are
untracked on purpose and there is no rule that separates them from an omission
by reading names. The three checks above are the places where an untracked
file is always a mistake rather than sometimes one.

It does not resolve an `include_str!` whose argument is computed beyond the
one form this tree uses, `concat!(env!("CARGO_MANIFEST_DIR"), "/some/path")`.
Anything else is counted and named in the output rather than passed over in
silence, so a new form is visible the day it appears.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# `include_str!("relative/path")`, resolved against the file holding it.
LITERAL = re.compile(r'include_(?:str|bytes)!\s*\(\s*"((?:[^"\\]|\\.)*)"\s*\)')

# `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/relative/path"))`,
# resolved against the crate root. Spread over three lines everywhere it is
# used, so this is matched against the whole file rather than line by line.
MANIFEST = re.compile(
    r'include_(?:str|bytes)!\s*\(\s*concat!\s*\(\s*env!\s*\(\s*"CARGO_MANIFEST_DIR"\s*\)\s*,'
    r'\s*"((?:[^"\\]|\\.)*)"\s*,?\s*\)\s*\)')

# Every occurrence, so the two above can be subtracted from it.
ANY_INCLUDE = re.compile(r"include_(?:str|bytes)!")

# The directories that exist to hold committed material and nothing else.
DATA_DIRS = ("assets",)
DATA_LEAF = "testdata"


def git(args, cwd, stdin=None):
    """Run git and return its stdout. Never raises on a non-zero exit."""
    done = subprocess.run(["git"] + args, cwd=cwd, input=stdin,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return done.stdout


def tracked(cwd):
    """Every path git carries, repository-relative, with forward slashes."""
    out = git(["ls-files", "-z"], cwd).decode("utf-8")
    return set(part for part in out.split("\0") if part)


def crate_root(path):
    """The directory of the nearest Cargo.toml at or above `path`."""
    at = os.path.dirname(path)
    while True:
        if os.path.exists(os.path.join(at, "Cargo.toml")):
            return at
        parent = os.path.dirname(at)
        if parent == at:
            return None
        at = parent


def sources(cwd, files):
    """Every tracked Rust file, as (repository-relative path, text)."""
    for rel in sorted(files):
        if rel.endswith(".rs"):
            full = os.path.join(cwd, rel)
            if os.path.exists(full):
                with open(full, encoding="utf-8", errors="replace") as handle:
                    yield rel, handle.read()


def line_of(text, at):
    return text.count("\n", 0, at) + 1


def embedded(cwd, files):
    """
    Every compile-time file read, and every one that could not be read.

    Returns (wanted, unread): `wanted` is (source, line, target) with target
    repository-relative; `unread` is (source, line) for a macro whose argument
    is in neither form.
    """
    wanted, unread = [], []
    for rel, text in sources(cwd, files):
        full = os.path.join(cwd, rel)
        seen = []
        for found in LITERAL.finditer(text):
            target = os.path.normpath(
                os.path.join(os.path.dirname(full), found.group(1)))
            wanted.append((rel, line_of(text, found.start()), target))
            seen.append(found.start())
        for found in MANIFEST.finditer(text):
            base = crate_root(full)
            if base is None:
                unread.append((rel, line_of(text, found.start())))
                continue
            target = os.path.normpath(
                os.path.join(base, found.group(1).lstrip("/")))
            wanted.append((rel, line_of(text, found.start()), target))
            seen.append(found.start())
        for found in ANY_INCLUDE.finditer(text):
            if found.start() in seen:
                continue
            # A macro named in prose is not a file read. Every doc comment in
            # this tree that discusses `include_str!` would otherwise be
            # reported as an argument this could not resolve.
            begins = text.rfind("\n", 0, found.start()) + 1
            if text[begins:found.start()].lstrip().startswith("//"):
                continue
            unread.append((rel, line_of(text, found.start())))
    return wanted, unread


def unescaped_hash(pattern):
    """Where an unescaped `#` begins, or None. A leading one is a comment."""
    at = 0
    while at < len(pattern):
        if pattern[at] == "\\":
            at += 2
            continue
        if pattern[at] == "#":
            return at
        at += 1
    return None


def ignore_files(files):
    return sorted(rel for rel in files
                  if rel == ".gitignore" or rel.endswith("/.gitignore"))


def deciding_rules(cwd, files):
    """Every (ignore file, line) that decides the fate of a tracked path.

    `--no-index` is the point of this: without it git answers "tracked, so not
    ignored" and never says which rule it would have used.
    """
    payload = "\0".join(sorted(files)).encode("utf-8") + b"\0"
    out = git(["check-ignore", "-v", "--no-index", "--non-matching",
               "--stdin", "-z"], cwd, stdin=payload).decode("utf-8")
    fields = out.split("\0")
    rules = set()
    for at in range(0, len(fields) - 3, 4):
        source, number = fields[at], fields[at + 1]
        if source:
            rules.add((source, int(number)))
    return rules


def data_files(cwd):
    """Every file under a directory that exists to hold committed material."""
    roots = [os.path.join(cwd, name) for name in DATA_DIRS]
    for current, directories, _ in os.walk(cwd):
        directories[:] = [d for d in directories
                          if d not in (".git", "target", "node_modules")]
        if os.path.basename(current) == DATA_LEAF:
            roots.append(current)
    out = []
    for root in roots:
        for current, directories, names in os.walk(root):
            directories[:] = [d for d in directories if d != "target"]
            for name in sorted(names):
                out.append(os.path.relpath(
                    os.path.join(current, name), cwd).replace(os.sep, "/"))
    return sorted(set(out))


def audit(cwd):
    """Every problem found, as a list of lines to print. Empty means clean."""
    files = tracked(cwd)
    problems = []

    wanted, unread = embedded(cwd, files)
    missing = []
    for rel, number, target in wanted:
        inside = os.path.relpath(target, cwd).replace(os.sep, "/")
        if inside not in files:
            why = "not in the repository" if os.path.exists(target) \
                else "not there at all"
            missing.append((rel, number, inside, why))
    if missing:
        problems.append("  these are read at compile time and git does not carry them:")
        for rel, number, inside, why in missing:
            problems.append("    %s:%d  %s" % (rel, number, inside))
            problems.append("        %s" % why)
        problems.append("")
        problems.append("    A file that is present and untracked reads exactly like a")
        problems.append("    file that is committed. This crate will not compile on any")
        problems.append("    other clone. If an ignore rule is swallowing it, allow it")
        problems.append("    back in by name in .gitignore rather than with `git add -f`,")
        problems.append("    which leaves no record of why the file is there.")

    for rel in ignore_files(files):
        full = os.path.join(cwd, rel)
        if not os.path.exists(full):
            continue
        with open(full, encoding="utf-8") as handle:
            lines = handle.read().split("\n")
        for number, raw in enumerate(lines, 1):
            pattern = raw.rstrip("\r")
            if not pattern.strip() or pattern.lstrip().startswith("#"):
                continue
            at = unescaped_hash(pattern)
            if at is not None:
                problems.append("  %s:%d carries a `#` inside a pattern:" % (rel, number))
                problems.append("    %s" % pattern)
                problems.append("        git has no inline comments. The pattern is the")
                problems.append("        whole line, `#` and the words after it included,")
                problems.append("        so it matches a path nobody will ever have. Put")
                problems.append("        the comment on its own line above the pattern.")

    rules = deciding_rules(cwd, files)
    for rel in ignore_files(files):
        full = os.path.join(cwd, rel)
        if not os.path.exists(full):
            continue
        with open(full, encoding="utf-8") as handle:
            lines = handle.read().split("\n")
        for number, raw in enumerate(lines, 1):
            pattern = raw.strip()
            if not pattern.startswith("!"):
                continue
            if (rel, number) in rules:
                continue
            problems.append("  %s:%d un-ignores nothing:" % (rel, number))
            problems.append("    %s" % pattern)
            problems.append("        No tracked file is allowed back in by this rule, so")
            problems.append("        nothing has ever tested that it works. Write the")
            problems.append("        negation when there is a file that needs it, and it")
            problems.append("        is exercised by that file existing.")

    stray = []
    for rel in data_files(cwd):
        if rel in files:
            continue
        done = subprocess.run(["git", "check-ignore", "-q", "--no-index", rel],
                              cwd=cwd, stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL)
        stray.append((rel, done.returncode == 0))
    if stray:
        problems.append("  these sit in a directory that exists for committed files,")
        problems.append("  and are not committed:")
        for rel, ignored in stray:
            problems.append("    %s%s" % (rel, "  (an ignore rule is hiding it)" if ignored else ""))
        problems.append("")
        problems.append("    assets/ and testdata/ hold what the build and the tests")
        problems.append("    read. Anything here that git will not carry is missing from")
        problems.append("    the next clone, and no check that reads the working tree")
        problems.append("    will say so.")

    return problems, len(wanted), unread


def self_test():
    """Prove each check fires, against repositories built to break them."""
    import shutil
    import tempfile

    def repo(where):
        subprocess.run(["git", "init", "-q", "-b", "main", where], check=True)
        subprocess.run(["git", "-C", where, "config", "user.email", "t@t"], check=True)
        subprocess.run(["git", "-C", where, "config", "user.name", "t"], check=True)

    def write(where, rel, text):
        full = os.path.join(where, rel)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        with open(full, "w", encoding="utf-8") as handle:
            handle.write(text)

    def commit(where):
        subprocess.run(["git", "-C", where, "add", "-A"], check=True)
        subprocess.run(["git", "-C", where, "commit", "-qm", "x"], check=True)

    cases = []

    # A fixture an include_str! names, excluded and therefore never committed.
    def swallowed(where):
        write(where, ".gitignore", "*.asc\n")
        write(where, "Cargo.toml", "[package]\nname = \"x\"\n")
        write(where, "src/lib.rs", 'const K: &str = include_str!("k.asc");\n')
        write(where, "src/k.asc", "key\n")
        commit(where)
    cases.append(("a compile-time read of an excluded file",
                  swallowed, "git does not carry them"))

    # A pattern with an inline comment, which git reads as part of the pattern.
    def inline(where):
        write(where, ".gitignore", "*.wav\n!assets/**   # opted back in\n")
        write(where, "Cargo.toml", "[package]\nname = \"x\"\n")
        commit(where)
    cases.append(("an inline comment in a pattern", inline, "carries a `#`"))

    # A negation no tracked file is allowed back in by.
    def dead(where):
        write(where, ".gitignore", "*.wav\n!never/here.wav\n")
        write(where, "Cargo.toml", "[package]\nname = \"x\"\n")
        commit(where)
    cases.append(("a negation that matches nothing", dead, "un-ignores nothing"))

    # A file in a directory that exists for committed material.
    def stray(where):
        write(where, ".gitignore", "*.wav\n")
        write(where, "Cargo.toml", "[package]\nname = \"x\"\n")
        commit(where)
        write(where, "assets/demo.wav", "sound\n")
    cases.append(("an ignored file under assets/", stray, "and are not committed"))

    failures = 0
    for name, build, expected in cases:
        where = tempfile.mkdtemp()
        try:
            repo(where)
            build(where)
            found, _, _ = audit(where)
            text = "\n".join(found)
            if expected in text:
                print("    caught: %s" % name)
            else:
                failures += 1
                print("    MISSED: %s" % name)
                print("      expected %r in:" % expected)
                print("\n".join("      " + line for line in found) or "      (clean)")
        finally:
            shutil.rmtree(where, ignore_errors=True)

    # And a repository with none of those faults must come back clean.
    where = tempfile.mkdtemp()
    try:
        repo(where)
        write(where, ".gitignore", "*.asc\n# the one place it belongs\n!keys/ours.asc\n")
        write(where, "Cargo.toml", "[package]\nname = \"x\"\n")
        write(where, "src/lib.rs", 'const K: &str = include_str!("../keys/ours.asc");\n')
        write(where, "keys/ours.asc", "key\n")
        subprocess.run(["git", "-C", where, "add", "-A"], check=True)
        subprocess.run(["git", "-C", where, "commit", "-qm", "x"], check=True)
        found, _, _ = audit(where)
        if found:
            failures += 1
            print("    MISSED: a sound repository was reported as faulty")
            print("\n".join("      " + line for line in found))
        else:
            print("    clean: a repository with none of these faults")
    finally:
        shutil.rmtree(where, ignore_errors=True)

    if failures:
        print("  %d self-test(s) failed: this guard does not catch what it claims to"
              % failures)
        return 1
    print("  every fault this guard exists for is caught by it")
    return 0


def main():
    if "--self-test" in sys.argv:
        return self_test()

    problems, reads, unread = audit(ROOT)
    if problems:
        print("\n".join(problems))
        return 1

    note = ""
    if unread:
        note = ", and %d whose argument this does not resolve (%s)" % (
            len(unread), ", ".join("%s:%d" % pair for pair in unread[:3]))
    print("  %d compile-time file read(s), every one of them committed%s"
          % (reads, note))
    return 0


if __name__ == "__main__":
    sys.exit(main())
