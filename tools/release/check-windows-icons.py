#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Check that a built Windows executable actually carries its icon.

    python tools/release/check-windows-icons.py target/release

# Why this is a check rather than a comment

`crates/*/build.rs` embeds `assets/icon.ico` into each Windows binary's
resource section. Nothing else verifies that it worked, and the failure is
completely silent: the build succeeds, every test passes, and the only symptom
is that Explorer draws the generic executable glyph. The icon was already
missing from every release for exactly that reason -- it was *shipped beside*
the binary, where Windows never looks.

So this reads the PE the linker produced and asserts the resource is in it. It
runs in the release workflow, on the artefacts that are about to be signed.

Pure standard library, and it parses only the few header fields it needs rather
than pulling in a PE library to answer one question.
"""

import os
import struct
import sys

# A resource section holding six icon images plus a version block is a few
# kilobytes. Anything much smaller means the resource exists but the icon did
# not go into it, which is a different failure with the same symptom.
MINIMUM_RSRC_BYTES = 2000

BINARIES = ("veilvoice-gui.exe", "veilvoice.exe", "veilvoice-verify.exe")


def rsrc_size(path):
    """Bytes in the PE's `.rsrc` section, or None if there is no such section."""
    with open(path, "rb") as handle:
        head = handle.read(2048)
    if head[:2] != b"MZ":
        raise ValueError("not a PE image (no MZ signature)")
    pe = struct.unpack("<I", head[0x3C:0x40])[0]
    if head[pe:pe + 4] != b"PE\0\0":
        raise ValueError("not a PE image (no PE signature)")
    sections = struct.unpack("<H", head[pe + 6:pe + 8])[0]
    optional = struct.unpack("<H", head[pe + 20:pe + 22])[0]
    table = pe + 24 + optional
    for index in range(sections):
        entry = table + index * 40
        name = head[entry:entry + 8].rstrip(b"\0").decode("ascii", "replace")
        if name == ".rsrc":
            return struct.unpack("<I", head[entry + 8:entry + 12])[0]
    return None


def main():
    if len(sys.argv) != 2:
        print(__doc__.strip().splitlines()[2])
        return 2
    directory = sys.argv[1]

    problems = []
    checked = 0
    for name in BINARIES:
        path = os.path.join(directory, name)
        if not os.path.exists(path):
            # Not every job builds every binary; absence is not this check's
            # business, and inventing a requirement here would fail builds for
            # a reason that has nothing to do with icons.
            continue
        checked += 1
        try:
            size = rsrc_size(path)
        except (OSError, ValueError) as error:
            problems.append("%s: %s" % (name, error))
            continue
        if size is None:
            problems.append("%s: no .rsrc section -- the icon was not embedded" % name)
        elif size < MINIMUM_RSRC_BYTES:
            problems.append(
                "%s: .rsrc is only %d bytes, so the resource exists but the "
                "icon is not in it" % (name, size))
        else:
            print("  ok    %-22s .rsrc %d bytes" % (name, size))

    if checked == 0:
        print("  no Windows binaries found in %s" % directory)
        print("  This check must not pass vacuously: point it at a directory")
        print("  containing the built executables.")
        return 1

    if problems:
        print()
        for line in problems:
            print("  MISSING %s" % line)
        print()
        print("  `crates/*/build.rs` should have embedded assets/icon.ico.")
        print("  A binary with no icon looks unfinished in Explorer, on the")
        print("  taskbar and in a pinned shortcut, and nothing else notices.")
        return 1

    print("  all %d Windows binaries carry their icon" % checked)
    return 0


if __name__ == "__main__":
    sys.exit(main())
