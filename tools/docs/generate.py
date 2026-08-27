#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate a README for every crate, a page for every source file, and the
website mirror of both.

    python tools/docs/generate.py           # write the documentation
    python tools/docs/generate.py --check   # verify it is current

# Why this is generated rather than written

Forty-eight source files each need a description, a flowchart and a banner.
Written by hand that is forty-eight documents that start out true and drift, and
drift is what finding **F-41** was: `website/assets/` had parted company with
the generator that produced it, silently, and nobody could tell by looking.

So the description of a file is **extracted from that file's own doc comments**
(`//!` and `///`), and the flowchart is **derived from the file's own items and
the calls between them**. Neither can disagree with the source, because neither
is a separate statement about the source -- they are the source, rearranged. If
a page here reads thinly, the fix is to write the doc comment in the `.rs` file,
which improves rustdoc and the code review at the same time.

`--check` regenerates into memory and compares, exactly as `assets/generate.py`
and `tools/search-index/generate.py` do, so CI fails when the tree and its
documentation part company.

# The four decisions this file implements

Recorded in `HANDOFF.md` section 9.7, and repeated here because this is where
they are actually load-bearing.

**1. Flowcharts are Mermaid, not images.** GitHub renders ```mermaid fences
natively in Markdown, so a flowchart stays *text*: diffable, greppable,
reviewable in a pull request, and picked up by the search index like any other
prose. A PNG flowchart is an opaque blob, which is the thing this project
refuses to ship.

**2. Banners are generated SVG.** Forty-eight more PNGs is forty-eight binaries
the repository has to carry and a `--check` that has to decode each one. SVG is
text: it costs almost nothing, scales to any width, and is auditable by reading
it. Deterministic, from a hash of the name -- no timestamps, no randomness.

**3. Per-file documentation comes from the source.** See above.

**4. Website parity comes from this one generator.** The same model is rendered
twice: once as Markdown for GitHub and once as HTML for the site. Two
hand-maintained copies would drift; one model cannot.

# The one thing the website cannot do, said plainly

Mermaid is a JavaScript library. This site loads **nothing** from a third party
and has no bundler, so the website cannot run Mermaid, and shipping a copy of it
would contradict the reason the site is written the way it is.

The website therefore renders the *same graph* through a small layout engine in
this file, as inline SVG that needs no script at all -- so the diagram works in
the no-JavaScript edition too. It is the same nodes and the same edges from the
same model; it is **not** the same picture, because a different layout algorithm
draws it. That difference is stated on the page rather than glossed, and the
Mermaid source is offered beside it so a reader can render it themselves.

Pure standard library. No build step, no dependencies.
"""

import hashlib
import html as html_mod
import io
import os
import re
import subprocess
import sys

# --- what is documented -----------------------------------------------------
#
# Every crate in the workspace. The request in HANDOFF section 9.7 asked for
# one crate first as a template; the follow-up asked for all of them, so this
# is the whole list. It is kept as an explicit tuple rather than a directory
# scan so that adding a crate to the workspace and forgetting to document it
# fails the `--check` in CI against ALL_CRATES below, rather than silently
# documenting whatever happens to be on disk.
CRATES = (
    "fuzz",
    "veilvoice-appctl",
    "veilvoice-audio",
    "veilvoice-capture",
    "veilvoice-check",
    "veilvoice-cli",
    "veilvoice-conversation",
    "veilvoice-core",
    "veilvoice-crypto",
    "veilvoice-drivers",
    "veilvoice-failsafe",
    "veilvoice-guard",
    "veilvoice-input",
    "veilvoice-gui",
    "veilvoice-meta",
    "veilvoice-policy",
    "veilvoice-priv",
    "veilvoice-proc",
    "veilvoice-sentry",
    "veilvoice-setup",
    "veilvoice-update",
    "veilvoice-verify",
    "veilvoice-video",
    "veilvoice-watch",
    "veilvoice-workspace",
)

# Every crate in the workspace, so this script can say what it is *not* yet
# covering rather than quietly covering less than the tree contains. A silent
# partial pass is exactly the failure mode section 4.5 of the audit describes.
ALL_CRATES = (
    "fuzz",
    "veilvoice-appctl",
    "veilvoice-audio",
    "veilvoice-capture",
    "veilvoice-check",
    "veilvoice-cli",
    "veilvoice-conversation",
    "veilvoice-core",
    "veilvoice-crypto",
    "veilvoice-drivers",
    "veilvoice-failsafe",
    "veilvoice-guard",
    "veilvoice-input",
    "veilvoice-gui",
    "veilvoice-meta",
    "veilvoice-policy",
    "veilvoice-priv",
    "veilvoice-proc",
    "veilvoice-sentry",
    "veilvoice-setup",
    "veilvoice-update",
    "veilvoice-verify",
    "veilvoice-video",
    "veilvoice-watch",
    "veilvoice-workspace",
)

def workspace_crates(root):
    """Every crate `Cargo.toml` lists as a member, plus `fuzz`.

    # Why this exists

    [`ALL_CRATES`] is hand-written, and its whole job is to let this tool say
    what it is *not* covering rather than quietly covering less than the tree
    contains. A hand-written list of what exists has one failure mode, and it
    happened: `veilvoice-check` and `veilvoice-update` were added to the
    workspace and to neither list, so they had no page, no banner, no diagram
    and no entry under "not yet covered" -- invisible rather than uncovered,
    which is the exact failure the list was written to prevent.

    So the list is now checked against the workspace manifest. It stays written
    out, because a generator that discovers its own inputs cannot tell you it is
    missing one; it is simply told, loudly, when the two disagree.

    `fuzz` is a workspace *exclusion* -- it needs nightly and libFuzzer -- so it
    is not a member and is added here by name.
    """
    manifest = read(os.path.join(root, "Cargo.toml"))
    members = []
    inside = False
    for line in manifest.split("\n"):
        stripped = line.strip()
        if stripped.startswith("members"):
            inside = True
            continue
        if inside:
            if stripped.startswith("]"):
                break
            name = stripped.strip(",").strip('"')
            if name.startswith("crates/"):
                members.append(name[len("crates/") :])
    return tuple(sorted(set(members) | {"fuzz"}))


def crates_missing_from_the_lists(root):
    """Crates the workspace has that this file does not name, and vice versa."""
    actual = set(workspace_crates(root))
    listed = set(ALL_CRATES)
    return sorted(actual - listed), sorted(listed - actual)


REPO = "tilas01/veilvoice"
REF = "main"

# Bounds on the derived diagrams. A file with ninety items produces a picture
# nobody can read, which is worse than no picture: it looks like information.
MAX_DIAGRAM_NODES = 22
MAX_LABEL = 30


# --- palette ----------------------------------------------------------------

def palette(root):
    """The Tokyo Night tokens, read from the stylesheet that defines them.

    Hardcoding the hexes here would give the documentation its own copy of the
    site's palette, and a copy is a thing that drifts. `website/css/themes.css`
    is where a colour is decided; this reads the `:root` block so that changing
    a colour there changes it in every banner and every diagram, and so that a
    token being renamed fails loudly here rather than silently rendering black.
    """
    path = os.path.join(root, "website", "css", "themes.css")
    with io.open(path, encoding="utf-8") as handle:
        text = handle.read()
    start = text.index(":root,")
    block = text[start:text.index("}", start)]
    found = dict(re.findall(r"--([a-z0-9-]+)\s*:\s*(#[0-9a-fA-F]{6})", block))
    needed = ("bg", "bg-soft", "bg-inset", "border", "fg", "muted",
              "accent", "accent-2", "cyan", "ok", "warn", "err")
    missing = [name for name in needed if name not in found]
    if missing:
        raise SystemExit(
            "website/css/themes.css no longer defines: %s\n"
            "The documentation palette is read from that file on purpose; "
            "update this list rather than hardcoding a colour."
            % ", ".join(missing)
        )
    return found


# --- reading the tree -------------------------------------------------------

def repo_root():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(here, "..", ".."))


def read(path):
    with io.open(path, encoding="utf-8") as handle:
        return handle.read()


def crate_description(root, crate):
    """The one-line description from the crate's own `Cargo.toml`."""
    text = read(os.path.join(root, crate_dir(crate).replace("/", os.sep), "Cargo.toml"))
    match = re.search(r'^description\s*=\s*"([^"]*)"', text, re.M)
    return match.group(1) if match else ""


# Where a crate keeps Rust, and what each location means.
#
# `src` is the crate itself. The other two are real code that ships in the
# repository and that a reader may well be looking for -- an example is often
# the fastest way to understand an API, and a fuzz target says exactly which
# parsers are considered to read untrusted bytes.
#
# They are documented, and they are kept out of the crate's module graph: an
# example is not something the crate depends on, and drawing it as one would
# make the flowchart say something untrue about the crate's shape.
AREAS = (
    ("src", "src", True),
    ("examples", "example", False),
    ("tests", "test", False),
)

# Where a crate's directory is, and which areas it has.
#
# `fuzz/` is a crate too: its own `Cargo.toml`, its own six Rust files, and
# arguably the most informative one in the repository -- the set of fuzz
# targets is this project's own answer to "which parsers here read bytes
# somebody else produced". It simply does not live under `crates/`, so the
# layout is a lookup rather than an assumption.
LAYOUT = {
    "fuzz": ("fuzz", (("fuzz_targets", "fuzz", False),)),
}

# Crates whose `README.md` is written by hand and must not be generated over.
#
# `fuzz/README.md` explains how to run the targets and records that they have
# **not** been run to convergence -- a sentence `docs/AUDIT.md` cites by name.
# Generating over it lost that, silently, with every check still passing.
HAND_WRITTEN_README = frozenset({"fuzz"})


def crate_dir(crate):
    return LAYOUT.get(crate, ("crates/" + crate, AREAS))[0]


def crate_areas(crate):
    return LAYOUT.get(crate, ("crates/" + crate, AREAS))[1]


def source_files(root, crate):
    """Every `.rs` file in a crate, as (subdirectory, filename, kind, in_graph).

    Sorted within each area, and the areas in a fixed order, so the output is
    identical on every machine -- `os.listdir` guarantees no order at all, and
    a generator whose output depends on filesystem ordering fails `--check` on
    somebody else's computer for no reason they can see.
    """
    out = []
    for subdir, kind, in_graph in crate_areas(crate):
        base = os.path.join(root, crate_dir(crate).replace("/", os.sep), subdir)
        if not os.path.isdir(base):
            continue
        for name in sorted(os.listdir(base)):
            if name.endswith(".rs"):
                out.append((subdir, name, kind, in_graph))
    return out


# --- parsing Rust -----------------------------------------------------------
#
# This is a *syntactic* reader, not a compiler front end, and the difference is
# worth stating because it bounds what the derived diagrams can claim. It knows
# about line comments, block comments, string and character literals, and brace
# depth. It does not resolve types, generics, traits or macros. So a call edge
# means "the name of this function appears, called, inside that function's
# body" -- which is true, useful, and not the same as a type-resolved call
# graph. The pages say so rather than implying more.

ITEM = re.compile(
    r"^(?P<vis>pub(?:\s*\([^)]*\))?\s+)?"
    r"(?:default\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+\"[^\"]*\"\s+)?"
    r"(?P<kind>fn|struct|enum|trait|mod|type|const|static|union)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)"
)
IMPL = re.compile(r"^impl(?:\s*<[^>]*>)?\s+(?P<body>.+?)\s*\{?\s*$")
FN_ANY = re.compile(r"\bfn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)")
USE_CRATE = re.compile(r"\b(?:crate|super)::([a-z_][a-z0-9_]*)")
MOD_DECL = re.compile(r"^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([a-z_][a-z0-9_]*)\s*;")


def strip_code_noise(text):
    """A copy of the source with comments and literals replaced by spaces.

    Offsets are preserved -- spaces in, not deletions -- so anything found in
    the stripped copy is at the same position in the original. Brace counting
    and call detection both run against this, so a `{` inside a string or a
    doc comment cannot throw the structure off. That is not hypothetical in
    this tree: `lock.rs` and `shred.rs` both contain braces inside prose.
    """
    out = []
    index = 0
    length = len(text)
    while index < length:
        char = text[index]
        pair = text[index:index + 2]
        if pair == "//":
            end = text.find("\n", index)
            end = length if end == -1 else end
            out.append(" " * (end - index))
            index = end
        elif pair == "/*":
            depth = 1
            scan = index + 2
            while scan < length and depth:
                if text[scan:scan + 2] == "/*":
                    depth += 1
                    scan += 2
                elif text[scan:scan + 2] == "*/":
                    depth -= 1
                    scan += 2
                else:
                    scan += 1
            out.append("".join(" " if c != "\n" else "\n"
                               for c in text[index:scan]))
            index = scan
        elif char == 'r' and re.match(r'r#*"', text[index:index + 8] or ""):
            hashes = re.match(r'r(#*)"', text[index:]).group(1)
            close = '"' + hashes
            end = text.find(close, index + len(hashes) + 2)
            end = length if end == -1 else end + len(close)
            out.append("".join(" " if c != "\n" else "\n"
                               for c in text[index:end]))
            index = end
        elif char in ('"', "'"):
            # A lifetime (`'a`) is not a character literal. Telling them apart
            # syntactically: a character literal closes within a few bytes.
            quote = char
            scan = index + 1
            closed = False
            while scan < length and scan - index < 12:
                if text[scan] == "\\":
                    scan += 2
                    continue
                if text[scan] == quote:
                    closed = True
                    scan += 1
                    break
                if quote == "'" and text[scan] == "\n":
                    break
                scan += 1
            if quote == '"' and not closed:
                while scan < length:
                    if text[scan] == "\\":
                        scan += 2
                        continue
                    if text[scan] == '"':
                        scan += 1
                        break
                    scan += 1
            if not closed and quote == "'":
                out.append(char)
                index += 1
                continue
            out.append("".join(" " if c != "\n" else "\n"
                               for c in text[index:scan]))
            index = scan
        else:
            out.append(char)
            index += 1
    return "".join(out)


def module_doc(text):
    """The `//!` block at the top of the file, as Markdown lines.

    The `# veilvoice-core` heading many of these files open with is dropped:
    the page supplies its own title, and two would nest wrongly. Everything
    else is kept verbatim, including the fenced examples, because those
    examples are compiled by `cargo test` and are therefore known to work.
    """
    lines = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("//!"):
            lines.append(stripped[3:].lstrip() if stripped[3:4] == " "
                         else stripped[3:])
        elif stripped.startswith("//") or stripped.startswith("#!["):
            continue
        elif stripped == "":
            if lines:
                continue
        else:
            break
    while lines and not lines[0].strip():
        lines.pop(0)
    if lines and re.match(r"^#\s+\S", lines[0]):
        lines.pop(0)
        while lines and not lines[0].strip():
            lines.pop(0)
    while lines and not lines[-1].strip():
        lines.pop()
    return lines


def parse_file(text):
    """Items, doc comments and intra-file call edges.

    Returns a dict with `items` (top-level and `impl` members, in source order)
    and `calls` (pairs of function names where the first's body names the
    second).
    """
    clean = strip_code_noise(text)
    lines = text.splitlines()
    clean_lines = clean.splitlines()

    items = []
    pending = []
    depth = 0
    current_impl = None
    impl_depth = None
    skip_until = None          # depth at which a `mod tests` block closes

    for number, raw in enumerate(lines, start=1):
        cleaned = clean_lines[number - 1] if number - 1 < len(clean_lines) else ""
        stripped = raw.strip()

        opens = cleaned.count("{")
        closes = cleaned.count("}")

        if skip_until is not None:
            depth += opens - closes
            if depth <= skip_until:
                skip_until = None
            continue

        if stripped.startswith("///"):
            pending.append(stripped[3:].lstrip() if stripped[3:4] == " "
                           else stripped[3:])
            depth += opens - closes
            continue
        if stripped.startswith("//") or stripped.startswith("#["):
            depth += opens - closes
            continue
        if not stripped:
            pending = []
            depth += opens - closes
            continue

        # A `#[cfg(test)] mod tests` block is not part of the crate's surface.
        if re.match(r"^\s*mod\s+tests?\s*\{", raw) or re.match(r"^\s*mod\s+tests?$", stripped):
            skip_until = depth
            depth += opens - closes
            pending = []
            continue

        if current_impl is not None and depth <= impl_depth:
            current_impl = None
            impl_depth = None

        impl_match = IMPL.match(stripped)
        if impl_match and depth == 0:
            body = impl_match.group("body")
            body = re.sub(r"\s*\{\s*$", "", body)
            current_impl = body.split(" for ")[-1].strip()
            impl_depth = depth
            pending = []
            depth += opens - closes
            continue

        item = ITEM.match(stripped)
        at_item_level = depth == 0 or (
            current_impl is not None and depth == impl_depth + 1)
        if item and at_item_level:
            kind = item.group("kind")
            name = item.group("name")
            if not (kind == "mod" and stripped.rstrip().endswith(";")):
                items.append({
                    "kind": kind,
                    "name": name,
                    "owner": current_impl,
                    "vis": (item.group("vis") or "").strip(),
                    "public": (item.group("vis") or "").strip() == "pub",
                    "line": number,
                    "doc": pending,
                    "signature": re.sub(r"\s*\{\s*$", "", stripped).rstrip(),
                })
            pending = []
            depth += opens - closes
            continue

        pending = []
        depth += opens - closes

    # --- call edges ---------------------------------------------------------
    # Each function's body is taken from the noise-stripped copy by matching
    # braces from its opening one, then scanned for the names of the other
    # functions this file defines.
    owners = {}
    for item in items:
        if item["kind"] == "fn":
            owners[item["name"]] = item["owner"]
    calls = []
    for item in items:
        if item["kind"] != "fn":
            continue
        body = _function_body(clean, item["name"])
        if body is None:
            continue
        for other in sorted(owners):
            if other == item["name"]:
                continue
            if _calls(body, other, owners[other], item["owner"]):
                calls.append((item["name"], other))

    return {"items": items, "calls": sorted(set(calls))}


def _calls(body, name, callee_owner, caller_owner):
    r"""Does `body` call `name`, as opposed to merely containing the word?

    The first version of this asked whether `\bname\s*\(` matched, and drew an
    edge from `SpectralState::transform` to `SpectralState::new` because the
    body constructs a `Complex::new(..)`. An edge that is not a call is worse
    than a missing one: these diagrams are offered as *derived from the
    source*, so a reader has no reason to doubt one.

    What is inspected is the qualifier immediately before the name:

      * nothing       -- a free function in this file. A call.
      * ``self.``     -- a method on this type. A call.
      * ``Self::``    -- likewise.
      * ``Owner::``   -- the type that actually owns that method. A call.
      * ``fn``        -- a definition, not a call.
      * anything else -- somebody else's function that happens to share a
                         name, which in Rust is most of `new`, `len`, `from`
                         and `default`. Not a call.
    """
    for match in re.finditer(r"\b%s\s*\(" % re.escape(name), body):
        head = body[:match.start()].rstrip()
        if re.search(r"\bfn$", head):
            continue
        if head.endswith("::"):
            qualifier = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*::$", head)
            qualifier = qualifier.group(1) if qualifier else ""
            if qualifier not in ("Self", callee_owner or "", caller_owner or ""):
                continue
        elif head.endswith("."):
            receiver = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*\.$", head)
            if not receiver or receiver.group(1) != "self":
                continue
        return True
    return False


def _function_body(clean, name):
    """The braced body of `fn name`, taken from the noise-stripped source."""
    match = re.search(r"\bfn\s+%s\b" % re.escape(name), clean)
    if not match:
        return None
    start = clean.find("{", match.end())
    if start == -1:
        return None
    depth = 0
    for index in range(start, len(clean)):
        if clean[index] == "{":
            depth += 1
        elif clean[index] == "}":
            depth -= 1
            if depth == 0:
                return clean[start:index + 1]
    return clean[start:]


def module_edges(root, crate, files):
    """Which module uses which, from `crate::` and `super::` paths.

    This is the crate-level flowchart's raw material, and it is derived rather
    than drawn: a module that stops depending on another loses the arrow the
    next time this runs, with no chance for the picture to keep asserting a
    relationship the code gave up.
    """
    src = [name for subdir, name, _, in_graph in files if in_graph and subdir == "src"]
    stems = {name[:-3] for name in src} - {"lib", "main"}
    edges = []
    for name in src:
        stem = name[:-3]
        text = strip_code_noise(
            read(os.path.join(root, crate_dir(crate).replace("/", os.sep), "src", name)))
        for target in sorted(set(USE_CRATE.findall(text))):
            if target in stems and target != stem:
                edges.append((stem, target))
        if stem in ("lib", "main"):
            for target in MOD_DECL.findall(text):
                if target in stems:
                    edges.append((stem, target))
    return sorted(set(edges))


# --- the model --------------------------------------------------------------

RUSTDOC_LINK = re.compile(r"\[`([^`\]]+)`\]\((?!)")
BARE_RUSTDOC = re.compile(r"\[`([^`\]]+)`\](?!\()")


# Rustdoc lays crates out as sibling directories, so a doc comment may link to
# `../veilvoice_core/index.html` or `../../veilvoice_crypto/shred/index.html`.
# Both are correct in rustdoc output and dangling everywhere else.
RUSTDOC_PATH = re.compile(
    r"\[([^\]]+)\]\((?:\.\./)+([a-z][a-z0-9_]*)((?:/[a-z][a-z0-9_]*)*)/index\.html\)")


def rewrite_rustdoc_paths(lines, known, target):
    """Point rustdoc's cross-crate links at this generator's own pages.

    `known` maps a crate name to the set of module stems it contains, so a link
    naming a module that has been deleted or renamed degrades to the crate page
    rather than to a confident link at nothing. `target` is a callback that
    formats one link for whichever of the four renderings is being written --
    the repository README, the per-file page, the website, or the GitHub wiki
    all spell the same destination differently.
    """
    out = []
    in_fence = False
    for line in lines:
        if line.strip().startswith("```"):
            in_fence = not in_fence
            out.append(line)
            continue
        if in_fence:
            out.append(line)
            continue

        def replace(match):
            label = match.group(1)
            crate = match.group(2).replace("_", "-")
            modules = [m for m in match.group(3).split("/") if m]
            if crate not in known:
                return match.group(0)
            stem = modules[-1] if modules else None
            if stem is not None and stem not in known[crate]:
                stem = None
            return target(label, crate, stem)

        out.append(RUSTDOC_PATH.sub(replace, line))
    return out


def link_targets(known):
    """The five spellings of "link to this crate, or to this file in it"."""

    def readme(label, crate, stem):
        if stem:
            return "[%s](../../docs/files/%s/%s.md)" % (label, crate, stem)
        return "[%s](../%s/README.md)" % (label, crate_dir(crate).split("/")[-1])

    def filepage(label, crate, stem):
        if stem:
            return "[%s](../%s/%s.md)" % (label, crate, stem)
        return "[%s](../../../%s/README.md)" % (label, crate_dir(crate))

    def site_crate(label, crate, stem):
        if stem:
            return "[%s](%s/%s.html)" % (label, crate, stem)
        return "[%s](%s.html)" % (label, crate)

    def site_file(label, crate, stem):
        if stem:
            return "[%s](../%s/%s.html)" % (label, crate, stem)
        return "[%s](../%s.html)" % (label, crate)

    def wiki(label, crate, stem):
        if stem:
            return "[[%s|%s]]" % (label, wiki_file_page(crate, stem))
        return "[[%s|%s]]" % (label, wiki_crate_page(crate))

    return {"readme": readme, "filepage": filepage, "site_crate": site_crate,
            "site_file": site_file, "wiki": wiki}


def markdown_doc(lines):
    """Doc-comment lines, with rustdoc's intra-doc links made readable.

    ``[`AccentConfig`]`` is a link rustdoc resolves and Markdown does not, so
    on GitHub it renders as the literal text `[AccentConfig]` -- a link that
    looks broken, in a document whose argument is that everything here can be
    checked. Outside a fence it becomes a plain code span, which is what it
    means. Inside a fence nothing is touched: that is compiled example code.
    """
    out = []
    in_fence = False
    for line in lines:
        if line.strip().startswith("```"):
            in_fence = not in_fence
            out.append(line)
            continue
        out.append(line if in_fence else BARE_RUSTDOC.sub(r"`\1`", line))
    return out


def slug(text):
    """GitHub's heading-anchor rule, which is what GitHub and this site both use.

    Copied in behaviour from `tools/search-index/generate.py`, deliberately:
    the two generators produce links into the same documents, and a table of
    contents whose anchors disagree with the search results' anchors would send
    a reader to the top of the page instead of the section they asked for.
    """
    text = re.sub(r"\s+", " ", text).strip().lower()
    text = re.sub(r"[^\w\- ]+", "", text, flags=re.UNICODE)
    return text.replace(" ", "-")


def doc_headings(lines):
    """The headings inside a doc-comment block, for a table of contents."""
    out = []
    in_fence = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        heading = re.match(r"^(#{1,6})\s+(.*)$", stripped)
        if heading:
            title = re.sub(r"[*`_]", "", heading.group(2)).strip()
            out.append((len(heading.group(1)), title))
    return out


class Anchors(object):
    """Hands out heading anchors for one page, in the order they appear.

    GitHub's rule for a repeated heading is to suffix the second and later
    occurrences (`items`, `items-1`, `items-2`), so that is the rule here --
    the same document is rendered by GitHub from the Markdown and by this
    generator into HTML, and an anchor that differs between the two is a
    contents entry that works in one place and not the other.
    """

    def __init__(self):
        self.seen = {}

    def take(self, title):
        base = slug(title)
        count = self.seen.get(base, 0)
        self.seen[base] = count + 1
        return base if count == 0 else "%s-%d" % (base, count)


def sections_for_page(doc, fixed):
    """Every heading on a page, in order, with its anchor allocated once."""
    anchors = Anchors()
    out = [(level, title, anchors.take(title))
           for level, title in doc_headings(doc)]
    out += [(level, title, anchors.take(title)) for level, title in fixed]
    return out


def toc_markdown(sections):
    """A table of contents as a nested Markdown list.

    `sections` is a list of (level, title) pairs. Levels are normalised so the
    shallowest heading on the page sits at the left margin -- a doc comment
    that happens to start at `##` should not produce a list indented for no
    reason.
    """
    if not sections:
        return []
    base = min(level for level, _, _ in sections)
    out = ["## Contents", ""]
    for level, title, anchor in sections:
        out.append("%s- [%s](#%s)" % ("  " * (level - base), title, anchor))
    out.append("")
    return out


def toc_html(sections):
    """The same table of contents for the website, in the site's `nav.toc`."""
    if not sections:
        return []
    out = ['<nav class="toc" aria-label="Contents">']
    for _, title, anchor in sections:
        out.append('  <a href="#%s">%s</a>' % (anchor, esc(title)))
    out.append('</nav>')
    return out


def first_sentence(lines):
    """The opening sentence of a doc block, for a table cell or a subtitle."""
    prose = []
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("#") or stripped.startswith("```"):
            if prose:
                break
            continue
        if not stripped:
            if prose:
                break
            continue
        prose.append(stripped)
    text = " ".join(prose)
    text = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"\[`?([^\]`]+)`?\]", r"\1", text)
    text = text.replace("**", "").replace("`", "")
    match = re.search(r"^(.+?[.!?])(\s|$)", text)
    if match:
        return match.group(1).strip()
    return text.strip()


