// SPDX-License-Identifier: GPL-3.0-or-later
//
// Search across the whole repository and this website.
//
// The index is built by `tools/search-index/generate.py` and committed, so this
// file only has to read it. CI regenerates and compares, which means a stale
// index fails the build instead of quietly answering questions about code that
// no longer looks like that.
//
// # This file is pure ASCII, on purpose
//
// `website/js/*.js` is served raw and people are invited to open it. A viewer
// that guesses CP1252 turns an em dash into mojibake in the middle of a
// sentence promising the code is honest, so anything non-ASCII is written as a
// `\uXXXX` escape. `tools/site-tests/characters.test.js` fails the build
// otherwise.
//
// # Nothing from the index is ever HTML
//
// The index contains text taken verbatim from source files -- including, by
// construction, this project's own tests for hostile markup, which contain
// `<script>` and `onerror=` as ordinary content. Every value out of the index
// therefore reaches the page through `textContent` or `createTextNode` and
// never through `innerHTML`. Match highlighting is done by splitting a string
// and appending text nodes and `<mark>` elements built with `createElement`,
// which is why it looks more long-winded than a `replace` would.
//
// # Bounded work
//
// F-22 and F-23 were quadratic blow-ups in the Markdown renderer on text
// fetched over the network, and the lesson generalises: this file bounds the
// query, the number of terms, the number of results scored and the number
// rendered, so no input makes the tab do an unbounded amount of work. Scoring
// is a linear pass with a plain `indexOf` -- no regular expression is ever
// built from user input, so there is no pattern for a query to blow up.
//
// In plain words
//
// This is the search box. It looks through every file in the project -- the
// code, the documents and this website -- and shows you the lines that match.
//
// The searching happens in your browser, on a list that ships with the site.
// Nothing you type is sent anywhere, and there is nothing here that could
// collect it.

