#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build the search index for the repository and the website.

    python tools/search-index/generate.py           # write the index
    python tools/search-index/generate.py --check   # verify it is current

Two things come out of this script, and they exist for two different readers:

  * ``website/search-index.json`` -- the machine-readable index. ``search.js``
    fetches it and scores against it. Nobody reads this by hand.
  * ``website/nojs/search.html`` -- a complete, static, browsable index of every
    file and every section in the project. It needs no JavaScript at all: the
    whole corpus is *in the page*, so a reader's own find-in-page searches it.

The second is not a courtesy stub. ``website/nojs/`` is a supported edition of
this site, and "search" that silently does nothing without JavaScript would be
exactly the kind of quiet degradation this project audits itself against. The
static page therefore carries the same entries as the JSON, from the same walk,
in the same order -- it is the same index rendered for a different reader.

# Why generated, and why ``--check``

Same reason as ``assets/generate.py``: an index committed as an opaque blob is
one more thing a reader has to take on trust, and a stale index is a search box
that confidently reports the wrong thing. The output is deterministic -- files
walked in sorted order, no timestamps, no absolute paths, LF endings, sorted
JSON keys -- so ``--check`` regenerates into memory and compares. CI runs it,
so an index that has drifted from the tree fails the build rather than shipping.

# What is indexed, stated precisely

Everything ``git ls-files`` reports that is text, split into *sections*. What a
section contains depends on the format, and the difference is worth stating
plainly rather than rounding up to "everything":

  * **Markdown** -- one section per heading, carrying **all** the prose under
    it. Complete.
  * **HTML** -- one section per heading, carrying **all** the text up to the
    next heading. Complete.
  * **Rust** -- one section per item (``fn``, ``struct``, ``enum``, ``trait``,
    ``mod``, ``const``, ...), carrying the item's name and its doc comment.
    **Not the function bodies.** The doc comments in this codebase are the
    argument for the code, so they are the part worth searching; the statements
    inside a function are not. This is the one deliberate gap, and it means
    searching for a local variable will not find it.
  * **Everything else** (JavaScript, CSS, TOML, YAML, licence texts, ...) --
    the whole file, in consecutive chunks. Complete.

Where a body is longer than ``MAX_EXCERPT`` it is **split into several
sections**, never truncated: the bound is on how much one result *displays*,
not on how much is searched. An earlier version truncated, which quietly left
most of every long file out of the index while the page claimed to search it.

