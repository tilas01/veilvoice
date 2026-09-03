#!/usr/bin/env python3
"""Generate APPMANIFEST.json: a signed-list VeilVoice can carry about itself.

    python3 tools/sign/manifest.py DIR            # write DIR/APPMANIFEST.json
    python3 tools/sign/manifest.py DIR --check    # verify an existing one

# What this is, beside the OpenPGP one

Every release already publishes `SHA256SUMS`, signed with the project's OpenPGP
key, and that remains the real check. This is a second, self-contained record --
a small JSON manifest naming each binary, its size and its SHA-256, plus the
project and version -- meant to be signed with the project's **self-signed code
certificate** (see `tools/sign/selfsign.sh`) rather than the OpenPGP key.

The point of a self-signed certificate is narrow and honest: it is not a
certificate authority vouching for anyone, and Windows SmartScreen will not
trust it on sight. What it is good for is a user or an organisation who chooses
to import it once, after checking its fingerprint, so that this publisher
becomes *known* on their machines. From then on files carrying this manifest can
be checked against a certificate they decided to trust, which is the same shape
as the OpenPGP flow: compare a fingerprint by hand once, then let the tools do
the rest.

The manifest is detached from the binaries -- it describes them, it is not
embedded in them -- so signing it changes nothing about the binaries and does
not touch reproducibility. That is the same reason the OpenPGP signature is over
`SHA256SUMS` and never over a binary in place.

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = "APPMANIFEST.json"

#: The files a manifest describes: the shipped programs, by the names they have
#: on each platform. A file that is not there is simply not listed, so one
#: manifest generator serves every platform's archive.
BINARIES = ["veilvoice", "veilvoice-gui", "veilvoice.exe", "veilvoice-gui.exe"]


def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version = "([^"]+)"', text, re.M)
    if not match:
        raise SystemExit("no workspace version in Cargo.toml")
    return match.group(1)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def build(directory: Path) -> dict:
    """The manifest for the binaries found in `directory`.

    Deterministic: the file list is sorted, and the JSON is written with sorted
    keys and a fixed separator, so the same inputs produce byte-identical output
    on any machine. A manifest that varied run to run could not be signed once
    and checked later.
    """
    files = []
    for name in sorted(set(BINARIES)):
        path = directory / name
        if not path.is_file():
            continue
        files.append(
            {
                "name": name,
                "size": path.stat().st_size,
                "sha256": sha256(path),
            }
        )
    if not files:
        raise SystemExit(
            f"no VeilVoice binaries found in {directory}. "
            f"Point this at a built or unpacked release."
        )
    return {
        "manifest_version": 1,
        "project": "VeilVoice",
        "version": workspace_version(),
        "files": files,
    }


def render(manifest: dict) -> str:
    # Sorted keys and a trailing newline, so the file is stable and diff-clean.
    return json.dumps(manifest, indent=2, sort_keys=True) + "\n"


def write(directory: Path) -> int:
    manifest = build(directory)
    (directory / MANIFEST).write_text(render(manifest), encoding="utf-8")
    print(f"wrote {directory / MANIFEST} ({len(manifest['files'])} file(s))")
    return 0


def check(directory: Path) -> int:
    path = directory / MANIFEST
    if not path.exists():
        print(f"no {MANIFEST} in {directory}")
        return 1
    stored = json.loads(path.read_text(encoding="utf-8"))
    problems: list[str] = []
    for entry in stored.get("files", []):
        binary = directory / entry["name"]
        if not binary.is_file():
            problems.append(f"{entry['name']}: listed but not present")
            continue
        got = sha256(binary)
        if got != entry["sha256"]:
            problems.append(
                f"{entry['name']}: manifest {entry['sha256'][:16]}…, file {got[:16]}…"
            )
        if binary.stat().st_size != entry["size"]:
            problems.append(f"{entry['name']}: size differs from the manifest")
    if stored.get("version") != workspace_version():
        problems.append(
            f"manifest version {stored.get('version')}, workspace {workspace_version()}"
        )
    if problems:
        for line in problems:
            print(f"  {line}")
        print(f"\n{len(problems)} manifest problem(s).")
        return 1
    print(f"{MANIFEST} matches the {len(stored['files'])} binary(ies) beside it")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="where the binaries are")
    parser.add_argument("--check", action="store_true", help="verify, do not write")
    args = parser.parse_args()
    directory = args.directory
    if not directory.is_dir():
        raise SystemExit(f"not a directory: {directory}")
    return check(directory) if args.check else write(directory)


if __name__ == "__main__":
    sys.exit(main())
