#!/usr/bin/env python3
"""Prove the app-manifest tooling still works: round trip, and catch a tamper.

The self-signing scripts need OpenSSL and a private key, so they cannot run in
CI unattended. The manifest generator can, and it is the part a mistake would
most quietly break -- a manifest that no longer matches the binaries it
describes verifies nothing. This exercises it end to end against a temporary
directory, so `tools/verify.py` fails if the generator or its checker drifts.

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GEN = ROOT / "tools" / "sign" / "manifest.py"


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(GEN), *args],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        (d / "veilvoice").write_bytes(b"pretend binary one")
        (d / "veilvoice-gui").write_bytes(b"pretend binary two, longer")

        if run(str(d)).returncode != 0:
            print("  the manifest generator failed on a clean directory")
            return 1
        if run(str(d), "--check").returncode != 0:
            print("  a freshly written manifest did not verify against its own files")
            return 1

        # Tamper with a binary; the check must now fail.
        (d / "veilvoice").write_bytes(b"tampered")
        if run(str(d), "--check").returncode == 0:
            print("  a tampered binary passed the manifest check -- the check is broken")
            return 1

    print("app-manifest tooling round-trips and catches a tampered binary")
    return 0


if __name__ == "__main__":
    sys.exit(main())