Pure standard library. No build step, no dependencies.
"""

import html
import json
import os
import re
import subprocess
import sys

# --- limits -----------------------------------------------------------------
# Each of these bounds the output, and each is far past anything in this tree.
# They exist so that a file nobody expected -- a vendored blob, a generated
# table, a lock file that grew -- cannot silently turn the index into something
# a phone has to download.
MAX_EXCERPT = 240        # characters shown in one result
MAX_SECTIONS_PER_DOC = 400
MAX_FILE_BYTES = 512 * 1024
MAX_HEADING = 120

# This script's own output is tracked, lives under `website/`, and would
# otherwise be walked like any other file -- which does not merely bloat the
# index, it stops it converging. Indexing the index makes the next run's input
# contain the previous run's output, so the file grows on every regeneration
# and `--check` can never agree with a freshly built one. Excluded by path, and
# asserted by a test, because the failure looks like flaky CI rather than a bug.
GENERATED = frozenset({
    "website/search-index.json",
    "website/nojs/search.html",
})

# `tools/docs/generate.py` renders the doc comments in the source into four
# places: a README per crate, a page per file, the website reference, and the
# GitHub wiki. All four are the *same prose*, and that prose is already indexed
# at its origin -- the `.rs` files, whose `//!` and `///` comments this walk
# reads directly.
#
# Indexing the renderings as well would return four results for one sentence,
# three of them copies, and would roughly quadruple the megabyte a reader's
# browser downloads to search at all. So the source is indexed and the
# renderings are not, which is the same decision as excluding this script's own
# output above, for the same reason.
GENERATED_PREFIXES = (
    "docs/files/",
    "website/reference/",
    "wiki/",
    "assets/banners/",
    "website/assets/banners/",
)
CRATE_README = re.compile(r"^crates/[^/]+/README\.md$")


def is_generated(rel):
    return (rel in GENERATED
            or rel.startswith(GENERATED_PREFIXES)
            or bool(CRATE_README.match(rel)))

REPO = "tilas01/veilvoice"
REF = "main"

# --- what kind of thing is this ---------------------------------------------
# Ordered: the first pattern that matches wins.
KIND_RULES = [
    (re.compile(r"^crates/[^/]+/(src|examples)/.*\.rs$"), "rust"),
    (re.compile(r"^crates/[^/]+/tests/.*\.rs$"), "test"),
    (re.compile(r"^fuzz/.*\.rs$"), "test"),
    (re.compile(r"^website/nojs/"), "web"),
    (re.compile(r"^website/user-agreements/"), "legal"),
    (re.compile(r"^website/"), "web"),
    (re.compile(r"^tools/"), "tool"),
    (re.compile(r"^assets/"), "tool"),
    (re.compile(r"^\.github/"), "build"),
    (re.compile(r"\.md$"), "doc"),
    (re.compile(r"\.(toml|lock|yml|yaml)$"), "build"),
    (re.compile(r"^(LICENSE|COPYING)"), "legal"),
    (re.compile(r"\.(txt)$"), "legal"),
]

KIND_LABELS = [
    ("doc", "Documentation"),
    ("rust", "Rust source"),
    ("test", "Tests and fuzzing"),
    ("web", "Website"),
    ("tool", "Tools and generators"),
    ("build", "Build and CI"),
    ("legal", "Licence and legal"),
    ("other", "Other"),
]

BINARY = re.compile(
    r"\.(png|jpg|jpeg|gif|ico|icns|rgba|woff2?|ttf|otf|pdf|zip|gz|asc|wav|mp3|flac)$", re.I
)

# Rust items worth an index entry of their own.
RUST_ITEM = re.compile(
    r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?"
    r"(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+\"[^\"]*\"\s+)?"
    r"(fn|struct|enum|trait|mod|type|const|static|union|macro_rules!)\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)"
)
RUST_IMPL = re.compile(r"^\s*impl(?:\s*<[^>]*>)?\s+(.+?)\s*\{?\s*$")
HTML_HEADING = re.compile(
    r"<h([1-4])\b([^>]*)>(.*?)</h\1>", re.I | re.S
)
HTML_ID = re.compile(r"""\bid\s*=\s*["']([^"']+)["']""", re.I)
TAG = re.compile(r"<[^>]+>")


def repo_root():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(here, "..", ".."))


def tracked_files(root):
    """Every file git tracks -- exactly the set that ships."""
    out = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=root, check=True, stdout=subprocess.PIPE,
    ).stdout.decode("utf-8")
    names = [n for n in out.split("\0") if n]
    return sorted(names)


def untracked_files(root):
    """Files git does not track and is not ignoring."""
    out = subprocess.run(
        ["git", "ls-files", "-z", "--others", "--exclude-standard"],
        cwd=root, check=True, stdout=subprocess.PIPE,
    ).stdout.decode("utf-8")
    return sorted(n for n in out.split("\0") if n)


def warn_about_untracked(root):
    """Say so when a new file would be indexed but has not been staged yet.

    The index is built from `git ls-files`, which lists *tracked* files. Write a
    new document, generate, and commit it in one step and the index you commit
    was built without it -- because at the moment it was built, git had never
    heard of it. CI then regenerates from the committed tree, finds one more
    file, and fails with "differs from the generator output", which is true and
    tells you nothing about why.

    That happened while this feature was being built, so the failure is now
    explained where it can be acted on rather than discovered from a red CI run
    ten minutes later. It is a warning rather than a refusal: an untracked file
    is a perfectly ordinary state to be in, and the generator should still work.
    """
    would_index = [
        rel for rel in untracked_files(root)
        if not BINARY.search(rel) and not is_generated(rel)
    ]
    if not would_index:
        return
    print()
    print("  NOTE: %d file(s) are not tracked by git, so they are NOT in this"
          % len(would_index))
    print("  index. `git ls-files` is what this walks, and it lists tracked")
    print("  files only:")
    for rel in would_index[:10]:
        print("    %s" % rel)
    if len(would_index) > 10:
        print("    ...and %d more" % (len(would_index) - 10))
    print()
    print("  If they belong in the index, stage them and generate again:")
    print("    git add -A && python tools/search-index/generate.py")
    print()


def kind_of(rel):
    for pattern, kind in KIND_RULES:
        if pattern.search(rel):
            return kind
    return "other"


def area_of(rel):
    """The part of the project a file belongs to, for filtering."""
    parts = rel.split("/")
    if parts[0] == "crates" and len(parts) > 1:
        return parts[1]
    if parts[0] in ("website", "tools", "docs", "fuzz", "assets", ".github"):
        return parts[0]
    return "root"


def squash(text):
    """One line of plain text, collapsed, with no stray whitespace."""
    return re.sub(r"\s+", " ", text).strip()


def chunks(text):
    """Split text into bounded pieces that together cover **all** of it.

    This deliberately splits rather than truncates, and the difference is the
    whole honesty of the feature. The first version kept the first 240
    characters of each section and dropped the rest, which meant the index
    covered roughly an eighth of a long file while the page said it searched
    every file. It was caught by a test asking whether searching for `onerror`
    -- a string this repository definitely contains, in its own hostile-input
    fixtures -- found anything. It did not.

    A search box that silently does not look at most of the corpus is worse
    than no search box, because it answers "no results" with the same
    confidence either way. So every character of every indexed file now lands
    in exactly one chunk, and the bound applies to how much text a single
    result *displays*, not to how much is searched.
    """
    text = squash(text)
    if not text:
        return []
    if len(text) <= MAX_EXCERPT:
        return [text]

    out = []
    at = 0
    while at < len(text):
        cut = text[at:at + MAX_EXCERPT]
        if at + MAX_EXCERPT < len(text):
            # Break on a word boundary so a snippet reads as a sentence, but
            # never lose the tail: the next chunk resumes exactly where this
            # one stopped.
            space = cut.rfind(" ")
            if space > MAX_EXCERPT * 0.6:
                cut = cut[:space]
        out.append(cut.strip())
        at += len(cut)
        while at < len(text) and text[at] == " ":
            at += 1
    return [c for c in out if c]


def slug(text):
    """GitHub's heading anchor rule, which is what both GitHub and this site use."""
    s = squash(text).lower()
    s = re.sub(r"[^\w\- ]+", "", s, flags=re.UNICODE)
    return s.replace(" ", "-")


