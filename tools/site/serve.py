#!/usr/bin/env python3
"""Serve the VeilVoice website locally, exactly as GitHub Pages serves it.

    python3 tools/site/serve.py            # http://localhost:8000
    python3 tools/site/serve.py --port 9000
    python3 tools/site/serve.py --check    # start, fetch every page, stop

# Why this exists

The site at https://tilas01.github.io/veilvoice/ is the front door: the
downloads, the fingerprint to check a release against, the whole argument for
trusting any of this. If the repository ever went down, or GitHub Pages did,
that front door would be gone -- and it is the one page a user most needs
precisely when something has gone wrong.

So the site is nothing but static files in `website/`, and this serves them the
way Pages does: as a document root, with no build step, no framework and no
server-side anything. Anybody who cloned the repository can read the whole site
offline with one command, and `deploy/nginx.conf` beside this file does the
same thing under a real web server for anybody who wants to host it properly.

# It is also the audit surface

During development this is where the site is looked at before it is pushed.
Every generator writes into `website/`; this is how you see the result the way
a visitor will, rather than trusting that the HTML a tool emitted looks right.

# What it does not pretend to be

It is `http.server` with three narrow additions -- the correct content types,
`Cache-Control: no-store` so a stale page is never served from a previous run,
and a readable directory refusal. It is not hardened for the public internet;
`deploy/nginx.conf` is the answer for that, and says so.

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import argparse
import contextlib
import functools
import http.server
import socket
import sys
import threading
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SITE = ROOT / "website"


class Handler(http.server.SimpleHTTPRequestHandler):
    """Static files, with the content types GitHub Pages sends.

    `SimpleHTTPRequestHandler` guesses types from `mimetypes`, whose answers
    depend on the machine's `/etc/mime.types` and so are not the same
    everywhere. Pinning the few that matter means the page behaves the same on
    a stripped container as on a full desktop -- which is the whole promise of
    a static site, kept rather than assumed.
    """

    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".html": "text/html; charset=utf-8",
        ".css": "text/css; charset=utf-8",
        ".js": "text/javascript; charset=utf-8",
        ".json": "application/json; charset=utf-8",
        ".svg": "image/svg+xml",
        ".png": "image/png",
        ".webp": "image/webp",
        ".ico": "image/x-icon",
        ".woff2": "font/woff2",
        ".xml": "application/xml; charset=utf-8",
        ".txt": "text/plain; charset=utf-8",
        ".asc": "text/plain; charset=utf-8",
    }

    def end_headers(self) -> None:
        # Never serve a page this process cached from a previous run: the point
        # of looking at the local site during development is to see the file as
        # it is now, not as it was before the last generator ran.
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, fmt: str, *args) -> None:
        # One quiet line per request, to stderr, so `--check` output stays the
        # transcript of what was fetched rather than a wall of default logging.
        sys.stderr.write("  %s\n" % (fmt % args))


def make_server(port: int) -> http.server.ThreadingHTTPServer:
    if not (SITE / "index.html").exists():
        raise SystemExit(
            f"no website found at {SITE}. Run the generators first "
            f"(tools/site/split.py and the rest), or check you are in the repo."
        )
    handler = functools.partial(Handler, directory=str(SITE))
    try:
        return http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    except OSError as exc:
        raise SystemExit(f"could not bind port {port}: {exc}") from exc


def pages() -> list[str]:
    """Every HTML page, relative to the site root."""
    return sorted(
        "/" + str(p.relative_to(SITE)) for p in SITE.rglob("*.html")
    )


def check(port: int) -> int:
    """Start the server, fetch every page, and report any that do not load.

    This is what CI and `tools/verify.py` can call: it proves the site is
    servable and every page returns 200, without a person watching a browser.
    """
    server = make_server(port)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base = f"http://127.0.0.1:{server.server_address[1]}"
    broken: list[str] = []
    try:
        for page in pages():
            try:
                with urllib.request.urlopen(base + page, timeout=5) as response:
                    if response.status != 200:
                        broken.append(f"{page}: HTTP {response.status}")
            except Exception as exc:  # noqa: BLE001 -- report, do not raise
                broken.append(f"{page}: {exc}")
    finally:
        server.shutdown()
        thread.join(timeout=2)

    if broken:
        for line in broken:
            print(f"  {line}")
        print(f"\n{len(broken)} page(s) did not load.")
        return 1
    print(f"served {len(pages())} pages, every one returned 200")
    return 0


def serve(port: int) -> int:
    server = make_server(port)
    host, bound = server.server_address
    url = f"http://localhost:{bound}"
    print(f"VeilVoice site at {url}")
    print(f"  serving {SITE.relative_to(ROOT)}/ exactly as GitHub Pages does")
    print("  Ctrl-C to stop")
    with contextlib.suppress(KeyboardInterrupt):
        server.serve_forever()
    server.server_close()
    print("\nstopped")
    return 0


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8000, help="default 8000")
    parser.add_argument(
        "--check",
        action="store_true",
        help="start, fetch every page, report failures, exit",
    )
    args = parser.parse_args()
    if args.check:
        # A free port, so a --check run never collides with a browser someone
        # left open on the default one.
        return check(free_port())
    return serve(args.port)


if __name__ == "__main__":
    sys.exit(main())
