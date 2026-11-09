#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build the MSIX package for the Microsoft Store.

    python tools/package/msix.py BIN_DIR OUT_DIR --version 0.1.23 \\
        --identity-name 12345tilas01.VeilVoice \\
        --publisher "CN=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX" \\
        --publisher-display-name tilas01
    python tools/package/msix.py ... --stage-only    # lay it out, do not pack
    python tools/package/msix.py --self-test

The three identity values come from Partner Center, on the app's Product
identity page, once the name is reserved. They are not secret, and the release
workflow reads them from the repository's variables MSIX_IDENTITY_NAME,
MSIX_PUBLISHER and MSIX_PUBLISHER_DISPLAY_NAME. A package whose identity does
not match the reservation is refused at upload, so without them no MSIX is
built for a release at all.

# Signing, plainly

The package this writes is unsigned, and that is what the Store wants: the
Store signs every package it distributes with Microsoft's own certificate,
which every copy of Windows trusts. That signature is free and it is the only
free one there is. Outside the Store an MSIX must be signed by a certificate
the computer already trusts, which means a paid code-signing certificate, so
the release does not publish a sideloadable MSIX; the installers for use
outside the Store are the two MSIs.

# The tools

`makeappx` and `makepri` from the Windows SDK, which every GitHub Windows
runner carries. They are Microsoft's and not open source; the package's
contents and this recipe are. There is no open-source MSIX packer that
Partner Center documents as accepted.

# In plain words