# --- per-format section extraction ------------------------------------------

def sections_markdown(text):
    out = []
    current = {"h": "", "anchor": "", "line": 1, "body": []}
    in_fence = False
    for number, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("```") or stripped.startswith("~~~"):
            in_fence = not in_fence
            continue
        heading = re.match(r"^(#{1,6})\s+(.*)$", line) if not in_fence else None
        if heading:
            if current["h"] or current["body"]:
                out.append(current)
            title = squash(heading.group(2))
            title = re.sub(r"[*`_]", "", title)
            current = {
                "h": title[:MAX_HEADING],
                "anchor": slug(title),
                "line": number,
                "body": [],
            }
        else:
            current["body"].append(line)
    if current["h"] or current["body"]:
        out.append(current)
    return out


def sections_rust(text):
    """One entry per item, carrying the doc comment written above it.

    The doc comments in this codebase are the argument for the code -- they say
    why a thing is done the way it is. Indexing the signature without them would
    make the search find names and miss reasons.
    """
    out = []
    lines = text.splitlines()

    module_doc = []
    for line in lines:
        s = line.strip()
        if s.startswith("//!"):
            module_doc.append(s[3:].strip())
        elif s and not s.startswith("//"):
            break
    if module_doc:
        out.append({"h": "(module)", "anchor": "", "line": 1, "body": module_doc})

    pending = []
    for number, line in enumerate(lines, start=1):
        s = line.strip()
        if s.startswith("///"):
            pending.append(s[3:].strip())
            continue
        if s.startswith("#[") or s.startswith("#!["):
            continue
        item = RUST_ITEM.match(line)
        name = None
        if item:
            name = item.group(1) + " " + item.group(2)
        else:
            impl = RUST_IMPL.match(line)
            # Only a real `impl` header, not a line that happens to start with it.
            if impl and ("{" in line or line.rstrip().endswith("{")):
                name = "impl " + squash(impl.group(1))
        if name:
            out.append({
                "h": name[:MAX_HEADING],
                "anchor": "",
                "line": number,
                "body": pending[:],
            })
        if s:
            pending = []
    return out


