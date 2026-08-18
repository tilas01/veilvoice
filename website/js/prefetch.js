// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//
// Fetch, quietly and in the background, the few things a reader is most likely
// to open next -- so that clicking them is instant instead of a wait.
//
// # Why any of this is in JavaScript at all
//
// Most of it is not. The pages carry `<link rel="prefetch">` in their markup,
// which is declarative, costs no script, and works in the JavaScript-free
// edition exactly as it does here. That is deliberate: a reader who runs no
// scripts should not get a slower site as a punishment.
//
// This file exists for the one thing that should *not* be declared in markup:
// `search-index.json` is about a megabyte. A `<link rel="prefetch">` for it
// would be fetched by every visitor on every page, including somebody on a
// metered phone connection who never opens the search. So it is fetched from
// script, where the conditions below can be checked first.
//
// # The conditions, and why each one
//
//   - `Save-Data`. If the reader has asked their browser to use less data,
//     downloading a megabyte they did not ask for is precisely the thing they
//     asked not to happen.
//   - `prefers-reduced-data`. The same request expressed as a media query,
//     which is what Safari implements.
//   - `effectiveType`. On 2G, a megabyte in the background competes with the
//     page the reader is actually trying to read.
//   - Idle time. `requestIdleCallback` means this never runs while the browser
//     has something better to do. Without it a prefetch can delay the very
//     page it was meant to make faster.
//
// # Same origin only
//
// Every URL here is a path on this site. This is stated because a prefetch is
// a real network request, and a privacy tool that quietly reached a third party
// to make itself feel fast would be undermining its own argument. There is no
// third-party host in this file and there is nothing to configure.

(function () {
  "use strict";

  // Small documents worth having ready, by page. Keep these short: a prefetch
  // list that includes everything is a download of the whole site.
  var LIKELY = {
    "index.html": ["wiki.html", "search.html"],
    "wiki.html": ["search.html", "index.html"],
    "search.html": []
  };

  // Fetched only from the pages where search is one click away, and only when
  // the connection looks willing. About a megabyte.
  var INDEX = "search-index.json";

  function pageName() {
    var path = window.location.pathname;
    var last = path.slice(path.lastIndexOf("/") + 1);
    return last || "index.html";
  }

  /** Has the reader asked, in any of the available ways, for less data? */
  function wantsLessData() {
    var connection = navigator.connection || navigator.mozConnection ||
                     navigator.webkitConnection;
    if (connection) {
      if (connection.saveData) { return true; }
      var type = connection.effectiveType || "";
      if (type === "slow-2g" || type === "2g") { return true; }
    }
    if (window.matchMedia && window.matchMedia("(prefers-reduced-data: reduce)").matches) {
      return true;
    }
    return false;
  }

  /** Already declared in the markup, or already added by an earlier call? */
  function alreadyQueued(url) {
    var existing = document.querySelectorAll('link[rel="prefetch"]');
    for (var i = 0; i < existing.length; i++) {
      if (existing[i].getAttribute("href") === url) { return true; }
    }
    return false;
  }

  function prefetch(url, as) {
    // The pages already carry `<link rel="prefetch">` for the small documents,
    // because that works with no script at all. Adding them again from here
    // asks the browser for the same file twice -- which it may well collapse,
    // but "the browser probably deduplicates it" is not a reason to send it.
    if (alreadyQueued(url)) { return; }

    var link = document.createElement("link");
    // `prefetch` rather than `preload`: this is for a *later* navigation, and
    // `preload` would tell the browser the current page needs it, which is
    // false and produces a console warning saying so.
    link.rel = "prefetch";
    link.href = url;
    if (as) { link.as = as; }
    document.head.appendChild(link);
  }

  function whenIdle(fn) {
    if (window.requestIdleCallback) {
      window.requestIdleCallback(fn, { timeout: 4000 });
    } else {
      // Safari has no requestIdleCallback. A timeout well after load is a
      // poor imitation, but it keeps the work off the critical path, which is
      // the part that matters.
      window.setTimeout(fn, 2500);
    }
  }

  function start() {
    if (wantsLessData()) { return; }

    var here = pageName();
    var pages = LIKELY[here];
    if (!pages) { return; }

    whenIdle(function () {
      for (var i = 0; i < pages.length; i++) { prefetch(pages[i], "document"); }

      // The index, only where search is the obvious next click, and only after
      // the small pages are queued.
      if (here === "index.html" || here === "wiki.html") {
        whenIdle(function () { prefetch(INDEX, "fetch"); });
      }
    });
  }

  // `load`, not `DOMContentLoaded`: prefetching before the current page has
  // finished fetching its own assets is competing with itself.
  if (document.readyState === "complete") { start(); }
  else { window.addEventListener("load", start, { once: true }); }
})();
