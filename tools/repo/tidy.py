#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Branch housekeeping for this repository, run from a machine that can push.

# Why this exists rather than a note in a checklist

Two things accumulate on the remote and neither has an owner: branches a cloud
session opened for its work, and branches Dependabot opened for an upgrade. Both
are fully merged long before anybody notices them, and a list of stale branches
is exactly the kind of thing that gets tidied by hand once and then never again.

# Why it is not run from CI

Deleting a branch is not a thing to do on a schedule without somebody watching,
and the session this was written in could not do it at all: the git proxy in a
cloud container answers a ref deletion with 403 while allowing every other
push, so the deletion has to happen somewhere with ordinary credentials.

# What it will not do

It refuses to delete anything that is not already contained in `main`. That is
the whole safety property: a branch whose commits are all in `main` can be
recreated from `main`, so deleting it loses nothing, and a branch with even one
commit that is not is left alone and reported. `main` itself is never a
candidate, and neither is the current branch.

Dry run unless `--go` is passed, because a list of what would happen is the
useful output and the deletion is the rare one.

Pure standard library.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys

# Branches that are never candidates for deletion whatever their state. `main`
# is the published history; `dev` is where work happens and is routinely ahead
# of `main` rather than contained in it.
PROTECTED = {"main", "dev", "HEAD"}


def git(*args, check=True, cwd=None):
    """Run git and give back its output, with the command in any error."""
    done = subprocess.run(["git", *args], capture_output=True, text=True, cwd=cwd)
    if check and done.returncode != 0:
        raise SystemExit("git %s failed: %s" % (" ".join(args), done.stderr.strip()))
    return done.stdout.strip()


# The full name of each remote ref, then what it points at when it is a
# symbolic ref and nothing when it is a branch.
LISTING = "--format=%(refname) %(symref)"


def branch_names(listing, remote):
    """The branches in a `LISTING` of one remote, without its name on the front.

    F-242. This used `%(refname:short)` and cut the remote's name off the front
    of each line. Every clone has `refs/remotes/origin/HEAD`, a symbolic ref
    naming the remote's default branch, and git shortens that one to `origin`
    alone, because `origin` is enough to find it. Cutting eight characters off
    a six-character name left an empty branch name, which no check here
    recognised, so the dry run went on to ask git about `origin/` and stopped
    with "ambiguous argument". The tool failed on every fresh clone.

    So the full ref name is read, and a symbolic ref is skipped: it is another
    name for a branch already in the list, not a branch of its own.
    """
    prefix = "refs/remotes/%s/" % remote
    names = []
    for line in listing.split("\n"):
        ref, _, target = line.strip().partition(" ")
        if not ref or target or not ref.startswith(prefix):
            continue
        name = ref[len(prefix):]
        if name in PROTECTED:
            continue
        names.append(name)
    return names


def remote_branches(remote, cwd=None):
    """Every branch on the remote, without the remote's name on the front."""
    return branch_names(
        git("for-each-ref", LISTING, "refs/remotes/%s" % remote, cwd=cwd), remote)


def contained_in_main(remote, branch):
    """Whether every commit on this branch is already in the remote's main."""
    done = subprocess.run(
        ["git", "merge-base", "--is-ancestor",
         "%s/%s" % (remote, branch), "%s/main" % remote],
        capture_output=True, text=True)
    return done.returncode == 0


