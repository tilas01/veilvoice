#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Screenshot a page of the site with headless Edge, over the DevTools protocol.

    python -m http.server 8787 --bind 127.0.0.1 --directory website
    python tools/render/shot.py reference/veilvoice-core.html out.png

    --width N          viewport width (default 1280)
    --height N         viewport height (default 900)
    --full             capture the whole page, not just the viewport
    --no-js            render with script execution disabled
    --reduced-motion   render as though the reader asked for less motion
    --port N           DevTools port (default 9223)
    --server URL       page origin (default http://127.0.0.1:8787)

# Why this exists at all

`HANDOFF.md` section 8 has said since v0.1.7 that the page must be *looked at*,
and it has been right twice: three paragraphs of the walkthrough were invisible
on the published site with every unit test passing (F-30's neighbourhood), and
finding **F-37** had the banner rendering its own text illegibly on every
viewport for as long as the banner had existed. Neither was findable from the
tests. Both were obvious on sight.

# Why not `--screenshot`

Edge's own `--headless=new --screenshot=FILE` exits 0 and writes nothing in this
environment, and the editor's browser pane has never composited here either --
two sessions lost time to that before it was written down. The DevTools protocol
works, so this drives it directly.

# Why a WebSocket client is in here

The protocol is JSON over a WebSocket, and the standard library has no
WebSocket client. Adding a dependency for this would put a package on the
critical path of a repository whose argument is that it has no supply chain
worth attacking, to take a picture. The framing is about sixty lines, so it is
sixty lines below.

Pure standard library. No build step, no dependencies.
"""

import argparse
import base64
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.request

# Where to find a browser that speaks the DevTools protocol.
#
# Edge first, because that is what this was written against and what the
# committed screenshots were taken with. The rest are here because the tool was
# Windows-only and the rule it exists to serve -- look at the page -- is not:
# a session on Linux could run every test in this repository and could not open
# a single page of the site it had just rebuilt.
#
# `VEILVOICE_BROWSER` overrides the lot, for a browser installed somewhere
# these lists do not name. Any Chromium will do; the protocol is the same.
BROWSER_CANDIDATES = (
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/opt/pw-browsers/chromium",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/usr/bin/google-chrome",
    "/usr/bin/microsoft-edge",
    "/snap/bin/chromium",
)


def find_browser():
    """The first browser on this machine that can be driven, or nothing."""
    override = os.environ.get("VEILVOICE_BROWSER", "").strip()
    if override:
        return override if os.path.exists(override) else None
    return next((path for path in BROWSER_CANDIDATES if os.path.exists(path)), None)

# The legal gate covers the page until it is accepted, so every screenshot would
# otherwise be a picture of the same modal. Setting the key the gate looks for
# *before the document runs* is the supported way past it -- and it is set per
# session, so this changes nothing for a real reader.
ACCEPT = 'sessionStorage["veilvoice-accepted-v1"] = "yes";'


# --- the smallest WebSocket client that can carry this -----------------------

class Socket(object):
    """A WebSocket text channel. Client to server only ever sends text."""

    def __init__(self, url):
        rest = url.split("://", 1)[1]
        hostport, _, path = rest.partition("/")
        host, _, port = hostport.partition(":")
        self.sock = socket.create_connection((host, int(port or 80)), timeout=30)
        key = base64.b64encode(os.urandom(16)).decode("ascii")
        self.sock.sendall((
            "GET /%s HTTP/1.1\r\n"
            "Host: %s\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            "Sec-WebSocket-Key: %s\r\n"
            "Sec-WebSocket-Version: 13\r\n\r\n" % (path, hostport, key)
        ).encode("ascii"))
        self.buffer = b""
        while b"\r\n\r\n" not in self.buffer:
            chunk = self.sock.recv(4096)
            if not chunk:
                raise SystemExit("the browser closed the connection during the handshake")
            self.buffer += chunk
        head, _, rest = self.buffer.partition(b"\r\n\r\n")
        if b"101" not in head.split(b"\r\n")[0]:
            raise SystemExit("upgrade refused: %s" % head.split(b"\r\n")[0])
        self.buffer = rest

    def _read(self, count):
        while len(self.buffer) < count:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise SystemExit("the browser closed the connection")
            self.buffer += chunk
        out, self.buffer = self.buffer[:count], self.buffer[count:]
        return out

    def send(self, text):
        payload = text.encode("utf-8")
        header = bytearray([0x81])            # FIN + text frame
        length = len(payload)
        if length < 126:
            header.append(0x80 | length)      # the mask bit is mandatory client-side
        elif length < 65536:
            header.append(0x80 | 126)
            header += struct.pack(">H", length)
        else:
            header.append(0x80 | 127)
            header += struct.pack(">Q", length)
        mask = os.urandom(4)
        header += mask
        masked = bytes(byte ^ mask[index % 4] for index, byte in enumerate(payload))
        self.sock.sendall(bytes(header) + masked)

    def receive(self):
        """One complete message, reassembled across continuation frames."""
        chunks = []
        while True:
            first, second = self._read(2)
            final = first & 0x80
            length = second & 0x7F
            if length == 126:
                length = struct.unpack(">H", self._read(2))[0]
            elif length == 127:
                length = struct.unpack(">Q", self._read(8))[0]
            # Server-to-client frames are never masked, so there is no key here.
            chunks.append(self._read(length))
            if final:
                return b"".join(chunks).decode("utf-8", "replace")


class Browser(object):
    def __init__(self, port, width, height):
        browser = find_browser()
        if browser is None:
            raise SystemExit(
                "no browser found. Looked in:\n  "
                + "\n  ".join(BROWSER_CANDIDATES)
                + "\n\nSet VEILVOICE_BROWSER to the one on this machine.")
        profile = os.path.join(tempfile.gettempdir(), "vv-render-%d" % port)
        command = [browser, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                   "--no-first-run", "--no-default-browser-check",
                   "--remote-debugging-port=%d" % port,
                   "--user-data-dir=%s" % profile,
                   "--window-size=%d,%d" % (width, height)]
        # Chromium refuses to run as root without this, and a container is
        # very often root. It changes nothing about what is rendered.
        if hasattr(os, "geteuid") and os.geteuid() == 0:
            command.append("--no-sandbox")
        command.append("about:blank")
        self.process = subprocess.Popen(
            command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.socket = Socket(self._wait_for_target(port))
        self.next_id = 0

    @staticmethod
    def _wait_for_target(port, seconds=25):
        """Poll until DevTools answers. It is not listening the instant it starts."""
        deadline = time.time() + seconds
        last = None
        while time.time() < deadline:
            try:
                raw = urllib.request.urlopen(
                    "http://127.0.0.1:%d/json/list" % port, timeout=2).read()
                for target in json.loads(raw.decode("utf-8")):
                    if target.get("type") == "page" and target.get("webSocketDebuggerUrl"):
                        return target["webSocketDebuggerUrl"]
            except Exception as exc:          # not listening yet, or no page yet
                last = exc
            time.sleep(0.25)
        raise SystemExit("DevTools never came up on port %d (%s)" % (port, last))

    def call(self, method, **params):
        self.next_id += 1
        wanted = self.next_id
        self.socket.send(json.dumps({"id": wanted, "method": method,
                                     "params": params}))
        while True:
            message = json.loads(self.socket.receive())
            if message.get("id") == wanted:
                if "error" in message:
                    raise SystemExit("%s failed: %s" % (method, message["error"]))
                return message.get("result", {})

    def await_event(self, name, seconds=20):
        deadline = time.time() + seconds
        while time.time() < deadline:
            self.socket.sock.settimeout(max(0.5, deadline - time.time()))
            try:
                message = json.loads(self.socket.receive())
            except socket.timeout:
                break
            if message.get("method") == name:
                return True
        return False

    def close(self):
        try:
            self.socket.sock.close()
        finally:
            self.process.terminate()


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("page")
    parser.add_argument("output")
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=900)
    parser.add_argument("--full", action="store_true")
    parser.add_argument("--no-js", action="store_true")
    parser.add_argument("--reduced-motion", action="store_true")
    parser.add_argument("--port", type=int, default=9223)
    parser.add_argument("--server", default="http://127.0.0.1:8787")
    args = parser.parse_args()

    browser = Browser(args.port, args.width, args.height)
    try:
        browser.call("Page.enable")
        browser.call("Runtime.enable")
        # The disk cache is the reason this is here.
        #
        # The profile directory is reused between runs, so Edge keeps a
        # cached copy of `main.css` and serves it back on the next one. A
        # stylesheet fix measured as applied by one tool photographed as
        # *not* applied by this one, and the two disagreed for a full round
        # of debugging before the cache was the answer. A renderer whose
        # picture can be one edit out of date is worse than no renderer.
        browser.call("Network.enable")
        browser.call("Network.setCacheDisabled", cacheDisabled=True)
        browser.call("Emulation.setDeviceMetricsOverride",
                     width=args.width, height=args.height,
                     deviceScaleFactor=1, mobile=False)
        browser.call("Page.addScriptToEvaluateOnNewDocument", source=ACCEPT)
        if args.reduced_motion:
            browser.call("Emulation.setEmulatedMedia", features=[
                {"name": "prefers-reduced-motion", "value": "reduce"}])
        # Ordered deliberately: disabling scripts must happen before the
        # navigation, or the page has already run them. This switch is what
        # caught the JavaScript toggle claiming "on" with scripts disabled.
        if args.no_js:
            browser.call("Emulation.setScriptExecutionDisabled", value=True)

        url = "%s/%s" % (args.server.rstrip("/"), args.page.lstrip("/"))
        browser.call("Page.navigate", url=url)
        browser.await_event("Page.loadEventFired")
        time.sleep(1.0)

        shot = browser.call("Page.captureScreenshot", format="png",
                            captureBeyondViewport=bool(args.full))
        with open(args.output, "wb") as handle:
            handle.write(base64.b64decode(shot["data"]))
        print("wrote %s (%d bytes) from %s"
              % (args.output, os.path.getsize(args.output), url))
    finally:
        browser.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
