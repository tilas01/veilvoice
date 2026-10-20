#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Is the tip of `main` ready to be released, and is that version coherent?

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
binary has been compiled twice and compared.

So this asks the whole question **before** anything is built, from a checkout
of `main`, and it is the gate `promote.yml` runs first. There is one branch,
so the tree being asked about is the commit that would be released.

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

There was a sixth while development happened on a branch of its own and a
release merged it into `main`: that nothing had landed on `main` the
development branch lacked. With one branch there is nothing for `main` to have
that the tree being released has not, so the question is gone rather than
skipped.

Checks 4 and 5 need the repository's history, so they say so and are skipped
rather than guessed at when this runs somewhere without it.

Pure standard library, like everything in `tools/`.
"""

import os
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


def release_point(root: Path, tag: str):
    """The commit a previous release was cut at, and how that was decided.

    This exists because the tag cannot be trusted to be on the branch, and
    that is not hypothetical: every one of this repository's twenty-three
    release tags pointed at a commit reachable from no branch.
    The history was rewritten at some point and the tags were left behind on
    the pre-rewrite objects. The trees are identical, so what was published is
    what the branch has, but the commits are a parallel line. That is F-254.

    The first version of this function took the tag as the range boundary and
    asked whether `docs/AUDIT.md` had changed since. With an off-branch tag
    that range means "reachable from HEAD and not from the tag", which is most
    of the branch's history rather than the part after the release, so the
    question was answered by commits written *before* the release it was
    gating. It passed every time, which is the worst way for a gate to be
    wrong. That is F-255.

    Three answers are tried, in order of how much they prove.

    1. The tag, when it is genuinely an ancestor. That is what a healthy
       repository looks like and what this one will look like once the tags
       are moved.
    2. The commit on this branch whose **tree is the tag's tree**. An
       off-branch tag is still a record of exactly what was published, and
       content is what a release is. This is exact: for `v0.1.22` it names
       `ebde965f`, which is byte for byte the commit the tag points at.
    3. The commit that added that version's heading to `CHANGELOG.md`.

    The third is a fallback rather than the answer, and the reason is worth
    writing down because it was the answer first. Cutting a release is not one
    commit here: `Cut 0.1.22` wrote the heading at 17:23 and two more commits
    landed before the release was built at 18:00. Taking the heading commit
    therefore counts work that shipped *in* v0.1.22 as work since it, which is
    a smaller version of the same mistake the tag range made. It is kept for
    the case where the tag object is gone entirely, where being two commits
    early beats having no boundary at all, and it says so in its reason.
    """
    on_branch, _ = git(root, "merge-base", "--is-ancestor", tag, "HEAD")
    if on_branch:
        return tag, "the tag, which is on this branch"

    ok, tree = git(root, "rev-parse", tag + "^{tree}")
    if ok and tree.strip():
        # `--format=%T %H` rather than asking git to filter: there is no
        # revision selector for "same tree as", and the branch is a few
        # thousand commits.
        listed, log = git(root, "log", "--format=%T %H", "HEAD")
        if listed:
            want = tree.strip()
            for line in log.splitlines():
                parts = line.split()
                if len(parts) == 2 and parts[0] == want:
                    return parts[1], (
                        "%s is not on this branch, so the release point is the "
                        "commit here with the same tree as the tag, which is "
                        "the same content under a different parent" % tag
                    )

    found_ok, found = git(
        root, "log", "--reverse", "--format=%H", "-S", "## " + tag,
        "HEAD", "--", "CHANGELOG.md",
    )
    if found_ok and found.strip():
        return found.split()[0], (
            "%s is not on this branch and no commit here has its tree, so the "
            "release point is the commit that added its CHANGELOG.md heading. "
            "That is the start of cutting the release rather than the end of "
            "it, so a little of what shipped may be counted as since" % tag
        )
    return None, (
        "%s is not on this branch, no commit here has its tree, and no commit "
        "here adds a '## %s' heading to CHANGELOG.md." % (tag, tag)
    )


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
        under = "## Unreleased" in (root / "CHANGELOG.md").read_text(encoding="utf-8")
        gaps.append(
            "CHANGELOG.md has no '## %s' section, so the release would publish "
            "with no notes. The heading must be exactly '## %s'.%s"
            % (tag, tag, " The notes for it are under '## Unreleased'; renaming "
                         "that heading is part of the same commit as the version "
                         "bump." if under else "")
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
            "not a git checkout, so the audit round and the tag were not "
            "checked"
        )
        return gaps

    exists, _ = git(root, "rev-parse", "--verify", "--quiet", "refs/tags/" + tag)
    if exists:
        gaps.append(
            "the tag %s already exists, so this release has been cut. A second "
            "one would publish against the commit the tag already names." % tag
        )

    # The newest release tag, by version rather than by ancestry.
    ok, tags = git(root, "tag", "--list", "v*", "--sort=-v:refname")
    previous = tags.splitlines()[0].strip() if ok and tags.strip() else ""
    if not previous:
        gaps.append("no v* tag was found, so the audit round was not checked")
    else:
        boundary, how = release_point(root, previous)
        if not boundary:
            gaps.append(
                "the commit %s was cut at could not be identified, so the audit "
                "round was not checked. %s" % (previous, how)
            )
        else:
            ok, touched = git(
                root, "log", "--oneline", boundary + "..HEAD", "--", "docs/AUDIT.md"
            )
            if ok and not touched:
                gaps.append(
                    "docs/AUDIT.md has not changed since %s (%s), so no audit "
                    "round has been written up for anything in this release."
                    % (previous, how)
                )

    return gaps


# -------------------------------------------------------------- the self-test
#
# The parsers above are the part that can be wrong quietly: a regular
# expression that stops matching reports a clean gate rather than a failure,
# which is the one way a gate can be worse than no gate. So each is exercised
# against text it must read and against text it must not.


def release_point_self_test() -> list:
    """Every branch of `release_point`, against a repository built for it.

    The three answers differ by what they prove, so a test that only asserts
    the first one is the half of this function that was never wrong. A throwaway
    repository is cheap and it can be given the exact condition this repository
    is in: a tag on a commit that is on no branch, with the same tree as one
    that is.
    """
    import shutil
    import tempfile

    if shutil.which("git") is None:
        return ["release_point: no git on this machine, so it went unchecked"]

    problems = []
    work = Path(tempfile.mkdtemp(prefix="veilvoice-release-point-"))
    try:
        env = dict(os.environ)
        env.update(
            GIT_AUTHOR_NAME="t",
            GIT_AUTHOR_EMAIL="t@example.invalid",
            GIT_COMMITTER_NAME="t",
            GIT_COMMITTER_EMAIL="t@example.invalid",
        )

        def run(*args):
            subprocess.run(
                ("git",) + args, cwd=str(work), env=env,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True,
            )

        def commit(text, message):
            (work / "CHANGELOG.md").write_text(text, encoding="utf-8")
            run("add", "-A")
            run("commit", "-m", message)

        run("init", "-q", "-b", "main")
        commit("# Changelog\n", "first")
        commit("# Changelog\n\n## v1.0.0\n\n- a thing\n", "Cut 1.0.0")
        commit("# Changelog\n\n## v1.0.0\n\n- a thing\n- and another\n", "the rest of it")
        released = subprocess.run(
            ("git", "rev-parse", "HEAD"), cwd=str(work), env=env,
            stdout=subprocess.PIPE, check=True,
        ).stdout.decode().strip()

        # An on-branch tag: the answer is the tag itself.
        run("tag", "v1.0.0")
        at, how = release_point(work, "v1.0.0")
        if at != "v1.0.0" or "on this branch" not in how:
            problems.append("release_point: an ancestor tag should be used as-is, got %r" % (how,))

        # Now make it this repository's condition. An orphan commit carrying the
        # released tree is exactly what a rewritten history leaves behind: same
        # content, no parent in common with the branch.
        run("tag", "-d", "v1.0.0")
        orphan = subprocess.run(
            ("git", "commit-tree", released + "^{tree}", "-m", "the same content, elsewhere"),
            cwd=str(work), env=env, stdout=subprocess.PIPE, check=True,
        ).stdout.decode().strip()
        run("tag", "v1.0.0", orphan)

        ancestor = subprocess.run(
            ("git", "merge-base", "--is-ancestor", "v1.0.0", "HEAD"),
            cwd=str(work), env=env,
        ).returncode
        if ancestor == 0:
            problems.append("release_point: the fixture's tag is still an ancestor, so nothing below is tested")

        commit("# Changelog\n\n## v1.0.0\n\n- a thing\n- and another\n\nlater work\n", "after the release")
        at, how = release_point(work, "v1.0.0")
        if at != released:
            problems.append(
                "release_point: an off-branch tag should resolve to the commit "
                "with its tree (%s), got %r" % (released[:8], (at or "")[:8])
            )
        if "same tree" not in how:
            problems.append("release_point: the reason should say the tree matched, got %r" % (how,))

        # And the fallback, when the tag object is gone entirely. It lands on
        # the commit that wrote the heading, which is two commits early here,
        # and its reason has to admit that rather than sounding exact.
        run("tag", "-d", "v1.0.0")
        at, how = release_point(work, "v1.0.0")
        heading = subprocess.run(
            ("git", "log", "--reverse", "--format=%H", "-S", "## v1.0.0", "HEAD", "--", "CHANGELOG.md"),
            cwd=str(work), env=env, stdout=subprocess.PIPE, check=True,
        ).stdout.decode().split()
        if not heading or at != heading[0]:
            problems.append("release_point: with no tag it should fall back to the heading commit, got %r" % ((at or "")[:8],))
        if "may be counted as since" not in how:
            problems.append("release_point: the fallback must say it is approximate, got %r" % (how,))
    finally:
        shutil.rmtree(str(work), ignore_errors=True)
    return problems


def check_self_test() -> list:
    """The whole gate, over a repository with one branch that is ready.

    The readers above are tested on text and `release_point` on a fixture, and
    neither says whether `check` itself passes a release it should pass. That
    is the half a gate can be wrong about without anybody noticing, because a
    gate that refuses everything is fixed by hand on release morning and a
    gate that refuses one healthy layout looks like a release problem.

    The layout here is this repository's: one branch, a previous release
    tagged on it, `docs/AUDIT.md` written in since, and a remote-tracking
    `origin/main` holding a commit this checkout lacks, which is a clone that
    has simply not pulled. The gate used to fail exactly that, when a second
    branch existed and such a commit meant work put on the released branch
    directly; with one branch it is not a gap, and asking about it would
    refuse a ready release.
    """
    import shutil
    import tempfile

    if shutil.which("git") is None:
        return ["check: no git on this machine, so it went unchecked"]

    problems = []
    work = Path(tempfile.mkdtemp(prefix="veilvoice-readiness-"))
    try:
        env = dict(os.environ)
        env.update(
            GIT_AUTHOR_NAME="t",
            GIT_AUTHOR_EMAIL="t@example.invalid",
            GIT_COMMITTER_NAME="t",
            GIT_COMMITTER_EMAIL="t@example.invalid",
        )

        def run(*args):
            return subprocess.run(
                ("git",) + args, cwd=str(work), env=env,
                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, check=True,
            ).stdout.decode().strip()

        def write(version, notes, audit):
            (work / "docs").mkdir(exist_ok=True)
            (work / "Cargo.toml").write_text(
                '[workspace.package]\nversion = "%s"\n' % version, encoding="utf-8")
            (work / "CHANGELOG.md").write_text(
                "# Changelog\n\n" + notes, encoding="utf-8")
            (work / "ROADMAP.md").write_text(
                "| 1 | **A thing** | **done** | - |\n"
                "\n### Next: v1.0.1, the thing\n\n**1**.\n", encoding="utf-8")
            (work / "docs" / "AUDIT.md").write_text(audit, encoding="utf-8")
            run("add", "-A")
            run("commit", "-q", "-m", "version %s" % version)

        run("init", "-q", "-b", "main")
        write("1.0.0", "## v1.0.0\n\n- the first\n", "# Audit\n")
        run("tag", "v1.0.0")
        write("1.0.1", "## v1.0.1\n\n- the second\n\n## v1.0.0\n\n- the first\n",
              "# Audit\n\nF-1, read since v1.0.0.\n")
        here = run("rev-parse", "HEAD")
        run("commit", "-q", "--allow-empty", "-m", "pushed by somebody else")
        run("update-ref", "refs/remotes/origin/main", "HEAD")
        run("reset", "-q", "--hard", here)

        gaps = check(work, "v1.0.1")
        if gaps:
            problems.append("check: a ready release on one branch was refused: %r" % (gaps,))

        run("tag", "v1.0.1")
        if not any("already exists" in gap for gap in check(work, "v1.0.1")):
            problems.append("check: a release whose tag exists was not refused")
    finally:
        shutil.rmtree(str(work), ignore_errors=True)
    return problems


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

    failures.extend(release_point_self_test())
    failures.extend(check_self_test())

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
