#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""The workspace version, and every other place that repeats it.

Why this exists
---------------

`Cargo.toml` decides what version VeilVoice is. Eleven other files say it
again: the README tells a reader which archive to download, the WiX recipe
names the folder it builds an installer from, the Homebrew formula names a
tag, the Flatpak manifest names the same tag, the RPM spec carries a default,
and so on. None of them is derived from the first one.

That is this project's oldest defect class, written up several times in
`docs/AUDIT.md` under different names: N hand-kept copies of one fact, checked
only against each other by whoever remembers. Its cost here is specific and
public. The README's install block is the first thing anybody runs, and a
release that forgets to bump it hands every new reader a command that
downloads the *previous* version and then verifies it successfully, which is
the worst possible failure: it looks like it worked.

So the version has one source and this reads it. `--check` fails when any copy
disagrees, and runs in `tools/verify.py` alongside every other generator.
`--set` moves them all at once.

    tools/release/version.py --check
    tools/release/version.py --set 0.1.17

What is deliberately not rewritten
----------------------------------

Three files keep a history: `packaging/debian/changelog`, the `%changelog` in
the RPM spec, and the `<releases>` list in the AppStream metainfo. Those name
every past version on purpose and rewriting them would be vandalism. For those
the rule is different and weaker: the *newest* entry must name the current
version. `--set` adds a new entry rather than editing the old one.

