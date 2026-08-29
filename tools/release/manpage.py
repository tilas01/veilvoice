#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Man pages, written from the command's own `--help`.

    python tools/release/manpage.py target/release/veilvoice out/veilvoice.1

# Why this exists rather than a hand-written page

`lintian` reports `no-manual-page` for every binary in the Debian packages, and
it is right to: somebody who installs a package on a Unix system types
`man veilvoice` and, until now, got nothing.

A man page written by hand would be a second description of the interface, kept
beside the first by nothing but attention. This project has already paid for
that arrangement once: F-71 was two hand-typed numbers that drifted together
because each was only ever compared against the other. So the page is not
written, it is *derived*, from the one description that cannot drift from the
program: the program.

# Why not help2man

`help2man` does exactly this job and is the obvious answer. It was tried first
and it mangles the output: VeilVoice's help text contains em dashes, and every
one of them came back as `???`, at `C`, at `C.utf8`, and with `LC_ALL` set
either way. A manual page that renders the program's own description as three
question marks is worse than no manual page, because it looks finished.

This is forty lines and gets the encoding right, which is the whole of what it
adds.

# When it runs

At package build time, in `debian/rules` and in the RPM's `%install`, where the
binary exists. Nothing is committed, so nothing can go stale: the page is a
function of the binary in the package it ships beside.

# In plain words

Turns `veilvoice --help` into a manual page, so `man veilvoice` works.

