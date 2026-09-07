// SPDX-License-Identifier: GPL-3.0-or-later
//
// Every screen of the application as a photograph you pick between, and the
// command line as a list of jobs rather than a list of flags.
//
// # What this is, and what it deliberately is not
//
// Nothing here pretends to run. The pictures are captures of the real window,
// taken by the build on every commit, and the only thing the reader drives is
// which one they are looking at. That is a smaller claim than an interactive
// model of the program, and it is one the page can actually keep.
//
// There used to be such a model, drawn in CSS and opened from a button. It is
// gone: a drawing of an interface is exactly the thing a reader cannot check,
// and offering it beside photographs of the same interface asked somebody to
// decide which of the two to believe. `js/sessions.js` replaced it with the
// recorded terminal sessions, which are the real programs' real output.
//
// # Where the content comes from
//
// All of it is in `window.VEILVOICE_DEMO`, which `tools/site/demo.py` writes
// from the source: the tab list out of `app.rs`, the pictures out of
// `assets/screenshots`, and every worked command checked against the program's
// own `--help`. Nothing is typed here, so nothing here can drift.
//
// In plain words
//
// Lets you click through screenshots of the real app, and read what each
// command line job actually does, without downloading anything.

(function () {
  "use strict";

  var data = window.VEILVOICE_DEMO || {};
  var shots = data.shots || [];
  var cases = data.usecases || [];

  var tabsEl = null;
  var imgEl = null;
  var noteEl = null;
  var casesEl = null;

  var current = 0;

  function select(index, focus) {
    if (!shots.length) { return; }
    if (index < 0) { index = shots.length - 1; }
    if (index >= shots.length) { index = 0; }
    current = index;
    var shot = shots[index];

    imgEl.setAttribute("src", shot.image);
    // The label already names the screen, so the alt text says what the
    // picture is rather than repeating the button next to it.
    imgEl.setAttribute("alt", "The " + shot.label + " screen of VeilVoice");
    noteEl.textContent = shot.note;

    var buttons = tabsEl.querySelectorAll("button");
    Array.prototype.forEach.call(buttons, function (button, at) {
      var on = at === index;
      button.setAttribute("aria-selected", on ? "true" : "false");
      // Only the selected tab is in the tab order: a tablist is one stop, and
      // arrow keys move within it. Nine tab stops in a row would be nine
      // things to get past for somebody who does not want any of them.
      button.setAttribute("tabindex", on ? "0" : "-1");
      button.className = on ? "walk-tab walk-tab-on" : "walk-tab";
      if (on && focus) { button.focus(); }
    });
  }

  function buildTabs() {
    if (!shots.length) { return; }
    shots.forEach(function (shot, index) {
      var button = document.createElement("button");
      button.type = "button";
      button.className = "walk-tab";
      button.setAttribute("role", "tab");
      button.textContent = shot.label;
      button.addEventListener("click", function () { select(index, false); });
      tabsEl.appendChild(button);
    });

    tabsEl.addEventListener("keydown", function (event) {
      var key = event.key;
      if (key === "ArrowRight" || key === "ArrowDown") {
        event.preventDefault();
        select(current + 1, true);
      } else if (key === "ArrowLeft" || key === "ArrowUp") {
        event.preventDefault();
        select(current - 1, true);
      } else if (key === "Home") {
        event.preventDefault();
        select(0, true);
      } else if (key === "End") {
        event.preventDefault();
        select(shots.length - 1, true);
      }
    });

    select(0, false);
  }

  function buildCases() {
    if (!cases.length) { return; }
    cases.forEach(function (item) {
      var row = document.createElement("div");
      row.className = "walk-case";

      var title = document.createElement("h4");
      title.className = "walk-case-title";
      title.textContent = item.title;

      var code = document.createElement("code");
      code.className = "walk-case-cmd";
      code.textContent = item.typed;

      var note = document.createElement("p");
      note.className = "walk-case-note";
      note.textContent = item.note;

      row.appendChild(title);
      row.appendChild(code);
      row.appendChild(note);
      casesEl.appendChild(row);
    });
  }

  // The Demo link in the header lands on the section. Somebody who followed it
  // came to look at the program, so the first screen is already shown by the
  // time they arrive rather than waiting behind another click.
  function fromFragment() {
    var hash = window.location.hash;
    if (hash !== "#demo" && hash !== "#walkthrough") { return; }
    var target = document.getElementById("walkthrough");
    if (!target) { return; }
    select(current, false);
    if (hash === "#walkthrough") {
      target.scrollIntoView({ block: "start" });
    }
  }

  // The elements are looked up here rather than at load, and the handler stops
  // if the ones it cannot work without are missing. `tools/site/split.py` reads
  // this guard to decide which of the section pages need this file at all, so a
  // page that carries none of this markup does not download it.
  document.addEventListener("DOMContentLoaded", function () {
    var imgNode = document.getElementById("walk-img");
    var noteNode = document.getElementById("walk-note");
    if (!imgNode || !noteNode) { return; }
    imgEl = imgNode;
    noteEl = noteNode;
    tabsEl = document.querySelector(".walk-tabs");
    casesEl = document.querySelector(".walk-cases");
    if (!tabsEl || !casesEl) { return; }
    buildTabs();
    buildCases();
    fromFragment();
  });

  window.addEventListener("hashchange", fromFragment);
})();
