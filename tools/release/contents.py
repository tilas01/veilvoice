#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""The signed list of what is inside each release archive.

    python tools/release/contents.py staging > staging/CONTENTS.sha256

# What this produces, and why a release publishes it

`SHA256SUMS` covers the archives. That proves a download is the one that was
published and says nothing at all about the folder somebody unzipped it into,
which is the copy they actually run. Nothing on disk records which archive a
directory was extracted from, so until this existed a verifier could only
report the two separately and advise unzipping the checked file again.

This lists every file inside every archive with its SHA-256. The release job
writes it **before** `SHA256SUMS` is computed, so the hash list covers it and
the signature therefore covers it too, and the chain runs all the way down:

    SHA256SUMS.asc -> SHA256SUMS -> CONTENTS.sha256 -> each file on disk

`veilvoice-check`'s `contents` module is the reader. The format is deliberately
the shape of `sha256sum` output with a `# <archive>` line before each group, so
that somebody with neither VeilVoice nor a parser can use it by eye.

# Why this is a script and not six lines of YAML

It was six lines of YAML, and six lines of YAML cannot be run on a laptop, cannot
be tested, and are exercised for the first time on the day a release goes out.
This is the one link in the chain above that nothing else checks, and a defect
in it would appear as a verifier confidently reporting on files it had never
compared.

`crates/veilvoice-verify/tests/release_manifest.rs` builds a synthetic release,
runs this, and reads the result back with the parser that will read it for real.
That test is the whole reason this is a file.

# Why the standard library and not `tar` and `unzip`

`zipfile` and `tarfile` read both formats without either tool being installed,
which matters because the publish job runs on one runner and the archives were
built on five. It also removes a difference this had already: `unzip -d` writes
the files out and hashes them from disk, so an unpacking quirk of the runner
would silently become part of the published list. Reading the members straight
out of the archive hashes what the archive actually holds.

# In plain words

