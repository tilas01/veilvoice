#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Is `dev` ready to become `main`, and is that version coherent?

    python tools/release/readiness.py            # ask about the workspace version
    python tools/release/readiness.py v0.1.23    # ask about a named version
    python tools/release/readiness.py --self-test

# What this is for

`docs/CONTRIBUTING.md` says when a version is cut: the roadmap items in the
group are done, `tools/verify.py` passes whole rather than `--quick`,
`CHANGELOG.md` has the notes, and an audit round has read what changed. That
sentence was a description of a habit. Nothing read it, so every clause of it
was enforced by somebody remembering it on the morning of a release.

Two of those clauses already have machinery. `tools/verify.py` is the whole
verification pass, and CI runs its checks on every push. `release.yml` refuses
to publish a release whose `## v<version>` section is missing from
`CHANGELOG.md`, which is F-170's other half.

The trouble with the second one is *when* it fires. It is the last step of a
twelve-job build, so a release missing its notes is discovered after every
binary has been compiled twice and compared, and after `main` has already
moved. `main` moving is the part that is awkward to undo: it is the released
branch, the website and the wiki publish from it, and a reader who fetches
between the merge and the repair gets a `main` that claims to be a release
nobody can read the notes of.

So this asks the whole question **before** anything moves, from a checkout of
`dev`, and it is the gate `promote.yml` runs first.

# What it checks, and why each one

1. **The version being promoted is the version the workspace says it is.**
   Promoting `v0.1.23` from a tree whose `Cargo.toml` says `0.1.22` publishes
   binaries that report the previous version. `tools/release/version.py`
   already checks that every *copy* of the version agrees with `Cargo.toml`;
   this checks that the number a person typed agrees with it too, which is the
   one comparison that tool cannot make because nobody has typed anything yet
   when it runs.

2. **`CHANGELOG.md` has a `## v<version>` section, and it is the newest one.**
   The heading match is the same one `release.yml` makes, spelled the same
   way, so this cannot pass and that fail. The ordering half is extra: a
   section added below an older release still matches, and would then publish
   correct notes under a changelog that reads out of order for as long as the
   file exists.

3. **Every roadmap item the release names is done.** `ROADMAP.md` has a
   section per planned version saying which items it carries, and a table
   giving each item a status. Those are two statements of the same fact in one
   file, and the one that is easy to forget is the table. An item still marked
   `planned` in a release that has shipped is the roadmap lying about what is
   finished, which is the one thing a roadmap is for.

4. **`docs/AUDIT.md` has been written in since the previous release.** The
   audit round is a reading rather than a run, so no tool can say whether it
   was a good one. What a tool can say is whether it happened at all: if the
   file has not changed since the last tag, then nothing has been written up
   about anything in the release, and the clause in `docs/CONTRIBUTING.md`
   about an audit round is being skipped rather than satisfied.

5. **The tag does not exist yet.** Publishing a release for a tag that is
   already there either fails late or, worse, succeeds against the older
   commit and publishes binaries that do not match it.

6. **Nothing has landed on `main` that `dev` has not got.** The release merge
   takes `dev` into `main`, so `main` always carries merge commits `dev` does
   not, and asking whether it is an ancestor would fail on a healthy
   repository. What would be wrong is a *non-merge* commit reachable from
   `main` and not from here, because that is work put on the released branch
   directly, which is F-200's shape: the next release merge either loses it or
   stops on a conflict, and neither announces itself at the time.

Checks 4, 5 and 6 need the repository's history, so they say so and are skipped
rather than guessed at when this runs somewhere without it.