def known_modules(root):
    """Every crate, and the module stems inside it.

    Used to decide whether a rustdoc link naming a module still names something
    real. A link that resolves to a page which does not exist is worse than one
    that resolves to the crate: the first is a broken promise, the second is a
    slightly less specific kept one.
    """
    return {crate: {name[:-3] for name in source_files(root, crate)}
            for crate in ALL_CRATES}


# Every crate's `//!` block has to say what the crate is for **in plain words**,
# under this heading, as well as technically.
#
# The two are for different readers and the technical one does not become the
# plain one by being read slowly. A person deciding whether to trust a privacy
# tool should be able to find out what each part of it does without knowing what
# a formant or a KDF is, and the place to put that is beside the code, where it
# is reviewed in the same diff as the thing it describes.
#
# Required rather than encouraged: this generator refuses to write a page for a
# crate that has not got one, which is the only version of "we should document
# that" that survives a busy week. `tools/docs/sources.py` requires the same of
# the website's own files, under the same heading, for the same reason.
PLAIN_HEADING = "In plain words"


def has_plain_words(doc):
    """Whether a `//!` block carries the plain-words section."""
    return any(
        line.strip().lstrip("#").strip().lower() == PLAIN_HEADING.lower() for line in doc
    )


def build(root, crate):
    """Everything the renderers need, gathered once."""
    files = source_files(root, crate)
    entries = []
    for subdir, name, kind, in_graph in files:
        path = os.path.join(root, crate_dir(crate).replace("/", os.sep), subdir, name)
        text = read(path)
        doc = module_doc(text)
        parsed = parse_file(text)
        # Page names have to be unique within a crate, and `src/lib.rs` and a
        # test called `lib.rs` would otherwise collide. Only `src` keeps the
        # bare stem, so existing page addresses do not move.
        stem = name[:-3] if subdir == "src" else "%s-%s" % (subdir, name[:-3])
        entries.append({
            "name": name,
            "stem": stem,
            "kind": kind,
            "in_graph": in_graph,
            "area": subdir,
            "rel": "%s/%s/%s" % (crate_dir(crate), subdir, name),
            "lines": text.count("\n") + (0 if text.endswith("\n") else 1),
            "doc": doc,
            "summary": first_sentence(doc),
            "items": parsed["items"],
            "calls": parsed["calls"],
        })
    return {
        "crate": crate,
        "description": crate_description(root, crate),
        "files": entries,
        "edges": module_edges(root, crate, files),
    }