`CHANGELOG.md` is not touched at all. It is prose about what changed, and no
program should be writing that.
"""

from __future__ import annotations

import argparse
import datetime
import pathlib
import re
import sys


def repo_root() -> pathlib.Path:
    return pathlib.Path(__file__).resolve().parents[2]


def workspace_version(root: pathlib.Path) -> str:
    """The one source of truth: `[workspace.package] version` in Cargo.toml."""
    text = (root / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"$', text)
    if not match:
        raise SystemExit("Cargo.toml has no workspace version to read")
    return match.group(1)


# Each entry is a file and a pattern with exactly one group: the version, in
# whichever spelling that file uses. Every match in the file must agree with
# the workspace, and `--set` rewrites the group.
#
# The patterns are deliberately narrow. A blanket search for the version
# string would also hit the changelog entries and the historical release
# lists, which must not move.
PLACES: list[tuple[str, str, str]] = [
    ("README.md", r"^V=v([0-9]+\.[0-9]+\.[0-9]+)$", "the Linux, macOS and BSD install blocks"),
    ("README.md", r'^\$V = "v([0-9]+\.[0-9]+\.[0-9]+)"$', "the Windows install block"),
    ("README.md", r"^sh reproduce-veilvoice\.sh v([0-9]+\.[0-9]+\.[0-9]+)$", "the reproducible-build example"),
    ("README.md", r"^\*\*v([0-9]+\.[0-9]+\.[0-9]+): early but real\.\*\*", "the status section"),
    ("docs/USER_GUIDE.md", r"^sh reproduce-veilvoice\.sh v([0-9]+\.[0-9]+\.[0-9]+)$", "the reproducible-build example"),
    # This one had gone three releases stale saying "v0.1.14 is released",
    # which is what a roadmap is read for.
    ("ROADMAP.md", r"\*\*v([0-9]+\.[0-9]+\.[0-9]+) is released\*\*", "the where-we-are-now line"),
    ("docs/PACKAGING.md", r"-d Version=([0-9]+\.[0-9]+\.[0-9]+) ", "the WiX example"),
    ("docs/PACKAGING.md", r"BinDir=dist/veilvoice-v([0-9]+\.[0-9]+\.[0-9]+)-windows", "the WiX example"),
    ("docs/PACKAGING.md", r"-o dist/VeilVoice-([0-9]+\.[0-9]+\.[0-9]+)-x64\.msi", "the WiX example"),
    ("docs/PACKAGING.md", r'"vv_version ([0-9]+\.[0-9]+\.[0-9]+)"', "the rpmbuild example"),
    ("packaging/wix/veilvoice.wxs", r"-d Version=([0-9]+\.[0-9]+\.[0-9]+) ", "the build command in the header"),
    ("packaging/wix/veilvoice.wxs", r"BinDir=dist\\veilvoice-v([0-9]+\.[0-9]+\.[0-9]+)-windows", "the build command in the header"),
    ("packaging/wix/veilvoice.wxs", r"-o dist/VeilVoice-([0-9]+\.[0-9]+\.[0-9]+)-x64\.msi", "the build command in the header"),
    ("packaging/homebrew/veilvoice.rb", r"refs/tags/v([0-9]+\.[0-9]+\.[0-9]+)\.tar\.gz", "the source tarball"),
    ("packaging/flatpak/io.github.tilas01.VeilVoice.yml", r"^        tag: v([0-9]+\.[0-9]+\.[0-9]+)$", "the git tag built from"),
    ("packaging/rpm/veilvoice.spec", r'"vv_version ([0-9]+\.[0-9]+\.[0-9]+)"', "the rpmbuild example in the header"),
    ("packaging/rpm/veilvoice.spec", r"%\{!\?vv_version:([0-9]+\.[0-9]+\.[0-9]+)\}", "the default when none is passed"),
    ("packaging/aur/PKGBUILD", r"^pkgver=([0-9]+\.[0-9]+\.[0-9]+)$", "the Arch package version"),
    ("packaging/aur/.SRCINFO", r"^\tpkgver = ([0-9]+\.[0-9]+\.[0-9]+)$", "the Arch .SRCINFO version"),
    # **F-144.** The worked example a nervous user copies to verify their
    # download. It said v0.1.9 while the workspace said 0.1.17 -- eight
    # releases, naming a real old tarball rather than an obvious placeholder,
    # so following the instructions verbatim downloaded and verified the wrong
    # release perfectly. The surrounding text does say "replace this with the
    # release you want", which is exactly the sentence people skip.
    ("docs/INSTALL.md", r"^V=v([0-9]+\.[0-9]+\.[0-9]+)$", "the by-hand verification example"),
    ("docs/INSTALL.md", r"veilvoice-v([0-9]+\.[0-9]+\.[0-9]+)-windows-x86_64\.zip", "the PowerShell hash example"),
    ("docs/INSTALL.md", r"veilvoice-v([0-9]+\.[0-9]+\.[0-9]+)-linux-x86_64\.tar\.gz", "the verifier examples"),
    ("docs/INSTALL.md", r"`--version v([0-9]+\.[0-9]+\.[0-9]+)`", "the install-script option table"),
    ("docs/INSTALL.md", r"`-Version v([0-9]+\.[0-9]+\.[0-9]+)`", "the PowerShell option table"),
]

# The three files that keep a history. Only the newest entry is checked, and
# `--set` prepends rather than edits.
HISTORIES: list[tuple[str, str, str]] = [
    ("packaging/debian/changelog", r"^veilvoice \(([0-9]+\.[0-9]+\.[0-9]+)-1\)", "the newest changelog entry"),
    ("packaging/rpm/veilvoice.spec", r"^\* .* - ([0-9]+\.[0-9]+\.[0-9]+)-1$", "the newest %changelog entry"),
    ("packaging/flatpak/io.github.tilas01.VeilVoice.metainfo.xml", r'<release version="([0-9]+\.[0-9]+\.[0-9]+)"', "the newest release entry"),
]


def disagreements(root: pathlib.Path, version: str) -> list[str]:
    """Every place whose version is not the workspace version."""
    problems: list[str] = []

    for name, pattern, what in PLACES:
        path = root / name
        text = path.read_text(encoding="utf-8")
        found = re.findall(pattern, text, flags=re.MULTILINE)
        if not found:
            # A pattern that stops matching is a defect in this file, not a
            # pass. Silently checking nothing is how a checker becomes
            # decorative.
            problems.append(f"{name}: nothing matched for {what} -- the file changed shape")
            continue
        for seen in found:
            if seen != version:
                problems.append(f"{name}: {what} says {seen}, the workspace says {version}")

    for name, pattern, what in HISTORIES:
        path = root / name
        text = path.read_text(encoding="utf-8")
        found = re.findall(pattern, text, flags=re.MULTILINE)
        if not found:
            problems.append(f"{name}: nothing matched for {what} -- the file changed shape")
        elif found[0] != version:
            problems.append(f"{name}: {what} says {found[0]}, the workspace says {version}")

    return problems


def rewrite(root: pathlib.Path, old: str, new: str) -> list[str]:
    """Move every plain copy of the version. Returns what was touched."""
    touched: list[str] = []
    for name, pattern, _what in PLACES:
        path = root / name
        text = path.read_text(encoding="utf-8")

        def swap(match: re.Match[str]) -> str:
            whole = match.group(0)
            start, end = match.span(1)
            return whole[: start - match.start()] + new + whole[end - match.start() :]

        after = re.sub(pattern, swap, text, flags=re.MULTILINE)
        if after != text:
            path.write_text(after, encoding="utf-8")
            touched.append(name)
    return sorted(set(touched))


def add_history(root: pathlib.Path, version: str) -> list[str]:
    """Prepend a new entry to each file that keeps a history."""
    touched: list[str] = []
    today = datetime.date.today()

    path = root / "packaging/debian/changelog"
    text = path.read_text(encoding="utf-8")
    if not text.startswith(f"veilvoice ({version}-1)"):
        stamp = today.strftime("%a, %d %b %Y") + " 00:00:00 +0000"
        entry = (
            f"veilvoice ({version}-1) unstable; urgency=medium\n"
            "\n"
            "  * See CHANGELOG.md in the source for what changed in this release.\n"
            "\n"
            f" -- tilas01 <tilas01@users.noreply.github.com>  {stamp}\n"
            "\n"
        )
        path.write_text(entry + text, encoding="utf-8")
        touched.append("packaging/debian/changelog")

    path = root / "packaging/rpm/veilvoice.spec"
    text = path.read_text(encoding="utf-8")
    marker = "%changelog\n"
    if marker in text and f"- {version}-1\n" not in text:
        stamp = today.strftime("%a %b %d %Y")
        entry = (
            f"* {stamp} tilas01 <tilas01@users.noreply.github.com> - {version}-1\n"
            "- See CHANGELOG.md in the source for what changed in this release.\n"
            "\n"
        )
        head, rest = text.split(marker, 1)
        path.write_text(head + marker + entry + rest, encoding="utf-8")
        touched.append("packaging/rpm/veilvoice.spec")

    path = root / "packaging/flatpak/io.github.tilas01.VeilVoice.metainfo.xml"
    text = path.read_text(encoding="utf-8")
    marker = "  <releases>\n"
    if marker in text and f'<release version="{version}"' not in text:
        entry = (
            f'    <release version="{version}" date="{today.isoformat()}">\n'
            "      <description>\n"
            "        <p>See the release notes for what changed.</p>\n"
            "      </description>\n"
            "    </release>\n"
        )
        head, rest = text.split(marker, 1)
        path.write_text(head + marker + entry + rest, encoding="utf-8")
        touched.append("packaging/flatpak/io.github.tilas01.VeilVoice.metainfo.xml")

    return touched


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="report copies that disagree, and fail")
    parser.add_argument("--set", metavar="X.Y.Z", help="move the workspace and every copy to this version")
    args = parser.parse_args()

    root = repo_root()

    if args.set:
        if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", args.set):
            print(f"not a version: {args.set}", file=sys.stderr)
            return 2
        old = workspace_version(root)
        cargo = root / "Cargo.toml"
        text = cargo.read_text(encoding="utf-8")
        cargo.write_text(
            re.sub(r'(?m)^version = "[0-9]+\.[0-9]+\.[0-9]+"$', f'version = "{args.set}"', text, count=1),
            encoding="utf-8",
        )
        touched = ["Cargo.toml"] + rewrite(root, old, args.set) + add_history(root, args.set)
        print(f"  {old} -> {args.set} in {len(touched)} files")
        for name in touched:
            print(f"    {name}")
        left = disagreements(root, args.set)
        if left:
            for problem in left:
                print(f"  still wrong: {problem}", file=sys.stderr)
            return 1
        return 0

    version = workspace_version(root)
    problems = disagreements(root, version)
    if problems:
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        print(f"  {len(problems)} place(s) disagree with the workspace version", file=sys.stderr)
        return 1
    print(f"  every copy of the version says {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
