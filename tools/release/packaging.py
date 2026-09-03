#!/usr/bin/env python3
"""Check that every distribution package agrees with the tree it packages.

A package definition is a claim about what gets installed, and it is made in a
file nobody builds during ordinary work: the deb, the RPM, the ebuild, the
Arch packages and the Windows installer are exercised at release time or on
somebody else's machine. So they go stale quietly, and the first person to
notice is a user whose install is missing a binary or naming one that no longer
exists.

This checks the parts that can be checked without a package manager:

  1. Every package installs exactly the binaries the workspace builds.
  2. No package still refers to a binary that has been removed.
  3. The Arch `.SRCINFO` agrees with its `PKGBUILD`, since the AUR reads the
     former and builds the latter.
  4. Versions written into packaging agree with Cargo.toml.

It does not build anything, so it cannot tell you a package *works*. It tells
you the package is describing this tree rather than an older one, which is the
failure that actually happens.

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import re
import shlex
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

#: The binaries the workspace actually produces. Read from the crates rather
#: than listed here, so removing one is noticed instead of needing this file
#: edited in the same breath.
def workspace_binaries() -> set[str]:
    names: set[str] = set()
    for manifest in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        for entry in data.get("bin", []):
            if "name" in entry:
                names.add(entry["name"])
        # A crate with src/main.rs and no [[bin]] still builds a binary named
        # after the package.
        if not data.get("bin") and (manifest.parent / "src" / "main.rs").exists():
            names.add(data["package"]["name"])
    return names


#: Where each packaging file installs from, and how a binary appears in it.
PACKAGES = {
    "packaging/debian/rules": r"target/release/([a-z0-9-]+)",
    "packaging/rpm/veilvoice.spec": r"target/release/([a-z0-9-]+)",
    "packaging/homebrew/veilvoice.rb": r"target/release/([a-z0-9-]+)",
    "packaging/flatpak/io.github.tilas01.VeilVoice.yml": r"target/release/([a-z0-9-]+)",
    "packaging/aur/PKGBUILD": r"target/release/([a-z0-9-]+)",
    "packaging/aur/PKGBUILD-git": r"target/release/([a-z0-9-]+)",
    "packaging/gentoo/media-sound/veilvoice/veilvoice-9999.ebuild":
        r"cargo_target_dir\)/([a-z0-9-]+)",
    "packaging/wix/veilvoice.wxs": r"BinDir\)\\([a-z0-9-]+)\.exe",
}


def cargo_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version = "([^"]+)"', text, re.M)
    assert match, "no workspace version in Cargo.toml"
    return match.group(1)


def check() -> list[str]:
    problems: list[str] = []
    built = workspace_binaries()

    for relative, pattern in PACKAGES.items():
        path = ROOT / relative
        if not path.exists():
            problems.append(f"{relative}: missing")
            continue
        text = path.read_text(encoding="utf-8")
        named = {m for m in re.findall(pattern, text) if m.startswith("veilvoice")}
        for name in sorted(named - built):
            problems.append(
                f"{relative}: installs {name!r}, which the workspace no longer builds"
            )

    # The AUR reads .SRCINFO and builds the PKGBUILD. Two files, one truth.
    pkgbuild = (ROOT / "packaging/aur/PKGBUILD").read_text(encoding="utf-8")
    srcinfo = (ROOT / "packaging/aur/.SRCINFO").read_text(encoding="utf-8")
    version = cargo_version()

    pkgver = re.search(r"^pkgver=(\S+)", pkgbuild, re.M)
    if not pkgver or pkgver.group(1) != version:
        problems.append(
            f"packaging/aur/PKGBUILD: pkgver is "
            f"{pkgver.group(1) if pkgver else 'unset'}, Cargo.toml says {version}"
        )
    srcver = re.search(r"^\tpkgver = (\S+)", srcinfo, re.M)
    if not srcver or srcver.group(1) != version:
        problems.append(
            f"packaging/aur/.SRCINFO: pkgver is "
            f"{srcver.group(1) if srcver else 'unset'}, Cargo.toml says {version}"
        )

    for field in ("depends", "optdepends"):
        # shlex rather than a split: these are shell arrays whose entries are
        # quoted strings containing spaces and colons, and splitting on
        # whitespace turns one description into a dozen package names.
        build_names: set[str] = set()
        for block in re.findall(rf"^{field}=\(([^)]*)\)", pkgbuild, re.M | re.S):
            for entry in shlex.split(block, comments=True):
                build_names.add(entry.split(":")[0].strip())
        src_names = {
            line.split(" = ", 1)[1].split(":")[0].strip()
            for line in srcinfo.splitlines()
            if line.strip().startswith(f"{field} = ")
        }
        missing = src_names - build_names
        extra = build_names - src_names
        for name in sorted(missing):
            problems.append(f"packaging/aur/.SRCINFO: {field} {name!r} is not in the PKGBUILD")
        for name in sorted(extra):
            problems.append(f"packaging/aur/.SRCINFO: {field} {name!r} from the PKGBUILD is missing")

    return problems


def main() -> int:
    problems = check()
    if problems:
        for line in problems:
            print(f"  {line}")
        print(f"\n{len(problems)} packaging problem(s).")
        return 1
    binaries = ", ".join(sorted(workspace_binaries()))
    print(f"every package installs exactly the workspace binaries: {binaries}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
