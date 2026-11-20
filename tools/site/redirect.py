#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build what the old address serves now that the website has moved.

    python tools/site/redirect.py OUT     # write the redirecting site into OUT
    python tools/site/redirect.py --check # confirm website/ still has the files kept

The website moved from tilas01.github.io/veilvoice to
tilas01.github.io/veilvoice-website. The old address is in shipped binaries,
package metadata, bookmarks and other people's links, so it keeps answering:

- Every page the old site had gets a page of its own at the same path, which
  sends the browser to the same page at the new address. The query string and
  the #fragment come along, so a link to one heading lands on that heading.
- Any other address goes through 404.html. If the old site had a file there
  (a picture, the search index), it goes to the same file at the new
  address; if it never existed, it goes to the new home page rather than to
  a second "not found".
- Readers without JavaScript follow a refresh tag to the right page. A refresh
  cannot see the query or the fragment, so for them those are lost, and the
  link on the page still names the exact page.
- Search engines read the refresh as a permanent move and the canonical link
  as the page to list instead.

The pages show no text at all: the move is meant to be seamless, and the
browser is on its way before anything could be read.

A few files are copied as they are rather than redirected, because they are
fetched by tools that never follow an HTML redirect: docs/INSTALL.md tells
people to fetch the signing key with curl from the old address, and the
fingerprint beside it is the evidence that key is the right one.
"""

import html
import json
import os
import shutil
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
WEBSITE = os.path.join(ROOT, "website")

NEW = "https://tilas01.github.io/veilvoice-website"

# Served byte for byte at the old address, for curl and wget.
KEPT = ("assets/veilvoice-signing-key.asc", "assets/fingerprint.txt")

# One script for every page: it reads the path from the address bar, so the
# same text works for a listed page and for 404.html. The path loses the old
# project prefix and keeps the rest; search and hash are passed unchanged.
SCRIPT = (
    "(function () {"
    "var path = location.pathname.replace(/^\\/veilvoice(?=\\/|$)/, '') || '/';"
    f"location.replace('{NEW}' + path + location.search + location.hash);"
    "})();"
)

# 404.html answers for every path that has no page of its own. It carries the
# list of files the old site had, so an address that once worked still reaches
# the same file, and one that never did reaches the home page.
SCRIPT_404 = (
    "(function () {"
    "var known = new Set(%s);"
    "var path = location.pathname.replace(/^\\/veilvoice(?=\\/|$)/, '') || '/';"
    "var rel = path.replace(/^\\//, '');"
    "var found = known.has(rel) || (rel === '' || /\\/$/.test(rel)) && known.has(rel + 'index.html');"
    f"location.replace(found ? '{NEW}' + path + location.search + location.hash : '{NEW}/');"
    "})();"
)

PAGE = """<!doctype html>
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Written by tools/site/redirect.py: the website moved to {new}. -->
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Redirecting to {new_text}</title>
<link rel="canonical" href="{target}">
<script>{script}</script>
<meta http-equiv="refresh" content="0; url={target}">
</head>
<body>
</body>
</html>
"""

ROBOTS = """# The website moved to {new}/.
# Every page here redirects to the same page there, so crawlers are welcome
# to follow them.
User-agent: *
Allow: /
""".format(new=NEW)


def page(target, script=SCRIPT):
    """The redirect page for one destination."""
    return PAGE.format(
        new=NEW,
        new_text=html.escape(NEW.split("://", 1)[1]),
        target=html.escape(target, quote=True),
        script=script,
    )


def pages():
    """Every HTML page the old site served, as paths relative to its root."""
    found = []
    for base, _dirs, files in os.walk(WEBSITE):
        for name in files:
            if name.endswith(".html"):
                rel = os.path.relpath(os.path.join(base, name), WEBSITE)
                found.append(rel.replace(os.sep, "/"))
    return sorted(found)


def files():
    """Every file the old site served, as paths relative to its root."""
    found = []
    for base, _dirs, names in os.walk(WEBSITE):
        for name in names:
            found.append(os.path.relpath(os.path.join(base, name), WEBSITE).replace(os.sep, "/"))
    return sorted(found)


def destination(rel):
    """The new address of one old page. index.html is the folder itself."""
    if rel == "index.html":
        return NEW + "/"
    if rel.endswith("/index.html"):
        return f"{NEW}/{rel[:-len('index.html')]}"
    return f"{NEW}/{rel}"


def catch_all():
    """404.html. Readers without scripts go to the home page, because a
    refresh tag cannot see which address was asked for."""
    known = json.dumps(files(), separators=(",", ":"))
    return page(NEW + "/", SCRIPT_404 % known)


def write(out):
    if os.path.exists(out):
        sys.exit(f"error: {out} already exists; give an empty or new folder")
    listed = pages()
    for rel in listed:
        target = os.path.join(out, *rel.split("/"))
        os.makedirs(os.path.dirname(target), exist_ok=True)
        with open(target, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(catch_all() if rel == "404.html" else page(destination(rel)))
    for rel in KEPT:
        source = os.path.join(WEBSITE, *rel.split("/"))
        target = os.path.join(out, *rel.split("/"))
        os.makedirs(os.path.dirname(target), exist_ok=True)
        shutil.copyfile(source, target)
    with open(os.path.join(out, "robots.txt"), "w", encoding="utf-8", newline="\n") as handle:
        handle.write(ROBOTS)
    if "404.html" not in listed:
        with open(os.path.join(out, "404.html"), "w", encoding="utf-8", newline="\n") as handle:
            handle.write(catch_all())
    print(f"{len(listed)} pages redirect to {NEW}, {len(KEPT)} files kept as they are")


def check():
    missing = [rel for rel in KEPT if not os.path.isfile(os.path.join(WEBSITE, *rel.split("/")))]
    if missing:
        sys.exit("error: the old address must keep serving " + ", ".join(missing))
    print(f"the old address can redirect {len(pages())} pages and keep {len(KEPT)} files")


def main():
    if sys.argv[1:] == ["--check"]:
        check()
    elif len(sys.argv) == 2 and not sys.argv[1].startswith("-"):
        write(sys.argv[1])
    else:
        sys.exit(__doc__.split("\n\n", 2)[1])


if __name__ == "__main__":
    main()