def crates_without_plain_words(models):
    """Which crates have no plain-words section, in order.

    `fuzz` is exempt for the same reason its README is hand-written: it has no
    library and no `//!` block for this generator to read. Its plain-words
    section lives in `fuzz/README.md`, where the rest of its documentation is.
    """
    missing = []
    for model in models:
        if model["crate"] in HAND_WRITTEN_README:
            continue
        lib = next(
            (entry for entry in model["files"] if entry["stem"] in ("lib", "main")),
            None,
        )
        if lib is None or not has_plain_words(lib["doc"]):
            missing.append(model["crate"])
    return missing


# --- graphs -----------------------------------------------------------------

def crate_graph(model):
    """Nodes and edges for the crate-level flowchart."""
    by_stem = {entry["stem"]: entry for entry in model["files"]
               if entry.get("in_graph")}
    order = []
    for preferred in ("lib", "main"):
        if preferred in by_stem:
            order.append(preferred)
    order += [entry["stem"] for entry in model["files"]
              if entry.get("in_graph") and entry["stem"] not in order]
    nodes = []
    for stem in order:
        entry = by_stem[stem]
        nodes.append({
            "id": stem,
            "label": [entry["name"], "%d lines" % entry["lines"]],
            "url": "https://github.com/%s/blob/%s/%s"
                   % (REPO, REF, entry["rel"]),
            "root": stem in ("lib", "main"),
        })
    return nodes, list(model["edges"])


def file_graph(entry):
    """Nodes and edges for one file's flowchart.

    Functions the file defines, and the calls between them. Types are included
    when they own methods, so a reader can see which functions belong to what.
    Bounded at `MAX_DIAGRAM_NODES`: past that a diagram stops being readable
    and the item table below it is the better answer, so the page says the
    diagram was bounded rather than silently showing a subset.
    """
    functions = [item for item in entry["items"] if item["kind"] == "fn"]
    calls = entry["calls"]
    named = {item["name"] for item in functions}

    # Prefer the functions that participate in the call graph, then public
    # ones, then the rest -- so a bounded diagram keeps the part with structure
    # in it rather than the first N alphabetically.
    involved = {a for a, _ in calls} | {b for _, b in calls}
    def rank(item):
        return (0 if item["name"] in involved else 1,
                0 if item["public"] else 1,
                item["line"])
    chosen = sorted(functions, key=rank)[:MAX_DIAGRAM_NODES]
    chosen_names = {item["name"] for item in chosen}
    chosen.sort(key=lambda item: item["line"])

    # Which functions nothing else in this file calls. Those are the ways in
    # -- what a caller outside the file reaches first -- and they are the most
    # useful thing a reader can be shown, so they are marked rather than left
    # to be inferred from the arrow directions.
    called = {b for _, b in calls}

    nodes = []
    for item in chosen:
        label = item["name"]
        if item["owner"]:
            label = "%s::%s" % (item["owner"], item["name"])
        if len(label) > MAX_LABEL:
            label = label[:MAX_LABEL - 1] + "…"

        if item["public"] and item["name"] not in called:
            role = "entry"      # public, and nothing here calls it: a way in
        elif item["public"]:
            role = "api"        # public, but also used internally
        else:
            role = "helper"     # private to this file

        nodes.append({
            "id": item["name"],
            # The line number is half of "reference the lines"; the URL is the
            # other half. A reader who wants to know what a box actually does
            # should be one click from the code, not one search.
            "label": [label, "line %d" % item["line"]],
            "line": item["line"],
            "url": "https://github.com/%s/blob/%s/%s#L%d"
                   % (REPO, REF, entry["rel"], item["line"]),
            "role": role,
            "root": role == "entry",
        })
    edges = [(a, b) for a, b in calls
             if a in chosen_names and b in chosen_names]
    truncated = len(functions) - len(chosen)
    return nodes, edges, truncated, len(named)


# --- Mermaid ----------------------------------------------------------------

def reachable(start, calls, limit=40):
    """Everything `start` can reach through the call edges, breadth first.

    Bounded, and the bound is not decoration: a call graph derived
    syntactically can contain a cycle -- mutual recursion, or two helpers that
    both reach a third -- and an unbounded walk over one does not terminate.
    """
    seen = []
    queue = [start]
    visited = {start}
    while queue and len(seen) < limit:
        current = queue.pop(0)
        for a, b in calls:
            if a == current and b not in visited:
                visited.add(b)
                seen.append(b)
                queue.append(b)
    return seen


def contains(entry):
    """A structured account of what one file holds, derived from its items.

    Returns (counts, types, ways_in) where `ways_in` pairs each entry point
    with what calling it reaches. Nothing here is written by hand, so nothing
    here can drift from the code it describes.
    """
    items = entry["items"]
    functions = [i for i in items if i["kind"] == "fn"]
    types = [i for i in items if i["kind"] in ("struct", "enum", "trait", "union")]
    constants = [i for i in items if i["kind"] in ("const", "static")]

    calls = entry["calls"]
    called = {b for _, b in calls}
    ways_in = []
    for item in functions:
        if not item["public"] or item["name"] in called:
            continue
        ways_in.append((item, reachable(item["name"], calls)))

    counts = {
        "types": len(types),
        "functions": len(functions),
        "constants": len(constants),
        "public": len([i for i in functions if i["public"]]),
        "lines": entry["lines"],
    }
    return counts, types, ways_in


def contains_markdown(entry):
    """The "what is in here" section, as Markdown."""
    counts, types, ways_in = contains(entry)
    if not (types or ways_in or counts["functions"]):
        return []

    out = ["## What this file contains", ""]
    out.append(
        "%d lines defining **%d function%s** (%d public), **%d type%s** and "
        "**%d constant%s**. Everything below is read out of the source, so it "
        "cannot disagree with the code."
        % (counts["lines"],
           counts["functions"], "" if counts["functions"] == 1 else "s",
           counts["public"],
           counts["types"], "" if counts["types"] == 1 else "s",
           counts["constants"], "" if counts["constants"] == 1 else "s"))
    out.append("")

    if types:
        out.append("**The types it owns.**")
        out.append("")
        for item in types:
            summary = first_sentence(item["doc"]) or ""
            out.append("- `%s %s` (line %d)%s"
                       % (item["kind"], item["name"], item["line"],
                          " -- " + summary if summary else ""))
        out.append("")

    if ways_in:
        out.append("**What happens when it runs.** These are the ways in: "
                   "public, and nothing else in this file calls them, so they "
                   "are what an outside caller reaches first.")
        out.append("")
        for item, reaches in ways_in:
            name = ("`%s::%s`" % (item["owner"], item["name"])
                    if item["owner"] else "`%s`" % item["name"])
            summary = first_sentence(item["doc"]) or ""
            out.append("- %s (line %d)%s"
                       % (name, item["line"], " -- " + summary if summary else ""))
            if reaches:
                out.append("  - reaches: %s"
                           % ", ".join("`%s`" % r for r in reaches[:12]))
        out.append("")
    return out


