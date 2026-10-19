#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Set this clone up to commit as tilas01 and sign with his key, from the environment.

    python tools/repo/signing.py              # import the key, configure this clone
    python tools/repo/signing.py --self-test  # prove it, with a throwaway key

# Why this exists

A cloud container comes with a signer of its own, set in its global git config,
and a key registered to a different GitHub account. A commit it signs in
tilas01's name reads Unverified on GitHub, which is how twenty-five of his
commits came to (F-235). The fix is a key that is his: a GPG key made for this
purpose, registered to his GitHub account, and held base64-encoded in the
project's `VEILVOICE_SIGNING_KEY` environment variable, which every session in
the project starts with. `docs/CONTRIBUTING.md` says how it was made.

# What it sets, and where

Repository-local config, which beats the container's global config:
`user.name`, `user.email`, `gpg.format openpgp`, `user.signingkey` with the
key's fingerprint and `commit.gpgsign true`. Nothing global is touched, so a
second clone on the same machine is not changed behind anybody's back.

The fingerprint it reports, the one to add to `SIGNING_KEYS`, is the key a
signature will name: the signing subkey when the key has one, the primary key
otherwise.

When setup cannot finish (no variable, not base64, a key gpg cannot read or
import, a config write that fails) it sets `commit.gpgsign false` for this
clone and says why. Leaving it alone would leave the container's own signing
on, and that key reads Unverified in tilas01's name (F-235).

Four `GIT_AUTHOR_*` and `GIT_COMMITTER_*` variables beat every config file, and
some containers set them. A program cannot change the shell that ran it, so
when they are set to anything else this prints the line to export and fails,
rather than reporting success over an identity it could not fix.

# What it refuses

A key whose fingerprint is not in `SIGNING_KEYS` in `tools/audit/authorship.py`
is imported and configured, and reported: the guard would refuse every commit
it signed, so saying so here is the useful moment.

