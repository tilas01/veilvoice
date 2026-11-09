#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build the two Windows installers from a folder holding the two programs.

    python tools/package/msi.py BIN_DIR OUT_DIR --version 0.1.23
    python tools/package/msi.py --self-test

BIN_DIR is the folder the release's Windows zip unpacks to: `veilvoice.exe`,
`veilvoice-gui.exe`, `LICENSE` and `README.md`. The installers wrap exactly
those files, so a program installed from either one is byte for byte the
program in the zip, which the release's signed CONTENTS.sha256 lists.

Writes, for version 0.1.23:

    veilvoice-v0.1.23-windows-x86_64-user.msi      no administrator needed
    veilvoice-v0.1.23-windows-x86_64-machine.msi   everyone on the computer

Needs the WiX Toolset 5 command-line tool on PATH, with its UI and Util
extensions. Version 5 on purpose: from 6 onwards its binaries come under a
separate maintenance-fee licence, while 5.0.2 is under the Microsoft
Reciprocal Licence alone, an OSI-approved open-source licence.

    dotnet tool install --global wix --version 5.0.2
    wix extension add --global WixToolset.UI.wixext/5.0.2
    wix extension add --global WixToolset.Util.wixext/5.0.2

# Two things this works out so that a person does not have to

The product code. Windows Installer needs a new one for every version, and
winget uses it to recognise an installed copy. It is derived from the version
and the scope, so building the same version twice gives the same code and
nothing has to be written down anywhere.

The licence. The installer's licence page reads RTF, and the repository holds
the GPL as plain text. It is converted here, unaltered, rather than a second
copy of it being kept in a format nobody reviews.

# In plain words

Makes the two Windows installers, one that needs no administrator and one that
installs for everybody, from the programs in the Windows download.
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import uuid

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SOURCE = os.path.join(ROOT, "packaging", "wix", "veilvoice.wxs")

# A fixed namespace for the product codes. Changing it changes every code, so
# an upgrade could no longer find the version before it. Never change it.
NAMESPACE = uuid.UUID("3c6f7a52-1d0e-4b9a-8e57-0f2b6d4c9a13")

SCOPES = ("user", "machine")
FILES = ("veilvoice.exe", "veilvoice-gui.exe", "LICENSE", "README.md")
WIX_EXTENSIONS = ("WixToolset.UI.wixext", "WixToolset.Util.wixext")


def product_code(version, scope):
    """The same version and scope always give the same code."""
    return str(uuid.uuid5(NAMESPACE, f"veilvoice {version} {scope} x64")).upper()


def msi_name(version, scope):
    return f"veilvoice-v{version}-windows-x86_64-{scope}.msi"


def rtf(text):
    """Plain text as RTF, every character kept.

    Monospaced, because the GPL is laid out with spaces and reads badly
    reflowed. Anything outside ASCII is written as a Unicode escape so the
    file means the same thing whatever code page reads it.
    """
    out = [r"{\rtf1\ansi\deff0{\fonttbl{\f0\fmodern Consolas;}}\f0\fs16 "]
    for ch in text.replace("\r\n", "\n"):
        if ch == "\n":
            out.append("\\par\n")
        elif ch in "\\{}":
            out.append("\\" + ch)
        elif ord(ch) < 128:
            out.append(ch)
        else:
            code = ord(ch)
            if code > 0xFFFF:
                # RTF's \u takes a signed 16-bit value: a character outside
                # the basic plane goes as its UTF-16 surrogate pair.
                code -= 0x10000
                pair = (0xD800 + (code >> 10), 0xDC00 + (code & 0x3FF))
            else:
                pair = (code,)
            for unit in pair:
                out.append("\\u%d?" % (unit - 0x10000 if unit > 0x7FFF else unit))
    out.append("}")
    return "".join(out)


