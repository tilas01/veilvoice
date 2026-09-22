#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Every manifest in this tree is covered by a Dependabot entry, aimed at `dev`.

    python tools/audit/dependabot.py     # report, and fail on any gap
    python tools/audit/dependabot.py --self-test   # prove the checks catch it

There is no `--check`, for the reason `dependencies.py` has none: this tool
only reads, so reporting and checking are the same run and there is no mode in
which it passes quietly over a manifest nobody monitors.

# What this is for

`.github/dependabot.yml` names the directories to watch. Nothing in Cargo or in
GitHub Actions tells it when a new one appears, so a crate added outside the
workspace, a second workflow directory, or a `package.json` for a website that
grew a real dependency would all be unmonitored, and would be unmonitored
*quietly*. That is the failure this is built to make loud: not a dependency
with a known problem, but one nothing is looking at.

# And where the pull requests point

An entry with no `target-branch` opens against the repository's default
branch, which here is `main`. `main` is the released program and moves only
when a release is cut, by merging `dev` into it. A bump merged into `main` any
other way is a commit `dev` does not have, so `dev` falls behind on it and the
next release merge either loses the change or conflicts with it, and nothing
says so: the pull request looks entirely ordinary.

That is F-200, and it is the fourth time a branch policy changed without the
file reading it changing too. F-197 was `ci.yml`, still running on `main` only
after development moved to `dev`, so every push of real work ran no CI. The
shape is the same both times, which is why the branch is now **derived rather
than named here**: the target of every entry has to be a branch `ci.yml`
actually builds on a push, and must not be the release branch. Writing `dev`
into a third file is how this happens a third time.

`CLAUDE.md` asks that a fact existing in more than one place be derived or
checked rather than repeated. The list of directories to monitor exists in the
configuration and in the tree, and the tree is the one that is true. This is
the check that keeps them the same.

# What it does not check

Whether Dependabot is *enabled* for the repository, and whether alerts are on.
Those are repository settings rather than files, and a tool that read the tree
and reported on a setting it cannot see would be guessing.

Nor does it validate the whole file as YAML. This project ships no YAML parser
and will not add one to read one file it wrote itself; what it reads is the
`package-ecosystem`, `directory` and `target-branch` keys of each entry, which
have one shape.

Pure standard library, like everything in `tools/`.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
CONFIG = os.path.join(ROOT, ".github", "dependabot.yml")
CI = os.path.join(ROOT, ".github", "workflows", "ci.yml")

# The branch a release is cut from, and therefore the one a dependency bump
# must not arrive on directly. This is the single name this file spells out,
# and it is the one that cannot be derived: `main` is the default branch, which
# is a repository setting rather than a file in the tree, so a tool reading
# only the tree would be guessing at it.
RELEASE_BRANCH = "main"

# The manifest each ecosystem is recognised by. Only the ones this repository
# can actually grow are listed: adding every ecosystem GitHub supports would be
# a list to keep true for no benefit, and an unknown manifest appearing is
# caught by a person rather than by a longer table.
MANIFESTS = {
    "cargo": ("Cargo.toml",),
    "npm": ("package.json",),
    "bundler": ("Gemfile",),
    "pip": ("requirements.txt", "pyproject.toml"),
}

# Directories that are not this project's code and are never monitored.
SKIP = {".git", "target", "node_modules", "wiki", ".dependabot"}


def value(stripped, key):
    """The value after `key:`, unquoted, or `None` if this is not that key."""
    if not stripped.startswith(key + ":"):
        return None
    return stripped.split(":", 1)[1].strip().strip("\"'")


def read_entries(text):
    """Every entry the configuration declares: ecosystem, directory, target.

    Read line by line rather than parsed as YAML, and indentation is what
    separates an entry's own keys from the keys of the blocks nested inside it.
    An entry opens with `- package-ecosystem:` at some indent, and its own keys
    sit two columns further in; `schedule`, `groups` and `ignore` all put their
    contents deeper than that, which is how `- dependency-name:` inside
    `ignore` is not mistaken for the start of a second entry.

    This used to track nothing but a `package-ecosystem` seen and the first
    `directory` after it, and it was that shortcut which made adding
    `target-branch` a change here rather than a line: the old reader cleared
    its state at `directory` and could not have seen a key written after it.

    `target-branch` is `None` when the entry does not say, which is not the
    same as an entry naming a branch. Dependabot's default in that case is the
    repository's default branch, and that default is the whole of F-200.
    """
    found = []
    current = None
    keys_at = None
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        if stripped.startswith("- package-ecosystem:"):
            if current is not None:
                found.append(current)
            ecosystem = stripped.split("package-ecosystem:", 1)[1].strip().strip("\"'")
            current = {"ecosystem": ecosystem, "directory": None,
                       "target_branch": None}
            keys_at = indent + 2
            continue
        if current is None or indent != keys_at:
            continue
        directory = value(stripped, "directory")
        if directory is not None:
            current["directory"] = directory
            continue
        target = value(stripped, "target-branch")
        if target is not None:
            current["target_branch"] = target
    if current is not None:
        found.append(current)
    return found


