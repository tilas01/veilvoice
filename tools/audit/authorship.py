#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Every commit is tilas01's in both of its fields, names no assistant, and is signed.

    python tools/audit/authorship.py                  # what this clone would push
    python tools/audit/authorship.py --range A..B     # an explicit range
    python tools/audit/authorship.py --pushed BEFORE AFTER   # a push, as CI sees it
    python tools/audit/authorship.py --pushed BEFORE AFTER --github   # and GitHub's verdict
    python tools/audit/authorship.py --self-test

# The defect this exists to stop coming back

Fifteen commits reached `dev` between 21 and 26 September naming an assistant
rather than tilas01: eleven as author and committer, four as committer only.
A cloud container presets its own name and address for git, in a config file
and in some containers in four `GIT_*` environment variables as well, and the
variables beat every config file, so a plain `git commit` there is wrong by
default and looks right. Four of the fifteen were found only because somebody
read the committer rather than the author, which is the field a log shows
least often and the one GitHub verifies a signature against.

Twenty-five more carried tilas01's name correctly and showed Unverified on
GitHub, because the container signs every commit with a key registered to a
different GitHub account. A signature GitHub cannot connect to the committer
reads worse than no signature at all. That is F-235.

# What is checked

**The commit that has not been made yet.** `git var GIT_AUTHOR_IDENT` and
`GIT_COMMITTER_IDENT` answer what the next commit will say, with the
environment taken into account, which is the only way to see the variables that
beat the config. The signing config says which key will sign it, and a key not
in `SIGNING_KEYS` is refused here even while unsigned commits are allowed: a
container's own key in tilas01's name makes a commit read Unverified, which is
worse than no badge. `tools/verify.py` runs before a commit is made, so this is the
half that stops the next one rather than reporting it afterwards.

**Every commit in the range.** Both addresses belong to tilas01's account.
The committer may also be GitHub itself, under the name GitHub, which is what
GitHub records when it makes a commit on his behalf: an edit in the website,
or a merge there. No name in either field names an assistant, whatever the
address. The
message carries no co-author trailer, no session trailer or link, no
"generated with" footer, and no assistant or model name anywhere, because
credit for assistance lives in the README and the website footer and nowhere
else. Its tree holds no instruction file for an AI assistant (`CLAUDE.md`,
`AGENTS.md`, `.claude/` and the rest of `ASSISTANT_FILES`), which `.gitignore`
keeps out of a plain `git add` and this catches when one is added anyway.
And its title stands alone: a message longer than one line has a blank line
after the first, because git takes everything before the first blank line as
the title, and a body run on beneath it becomes one enormous title (F-248).

**The signature.** Every commit carries one, made by a key in
`SIGNING_KEYS`: keys registered to tilas01's GitHub account, which is what
makes GitHub show a commit Verified. The key is read out of the signature
itself, so this needs neither GnuPG nor the public key, and it names the key
it found when it is the wrong one. While `SIGNATURES_REQUIRED` is off, an
unsigned or wrongly signed commit is reported and does not fail.

# Which commits

By default, the commits this clone would push: reachable from HEAD and from
no remote-tracking branch, or all of HEAD in a clone with no remote at all. In
CI, `--pushed` takes the push event's before and after. A new branch has no
meaningful before, so everything it has and `main` lacks is checked. A force
push is checked whole: every commit the branch now has, because a rewrite can
replace commits `main` shares with it, and each needs GitHub's verdict.

# What this deliberately does not do

It does not ask GitHub whether a commit is Verified unless `--github` is given.
That answer exists only after a push, so it cannot stop one; CI asks it of
every pushed commit, which is the whole of what `--github` is for. It does not check
the cryptography of a signature either, for the same reason: GitHub does, and
a local check with a copy of the public key would be a second answer to a
question with one authority.

It does not read history that is already on a remote. A rule applied
backwards is a force push, and that is a decision about a branch rather than
something a guard should imply.