def sections_html(text):
    out = []
    for match in HTML_HEADING.finditer(text):
        attrs = match.group(2)
        title = squash(html.unescape(TAG.sub(" ", match.group(3))))
        if not title:
            continue
        ident = HTML_ID.search(attrs)
        line = text.count("\n", 0, match.start()) + 1
        # Everything after this heading, up to the next one. Not a fixed window:
        # a window drops whatever falls past it, which is the truncation bug
        # `chunks()` exists to avoid, arriving through a different door.
        tail = text[match.end():]
        nxt = HTML_HEADING.search(tail)
        if nxt:
            tail = tail[:nxt.start()]
        body = squash(html.unescape(TAG.sub(" ", tail)))
        out.append({
            "h": title[:MAX_HEADING],
            "anchor": ident.group(1) if ident else "",
            "line": line,
            "body": [body],
        })
    return out


def sections_plain(text):
    """Fall-back: the file in bounded chunks, so long files stay findable."""
    lines = text.splitlines()
    out = []
    step = 40
    for start in range(0, len(lines), step):
        chunk = lines[start:start + step]
        body = squash("\n".join(chunk))
        if not body:
            continue
        out.append({"h": "", "anchor": "", "line": start + 1, "body": [body]})
    return out


def sections_for(rel, text):
    if rel.endswith(".md"):
        return sections_markdown(text)
    if rel.endswith(".rs"):
        return sections_rust(text)
    if rel.endswith(".html"):
        return sections_html(text)
    return sections_plain(text)


# --- links ------------------------------------------------------------------

def doc_url(rel, line):
    """Where a result sends the reader.

    Website pages are on this site, so they get a same-site link to the exact
    section. Everything else lives in the repository and gets a GitHub link with
    a line number, because that is where the file actually is.
    """
    if rel.startswith("website/"):
        local = rel[len("website/"):]
        return local
    anchor = "#L%d" % line if line and line > 1 else ""
    return "https://github.com/%s/blob/%s/%s%s" % (REPO, REF, rel, anchor)


# --- the index --------------------------------------------------------------

def build(root):
    docs = []
    secs = []

    for rel in tracked_files(root):
        if BINARY.search(rel) or is_generated(rel):
            continue
        full = os.path.join(root, rel.replace("/", os.sep))
        try:
            size = os.path.getsize(full)
        except OSError:
            continue
        if size > MAX_FILE_BYTES:
            continue
        try:
            with open(full, "r", encoding="utf-8") as handle:
                text = handle.read()
        except (OSError, UnicodeDecodeError):
            continue
        if "\0" in text:
            continue

        kind = kind_of(rel)
        title = rel.rsplit("/", 1)[-1]
        doc_index = len(docs)
        found = sections_for(rel, text)

        kept = 0
        for section in found:
            if kept >= MAX_SECTIONS_PER_DOC:
                break
            body = section["body"]
            body_text = " ".join(body) if isinstance(body, list) else str(body)
            pieces = chunks(body_text)
            if not pieces:
                # A heading with nothing under it is still worth finding.
                pieces = [""] if section["h"] else []
            for offset, piece in enumerate(pieces):
                if kept >= MAX_SECTIONS_PER_DOC:
                    break
                entry = {
                    "d": doc_index,
                    # Continuations keep the heading so a result still says
                    # where it is, and the reader is not shown a bare fragment.
                    "h": section["h"],
                    "l": section["line"],
                    "x": piece,
                }
                if section["anchor"] and offset == 0:
                    entry["a"] = section["anchor"]
                secs.append(entry)
                kept += 1

        docs.append({
            "p": rel,
            "t": title,
            "k": kind,
            "r": area_of(rel),
            "n": text.count("\n") + 1,
            "b": size,
            "u": doc_url(rel, 0),
        })

    areas = sorted({d["r"] for d in docs})
    kinds = [[k, label] for k, label in KIND_LABELS if any(d["k"] == k for d in docs)]

    return {
        "v": 1,
        "repo": REPO,
        "ref": REF,
        "kinds": kinds,
        "areas": areas,
        "docs": docs,
        "secs": secs,
    }


def render_json(index):
    # ensure_ascii keeps the file pure ASCII, so no viewer can turn a character
    # of it into mojibake; sort_keys and fixed separators keep it byte-stable.
    return json.dumps(index, ensure_ascii=True, sort_keys=True,
                      separators=(",", ":")) + "\n"


# --- the static, no-JavaScript index ----------------------------------------

def esc(text):
    """HTML-escape, and force the result to ASCII.

    Numeric references keep the page byte-identical whatever a reader's viewer
    guesses about encoding -- the same reason `website/js/*.js` is ASCII.
    """
    out = html.escape(text, quote=True)
    return out.encode("ascii", "xmlcharrefreplace").decode("ascii")