Writes down the fingerprint of every file inside every release archive, so a
checker can tell you the program in your folder is the one that was published,
rather than only that the zip you downloaded was.
"""

import argparse
import hashlib
import sys
import tarfile
import zipfile
from pathlib import Path

# The archive kinds this project publishes. A file that is not one of these is
# not walked into: `SHA256SUMS` itself, the signature and the key sit in the
# same directory and are covered by the hash list rather than by this.
TARBALLS = (".tar.gz", ".tgz")
ZIPS = (".zip",)

# Read in chunks. A release archive is tens of megabytes and there is no reason
# for this to hold one in memory, let alone five.
CHUNK = 1 << 20


def digest(stream):
    """The SHA-256 of a stream, read in chunks."""
    hasher = hashlib.sha256()
    while True:
        block = stream.read(CHUNK)
        if not block:
            break
        hasher.update(block)
    return hasher.hexdigest()


class Refused(Exception):
    """A member path this will not write down."""


def member_path(raw, archive):
    """One archive member's path, as the manifest records it.

    **F-102.** This was `name.replace("\\", "/").lstrip("./")`, and `lstrip`
    takes a *set of characters* rather than a prefix. Measured: `.hidden/file`
    came out as `hidden/file` and `../escape` came out as `escape`.

    Both are bad and the second is worse. A dotfile in a release would be
    published under a name no file on disk has, so every verifier would report
    it missing on a release that is perfectly sound. And a member that climbs
    out of the release directory would be quietly rewritten into one that looks
    ordinary -- sanitised into acceptability, which is exactly what the reader's
    own note says must never happen, because a manifest with such a path in it
    is not a manifest with one bad line: it is a file that did not come from
    this project's release job.

    So: exactly one leading `./` is removed, which is a thing `tar` genuinely
    writes, and anything else that would not survive the reader is refused here
    rather than published. The rule is the reader's rule
    (`veilvoice_check::contents::parse`), stated on this side too, so the two
    ends of this seam agree by construction rather than by attention.
    """
    name = raw.replace("\\", "/")
    if name.startswith("./"):
        name = name[2:]
    if not name or name.endswith("/"):
        raise Refused("%s: a member with no name" % archive)
    if name.startswith("/"):
        raise Refused("%s: an absolute path, %r" % (archive, raw))
    if len(name) > 1 and name[1] == ":":
        raise Refused("%s: a drive letter, %r" % (archive, raw))
    if any(part in ("..", ".") for part in name.split("/")):
        raise Refused("%s: a path that leaves the release, %r" % (archive, raw))
    return name


def members_of_zip(path):
    """Every file inside a zip, as `(path, sha256)`, sorted by path.

    Directories are skipped: they carry no bytes and a verifier compares files.
    So is anything whose name is empty after normalisation, which a zip is free
    to contain and which no release of this project has ever held.
    """
    out = []
    with zipfile.ZipFile(path) as archive:
        for info in archive.infolist():
            if info.is_dir():
                continue
            name = member_path(info.filename, path.name)
            with archive.open(info) as member:
                out.append((name, digest(member)))
    return sorted(out)


def members_of_tar(path):
    """Every file inside a tar.gz, as `(path, sha256)`, sorted by path.

    Only regular files. A tar can hold links, devices and directories; a link
    is a name rather than content, and the reader refuses one where a file
    should be (see F-99), so listing one here would publish a hash for
    something the checker will never accept.
    """
    out = []
    with tarfile.open(path, "r:gz") as archive:
        for info in archive:
            if not info.isfile():
                continue
            name = member_path(info.name, path.name)
            member = archive.extractfile(info)
            if member is None:
                continue
            out.append((name, digest(member)))
    return sorted(out)


def archives_in(directory):
    """Every release archive in a directory, sorted by name."""
    found = []
    for path in sorted(Path(directory).iterdir()):
        if not path.is_file():
            continue
        name = path.name.lower()
        if name.endswith(TARBALLS) or name.endswith(ZIPS):
            found.append(path)
    return found


def manifest(directory):
    """The whole `CONTENTS.sha256`, as text.

    Two spaces between the hash and the path, as `sha256sum` writes it, and a
    blank line between archives so it can be read by eye. The order is by
    archive name and then by path inside it, so two runs over the same
    directory produce the same bytes and a diff between two releases is a diff
    about the releases.
    """
    lines = []
    for archive in archives_in(directory):
        if archive.name.lower().endswith(ZIPS):
            members = members_of_zip(archive)
        else:
            members = members_of_tar(archive)
        lines.append(f"# {archive.name}")
        for path, sha in members:
            lines.append(f"{sha}  {path}")
        lines.append("")
    return "\n".join(lines) + ("\n" if lines and lines[-1] != "" else "")


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("directory", help="the staging directory holding the archives")
    parser.add_argument(
        "-o",
        "--output",
        help="write here rather than to standard output",
    )
    args = parser.parse_args(argv)

    directory = Path(args.directory)
    if not directory.is_dir():
        print(f"not a directory: {directory}", file=sys.stderr)
        return 1

    found = archives_in(directory)
    if not found:
        # An empty manifest would be published, covered by the hash list, and
        # read by a verifier as "this release lists nothing inside its
        # archives", which is a lie about a release that has archives. Better
        # to fail the job.
        print(f"no release archives in {directory}", file=sys.stderr)
        return 1

    try:
        text = manifest(directory)
    except Refused as why:
        # The release job stops here. An archive holding a path like that was
        # not built by this project, and publishing a manifest that every
        # verifier will refuse is worse than not publishing one.
        print("refusing to write a contents list: %s" % why, file=sys.stderr)
        return 1
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8", newline="\n")
        # Counted from the lines that are files, not derived from the line
        # count and the number of archives. The derivation was off by one per
        # archive: measured, it reported five files for six.
        files = sum(1 for line in text.splitlines() if line and not line.startswith("# "))
        print(f"  {len(found)} archive(s), {files} files", file=sys.stderr)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