Pure standard library.
"""

from __future__ import annotations

import base64
import hashlib
import os
import re
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# The one identity a commit made from a clone carries.
NAME = "tilas01"
EMAIL = "tilas01@users.noreply.github.com"

# Every address GitHub attributes to tilas01's account. The second is the one
# GitHub itself writes when he commits through the website.
ACCOUNT_EMAILS = {EMAIL, "95286414+tilas01@users.noreply.github.com"}

# GitHub, as committer of a commit it made for him. It signs those itself, with
# its own key, and shows them Verified.
GITHUB_COMMITTER = "noreply@github.com"

# Keys registered to tilas01's GitHub account as signing keys, by the
# fingerprint a signature names. The comment says where each one lives.
SIGNING_KEYS = {
    # OpenPGP, RSA 4096. On his own computer. GitHub verifies 0c6dde9 and the
    # fifteen commits before it by this key.
    "openpgp:DB74FC8D4C7E94AA2CF74B53F8ADBAB7A4FB333C": "tilas01's own key",
    # OpenPGP, GitHub's own key, for the commits it makes for him in the
    # website. Its signatures carry only the key ID, B5690EEEBB952194, which
    # is what `accepted` matches against the tail of this.
    "openpgp:968479A1AFF927E37D1A566BB5690EEEBB952194": "GitHub, for its website",
    # OpenPGP, RSA 4096, primary 8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A: the
    # release key, held in the repository's GPG_PRIVATE_KEY secret. This is its
    # signing subkey, which is the fingerprint a signature carries. Workflows
    # sign with it; .github/workflows/signing.yml asks GitHub for its verdict.
    "openpgp:83ABD67A93772B3FA15D2642BD752A144704C231": "tilas01's release key, in Actions",
}

# The key a cloud container signs with. Registered to a different account, so
# a commit it signs in tilas01's name reads Unverified. Named so the report
# says what it found rather than an opaque fingerprint.
KNOWN_ELSEWHERE = {
    "ssh:SHA256:32dP45eSMmVSt/G/CGvcxl/P+MO3Nwj9xeTh/GSA2wc":
        "the cloud container's own key, registered to another account",
}

# Off until every place a commit is made from can sign with a key above. While
# off, signature findings are printed and do not fail.
SIGNATURES_REQUIRED = False

# Trailer and footer lines a tool adds when it writes a commit for somebody.
# Matched a line at a time.
TRAILER = re.compile(
    r"^\s*(?:co-authored-by|claude-session)\s*:"
    r"|^\s*(?:\U0001F916\s*)?generated with \["
    r"|https?://claude\.ai/",
    re.IGNORECASE)

# An assistant or its maker by name, or a model by name and version. The model
# names are ordinary words on their own, so a version has to follow, and only
# one shaped like a model's: a single digit from 3, with at most one decimal.
# "Opus" is also the audio codec this program handles, and "Opus 48 kHz" or
# "Opus 1.5" is about the codec.
ASSISTANT = re.compile(
    r"\bclaude\b|\banthropic\b|\b(?:opus|sonnet|haiku|fable)[ -]?[3-9](?:\.\d)?\b",
    re.IGNORECASE)


# Instruction files for AI coding assistants, by the names the common ones
# use. None belongs in this repository on either branch: `.gitignore` keeps
# them out of a plain `git add`, and this catches one added with `-f`.
ASSISTANT_FILES = re.compile(
    r"(?:^|/)(?:CLAUDE(?:\.local)?\.md|AGENTS\.md|GEMINI\.md|\.cursorrules"
    r"|\.windsurfrules|\.clinerules)$"
    r"|^\.github/(?:copilot-instructions\.md$|instructions/)"
    r"|(?:^|/)\.(?:claude[^/]*|cursor|windsurf|clinerules|roo|continue|codex|gemini"
    r"|aider[^/]*)/",
    re.IGNORECASE)


def git(args, cwd, check=False):
    """Run git and return stdout as text. Raises only when asked to."""
    done = subprocess.run(["git"] + args, cwd=cwd, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE)
    if check and done.returncode != 0:
        raise RuntimeError("git %s: %s" % (" ".join(args),
                                           done.stderr.decode("utf-8", "replace").strip()))
    return done.stdout.decode("utf-8", "replace")


def ident(line):
    """`Name <email> 1790000000 +0000` -> (name, email)."""
    match = re.match(r"^(.*?) <([^>]*)>", line.strip())
    return (match.group(1), match.group(2)) if match else (line.strip(), "")


def openpgp_signer(armoured):
    """The fingerprint (or, failing that, the key ID) a signature names.

    Reads the one signature packet in an ASCII-armoured detached signature:
    the issuer fingerprint subpacket (33) where there is one, the issuer key
    ID (16) where there is not. None when it cannot be read.
    """
    body = []
    inside = False
    for line in armoured.splitlines():
        if line.startswith("-----BEGIN"):
            inside = True
            continue
        if line.startswith("-----END"):
            break
        if not inside or ":" in line or not line.strip():
            continue
        if line.startswith("="):
            continue
        body.append(line.strip())
    try:
        data = base64.b64decode("".join(body))
    except ValueError:
        return None
    if not data or not data[0] & 0x80:
        return None
    # Packet header, old or new format.
    if data[0] & 0x40:
        at = 1
        first = data[at]
        if first < 192:
            at += 1
        elif first < 224:
            at += 2
        else:
            at += 5
    else:
        at = 1 + {0: 1, 1: 2, 2: 4}.get(data[0] & 0x03, 0)
    if at >= len(data) or data[at] != 4:
        return None
    at += 4
    found = {}
    for _ in range(2):  # hashed, then unhashed subpackets
        if at + 2 > len(data):
            return None
        size = struct.unpack(">H", data[at:at + 2])[0]
        at += 2
        end = at + size
        while at < end:
            first = data[at]
            if first < 192:
                length, at = first, at + 1
            elif first < 255:
                length, at = ((first - 192) << 8) + data[at + 1] + 192, at + 2
            else:
                length, at = struct.unpack(">I", data[at + 1:at + 5])[0], at + 5
            kind = data[at] & 0x7F
            value = data[at + 1:at + length]
            if kind == 33 and value:
                found.setdefault("fpr", value[1:].hex().upper())
            elif kind == 16:
                found.setdefault("keyid", value.hex().upper())
            at += length
        at = end
    if "fpr" in found:
        return "openpgp:" + found["fpr"]
    if "keyid" in found:
        return "openpgp:" + found["keyid"]
    return None


def ssh_signer(armoured):
    """`ssh:SHA256:...` for the public key an SSHSIG blob carries."""
    lines = [line.strip() for line in armoured.splitlines()
             if line.strip() and not line.startswith("-----")]
    try:
        blob = base64.b64decode("".join(lines))
    except ValueError:
        return None
    if blob[:6] != b"SSHSIG" or len(blob) < 14:
        return None
    size = struct.unpack(">I", blob[10:14])[0]
    key = blob[14:14 + size]
    digest = base64.b64encode(hashlib.sha256(key).digest()).decode().rstrip("=")
    return "ssh:SHA256:" + digest


def signature_of(raw):
    """The signer of a raw commit object, None if unsigned, '?' if unreadable."""
    lines = raw.split("\n")
    collecting = None
    for line in lines:
        if collecting is not None:
            if line.startswith(" "):
                collecting.append(line[1:])
                continue
            break
        if line == "":
            break
        if line.startswith("gpgsig ") or line.startswith("gpgsig-sha256 "):
            collecting = [line.split(" ", 1)[1]]
    if collecting is None:
        return None
    armoured = "\n".join(collecting)
    if "BEGIN SSH SIGNATURE" in armoured:
        return ssh_signer(armoured) or "?"
    return openpgp_signer(armoured) or "?"


def accepted(signer, keys):
    """Whether a signer names a key in `keys`, by fingerprint or by its tail."""
    if signer in keys:
        return True
    # A signature that carries only a key ID names the last 16 hex digits.
    if signer and signer.startswith("openpgp:") and len(signer) == 24:
        return any(k.startswith("openpgp:") and k.endswith(signer[8:]) for k in keys)
    return False


def commit_faults(cwd, sha, keys=None):
    """(faults, signature_faults) for one commit, each a list of sentences."""
    keys = SIGNING_KEYS if keys is None else keys
    raw = git(["cat-file", "commit", sha], cwd, check=True)
    head, _, message = raw.partition("\n\n")
    author = committer = ("", "")
    for line in head.split("\n"):
        if line.startswith("author "):
            author = ident(line[7:])
        elif line.startswith("committer "):
            committer = ident(line[10:])
    faults = []
    if author[1] not in ACCOUNT_EMAILS:
        faults.append("author is %s <%s>, not tilas01" % author)
    if committer[1] not in ACCOUNT_EMAILS and committer[1] != GITHUB_COMMITTER:
        faults.append("committer is %s <%s>, not tilas01" % committer)
    # GitHub's address is GitHub's only under GitHub's name. Anybody can write
    # it into a commit, so the address alone proves nothing.
    if committer[1] == GITHUB_COMMITTER and committer[0] != "GitHub":
        faults.append("committer is %s <%s>, and only GitHub commits under that address"
                      % committer)
    # An assistant's name is an assistant's name whatever address it carries.
    for role, (name, email) in (("author", author), ("committer", committer)):
        if ASSISTANT.search(name):
            faults.append("%s name %r names an assistant" % (role, name))
    # git reads everything up to the first blank line as the title, so a body
    # that starts on the line after it becomes part of the title: in
    # `git log --oneline`, in GitHub's commit list and on the releases page.
    lines = message.split("\n")
    if len(lines) > 1 and lines[1].strip():
        faults.append("message line 2 follows the title with no blank line between, "
                      "so the whole paragraph reads as the title")
    for number, line in enumerate(lines, 1):
        if TRAILER.search(line):
            faults.append("message line %d carries %r" % (number, line.strip()[:72]))
            continue
        found = ASSISTANT.search(line)
        if found:
            faults.append("message line %d names %r: %s"
                          % (number, found.group(0), line.strip()[:72]))
    for path in git(["ls-tree", "-r", "--name-only", sha], cwd).split("\n"):
        if path and ASSISTANT_FILES.search(path):
            faults.append("tree carries %s, an assistant's instruction file" % path)
    signature = []
    signer = signature_of(raw)
    if signer is None:
        signature.append("unsigned")
    elif signer == "?":
        signature.append("carries a signature this cannot read")
    elif not accepted(signer, keys):
        what = KNOWN_ELSEWHERE.get(signer, "a key not registered to tilas01's account")
        signature.append("signed by %s (%s)" % (what, signer))
    return faults, signature


def next_commit_faults(cwd):
    """What the next commit made here would say, before it is made."""
    faults = []
    for role, var in (("author", "GIT_AUTHOR_IDENT"), ("committer", "GIT_COMMITTER_IDENT")):
        line = git(["var", var], cwd).strip()
        if not line:
            continue
        name, email = ident(line)
        if (name, email) == (NAME, EMAIL):
            continue
        preset = [var for var in ("GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL",
                                  "GIT_COMMITTER_NAME", "GIT_COMMITTER_EMAIL")
                  if var in os.environ]
        if preset:
            fix = ("this environment sets %s, which beat every config file, so\n"
                   "      export all four in the shell this runs in:\n"
                   "        export GIT_AUTHOR_NAME=%s GIT_AUTHOR_EMAIL=%s\n"
                   "        export GIT_COMMITTER_NAME=%s GIT_COMMITTER_EMAIL=%s"
                   % (", ".join(preset), NAME, EMAIL, NAME, EMAIL))
        else:
            fix = ("set it for this clone:\n"
                   "        git config user.name %s && git config user.email %s"
                   % (NAME, EMAIL))
        faults.append("the next commit here would have %s %s <%s>; %s"
                      % (role, name, email, fix))
        break
    signer = next_signer(cwd)
    if signer and not accepted(signer, SIGNING_KEYS):
        what = KNOWN_ELSEWHERE.get(signer, "a key not registered to tilas01's account")
        faults.append("the next commit here would be signed by %s (%s), which GitHub\n"
                      "      shows Unverified in tilas01's name. Run\n"
                      "        python tools/repo/signing.py\n"
                      "      which signs with the project's key, or, while there is none, turns\n"
                      "      signing off for this clone" % (what, signer))
    return faults


def next_signer(cwd):
    """The key the next commit here would be signed with, or None if unsigned.

    Read from the config the way git reads it. An SSH key is named by its
    fingerprint when ssh-keygen can read the key file, and as an unknown SSH
    key otherwise, which no entry in SIGNING_KEYS matches.
    """
    if git(["config", "--bool", "commit.gpgsign"], cwd).strip() != "true":
        return None
    form = git(["config", "gpg.format"], cwd).strip() or "openpgp"
    key = git(["config", "user.signingkey"], cwd).strip()
    if form == "openpgp":
        key = key.upper().replace("0X", "").rstrip("!")
        return "openpgp:" + (key or "the default key")
    if form == "ssh":
        # ssh-keygen names a key the way a signature does; without it, or
        # without a readable key, the key is unknown, which no entry matches.
        if key.startswith("key::"):
            args, given = ["ssh-keygen", "-lf", "-"], key[5:].encode()
        else:
            args, given = ["ssh-keygen", "-lf", os.path.expanduser(key)], None
        try:
            done = subprocess.run(args, input=given, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE) if key else None
        except OSError:
            done = None
        words = done.stdout.decode().split() if done and done.returncode == 0 else []
        return "ssh:" + (words[1] if len(words) > 1 else "an SSH key this cannot read")
    return "%s:%s" % (form, key or "the default key")


def unpushed(cwd):
    """Commits reachable from HEAD and from no remote-tracking branch."""
    if not git(["rev-parse", "--verify", "-q", "HEAD"], cwd).strip():
        return []
    remotes = git(["for-each-ref", "--format=%(refname)", "refs/remotes"], cwd).split()
    if not remotes:
        # Nothing here has been pushed anywhere this clone knows of, so all of
        # it is what a push would send.
        return git(["rev-list", "--reverse", "HEAD"], cwd).split()
    return git(["rev-list", "--reverse", "HEAD", "--not", "--remotes"], cwd).split()


def pushed(cwd, before, after):
    """The commits a push added, as CI sees it."""
    zero = not before or set(before) == {"0"}
    if not zero:
        known = git(["cat-file", "-t", before], cwd).strip() == "commit"
        forward = known and subprocess.run(
            ["git", "merge-base", "--is-ancestor", before, after], cwd=cwd,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
        if forward:
            return git(["rev-list", "--reverse", "%s..%s" % (before, after)], cwd).split()
        # A force push: every commit the branch now has may be new, including
        # those main has too when main was rewritten with it, so all of them.
        return git(["rev-list", "--reverse", after], cwd).split()
    # A new branch: everything the pushed commit has that main lacks.
    base = "origin/main"
    if not git(["rev-parse", "--verify", "-q", base], cwd).strip():
        return git(["rev-list", "--reverse", after], cwd).split()
    return git(["rev-list", "--reverse", after, "--not", base], cwd).split()


def github_verdicts(commits):
    """(sha, verified, reason) from GitHub for each commit, in CI.

    Needs GITHUB_REPOSITORY and GITHUB_TOKEN, which a workflow has. The one
    authority on whether GitHub shows a commit Verified is GitHub, so this asks
    it rather than reproducing its rules.
    """
    import json
    import urllib.request

    repository = os.environ["GITHUB_REPOSITORY"]
    token = os.environ.get("GITHUB_TOKEN", "")
    out = []
    for sha in commits:
        request = urllib.request.Request(
            "https://api.github.com/repos/%s/commits/%s" % (repository, sha),
            headers={"Accept": "application/vnd.github+json",
                     "Authorization": "Bearer " + token} if token else {})
        with urllib.request.urlopen(request, timeout=30) as response:
            verification = json.load(response)["commit"]["verification"]
        out.append((sha, bool(verification.get("verified")),
                    verification.get("reason", "")))
    return out


def audit(cwd, commits, keys=None, required=None, look_ahead=True):
    """(failures, notes) over some commits: lists of lines to print."""
    required = SIGNATURES_REQUIRED if required is None else required
    failures = []
    notes = []
    if look_ahead:
        for fault in next_commit_faults(cwd):
            failures.append("  %s" % fault)
    for sha in commits:
        faults, signature = commit_faults(cwd, sha, keys)
        subject = git(["log", "-1", "--format=%h %s", sha], cwd).strip()[:70]
        if faults:
            failures.append("  %s" % subject)
            failures.extend("      %s" % fault for fault in faults)
        if signature:
            target = failures if required else notes
            target.append("  %s" % subject)
            target.extend("      %s" % fault for fault in signature)
    return failures, notes


def self_test():
    """Prove each check fires, against commits built to break it."""
    import shutil
    import tempfile

    where = tempfile.mkdtemp()
    base = dict(os.environ)
    for var in ("GIT_AUTHOR_NAME", "GIT_AUTHOR_EMAIL", "GIT_COMMITTER_NAME",
                "GIT_COMMITTER_EMAIL", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM"):
        base.pop(var, None)
    base["GIT_CONFIG_GLOBAL"] = os.devnull
    base["GIT_CONFIG_SYSTEM"] = os.devnull

    def run(args, env=None):
        subprocess.run(["git", "-C", where] + args, check=True, env=env or base,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def make(message, author=(NAME, EMAIL), committer=(NAME, EMAIL), extra=None):
        env = dict(base)
        env.update({"GIT_AUTHOR_NAME": author[0], "GIT_AUTHOR_EMAIL": author[1],
                    "GIT_COMMITTER_NAME": committer[0],
                    "GIT_COMMITTER_EMAIL": committer[1]})
        with open(os.path.join(where, "f"), "a") as handle:
            handle.write("x\n")
        run(["add", "f"], env)
        if extra:
            os.makedirs(os.path.join(where, os.path.dirname(extra) or "."), exist_ok=True)
            with open(os.path.join(where, extra), "w") as handle:
                handle.write("notes\n")
            run(["add", "-f", extra], env)
        run(["commit", "-q", "--no-gpg-sign", "-m", message], env)
        return git(["rev-parse", "HEAD"], where).strip()

    bot = "Claude"
    maker = "Anthropic"
    elsewhere = (bot, "noreply@anthropic.com")
    cases = [
        ("an assistant as author and committer",
         dict(message="Fix", author=elsewhere, committer=elsewhere),
         "author is %s" % bot),
        ("an assistant as committer only",
         dict(message="Fix", committer=elsewhere),
         "committer is %s" % bot),
        ("a co-author trailer",
         dict(message="Fix\n\nCo-Authored-By: %s <noreply@x>" % bot),
         "Co-Authored-By"),
        ("a session trailer",
         dict(message="Fix\n\nClaude-Session: https://claude.ai/code/session_1"),
         "Claude-Session"),
        ("a generated-with footer",
         dict(message="Fix\n\n\U0001F916 Generated with [Claude Code](https://x)"),
         "Generated with"),
        ("the assistant named in the prose",
         dict(message="Fix\n\nThe proxy in a Claude Code container refused it"),
         "names %r" % bot),
        ("its notes file named in the prose",
         dict(message="Fix\n\n- CLAUDE.md added to the list"),
         "names 'CLAUDE'"),
        ("the maker named",
         dict(message="Fix\n\nWith help from %s" % maker),
         "names %r" % maker),
        ("a model by name and number",
         dict(message="Fix\n\nWritten with Opus 5"),
         "names 'Opus 5'"),
        ("a model by name and decimal version",
         dict(message="Fix\n\nWritten with Sonnet 4.5"),
         "names 'Sonnet 4.5'"),
        ("an assistant's name at GitHub's address",
         dict(message="Fix", committer=(bot, GITHUB_COMMITTER)),
         "only GitHub commits under that address"),
        ("a title run straight into its body",
         dict(message="Fix\n- the change"),
         "no blank line"),
        ("an address that is nobody's",
         dict(message="Fix", author=(NAME, "someone@example.com")),
         "not tilas01"),
        # Last: every commit after it would carry the file too.
        ("an assistant's instruction file in the tree",
         dict(message="Notes", extra="AGENTS.md"),
         "AGENTS.md, an assistant's instruction file"),
    ]
    failures = 0
    try:
        run(["init", "-q", "-b", "main"])
        make("Start")
        for name, spec, expected in cases:
            sha = make(**spec)
            faults, _ = commit_faults(where, sha)
            text = "\n".join(faults)
            if expected in text:
                print("    caught: %s" % name)
            else:
                failures += 1
                print("    MISSED: %s" % name)
                print("      expected %r in: %s" % (expected, text or "(clean)"))

        run(["rm", "-q", "-f", "AGENTS.md"])

        # Words that must pass: "marker" is a real word here, "regenerated
        # with" is not a footer, a model name with no number is a word, and
        # GitHub as committer is GitHub making a commit for him.
        clean = [
            dict(message="The session marker, regenerated with the new seed"),
            dict(message="A sonnet, an opus and a haiku walk into a changelog"),
            dict(message="Accept Opus 48 kHz input, as the Opus 1.5 decoder reads it"),
            dict(message="A title\n\n- one change\n- and another"),
            dict(message="Edited on the website",
                 author=("Starlight", "95286414+tilas01@users.noreply.github.com"),
                 committer=("GitHub", GITHUB_COMMITTER)),
        ]
        for spec in clean:
            sha = make(**spec)
            faults, signature = commit_faults(where, sha)
            if faults:
                failures += 1
                print("    MISSED: a sound commit was refused: %s" % "; ".join(faults))
        if not failures:
            print("    clean: five sound commits, one of them made by GitHub")

        # Unsigned: reported while signatures are optional, failing once not.
        sha = git(["rev-parse", "HEAD"], where).strip()
        failed, noted = audit(where, [sha], required=False, look_ahead=False)
        if failed or not any("unsigned" in line for line in noted):
            failures += 1
            print("    MISSED: an unsigned commit should be a note while optional")
        failed, _ = audit(where, [sha], required=True, look_ahead=False)
        if not any("unsigned" in line for line in failed):
            failures += 1
            print("    MISSED: an unsigned commit should fail once required")
        else:
            print("    caught: an unsigned commit, once signatures are required")

        # Which commits a push would carry. With no remote at all, every one.
        everything = git(["rev-list", "--reverse", "HEAD"], where).split()
        if unpushed(where) == everything:
            print("    caught: a clone with no remote checks all of its history")
        else:
            failures += 1
            print("    MISSED: a clone with no remote should check every commit")
        # A force push is checked whole, even where main has the same commits.
        run(["update-ref", "refs/remotes/origin/main", everything[1]])
        gone = make("Replaced by the force push")
        run(["reset", "-q", "--hard", "HEAD~1"])
        tip = make("What the branch is now")
        whole = git(["rev-list", "--reverse", tip], where).split()
        if pushed(where, gone, tip) == whole:
            print("    caught: a force push is checked commit by commit, all of it")
        else:
            failures += 1
            print("    MISSED: a force push should check every commit the branch has")

        # The identity of the next commit, which is what the variables decide.
        env = dict(base)
        env.update({"GIT_AUTHOR_NAME": bot, "GIT_AUTHOR_EMAIL": "noreply@x",
                    "GIT_COMMITTER_NAME": bot, "GIT_COMMITTER_EMAIL": "noreply@x"})
        run(["config", "user.name", NAME])
        run(["config", "user.email", EMAIL])
        saved = dict(os.environ)
        os.environ.clear()
        os.environ.update(env)
        try:
            ahead = next_commit_faults(where)
        finally:
            os.environ.clear()
            os.environ.update(saved)
        if ahead and "the next commit here would have" in ahead[0]:
            print("    caught: variables that beat a correct config")
        else:
            failures += 1
            print("    MISSED: the environment's identity should be refused")

        # The key the next commit would be signed with: a container's own SSH
        # key, set the way a container sets it, and then signing switched off.
        saved = dict(os.environ)
        os.environ.clear()
        os.environ.update(base)
        try:
            run(["config", "commit.gpgsign", "true"])
            run(["config", "gpg.format", "ssh"])
            run(["config", "user.signingkey", os.path.join(where, "missing.pub")])
            foreign = next_commit_faults(where)
            run(["config", "commit.gpgsign", "false"])
            off = next_commit_faults(where)
        finally:
            os.environ.clear()
            os.environ.update(saved)
        if any("would be signed by" in f for f in foreign) and not off:
            print("    caught: a signing key GitHub does not know as tilas01's")
        else:
            failures += 1
            print("    MISSED: a foreign signing key should be refused, and none passed "
                  "(%r, %r)" % (foreign, off))
    finally:
        shutil.rmtree(where, ignore_errors=True)

    # The signature readers, against the two kinds this repository has seen:
    # a version 4 OpenPGP signature packet built here, carrying the issuer
    # fingerprint hashed and the key ID unhashed as GnuPG writes them, and an
    # SSHSIG blob.
    fpr = bytes.fromhex("DB74FC8D4C7E94AA2CF74B53F8ADBAB7A4FB333C")
    hashed = bytes([22, 33, 4]) + fpr
    unhashed = bytes([9, 16]) + fpr[-8:]
    body = (bytes([4, 0, 1, 10]) + struct.pack(">H", len(hashed)) + hashed
            + struct.pack(">H", len(unhashed)) + unhashed + b"\x63\xed")
    packet = bytes([0x89]) + struct.pack(">H", len(body)) + body
    pgp = "-----BEGIN PGP SIGNATURE-----\n\n%s\n=AAAA\n-----END PGP SIGNATURE-----" % (
        base64.b64encode(packet).decode())
    if openpgp_signer(pgp) == "openpgp:" + fpr.hex().upper():
        print("    read: the key an OpenPGP signature names")
    else:
        failures += 1
        print("    MISSED: the OpenPGP reader got %r" % openpgp_signer(pgp))
    key = b"\x00\x00\x00\x0bssh-ed25519\x00\x00\x00\x20" + bytes(range(32))
    blob = b"SSHSIG" + struct.pack(">I", 1) + struct.pack(">I", len(key)) + key
    armoured = "-----BEGIN SSH SIGNATURE-----\n%s\n-----END SSH SIGNATURE-----" % (
        base64.b64encode(blob).decode())
    wanted = "ssh:SHA256:" + base64.b64encode(hashlib.sha256(key).digest()).decode().rstrip("=")
    if ssh_signer(armoured) == wanted:
        print("    read: the key an SSH signature carries")
    else:
        failures += 1
        print("    MISSED: the SSH reader got %r" % ssh_signer(armoured))

    if failures:
        print("  %d self-test(s) failed: this guard does not catch what it claims to"
              % failures)
        return 1
    print("  every fault this guard exists for is caught by it")
    return 0


def main(argv):
    if "--self-test" in argv:
        return self_test()
    if "--range" in argv:
        spec = argv[argv.index("--range") + 1]
        commits = git(["rev-list", "--reverse", spec], ROOT, check=True).split()
        look_ahead = False
    elif "--pushed" in argv:
        at = argv.index("--pushed")
        commits = pushed(ROOT, argv[at + 1], argv[at + 2])
        look_ahead = False
    else:
        commits = unpushed(ROOT)
        look_ahead = True

    failures, notes = audit(ROOT, commits, look_ahead=look_ahead)
    if "--github" in argv:
        for sha, verified, reason in github_verdicts(commits):
            if verified:
                continue
            subject = git(["log", "-1", "--format=%h %s", sha], ROOT).strip()[:70]
            target = failures if SIGNATURES_REQUIRED else notes
            target.append("  %s" % subject)
            target.append("      GitHub does not show it Verified (%s)" % reason)
    if notes:
        print("  signatures (reported, not yet required):")
        print("\n".join(notes))
    if failures:
        print("\n".join(failures))
        print()
        print("  Every commit here is tilas01's and names no assistant: docs/CONTRIBUTING.md,")
        print("  under Commits. A commit that has not been pushed is repaired, once the")
        print("  identity above is right, with `git commit --amend --no-edit --reset-author`.")
        return 1
    where = "to push" if look_ahead else "in the range"
    print("  %d commit(s) %s, every one of them tilas01's in both fields%s"
          % (len(commits), where, ", and the next one will be too" if look_ahead else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