def entries():
    """Every entry in `.github/dependabot.yml`, or `None` if there is no file."""
    if not os.path.isfile(CONFIG):
        return None
    with open(CONFIG, "r", encoding="utf-8") as handle:
        return read_entries(handle.read())


def read_ci_push_branches(text):
    """The branches `ci.yml` builds on a push, from its own `branches:` line.

    Derived rather than written down here, because the last two findings in
    this area were both a branch policy changed in one file and not in the
    other. If `ci.yml` is the file that says which branches are built, then it
    is also the file that says which branch a dependency bump may be aimed at:
    a bump opened against a branch nothing builds is a bump nobody can judge.

    Only the `push:` trigger's list counts. `pull_request:` has one here with
    no branches at all, and reading a `branches:` from anywhere in the file
    would take whichever came first.
    """
    branches = []
    in_push = False
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        if stripped.startswith("push:"):
            in_push = True
            push_at = indent
            continue
        if in_push and indent <= push_at:
            # Any key back at the trigger's own level ends the push block.
            in_push = False
        if not in_push or not stripped.startswith("branches:"):
            continue
        listed = stripped.split("branches:", 1)[1].strip()
        if listed.startswith("[") and listed.endswith("]"):
            branches = [b.strip().strip("\"'") for b in listed[1:-1].split(",")]
            branches = [b for b in branches if b]
        in_push = False
    return branches


def ci_push_branches():
    """What `ci.yml` builds on a push, or an empty list if it cannot be read."""
    if not os.path.isfile(CI):
        return []
    with open(CI, "r", encoding="utf-8") as handle:
        return read_ci_push_branches(handle.read())


def branch_gaps(declared, built):
    """Where each entry points, and whether that is somewhere work belongs.

    Separate from `main` so the self-test can drive it on text rather than on
    this repository, which is the only way to prove a guard notices the thing
    it was written for.
    """
    gaps = []
    wanted = [b for b in built if b != RELEASE_BRANCH]
    for entry in declared:
        where = "%s in %s" % (entry["ecosystem"], entry["directory"])
        target = entry["target_branch"]
        if target is None:
            gaps.append(
                "%s has no target-branch, so its pull requests open against "
                "the default branch. A bump merged there is a change `dev` "
                "does not have, and the release merge that follows either "
                "loses it or conflicts with it" % where
            )
        elif target == RELEASE_BRANCH:
            gaps.append(
                "%s targets %s, which is the released branch: it moves only "
                "when a release is cut, by merging the development branch "
                "into it" % (where, RELEASE_BRANCH)
            )
        elif wanted and target not in wanted:
            gaps.append(
                "%s targets %s, which is not a branch ci.yml builds on a "
                "push (%s), so nothing would check the bump"
                % (where, target, ", ".join(built))
            )
    return gaps


def ignored():
    """Every `ignore` entry: the dependency, and the update types it silences.

    Line by line, for the same reason `entries` is: this file is read by
    something that is not here, so the check has to read what is written
    rather than what a YAML library can be talked into.
    """
    out, name, types, inside = [], None, None, False
    with open(CONFIG, "r", encoding="utf-8") as handle:
        for line in handle:
            stripped = line.strip()
            if stripped.startswith("#"):
                continue
            if stripped == "ignore:":
                inside = True
                continue
            if inside and stripped and not stripped.startswith(("-", "update-types:")):
                # Any other key at this level ends the ignore block.
                if ":" in stripped and not stripped.startswith("dependency-name:"):
                    if name is not None:
                        out.append((name, types))
                        name, types = None, None
                    inside = False
                    continue
            if not inside:
                continue
            if "dependency-name:" in stripped:
                if name is not None:
                    out.append((name, types))
                    types = None
                name = stripped.split("dependency-name:", 1)[1].strip().strip("\"'")
            elif stripped.startswith("update-types:"):
                types = stripped.split("update-types:", 1)[1].strip()
    if name is not None:
        out.append((name, types))
    return out


