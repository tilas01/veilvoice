#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Every sentence a person reads is spelt, built and pitched the way a careful
person would write it.

# What is read

The prose this project publishes: every tracked Markdown, HTML and text file,
and the message of every commit that has not been pushed yet. Code is not
prose, so fenced and inline code, `<code>`, `<pre>`, scripts, styles, tags and
addresses are blanked out first, keeping the line numbers where they were.
The wiki and the website's copies of the documents are skipped, because they
are generated from files this already reads, and one mistake should be one
report.

# What is asked of it

- **Spelling.** British spelling, which is the house style: `colour`,
  `behaviour`, `organise`. And no misspelling `codespell` knows of, when it is
  installed. CI installs it and sets `VEILVOICE_REQUIRE_CODESPELL=1`, so there
  it is required; on a machine without it the check says that it skipped it,
  in as many words.
- **Grammar.** No word written twice in a row (`the the`), and `a` and `an`
  the right way round before an ordinary lower-case word.
- **Tone.** Nothing a professional document does not say: placeholders
  (`TODO`, `TBD`, `lorem ipsum`), chat filler (`I hope this helps`, `Great
  question`), slang (`gonna`, `kinda`), and stacked punctuation (`!!`, `?!`).
- **Fluency.** No space before a comma or a full stop, and no word cut off in
  the middle by a generator that shortens text.

What this cannot do is read for sense. A sentence can pass every rule here and
still be wrong, which is why the documents are also read before a release.

Each rule has a short list of what it deliberately allows, with the reason, so
an exception is visible rather than silent. `--self-test` runs each rule
against text it must catch and text it must leave alone.