def legend_markdown(nodes):
    """Say what the colours mean. Colouring without a legend is worse than not."""
    used = {n.get("role") for n in nodes}
    shown = [(role, why) for role, _, why in ROLES if role in used]
    if not shown:
        return []
    out = ["_Colour key: "]
    out.append("; ".join("**%s** -- %s" % (role, why) for role, why in shown))
    out.append("._")
    return ["".join(out), ""]


def mermaid_theme(colours):
    """The init directive, declared once and reused by every diagram."""
    return (
        '%%%%{init: {"theme":"base","themeVariables":{'
        '"background":"%(bg)s",'
        '"primaryColor":"%(bg-soft)s",'
        '"primaryTextColor":"%(fg)s",'
        '"primaryBorderColor":"%(accent)s",'
        '"secondaryColor":"%(bg-inset)s",'
        '"tertiaryColor":"%(bg-inset)s",'
        '"lineColor":"%(muted)s",'
        '"textColor":"%(fg)s",'
        '"mainBkg":"%(bg-soft)s",'
        '"nodeBorder":"%(accent)s",'
        '"clusterBkg":"%(bg-inset)s",'
        '"clusterBorder":"%(border)s",'
        '"fontFamily":"ui-monospace, SFMono-Regular, Consolas, monospace",'
        '"fontSize":"14px"'
        '}}}%%%%'
    ) % colours


def mermaid_id(name):
    """A Mermaid-safe node id. Names here are Rust identifiers, so this is
    conservative rather than clever."""
    return "n_" + re.sub(r"[^A-Za-z0-9_]", "_", name)


# What each role means, and the colour it is drawn in.
#
# Three roles, not seven. A diagram whose legend needs studying has replaced
# one problem with another, and the useful question a reader asks of a source
# file is only ever "where does this start, and what does it reach?".
ROLES = (
    ("entry",  "accent",   "a way in: public, and nothing in this file calls it"),
    ("api",    "cyan",     "public, and also used inside this file"),
    ("helper", "accent-2", "private to this file"),
)


def mermaid(colours, nodes, edges, direction="TD"):
    out = [mermaid_theme(colours), "flowchart %s" % direction]
    for node in nodes:
        label = "<br/>".join(node["label"])
        shape = ('(["%s"])' if node.get("root") else '["%s"]') % label
        out.append("    %s%s" % (mermaid_id(node["id"]), shape))
    for src, dst in edges:
        out.append("    %s --> %s" % (mermaid_id(src), mermaid_id(dst)))

    # One `click` per node: GitHub renders these as real links inside the
    # fence, so a box in the diagram opens the code it stands for.
    for node in nodes:
        if not node.get("url"):
            continue
        out.append('    click %s href "%s" "open the source"'
                   % (mermaid_id(node["id"]), node["url"]))

    # Roles as classes rather than per-node styling: one declaration each,
    # readable in the diff, and the same three colours on every page so a
    # reader learns them once.
    used = {node.get("role") for node in nodes}
    for role, token, _ in ROLES:
        if role not in used:
            continue
        out.append("    classDef %s fill:%s,stroke:%s,color:%s"
                   % (role, colours["bg-soft"], colours[token], colours["fg"]))
        members = [mermaid_id(n["id"]) for n in nodes if n.get("role") == role]
        out.append("    class %s %s" % (",".join(members), role))
    return "\n".join(out)


# --- SVG: shared helpers ----------------------------------------------------

MONO = "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
CHAR_W = 7.6          # width of one monospace character at 13px, near enough
LINE_H = 17.0


def esc(text):
    """XML-escape, and force ASCII.

    Same rule as the search generator's static page and `website/js/*.js`: a
    file a reader may open raw must not depend on the viewer guessing the right
    encoding, because a viewer that guesses CP1252 turns an em dash into
    mojibake. Non-ASCII becomes a numeric reference.
    """
    out = html_mod.escape(text, quote=True)
    return out.encode("ascii", "xmlcharrefreplace").decode("ascii")


def seeded(name):
    """A deterministic byte stream from a name.

    The banners have to be reproducible -- `--check` compares them byte for
    byte -- so nothing here may consult the clock, the filesystem or a global
    random state. SHA-256 of the name, read as needed.
    """
    digest = hashlib.sha256(name.encode("utf-8")).digest()
    while True:
        for byte in digest:
            yield byte
        digest = hashlib.sha256(digest).digest()


# --- SVG: the banners -------------------------------------------------------

BANNER_W = 1200
BANNER_H = 150


def banner_svg(colours, title, subtitle, kind):
    """A banner for a crate or a file.

    The mark is the same soundbar motif as the project's own banner, with the
    bar heights derived from a hash of the title -- so every file gets a
    distinct silhouette, the same one every time, and no two are confusable at
    a glance. `assets/generate.py` draws the real thing in pixels; this is its
    text-shaped sibling, and the two share the palette rather than a copy of it.
    """
    bars = 26
    stream = seeded(title)
    heights = []
    for _ in range(bars):
        low = next(stream)
        high = next(stream)
        # Two bytes averaged: a flatter distribution than one, so a silhouette
        # is a waveform rather than a picket fence.
        heights.append(0.18 + 0.82 * ((low + high) / 510.0))

    out = []
    add = out.append
    add('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %d %d" '
        'width="%d" height="%d" role="img" aria-label="%s">'
        % (BANNER_W, BANNER_H, BANNER_W, BANNER_H, esc(title)))
    # Every generated document says so, in a form the writer can read back.
    # Without it the no-clobber guard cannot tell a banner it wrote from one
    # somebody drew by hand, and refuses to run at all.
    add('<!-- GENERATED by tools/docs/generate.py. Do not edit. -->')
    add('<title>%s</title>' % esc(title))
    add('<rect x="0" y="0" width="%d" height="%d" rx="10" fill="%s"/>'
        % (BANNER_W, BANNER_H, colours["bg"]))
    add('<rect x="0.5" y="0.5" width="%d" height="%d" rx="10" fill="none" '
        'stroke="%s"/>' % (BANNER_W - 1, BANNER_H - 1, colours["border"]))

    # The mark: bars fading from the "clean voice" blue to the "veiled" purple,
    # left to right, which is the same story the project banner tells.
    x = 34.0
    step = 13.0
    width = 7.0
    mid = BANNER_H / 2.0
    for index, height in enumerate(heights):
        half = height * 46.0
        ratio = index / float(bars - 1)
        colour = colours["accent"] if ratio < 0.42 else (
            colours["cyan"] if ratio < 0.58 else colours["accent-2"])
        add('<rect x="%.1f" y="%.1f" width="%.1f" height="%.1f" rx="3" '
            'fill="%s" opacity="%.2f"/>'
            % (x, mid - half, width, half * 2, colour, 0.55 + 0.45 * height))
        x += step

    text_x = 34.0 + bars * step + 24.0
    add('<text x="%.1f" y="%.1f" font-family="%s" font-size="30" '
        'font-weight="700" fill="%s" letter-spacing="0.5">%s</text>'
        % (text_x, mid - 6.0, MONO, colours["fg"], esc(title)))
    add('<text x="%.1f" y="%.1f" font-family="%s" font-size="15" '
        'fill="%s">%s</text>'
        % (text_x, mid + 22.0, MONO, colours["muted"], esc(subtitle)))
    add('<text x="%d" y="30" font-family="%s" font-size="12" fill="%s" '
        'text-anchor="end" letter-spacing="1.5">%s</text>'
        % (BANNER_W - 24, MONO, colours["border"], esc(kind.upper())))
    add('</svg>')
    return "\n".join(out) + "\n"


# --- SVG: the diagrams ------------------------------------------------------

# What a drawing is allowed to be, in the markdown renderings. The layout
# already wraps a rank to fit, so this only ever matches or exceeds the
# canvas -- an `<img width>` narrower than the picture would scale it down
# again, which is the thing being fixed.
DIAGRAM_MAX_W = 640


def rank_nodes(nodes, edges):
    """Longest-path ranking, with cycles broken by ignoring back edges.

    A call graph has cycles (mutual recursion, and more often two helpers that
    both call a third which calls a fourth). A layered drawing needs an acyclic
    graph, so edges that would point backwards are kept in the picture but not
    allowed to influence the ranking. The alternative -- dropping them -- would
    make the diagram assert that a call does not happen.
    """
    ids = [node["id"] for node in nodes]
    index = {name: number for number, name in enumerate(ids)}
    rank = {name: 0 for name in ids}
    forward = [(a, b) for a, b in edges
               if a in index and b in index and index[a] < index[b]]
    for _ in range(len(ids)):
        changed = False
        for src, dst in forward:
            if rank[dst] < rank[src] + 1:
                rank[dst] = rank[src] + 1
                changed = True
        if not changed:
            break
    return rank


def diagram_markdown(source, alt, note, mermaid_source, extra=None):
    """The drawing as an image, the explanation, and the Mermaid source under it.

    A Mermaid fence alone was what the repository and the wiki showed, and it
    left the layout to GitHub: a rank a dozen nodes wide is either wider than
    the column or scaled until the labels stop being readable. The generator
    already draws the same graph for the website, so the repository shows that
    same picture -- one layout, checked in one place -- and keeps the source in
    a `<details>` so GitHub can still render it natively for anyone who wants
    the interactive version.

    The `<img>` carries the drawing's own pixel width rather than `100%`, for
    the reason the SVG does: an image told to fill the column is scaled up on a
    wide screen and down on a narrow one, and only one of those is wanted.
    """
    out = []
    if note:
        out.append(note)
    if extra:
        out.append(extra)
    out.append('<p align="center">')
    out.append('  <img src="%s" alt="%s" width="%d">'
               % (source, alt, DIAGRAM_MAX_W))
    out.append('</p>\n')
    out.append("<details>")
    out.append("<summary>The same graph as Mermaid source</summary>\n")
    out.append("```mermaid\n%s\n```\n" % mermaid_source)
    out.append("</details>\n")
    return out


