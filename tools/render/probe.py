#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Measure the rendered page, rather than reason about it.

    python -m http.server 8787 --bind 127.0.0.1 --directory website
    python tools/render/probe.py overflow --width 390
    python tools/render/probe.py overflow --width 390 --page index.html
    python tools/render/probe.py eval --page index.html --js "innerWidth"

Two commands:

  **overflow** -- for each page, the document's `clientWidth` against its
  `scrollWidth`, and when they differ, the elements whose boxes reach past the
  right edge and the widest thing inside each. A page that scrolls sideways on
  a phone is a defect that no unit test can see and that every reader can.

  **eval** -- run an expression in the page and print what it returns as JSON.
  For the one-off question: what is this element's computed width, did that
  media query match, what scale did the browser choose for that drawing.

# Why this exists

`tools/render/shot.py` takes the picture. A picture answers "does this look
right", and there is a second class of question -- *how many pixels wide is it
actually* -- where a picture is the worst possible instrument, because reading
a number off a screenshot is exactly the eyeballing this project has been
caught out by before.

Three claims in this repository were falsified by measurement after being
argued from first principles, and each cost a rewrite. The flowcharts are the
clearest: they were `width="100%"` inside a column, which sounds like the
responsible thing to write, and it meant a 4490 px drawing rendered at a scale
of **0.147** with labels under two pixels tall. Nothing about that is visible
in the source, and it took one number to settle.

So: a measurement, printed, before the argument.

Pure standard library. The DevTools plumbing is `shot.py`'s -- see its header
for why there is a hand-rolled WebSocket client in this repository.
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import shot  # noqa: E402  (the path has to be set first)


# The pages worth checking by default: one of every shape the site has, rather
# than all 300-odd reference pages, which are three templates repeated.
DEFAULT_PAGES = (
    "index.html",
    "what.html",
    "guide.html",
    "download.html",
    "verify.html",
    "crypto.html",
    "search.html",
    "wiki.html",
    "reference/index.html",
    "reference/veilvoice-core.html",
    "reference/veilvoice-core/chain.html",
    "nojs/index.html",
)

# Viewport widths that stand for something real. 320 is the narrowest phone
# still in use (an iPhone SE in its first generation); 390 is the modern
# baseline; 768 is a tablet held upright, which is where a two-column layout
# usually collapses badly rather than not at all.
DEFAULT_WIDTHS = (320, 390, 768)

OVERFLOW_JS = r"""
(() => {
  const view = document.documentElement.clientWidth;
  const out = {
    clientWidth: view,
    scrollWidth: document.documentElement.scrollWidth,
    offenders: []
  };
  if (out.scrollWidth <= view + 1) return out;

  // Two things have to be filtered out before the list means anything.
  //
  // The first is a scroll container's contents. The navigation row on a phone
  // is deliberately a sideways scroller, so its links really do sit past the
  // right edge and are not a fault -- reporting them buries the real offender
  // under nine false ones, which is how the first run of this looked.
  //
  // The second is inheritance: an element that is only wide because its parent
  // is wide is not the fault either. The fault is the innermost one.
  const scrolls = (el) => {
    const x = getComputedStyle(el).overflowX;
    return x === "auto" || x === "scroll" || x === "hidden";
  };
  const inScroller = (el) => {
    for (let p = el.parentElement; p && p !== document.body; p = p.parentElement) {
      if (scrolls(p)) return true;
    }
    return false;
  };

  const past = [];
  document.querySelectorAll("body *").forEach(el => {
    const r = el.getBoundingClientRect();
    if (r.width > 0 && r.right > view + 1 && !inScroller(el)) past.push([el, r]);
  });
  const set = new Set(past.map(p => p[0]));
  for (const [el, r] of past) {
    let innerBlamed = false;
    for (const child of el.children) if (set.has(child)) innerBlamed = true;
    if (innerBlamed) continue;
    const cs = getComputedStyle(el);
    out.offenders.push({
      tag: el.tagName.toLowerCase(),
      cls: (el.getAttribute("class") || "").slice(0, 48),
      id: el.id || null,
      width: Math.round(r.width),
      right: Math.round(r.right),
      overflowX: cs.overflowX,
      whiteSpace: cs.whiteSpace,
      text: (el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 60)
    });
  }
  out.offenders.sort((a, b) => b.right - a.right);
  out.offenders = out.offenders.slice(0, 12);
  return out;
})()
"""


