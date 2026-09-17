#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Every random number this project draws comes from a cryptographic source.

# Why this is a guard and not a habit

A de-identifier whose randomness is predictable is a de-identifier that does
not work. The seed that drives the veiling is what stands between a recording
and the voice it came from, and a nonce reused is an AEAD broken. None of that
degrades gracefully: a weak draw produces output that looks exactly like a
strong one, and nothing downstream notices.

Rust makes the wrong thing easy to reach. `rand::thread_rng`, `SmallRng` and
`StdRng` are the three most obvious names in the most obvious crate, they are
what most examples use, and none of them promises what this project needs.
`SmallRng` says in its own documentation that it is not cryptographically
secure. `seed_from_u64` takes 64 bits where 256 are wanted.

# What is allowed

  * `getrandom`, which is the operating system's CSPRNG and the source
    everything else here is built on.
  * `rand_core::OsRng`, a thin wrapper over the same call.
  * `ChaCha20Rng`, but only seeded from one of those. It is a CSPRNG, and it is
    used where a draw has to be reproducible from a seed: the veiling is
    deterministic for a given seed by design, which is what lets the tests
    assert anything about it at all.

# What this deliberately does not read

Test code. A test that wants a fixed sequence should have one, and
`from_seed([0u8; 32])` in a test is the opposite of a defect: it is what makes
the assertion possible. The walk stops at `mod tests`, `tests.rs`, `tests/`,
fuzz targets and examples.

Pure standard library.
"""

from __future__ import annotations

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
CRATES = os.path.join(ROOT, "crates")

# Names that do not promise a cryptographic draw, each with what to use instead.
FORBIDDEN = {
    "thread_rng": "getrandom::getrandom, or ChaCha20Rng seeded from it",
    "SmallRng": "ChaCha20Rng; SmallRng documents itself as not cryptographically secure",
    "StdRng": "ChaCha20Rng, which names the algorithm rather than leaving it to the crate",
    "seed_from_u64": "from_seed with 32 bytes out of getrandom; 64 bits is not a seed",
    "from_entropy": "getrandom::getrandom directly, so a failure can be reported",
}
PATTERN = re.compile(r"\b(%s)\b" % "|".join(FORBIDDEN))


def is_test(path):
    parts = path.replace(os.sep, "/")
    return ("/tests/" in parts or parts.endswith("/tests.rs")
            or parts.endswith("_fuzz.rs") or "/examples/" in parts
            or "/fuzz/" in parts or parts.endswith("build.rs"))


def production_lines(path):
    """Every line of a file that is not inside its test module."""
    out = []
    in_tests = False
    with open(path, encoding="utf-8") as handle:
        for number, line in enumerate(handle, 1):
            if re.match(r"\s*mod tests\b", line):
                in_tests = True
            if in_tests:
                continue
            out.append((number, line))
    return out


def main():
    offenders, scanned = [], 0
    for current, directories, files in os.walk(CRATES):
        directories[:] = [d for d in directories if d != "target"]
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            path = os.path.join(current, name)
            if is_test(path):
                continue
            scanned += 1
            for number, line in production_lines(path):
                stripped = line.strip()
                if stripped.startswith("//"):
                    continue
                found = PATTERN.search(line)
                if found:
                    offenders.append(
                        (os.path.relpath(path, ROOT), number, found.group(1)))

    if offenders:
        print("  these draw randomness from a source that is not cryptographic:")
        for where, number, what in offenders:
            print("    %s:%d  %s" % (where, number, what))
            print("        use %s" % FORBIDDEN[what])
        print()
        print("  A weak draw produces output that looks exactly like a strong")
        print("  one, so nothing downstream will ever notice this for you.")
        return 1

    print("  %d production source file(s), every random draw from the OS CSPRNG"
          % scanned)
    return 0


if __name__ == "__main__":
    sys.exit(main())