def declared_dependencies():
    """Every dependency name any manifest in this tree asks for."""
    names = set()
    for current, directories, files in os.walk(ROOT):
        directories[:] = [d for d in directories if d not in SKIP]
        if "Cargo.toml" not in files:
            continue
        with open(os.path.join(current, "Cargo.toml"), "r", encoding="utf-8") as handle:
            for line in handle:
                stripped = line.strip()
                if stripped.startswith("#") or "=" not in stripped:
                    continue
                key = stripped.split("=", 1)[0].strip().strip("\"'")
                if key and all(c.isalnum() or c in "-_" for c in key):
                    names.add(key)
    return names


def workspace_members():
    """The directories the root `Cargo.toml` already covers through the workspace.

    Cargo is workspace-aware to Dependabot: an entry on the root reaches every
    member through the one `Cargo.lock`, so a member needing an entry of its
    own would be twenty-seven pull requests for one bumped version. What needs
    its own entry is a manifest the workspace does **not** reach, which is what
    this exists to tell apart.
    """
    root = os.path.join(ROOT, "Cargo.toml")
    if not os.path.isfile(root):
        return set()
    covered = set()
    inside = False
    with open(root, "r", encoding="utf-8") as handle:
        for line in handle:
            stripped = line.strip()
            if stripped.startswith("["):
                inside = stripped.strip("[]") == "workspace"
                continue
            if not inside or stripped.startswith("#"):
                continue
            # `members = ["crates/*"]`, possibly over several lines. Only the
            # quoted pieces matter, and a trailing `/*` is a directory of them.
            for piece in stripped.split('"')[1::2]:
                if piece.endswith("/*"):
                    parent = os.path.join(ROOT, piece[:-2])
                    if os.path.isdir(parent):
                        for name in sorted(os.listdir(parent)):
                            if os.path.isfile(os.path.join(parent, name, "Cargo.toml")):
                                covered.add("/%s/%s" % (piece[:-2].strip("/"), name))
                elif piece:
                    covered.add("/%s" % piece.strip("/"))
    return covered


def found_in_tree():
    """Every (ecosystem, directory) this repository actually has a manifest for."""
    found = set()
    for here, directories, files in os.walk(ROOT):
        directories[:] = [d for d in directories if d not in SKIP and not d.startswith(".")]
        relative = os.path.relpath(here, ROOT).replace(os.sep, "/")
        where = "/" if relative == "." else "/%s" % relative
        for ecosystem, names in MANIFESTS.items():
            if any(name in files for name in names):
                found.add((ecosystem, where))

    # The workflows, which live in a directory the walk above skips because it
    # begins with a dot. Dependabot addresses them as "/" rather than as the
    # directory they are in, which is its own convention and is why this is
    # separate rather than another row in the table.
    if os.path.isdir(os.path.join(ROOT, ".github", "workflows")):
        found.add(("github-actions", "/"))
    return found


def main():
    declared = entries()
    if declared is None:
        print("there is no .github/dependabot.yml, so nothing is monitored.")
        print()
        print(
            "Dependabot reads that path on the default branch and no other. "
            "A configuration anywhere else, committed or not, does nothing."
        )
        return 1

    covered_by_workspace = workspace_members()
    have = {(e["ecosystem"], e["directory"]) for e in declared}
    gaps = []

    for ecosystem, where in sorted(found_in_tree()):
        if (ecosystem, where) in have:
            continue
        if ecosystem == "cargo" and where in covered_by_workspace:
            # Reached through the root entry, if there is one.
            if ("cargo", "/") in have:
                continue
        gaps.append(
            "%s in %s has no entry, so nothing is watching it" % (ecosystem, where)
        )

    # And the other direction: an entry naming a directory that is not there
    # any more. Dependabot ignores it silently, and a configuration carrying a
    # line about a crate that was deleted is the kind of thing somebody later
    # reads as evidence that the crate exists.
    for entry in declared:
        ecosystem, where = entry["ecosystem"], entry["directory"]
        if where is None:
            gaps.append("the %s entry names no directory" % ecosystem)
            continue
        if where == "/":
            continue
        if not os.path.isdir(os.path.join(ROOT, where.strip("/"))):
            gaps.append(
                "%s names %s, which is not in this tree any more" % (ecosystem, where)
            )

    # And where each entry's pull requests land, which is F-200. The branches
    # come from `ci.yml` rather than from a name written here: see the note at
    # the top about why this is derived.
    gaps.extend(branch_gaps(declared, ci_push_branches()))

    # And the `ignore` list, which is the part of this file that can quietly
    # stop doing what it says. Two ways it goes wrong, both silent:
    #
    #   * an entry naming a dependency that is no longer in any manifest. It
    #     does nothing, and it reads as evidence of a decision about something
    #     this project still uses.
    #   * an entry with no `update-types`. That silences *every* update for
    #     that dependency, including the security update, which is the exact
    #     opposite of what this file exists to do. Every entry here is meant to
    #     hold back a major while letting patches through.
    have_dependency = declared_dependencies()
    for name, types in ignored():
        if name not in have_dependency:
            gaps.append(
                "the ignore list names %s, which no manifest asks for any more"
                % name
            )
        if not types:
            gaps.append(
                "the ignore for %s has no update-types, so it silences that "
                "dependency's security updates too" % name
            )

    if gaps:
        print("the Dependabot configuration and this tree disagree:")
        for gap in gaps:
            print("  %s" % gap)
        print()
        print(
            "Fix .github/dependabot.yml. Add an entry for a manifest nothing "
            "covers, take out one naming a directory that has gone, drop an "
            "ignore for a dependency this tree no longer asks for, give a "
            "bare ignore its update-types, or point an entry at the branch "
            "development happens on. A manifest nothing watches is worse "
            "than one with a known problem, because nobody is looking; an "
            "ignore with no update-types is worse still, because it looks "
            "like a held-back major and silences the advisory as well; and a "
            "bump aimed at the released branch is a change that never passes "
            "through development at all."
        )
        return 1

    print(
        "  %d Dependabot entries, covering every manifest in the tree; "
        "%d held-back major(s), every one still used and none silencing a "
        "security update" % (len(declared), len(ignored()))
    )
    for entry in declared:
        print("    %-16s %-8s -> %s"
              % (entry["ecosystem"], entry["directory"], entry["target_branch"]))
    return 0