Pure standard library, like everything in `tools/`.
"""

import re
import subprocess
import sys
from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent.parent


# ---------------------------------------------------------------- the readers
#
# Each of these is a pure function of text, so the self-test can exercise it
# without a repository, and so a failure here is a failure of parsing rather
# than of whatever the parse was being used for.


def workspace_version(cargo_toml: str) -> str:
    """The `version` under `[workspace.package]`.

    Anchored to that table rather than to the first `version =` in the file: a
    dependency pinned by version is also a `version =` line, and there are
    dozens of them.
    """
    table = re.search(
        r"^\[workspace\.package\]\s*$(.*?)(?=^\[|\Z)",
        cargo_toml,
        re.MULTILINE | re.DOTALL,
    )
    if not table:
        raise ValueError("Cargo.toml has no [workspace.package] table")
    found = re.search(r'^\s*version\s*=\s*"([^"]+)"', table.group(1), re.MULTILINE)
    if not found:
        raise ValueError("[workspace.package] declares no version")
    return found.group(1)


def changelog_sections(changelog: str) -> list:
    """Every `## v...` heading, in the order the file gives them.

    `## v0.1.22` and nothing else: the file also carries prose headings, and a
    release section is the one whose text is a version.
    """
    return re.findall(r"^##\s+(v[0-9][^\s]*)\s*$", changelog, re.MULTILINE)


def release_items(roadmap: str, tag: str) -> list:
    """The roadmap items a named release says it carries.

    The per-version sections are `### Next: v0.1.23, ...`, `### After that:
    v0.1.24, ...` and so on, so the heading is matched on the version it names
    rather than on the words in front of it, which are a running commentary and
    change every release.

    Inside the section the items are written as bold numbers, `**167**`, in
    prose that explains the order. That is the form to read: a plain number in
    that prose is a duration or a count, and a bold one is an item.
    """
    heading = re.compile(r"^###\s+[^\n]*?\b" + re.escape(tag) + r"\b[^\n]*$", re.MULTILINE)
    start = heading.search(roadmap)
    if not start:
        return []
    rest = roadmap[start.end():]
    end = re.search(r"^###?\s", rest, re.MULTILINE)
    section = rest[: end.start()] if end else rest
    seen = []
    for number in re.findall(r"\*\*([0-9]{1,4})\*\*", section):
        if number not in seen:
            seen.append(number)
    return seen


def item_status(roadmap: str, number: str):
    """The status cell of one row of the roadmap table, or None if no such row.

    The table is `| 62 | **What it is** | **done** | - |`. The status is read
    with the emphasis stripped, because `done` and `**done**` are the same
    statement and the file uses both.
    """
    row = re.search(
        r"^\|\s*" + re.escape(number) + r"\s*\|(?:[^\n|]*\|)*?\s*\*{0,2}(done|planned|dropped|blocked|in progress)\*{0,2}\s*\|",
        roadmap,
        re.MULTILINE | re.IGNORECASE,
    )
    return row.group(1).lower() if row else None


# ------------------------------------------------------------------- the gate


def git(root: Path, *args):
    """Run git, returning (ok, output). Never raises: a checkout with no
    history is a reason to skip a check rather than to crash."""
    try:
        done = subprocess.run(
            ["git", *args], cwd=str(root), capture_output=True, text=True
        )
    except OSError as error:
        return False, str(error)
    return done.returncode == 0, (done.stdout + done.stderr).strip()


def check(root: Path, tag: str) -> list:
    """Every gap, as a list of sentences. Empty means ready."""
    gaps = []
    version = tag[1:] if tag.startswith("v") else tag

    declared = workspace_version((root / "Cargo.toml").read_text(encoding="utf-8"))
    if declared != version:
        gaps.append(
            "the workspace is version %s, so promoting %s would publish binaries "
            "that report %s. Bump Cargo.toml and run tools/release/version.py first."
            % (declared, tag, declared)
        )

    sections = changelog_sections((root / "CHANGELOG.md").read_text(encoding="utf-8"))
    if tag not in sections:
        gaps.append(
            "CHANGELOG.md has no '## %s' section, so the release would publish "
            "with no notes. The heading must be exactly '## %s'." % (tag, tag)
        )
    elif sections[0] != tag:
        gaps.append(
            "CHANGELOG.md has a '## %s' section but %s is above it, so the file "
            "reads out of order. The newest release goes first." % (tag, sections[0])
        )

    roadmap = (root / "ROADMAP.md").read_text(encoding="utf-8")
    items = release_items(roadmap, tag)
    if not items:
        gaps.append(
            "ROADMAP.md has no section naming %s, so nothing says which items "
            "this release carries." % tag
        )
    for number in items:
        status = item_status(roadmap, number)
        if status is None:
            gaps.append(
                "ROADMAP.md names item %s in the %s section and the table has no "
                "row for it." % (number, tag)
            )
        elif status != "done":
            gaps.append(
                "roadmap item %s is '%s' and %s says it carries it."
                % (number, status, tag)
            )

    ok, _ = git(root, "rev-parse", "--git-dir")
    if not ok:
        gaps.append(
            "not a git checkout, so the audit round, the tag and the branch "
            "ancestry were not checked"
        )
        return gaps

    exists, _ = git(root, "rev-parse", "--verify", "--quiet", "refs/tags/" + tag)
    if exists:
        gaps.append(
            "the tag %s already exists, so this release has been cut. A second "
            "one would publish against the commit the tag already names." % tag
        )

    # The newest release tag, by version rather than by ancestry.
    #
    # `git describe` is the obvious tool and the wrong one here. It walks back
    # from HEAD, and a release tag is made on the *merge into `main`*, which is
    # a commit `dev` does not contain: the merge takes `dev` into `main`, not
    # the other way. So `describe` from `dev` reports that no tag can describe
    # it, which is true and useless. Sorting the tag list by version answers
    # the question actually being asked, which is what the last release was.
    ok, tags = git(root, "tag", "--list", "v*", "--sort=-v:refname")
    previous = tags.splitlines()[0].strip() if ok and tags.strip() else ""
    if not previous:
        gaps.append("no v* tag was found, so the audit round was not checked")
    else:
        ok, touched = git(
            root, "log", "--oneline", previous + "..HEAD", "--", "docs/AUDIT.md"
        )
        if ok and not touched:
            gaps.append(
                "docs/AUDIT.md has not changed since %s, so no audit round has "
                "been written up for anything in this release." % previous
            )

    # And the same asymmetry one more time, for the same reason.
    #
    # `main` is never an ancestor of `dev`, because every release merge puts a
    # commit on `main` that `dev` has not got. Asking whether it is would fail
    # on a perfectly healthy repository. What can go wrong is work landing on
    # the released branch directly, and that work is a commit rather than a
    # merge, so the question is whether any non-merge commit is reachable from
    # `main` and not from here.
    ok, _ = git(root, "rev-parse", "--verify", "--quiet", "refs/remotes/origin/main")
    if ok:
        ok, stray = git(
            root, "log", "--oneline", "--no-merges", "origin/main", "^HEAD"
        )
        if ok and stray:
            first = stray.splitlines()[0]
            gaps.append(
                "origin/main carries %d commit(s) this branch has not, the newest "
                "being '%s'. Something landed on the released branch directly, so "
                "the release merge would either lose it or stop on a conflict. "
                "Merge origin/main into dev first." % (len(stray.splitlines()), first)
            )

    return gaps


# -------------------------------------------------------------- the self-test
#
# The parsers above are the part that can be wrong quietly: a regular
# expression that stops matching reports a clean gate rather than a failure,
# which is the one way a gate can be worse than no gate. So each is exercised
# against text it must read and against text it must not.


def self_test() -> int:
    failures = []

    def expect(label, got, want):
        if got != want:
            failures.append("%s: expected %r, got %r" % (label, want, got))

    expect(
        "workspace version is read from its own table",
        workspace_version(
            '[workspace.dependencies]\nclap = { version = "4.6.7" }\n\n'
            '[workspace.package]\nversion = "0.1.23"\nedition = "2021"\n'
        ),
        "0.1.23",
    )

    expect(
        "release headings, in file order",
        changelog_sections(
            "# Changelog\n\nSome prose.\n\n## v0.1.23\n\n- a change\n\n"
            "## Not a release\n\n## v0.1.22\n"
        ),
        ["v0.1.23", "v0.1.22"],
    )

    roadmap = (
        "| 166 | **A thing** | **done** | - |\n"
        "| 167 | **Another** | **planned** | 8 |\n"
        "| 181 | **A third** | done | - |\n"
        "\n### Next: v0.1.23, the window that never freezes\n\n"
        "1. **181**, an hour: the headings.\n"
        "2. **167**, the largest of the group.\n"
        "Fifteen items arrived at once, and 300 lines changed.\n"
        "\n### After that: v0.1.24, the parts that touch a file\n\n"
        "**173** and **185**.\n"
    )
    expect("only bold numbers inside the named section", release_items(roadmap, "v0.1.23"), ["181", "167"])
    expect("the next section is not read too", release_items(roadmap, "v0.1.24"), ["173", "185"])
    expect("a version with no section", release_items(roadmap, "v9.9.9"), [])
    expect("emphasised status", item_status(roadmap, "167"), "planned")
    expect("plain status", item_status(roadmap, "181"), "done")
    expect("no such row", item_status(roadmap, "999"), None)

    for line in failures:
        print("  " + line)
    print("%d failure(s)" % len(failures) if failures else "self-test passed")
    return 1 if failures else 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()

    root = repo_root()
    named = [a for a in sys.argv[1:] if not a.startswith("-")]
    if named:
        tag = named[0]
        if not tag.startswith("v"):
            tag = "v" + tag
    else:
        tag = "v" + workspace_version((root / "Cargo.toml").read_text(encoding="utf-8"))

    print("asking whether %s can be promoted from this tree" % tag)
    gaps = check(root, tag)
    if gaps:
        print()
        for gap in gaps:
            print("  not ready: " + gap)
        print()
        print("%d thing(s) stand between this tree and %s" % (len(gaps), tag))
        return 1
    print("ready: every gate this can ask about is satisfied for %s" % tag)
    return 0


if __name__ == "__main__":
    sys.exit(main())