Pure standard library, and GnuPG.
"""

from __future__ import annotations

import base64
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "tools", "audit"))

import authorship  # noqa: E402

VARIABLE = "VEILVOICE_SIGNING_KEY"
IDENTITY = {"GIT_AUTHOR_NAME": authorship.NAME, "GIT_AUTHOR_EMAIL": authorship.EMAIL,
            "GIT_COMMITTER_NAME": authorship.NAME, "GIT_COMMITTER_EMAIL": authorship.EMAIL}


def run(args, cwd=ROOT, stdin=None, env=None):
    done = subprocess.run(args, cwd=cwd, input=stdin, env=env,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    return done.returncode, done.stdout.decode("utf-8", "replace"), \
        done.stderr.decode("utf-8", "replace")


class Failure(Exception):
    """Setup could not finish; the message says why."""


def fingerprints(armoured, env=None):
    """(primary, signer) fingerprints in an armoured secret key, without importing it.

    The signer is the key a signature will name: a subkey with the signing
    capability when there is one, since gpg signs with the newest of those, and
    the primary key otherwise. GitHub and the guard both go by the signer.
    """
    code, out, err = run(["gpg", "--batch", "--with-colons", "--import-options",
                          "show-only", "--import"], stdin=armoured, env=env)
    primary, signer, kind = None, None, None
    for line in out.splitlines():
        fields = line.split(":")
        if fields[0] in ("sec", "pub", "ssb", "sub"):
            kind = (fields[0], fields[11] if len(fields) > 11 else "")
        elif fields[0] == "fpr" and kind:
            if kind[0] in ("sec", "pub") and primary is None:
                primary = fields[9]
                if "s" in kind[1].lower():
                    signer = signer or fields[9]
            elif "s" in kind[1].lower():
                signer = fields[9]
            kind = None
    if primary is None:
        raise Failure("%s does not hold a key gpg can read: %s" % (VARIABLE, err.strip()))
    if signer is None:
        raise Failure("the key in %s cannot sign: neither it nor a subkey has the "
                      "signing capability" % VARIABLE)
    return primary, signer


def set_config(cwd, key, value, env=None):
    code, _, err = run(["git", "config", "--local", key, value], cwd, env=env)
    if code != 0:
        raise Failure("git config %s failed: %s" % (key, err.strip()))


def configure(cwd, encoded, env=None):
    """Import the key and set this clone up. Returns (signer, problems)."""
    if not encoded:
        raise Failure("%s is not set in this session. It is added in the project's\n"
                      "  environment settings, and a session started after that has it."
                      % VARIABLE)
    try:
        armoured = base64.b64decode("".join(encoded.split()), validate=True)
    except ValueError:
        raise Failure("%s is not base64. It is the output of\n"
                      "  gpg --armor --export-secret-keys KEY | base64 | tr -d '\\n'"
                      % VARIABLE)
    primary, fpr = fingerprints(armoured, env)
    code, _, err = run(["gpg", "--batch", "--import"], cwd, armoured, env)
    if code != 0:
        raise Failure("gpg could not import the key: %s" % err.strip())
    for key, value in (("user.name", authorship.NAME), ("user.email", authorship.EMAIL),
                       ("gpg.format", "openpgp"), ("user.signingkey", primary),
                       ("commit.gpgsign", "true")):
        set_config(cwd, key, value, env)

    problems = []
    if not authorship.accepted("openpgp:" + fpr, authorship.SIGNING_KEYS):
        problems.append(
            "the key %s is not in SIGNING_KEYS in tools/audit/authorship.py, so the\n"
            "  guard will refuse every commit it signs. Add it there, with where it lives."
            % fpr)
    wrong = {k: v for k, v in (env or os.environ).items() if k in IDENTITY and v != IDENTITY[k]}
    if wrong:
        problems.append(
            "this shell sets %s, which beat every config file. Run:\n"
            "  export %s" % (", ".join(sorted(wrong)),
                             " ".join("%s=%s" % kv for kv in IDENTITY.items())))
    return fpr, problems


def self_test():
    """A throwaway key, a throwaway repository, and a commit it must sign."""
    if shutil.which("gpg") is None:
        print("  skipped: gpg is not installed here, so there is nothing to sign with")
        return 0
    # Short on purpose: gpg-agent's socket lives in this directory, and a
    # socket path has a length limit well under what a scratch path can reach.
    home = tempfile.mkdtemp(prefix="vvg")
    where = tempfile.mkdtemp(prefix="vvr")
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("GIT_AUTHOR_", "GIT_COMMITTER_", "GIT_CONFIG"))}
    env.update({"GNUPGHOME": home, "GIT_CONFIG_GLOBAL": os.devnull,
                "GIT_CONFIG_SYSTEM": os.devnull})
    env.update(IDENTITY)
    failures = 0
    try:
        os.chmod(home, 0o700)
        code, _, err = run(["gpg", "--batch", "--passphrase", "", "--quick-gen-key",
                            "tilas01 (test) <%s>" % authorship.EMAIL, "ed25519", "sign",
                            "1d"], where, env=env)
        if code != 0:
            print("  could not make a test key: %s" % err.strip().splitlines()[-1])
            return 1
        _, secret, _ = run(["gpg", "--batch", "--armor", "--export-secret-keys",
                            authorship.EMAIL], where, env=env)
        encoded = base64.b64encode(secret.encode()).decode()
        # A second keyring, so the import is what puts the key there.
        run(["gpgconf", "--kill", "gpg-agent"], where, env=env)
        shutil.rmtree(home)
        os.mkdir(home, 0o700)

        run(["git", "init", "-q", "-b", "main", where], env=env)
        fpr, problems = configure(where, encoded, env)
        if any("not in SIGNING_KEYS" in p for p in problems):
            print("    caught: a key the guard does not accept")
        else:
            failures += 1
            print("    MISSED: a throwaway key should not be in SIGNING_KEYS")

        with open(os.path.join(where, "f"), "w") as handle:
            handle.write("x\n")
        run(["git", "add", "f"], where, env=env)
        code, _, err = run(["git", "commit", "-q", "-m", "Signed"], where, env=env)
        _, raw, _ = run(["git", "cat-file", "commit", "HEAD"], where, env=env)
        signer = authorship.signature_of(raw) if code == 0 else None
        if signer == "openpgp:" + fpr:
            print("    signed: a commit made after setup carries the imported key")
        else:
            failures += 1
            print("    MISSED: the commit was signed by %r, not %s (%s)"
                  % (signer, fpr, err.strip()[-200:]))

        wrong = dict(env, GIT_AUTHOR_NAME="someone")
        _, problems = configure(where, encoded, wrong)
        if any("export" in p for p in problems):
            print("    caught: identity variables that beat the config it wrote")
        else:
            failures += 1
            print("    MISSED: a preset identity variable should be reported")

        # A key whose primary only certifies and whose subkey signs: the
        # signature names the subkey, so that is what setup must report.
        code, _, err = run(["gpg", "--batch", "--passphrase", "", "--quick-gen-key",
                            "tilas01 (subkey test) <%s>" % authorship.EMAIL, "ed25519",
                            "cert", "1d"], where, env=env)
        _, listing, _ = run(["gpg", "--batch", "--with-colons", "--list-keys",
                             "subkey test"], where, env=env)
        primary = [l.split(":")[9] for l in listing.splitlines() if l.startswith("fpr:")][0]
        run(["gpg", "--batch", "--passphrase", "", "--quick-add-key", primary, "ed25519",
             "sign", "1d"], where, env=env)
        _, secret, _ = run(["gpg", "--batch", "--armor", "--export-secret-keys",
                            primary], where, env=env)
        run(["gpgconf", "--kill", "gpg-agent"], where, env=env)
        shutil.rmtree(home)
        os.mkdir(home, 0o700)
        second = os.path.join(where, "second")
        run(["git", "init", "-q", "-b", "main", second], env=env)
        fpr, _ = configure(second, base64.b64encode(secret.encode()).decode(), env)
        with open(os.path.join(second, "f"), "w") as handle:
            handle.write("x\n")
        run(["git", "add", "f"], second, env=env)
        code, _, err = run(["git", "commit", "-q", "-m", "Signed"], second, env=env)
        _, raw, _ = run(["git", "cat-file", "commit", "HEAD"], second, env=env)
        signer = authorship.signature_of(raw) if code == 0 else None
        if fpr != primary and signer == "openpgp:" + fpr:
            print("    signed: a signing subkey is the key reported, and the one that signs")
        else:
            failures += 1
            print("    MISSED: reported %s, primary %s, signed by %r (%s)"
                  % (fpr, primary, signer, err.strip()[-200:]))

        # Setup that cannot finish switches signing off, so the machine's own
        # key does not sign in tilas01's name.
        for name, encoded in (("no key at all", ""), ("a key that is not base64", "!!")):
            third = tempfile.mkdtemp(prefix="vvs", dir=where)
            run(["git", "init", "-q", "-b", "main", third], env=env)
            run(["git", "config", "commit.gpgsign", "true"], third, env=env)
            code = setup(third, encoded, env)
            _, value, _ = run(["git", "config", "--local", "commit.gpgsign"], third, env=env)
            if code == 1 and value.strip() == "false":
                print("    off: %s leaves the clone unsigned, not signed by another key" % name)
            else:
                failures += 1
                print("    MISSED: %s left commit.gpgsign %r" % (name, value.strip()))
    finally:
        run(["gpgconf", "--kill", "gpg-agent"], where, env=env)
        shutil.rmtree(home, ignore_errors=True)
        shutil.rmtree(where, ignore_errors=True)

    if failures:
        print("  %d self-test(s) failed" % failures)
        return 1
    print("  a key from the environment signs the next commit, and a wrong one is named")
    return 0


def setup(cwd, encoded, env=None):
    """Configure, or switch signing off for this clone. Returns an exit code.

    Anything short of a working key leaves the clone committing unsigned
    rather than with whatever the machine's own config signs with: in a cloud
    container that is a key registered to another account, which reads
    Unverified in tilas01's name, worse than no badge at all.
    """
    try:
        fpr, problems = configure(cwd, encoded, env)
    except Failure as failure:
        code, _, err = run(["git", "config", "--local", "commit.gpgsign", "false"],
                           cwd, env=env)
        print("  " + str(failure))
        print("  signing is off for this clone until this succeeds" if code == 0 else
              "  and signing could not be switched off either: %s" % err.strip())
        return 1
    print("  this clone commits as %s <%s> and signs with %s"
          % (authorship.NAME, authorship.EMAIL, fpr))
    for problem in problems:
        print("  " + problem)
    return 1 if problems else 0


def main():
    if "--self-test" in sys.argv:
        return self_test()
    return setup(ROOT, os.environ.get(VARIABLE, ""))


if __name__ == "__main__":
    sys.exit(main())