Pure standard library, apart from the optional `codespell`.
"""

from __future__ import annotations

import html
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

PROSE = (".md", ".html", ".txt", ".rs")

# Generated copies of documents read elsewhere, and generated indexes.
COPIES = (
    "wiki/",
    "website/wiki/",
    "website/reference/",
    "website/nojs/",
    "assets/screenshots/",  # recorded program output, kept byte for byte
)

# Texts that must be reproduced exactly as their authors wrote them.
VERBATIM = (
    "LICENSE",
    "website/user-agreements/LICENSE.txt",
    "user-agreements/LICENSE.txt",
)

# American spellings, and the British form the house style uses. Stems, so
# `colours`, `coloured` and `colourful` are all caught by `colo(u)r`.
AMERICAN = [
    (r"colors?|colored|coloring|colorful", "colour"),
    (r"behaviors?|behavioral", "behaviour"),
    (r"favors?|favored|favorite|favorites", "favour"),
    (r"honors?|honored", "honour"),
    (r"neighbors?|neighboring", "neighbour"),
    (r"centers?|centered|centering", "centre"),
    (r"analyz(?:e|es|ed|ing|er|ers)", "analyse"),
    (r"organiz(?:e|es|ed|ing|ation|ations)", "organise"),
    (r"recogniz(?:e|es|ed|ing|able)", "recognise"),
    (r"initializ(?:e|es|ed|ing|ation)", "initialise"),
    (r"serializ(?:e|es|ed|ing|ation)", "serialise"),
    (r"normaliz(?:e|es|ed|ing|ation)", "normalise"),
    (r"prioritiz(?:e|es|ed|ing)", "prioritise"),
    (r"customiz(?:e|es|ed|ing|ation)", "customise"),
    (r"minimiz(?:e|es|ed|ing)", "minimise"),
    (r"maximiz(?:e|es|ed|ing)", "maximise"),
    (r"summariz(?:e|es|ed|ing)", "summarise"),
    (r"apologiz(?:e|es|ed|ing)", "apologise"),
    (r"defenses?", "defence"),
    (r"offenses?", "offence"),
    (r"gray|grayed", "grey"),
    (r"canceled|canceling", "cancelled"),
    (r"labeled|labeling", "labelled"),
    (r"modeled|modeling", "modelled"),
    (r"traveled|traveling|traveler", "travelled"),
    (r"signaled|signaling", "signalled"),
    (r"fulfill|fulfills|fulfillment", "fulfil"),
    (r"enroll|enrolls|enrollment", "enrol"),
    (r"catalogs?|cataloged", "catalogue"),
    (r"artifacts?", "artefact"),
]
# Where an American spelling is a name rather than a choice: an API, a CSS
# property, a product. A word is let through when one of these phrases holds
# it on the same line.
NAMES = (
    "Color32",                  # egui's colour type, named in prose about the code
    "color-scheme",             # the CSS property
    "upload-artifact",          # GitHub Actions
    "download-artifact",
    "minimized* Ubuntu",        # what Ubuntu calls its cut-down images
)

AMERICAN_RE = re.compile(
    r"(?<![\w-])(" + "|".join(stem for stem, _ in AMERICAN) + r")(?![\w-])",
    re.IGNORECASE,
)

# A word written twice. Some pairs are English: "that that" can be right, and
# "had had" is a tense.
DOUBLED_RE = re.compile(r"\b([A-Za-z]+) (\1)\b", re.IGNORECASE)
DOUBLED_ALLOWED = {"that", "had", "is", "do", "so"}

# `a` before a vowel sound, `an` before a consonant sound. Lower-case words
# only, because an initialism is read by its letters ("an SVG", "a URL") and
# no list of those stays complete. The exceptions are words whose first letter
# does not make the sound it usually does.
A_BEFORE_VOWEL_RE = re.compile(r"(?<![\w-])[Aa] ([aeiou][a-z]+)\b")
A_ALLOWED = ("one", "once", "uni", "use", "usa", "usu", "ur", "uti", "ubi",
             "euro", "eu", "ewe")
AN_BEFORE_CONSONANT_RE = re.compile(r"(?<![\w-])[Aa]n ([b-df-hj-np-tv-z][a-z]+)\b")
AN_ALLOWED = ("hour", "honest", "honour", "heir", "ff",
              "nginx", "x")  # said "engine-x"; and "an xrun", "an x86 build"

# What a professional document does not say.
TONE = [
    (re.compile(r"\b(TODO|FIXME|TBD|XXX)\b"), "a placeholder"),
    (re.compile(r"lorem ipsum", re.IGNORECASE), "a placeholder"),
    (re.compile(r"\bI hope this helps\b|\bGreat question\b|\bAs an AI\b"
                r"|\bCertainly!|\bdelve[sd]?\b|\bdelving\b", re.IGNORECASE),
     "chat filler"),
    (re.compile(r"\b(gonna|wanna|gotta|kinda|sorta|lol|omg|btw|imo)\b",
                re.IGNORECASE), "slang"),
    (re.compile(r"[!?]{2,}"), "stacked punctuation"),
]

# A space before a comma, full stop, semicolon or colon that ends a word.
SPACE_BEFORE_RE = re.compile(r"[A-Za-z] [,;](?=\s|$)|[A-Za-z] \.(?=\s|$)")

# A word a generator cut in half and ended with "(done)" or similar: the
# roadmap's titles were shortened to a length and lost the end of their last
# word ("off by defaul (done)").
CUT_RE = re.compile(r"\b(defaul|releas|generat|witho|befor|thi|th|wh|wi)\s+\((?:done|next|planned|blocked)\)")

# Words codespell reports that are right here: names, quoted output, test
# fixtures and identifiers that reach prose inside a quotation.
CODESPELL_ALLOWED = [
    "fo",          # a truncated column in quoted program output
    "hda",         # Intel's "HDA" sound card name
    "destoyed",    # the misread text a test fixture quotes on purpose
    "edgre",
    "inforce",     # an identifier, `KeepInForce`
    "ontop",
    "uptodate",
    "positionof",
    "fpr",
    "inout",
    "ot",
    "resett",
    "signatur",    # German, quoted from gpg's output
    "covert",      # a covert channel, which is a real term
    "lightening",  # making a colour lighter, which is the meaning wanted
    "unparseable",
    "pre-selected",
    "thi",         # reported separately, as a cut word
]


def git(args):
    done = subprocess.run(["git"] + args, cwd=ROOT, capture_output=True)
    return done.stdout.decode("utf-8", "replace") if done.returncode == 0 else ""


def tracked():
    names = git(["ls-files", "-z"]).split("\0")
    out = []
    for name in names:
        if not name or not name.endswith(PROSE):
            continue
        if name.startswith(COPIES) or name in VERBATIM:
            continue
        out.append(name)
    return sorted(out)


def blank(match):
    """Replace a match with spaces, keeping its line breaks."""
    return re.sub(r"[^\n]", " ", match.group(0))


def markdown_prose(text):
    lines = text.split("\n")
    fenced = None
    out = []
    for line in lines:
        stripped = line.lstrip()
        if fenced:
            if stripped.startswith(fenced):
                fenced = None
            out.append("")
            continue
        if stripped.startswith(("```", "~~~")):
            fenced = stripped[:3]
            out.append("")
            continue
        if line.startswith("    ") or line.startswith("\t"):
            out.append("")
            continue
        out.append(line)
    text = "\n".join(out)
    text = re.sub(r"<!--.*?-->", blank, text, flags=re.DOTALL)
    text = re.sub(r"`[^`\n]*`", blank, text)
    text = re.sub(r"\]\([^)\n]*\)", blank, text)
    text = re.sub(r"https?://\S+", blank, text)
    text = re.sub(r"<[^>\n]*>", blank, text)
    return text


def html_prose(text):
    for element in ("script", "style", "pre", "code", "kbd", "samp"):
        text = re.sub(r"<%s\b.*?</%s>" % (element, element), blank, text,
                      flags=re.DOTALL | re.IGNORECASE)
    text = re.sub(r"<!--.*?-->", blank, text, flags=re.DOTALL)
    text = re.sub(r"<[^>]*>", blank, text, flags=re.DOTALL)
    text = re.sub(r"https?://\S+", blank, text)
    lines = [html.unescape(line) for line in text.split("\n")]
    return "\n".join(lines)


def rust_prose(text):
    """The doc comments of a Rust file, which become the generated documents."""
    out = []
    for line in text.split("\n"):
        stripped = line.lstrip()
        if stripped.startswith(("///", "//!")):
            out.append(stripped[3:])
        else:
            out.append("")
    return markdown_prose("\n".join(out))


def generated_from_elsewhere(name, text):
    """A Markdown file a generator wrote from sources this reads directly.

    The generated web pages are still read, because how a generator shortens
    or joins text is only visible in what it wrote.
    """
    return name.endswith(".md") and "GENERATED" in text[:600]


def prose_of(name, text):
    if name.endswith(".html"):
        return html_prose(text)
    if name.endswith(".rs"):
        return rust_prose(text)
    return markdown_prose(text)


def line_faults(line):
    """Every rule but spelling's dictionary, on one line of prose."""
    faults = []
    for match in AMERICAN_RE.finditer(line):
        word = match.group(1)
        if any(word in name and name in line for name in NAMES):
            continue
        for stem, british in AMERICAN:
            if re.fullmatch(stem, word, re.IGNORECASE):
                faults.append("American spelling %r (house style: %s)" % (word, british))
                break
    for match in DOUBLED_RE.finditer(line):
        first, second = match.group(1), match.group(2)
        if first.lower() in DOUBLED_ALLOWED:
            continue
        # "ROADMAP roadmap" names a file and then says what it is.
        if first.isupper() != second.isupper() and len(first) > 1:
            continue
        faults.append("doubled word %r" % match.group(0))
    for match in A_BEFORE_VOWEL_RE.finditer(line):
        if not match.group(1).startswith(A_ALLOWED):
            faults.append("'a %s' wants 'an'" % match.group(1))
    for match in AN_BEFORE_CONSONANT_RE.finditer(line):
        if not match.group(1).startswith(AN_ALLOWED):
            faults.append("'an %s' wants 'a'" % match.group(1))
    for pattern, what in TONE:
        for match in pattern.finditer(line):
            faults.append("%s: %r" % (what, match.group(0)))
    for match in SPACE_BEFORE_RE.finditer(line):
        faults.append("a space before punctuation: %r" % match.group(0))
    for match in CUT_RE.finditer(line):
        faults.append("a word cut short: %r" % match.group(0))
    return faults