def render_static(index):
    kinds = dict(KIND_LABELS)
    by_kind = {}
    for number, doc in enumerate(index["docs"]):
        by_kind.setdefault(doc["k"], []).append((number, doc))

    secs_by_doc = {}
    for section in index["secs"]:
        secs_by_doc.setdefault(section["d"], []).append(section)

    total_docs = len(index["docs"])
    total_secs = len(index["secs"])

    out = []
    add = out.append
    add('<!DOCTYPE html>')
    add('<html lang="en">')
    add('<head>')
    add('<meta charset="utf-8">')
    add('<meta name="viewport" content="width=device-width, initial-scale=1">')
    add('<title>Search index &mdash; VeilVoice (no JavaScript)</title>')
    add('<meta name="description" content="A complete static index of every '
        'file and section in VeilVoice. No JavaScript required.">')
    # Markup, not script, so it works in this edition exactly as in the other.
    add('<link rel="prefetch" href="index.html">')
    add('<link rel="prefetch" href="../index.html">')
    add('<style>')
    add(':root{--bg:#1a1b26;--fg:#c0caf5;--muted:#737aa2;--accent:#7aa2f7;'
        '--accent-2:#bb9af7;--border:#414868;--bg-inset:#16161e;color-scheme:dark}')
    add('*{box-sizing:border-box}')
    add('body{background:var(--bg);color:var(--fg);margin:0;padding:20px;'
        'font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;'
        'font-size:14px;line-height:1.65}')
    add('main{max-width:900px;margin:0 auto}')
    add('a{color:var(--accent)}')
    add('h1{font-size:24px;margin:8px 0 4px;letter-spacing:.04em}')
    add('h2{font-size:16px;color:var(--accent);margin:30px 0 8px;'
        'border-bottom:1px solid var(--border);padding-bottom:6px}')
    add('p.lead{color:var(--muted);margin:6px 0 18px}')
    add('details{border:1px solid var(--border);border-radius:8px;'
        'background:var(--bg-inset);margin:8px 0;padding:6px 12px}')
    add('summary{cursor:pointer;padding:4px 0}')
    add('summary code{color:var(--fg)}')
    add('.meta{color:var(--muted);font-size:12px}')
    add('ul{margin:6px 0 10px;padding-left:20px}')
    add('ul.toc{columns:2;column-gap:24px}')
    add('ul.toc li{break-inside:avoid}')
    add('li{margin:4px 0}')
    add('.sec{color:var(--accent-2)}')
    add('.x{color:var(--muted)}')
    add('nav.top{margin-bottom:14px}')
    add('nav.top a{margin-right:14px}')
    add('.js-toggle{display:inline-flex;align-items:center;gap:7px;border:0;'
        'color:var(--muted);font-size:13px;min-height:24px;white-space:nowrap}')
    add('.js-toggle-track{position:relative;width:30px;height:16px;flex:none;'
        'border:1px solid var(--border);border-radius:999px;'
        'background:var(--bg-inset)}')
    add('.js-toggle-knob{position:absolute;top:2px;left:2px;width:10px;'
        'height:10px;border-radius:50%;background:var(--muted)}')
    add('</style>')
    add('</head>')
    add('<body>')
    add('<main>')
    add('<nav class="top">')
    add('<a href="index.html">no-JavaScript edition</a>')
    add('<a class="js-toggle" href="../index.html" role="switch" '
        'aria-checked="false" title="Switch to the full site, which runs '
        'scripts. This control changes no browser setting.">'
        '<span>JavaScript</span>'
        '<span class="js-toggle-track" aria-hidden="true">'
        '<span class="js-toggle-knob"></span></span>'
        '<span class="js-toggle-state">off</span></a>')
    add('<a href="../search.html">live search</a>')
    add('<a href="https://github.com/%s">repository</a>' % esc(REPO))
    add('</nav>')
    add('<h1>Search index</h1>')
    add('<p class="lead">Every file and every section in VeilVoice &mdash; '
        '%d files, %d sections &mdash; listed in full on this page. '
        'There is no JavaScript here and nothing to load: use your browser\'s '
        'own find-in-page (Ctrl+F, or Cmd+F on a Mac) to search it. '
        'Section headings and their opening text are included, so searching for '
        'a term finds the place it is discussed, not just the file name.</p>' %
        (total_docs, total_secs))
    add('<p class="lead">The <a href="../search.html">live search</a> on the '
        'main site scores and ranks the same index, and adds sorting and '
        'filtering. It needs JavaScript. This page does not, and is generated '
        'from the same walk of the repository, so the two never disagree.</p>')
    add('<p class="lead">Every entry is expanded rather than folded away, '
        'deliberately: text inside a collapsed section is not searchable by '
        'find-in-page on every browser, and an index that answers confidently '
        'with nothing would be worse than no index. That makes this a long '
        'page. The list below jumps to each part of it.</p>')

    add('<h2 id="contents">Contents</h2>')
    add('<ul class="toc">')
    for kind, label in KIND_LABELS:
        entries = by_kind.get(kind)
        if not entries:
            continue
        add('<li><a href="#k-%s">%s</a> <span class="meta">%d files</span></li>'
            % (esc(kind), esc(label), len(entries)))
    add('</ul>')

    for kind, label in KIND_LABELS:
        entries = by_kind.get(kind)
        if not entries:
            continue
        add('<h2 id="k-%s">%s <span class="meta">(%d)</span></h2>'
            % (esc(kind), esc(label), len(entries)))
        for number, doc in entries:
            sections = secs_by_doc.get(number, [])
            # `open`, and not negotiable.
            #
            # The entire mechanism of this page is the reader's own
            # find-in-page. Content inside a *closed* `<details>` is only
            # searchable on engines that auto-expand it -- Chromium since 102,
            # and later still elsewhere -- so on an older Safari or Firefox a
            # collapsed index would answer every search with nothing while
            # looking perfectly fine. That is the precise failure mode this
            # project keeps finding in its own website (F-30, F-31, F-33), and
            # it would be worse here because the page would be *confidently*
            # empty. Expanded costs height, which is free; collapsed costs
            # correctness on browsers a great many people run.
            add('<details open>')
            add('<summary><code>%s</code> <span class="meta">&mdash; %s, %d lines,'
                ' %d sections</span></summary>'
                % (esc(doc["p"]), esc(doc["r"]), doc["n"], len(sections)))
            add('<p class="meta"><a href="%s">open %s</a></p>'
                % (esc(doc["u"]), esc(doc["t"])))
            if sections:
                add('<ul>')
                for section in sections:
                    heading = section["h"] or ("line %d" % section["l"])
                    add('<li><span class="sec">%s</span> '
                        '<span class="meta">line %d</span><br>'
                        '<span class="x">%s</span></li>'
                        % (esc(heading), section["l"], esc(section["x"])))
                add('</ul>')
            add('</details>')

    add('</main>')
    add('</body>')
    add('</html>')
    return "\n".join(out) + "\n"


