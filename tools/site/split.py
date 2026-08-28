#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Give every section of the front page its own address, without a second copy.

    python tools/site/split.py           # write the section pages
    python tools/site/split.py --check   # verify they are current

# What this does, and the one thing it refuses to do

`website/index.html` has seven sections. Each is worth linking to directly --
"read the verification steps" is a thing people send each other, and a fragment
into a long page arrives without a title, without a heading of its own, and
scrolled to somewhere the reader did not choose.

So each section also gets a real page: its own `<title>`, its own `<h1>`, and a
URL that means something.

**The section pages are derived from `index.html`, not written beside it.** Two
hand-maintained copies of the same prose is finding F-41 waiting to happen --
website assets drifting from the generator that produced them, silently, with
nothing able to tell. Here the front page is the source and these are its
output, so they cannot disagree: `--check` regenerates into memory and compares,
and CI fails if they have parted company.

# Every published fragment still works, and that is not negotiable

`index.html` is **not modified**. Every `#what`, `#download`, `#verify`,
`#crypto` link that has ever been published still lands exactly where it did:
the sections are all still on the front page, in order, with their ids.

That matters more than the tidiness of removing them. Thirteen links to
`#verify` exist in this repository alone, and a fragment cannot be redirected
server-side -- browsers do not send it -- so a fragment whose target has moved
lands the reader at the top of a page with no explanation. The only safe split
is an additive one.

