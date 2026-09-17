#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
No crate reaching `pgp` may gain an RSA private-key code path.

# What this is holding up

`rsa` is in the dependency graph through `pgp`, and it carries
RUSTSEC-2023-0071: the Marvin attack, key recovery through a timing side
channel, with no fixed version available. `.cargo/audit.toml` accepts that
advisory on one specific ground, that the advisory is about RSA *private key*
operations and VeilVoice only ever verifies a signature against a public key
compiled into it. There is no secret for a timing oracle to leak.

That is an argument about how the crate is used rather than about the crate, so
it is enforced here rather than believed. If a secret-key or decryption path
ever appears, this fails and the acceptance has to be re-argued.

# Why this is not a `grep`

It was one, and the grep counted the word `decrypt` wherever it appeared,
including inside a string. The sentence the verifier prints about the signing
key it imported is:

    It is a public key: it lets you check signatures and can sign
    nothing and decrypt nothing. It carries no e-mail address.

which is a promise that no private-key operation happens, and it failed the
check that no private-key operation happens. The guard had been green for as
long as that sentence lived in a crate that did not reach `pgp`; consolidating
twenty-seven crates into thirteen moved it into one that does.

Weakening the pattern to get past it would have been the wrong repair, because
the pattern is deliberately broad. So the source is stripped of comments and
string literals first, and what is left is the code. A guard that reads prose
is not reading the program.

Pure standard library.
"""

from __future__ import annotations

import glob
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# Names that would mean a private key is being handled or a decryption is being
# performed. Broad on purpose: this is the check that lets an accepted advisory
# stay accepted, so it should fire on anything near the line rather than only on
# the exact call somebody thought of when writing it.
PATTERN = re.compile(
    r"\b(SignedSecretKey|SecretKeyParams|decrypt\w*|sign_binary_data|sign_text_data)\b"
)


def strip_rust(text):
    """Blank out comments, string literals and char literals, keeping offsets.

    Every removed byte becomes a space rather than disappearing, so a line and
    column reported against the result still points at the real source.
    """
    out = list(text)
    i, n = 0, len(text)

    def blank(start, stop):
        for k in range(start, min(stop, n)):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        two = text[i:i + 2]
        if two == "//":
            stop = text.find("\n", i)
            stop = n if stop == -1 else stop
            blank(i, stop)
            i = stop
        elif two == "/*":
            # Rust block comments nest.
            depth, j = 1, i + 2
            while j < n and depth:
                if text[j:j + 2] == "/*":
                    depth += 1
                    j += 2
                elif text[j:j + 2] == "*/":
                    depth -= 1
                    j += 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif text[i] == "r" and i + 1 < n and text[i + 1] in '#"':
            # Raw string: r"...", r#"..."#, r##"..."## and so on.
            j = i + 1
            hashes = 0
            while j < n and text[j] == "#":
                hashes += 1
                j += 1
            if j < n and text[j] == '"':
                close = '"' + "#" * hashes
                stop = text.find(close, j + 1)
                stop = n if stop == -1 else stop + len(close)
                blank(i, stop)
                i = stop
            else:
                i += 1
        elif text[i] == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            blank(i, j)
            i = j
        elif text[i] == "'":
            # A char literal, or a lifetime. A lifetime has no closing quote,
            # so only blank when one turns up within a few bytes.
            j = i + 1
            if j < n and text[j] == "\\":
                j += 2
            elif j < n:
                j += 1
            if j < n and text[j] == "'":
                blank(i, j + 1)
                i = j + 1
            else:
                i += 1
        else:
            i += 1
    return "".join(out)


def crates_reaching_pgp():
    """Every crate whose manifest names `pgp`, found rather than listed."""
    found = []
    for manifest in sorted(glob.glob(os.path.join(ROOT, "crates", "*", "Cargo.toml"))):
        with open(manifest, encoding="utf-8") as handle:
            if any(line.startswith("pgp = ") for line in handle):
                found.append(os.path.dirname(manifest))
    return found


def main():
    crates = crates_reaching_pgp()
    if not crates:
        print("  no crate depends on pgp any more, so this is checking nothing")
        return 1

    names = [os.path.basename(c) for c in crates]
    print("  crates reaching pgp: %s" % ", ".join(names))

    offenders = []
    scanned = 0
    for crate in crates:
        for path, _, files in os.walk(os.path.join(crate, "src")):
            for name in sorted(files):
                if not name.endswith(".rs"):
                    continue
                full = os.path.join(path, name)
                with open(full, encoding="utf-8") as handle:
                    text = handle.read()
                scanned += 1
                code = strip_rust(text)
                for found in PATTERN.finditer(code):
                    line = code.count("\n", 0, found.start()) + 1
                    offenders.append(
                        (os.path.relpath(full, ROOT), line, found.group(0))
                    )

    if offenders:
        print("  a private-key or decryption path appears in code:")
        for where, line, what in offenders:
            print("    %s:%d  %s" % (where, line, what))
        print()
        print("  RUSTSEC-2023-0071 is accepted in .cargo/audit.toml only on the")
        print("  ground that no such path exists. Either remove this, or re-argue")
        print("  the acceptance on the facts as they now are.")
        return 1

    print("  %d source file(s), no private-key or decryption path in any of them"
          % scanned)
    return 0


if __name__ == "__main__":
    sys.exit(main())
