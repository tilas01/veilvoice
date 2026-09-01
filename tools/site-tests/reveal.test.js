// SPDX-License-Identifier: GPL-3.0-or-later
//
// Behaviour tests for the scroll-reveal effect.
//
// The invariant is narrow and absolute: **every path must end with the content
// visible.** A reveal that hides text it then fails to show is worse than no
// effect at all.
//
// The previous version of this file passed while the deployed page had three
// permanently invisible paragraphs, one of which was the box explaining that
// the app lock is not tamper-proof. It passed because its stub only ever
// modelled the observer firing, and the real failure was the observer *not*
// firing, when a viewport jump carries an element from below the fold to above
// it between two frames without the intersection ratio ever leaving zero.
//
// So the stub here models a viewport with a position, and the tests drive it
// the way a browser does: gradual scrolling, anchor jumps, and a restored
// scroll position on load.

"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const SOURCE = fs.readFileSync(
  path.resolve(__dirname, "..", "..", "website", "js", "reveal.js"),
  "utf8"
);

/** A fake page: nodes at fixed document offsets, and a viewport over them. */
function makePage({ count = 6, spacing = 400, viewportHeight = 800, reducedMotion = false,
                    hasIO = true } = {}) {
  const page = { scrollY: 0, viewportHeight, frames: [], listeners: {}, observed: new Set() };

  page.nodes = Array.from({ length: count }, (_, i) => {
    const classes = new Set(["reveal"]);
    return {
      _top: i * spacing,
      classList: {
        add: v => classes.add(v),
        contains: v => classes.has(v)
      },
      getBoundingClientRect: () => ({ top: (i * spacing) - page.scrollY })
    };
  });

  let observerCallback = null;
  const sandbox = {
    document: {
      addEventListener: (ev, fn) => { if (ev === "DOMContentLoaded") { page.ready = fn; } },
      querySelectorAll: () => page.nodes
    },
    window: {
      matchMedia: () => ({ matches: reducedMotion }),
      get innerHeight() { return page.viewportHeight; },
      requestAnimationFrame: fn => { page.frames.push(fn); },
      addEventListener: (ev, fn) => { (page.listeners[ev] = page.listeners[ev] || []).push(fn); },
      removeEventListener: (ev, fn) => {
        page.listeners[ev] = (page.listeners[ev] || []).filter(f => f !== fn);
      }
    }
  };
  if (hasIO) {
    sandbox.window.IntersectionObserver = function (cb) {
      observerCallback = cb;
      this.observe = n => page.observed.add(n);
      this.unobserve = n => page.observed.delete(n);
      this.disconnect = () => { page.observed.clear(); observerCallback = null; };
    };
    // In a browser `window` *is* the global object, so `IntersectionObserver`
    // and `window.IntersectionObserver` are the same binding. The stub has to
    // model that, or code that feature-detects on `window` and then constructs
    // from the global, which is ordinary, idiomatic browser code, fails here
    // for a reason that could never happen in a browser.
    sandbox.IntersectionObserver = sandbox.window.IntersectionObserver;
  }
  sandbox.requestAnimationFrame = sandbox.window.requestAnimationFrame;

  vm.createContext(sandbox);
  vm.runInContext(SOURCE, sandbox);
  page.ready();

  /** Run any animation frames the code asked for. */
  page.flush = () => {
    while (page.frames.length) { page.frames.shift()(); }
  };
  /** Move the viewport and fire a scroll event, as a browser would. */
  page.scrollTo = y => {
    page.scrollY = y;
    (page.listeners.scroll || []).forEach(fn => fn());
    page.flush();
  };
  /**
   * Move the viewport **without** the observer noticing, such as an anchor jump that
   * carries elements from below the fold to above it in one step. This is the
   * case that shipped broken.
   */
  page.jumpPast = y => page.scrollTo(y);
  /** Deliver an observer callback for whatever is currently intersecting. */
  page.settle = () => {
    if (!observerCallback) { return; }
    const entries = [...page.observed]
      .map(target => {
        const top = target.getBoundingClientRect().top;
        return { target, isIntersecting: top >= 0 && top <= page.viewportHeight };
      })
      .filter(e => e.isIntersecting);
    if (entries.length) { observerCallback(entries); }
    page.flush();
  };
  page.shown = () => page.nodes.filter(n => n.classList.contains("in")).length;
  page.hidden = () => page.nodes.filter(n => !n.classList.contains("in")).length;
  page.listenerCount = () =>
    (page.listeners.scroll || []).length + (page.listeners.resize || []).length;

  return page;
}

