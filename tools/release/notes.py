#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""A release's notes, written for the person who just downloaded it.

    python tools/release/notes.py --tag v0.1.23 --repro repro/ --out NOTES.md \
                                  --technical TECHNICAL.md
    python tools/release/notes.py --check   # the changelog parses, and agrees

# What this replaces, and why it is not shell any more

The release notes were a hundred and twenty lines of `echo` inside
`.github/workflows/release.yml`. That worked, and it had two costs that only
show up at the worst moment.

It could not be run. A release's notes were seen for the first time when the
release was published, or by dispatching the whole workflow, which builds
eleven platforms twice each to find out whether a sentence reads well. So
nothing about them was ever checked, and v0.1.21 published with **no notes at
all**: the changelog heading had been written `## 0.1.21 - 2026-09-10` where
the extraction wanted `## v0.1.21`, and seven hundred lines were silently
dropped. A guard was added there for that exact case. This moves the rest of
the reasoning somewhere it can be read and run.

And it could not split anything. Everything the changelog entry held went into
the notes in one undifferentiated block, in the order it happened to be
written, so a reader looking for "what is new" scrolled past the audit round,
the dependency review and the undefined-behaviour checking to find it.

# What a release now opens with, and the order

Roadmap item 180. The order is what somebody arriving with a downloaded file
actually needs, which is not the order the work was done in:

  1. **The files.** Which archive is theirs, by platform, with the name.
  2. **Checking it**, before running it, with the fingerprint that decides it.
  3. **What is new and what was fixed**, in plain words.
  4. **The technical detail**, behind a link and a fold.

The fourth is the whole change. The reproducibility table per platform, the
audit findings, the dependency changes and the measurements are not removed
and are not summarised: they move to a document of their own, published beside
the archives as `TECHNICAL.md` and carried inline in a `<details>` fold so
nobody has to download a file to read a paragraph. One source, two renderings,
which is what every other generator here does.

# How the split is decided, which is by being told

A release's technical half begins at a heading that says exactly

    ### Technical detail

and runs to the end of that entry. Everything above it is what is new and what
was fixed.

**It is declared rather than guessed.** The alternative was to sort the
entry's sections by what their headings sound like, and a rule that reads
"Dependencies, reviewed one by one" as technical and "The window draws at the
display's rate" as a feature is a rule that will be wrong, quietly, on a
release nobody re-reads. A marker in the file is a decision somebody made in
the commit that wrote the entry, where it can be reviewed.

An entry with no marker is all of it "what is new", which is exactly what
happens today, so nothing that has already been published changes meaning.
`CHANGELOG.md` is a record of the past and is not edited to agree with this.

# The fingerprint is checked here rather than stated twice

The notes tell a reader to compare a fingerprint by eye, and it is the one
value in the whole chain that a person checks themselves. It appears in the
notes, in `README.md`, in `docs/INSTALL.md`, on the website, and compiled into
`veilvoice-verify` as the value a binary refuses to trust anything else over.

So this takes the fingerprint GnuPG reports for the key that actually signed
the release, compares it against the constant compiled into the verifier, and
**fails the release** if they differ. A release signed by a key the shipped
binaries will not accept is a release that cannot be verified by the program
it contains, and nothing else was watching for it.