(function () {
  "use strict";

  var INDEX_URL = "search-index.json";

  var MAX_QUERY = 128;      // characters accepted from the box
  var MAX_TERMS = 8;        // terms scored from one query
  var MAX_RESULTS = 200;    // results kept after scoring
  var MAX_RENDER = 60;      // results put in the page at once
  var MAX_STAGGER = 12;     // how many results get an entrance delay

  var index = null;
  var state = { q: "", sort: "relevance", kind: "", area: "" };

  var els = {};

  // --- small helpers --------------------------------------------------------

  function byId(id) { return document.getElementById(id); }

  function text(tag, value, className) {
    var node = document.createElement(tag);
    if (value) { node.appendChild(document.createTextNode(value)); }
    if (className) { node.className = className; }
    return node;
  }

  function clear(node) {
    while (node.firstChild) { node.removeChild(node.firstChild); }
  }

  /**
   * Append `value` to `parent`, wrapping each occurrence of each term in a
   * `<mark>`. Built from text nodes and elements, never from a string of HTML.
   */
  function appendHighlighted(parent, value, terms) {
    if (!value) { return; }
    if (!terms.length) {
      parent.appendChild(document.createTextNode(value));
      return;
    }
    var lower = value.toLowerCase();
    var at = 0;
    var guard = 0;
    while (at < value.length && guard++ < 400) {
      // The earliest match of any term from here.
      var bestAt = -1;
      var bestLen = 0;
      for (var i = 0; i < terms.length; i++) {
        var found = lower.indexOf(terms[i], at);
        if (found !== -1 && (bestAt === -1 || found < bestAt)) {
          bestAt = found;
          bestLen = terms[i].length;
        }
      }
      if (bestAt === -1) { break; }
      if (bestAt > at) {
        parent.appendChild(document.createTextNode(value.slice(at, bestAt)));
      }
      parent.appendChild(text("mark", value.slice(bestAt, bestAt + bestLen)));
      at = bestAt + bestLen;
    }
    if (at < value.length) {
      parent.appendChild(document.createTextNode(value.slice(at)));
    }
  }

  // --- scoring --------------------------------------------------------------

  function parseQuery(raw) {
    var q = String(raw || "").slice(0, MAX_QUERY).toLowerCase();
    var parts = q.split(/[^a-z0-9_.:/-]+/);
    var out = [];
    for (var i = 0; i < parts.length && out.length < MAX_TERMS; i++) {
      if (parts[i]) { out.push(parts[i]); }
    }
    return out;
  }

  /**
   * How well one section answers the query.
   *
   * Deliberately simple and explainable: a heading match is worth more than a
   * body match, a path match is worth something, and every term must appear
   * somewhere or the section does not match at all. Returning 0 means "not a
   * result", never "a weak result".
   */
  function score(section, doc, terms) {
    // Read from the folded copies made once at load. Folding here instead --
    // which is what this did first -- meant three `toLowerCase()` calls per
    // section per keystroke: about fifteen thousand new strings for every
    // character typed, all of them thrown away immediately.
    //
    // Measured properly, minimum of 25 runs over 5,061 sections, because
    // timing noise is one-sided and the fastest run is the closest estimate of
    // the work actually done:
    //
    //     query             folded   folding per call
    //     "en"               0.7 ms       1.1 ms
    //     "encrypt"          1.0 ms       1.4 ms
    //     "the voiceprint"   0.8 ms       1.2 ms
    //
    // Folding once at load costs 0.9 ms. So this is worth having and it is
    // *not* the expensive part of a keystroke -- scoring the whole corpus is
    // about a millisecond either way. The cost that matters is building the
    // result rows, which is why `render` uses a fragment.
    var heading = section._h;
    var body = section._x;
    var path = doc._p;
    var total = 0;

    for (var i = 0; i < terms.length; i++) {
      var term = terms[i];
      var here = 0;
      if (heading.indexOf(term) !== -1) {
        here += heading === term ? 120 : 60;
        if (heading.indexOf(term) === 0) { here += 15; }
      }
      if (path.indexOf(term) !== -1) {
        here += 25;
        // The file's own name is a stronger signal than a directory in its path.
        if (doc._t.indexOf(term) !== -1) { here += 20; }
      }
      if (body.indexOf(term) !== -1) { here += 12; }
      if (!here) { return 0; }
      total += here;
    }

    // Documentation is what most people are looking for when they search prose;
    // a test file matching the same word usually is not. A small nudge, not a
    // filter -- the kind filter is there for when someone wants only one kind.
    if (doc.k === "doc") { total += 8; }
    if (doc.k === "rust") { total += 4; }
    return total;
  }

  function search() {
    if (!index) { return []; }
    var terms = parseQuery(state.q);
    var results = [];

    if (terms.length) {
      var secs = index.secs;
      for (var i = 0; i < secs.length; i++) {
        var section = secs[i];
        var doc = index.docs[section.d];
        if (state.kind && doc.k !== state.kind) { continue; }
        if (state.area && doc.r !== state.area) { continue; }
        var value = score(section, doc, terms);
        if (value > 0) { results.push({ s: section, d: doc, v: value }); }
      }
    } else {
      // No query: show the corpus, filtered. This is what makes the page
      // useful before anyone has typed anything.
      for (var j = 0; j < index.docs.length; j++) {
        var d = index.docs[j];
        if (state.kind && d.k !== state.kind) { continue; }
        if (state.area && d.r !== state.area) { continue; }
        results.push({ s: null, d: d, v: 0 });
      }
    }

    sortResults(results);
    return results.slice(0, MAX_RESULTS);
  }

  function sortResults(results) {
    var how = state.sort;
    results.sort(function (a, b) {
      if (how === "path") {
        return a.d.p < b.d.p ? -1 : a.d.p > b.d.p ? 1 : (a.s ? a.s.l : 0) - (b.s ? b.s.l : 0);
      }
      if (how === "title") {
        // Folded copies again: a comparator runs O(n log n) times, so folding
        // inside it is the same waste as folding inside `score`, spread over
        // more calls.
        var at = a.d._t;
        var bt = b.d._t;
        return at < bt ? -1 : at > bt ? 1 : 0;
      }
      if (how === "size") {
        return b.d.n - a.d.n;
      }
      // Relevance, with a stable tie-break so equal scores do not reshuffle
      // between keystrokes -- a list that jitters is hard to read.
      if (b.v !== a.v) { return b.v - a.v; }
      if (a.d.p !== b.d.p) { return a.d.p < b.d.p ? -1 : 1; }
      return (a.s ? a.s.l : 0) - (b.s ? b.s.l : 0);
    });
  }

  // --- rendering ------------------------------------------------------------

  function resultUrl(doc, section) {
    if (doc.u.indexOf("https://") === 0) {
      // A repository file: link to the line.
      return section && section.l > 1 ? doc.u + "#L" + section.l : doc.u;
    }
    return section && section.a ? doc.u + "#" + section.a : doc.u;
  }

  function renderResult(item, position, terms) {
    var row = document.createElement("li");
    row.className = "sr";
    if (position < MAX_STAGGER) {
      row.style.setProperty("--i", String(position));
    }

    var link = document.createElement("a");
    link.className = "sr-head";
    link.href = resultUrl(item.d, item.s);
    if (item.d.u.indexOf("https://") === 0) {
      link.rel = "noopener noreferrer";
    }
    appendHighlighted(link, (item.s && item.s.h) || item.d.t, terms);
    row.appendChild(link);

    var meta = text("div", null, "sr-meta");
    var kind = text("span", item.d.k, "sr-kind sr-kind-" + item.d.k);
    meta.appendChild(kind);
    var path = text("span", null, "sr-path");
    appendHighlighted(path, item.d.p, terms);
    meta.appendChild(path);
    if (item.s && item.s.l > 1) {
      meta.appendChild(text("span", "line " + item.s.l, "sr-line"));
    }
    row.appendChild(meta);

    if (item.s && item.s.x) {
      var snippet = text("p", null, "sr-x");
      appendHighlighted(snippet, item.s.x, terms);
      row.appendChild(snippet);
    }
    return row;
  }

  // The staggered entrance is right once, and wrong on every keystroke.
  //
  // The list is rebuilt on each render, so animating every item every time
  // would replay the whole cascade for each character typed -- which reads as
  // flicker, not polish, and is the opposite of what a fast search should feel
  // like. So the stagger runs only when the shape of the answer changes: the
  // first results after an empty box, a sort, or a filter. Refining a query
  // just swaps the text, which is why narrowing a search feels still.
  var lastShape = null;

  function shapeOf() {
    return state.sort + "\n" + state.kind + "\n" + state.area + "\n" +
      (state.q ? "q" : "-");
  }

  function render() {
    var results = search();
    var terms = parseQuery(state.q);

    var shape = shapeOf();
    var stagger = shape !== lastShape;
    lastShape = shape;

    var list = els.results;
    clear(list);
    list.classList.toggle("sr-stagger", stagger);

    // Build the rows off-document and attach them in one go.
    //
    // Appending each row to the live list makes the browser consider layout up
    // to sixty times per keystroke; a `DocumentFragment` is not in the
    // document, so nothing is laid out until the single `appendChild` at the
    // end. This is the part of a keystroke that actually costs something --
    // scoring the whole corpus is about a millisecond, and building rows is
    // most of the rest.
    var shown = results.slice(0, MAX_RENDER);
    var fragment = document.createDocumentFragment();
    for (var i = 0; i < shown.length; i++) {
      fragment.appendChild(renderResult(shown[i], i, terms));
    }
    list.appendChild(fragment);

    // The count is the honest number, not the number drawn.
    var summary;
    if (!state.q) {
      summary = results.length + " file" + (results.length === 1 ? "" : "s") +
        " in the index";
    } else if (!results.length) {
      summary = "nothing matched " + JSON.stringify(state.q);
    } else {
      summary = results.length + " result" + (results.length === 1 ? "" : "s");
      if (results.length > shown.length) {
        summary += ", showing the first " + shown.length;
      }
      if (results.length === MAX_RESULTS) {
        summary = "more than " + MAX_RESULTS + " results, showing the best " +
          shown.length;
      }
    }
    clear(els.count);
    els.count.appendChild(document.createTextNode(summary));

    els.empty.hidden = results.length !== 0;
  }

  // Coalesce keystrokes into one render per frame. Typing quickly should not
  // queue a render per character.
  var frame = null;
  function scheduleRender() {
    if (frame !== null) { return; }
    frame = window.requestAnimationFrame(function () {
      frame = null;
      render();
    });
  }

  // --- wiring ---------------------------------------------------------------

  function fillSelect(node, pairs, allLabel) {
    clear(node);
    var first = document.createElement("option");
    first.value = "";
    first.appendChild(document.createTextNode(allLabel));
    node.appendChild(first);
    for (var i = 0; i < pairs.length; i++) {
      var option = document.createElement("option");
      option.value = pairs[i][0];
      option.appendChild(document.createTextNode(pairs[i][1]));
      node.appendChild(option);
    }
  }

  function readQueryFromUrl() {
    try {
      var q = new URL(window.location.href).searchParams.get("q");
      if (q) { return String(q).slice(0, MAX_QUERY); }
    } catch (e) { /* older engine: no deep link, which is not fatal */ }
    return "";
  }

  function fail(message) {
    clear(els.count);
    els.count.appendChild(document.createTextNode(message));
    els.empty.hidden = true;
  }

  /**
   * Fold every searchable string to lower case, once.
   *
   * Search is case-insensitive and JavaScript has no case-insensitive
   * `indexOf`, so the choice is to fold on every comparison or to fold once
   * and keep the result. Folding once costs one pass at load and roughly the
   * size of the index again in memory; folding per keystroke costs an
   * allocation per section per character, for ever.
   *
   * The folded fields are prefixed with `_` and are never rendered -- what the
   * reader sees is always the original text, so a heading still shows its
   * capitals.
   */
  function fold(loaded) {
    var docs = loaded.docs;
    for (var i = 0; i < docs.length; i++) {
      docs[i]._p = String(docs[i].p || "").toLowerCase();
      docs[i]._t = String(docs[i].t || "").toLowerCase();
    }
    var secs = loaded.secs;
    for (var j = 0; j < secs.length; j++) {
      secs[j]._h = String(secs[j].h || "").toLowerCase();
      secs[j]._x = String(secs[j].x || "").toLowerCase();
    }
  }

  function start(loaded) {
    fold(loaded);
    index = loaded;

    fillSelect(els.kind, index.kinds, "every kind");
    fillSelect(els.area, index.areas.map(function (a) { return [a, a]; }),
               "everywhere");

    els.q.disabled = false;
    els.q.placeholder = "search " + index.docs.length + " files";

    var initial = readQueryFromUrl();
    if (initial) { els.q.value = initial; state.q = initial; }

    els.q.addEventListener("input", function () {
      state.q = els.q.value.slice(0, MAX_QUERY);
      scheduleRender();
    });
    els.sort.addEventListener("change", function () {
      state.sort = els.sort.value;
      scheduleRender();
    });
    els.kind.addEventListener("change", function () {
      state.kind = els.kind.value;
      scheduleRender();
    });
    els.area.addEventListener("change", function () {
      state.area = els.area.value;
      scheduleRender();
    });
    els.form.addEventListener("submit", function (event) {
      // There is no server to submit to; everything happens here.
      event.preventDefault();
      scheduleRender();
    });

    document.documentElement.classList.add("search-live");
    render();
    if (!initial) { els.q.focus(); }
  }

  document.addEventListener("DOMContentLoaded", function () {
    els.form = byId("search-form");
    els.q = byId("q");
    els.sort = byId("sort");
    els.kind = byId("kind");
    els.area = byId("area");
    els.results = byId("results");
    els.count = byId("result-count");
    els.empty = byId("no-results");
    if (!els.form || !els.q || !els.results) { return; }

    els.q.disabled = true;

    if (!window.fetch) {
      fail("This browser cannot load the index. The complete static index is " +
           "linked below and needs nothing but your browser's find-in-page.");
      return;
    }

    window.fetch(INDEX_URL, { credentials: "omit" })
      .then(function (response) {
        if (!response.ok) { throw new Error("HTTP " + response.status); }
        return response.json();
      })
      .then(function (data) {
        // Treat the index as data of unknown shape rather than as something
        // that must be well-formed. A truncated deploy should say so, not throw
        // a TypeError somewhere further in and blame the network.
        if (!data || typeof data !== "object" ||
            !Array.isArray(data.docs) || !Array.isArray(data.secs) ||
            !Array.isArray(data.kinds) || !Array.isArray(data.areas)) {
          throw new Error("the index is not in the expected shape");
        }
        start(data);
      })
      .catch(function (error) {
        fail("Could not load the search index (" + error.message + "). The " +
             "complete static index is linked below and needs no JavaScript.");
      });
  });
})();