def diagram_svg(colours, nodes, edges, width=640):
    """Lay the same graph out as SVG, for readers with no JavaScript.

    A simple layered drawing: rank by longest path, order within a rank by
    declaration order, centre each rank. It is not Mermaid's algorithm and does
    not pretend to be -- see this module's docstring. It is deterministic,
    needs nothing at run time, and is legible for the sizes this project
    produces.

    # Why a rank wraps

    Ranking alone put every node of one rank on one line, and a rank is as wide
    as the file is busy: `veilvoice-core/chain.rs` reached **4490 px** of
    canvas. The drawing was then `width="100%"` inside a 630 px column, so the
    browser scaled it to **0.147** and the 13 px labels rendered under two
    pixels tall. Measured, not guessed -- the same page reported
    `scrollWidth` 561 against a `clientWidth` of 390 on a phone, so the wide
    diagrams were also what pushed the reference pages sideways on mobile.

    So a rank is broken into as many lines as it needs to fit `width`, and the
    canvas is only as wide as the widest line actually is. The drawing then
    carries real `width` and `height` attributes and a `max-width: 100%`, which
    means it renders at its own size on a desktop and scales *down* on a narrow
    screen -- never up, and never to a fifth of legible.
    """
    marker = "<!-- GENERATED by tools/docs/generate.py. Do not edit. -->"
    if not nodes:
        return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %d 40" '
                'width="%d" height="40" style="max-width:100%%;height:auto" '
                'role="img" aria-label="no items">%s<text x="10" '
                'y="25" font-family="%s" font-size="13" fill="%s">'
                'nothing to draw</text></svg>\n'
                % (width, width, marker, MONO, colours["muted"]))

    rank = rank_nodes(nodes, edges)
    rows = {}
    for node in nodes:
        rows.setdefault(rank[node["id"]], []).append(node)

    pad_x, pad_y = 14.0, 10.0
    gap_x, gap_y = 26.0, 46.0
    margin = 20.0
    box = {}
    for node in nodes:
        text_w = max(len(line) for line in node["label"]) * CHAR_W
        box[node["id"]] = (text_w + pad_x * 2,
                           len(node["label"]) * LINE_H + pad_y * 2)

    # A rank becomes one or more lines, each narrow enough to fit. Declaration
    # order is preserved across the break, so reading the lines top to bottom
    # reads the rank left to right.
    budget = max(width - margin * 2, max(w for w, _ in box.values()))
    lines = []
    for number in sorted(rows):
        line, used = [], 0.0
        for node in rows[number]:
            w = box[node["id"]][0]
            step = w if not line else gap_x + w
            if line and used + step > budget:
                lines.append(line)
                line, used, step = [], 0.0, w
            line.append(node)
            used += step
        if line:
            lines.append(line)

    line_widths = [sum(box[n["id"]][0] for n in line) + gap_x * (len(line) - 1)
                   for line in lines]

    def lay_out(canvas_w):
        placed, y = {}, margin
        for line, line_w in zip(lines, line_widths):
            height = max(box[n["id"]][1] for n in line)
            x = (canvas_w - line_w) / 2.0
            for node in line:
                w, h = box[node["id"]]
                placed[node["id"]] = (x, y + (height - h) / 2.0, w, h)
                x += w + gap_x
            y += height + gap_y
        return placed, y - gap_y + margin

    canvas_w = max(line_widths) + margin * 2
    placed, canvas_h = lay_out(canvas_w)

    # A back edge is drawn out to the side of both of its endpoints, so it can
    # need room the boxes did not. Widen and lay out again rather than let the
    # arrow leave the canvas -- an SVG does not clip to its viewBox on every
    # engine, and the ones that do clip drew a line that stopped in mid-air.
    def side_of(src, dst):
        sx, _, sw, _ = placed[src]
        dx, _, dw, _ = placed[dst]
        return max(sx + sw, dx + dw) + 22.0

    back = [(a, b) for a, b in edges
            if a in placed and b in placed
            and not placed[b][1] > placed[a][1] + placed[a][3]]
    if back:
        overflow = max(side_of(a, b) for a, b in back) + 6.0 - canvas_w
        if overflow > 0:
            canvas_w += overflow * 2.0
            placed, canvas_h = lay_out(canvas_w)

    out = []
    add = out.append
    add('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %.0f %.0f" '
        'width="%.0f" height="%.0f" style="max-width:100%%;height:auto" '
        'role="img" aria-label="flowchart">' % (canvas_w, canvas_h,
                                                canvas_w, canvas_h))
    add(marker)
    add('<defs><marker id="a" viewBox="0 0 8 8" refX="7" refY="4" '
        'markerWidth="7" markerHeight="7" orient="auto-start-reverse">'
        '<path d="M0 0 L8 4 L0 8 z" fill="var(--muted, %s)"/></marker></defs>'
        % colours["muted"])
    # `var(--token)` rather than a hex: this SVG is inline in the page, so it
    # inherits the custom properties the stylesheet defines, and the diagram
    # follows whichever of the nine themes the reader chose -- or one of their
    # own. The second value in each `var()` is the fallback for a context with
    # no stylesheet, which is what a raw SVG file opened on its own is.
    add('<rect x="0" y="0" width="%.0f" height="%.0f" '
        'fill="var(--bg-inset, %s)" rx="8"/>'
        % (canvas_w, canvas_h, colours["bg-inset"]))

    for src, dst in edges:
        if src not in placed or dst not in placed:
            continue
        sx, sy, sw, sh = placed[src]
        dx, dy, dw, dh = placed[dst]
        x1, y1 = sx + sw / 2.0, sy + sh
        x2, y2 = dx + dw / 2.0, dy
        if dy > sy + sh:
            add('<path d="M%.1f %.1f C %.1f %.1f, %.1f %.1f, %.1f %.1f" '
                'fill="none" stroke="%s" stroke-width="1.2" marker-end="url(#a)"/>'
                % (x1, y1, x1, y1 + 20, x2, y2 - 20, x2, y2 - 3,
                   "var(--muted, %s)" % colours["muted"]))
        else:
            # A back edge -- or an edge between two nodes that wrapping put on
            # the same line -- drawn to the side so it cannot be mistaken for a
            # forward one, and kept rather than dropped.
            side = max(sx + sw, dx + dw) + 22.0
            add('<path d="M%.1f %.1f C %.1f %.1f, %.1f %.1f, %.1f %.1f" '
                'fill="none" stroke="%s" stroke-width="1.1" '
                'stroke-dasharray="4 3" marker-end="url(#a)"/>'
                % (sx + sw, sy + sh / 2.0, side, sy + sh / 2.0,
                   side, dy + dh / 2.0, dx + dw + 3, dy + dh / 2.0,
                   "var(--border, %s)" % colours["border"]))

    role_token = {role: token for role, token, _ in ROLES}
    for node in nodes:
        x, y, w, h = placed[node["id"]]
        # Wrapped in an anchor so the whole box is the target rather than just
        # the words. `_blank` because the reader is in the middle of a diagram
        # and taking the page away to show one function is the wrong trade;
        # `noopener noreferrer` because a new tab should not get a handle back.
        linked = bool(node.get("url"))
        if linked:
            add('<a href="%s" target="_blank" rel="noopener noreferrer">'
                % esc(node["url"]))
        token = role_token.get(node.get("role"), "border")
        stroke = "var(--%s, %s)" % (token, colours.get(token, colours["border"]))
        add('<rect x="%.1f" y="%.1f" width="%.1f" height="%.1f" rx="7" '
            'fill="var(--bg-soft, %s)" stroke="%s" stroke-width="1.5"/>'
            % (x, y, w, h, colours["bg-soft"], stroke))
        for number, line in enumerate(node["label"]):
            token = "fg" if number == 0 else "muted"
            add('<text x="%.1f" y="%.1f" font-family="%s" font-size="13" '
                'fill="var(--%s, %s)" text-anchor="middle">%s</text>'
                % (x + w / 2.0, y + pad_y + LINE_H * (number + 0.75),
                   MONO, token, colours[token], esc(line)))
        if linked:
            add('</a>')
    add('</svg>')
    return "\n".join(out) + "\n"


# --- Markdown ---------------------------------------------------------------

BANNER_NOTE = (
    "<!-- SPDX-License-Identifier: GPL-3.0-or-later -->\n"
    "<!-- GENERATED by tools/docs/generate.py from the doc comments in the\n"
    "     source. Do not edit this file: edit the `//!` and `///` comments in\n"
    "     the .rs files and run the generator again. CI verifies it with\n"
    "         python tools/docs/generate.py --check\n"
    "-->\n"
)


def file_page_path(crate, stem):
    return "docs/files/%s/%s.md" % (crate, stem)


def tidy(text):
    """Collapse runs of blank lines, until it stops changing.

    `str.replace` does not rescan its own output, so one pass over four
    consecutive newlines leaves three. The same lesson as the placeholder
    un-parking in `inline_html`, in a much smaller place.
    """
    while "\n\n\n" in text:
        text = text.replace("\n\n\n", "\n\n")
    return text.rstrip("\n") + "\n"


def markdown_crate(colours, model, links):
    crate = model["crate"]
    nodes, edges = crate_graph(model)
    lib = next((entry for entry in model["files"]
                if entry["stem"] in ("lib", "main")), None)

    out = [BANNER_NOTE]
    out.append('<p align="center">\n'
               '  <img src="../../assets/banners/%s.svg" alt="%s" width="100%%">\n'
               '</p>\n' % (crate, crate))
    out.append("# %s\n" % crate)
    if model["description"]:
        out.append("> %s\n" % model["description"])

    TOC_AT = len(out)
    out.append("")
    if lib and lib["doc"]:
        out.append("\n".join(markdown_doc(rewrite_rustdoc_paths(lib["doc"], links[0], links[1]))) + "\n")

    fixed = [(2, "How the crate fits together"), (2, "The files")]
    if any(item["public"] and item["owner"] is None
           for entry in model["files"] for item in entry["items"]):
        fixed.append((2, "Public items"))
    fixed.append((2, "Reading it elsewhere"))
    sections = sections_for_page(lib["doc"] if lib else [], fixed)
    out.insert(TOC_AT, "\n".join(toc_markdown(sections)))

    out.append("## How the crate fits together\n")
    out.extend(diagram_markdown(
        "../../assets/diagrams/%s.svg" % crate,
        "how %s fits together" % crate,
        "Every arrow below is a `crate::` or `super::` path one module actually\n"
        "uses, read out of the source by the generator rather than drawn by\n"
        "hand. A dependency that goes away loses its arrow the next time this\n"
        "file is written.\n",
        mermaid(colours, nodes, edges)))

    out.append("## The files\n")
    out.append("| File | Lines | What it is |")
    out.append("|---|---:|---|")
    for entry in model["files"]:
        summary = entry["summary"] or "_no module documentation yet_"
        summary = summary.replace("|", "\\|")
        out.append("| [`%s`](../../%s) | %d | %s |"
                   % (entry["name"], file_page_path(crate, entry["stem"]),
                      entry["lines"], summary))
    out.append("")

    public = []
    for entry in model["files"]:
        for item in entry["items"]:
            if item["public"] and item["owner"] is None:
                public.append((entry, item))
    if public:
        out.append("## Public items\n")
        out.append("| Item | Where | What |")
        out.append("|---|---|---|")
        for entry, item in public:
            what = first_sentence(item["doc"]) or ""
            what = what.replace("|", "\\|")
            out.append("| `%s %s` | [`%s`](../../%s) | %s |"
                       % (item["kind"], item["name"], entry["name"],
                          file_page_path(crate, entry["stem"]), what))
        out.append("")

    out.append("## Reading it elsewhere\n")
    out.append(
        "- On the website: [`website/reference/%s.html`](../../website/reference/%s.html)\n"
        "- The audit this code is held to: [`docs/AUDIT.md`](../../docs/AUDIT.md)\n"
        "- The claims and their limits: [`docs/WHITEPAPER.md`](../../docs/WHITEPAPER.md)\n"
        "- Using the crates from your own project: [`docs/USING_THE_CRATES.md`](../../docs/USING_THE_CRATES.md)\n"
        % (crate, crate))
    out.append(
        "Signing key fingerprint `%s`.\n" % FINGERPRINT_PLACEHOLDER)
    return tidy("\n".join(out))


FINGERPRINT_PLACEHOLDER = "@@FINGERPRINT@@"


