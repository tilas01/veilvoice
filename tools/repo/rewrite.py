#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Rewrite `dev`, and `main` where it is inside the range, so every commit is tilas01's.

    python tools/repo/rewrite.py --sign KEYID          # build it, check it, say what it did
    python tools/repo/rewrite.py --sign KEYID --push   # and force-push, with a lease
    python tools/repo/rewrite.py --no-sign --from auto

# Why this exists

Fifteen commits reached `dev` naming an assistant as author or committer, and
twenty-five of tilas01's own showed Unverified because a cloud container signed
them with a key registered to another account (F-235). Neither can be corrected
without new commits, and new commits mean a force push. This is that rewrite,
as a file rather than a command line, because it may have to be run on a
machine other than the one it was written on: the signing key decides where.

# What changes, and what cannot

For every commit in the range: author and committer become tilas01, with their
original dates; the message loses any co-author or session trailer and any
assistant name, by the replacements in `REWORD`; a title that names the wrong
finding is corrected, by `TITLE`; a title run straight into the lines below it
gets the blank line it was missing; a commit hash quoted in a message is
translated to the rewritten commit it named; and the commit is signed with the
key given, or left unsigned.

The tree of every commit is byte for byte what it was. That is checked, commit
by commit, before anything is pushed, and it is what keeps a build of any
rewritten commit identical to a build of the original: the release builds take
`SOURCE_DATE_EPOCH` from the committer date, and that is kept too.

No tag is touched. Every published release tag points at a commit outside the
history of both branches, so a rewrite of them moves no release.

# The range

`--from root` rewrites all of it, which is the only way every commit ends up
signed. `--from auto` starts at the first commit that is wrong: a name, a
message, or a signature that is not one of the accepted keys. With `--no-sign`
a missing signature is not counted as wrong, because an unsigned rewrite would
only strip signatures GitHub already verifies. `--from <commit>` starts there.

`main` is rewritten only as far as it is already an ancestor of `dev`, and ends
at the rewrite of the commit it was at. If `main` is before the range it is left
exactly where it is.

# After it

The map of old to new commits is written to `.git/rewrite-map.txt`. A clone
with work on top of the old `dev` moves it across with:

    git fetch origin
    git rebase --onto origin/dev OLD_DEV_TIP

where OLD_DEV_TIP is the first line this prints. Nothing else is needed: the
trees are identical, so the rebase has nothing to resolve.

Pure standard library, and GnuPG when signing.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "tools", "audit"))

import authorship  # noqa: E402

# Replacements for message text that names an assistant, in order. Each says
# what the original said without the name. `authorship` checks the result, so
# a message this does not cover stops the rewrite rather than going out.
REWORD = [
    ('The footer said "Drafted with help from Claude, Anthropic\'s assistant" on 346',
     "The footer carried a line crediting the assistance on 346"),
    ("CLAUDE.md added to NOT_A_WIKI_PAGE",
     "the local working-notes file added to NOT_A_WIKI_PAGE"),
    # "one per Claude Code session", "the git proxy in a Claude Code container"
    ("Claude Code", "cloud"),
]

# Titles that name the wrong finding: each commit was pushed while another
# session took the number in its title, and its write-up in docs/AUDIT.md was
# renumbered while the title was not. Keyed on the original commit.
TITLE = {
    "262307427dc6b3062c0252e4115aaebfa697f995": ("F-227:", "F-228:"),
    "b73ddd34bb08e74562f15aa13d6a29797b129d7a": ("F-233:", "F-234:"),
}


