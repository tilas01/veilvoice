// SPDX-License-Identifier: GPL-3.0-or-later
//
// Reveal-on-scroll, with one rule that outranks every other consideration:
// **content must never stay invisible.**
//
// The first version of this file broke that rule, and it took rendering the
// page to notice. An IntersectionObserver fires when an element's intersection
// ratio crosses a threshold. If the viewport *jumps* -- an anchor link from the
// nav, a browser restoring your scroll position when you come back, a
// find-in-page hit -- an element can go from below the viewport to above it
// between two frames. It was not intersecting before and is not intersecting
// after, the ratio never left zero, and no callback ever runs. Because a reveal
// that re-hides on scroll-up is a gimmick, nothing ever showed it again.
//
// Three paragraphs of the walkthrough were invisible that way, and one of them
// was the box explaining that the app lock is not tamper-proof. A page whose
// entire argument is that it states its limits had made a limit unreadable.
//
// So there are two mechanisms here, and they are not redundant:
//
//   1. The observer, which does the animation and costs nothing while idle.
//   2. A sweep, which asks a much simpler question -- "is this element at or
//      above the bottom of the viewport?" -- and reveals anything that is,
//      whether or not the observer ever saw it cross. It runs on scroll and
//      resize, coalesced into one animation frame, and **both listeners and the
//      observer detach the moment the last element is revealed.** On an
//      ordinary read that is a fraction of a second of work in total, and none
//      at all thereafter.
//
// The other constraints, unchanged:
//
//  - Never hide what cannot then be shown. The `.reveal` rule is scoped to
//    `html.js`, set by `theme.js` from a blocking head script, so a reader
//    without JavaScript sees everything immediately.
//  - Animate only `opacity` and `transform`, which the compositor handles
//    without laying out the page again.
//  - Obey `prefers-reduced-motion`: someone who asked the system for less
//    movement gets the content with no movement at all.

(function () {
  "use strict";

  function showAll(nodes) {
    for (var i = 0; i < nodes.length; i++) { nodes[i].classList.add("in"); }
  }

  document.addEventListener("DOMContentLoaded", function () {
    var nodes = document.querySelectorAll(".reveal");
    if (!nodes.length) { return; }

    var still = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (still || !("IntersectionObserver" in window)) {
      showAll(nodes);
      return;
    }

    // Everything not yet shown. Emptying this is what tears the whole thing
    // down, so it is the single source of truth for "is there work left".
    var pending = Array.prototype.slice.call(nodes);
    var scheduled = false;

    function reveal(node) {
      node.classList.add("in");
      var at = pending.indexOf(node);
      if (at !== -1) { pending.splice(at, 1); }
      if (observer) { observer.unobserve(node); }
      if (!pending.length) { stop(); }
    }

    function stop() {
      if (observer) { observer.disconnect(); observer = null; }
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
    }

    // The safety net. Anything whose top has reached the bottom of the viewport
    // has been scrolled to, however the viewport got there.
    function sweep() {
      scheduled = false;
      var limit = window.innerHeight || document.documentElement.clientHeight;
      for (var i = pending.length - 1; i >= 0; i--) {
        if (pending[i].getBoundingClientRect().top <= limit) { reveal(pending[i]); }
      }
    }

    function schedule() {
      if (scheduled) { return; }
      scheduled = true;
      window.requestAnimationFrame(sweep);
    }

    var observer = new IntersectionObserver(function (entries) {
      for (var i = 0; i < entries.length; i++) {
        if (entries[i].isIntersecting) { reveal(entries[i].target); }
      }
    }, {
      // Start the transition slightly before the element reaches the viewport,
      // so it has finished by the time it is properly in view.
      rootMargin: "0px 0px -12% 0px",
      threshold: 0.05
    });

    for (var i = 0; i < pending.length; i++) { observer.observe(pending[i]); }

    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule, { passive: true });

    // And once now, because the page may already have been restored to a
    // position halfway down it before a single scroll event fires.
    schedule();
  });
})();