Pure standard library. No build step, no dependencies.
"""

import io
import os
import re
import sys

# The sections worth their own page, and what to call each one.
#
# `repo` is deliberately absent: it is a live panel that fetches from GitHub and
# means nothing on its own, and `demo` is an illustration of the section above
# it rather than a destination.
SECTIONS = [
    ("what", "What VeilVoice does", "The eight things it does, and the honest scope of each."),
    ("download", "Download VeilVoice", "Builds for ten platforms, signed, with verification instructions."),
    ("guide", "How to use VeilVoice", "A walkthrough: anonymise a file, scramble a microphone, verify a download."),
    ("verify", "Verify a download", "Check that what you downloaded is what was published, in your browser or with the portable verifier."),
    ("crypto", "Security and cryptography", "The primitives, the threat model, and what the app lock is and is not."),
]

SECTION_RE = re.compile(
    r'<section id="(?P<id>[a-z-]+)"[^>]*>(?P<body>.*?)</section>', re.S)


def repo_root():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(here, "..", ".."))


def read(path):
    with io.open(path, encoding="utf-8") as handle:
        return handle.read()


def shell(index_html):
    """The parts of the front page every section page reuses.

    Taken from `index.html` rather than written again, so a change to the
    header, the theme picker or the footer reaches these pages without anybody
    remembering to make it twice.
    """
    head_start = index_html.index("<head>")
    head_end = index_html.index("</head>") + len("</head>")
    header_start = index_html.index('<header class="top">')
    header_end = index_html.index("</header>") + len("</header>")
    footer_start = index_html.index("<footer")
    footer_end = index_html.index("</footer>") + len("</footer>")
    scripts = re.findall(r'<script src="[^"]+"[^>]*></script>', index_html)
    return {
        "head": index_html[head_start:head_end],
        "header": index_html[header_start:header_end],
        "footer": index_html[footer_start:footer_end],
        "scripts": scripts,
    }


def page(parts, section_id, title, description, body, relink_body=True):
    """One section, as a complete document.

    `relink_body` is for the callers that are not sections. A section's body
    came out of `index.html` and its `#anchor` links point at *other* sections,
    which exist on the front page and not on this one, so they are rewritten.
    A page written for itself, like the questions page with its own contents
    list, has `#anchor` links that point at its own headings, and rewriting
    those sends every one of them to the front page where they land nowhere.

    Found by `source.test.js`, which checks that a fragment naming another page
    resolves on it. Twenty of them did not.

    The header and the footer are relinked either way: the navigation is shared
    from `index.html` and is full of `#what` and `#download`, whoever is
    calling.
    """
    head = parts["head"]
    head = re.sub(r"<title>.*?</title>", "<title>%s &mdash; VeilVoice</title>" % title,
                  head, count=1, flags=re.S)
    head = re.sub(r'<meta name="description" content="[^"]*">',
                  '<meta name="description" content="%s">' % description,
                  head, count=1)

    # Any `#other-section` link resolves on the front page and nowhere else, so
    # every one is rewritten to `index.html#other-section`. A split that
    # quietly breaks the internal links has traded one problem for a worse one,
    # and `html.test.js` catches it: it checks that every fragment on a page
    # has a target on that page.
    #
    # The **header** needs this as much as the body does -- the nav is shared
    # from index.html and is full of `#what`, `#download`, `#verify`. Missing
    # that was the first thing the suite reported.
    def relink(text):
        return re.sub(r'href="#([a-z-]+)"', r'href="index.html#\1"', text)

    if relink_body:
        body = relink(body)
    header = relink(parts["header"])
    footer = relink(parts["footer"])

    return "\n".join([
        "<!doctype html>",
        "<!-- SPDX-License-Identifier: GPL-3.0-or-later -->",
        "<!-- GENERATED by tools/site/split.py from website/index.html.",
        "     Do not edit: edit the section in index.html and run the tool.",
        "     Verified in CI with `python tools/site/split.py --check`. -->",
        '<html lang="en" data-theme="tokyo-night">',
        head,
        "<body>",
        header,
        '<main class="wrap" style="padding-top:30px">',
        "<h1>%s</h1>" % title,
        '<p class="lede">%s</p>' % description,
        '<p style="color:var(--muted)">'
        'This section is also part of <a href="index.html">the front page</a>, '
        'where it sits in context with the rest.</p>',
        '<section id="%s">' % section_id,
        body,
        "</section>",
        "</main>",
        footer,
        "</body>",
        "</html>",
        "",
    ])


def build(root):
    index_path = os.path.join(root, "website", "index.html")
    index_html = read(index_path)
    parts = shell(index_html)

    found = {m.group("id"): m.group("body") for m in SECTION_RE.finditer(index_html)}
    missing = [i for i, _, _ in SECTIONS if i not in found]
    if missing:
        raise SystemExit(
            "website/index.html no longer has these sections: %s\n"
            "Either they were renamed -- in which case update SECTIONS here and\n"
            "keep the old ids as anchors so published links survive -- or the\n"
            "parser needs fixing." % ", ".join(missing))

    out = {}
    for section_id, title, description in SECTIONS:
        body = found[section_id]
        # The section's own <h2> is replaced by the page's <h1>, so it would
        # otherwise appear twice.
        body = re.sub(r"<h2[^>]*>.*?</h2>", "", body, count=1, flags=re.S)
        out["website/%s.html" % section_id] = page(
            parts, section_id, title, description, body.strip())
    return out


def main():
    root = repo_root()
    files = build(root)
    check = "--check" in sys.argv

    problems = []
    for rel, text in sorted(files.items()):
        path = os.path.join(root, rel.replace("/", os.sep))
        if check:
            try:
                with io.open(path, encoding="utf-8", newline="") as handle:
                    actual = handle.read()
            except OSError:
                problems.append("%s: missing" % rel)
                continue
            if actual.replace("\r\n", "\n") != text:
                problems.append("%s: differs from index.html" % rel)
        else:
            with io.open(path, "w", encoding="utf-8", newline="\n") as handle:
                handle.write(text)

    if check:
        if problems:
            for line in problems:
                print("  MISMATCH %s" % line)
            print()
            print("Run 'python tools/site/split.py' and commit the result.")
            return 1
        print("  section pages match index.html (%d pages)" % len(files))
        return 0

    print("  wrote %d section pages from website/index.html" % len(files))
    return 0


if __name__ == "__main__":
    sys.exit(main())