function run() {
  let fails = 0;
  const check = (name, ok) => {
    console.log((ok ? "ok   " : "FAIL ") + name);
    if (!ok) { fails++; }
  };

  // 1. Reduced motion: shown at once, nothing observed, nothing listening.
  {
    const p = makePage({ reducedMotion: true });
    check("reduced motion reveals everything at once", p.hidden() === 0);
    check("reduced motion observes nothing", p.observed.size === 0);
    check("reduced motion attaches no listeners", p.listenerCount() === 0);
  }

  // 2. No IntersectionObserver at all.
  {
    const p = makePage({ hasIO: false });
    check("missing IntersectionObserver still reveals everything", p.hidden() === 0);
  }

  // 3. The ordinary path: hidden at the top, revealed as it comes into view.
  {
    const p = makePage({ count: 6, spacing: 400, viewportHeight: 800 });
    p.flush();
    const initial = p.shown();
    check("only what is already on screen starts revealed", initial > 0 && initial < 6);
    p.scrollTo(400);
    p.settle();
    check("scrolling reveals more", p.shown() > initial);
  }

  // 4. **The regression.** An anchor jump straight past content, with the
  //    observer never reporting those elements as intersecting.
  {
    const p = makePage({ count: 6, spacing: 400, viewportHeight: 800 });
    p.flush();
    p.jumpPast(100000); // far below every element; all are now above the fold
    check("a jump past content still reveals all of it", p.hidden() === 0);
  }

  // 5. A restored scroll position on load, before any scroll event fires.
  {
    const p = makePage({ count: 6, spacing: 400, viewportHeight: 800 });
    p.scrollY = 100000;
    p.flush(); // only the frame the code queued for itself at startup
    check("a restored scroll position reveals what it skipped", p.hidden() === 0);
  }

  // 6. Teardown: once everything is shown, nothing is left running.
  {
    const p = makePage({ count: 4, spacing: 400, viewportHeight: 800 });
    p.scrollTo(100000);
    check("everything is revealed", p.hidden() === 0);
    check("listeners detach once there is no work left", p.listenerCount() === 0);
    check("the observer disconnects too", p.observed.size === 0);
  }

  // 7. Scroll bursts are coalesced rather than handled one by one.
  {
    const p = makePage({ count: 6, spacing: 4000, viewportHeight: 800 });
    // Run the frame the module queues at startup first. Discarding it instead
    // would leave the coalescing flag stuck on, and this test would then pass
    // for the wrong reason, by observing no frames at all.
    p.flush();
    p.frames.length = 0;
    p.scrollY = 10;
    (p.listeners.scroll || []).forEach(fn => fn());
    (p.listeners.scroll || []).forEach(fn => fn());
    (p.listeners.scroll || []).forEach(fn => fn());
    check("three scroll events queue one frame, not three", p.frames.length === 1);
    p.flush();
  }

  // 8. A page with no reveals must be a complete no-op.
  {
    let threw = false;
    try {
      const sandbox = {
        document: {
          addEventListener: (e, f) => e === "DOMContentLoaded" && (sandbox._r = f),
          querySelectorAll: () => []
        },
        window: {}
      };
      vm.createContext(sandbox);
      vm.runInContext(SOURCE, sandbox);
      sandbox._r();
    } catch (e) {
      threw = true;
    }
    check("a page with no reveals is a no-op", !threw);
  }

  return fails;
}

module.exports = { run, name: "scroll reveal" };

if (require.main === module) { process.exit(run() ? 1 : 0); }
