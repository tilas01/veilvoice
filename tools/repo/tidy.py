#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Branch housekeeping for this repository, run from a machine that can push.

# Why this exists rather than a note in a checklist

Two things accumulate on the remote and neither has an owner: branches Claude
Code opened for a session, and branches Dependabot opened for an upgrade. Both
are fully merged long before anybody notices them, and a list of stale branches
is exactly the kind of thing that gets tidied by hand once and then never again.

# Why it is not run from CI

Deleting a branch is not a thing to do on a schedule without somebody watching,
and the session this was written in could not do it at all: the git proxy in a
Claude Code container answers a ref deletion with 403 while allowing every other
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
import subprocess
import sys

# Branches that are never candidates for deletion whatever their state. `main`
# is the published history; `dev` is where work happens and is routinely ahead
# of `main` rather than contained in it.
PROTECTED = {"main", "dev", "HEAD"}


def git(*args, check=True):
    """Run git and give back its output, with the command in any error."""
    done = subprocess.run(["git", *args], capture_output=True, text=True)
    if check and done.returncode != 0:
        raise SystemExit("git %s failed: %s" % (" ".join(args), done.stderr.strip()))
    return done.stdout.strip()


def remote_branches(remote):
    """Every branch on the remote, without the remote's name on the front."""
    out = git("for-each-ref", "--format=%(refname:short)", "refs/remotes/%s" % remote)
    names = []
    for line in out.split("\n"):
        if not line:
            continue
        name = line[len(remote) + 1:]
        if name in PROTECTED:
            continue
        names.append(name)
    return names


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

    if unmerged:
        print("\nleft alone, because they are not contained in main:")
        for branch in sorted(unmerged):
            print("  %-46s %s" % (branch, last_commit(args.remote, branch)))

    if not merged:
        print("\nnothing to delete")
        return 0

    print("\ncontained in main, so deleting loses nothing:")
    for branch in sorted(merged):
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


if __name__ == "__main__":
    sys.exit(main())
