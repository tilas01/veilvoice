// SPDX-License-Identifier: GPL-3.0-or-later
//
// Following a link to a section of the page lands on that section's heading,
// with the heading visible.
//
// # The defect this replaces
//
// The header is `position: sticky`, so the top of the viewport is covered by
// it. `scroll-margin-top` is the property for that, and this site set it to a
// flat `90px`. The header is not 90px tall. Measured in a browser it is 133px
// at desktop widths, 171px where the navigation wraps to three rows, 115px on
// a phone, 143px at 320px, and 81px on the reference pages. Every one of those
// is a landing that is wrong, and at the common desktop width it is 43px
// wrong in the direction that hides the heading behind the header.
//
// It was also set on `section`, `h2` and `h3` only, so an `h4` or a list item
// with an id -- which is most of the releases page and every roadmap entry --
// had no offset at all and landed a full header height underneath it.
//
// The edition of this site that runs no scripts has never had this problem,
// because it has no sticky header. That is the standard being matched here.
//
// # How the offset is decided
//
// By measuring the header, not by writing a number down. `--anchor-offset` is
// set from the header's own height and kept current by a ResizeObserver, so a
// header that grows a row, a font that loads late and a phone that is turned
// sideways all correct themselves. The stylesheet carries a starting value for
// the moment before this runs, and that value is never the one that matters.
//
// # Why the scroll is repeated
//
// A fragment jump happens once, at the moment the browser reads the hash, and
// the page is not finished at that moment. An image without intrinsic size
// finishes loading, a web font replaces the fallback, a reveal transition
// takes its transform off, and the heading that was in the right place is now
// somewhere else. So the position is checked over the second that follows and
// corrected if it drifts, and the checking stops the instant the reader
// scrolls: catching up with a moving page is the job, fighting the reader is
// not.
//
// Reveal transitions are settled up front rather than corrected afterwards.
// `.reveal` holds an element 18px below where it belongs, and an element
// scrolled to is an element that has been reached, so the target and anything
// holding it are shown before the browser scrolls, while the click is still
// being handled.
//
// # The landing that arrived only once
//
// The highlight is drawn when a landing is *cued*, and until F-205 the only
// thing that cued one was `hashchange`. That covers arriving at a page with a
// fragment already in the address bar, and it covers the first click on a
// link into the page, because that click writes a hash where there was none
// or a different one.
//
// It does not cover a click on a link naming the fragment that is already
// there. No new hash is written, so no `hashchange` arrives, so nothing runs.
// The link still works and the browser still scrolls, and the reader gets no
// cue at all. On a page with a contents list that is most of the clicks after
// the first one: pick an entry, read, come back, pick the same entry again,
// and the second time nothing is marked.
//
// So the click handler below cues that one case itself, on the frame after
// the browser has done its own scrolling. It is the only case it takes: every
// other click writes a hash, and cueing it here as well would draw the
// highlight twice.
//
// # What is deliberately left alone
//
// The navigation itself. Clicks are not prevented and no history entry is
// written here, because the full-size screenshot viewers are opened by
// `:target`, which follows a real fragment navigation and not a `pushState`.
// The back button, the middle button and copying a link therefore behave
// exactly as they do with this file absent, which is also what happens if it
// fails to load: the stylesheet still clears the header, just less exactly.
//
// In plain words
//
// Clicking a link that points further down the same page takes you to exactly
// that spot, with the heading you asked for on screen rather than hidden under
// the bar at the top, and a brief highlight so you can see where you landed.