def markdown_file(colours, model, entry, links):
    crate = model["crate"]
    nodes, edges, truncated, total = file_graph(entry)

    out = [BANNER_NOTE]
    out.append('<p align="center">\n'
               '  <img src="../../../assets/banners/%s/%s.svg" alt="%s" width="100%%">\n'
               '</p>\n' % (crate, entry["stem"], entry["name"]))
    out.append("# `%s`\n" % entry["rel"])
    out.append("[`%s`](../../../%s/README.md) &middot; %d lines &middot; "
               "[read the source](https://github.com/%s/blob/%s/%s)\n"
               % (crate, crate_dir(crate), entry["lines"], REPO, REF, entry["rel"]))

    TOC_AT = len(out)
    out.append("")
    if entry["doc"]:
        out.append("\n".join(markdown_doc(rewrite_rustdoc_paths(entry["doc"], links[0], links[1]))) + "\n")
    else:
        out.append(
            "> This file has no `//!` module documentation yet. That is a gap in\n"
            "> the source rather than in this page: write the comment in\n"
            "> `%s` and it appears here.\n" % entry["rel"])

    fixed = []
    if contains_markdown(entry):
        fixed.append((2, "What this file contains"))
    fixed.append((2, "What calls what"))
    if entry["items"]:
        fixed.append((2, "Items"))
    sections = sections_for_page(entry["doc"], fixed)
    out.insert(TOC_AT, "\n".join(toc_markdown(sections)))

    out.extend(contains_markdown(entry))

    out.append("## What calls what\n")
    if nodes:
        out.append(
            "The functions this file defines, and the calls between them. Both\n"
            "are read out of the source: an edge means the callee's name appears,\n"
            "called, inside the caller's body. It is a syntactic reading, not a\n"
            "type-resolved one, so a call made through a trait object or a macro\n"
            "will not appear.\n")
        if truncated:
            out.append(
                "_%d of %d functions are drawn; the diagram is bounded at %d so it\n"
                "stays readable. The full list is in the table below._\n"
                % (len(nodes), total, MAX_DIAGRAM_NODES))
        out.extend(diagram_markdown(
            "../../../assets/diagrams/%s/%s.svg" % (crate, entry["stem"]),
            "what calls what in %s" % entry["name"],
            None,
            mermaid(colours, nodes, edges),
            extra="\n".join(legend_markdown(nodes))))
    else:
        out.append("This file defines no functions of its own.\n")

    if entry["items"]:
        out.append("## Items\n")
        out.append("| Item | Line | Documentation |")
        out.append("|---|---:|---|")
        for item in entry["items"]:
            name = ("`%s::%s`" % (item["owner"], item["name"])
                    if item["owner"] else "`%s`" % item["name"])
            vis = (item["vis"] + " ") if item["vis"] else ""
            what = first_sentence(item["doc"]) or ""
            what = what.replace("|", "\\|")
            out.append("| %s <sub>%s%s</sub> | [%d](https://github.com/%s/blob/%s/%s#L%d) | %s |"
                       % (name, vis, item["kind"], item["line"], REPO, REF,
                          entry["rel"], item["line"], what))
        out.append("")

    out.append("---\n")
    out.append("Generated from `%s`. On the website: "
               "[`website/reference/%s/%s.html`](../../../website/reference/%s/%s.html).\n"
               % (entry["rel"], crate, entry["stem"], crate, entry["stem"]))
    out.append("Signing key fingerprint `%s`.\n" % FINGERPRINT_PLACEHOLDER)
    return tidy("\n".join(out))


# --- HTML -------------------------------------------------------------------

def html_page(colours, depth, title, description, body, fingerprint):
    """One page of the website mirror.

    `depth` is how far below `website/` the page sits, so the relative links to
    the stylesheet and the icon are correct from `reference/x.html` and from
    `reference/crate/file.html` alike. Getting this wrong produces a page that
    renders unstyled and passes every test that does not look at it -- which is
    finding F-37's shape, so the depth is computed rather than typed.
    """
    up = "../" * depth
    out = []
    add = out.append
    add('<!doctype html>')
    add('<!-- SPDX-License-Identifier: GPL-3.0-or-later -->')
    add('<!-- GENERATED by tools/docs/generate.py. Do not edit. -->')
    add('<html lang="en" data-theme="tokyo-night">')
    add('<head>')
    add('<meta charset="utf-8">')
    add('<meta name="viewport" content="width=device-width, initial-scale=1">')
    add('<title>%s</title>' % esc(title))
    add('<meta name="description" content="%s">' % esc(description))
    add('<link rel="icon" href="%sassets/icon-32.png" type="image/png">' % up)
    add('<link rel="prefetch" href="%sindex.html">' % up)
    add('<link rel="prefetch" href="%swiki.html">' % up)
    add('<link rel="stylesheet" href="%scss/themes.css">' % up)
    add('<link rel="stylesheet" href="%scss/main.css">' % up)
    add('<script src="%sjs/theme.js"></script>' % up)
    add('<script src="%sjs/prefetch.js" defer></script>' % up)
    add('<script src="%sjs/legal.js" defer></script>' % up)
    add('</head>')
    add('<body>')
    add('<header class="top">')
    add('  <div class="wrap">')
    add('    <div class="brand">')
    add('      <img src="%sassets/icon-32.png" alt="">' % up)
    add('      <a href="%sindex.html">VEILVOICE</a>' % up)
    add('    </div>')
    add('    <nav class="links">')
    add('      <a href="%sindex.html">home</a>' % up)
    add('      <a href="%swiki.html">wiki</a>' % up)
    add('      <a href="%sreference/index.html">reference</a>' % up)
    add('      <a href="%ssearch.html">index</a>' % up)
    add('      <a href="https://github.com/%s" rel="noopener noreferrer">github</a>' % REPO)
    add('    </nav>')
    add('    <div class="controls">')
    add('      <label for="theme" style="position:absolute;left:-9999px">Colour scheme</label>')
    add('      <select id="theme" class="theme-pick" title="Colour scheme"></select>')
    add('    </div>')
    add('  </div>')
    add('</header>')
    add('<main class="wrap" style="padding-top:30px">')
    out.extend(body)
    add('</main>')
    add('<footer class="wrap" style="padding:40px 20px;color:var(--muted)">')
    add('<p>Generated from the source by <code>tools/docs/generate.py</code>. '
        'Releases are signed with key <code>%s</code>.</p>' % esc(fingerprint))
    add('</footer>')
    add('</body>')
    add('</html>')
    return "\n".join(out) + "\n"


MD_INLINE_CODE = re.compile(r"`([^`]+)`")
MD_BOLD = re.compile(r"\*\*([^*]+)\*\*")
MD_ITALIC = re.compile(r"(?<![\w*])\*([^*\n]+)\*(?![\w*])")
MD_LINK = re.compile(r"\[([^\]]+)\]\(([^)\s]+)\)")
MD_RUSTDOC_LINK = re.compile(r"\[`([^`\]]+)`\]")


# Schemes a generated link may use, copied from `website/js/markdown.js`.
#
# Same rule, same behaviour: protocol-relative and backslash-prefixed targets
# are refused outright, an explicit scheme must be http or https, and a
# relative path is allowed. Anything else keeps its label and loses its link,
# so the reader still sees the words rather than a silently vanished sentence.
SCHEME = re.compile(r"^([a-zA-Z][a-zA-Z0-9+.-]*):")


def safe_url(url):
    """Is this a link target a generated page may emit?"""
    if url.startswith("//") or url.startswith(chr(92)) or url.startswith("/" + chr(92)):
        return False
    match = SCHEME.match(url)
    if not match:
        return True
    return match.group(1).lower() in ("http", "https")


def inline_html(text):
    """Render the inline Markdown that appears in Rust doc comments.

    Deliberately small. This is not a Markdown engine and must not become one:
    the site already has `website/js/markdown.js`, which has been through two
    rounds of hostile-input auditing, and a second half-implementation would be
    a second attack surface. Everything here escapes first and inserts tags
    afterwards, so no text out of a doc comment can introduce markup.
    """
    out = esc(text)
    parked = []

    def park(html):
        parked.append(html)
        return "\ue000%d\ue001" % (len(parked) - 1)

    # Order matters, and getting it wrong is visible on the page.
    #
    # A rustdoc intra-doc link is `[`name`]` -- brackets around an inline code
    # span. Running the inline-code pass first replaces those backticks with a
    # placeholder, so the rustdoc pattern no longer matches and the brackets
    # survive into the page: readers saw `[hann]` with the brackets showing.
    # Found by looking at a rendered page, not by reading this function.
    out = MD_RUSTDOC_LINK.sub(lambda m: park("<code>%s</code>" % m.group(1)), out)
    out = MD_INLINE_CODE.sub(lambda m: park("<code>%s</code>" % m.group(1)), out)
    out = MD_LINK.sub(
        lambda m: park('<a href="%s">%s</a>' % (m.group(2), m.group(1)))
        if safe_url(m.group(2)) else park(m.group(1)), out)
    out = MD_BOLD.sub(lambda m: park("<strong>%s</strong>" % m.group(1)), out)
    out = MD_ITALIC.sub(lambda m: park("<em>%s</em>" % m.group(1)), out)

    # Un-park until it stops changing: a replacement may itself contain a
    # placeholder, and `str.replace` does not rescan its own output. That is
    # the bug recorded in HANDOFF section 8 as "placeholders must be un-parked
    # recursively", where a link whose label was inline code rendered as an
    # anchor around an invisible character.
    while "\ue000" in out:
        before = out
        for number, html in enumerate(parked):
            out = out.replace("\ue000%d\ue001" % number, html)
        if out == before:
            break
    return out


def doc_html(lines, anchors=None):
    """Render a doc-comment block as HTML: headings, paragraphs, lists, fences."""
    out = []
    paragraph = []
    listing = []
    fence = None

    def flush_paragraph():
        if paragraph:
            out.append("<p>%s</p>" % inline_html(" ".join(paragraph)))
            del paragraph[:]

    def flush_list():
        if listing:
            out.append("<ul>")
            out.extend("<li>%s</li>" % inline_html(item) for item in listing)
            out.append("</ul>")
            del listing[:]

    for line in lines:
        stripped = line.strip()
        if stripped.startswith("```"):
            if fence is None:
                flush_paragraph()
                flush_list()
                fence = []
            else:
                out.append("<pre><code>%s</code></pre>"
                           % esc("\n".join(fence)))
                fence = None
            continue
        if fence is not None:
            # Rustdoc hides setup lines with a leading `#`; they are not part
            # of the example a reader is meant to see.
            if not stripped.startswith("# "):
                fence.append(line)
            continue
        heading = re.match(r"^(#{1,6})\s+(.*)$", stripped)
        if heading:
            flush_paragraph()
            flush_list()
            level = min(len(heading.group(1)) + 1, 6)
            title = re.sub(r"[*`_]", "", heading.group(2)).strip()
            anchor = anchors.pop(0) if anchors else slug(title)
            out.append('<h%d id="%s">%s</h%d>'
                       % (level, anchor, inline_html(heading.group(2)),
                          level))
            continue
        bullet = re.match(r"^[-*]\s+(.*)$", stripped)
        if bullet:
            flush_paragraph()
            listing.append(bullet.group(1))
            continue
        if not stripped:
            flush_paragraph()
            flush_list()
            continue
        if listing:
            listing[-1] += " " + stripped
            continue
        paragraph.append(stripped)
    flush_paragraph()
    flush_list()
    if fence:
        out.append("<pre><code>%s</code></pre>" % esc("\n".join(fence)))
    return out


def diagram_block(colours, nodes, edges, note, mermaid_source):
    """The inline SVG, the explanation, and the Mermaid source beside it."""
    out = ['<div class="diagram">']
    out.append(diagram_svg(colours, nodes, edges).rstrip("\n"))
    out.append('</div>')
    out.append('<p class="muted" style="color:var(--muted);font-size:13px">%s</p>'
               % note)
    out.append('<details><summary>The same graph as Mermaid source</summary>')
    out.append('<pre><code>%s</code></pre>' % esc(mermaid_source))
    out.append('<p style="color:var(--muted);font-size:13px">This site loads no '
               'third-party script, so it cannot run Mermaid; the diagram above '
               'is the same nodes and edges drawn by the generator instead. '
               'GitHub renders the source below directly.</p>')
    out.append('</details>')
    return out


def contains_html(entry, anchor):
    """The same "what is in here" account, for the website."""
    counts, types, ways_in = contains(entry)
    if not (types or ways_in or counts["functions"]):
        return []

    out = ['<section id="s-contains">']
    out.append('<h2 id="%s">WHAT THIS FILE CONTAINS</h2>' % anchor)
    out.append(
        "<p>%d lines defining <strong>%d function%s</strong> (%d public), "
        "<strong>%d type%s</strong> and <strong>%d constant%s</strong>. "
        "Everything below is read out of the source, so it cannot disagree "
        "with the code.</p>"
        % (counts["lines"],
           counts["functions"], "" if counts["functions"] == 1 else "s",
           counts["public"],
           counts["types"], "" if counts["types"] == 1 else "s",
           counts["constants"], "" if counts["constants"] == 1 else "s"))

    if types:
        out.append("<p><strong>The types it owns.</strong></p><ul>")
        for item in types:
            summary = first_sentence(item["doc"]) or ""
            out.append("<li><code>%s %s</code> <span style=\"color:var(--muted)\">"
                       "line %d</span>%s</li>"
                       % (esc(item["kind"]), esc(item["name"]), item["line"],
                          " &mdash; " + inline_html(summary) if summary else ""))
        out.append("</ul>")

    if ways_in:
        out.append("<p><strong>What happens when it runs.</strong> These are the "
                   "ways in: public, and nothing else in this file calls them, so "
                   "they are what an outside caller reaches first.</p><ul>")
        for item, reaches in ways_in:
            name = ("%s::%s" % (item["owner"], item["name"])
                    if item["owner"] else item["name"])
            summary = first_sentence(item["doc"]) or ""
            out.append("<li><code>%s</code> <span style=\"color:var(--muted)\">"
                       "line %d</span>%s"
                       % (esc(name), item["line"],
                          " &mdash; " + inline_html(summary) if summary else ""))
            if reaches:
                out.append("<br><span style=\"color:var(--muted);font-size:12px\">"
                           "reaches %s</span>"
                           % ", ".join("<code>%s</code>" % esc(r)
                                       for r in reaches[:12]))
            out.append("</li>")
        out.append("</ul>")
    out.append("</section>")
    return out