def git(args, env=None, check=True, stdin=None):
    done = subprocess.run(["git"] + args, cwd=ROOT, env=env, input=stdin,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if check and done.returncode != 0:
        raise SystemExit("git %s failed: %s" % (
            " ".join(args[:3]), done.stderr.decode("utf-8", "replace").strip()))
    return done.stdout.decode("utf-8", "replace").strip()


def headers(sha):
    """(message, author date, committer date) of a commit, dates in git's raw form."""
    raw = git(["cat-file", "commit", sha])
    head, _, message = raw.partition("\n\n")
    dates = {}
    for line in head.split("\n"):
        for role in ("author", "committer"):
            if line.startswith(role + " "):
                dates[role] = " ".join(line.rsplit(" ", 2)[-2:])
    return message, dates["author"], dates["committer"]


def reword(message, mapping, sha=None):
    """The message without trailers or names, and with hashes translated.

    A title followed straight by more text, with no blank line between, is
    given one: git reads everything up to the first blank line as the title,
    so without it the whole message shows as one line.
    """
    lines = [line for line in message.split("\n") if not authorship.TRAILER.search(line)]
    if sha in TITLE:
        wrong, right = TITLE[sha]
        if not lines[0].startswith(wrong):
            raise SystemExit("%s no longer starts %r; TITLE is stale" % (sha[:10], wrong))
        lines[0] = right + lines[0][len(wrong):]
    if len(lines) > 1 and lines[1].strip():
        lines.insert(1, "")
    text = "\n".join(lines)
    for old, new in REWORD:
        text = text.replace(old, new)
    text = re.sub(r"\n{3,}$", "\n", text.rstrip("\n")) + "\n"

    def translate(match):
        short = match.group(0)
        named = [new for old, new in mapping.items() if old.startswith(short)]
        # A run of digits is a number far more often than it is a commit.
        if len(named) == 1 and re.search(r"[a-f]", short):
            return named[0][:len(short)]
        return short
    return re.sub(r"\b[0-9a-f]{7,40}\b", translate, text)


def wrong(sha, signing):
    """Whether a commit has to be rewritten.

    A name or an address that is not tilas01's always does, and so does a
    signature by a key GitHub will not verify as his, which reads worse than
    none. A message that names an assistant, or a missing signature, counts
    only when this run signs: rewriting from there unsigned would strip every
    signature GitHub verifies after it, to fix a sentence.
    """
    faults, signature = authorship.commit_faults(ROOT, sha)
    identity = [f for f in faults if not f.startswith("message ")]
    if identity:
        return True
    if signature and signature != ["unsigned"]:
        return True
    return signing and bool(faults or signature)


def main():
    parser = argparse.ArgumentParser()
    how = parser.add_mutually_exclusive_group(required=True)
    how.add_argument("--sign", metavar="KEYID")
    how.add_argument("--no-sign", action="store_true")
    parser.add_argument("--from", dest="start", default=None,
                        help="root, auto or a commit; root when signing, auto when not")
    parser.add_argument("--push", action="store_true")
    parser.add_argument("--remote", default="origin")
    options = parser.parse_args()

    git(["fetch", "-q", options.remote, "dev", "main"])
    old_dev = git(["rev-parse", "%s/dev" % options.remote])
    old_main = git(["rev-parse", "%s/main" % options.remote])
    if subprocess.run(["git", "merge-base", "--is-ancestor", old_main, old_dev],
                      cwd=ROOT).returncode != 0:
        raise SystemExit("main has commits dev does not; merge those into dev first")
    if git(["rev-list", "--merges", "--count", old_dev]) != "0":
        raise SystemExit("dev has merge commits, which this does not rewrite")

    history = git(["rev-list", "--reverse", old_dev]).split()
    signing = options.sign is not None
    if options.start is None:
        options.start = "root" if signing else "auto"
    if options.start == "root":
        first = 0
    elif options.start == "auto":
        first = next((n for n, sha in enumerate(history) if wrong(sha, signing)), None)
        if first is None:
            print("nothing on dev needs rewriting")
            return 0
    else:
        start = git(["rev-parse", options.start + "^{commit}"])
        first = history.index(start)

    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("GIT_AUTHOR_", "GIT_COMMITTER_"))}
    sign = (["-c", "gpg.format=openpgp", "-c", "user.signingkey=" + options.sign,
             "commit-tree", "-S" + options.sign] if signing
            else ["commit-tree", "--no-gpg-sign"])

    mapping = {}
    for sha in history[first:]:
        message, authored, committed = headers(sha)
        parents = git(["rev-list", "--parents", "-n", "1", sha]).split()[1:]
        args = list(sign) + [sha + "^{tree}"]
        for parent in parents:
            args += ["-p", mapping.get(parent, parent)]
        step = dict(env, GIT_AUTHOR_NAME=authorship.NAME, GIT_AUTHOR_EMAIL=authorship.EMAIL,
                    GIT_AUTHOR_DATE=authored, GIT_COMMITTER_NAME=authorship.NAME,
                    GIT_COMMITTER_EMAIL=authorship.EMAIL, GIT_COMMITTER_DATE=committed)
        mapping[sha] = git(args, env=step,
                           stdin=reword(message, mapping, sha).encode("utf-8"))

    new_dev = mapping[old_dev]
    new_main = mapping.get(old_main, old_main)

    # Proof before anything leaves this machine.
    for old, new in mapping.items():
        if git(["rev-parse", old + "^{tree}"]) != git(["rev-parse", new + "^{tree}"]):
            raise SystemExit("the tree of %s changed in the rewrite; nothing pushed" % old[:10])
    failures, notes = authorship.audit(ROOT, [mapping[s] for s in history[first:]],
                                       required=signing, look_ahead=False)
    if failures:
        print("\n".join(failures))
        raise SystemExit("the rewritten history still fails the authorship guard; nothing pushed")

    with open(os.path.join(ROOT, ".git", "rewrite-map.txt"), "w") as handle:
        for old, new in mapping.items():
            handle.write("%s %s\n" % (old, new))
    git(["update-ref", "refs/rewrite/dev", new_dev])
    git(["update-ref", "refs/rewrite/main", new_main])

    print("old dev tip   %s" % old_dev)
    print("dev   %s -> %s" % (old_dev[:10], new_dev[:10]))
    print("main  %s -> %s%s" % (old_main[:10], new_main[:10],
                                 "" if new_main != old_main else "  (unchanged)"))
    print("%d commit(s) rewritten from %s, every tree identical, %s"
          % (len(mapping), history[first][:10],
             "every one signed with %s" % options.sign if signing else "unsigned"))
    if notes and signing:
        print("\n".join(notes))

    if not options.push:
        print("dry run: the result is at refs/rewrite/dev and refs/rewrite/main; --push sends it")
        return 0
    leases = ["--force-with-lease=dev:%s" % old_dev]
    specs = ["%s:refs/heads/dev" % new_dev]
    if new_main != old_main:
        leases.append("--force-with-lease=main:%s" % old_main)
        specs.append("%s:refs/heads/main" % new_main)
    git(["push", "--atomic"] + leases + [options.remote] + specs)
    print("pushed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