def open_browser(port, width, height, reduced_motion=False, no_js=False):
    browser = shot.Browser(port, width, height)
    browser.call("Page.enable")
    browser.call("Runtime.enable")
    # See the same call in `shot.py`: the profile directory is reused
    # between runs, so without this a measurement can be taken against a
    # cached stylesheet and report a fix that is not on the page.
    browser.call("Network.enable")
    browser.call("Network.setCacheDisabled", cacheDisabled=True)
    browser.call("Emulation.setDeviceMetricsOverride",
                 width=width, height=height, deviceScaleFactor=1, mobile=False)
    browser.call("Page.addScriptToEvaluateOnNewDocument", source=shot.ACCEPT)
    if reduced_motion:
        browser.call("Emulation.setEmulatedMedia", features=[
            {"name": "prefers-reduced-motion", "value": "reduce"}])
    if no_js:
        browser.call("Emulation.setScriptExecutionDisabled", value=True)
    return browser


def measure(browser, server, page, expression, width, height):
    browser.call("Emulation.setDeviceMetricsOverride",
                 width=width, height=height, deviceScaleFactor=1, mobile=False)
    browser.call("Page.navigate",
                 url="%s/%s" % (server.rstrip("/"), page.lstrip("/")))
    browser.await_event("Page.loadEventFired")
    # The reveal-on-scroll work and the theme picker both settle a frame or two
    # after load, and measuring mid-transition reports a width nothing ever has.
    time.sleep(0.6)
    result = browser.call("Runtime.evaluate", expression=expression,
                          returnByValue=True, awaitPromise=False)
    if "exceptionDetails" in result:
        raise SystemExit("the expression threw: %s"
                         % json.dumps(result["exceptionDetails"])[:400])
    return result.get("result", {}).get("value")


def command_overflow(args):
    pages = args.page or list(DEFAULT_PAGES)
    widths = args.width or list(DEFAULT_WIDTHS)
    browser = open_browser(args.port, widths[0], args.height,
                           args.reduced_motion, args.no_js)
    bad = 0
    try:
        for width in widths:
            print("\n%d px" % width)
            for page in pages:
                found = measure(browser, args.server, page, OVERFLOW_JS,
                                width, args.height)
                over = found["scrollWidth"] - found["clientWidth"]
                if over <= 1:
                    print("  ok    %-38s %d" % (page, found["clientWidth"]))
                    continue
                bad += 1
                print("  WIDE  %-38s %d wide, scrolls to %d (+%d)"
                      % (page, found["clientWidth"], found["scrollWidth"], over))
                for one in found["offenders"]:
                    print("          <%s%s%s> %dpx, right edge %d "
                          "[overflow-x:%s white-space:%s] %s"
                          % (one["tag"],
                             (" class=%s" % one["cls"]) if one["cls"] else "",
                             (" id=%s" % one["id"]) if one["id"] else "",
                             one["width"], one["right"], one["overflowX"],
                             one["whiteSpace"], one["text"]))
    finally:
        browser.close()
    print("\n%s" % ("no page scrolls sideways" if bad == 0
                    else "%d page/width combinations scroll sideways" % bad))
    return 1 if bad else 0


def command_eval(args):
    pages = args.page or ["index.html"]
    widths = args.width or [1280]
    browser = open_browser(args.port, widths[0], args.height,
                           args.reduced_motion, args.no_js)
    try:
        for width in widths:
            for page in pages:
                value = measure(browser, args.server, page, args.js,
                                width, args.height)
                print("%s @ %dpx" % (page, width))
                print(json.dumps(value, indent=1, sort_keys=True))
    finally:
        browser.close()
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("command", choices=("overflow", "eval"))
    parser.add_argument("--page", action="append",
                        help="page under the server root; repeatable")
    parser.add_argument("--width", action="append", type=int,
                        help="viewport width; repeatable")
    parser.add_argument("--height", type=int, default=900)
    parser.add_argument("--js", default="1",
                        help="expression to evaluate, for `eval`")
    parser.add_argument("--no-js", action="store_true")
    parser.add_argument("--reduced-motion", action="store_true")
    parser.add_argument("--port", type=int, default=9225)
    parser.add_argument("--server", default="http://127.0.0.1:8787")
    args = parser.parse_args()

    if args.command == "overflow":
        return command_overflow(args)
    return command_eval(args)


if __name__ == "__main__":
    sys.exit(main())