Puts the two VeilVoice programs, the licence and the icons into the package
format the Microsoft Store takes, ready to upload.
"""

import argparse
import glob
import os
import re
import shutil
import subprocess
import sys
import tempfile
import xml.dom.minidom

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
TEMPLATE = os.path.join(ROOT, "packaging", "msix", "AppxManifest.xml")
FILES = ("veilvoice.exe", "veilvoice-gui.exe", "LICENSE", "README.md")

sys.path.insert(0, HERE)
import icons  # noqa: E402

# Partner Center's own rules for these fields, so a bad value fails here
# rather than at upload.
IDENTITY_NAME = re.compile(r"^[A-Za-z0-9.-]{3,50}$")
PUBLISHER = re.compile(r"^CN=[^,]+(,.+)?$")
VERSION = re.compile(r"^\d+\.\d+\.\d+$")

# For a package built only to be installed and tested on a build machine.
TEST_IDENTITY = ("tilas01.VeilVoice.Test", "CN=VeilVoice Test", "VeilVoice test build")


def msix_name(version):
    return f"veilvoice-v{version}-windows-x86_64-store.msix"


def manifest(version, name, publisher, display):
    problems = []
    if not VERSION.match(version):
        problems.append(f"version {version!r} is not three numbers")
    if not IDENTITY_NAME.match(name):
        problems.append(f"identity name {name!r} is not 3 to 50 letters, digits, dots or hyphens")
    if not PUBLISHER.match(publisher):
        problems.append(f"publisher {publisher!r} does not start CN=")
    if not display.strip():
        problems.append("the publisher display name is empty")
    if problems:
        raise ValueError("; ".join(problems))
    with open(TEMPLATE, encoding="utf-8") as handle:
        text = handle.read()
    escape = lambda s: (s.replace("&", "&amp;").replace("<", "&lt;")
                        .replace(">", "&gt;").replace('"', "&quot;"))
    for token, value in (("@VERSION@", version), ("@IDENTITY_NAME@", name),
                         ("@PUBLISHER@", publisher),
                         ("@PUBLISHER_DISPLAY_NAME@", display)):
        text = text.replace(token, escape(value))
    left = re.findall(r"@[A-Z_]+@", text)
    if left:
        raise ValueError(f"the template has tokens nothing fills: {', '.join(left)}")
    xml.dom.minidom.parseString(text.encode("utf-8"))
    return text


def stage(bin_dir, into, text):
    for name in FILES:
        shutil.copy2(os.path.join(bin_dir, name), os.path.join(into, name))
    icons.write_set(icons.MSIX, os.path.join(into, "Assets"))
    with open(os.path.join(into, "AppxManifest.xml"), "w", encoding="utf-8",
              newline="\r\n") as handle:
        handle.write(text)


def referenced_assets(text):
    """Every Assets\\ file the manifest names, by the name it uses."""
    return sorted(set(re.findall(r'"(Assets\\[A-Za-z0-9]+\.png)"', text))
                  | set(re.findall(r">(Assets\\[A-Za-z0-9]+\.png)<", text)))


def has_variant(directory, reference):
    """Whether a qualified file exists for an unqualified reference."""
    stem = os.path.splitext(reference.split("\\", 1)[1])[0]
    return bool(glob.glob(os.path.join(directory, "Assets", stem + ".*.png")))


def sdk_tool(name):
    """The newest x64 copy of a Windows SDK tool, or None."""
    found = shutil.which(name)
    if found:
        return found
    base = os.path.join(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)"),
                        "Windows Kits", "10", "bin")
    candidates = glob.glob(os.path.join(base, "10.*", "x64", name + ".exe"))
    key = lambda p: [int(x) for x in os.path.basename(os.path.dirname(os.path.dirname(p))).split(".")]
    return max(candidates, key=key) if candidates else None


def run(command, **kwargs):
    print("+ " + " ".join(command), flush=True)
    return subprocess.run(command, **kwargs).returncode


def build(args):
    missing = [n for n in FILES if not os.path.isfile(os.path.join(args.bin_dir, n))]
    if missing:
        print(f"{args.bin_dir} is missing {', '.join(missing)}", file=sys.stderr)
        return 1
    if args.test_identity:
        name, publisher, display = TEST_IDENTITY
    else:
        name, publisher, display = args.identity_name, args.publisher, args.publisher_display_name
        if not (name and publisher and display):
            print("the three identity values are needed, or --test-identity", file=sys.stderr)
            return 2
    try:
        text = manifest(args.version, name, publisher, display)
    except ValueError as why:
        print(why, file=sys.stderr)
        return 1
    os.makedirs(args.out_dir, exist_ok=True)
    if args.stage_only:
        into = os.path.join(args.out_dir, "msix")
        os.makedirs(into, exist_ok=True)
        stage(args.bin_dir, into, text)
        print(f"laid out {into}")
        return 0
    makeappx, makepri = sdk_tool("makeappx"), sdk_tool("makepri")
    if not (makeappx and makepri):
        print("makeappx and makepri come with the Windows SDK; use --stage-only elsewhere",
              file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory() as scratch:
        into = os.path.join(scratch, "package")
        os.makedirs(into)
        stage(args.bin_dir, into, text)
        config = os.path.join(scratch, "priconfig.xml")
        # The resource index is what lets Windows choose the right size of
        # each icon for the display it is drawn on.
        code = run([makepri, "createconfig", "/cf", config, "/dq", "en-GB", "/o"])
        code = code or run([makepri, "new", "/pr", into, "/cf", config,
                            "/mn", os.path.join(into, "AppxManifest.xml"),
                            "/of", os.path.join(into, "resources.pri"), "/o"])
        target = os.path.join(args.out_dir, msix_name(args.version))
        code = code or run([makeappx, "pack", "/d", into, "/p", target, "/o"])
        if code == 0:
            print(f"wrote {target}")
        return code


def self_test():
    failures = 0

    def check(ok, what):
        nonlocal failures
        print(("ok   " if ok else "FAIL ") + what)
        failures += not ok

    text = manifest("0.1.23", "12345tilas01.VeilVoice",
                    "CN=00000000-0000-0000-0000-000000000000", "tilas01")
    check('Version="0.1.23.0"' in text, "the package version is four numbers, the last 0")
    check('Alias="veilvoice.exe"' in text, "veilvoice.exe is on PATH through an alias")
    check('Alias="veilvoice-gui.exe"' in text, "veilvoice-gui.exe is on PATH through an alias")
    check('AppListEntry="none"' in text, "the command line is kept out of the Start menu")
    check('Name="runFullTrust"' in text and 'Name="microphone"' in text,
          "it asks for full trust and the microphone")
    check("internetClient" not in text, "it does not ask for the network")
    for bad in (("0.1", "a.b.c", "CN=x", "p"), ("0.1.23", "a b", "CN=x", "p"),
                ("0.1.23", "abc", "O=x", "p"), ("0.1.23", "abc", "CN=x", " ")):
        try:
            manifest(*bad)
            check(False, f"{bad} is refused")
        except ValueError:
            check(True, f"{bad} is refused")
    escaped = manifest("0.1.23", "abc", 'CN=a "b" & c', "x < y")
    check("&amp;" in escaped and "&lt;" in escaped and "&quot;" in escaped,
          "identity values are escaped into the XML")

    with tempfile.TemporaryDirectory() as scratch:
        bins = os.path.join(scratch, "bin")
        os.makedirs(bins)
        for n in FILES:
            with open(os.path.join(bins, n), "wb") as handle:
                handle.write(n.encode())
        into = os.path.join(scratch, "pkg")
        os.makedirs(into)
        stage(bins, into, text)
        refs = referenced_assets(text)
        check(len(refs) == 6, f"the manifest names six logos ({len(refs)})")
        for ref in refs:
            check(has_variant(into, ref), f"{ref} has sized variants in the package")
        for n in FILES:
            check(os.path.isfile(os.path.join(into, n)), f"{n} is in the package")

    print(f"\n{failures} failure(s)" if failures else "\nall checks pass")
    return 1 if failures else 0


def main():
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    parser = argparse.ArgumentParser(description=__doc__.strip().splitlines()[0])
    parser.add_argument("bin_dir")
    parser.add_argument("out_dir")
    parser.add_argument("--version", required=True, help="e.g. 0.1.23, without the v")
    parser.add_argument("--identity-name", default=os.environ.get("MSIX_IDENTITY_NAME", ""))
    parser.add_argument("--publisher", default=os.environ.get("MSIX_PUBLISHER", ""))
    parser.add_argument("--publisher-display-name",
                        default=os.environ.get("MSIX_PUBLISHER_DISPLAY_NAME", ""))
    parser.add_argument("--test-identity", action="store_true",
                        help="a throwaway identity, for a package only a build machine installs")
    parser.add_argument("--stage-only", action="store_true")
    args = parser.parse_args()
    args.version = args.version.lstrip("v")
    return build(args)


if __name__ == "__main__":
    sys.exit(main())