# --- writing and checking ---------------------------------------------------

OUTPUTS = [
    ("website/search-index.json", render_json),
    ("website/nojs/search.html", render_static),
]


def write(root):
    index = build(root)
    for rel, render in OUTPUTS:
        path = os.path.join(root, rel.replace("/", os.sep))
        os.makedirs(os.path.dirname(path), exist_ok=True)
        # newline="\n": the committed file must be identical on every platform,
        # or --check fails on Windows for a reason that has nothing to do with
        # the index.
        with open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(render(index))
        print("  wrote %s" % rel)
    print("  %d files, %d sections" % (len(index["docs"]), len(index["secs"])))
    warn_about_untracked(root)
    return 0


def check(root):
    index = build(root)
    problems = []
    for rel, render in OUTPUTS:
        path = os.path.join(root, rel.replace("/", os.sep))
        want = render(index)
        try:
            with open(path, "r", encoding="utf-8", newline="") as handle:
                got = handle.read().replace("\r\n", "\n")
        except OSError as exc:
            problems.append("%s: cannot read (%s)" % (rel, exc))
            continue
        if got != want:
            problems.append("%s: differs from the generator output" % rel)

    if problems:
        for line in problems:
            print("  MISMATCH %s" % line)
        print()
        print("Run 'python tools/search-index/generate.py' and commit the result.")
        warn_about_untracked(root)
        return 1
    print("  search index matches the repository (%d files, %d sections)"
          % (len(index["docs"]), len(index["secs"])))
    return 0


def main():
    root = repo_root()
    if "--check" in sys.argv:
        return check(root)
    return write(root)


if __name__ == "__main__":
    sys.exit(main())