def codespell(files):
    """Misspellings in `files`, a {name: prose} map, or None without codespell."""
    program = shutil.which("codespell")
    if program is None:
        return None
    with tempfile.TemporaryDirectory() as scratch:
        for name, prose in files.items():
            path = os.path.join(scratch, name)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(prose)
        done = subprocess.run(
            [program, "--quiet-level", "3",
             "--ignore-words-list", ",".join(CODESPELL_ALLOWED), "."],
            cwd=scratch, capture_output=True)
        found = []
        for line in done.stdout.decode("utf-8", "replace").splitlines():
            where, _, what = line.partition(": ")
            name, _, number = where.lstrip("./").rpartition(":")
            if number.isdigit():
                found.append((name, int(number), "misspelt: %s" % what))
        return found


def commit_messages():
    """The commits not on any remote yet, as (sha, message)."""
    spec = os.environ.get("WORDING_COMMITS")
    if spec:
        shas = git(["rev-list"] + spec.split()).split()
    else:
        shas = git(["rev-list", "HEAD", "--not", "--remotes"]).split()
    return [(sha, git(["log", "-1", "--format=%B", sha])) for sha in shas]


def main():
    faults = []
    files = {}
    for name in tracked():
        try:
            with open(os.path.join(ROOT, name), encoding="utf-8") as handle:
                text = handle.read()
        except (OSError, UnicodeDecodeError):
            continue
        if generated_from_elsewhere(name, text):
            continue
        files[name] = prose_of(name, text)
    for sha, message in commit_messages():
        files["commit %s" % sha[:8]] = markdown_prose(message)

    for name, prose in files.items():
        for number, line in enumerate(prose.split("\n"), 1):
            for fault in line_faults(line):
                faults.append((name, number, fault))

    spelt = codespell({n: p for n, p in files.items() if not n.startswith("commit ")}
                      | {n.replace(" ", "-"): p for n, p in files.items() if n.startswith("commit ")})
    if spelt is None:
        if os.environ.get("VEILVOICE_REQUIRE_CODESPELL") == "1":
            print("  codespell is required here and is not installed:")
            print("    python -m pip install --require-hashes -r tools/audit/requirements.txt")
            return 1
        print("  codespell is not installed, so misspellings were not looked for.")
        print("  CI looks for them. To here as well:")
        print("    python -m pip install -r tools/audit/requirements.txt")
    else:
        faults.extend((n.replace("commit-", "commit ", 1), line, what) for n, line, what in spelt)

    if faults:
        print("  %d place(s) where the writing needs another look:" % len(faults))
        for name, number, fault in sorted(faults):
            print("    %s:%d  %s" % (name, number, fault))
        print()
        print("  The house style is British spelling, whole sentences and a")
        print("  professional tone. A word that is right here but reported (a")
        print("  name, quoted output) goes in the allowed list in this file,")
        print("  with the reason.")
        return 1

    print("  %d file(s) and %d unpushed commit message(s) read; nothing to change"
          % (sum(1 for n in files if not n.startswith("commit ")),
             sum(1 for n in files if n.startswith("commit "))))
    return 0