def html_crate(colours, model, fingerprint, links):
    crate = model["crate"]
    nodes, edges = crate_graph(model)
    lib = next((entry for entry in model["files"]
                if entry["stem"] in ("lib", "main")), None)

    body = []
    body.append('<p><img src="../assets/banners/%s.svg" alt="%s" '
                'style="width:100%%;height:auto"></p>' % (crate, esc(crate)))
    body.append("<h1>%s</h1>" % esc(crate))
    if model["description"]:
        body.append('<p class="lede">%s</p>' % esc(model["description"]))
    body.append('<p style="color:var(--muted)"><a href="index.html">reference</a> '
                '&middot; <a href="https://github.com/%s/blob/%s/%s/README.md" '
                'rel="noopener noreferrer">the same page on GitHub</a></p>'
                % (REPO, REF, crate_dir(crate)))

    doc_lines = lib["doc"] if lib else []
    sections = sections_for_page(
        doc_lines, [(2, "How the crate fits together"), (2, "The files")])
    doc_anchors = [a for _, _, a in sections[:len(doc_headings(doc_lines))]]
    fixed_anchors = [a for _, _, a in sections[len(doc_headings(doc_lines)):]]
    body.append('<div class="wiki-layout">')
    body.extend(toc_html(sections))
    body.append('<div>')

    if lib and lib["doc"]:
        body.append('<section id="s-about">')
        body.extend(doc_html(rewrite_rustdoc_paths(lib["doc"], links[0], links[1]), list(doc_anchors)))
        body.append('</section>')

    body.append('<section id="s-structure">')
    body.append('<h2 id="%s">HOW THE CRATE FITS TOGETHER</h2>'
                % fixed_anchors[0])
    body.extend(diagram_block(
        colours, nodes, edges,
        "Every arrow is a <code>crate::</code> or <code>super::</code> path one "
        "module actually uses, read out of the source rather than drawn by hand.",
        mermaid(colours, nodes, edges)))
    body.append('</section>')

    body.append('<section id="s-files">')
    body.append('<h2 id="%s">THE FILES</h2>' % fixed_anchors[1])
    body.append('<table><thead><tr><th>File</th><th>Lines</th>'
                '<th>What it is</th></tr></thead><tbody>')
    for entry in model["files"]:
        body.append('<tr><td><a href="%s/%s.html"><code>%s</code></a></td>'
                    '<td>%d</td><td>%s</td></tr>'
                    % (crate, entry["stem"], esc(entry["name"]),
                       entry["lines"],
                       inline_html(entry["summary"]) if entry["summary"]
                       else '<span style="color:var(--muted)">no module '
                            'documentation yet</span>'))
    body.append('</tbody></table>')
    body.append('</section>')
    body.append('</div></div>')
    return html_page(colours, 1, "%s — VeilVoice reference" % crate,
                     model["description"] or crate, body, fingerprint)


def html_file(colours, model, entry, fingerprint, links):
    crate = model["crate"]
    nodes, edges, truncated, total = file_graph(entry)

    body = []
    body.append('<p><img src="../../assets/banners/%s/%s.svg" alt="%s" '
                'style="width:100%%;height:auto"></p>'
                % (crate, entry["stem"], esc(entry["name"])))
    body.append("<h1><code>%s</code></h1>" % esc(entry["rel"]))
    body.append('<p style="color:var(--muted)"><a href="../%s.html">%s</a> '
                '&middot; %d lines &middot; '
                '<a href="https://github.com/%s/blob/%s/%s" rel="noopener noreferrer">'
                'read the source</a></p>'
                % (crate, esc(crate), entry["lines"], REPO, REF, entry["rel"]))

    fixed = []
    has_contains = bool(contains_html(entry, "x"))
    if has_contains:
        fixed.append((2, "What this file contains"))
    fixed.append((2, "What calls what"))
    if entry["items"]:
        fixed.append((2, "Items"))
    sections = sections_for_page(entry["doc"], fixed)
    split = len(doc_headings(entry["doc"]))
    doc_anchors = [a for _, _, a in sections[:split]]
    fixed_anchors = [a for _, _, a in sections[split:]]
    body.append('<div class="wiki-layout">')
    body.extend(toc_html(sections))
    body.append('<div>')

    if entry["doc"]:
        body.append('<section id="s-about">')
        body.extend(doc_html(rewrite_rustdoc_paths(entry["doc"], links[0], links[1]), list(doc_anchors)))
        body.append('</section>')
    else:
        body.append('<section id="about"><p style="color:var(--muted)">This file '
                    'has no <code>//!</code> module documentation yet. That is a '
                    'gap in the source rather than in this page.</p></section>')

    if has_contains:
        body.extend(contains_html(entry, fixed_anchors[0]))
    calls_anchor = fixed_anchors[1] if has_contains else fixed_anchors[0]
    body.append('<section id="s-calls">')
    body.append('<h2 id="%s">WHAT CALLS WHAT</h2>' % calls_anchor)
    if nodes:
        note = ("The functions this file defines, and the calls between them. An "
                "edge means the callee's name appears, called, inside the caller's "
                "body — a syntactic reading, not a type-resolved one.")
        if truncated:
            note += (" %d of %d functions are drawn; the diagram is bounded at %d "
                     "so it stays readable." % (len(nodes), total, MAX_DIAGRAM_NODES))
        body.extend(diagram_block(colours, nodes, edges, note,
                                  mermaid(colours, nodes, edges)))
    else:
        body.append("<p>This file defines no functions of its own.</p>")
    body.append('</section>')

    if entry["items"]:
        body.append('<section id="s-items">')
        body.append('<h2 id="%s">ITEMS</h2>'
                    % fixed_anchors[2 if has_contains else 1])
        body.append('<table><thead><tr><th>Item</th><th>Line</th>'
                    '<th>Documentation</th></tr></thead><tbody>')
        for item in entry["items"]:
            name = ("%s::%s" % (item["owner"], item["name"])
                    if item["owner"] else item["name"])
            vis = (item["vis"] + " ") if item["vis"] else ""
            what = first_sentence(item["doc"])
            body.append('<tr><td><code>%s</code> <sub>%s%s</sub></td>'
                        '<td><a href="https://github.com/%s/blob/%s/%s#L%d" '
                        'rel="noopener noreferrer">%d</a></td><td>%s</td></tr>'
                        % (esc(name), esc(vis), esc(item["kind"]), REPO, REF,
                           entry["rel"], item["line"], item["line"],
                           inline_html(what) if what else ""))
        body.append('</tbody></table>')
        body.append('</section>')

    body.append('</div></div>')
    return html_page(colours, 2,
                     "%s — VeilVoice reference" % entry["rel"],
                     entry["summary"] or entry["rel"], body, fingerprint)


def html_index(colours, models, fingerprint, covered, uncovered):
    body = []
    body.append("<h1>Reference</h1>")
    body.append('<p class="lede">Every crate and every source file, generated '
                'from the doc comments in the code.</p>')
    body.append('<p>These pages are written by '
                '<code>tools/docs/generate.py</code> from the <code>//!</code> '
                'and <code>///</code> comments in the source, so they cannot '
                'disagree with it. Regenerate with the same command; CI runs '
                'it with <code>--check</code> and fails if the tree and these '
                'pages have parted company.</p>')
    # The website's own source is documented the same way, by a sibling tool:
    # this one reads Rust doc comments and those files are JavaScript and CSS.
    # Linked from here because a reader looking for "how does this work" should
    # not have to know which generator wrote which page.
    body.append('<p><a href="source/index.html">The website&rsquo;s own '
                'source</a> &mdash; every script and stylesheet this site is '
                'made of, explained technically and then in plain words.</p>')

    for model in models:
        crate = model["crate"]
        body.append('<section id="%s">' % crate)
        body.append('<h2><a href="%s.html">%s</a></h2>' % (crate, esc(crate)))
        body.append("<p>%s</p>" % esc(model["description"]))
        body.append("<ul>")
        for entry in model["files"]:
            body.append('<li><a href="%s/%s.html"><code>%s</code></a> '
                        '<span style="color:var(--muted)">%s</span></li>'
                        % (crate, entry["stem"], esc(entry["name"]),
                           inline_html(entry["summary"])))
        body.append("</ul>")
        body.append('</section>')

    if uncovered:
        body.append('<section id="not-yet">')
        body.append("<h2>NOT YET COVERED</h2>")
        body.append('<p>%d of the workspace’s %d crates are documented here. '
                    'The rest are listed rather than left out silently, because '
                    'a page that shows a subset without saying so is the failure '
                    'this project keeps finding in its own work:</p>'
                    % (len(covered), len(covered) + len(uncovered)))
        body.append("<ul>")
        for crate in uncovered:
            body.append("<li><code>%s</code></li>" % esc(crate))
        body.append("</ul>")
        body.append('</section>')

    return html_page(colours, 1, "Reference — VeilVoice",
                     "Every VeilVoice crate and source file, generated from the "
                     "doc comments in the code.", body, fingerprint)


# --- the GitHub wiki --------------------------------------------------------
#
# A GitHub wiki is a separate repository of flat Markdown pages: no
# directories, no relative links into the code repository, and no way to
# reference an image by relative path. So the wiki rendering differs from the
# repository rendering in exactly three mechanical ways, and in nothing else:
#
#   * page names are flattened (`File-veilvoice-core-accent`),
#   * links between pages use the wiki's own `[[Page]]` syntax,
#   * images are absolute `raw.githubusercontent.com` URLs.
#
# `[[Page]]` is used rather than `[text](Page)` deliberately: the second is a
# relative Markdown link, and `tools/site-tests/links.test.js` would correctly
# report every one of them as pointing at a file that does not exist in this
# repository -- because it does not. The wiki syntax is not a Markdown link at
# all, so the checker and the wiki both read it the way it is meant.

RAW = "https://raw.githubusercontent.com/%s/%s/" % (REPO, REF)


def wiki_crate_page(crate):
    return "Crate-%s" % crate


def wiki_file_page(crate, stem):
    return "File-%s-%s" % (crate, stem)


def wiki_crate(colours, model, links):
    """The crate page, rendered for the GitHub wiki."""
    crate = model["crate"]
    nodes, edges = crate_graph(model)
    lib = next((entry for entry in model["files"]
                if entry["stem"] in ("lib", "main")), None)

    out = []
    out.append("<!-- GENERATED by tools/docs/generate.py from the doc comments in the source. Do not edit: edit the .rs file and run the generator. -->")
    out.append("![%s](%sassets/banners/%s.svg)\n" % (crate, RAW, crate))
    out.append("# %s\n" % crate)
    if model["description"]:
        out.append("> %s\n" % model["description"])
    out.append("[[Reference]] &middot; "
               "[the same page in the repository]"
               "(https://github.com/%s/blob/%s/%s/README.md)\n"
               % (REPO, REF, crate_dir(crate)))

    sections = sections_for_page(
        lib["doc"] if lib else [],
        [(2, "How the crate fits together"), (2, "The files")])
    out.append("\n".join(toc_markdown(sections)))

    if lib and lib["doc"]:
        out.append("\n".join(markdown_doc(rewrite_rustdoc_paths(lib["doc"], links[0], links[1]))) + "\n")

    out.append("## How the crate fits together\n")
    out.extend(diagram_markdown(
        "%sassets/diagrams/%s.svg" % (RAW, crate),
        "how %s fits together" % crate, None,
        mermaid(colours, nodes, edges)))

    out.append("## The files\n")
    out.append("| File | Lines | What it is |")
    out.append("|---|---:|---|")
    for entry in model["files"]:
        summary = (entry["summary"] or "_no module documentation yet_").replace("|", "\\|")
        out.append("| [[`%s`|%s]] | %d | %s |"
                   % (entry["name"], wiki_file_page(crate, entry["stem"]),
                      entry["lines"], summary))
    out.append("")
    return tidy("\n".join(out))


