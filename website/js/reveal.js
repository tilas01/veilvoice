// SPDX-License-Identifier: GPL-3.0-or-later
//
// Reveal-on-scroll, in about thirty lines and with no dependencies.
//
// Design constraints, in order of importance:
//
//  1. Never hide content it cannot then show. The `.reveal` rule is scoped to
//     `html.js`, which `theme.js` sets from a blocking head script — so a
//     reader without JavaScript sees everything immediately, and there is no
//     window in which text exists but is invisible.
//  2. Cost nothing while scrolling. An IntersectionObserver is called by the
//     browser only when an element crosses the threshold; there is no scroll
//     handler, no `getBoundingClientRect` in a loop, and no rAF pump. Each
//     element is unobserved the moment it has been shown, so a fully-read page
//     has no observer work left to do at all.
//  3. Animate only `opacity` and `transform`, which the compositor can do
//     without laying the page out again.
//  4. Obey `prefers-reduced-motion`. Someone who has asked the operating system
//     for less movement gets the content with no movement at all, not a
//     politely shortened animation.

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

    var observer = new IntersectionObserver(function (entries, self) {
      for (var i = 0; i < entries.length; i++) {
        if (entries[i].isIntersecting) {
          entries[i].target.classList.add("in");
          // Once shown, stay shown: re-hiding on scroll-up is a gimmick that
          // makes a page harder to read, not nicer to look at.
          self.unobserve(entries[i].target);
        }
      }
    }, {
      // Start the transition slightly before the element reaches the viewport,
      // so it has finished by the time it is properly in view.
      rootMargin: "0px 0px -12% 0px",
      threshold: 0.05
    });

    for (var i = 0; i < nodes.length; i++) { observer.observe(nodes[i]); }
  });
})();