It is generated from the program itself every time a package is built, rather
than written out and kept up to date by hand, because the second kind goes
wrong quietly and nobody notices until a user reads it.
"""

import argparse
import datetime
import os
import re
import subprocess
import sys

# Roff escapes for the characters that otherwise come out wrong or change
# meaning. The backslash must be first: every other replacement introduces one.
ESCAPES = [
    ("\\", "\\e"),
    ("—", "\\(em"),
    ("–", "\\(en"),
    ("’", "\\(cq"),
    ("“", "\\(lq"),
    ("”", "\\(rq"),
    ("…", "\\&..."),
    ("-", "\\-"),
]


def roff(text):
    """One line of help text, safe to put in a roff document."""
    for src, dst in ESCAPES:
        text = text.replace(src, dst)
    # A line starting with a full stop or an apostrophe is a roff request.
    if text[:1] in (".", "'"):
        text = "\\&" + text
    return text


def help_text(binary):
    """What the program says about itself."""
    out = subprocess.run(
        [binary, "--help"],
        capture_output=True,
        check=True,
        # Decode explicitly rather than trusting the locale, which is what
        # help2man got wrong.
        encoding="utf-8",
        errors="strict",
    )
    return out.stdout.replace("\r\n", "\n").rstrip("\n")


def sections(text):
    """Split help output into (heading, body-lines) pairs.

    clap emits `Usage:` and then headed blocks such as `Commands:` and
    `Options:`. Anything before the first heading is the description.
    """
    blocks = [(None, [])]
    for line in text.split("\n"):
        stripped = line.strip()
        # Two heading styles, because the two binaries do not share one. clap
        # writes `Commands:` and `Options:` in title case with a colon;
        # `veilvoice-verify` hand-writes `USAGE` and `EXIT STATUS` in capitals
        # with none. Matching only the first left the whole of the second
        # binary's help inside DESCRIPTION, re-flowed into one paragraph that
        # ran every command together.
        if not line.startswith(" ") and (
            re.fullmatch(r"[A-Z][A-Za-z ]*:", stripped)
            or re.fullmatch(r"[A-Z][A-Z ]{2,}", stripped)
        ):
            blocks.append((stripped.rstrip(":"), []))
        # `Usage:` is different: clap puts the usage on the same line as the
        # word. Missing that left the synopsis buried in the description, which
        # is where a reader looks last.
        elif stripped.startswith("Usage:") and not line.startswith(" "):
            blocks.append(("Usage", [stripped[len("Usage:"):].strip()]))
        else:
            blocks[-1][1].append(line)
    return blocks


def page(binary, name, summary, version, date):
    text = help_text(binary)
    blocks = sections(text)

    out = [
        f'.\\" Generated from `{name} --help` by tools/release/manpage.py.',
        '.\\" Do not edit: it is rewritten from the program on every package build.',
        f'.TH {name.upper()} 1 "{date}" "{name} {version}" "User Commands"',
        ".SH NAME",
        f"{roff(name)} \\- {roff(summary)}",
    ]

    # A manual page opens NAME, SYNOPSIS, DESCRIPTION, whatever clap's own
    # order happens to be. clap prints the description first and the usage
    # after it; a reader looking for the synopsis looks second, so the two are
    # swapped here rather than left in the order they arrived.
    def rank(block):
        heading = block[0]
        if heading is not None and heading.lower().startswith("usage"):
            return 0
        return 1 if heading is None else 2

    for heading, lines in sorted(blocks, key=rank):
        body = [l for l in lines if l.strip()]
        if not body:
            continue
        if heading is None:
            # A help text whose first line is its own title repeats what NAME
            # has just said. `veilvoice-gui --help` opens that way, because in
            # a terminal the title is worth having; in a manual page it is the
            # line above.
            title = f"{name} - {summary}"
            body = [l for l in body if l.strip() != title]
            if not body:
                continue
            out += [".SH DESCRIPTION"]
            # Prose: let roff fill and justify it, which is what prose wants.
            out += [roff(l.strip()) for l in body]
            continue
        if heading.lower().startswith("usage"):
            out += [".SH SYNOPSIS"]
        else:
            out += [f".SH {roff(heading.upper())}"]

        # Everything else verbatim, between `.nf` and `.fi`.
        #
        # The first version reformatted these into `.TP` terms, which looked
        # better for `veilvoice` and mangled `veilvoice-verify`: the two
        # binaries do not share a help style, because only one of them uses
        # clap. The hand-laid-out one has indented continuation lines carrying
        # its meaning, and re-flowing them ran the whole thing into a single
        # paragraph.
        #
        # A page that reproduces `--help` exactly is worth more than one that
        # is prettier for one binary and wrong for the other, and it cannot go
        # wrong for a third.
        out += [".nf"]
        # Trailing whitespace only, so an intentional blank line survives as a
        # blank line. Leading whitespace is the layout and must not be touched.
        out += [roff(line.rstrip()) for line in lines]
        out += [".fi"]

    out += [
        ".SH SEE ALSO",
        "The full documentation is in the installed"
        f" {roff('/usr/share/doc/veilvoice')} directory,",
        "and at " + roff("https://github.com/tilas01/veilvoice") + ".",
        ".SH REPORTING BUGS",
        roff("https://github.com/tilas01/veilvoice/issues"),
    ]
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("binary", help="the built executable to ask")
    ap.add_argument("output", help="where to write the roff page")
    ap.add_argument("--name", help="the command's name (default: the file name)")
    ap.add_argument("--summary", default="irreversible voice de-identification, fully offline")
    args = ap.parse_args()

    name = args.name or os.path.basename(args.binary)
    version = (
        subprocess.run(
            [args.binary, "--version"], capture_output=True, encoding="utf-8", check=True
        )
        .stdout.strip()
        .split()[-1]
    )
    # SOURCE_DATE_EPOCH so a package built twice produces the same page, which
    # is the same rule docs/REPRODUCIBLE_BUILDS.md states for everything else.
    epoch = os.environ.get("SOURCE_DATE_EPOCH")
    when = datetime.datetime.fromtimestamp(
        int(epoch) if epoch else 0, datetime.timezone.utc
    ) if epoch else datetime.datetime.now(datetime.timezone.utc)
    text = page(args.binary, name, args.summary, version, when.strftime("%Y-%m-%d"))

    os.makedirs(os.path.dirname(os.path.abspath(args.output)), exist_ok=True)
    with open(args.output, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