def self_test():
    must_catch = [
        "The colour of the button, and its color.",
        "It was was fine.",
        "Take a apple.",
        "Take an banana.",
        "TODO: write this.",
        "I hope this helps!",
        "This is kinda broken.",
        "Really!!",
        "A list , badly spaced.",
        "off by defaul (done)",
        "The behavior changed.",
        "We organized it.",
        "The the start.",
    ]
    must_pass = [
        "The colour of the button.",
        "He said that that was fine.",
        "An SVG and a URL, a one-off and a user, an hour and an honest answer.",
        "Behind an nginx proxy, an xrun.",
        "The README file, the ROADMAP roadmap.",
        "| render   | Render the file |",
        "Use Color32 for it.",
        "Download it with download-artifact.",
        "The meter reads zero.",
        "It works. Then it stops; then, it starts.",
        "Organise and analyse, then recognise.",
    ]
    failed = 0
    for text in must_catch:
        if not line_faults(text):
            print("  self-test: %r was not caught" % text)
            failed += 1
    for text in must_pass:
        found = line_faults(text)
        if found:
            print("  self-test: %r was reported: %s" % (text, found))
            failed += 1
    if line_faults(rust_prose("let color = 1; // color\n/// Uses `color`.")):
        print("  self-test: Rust code was read as prose")
        failed += 1
    prose = markdown_prose("Text `color` here.\n```\ncolor\n```\n[x](http://a/color)")
    if line_faults(prose.replace("\n", " ")):
        print("  self-test: code in Markdown was read as prose")
        failed += 1
    page = html_prose("<p>Fine.</p><code>color</code><style>a{color:red}</style>")
    if line_faults(page):
        print("  self-test: code in HTML was read as prose")
        failed += 1
    if failed:
        return 1
    print("  every rule catches what it should and leaves the rest")
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv[1:]:
        sys.exit(self_test())
    sys.exit(main())