Pure standard library, like the rest of the tooling here. The archive names
come from `tools/site/releases.py`, which reads them out of the build matrix
in `release.yml`, rather than being listed again.
"""

from __future__ import annotations

import argparse
import io
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

sys.path.insert(0, os.path.join(ROOT, "tools", "site"))
import releases as site  # noqa: E402  (the path has to be set first)

CHANGELOG = os.path.join(ROOT, "CHANGELOG.md")

#: The heading that begins a release's technical half, and the splitter that
#: reads it. Both live in `tools/site/releases.py`, with every other piece of
#: `CHANGELOG.md` parsing, so the release page and this tool cannot come to
#: different conclusions about where an entry's two halves divide.
#:
#: Exact, including case, because a loose match is how the extraction this
#: replaces lost a whole entry: a heading that was nearly right read as no
#: heading at all. A marker worth relying on is one whose absence is a fact
#: rather than a near miss, so `--check` reports the entry it found none in
#: rather than silently treating all of it as a feature list.
MARKER = site.TECHNICAL_MARKER
split = site.split_entry
trimmed = site.trimmed

#: What the fold says before somebody opens it.
FOLD = "Click to see the technical detail: reproducibility per platform, the " \
       "audit findings, the dependency changes and the measurements"

#: The technical document, published beside the archives.
TECHNICAL_NAME = "TECHNICAL.md"


def read(path):
    with io.open(path, encoding="utf-8") as handle:
        return handle.read().replace("\r\n", "\n")


def entry(tag, text=None):
    """The lines of `## <tag>` in the changelog, or None if it has none.

    None rather than an empty list, because "the release has no entry" and
    "the release's entry is empty" are different failures and only the first
    one has ever happened.
    """
    text = read(CHANGELOG) if text is None else text
    want = "## %s" % tag
    taking = False
    out = None
    for line in text.split("\n"):
        if line.rstrip() == want:
            taking = True
            out = []
            continue
        if taking and line.startswith("## "):
            break
        if taking:
            out.append(line)
    return out


def compiled_fingerprint():
    """The fingerprint the shipped binaries will accept, from their source."""
    source = read(os.path.join(ROOT, site.FINGERPRINT_SOURCE))
    found = re.search(r'pub const FINGERPRINT: &str = "([0-9A-F]{40})";', source)
    if not found:
        raise SystemExit(
            "%s no longer declares FINGERPRINT, so a release cannot state a\n"
            "  fingerprint without typing it somewhere a check cannot reach."
            % site.FINGERPRINT_SOURCE)
    return found.group(1)


def files_section(version):
    """Which archive is which, by platform.

    GitHub lists a release's assets below the notes, as file names and sizes
    and nothing else. `veilvoice-v0.1.22-linux-arm64-musl-static.tar.gz` is
    exact and is not an answer to "which one do I want", so the same list is
    given here with the platform each one is for. Read out of the build matrix
    in `release.yml`, so a platform added there appears here.
    """
    out = ["### The files", "",
           "Every file below is attached to this release.", "",
           "| Platform | Archive |", "|---|---|"]
    for label, pattern in site.archives():
        out.append("| %s | `%s` |" % (label, pattern.format(v=version)))
    out.append("")
    out.append("And beside them:")
    out.append("")
    for name, what in site.beside_for(version):
        out.append("- `%s`, %s" % (name, what))
    out.append("- `%s`, everything technical about this build" % TECHNICAL_NAME)
    return out


def checking_section(fingerprint, signed):
    """How to check the download, which is the part to do before running it."""
    if not signed:
        return [
            "### Checking what you downloaded", "",
            "> This build is **unsigned**: no signing key is configured for "
            "this repository, so there is no signature to check and the hash "
            "list below proves only that the files arrived intact.", "",
            "```bash",
            "sha256sum -c SHA256SUMS --ignore-missing",
            "```",
        ]

    spaced = " ".join(fingerprint[i:i + 4] for i in range(0, 40, 4))
    return [
        "### Check it before you run it", "",
        "`SHA256SUMS.asc` is a detached OpenPGP signature over the hash list. "
        "No binary is signed in place, so signing cannot disturb the "
        "bit-for-bit reproducibility recorded in the technical detail below.",
        "",
        "**1. Import the key**, which is attached here and also on the "
        "[website](https://tilas01.github.io/veilvoice/#verify):", "",
        "```bash",
        "gpg --import veilvoice-signing-key.asc",
        "```", "",
        "**2. Check the fingerprint is exactly this.** A \"Good signature\" "
        "from some other key proves nothing, so this is the step that decides "
        "it and the one no tool can do for you:", "",
        "```",
        spaced,
        "```", "",
        "**3. Verify the hash list, then the files against it:**", "",
        "```bash",
        "gpg --verify SHA256SUMS.asc SHA256SUMS",
        "sha256sum -c SHA256SUMS --ignore-missing",
        "```", "",
        "Expect `Good signature from \"tilas01\"`. The key carries no e-mail "
        "address by design. GnuPG will also warn that the key is not certified "
        "with a trusted signature: that is normal, and means only that you "
        "have not personally signed it. The fingerprint check above is what "
        "establishes that it is the right key.", "",
        "On macOS use `shasum -a 256 -c SHA256SUMS --ignore-missing`.", "",
        "If you would rather not use a terminal, the desktop application's "
        "Verify tab does the same check with the same code underneath, and "
        "`veilvoice verify` does it in one command.",
    ]


def technical_document(tag, half, repro, built_on):
    """Everything about the build, as a document of its own."""
    out = [
        "<!-- SPDX-License-Identifier: GPL-3.0-or-later -->",
        "<!-- GENERATED by tools/release/notes.py for %s. -->" % tag,
        "",
        "# VeilVoice %s: the technical detail" % tag,
        "",
        "Everything about this build that is not a description of what it does "
        "for you. The release notes are the other half, and neither is a "
        "summary of the other.",
        "",
    ]
    if repro:
        out += [
            "## Reproducibility",
            "",
            "Every binary was built twice, in different directories, and the "
            "two results compared byte for byte. What each platform reported:",
            "",
        ]
        out += ["- `%s`" % line for line in repro]
        out += [
            "",
            "To reproduce a build yourself and compare it against what was "
            "published, see "
            "[`docs/REPRODUCIBLE_BUILDS.md`]"
            "(https://github.com/%s/blob/%s/docs/REPRODUCIBLE_BUILDS.md)."
            % (site.docs.REPO, tag),
            "",
        ]
    out += [
        "## The build",
        "",
        "- Built on `%s` runners." % built_on,
        "- Toolchain pinned by `rust-toolchain.toml`.",
        "- Nothing in this build reads a secret, and a fresh clone builds it.",
        "",
    ]
    if half:
        out += ["## From the changelog", ""]
        out += half
        out += [""]
    else:
        out += [
            "## From the changelog",
            "",
            "This release's entry in `CHANGELOG.md` carries no `%s` section, "
            "so everything it says is in the release notes rather than here."
            % MARKER.lstrip("# "),
            "",
        ]
    return "\n".join(out).rstrip("\n") + "\n"


def notes_document(tag, version, new, has_technical, fingerprint, signed):
    """The release body: the files, checking them, then what changed."""
    # Roadmap item 183. The card is committed, so it is addressed at this
    # release's own tag rather than as a release asset: the tag exists by the
    # time anybody reads these notes, and a picture that resolves through the
    # tag cannot start showing a later release's card the way a `main` link
    # would. The alt text says what the picture is; every word on it is in the
    # list of changes further down.
    card = ("![A card for %s, carrying the version and the changes it led "
            "with](https://github.com/%s/raw/%s/assets/changelog/v%s.png)"
            % (tag, site.docs.REPO, tag, version))
    out = [
        "## VeilVoice %s" % tag,
        "",
        card,
        "",
        "Irreversible voice de-identification, fully offline. It destroys the "
        "biometric voiceprint, pitch, formants, timbre and the melody of an "
        "accent, and leaves the words transcribable.",
        "",
    ]
    out += files_section(version)
    out += ["", "---", ""]
    out += checking_section(fingerprint, signed)
    out += ["", "---", ""]
    out += ["### What is new and what was fixed", ""]
    out += new if new else ["No changelog entry was written for this release."]
    out += ["", "---", ""]
    where = ("[`%s`](https://github.com/%s/releases/download/%s/%s), attached "
             "to this release" % (TECHNICAL_NAME, site.docs.REPO, tag,
                                  TECHNICAL_NAME))
    out += ["### The technical detail", ""]
    if has_technical:
        out += [
            "The reproducibility result for every platform, the audit "
            "findings, the dependency changes and the measurements are in "
            "%s. It is also below, if you would rather not open a file." % where,
            "",
            "<details>",
            "<summary>%s</summary>" % FOLD,
            "",
        ]
        out += has_technical
        out += ["", "</details>", ""]
    else:
        # No fold when there is nothing to fold. A `<details>` opening on an
        # empty panel reads as a page that failed to load, and a sentence
        # promising detail that is not there is worse than no sentence.
        out += [
            "The reproducibility result for every platform, and how this "
            "build was made, are in %s." % where,
            "",
        ]
    return "\n".join(out).rstrip("\n") + "\n"


def repro_lines(folder):
    """One line per platform, from the files the build jobs left behind."""
    if not folder or not os.path.isdir(folder):
        return []
    out = []
    for name in sorted(os.listdir(folder)):
        if not (name.startswith("repro-") and name.endswith(".txt")):
            continue
        out.append(read(os.path.join(folder, name)).strip())
    return [line for line in out if line]


def check():
    """The newest entry parses, and the fingerprint agrees with the binaries.

    Run by `tools/verify.py` and by CI, on every push, which is the whole
    point: the notes for the release being worked towards are checkable now
    rather than at the moment of publishing, when the answer arrives as a
    failed release or, worse, as a published one nobody can read.
    """
    text = read(CHANGELOG)
    tags = re.findall(r"^## (v\d+\.\d+\.\d+)\s*$", text, re.M)
    if not tags:
        print("  CHANGELOG.md has no '## vX.Y.Z' entries at all")
        return 1

    newest = tags[0]
    lines = entry(newest, text)
    if not lines or not trimmed(lines):
        print("  CHANGELOG.md's newest entry, %s, is empty" % newest)
        return 1

    new, technical = split(lines)
    if not new:
        print("  %s says nothing above its '%s' heading, so its release notes"
              % (newest, MARKER))
        print("  would open with no description of what changed")
        return 1

    compiled = compiled_fingerprint()
    if len(compiled) != 40:
        print("  the compiled-in fingerprint is not 40 characters")
        return 1

    # The archives are read out of the build matrix; a workflow this cannot
    # read would publish notes with no download table, so it is asserted here
    # rather than discovered during a release.
    found = site.archives()

    print("  release notes: %s parses, %d line(s) of what changed, %d of "
          "technical detail, %d archive(s) listed"
          % (newest, len(new), len(technical), len(found)))
    if not technical:
        print("  note: %s has no '%s' section, so all of it reads as what "
              "changed" % (newest, MARKER))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="verify the newest entry parses, write nothing")
    parser.add_argument("--tag", help="the release, as vX.Y.Z")
    parser.add_argument("--repro", help="the folder of repro-*.txt reports")
    parser.add_argument("--fingerprint",
                        help="the fingerprint GnuPG reports for the signing "
                             "key, compared against the compiled-in one")
    parser.add_argument("--built-on", default="ubuntu-latest",
                        help="what the runner reported as its system")
    # Both optional, and at least one required. The two are written at
    # different moments of a release and that is not a convenience: the
    # technical document is staged **before** `SHA256SUMS` is computed, so
    # the hash list covers it and the signature therefore covers it too,
    # which is the same argument `CONTENTS.sha256` is staged early for.
    # The notes cannot be written that early, because they state the
    # fingerprint of the key that has not signed anything yet.
    parser.add_argument("--out", help="write the release body here")
    parser.add_argument("--technical",
                        help="write the technical document here")
    parser.add_argument("--allow-missing", action="store_true",
                        help="a dry run: report a missing entry rather than "
                             "refusing to write")
    args = parser.parse_args()

    if args.check:
        return check()

    if not args.tag:
        parser.error("--tag is required unless --check is given")
    if not args.out and not args.technical:
        parser.error("give --out, --technical, or both")
    tag = args.tag
    version = tag.lstrip("v")

    lines = entry(tag)
    if lines is None or not trimmed(lines):
        message = (
            "CHANGELOG.md has no '## %s' section, so this release would "
            "publish with no notes. The heading must be exactly '## %s', "
            "matching every other entry in the file." % (tag, tag))
        if not args.allow_missing:
            print("::error::%s" % message)
            return 1
        print("  %s" % message)
        lines = []

    new, technical = split(lines)

    # A release signed by a key the shipped binaries will not accept cannot be
    # verified by the program it contains. Nothing else compares these two.
    compiled = compiled_fingerprint()
    signed = bool(args.fingerprint)
    if signed and args.fingerprint.strip().upper() != compiled:
        print("::error::the key that signed this release has fingerprint %s, "
              "and the binaries in it are compiled to accept %s. A release "
              "its own verifier rejects does not publish."
              % (args.fingerprint.strip().upper(), compiled))
        return 1

    repro = repro_lines(args.repro)

    wrote = []
    if args.technical:
        with io.open(args.technical, "w", encoding="utf-8",
                     newline="\n") as handle:
            handle.write(technical_document(tag, technical, repro,
                                            args.built_on))
        wrote.append("%s (%d line(s) from the changelog, %d reproducibility "
                     "report(s))" % (args.technical, len(technical),
                                     len(repro)))
    if args.out:
        with io.open(args.out, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(notes_document(tag, version, new, technical,
                                        compiled, signed))
        wrote.append("%s (%d line(s) of what changed)" % (args.out, len(new)))

    print("  wrote " + ", and ".join(wrote))
    return 0


if __name__ == "__main__":
    sys.exit(main())
