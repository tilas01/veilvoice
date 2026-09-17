#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Every manifest in this tree is covered by a Dependabot entry.

    python tools/audit/dependabot.py     # report, and fail on any gap

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
`package-ecosystem` and `directory` pairs, which have one shape.

Pure standard library, like everything in `tools/`.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
CONFIG = os.path.join(ROOT, ".github", "dependabot.yml")

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


def entries():
    """The (ecosystem, directory) pairs the configuration declares.

    Read line by line rather than parsed as YAML. An entry opens with
    `- package-ecosystem:` and its `directory:` follows within the same block,
    which is the only shape this file has and the only one it is allowed to
    grow: a second `directory` under one ecosystem would be read as a second
    entry here and would be reported, which is the safe direction to be wrong
    in.
    """
    if not os.path.isfile(CONFIG):
        return None

    found = []
    ecosystem = None
    with open(CONFIG, "r", encoding="utf-8") as handle:
        for line in handle:
            stripped = line.strip()
            if stripped.startswith("#"):
                continue
            if "package-ecosystem:" in stripped:
                ecosystem = stripped.split("package-ecosystem:", 1)[1].strip().strip("\"'")
                continue
            if stripped.startswith("directory:") and ecosystem:
                directory = stripped.split("directory:", 1)[1].strip().strip("\"'")
                found.append((ecosystem, directory))
                ecosystem = None
    return found


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
    have = set(declared)
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
    for ecosystem, where in declared:
        if where == "/":
            continue
        if not os.path.isdir(os.path.join(ROOT, where.strip("/"))):
            gaps.append(
                "%s names %s, which is not in this tree any more" % (ecosystem, where)
            )

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
            "ignore for a dependency this tree no longer asks for, or give a "
            "bare ignore its update-types. A manifest nothing watches is worse "
            "than one with a known problem, because nobody is looking, and an "
            "ignore with no update-types is worse still: it looks like a "
            "held-back major and silences the advisory as well."
        )
        return 1

    print(
        "  %d Dependabot entries, covering every manifest in the tree; "
        "%d held-back major(s), every one still used and none silencing a "
        "security update" % (len(declared), len(ignored()))
    )
    for ecosystem, where in declared:
        print("    %-16s %s" % (ecosystem, where))
    return 0


if __name__ == "__main__":
    sys.exit(main())