def build(bin_dir, out_dir, version):
    missing = [name for name in FILES if not os.path.isfile(os.path.join(bin_dir, name))]
    if missing:
        print(f"{bin_dir} is missing {', '.join(missing)}", file=sys.stderr)
        return 1
    wix = shutil.which("wix")
    if not wix:
        print("wix is not on PATH; see this script's docstring", file=sys.stderr)
        return 1
    os.makedirs(out_dir, exist_ok=True)
    with tempfile.TemporaryDirectory() as scratch:
        licence = os.path.join(scratch, "LICENSE.rtf")
        with open(os.path.join(bin_dir, "LICENSE"), encoding="utf-8") as handle:
            text = handle.read()
        with open(licence, "w", encoding="ascii", newline="\n") as handle:
            handle.write(rtf(text))
        for scope in SCOPES:
            target = os.path.join(out_dir, msi_name(version, scope))
            command = [wix, "build", SOURCE, "-arch", "x64"]
            for extension in WIX_EXTENSIONS:
                command += ["-ext", extension]
            command += [
                "-d", f"Version={version}",
                "-d", f"Scope={scope}",
                "-d", f"ProductCode={product_code(version, scope)}",
                "-d", f"BinDir={os.path.abspath(bin_dir)}",
                "-d", f"LicenceRtf={licence}",
                "-o", target,
            ]
            print("+ " + " ".join(command), flush=True)
            # Run from the repository root: the source names assets/icon.ico
            # relative to it.
            result = subprocess.run(command, cwd=ROOT)
            if result.returncode != 0:
                return result.returncode
            # wix writes a .wixpdb beside the package. It is a debugging aid
            # for the build machine, not something a release should carry.
            pdb = os.path.splitext(target)[0] + ".wixpdb"
            if os.path.exists(pdb):
                os.remove(pdb)
            print(f"wrote {target}")
    return 0


def self_test():
    failures = 0

    def check(ok, what):
        nonlocal failures
        print(("ok   " if ok else "FAIL ") + what)
        failures += not ok

    a = product_code("0.1.23", "user")
    check(a == product_code("0.1.23", "user"), "a product code is the same every time")
    check(a != product_code("0.1.24", "user"), "a new version gets a new product code")
    check(a != product_code("0.1.23", "machine"), "the two scopes get different codes")
    check(str(uuid.UUID(a)).upper() == a, "a product code is an upper-case GUID")

    sample = "a {b} c\\d\nlicence \u00e9 \u2014 \U0001F512\n"
    out = rtf(sample)
    check(out.startswith("{\\rtf1") and out.endswith("}"), "the RTF is one group")
    check("\\{b\\}" in out and "c\\\\d" in out, "braces and backslashes are escaped")
    check(out.count("\\par") == 2, "each line ends in a paragraph mark")
    check("\\u233?" in out and "\\u8212?" in out, "accents and dashes survive as escapes")
    check("\\u-10179?\\u-8942?" in out, "a character beyond 16 bits becomes a surrogate pair")
    check(all(ord(c) < 128 for c in out), "the RTF is plain ASCII")

    with open(os.path.join(ROOT, "LICENSE"), encoding="utf-8") as handle:
        licence = handle.read()
    converted = rtf(licence)
    check(converted.count("\\par") == licence.replace("\r\n", "\n").count("\n"),
          "the whole licence converts, one paragraph per line")

    with open(SOURCE, encoding="utf-8") as handle:
        source = handle.read()
    for name in ("veilvoice.exe", "veilvoice-gui.exe"):
        check(f"$(var.BinDir)\\{name}" in source, f"the installer source names {name}")
    check(source.count('Scope) = "user"') >= 1 and 'Scope) = "machine"' in source,
          "the installer source knows both scopes")
    check('Id="PathEntry"' in source and 'Level="1"' in source,
          "PATH is a feature that is on by default")

    print(f"\n{failures} failure(s)" if failures else "\nall checks pass")
    return 1 if failures else 0


def main():
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    parser = argparse.ArgumentParser(description=__doc__.strip().splitlines()[0])
    parser.add_argument("bin_dir")
    parser.add_argument("out_dir")
    parser.add_argument("--version", required=True, help="e.g. 0.1.23, without the v")
    args = parser.parse_args()
    return build(args.bin_dir, args.out_dir, args.version.lstrip("v"))


if __name__ == "__main__":
    sys.exit(main())