# The configurations the branch check has to have an opinion about, each with
# the reason it is here. Text rather than files: a guard proved against the
# repository it guards is a guard proved against one case, which is the case
# that already passes.
#
# `ci.yml` builds `main` and `dev`, so `dev` is the one branch a bump may be
# aimed at. Every case below is read against that.
CI_SAMPLE = """
on:
  push:
    branches: [main, dev]
  pull_request:
"""

CASES = [
    ("""
updates:
  - package-ecosystem: cargo
    directory: "/"
    target-branch: dev
    schedule:
      interval: weekly
""", 0, "an entry aimed at the development branch is what this file should say"),

    ("""
updates:
  - package-ecosystem: cargo
    directory: "/"
    schedule:
      interval: weekly
""", 1, "no target-branch at all, which is F-200 itself: it opens against the "
        "default branch and nothing in the file says so"),

    ("""
updates:
  - package-ecosystem: cargo
    directory: "/"
    target-branch: main
    schedule:
      interval: weekly
""", 1, "aimed at the released branch, which moves only on a release merge"),

    ("""
updates:
  - package-ecosystem: cargo
    directory: "/"
    target-branch: develop
    schedule:
      interval: weekly
""", 1, "aimed at a branch ci.yml does not build, so nothing would check it"),

    ("""
updates:
  - package-ecosystem: cargo
    directory: "/"
    target-branch: dev
    schedule:
      interval: weekly
  - package-ecosystem: github-actions
    directory: "/"
    schedule:
      interval: weekly
""", 1, "one entry right and one silently wrong, which is the state this "
        "repository was actually in for three ecosystems"),

    ("""
updates:
  - package-ecosystem: cargo
    directory: "/"
    target-branch: dev
    schedule:
      interval: weekly
    ignore:
      - dependency-name: "region"
        update-types: ["version-update:semver-major"]
""", 0, "an ignore list is nested deeper than the entry's own keys, so its "
        "`- dependency-name:` must not read as a second entry"),
]


def self_test():
    """Drive the branch check over the cases above. Returns the exit status.

    A guard is only worth the case that fails it. The first version of this
    check passed on this repository before `target-branch` was added to it,
    because it was reading the key from a parser that stopped looking after
    `directory:`, and nothing would have said so.
    """
    built = read_ci_push_branches(CI_SAMPLE)
    if sorted(built) != ["dev", "main"]:
        print("the ci.yml reader is wrong about the sample: %r" % (built,))
        return 1

    wrong = []
    for text, expected, why in CASES:
        got = len(branch_gaps(read_entries(text), built))
        if (got > 0) != (expected > 0):
            wrong.append("  expected %s, got %d complaint(s)  (%s)"
                         % ("a complaint" if expected else "no complaint",
                            got, why))
    if wrong:
        print("the branch check is wrong on %d case(s):" % len(wrong))
        print("\n".join(wrong))
        return 1
    print("  the branch check is right on all %d cases, and reads ci.yml's "
          "push branches" % len(CASES))
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv[1:]:
        sys.exit(self_test())
    sys.exit(main())