def last_commit(remote, branch):
    """When this branch was last touched, for the report."""
    return git("log", "-1", "--format=%cs  %h  %s",
               "%s/%s" % (remote, branch))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[1])
    parser.add_argument("--remote", default="origin", help="the remote to work on")
    parser.add_argument("--go", action="store_true",
                        help="actually delete; without it, only say what would go")
    parser.add_argument("--including", metavar="BRANCH", action="append", default=[],
                        help="also delete this branch even though it is not "
                             "contained in main, naming it explicitly. Repeatable. "
                             "What it would lose is printed first.")
    args = parser.parse_args()

    print("fetching %s" % args.remote)
    git("fetch", args.remote, "--prune")

    branches = remote_branches(args.remote)
    if not branches:
        print("nothing but the protected branches on %s" % args.remote)
        return 0

    merged, unmerged = [], []
    for branch in branches:
        (merged if contained_in_main(args.remote, branch) else unmerged).append(branch)

    # A branch named explicitly moves from the left-alone list to the delete
    # list, and what deleting it loses is printed rather than implied. This is
    # the escape hatch for a branch whose work arrived by another route: a
    # Dependabot branch whose upgrade was applied by hand, for instance, is
    # never contained in `main` and is still finished with.
    named = set(args.including)
    unknown = named - set(branches)
    if unknown:
        print("no such branch on %s: %s" % (args.remote, ", ".join(sorted(unknown))))
        return 1
    chosen = [b for b in unmerged if b in named]
    unmerged = [b for b in unmerged if b not in named]
    merged += chosen

    if chosen:
        print("\nnamed explicitly, and NOT contained in main. Deleting these")
        print("loses the commits listed, which exist nowhere else:")
        for branch in sorted(chosen):
            ahead = git("rev-list", "--count",
                        "%s/main..%s/%s" % (args.remote, args.remote, branch))
            print("  %-46s %s commit(s) not in main" % (branch, ahead))
            print("  %-46s %s" % ("", last_commit(args.remote, branch)))

    if unmerged:
        print("\nleft alone, because they are not contained in main:")
        for branch in sorted(unmerged):
            print("  %-46s %s" % (branch, last_commit(args.remote, branch)))

    if not merged:
        print("\nnothing to delete")
        return 0

    contained = [b for b in merged if b not in named]
    if contained:
        print("\ncontained in main, so deleting loses nothing:")
        for branch in sorted(contained):
            print("  %-46s %s" % (branch, last_commit(args.remote, branch)))

    if not args.go:
        print("\n%d branch(es) would go. Pass --go to delete them." % len(merged))
        return 0

    print()
    failed = 0
    for branch in sorted(merged):
        done = subprocess.run(["git", "push", args.remote, "--delete", branch],
                              capture_output=True, text=True)
        if done.returncode == 0:
            print("  deleted  %s" % branch)
        else:
            failed += 1
            print("  FAILED   %s: %s" % (branch, done.stderr.strip().split("\n")[-1]))
    return 1 if failed else 0


def self_test():
    """Read the branches of a real clone, made here. Returns the exit status.

    A real one rather than text, because the defect was in what git prints,
    and a sample written here would only be a record of what somebody
    believed it printed. The clone has the `origin/HEAD` every clone has.
    """
    import tempfile

    quiet = ["-c", "user.name=tidy", "-c", "user.email=tidy@localhost",
             "-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"]
    with tempfile.TemporaryDirectory() as scratch:
        upstream = os.path.join(scratch, "upstream")
        clone = os.path.join(scratch, "clone")
        git(*quiet, "init", "-q", upstream)
        git(*quiet, "commit", "-q", "--allow-empty", "-m", "first", cwd=upstream)
        git("branch", "-q", "finished", cwd=upstream)
        git("branch", "-q", "dev", cwd=upstream)
        git("clone", "-q", upstream, clone)
        listing = git("for-each-ref", LISTING, "refs/remotes/origin", cwd=clone)
        if "refs/remotes/origin/HEAD refs/remotes/origin/main" not in listing:
            print("the clone has no origin/HEAD, so this proves nothing:\n%s"
                  % listing)
            return 1
        found = remote_branches("origin", cwd=clone)
    if found != ["finished"]:
        print("a fresh clone's branches read as %r, not ['finished']" % (found,))
        return 1
    print("  a fresh clone's branches read as its branches, origin/HEAD and "
          "the protected ones left out")
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv[1:]:
        sys.exit(self_test())
    sys.exit(main())