def wiki_file(colours, model, entry, links):
    """One source file's page, rendered for the GitHub wiki."""
    crate = model["crate"]
    nodes, edges, truncated, total = file_graph(entry)

    out = []
    out.append("<!-- GENERATED by tools/docs/generate.py from the doc comments in the source. Do not edit: edit the .rs file and run the generator. -->")
    out.append("![%s](%sassets/banners/%s/%s.svg)\n"
               % (entry["name"], RAW, crate, entry["stem"]))
    out.append("# `%s`\n" % entry["rel"])
    out.append("[[%s|%s]] &middot; %d lines &middot; "
               "[read the source](https://github.com/%s/blob/%s/%s)\n"
               % (crate, wiki_crate_page(crate), entry["lines"], REPO, REF,
                  entry["rel"]))

    fixed = [(2, "What calls what")]
    if entry["items"]:
        fixed.append((2, "Items"))
    sections = sections_for_page(entry["doc"], fixed)
    out.append("\n".join(toc_markdown(sections)))

    if entry["doc"]:
        out.append("\n".join(markdown_doc(rewrite_rustdoc_paths(entry["doc"], links[0], links[1]))) + "\n")
    else:
        out.append("> This file has no `//!` module documentation yet.\n")

    out.extend(contains_markdown(entry))

    out.append("## What calls what\n")
    if nodes:
        if truncated:
            out.append("_%d of %d functions are drawn; the diagram is bounded "
                       "at %d so it stays readable._\n"
                       % (len(nodes), total, MAX_DIAGRAM_NODES))
        out.extend(diagram_markdown(
            "%sassets/diagrams/%s/%s.svg" % (RAW, crate, entry["stem"]),
            "what calls what in %s" % entry["name"], None,
            mermaid(colours, nodes, edges),
            extra="\n".join(legend_markdown(nodes))))
    else:
        out.append("This file defines no functions of its own.\n")

    if entry["items"]:
        out.append("## Items\n")
        out.append("| Item | Line | Documentation |")
        out.append("|---|---:|---|")
        for item in entry["items"]:
            name = ("`%s::%s`" % (item["owner"], item["name"])
                    if item["owner"] else "`%s`" % item["name"])
            vis = (item["vis"] + " ") if item["vis"] else ""
            what = (first_sentence(item["doc"]) or "").replace("|", "\\|")
            out.append("| %s <sub>%s%s</sub> | [%d](https://github.com/%s/blob/%s/%s#L%d) | %s |"
                       % (name, vis, item["kind"], item["line"], REPO, REF,
                          entry["rel"], item["line"], what))
        out.append("")
    return tidy("\n".join(out))


def wiki_index(models):
    out = ["# Reference\n"]
    out.append("<!-- GENERATED by tools/docs/generate.py from the doc comments in the source. Do not edit: edit the .rs file and run the generator. -->")
    out.append("Every crate and every source file, generated from the doc "
               "comments in the code by `tools/docs/generate.py`. The same "
               "pages are in the repository and on the website; all three come "
               "out of one generator, so they cannot disagree.\n")
    for model in models:
        crate = model["crate"]
        out.append("## [[%s|%s]]\n" % (crate, wiki_crate_page(crate)))
        out.append("%s\n" % model["description"])
        for entry in model["files"]:
            out.append("- [[`%s`|%s]] &mdash; %s"
                       % (entry["name"], wiki_file_page(crate, entry["stem"]),
                          entry["summary"] or "no module documentation yet"))
        out.append("")
    return tidy("\n".join(out))


# --- writing ----------------------------------------------------------------

def outputs(root):
    """Every file this generator owns, as {relative path: text}.

    Built into memory first so that `--check` and the write path cannot
    disagree about what should exist -- the same structure `assets/generate.py`
    uses, and for the same reason.
    """
    colours = palette(root)
    fingerprint = read(os.path.join(root, "website", "assets", "fingerprint.txt")).strip()
    files = {}

    known = known_modules(root)
    spellings = link_targets(known)
    unlisted, phantom = crates_missing_from_the_lists(root)
    if unlisted or phantom:
        lines = []
        if unlisted:
            lines.append("  the workspace has crates this generator does not name:")
            lines.extend("    %s" % name for name in unlisted)
        if phantom:
            lines.append("  this generator names crates the workspace does not have:")
            lines.extend("    %s" % name for name in phantom)
        raise SystemExit(
            "\n".join(lines)
            + "\n\n  Add them to CRATES and ALL_CRATES in tools/docs/generate.py.\n"
            "  A crate missing from both lists has no page and no entry under\n"
            "  \"not yet covered\" -- it is invisible rather than uncovered, which\n"
            "  is the failure those lists exist to prevent."
        )

    models = [build(root, crate) for crate in CRATES]
    missing = crates_without_plain_words(models)
    if missing:
        raise SystemExit(
            "  these crates have no '%s' section in their //! block:\n    %s\n\n"
            "  Every crate says what it is for technically and then says the same\n"
            "  thing in plain words, for a reader who does not write software. The\n"
            "  plain half cannot be generated from the technical half -- if it\n"
            "  could, it would not be worth having. Write it at the end of the\n"
            "  crate's //! block, under that heading."
            % (PLAIN_HEADING, "\n    ".join(missing))
        )
    covered = list(CRATES)
    uncovered = [crate for crate in ALL_CRATES if crate not in CRATES]

    for model in models:
        crate = model["crate"]

        files["assets/banners/%s.svg" % crate] = banner_svg(
            colours, crate, model["description"], "crate")
        crate_nodes, crate_edges = crate_graph(model)
        files["assets/diagrams/%s.svg" % crate] = diagram_svg(
            colours, crate_nodes, crate_edges)
        if crate not in HAND_WRITTEN_README:
            files["%s/README.md" % crate_dir(crate)] = markdown_crate(
                colours, model, (known, spellings["readme"])
            ).replace(FINGERPRINT_PLACEHOLDER, fingerprint)
        files["website/reference/%s.html" % crate] = html_crate(
            colours, model, fingerprint, (known, spellings["site_crate"]))
        files["wiki/%s.md" % wiki_crate_page(crate)] = wiki_crate(
            colours, model, (known, spellings["wiki"]))

        for entry in model["files"]:
            subtitle = entry["summary"] or ("%s · %d lines"
                                            % (crate, entry["lines"]))
            if len(subtitle) > 96:
                subtitle = subtitle[:95] + "…"
            files["assets/banners/%s/%s.svg" % (crate, entry["stem"])] = banner_svg(
                colours, entry["name"], subtitle, crate.replace("veilvoice-", ""))
            file_nodes, file_edges, _, _ = file_graph(entry)
            files["assets/diagrams/%s/%s.svg" % (crate, entry["stem"])] = diagram_svg(
                colours, file_nodes, file_edges)
            files[file_page_path(crate, entry["stem"])] = markdown_file(
                colours, model, entry, (known, spellings["filepage"])
            ).replace(FINGERPRINT_PLACEHOLDER, fingerprint)
            files["website/reference/%s/%s.html" % (crate, entry["stem"])] = html_file(
                colours, model, entry, fingerprint, (known, spellings["site_file"]))
            files["wiki/%s.md" % wiki_file_page(crate, entry["stem"])] = wiki_file(
                colours, model, entry, (known, spellings["wiki"]))

    files["website/reference/index.html"] = html_index(
        colours, models, fingerprint, covered, uncovered)
    files["wiki/Reference.md"] = wiki_index(models)

    # The website serves only what is under `website/`, so the banners the
    # pages reference have to exist there too. `assets/generate.py` learned
    # this the hard way -- finding F-41 was exactly a generator writing one
    # copy and a second copy drifting -- so both are written here and both are
    # checked.
    for name in list(files):
        if name.startswith("assets/banners/"):
            files["website/" + name] = files[name]

    return files


# The string every generated document carries, and the writer's only way of
# telling its own output from somebody's work.
MARKER = "GENERATED by tools/docs/generate.py"


def is_ours(path):
    """Did this generator write the file already there?

    Only the head is read: the marker is in the first few lines of everything
    this script produces, and a file large enough to matter should not be read
    whole to answer a yes/no question.
    """
    try:
        with io.open(path, encoding="utf-8", errors="replace") as handle:
            return MARKER in handle.read(2048)
    except OSError:
        return False


def write(root, files):
    # Nothing is written until every destination has been checked, so a refusal
    # leaves the tree exactly as it was rather than half-regenerated.
    clobbered = []
    for rel in sorted(files):
        path = os.path.join(root, rel.replace("/", os.sep))
        if os.path.exists(path) and not is_ours(path):
            clobbered.append(rel)
    if clobbered:
        print()
        print("  REFUSING to overwrite %d file(s) this generator did not write:"
              % len(clobbered))
        for rel in clobbered:
            print("    %s" % rel)
        print()
        print("  Each of these exists and does not carry the generator's marker,")
        print("  so it is somebody's work rather than a previous run's output.")
        print("  Either delete it deliberately, or add its crate to")
        print("  HAND_WRITTEN_README at the top of this file.")
        return 1

    for rel, text in sorted(files.items()):
        path = os.path.join(root, rel.replace("/", os.sep))
        directory = os.path.dirname(path)
        if directory and not os.path.isdir(directory):
            os.makedirs(directory)
        with io.open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
    print("  wrote %d files for %d crate(s)" % (len(files), len(CRATES)))
    remaining = [c for c in ALL_CRATES if c not in CRATES]
    if remaining:
        print()
        print("  NOTE: %d of %d crates are covered. Not yet generated:"
              % (len(CRATES), len(ALL_CRATES)))
        for crate in remaining:
            print("    %s" % crate)
        print()
        print("  Add them to CRATES at the top of this file once the format is")
        print("  agreed; everything below that list is generic over it.")
    return 0


def check(root, files):
    problems = []
    for rel, text in sorted(files.items()):
        path = os.path.join(root, rel.replace("/", os.sep))
        try:
            with io.open(path, encoding="utf-8", newline="") as handle:
                actual = handle.read()
        except OSError:
            problems.append("%s: missing" % rel)
            continue
        if actual.replace("\r\n", "\n") != text:
            problems.append("%s: differs from the generator output" % rel)

    # A file this generator used to own and no longer produces would otherwise
    # sit in the tree for ever, describing something that has been deleted.
    #
    # `website/reference/source/` and `wiki/Source-*` are somebody else's:
    # `tools/docs/sources.py` documents the website's own JavaScript and CSS,
    # which this tool cannot read because it reads Rust doc comments. It has its
    # own `--check` with its own orphan sweep over exactly those paths, so they
    # are covered -- by the generator that knows what belongs in them.
    #
    # The wiki is one flat namespace, so the exclusion there is a *prefix* on a
    # file name rather than a directory. That is the price of the wiki's shape,
    # and it is why those pages are named `Source-` and nothing else is.
    not_ours = ("website/reference/source/", "wiki/Source-")
    for base in ("docs/files", "website/reference", "assets/banners",
                 "website/assets/banners", "wiki"):
        directory = os.path.join(root, base.replace("/", os.sep))
        if not os.path.isdir(directory):
            continue
        for current, _, names in os.walk(directory):
            for name in names:
                rel = os.path.relpath(os.path.join(current, name), root)
                rel = rel.replace(os.sep, "/")
                if rel.startswith(not_ours):
                    continue
                if rel not in files:
                    problems.append("%s: not produced by the generator any more" % rel)

    if problems:
        for line in problems[:40]:
            print("  MISMATCH %s" % line)
        if len(problems) > 40:
            print("  ...and %d more" % (len(problems) - 40))
        print()
        print("Run 'python tools/docs/generate.py' and commit the result.")
        return 1
    print("  documentation matches the source (%d files, %d crate(s))"
          % (len(files), len(CRATES)))
    return 0


def main():
    root = repo_root()
    files = outputs(root)
    if "--check" in sys.argv:
        return check(root, files)
    return write(root, files)


if __name__ == "__main__":
    sys.exit(main())