(function () {
  "use strict";

  var doc = document.documentElement;

  // Clear of the header, plus enough that the heading is not touching it.
  var BREATHING_ROOM = 14;

  // How long the page is given to stop moving after a landing.
  var SETTLE_MS = 1000;

  // How long the landing stays highlighted.
  var HIGHLIGHT_MS = 2000;

  var settleUntil = 0;
  var settling = false;
  var touched = false;
  var wanted = null;
  var highlighted = null;
  var highlightTimer = 0;

  function header() {
    return document.querySelector("header.top");
  }

  /** Publish the header's real height, so the stylesheet can clear it. */
  function measure() {
    var bar = header();
    if (!bar) { return; }
    // A header that is not sticky covers nothing: the reference pages and any
    // page narrow enough for the browser to drop stickiness need no offset.
    var stuck = window.getComputedStyle(bar).position === "sticky";
    var height = stuck ? Math.ceil(bar.getBoundingClientRect().height) : 0;
    doc.style.setProperty("--anchor-offset", (height + BREATHING_ROOM) + "px");
  }

  /** Where the top of `el` should end up, in document coordinates. */
  function restingPlace(el) {
    var bar = header();
    var stuck = bar && window.getComputedStyle(bar).position === "sticky";
    var clearance = stuck ? bar.getBoundingClientRect().height + BREATHING_ROOM : 0;
    var top = el.getBoundingClientRect().top + window.pageYOffset - clearance;
    var floor = Math.max(0, doc.scrollHeight - window.innerHeight);
    return Math.max(0, Math.min(Math.round(top), floor));
  }

  /**
   * Scroll there now, whatever `scroll-behavior` the stylesheet asks for.
   *
   * Not `scrollTo({ behavior: "auto" })`: in the specification `auto` means
   * "whatever the CSS property says", which here is `smooth`, so that form
   * animates. `instant` is the value that means instant, and it is newer than
   * some of the browsers this site supports. Turning the property off for the
   * one call is older than both and does the same thing.
   */
  function snapTo(place) {
    var was = doc.style.scrollBehavior;
    doc.style.scrollBehavior = "auto";
    window.scrollTo(0, place);
    doc.style.scrollBehavior = was;
  }

  /**
   * Take the reveal transition off the target and everything holding it.
   *
   * Without this the element is measured 18px below where it will settle, and
   * the landing is 18px out by the time the transition finishes.
   */
  function settleReveal(el) {
    for (var node = el; node && node !== document.body; node = node.parentElement) {
      if (node.classList && node.classList.contains("reveal")) {
        node.classList.add("in");
      }
    }
  }

  /**
   * What to outline. Outlining a whole section outlines most of the screen,
   * which points at nothing; its heading is the thing the reader came for.
   */
  function cueFor(el) {
    if (/^(SECTION|ARTICLE|DIV|MAIN)$/.test(el.tagName)) {
      var heading = el.querySelector("h1, h2, h3, h4");
      if (heading) { return heading; }
    }
    return el;
  }

  /** Mark where the reader landed, briefly. */
  function highlight(el) {
    el = cueFor(el);
    if (highlighted) { highlighted.classList.remove("landed"); }
    if (highlightTimer) { window.clearTimeout(highlightTimer); }
    // Restarting the animation needs the class gone for a frame.
    highlighted = el;
    window.requestAnimationFrame(function () {
      el.classList.add("landed");
      highlightTimer = window.setTimeout(function () {
        el.classList.remove("landed");
        if (highlighted === el) { highlighted = null; }
      }, HIGHLIGHT_MS);
    });
  }

  /**
   * Continue the landing for as long as the page is still moving underneath
   * it. Cancelled by the reader touching the page at all.
   *
   * The stylesheet asks for `scroll-behavior: smooth`, so the jump is an
   * animation rather than an event, and a correction issued while it is
   * running restarts it from wherever it had got to. That is what a first
   * version of this did on every frame, and the page crept towards the target
   * over more than a second instead of gliding to it. So corrections wait for
   * the scrolling to stop, and when they are made they are made instantly: by
   * then it is a few pixels, and a few pixels do not need an animation.
   */
  function settle() {
    if (settling) { return; }
    settling = true;
    var previous = null;
    window.requestAnimationFrame(function step() {
      if (!wanted || Date.now() > settleUntil) { settling = false; wanted = null; return; }
      var here = window.pageYOffset;
      if (here === previous) {
        var place = restingPlace(wanted);
        if (Math.abs(here - place) > 1) { snapTo(place); }
      }
      previous = here;
      window.requestAnimationFrame(step);
    });
  }

  function stopSettling() {
    touched = true;
    settleUntil = 0;
    wanted = null;
  }

  /** Put `id` on screen properly. `cue` moves focus and marks the landing. */
  function teleport(id, cue) {
    var el = document.getElementById(id);
    if (!el) { return; }

    // The full-size screenshot viewers are fixed overlays opened by `:target`.
    // They cover the page rather than sitting in it, so there is nothing to
    // scroll to and scrolling the page underneath one is a change the reader
    // sees when they close it.
    if (el.classList.contains("viewer")) { return; }

    settleReveal(el);

    // A short hop glides, because watching the page move the height of a
    // screen or two is what tells somebody they have gone down rather than
    // sideways. A long one does not: the stylesheet asks for
    // `scroll-behavior: smooth`, and smooth over twenty thousand pixels is a
    // second and a half of everything on the page rushing past, which orients
    // nobody. Past two screens this goes straight there and the outline below
    // says where "there" is.
    var place = restingPlace(el);
    if (Math.abs(window.pageYOffset - place) > 2 * window.innerHeight) {
      snapTo(place);
    } else {
      window.scrollTo(0, place);
    }

    if (cue) {
      // Focus follows the landing, or a keyboard reader carries on from
      // wherever they were rather than from what they asked for.
      if (!el.hasAttribute("tabindex")) { el.setAttribute("tabindex", "-1"); }
      try { el.focus({ preventScroll: true }); } catch (e) { el.focus(); }
      highlight(el);
    }

    touched = false;
    wanted = el;
    settleUntil = Date.now() + SETTLE_MS;
    settle();
  }

  /** The id in the address bar, if it has one. */
  function named() {
    var hash = window.location.hash;
    if (hash.length < 2) { return null; }
    try {
      return decodeURIComponent(hash.slice(1));
    } catch (e) {
      // A hash that is not valid percent-encoding is not an id here either,
      // but `decodeURIComponent` throws rather than saying so.
      return hash.slice(1);
    }
  }

  function fromHash() {
    var id = named();
    if (id) { teleport(id, true); }
  }

  document.addEventListener("DOMContentLoaded", function () {
    measure();

    if (window.ResizeObserver) {
      var bar = header();
      if (bar) { new window.ResizeObserver(measure).observe(bar); }
    }
    window.addEventListener("resize", measure, { passive: true });
    window.addEventListener("orientationchange", measure, { passive: true });
    if (document.fonts && document.fonts.ready && document.fonts.ready.then) {
      document.fonts.ready.then(measure);
    }

    // A click on a link into this page settles the target's reveal transition
    // before the browser scrolls, and then lets the browser scroll. Nothing is
    // prevented and no history entry is written here: the screenshot viewers
    // are opened by `:target`, which follows real fragment navigation and not
    // a pushState, and the back button, the middle button and copying a link
    // all keep working because none of them has been taken over.
    //
    // A link naming the fragment that is already in the address bar writes no
    // new hash, so no `hashchange` arrives and `fromHash` never runs. The link
    // works and the page moves, but the landing is not cued: the same link
    // highlights the first time it is followed and never again, and a contents
    // page whose reader keeps coming back to it loses the cue after the first
    // entry they pick. That case is landed here instead, on the frame after
    // the browser has done its own scrolling so the two are not correcting the
    // same pixels at once.
    document.addEventListener("click", function (event) {
      if (event.defaultPrevented || event.button !== 0) { return; }
      // A held modifier means a new tab, a new window or a saved file. This
      // page is not going anywhere, so there is nothing here to cue.
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) { return; }
      var link = event.target.closest ? event.target.closest('a[href^="#"]') : null;
      if (!link) { return; }
      var id = decodeURIComponent(link.getAttribute("href").slice(1));
      var el = id && document.getElementById(id);
      if (!el) { return; }
      settleReveal(el);
      if (named() === id) {
        window.requestAnimationFrame(function () { teleport(id, true); });
      }
    }, true);

    // The browser has already jumped by the time this runs, using whatever the
    // stylesheet's starting offset was. Landing again with the measured one is
    // the same frame, so there is nothing to see.
    fromHash();

    // Everything below the fold finishes loading after this, and some of it
    // changes the height of what is above it. Landing once more when it is all
    // in, quietly: no second highlight, and nothing at all if the reader has
    // started reading in the meantime.
    window.addEventListener("load", function () {
      var id = named();
      if (id && !touched) { teleport(id, false); }
    });
  });

  window.addEventListener("hashchange", fromHash);
  window.addEventListener("popstate", fromHash);

  for (var i = 0, events = ["wheel", "touchstart", "keydown", "mousedown"]; i < events.length; i++) {
    window.addEventListener(events[i], stopSettling, { passive: true });
  }
})();
