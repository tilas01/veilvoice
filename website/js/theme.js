// SPDX-License-Identifier: GPL-3.0-or-later
//
// Theme switching. Nine palettes, Tokyo Night default.
//
// The choice is kept in localStorage, which never leaves the browser. No
// cookie, so nothing is attached to a request and there is nothing to consent
// to -- a preference the server never sees is not tracking.
//
// In plain words
//
// This is the colour-scheme menu in the corner of the page. Pick a theme and
// every page on this site changes to it, and stays that way next time you
// come back.
//
// Your choice is kept in your own browser. It is not a cookie, so it is never
// sent anywhere: this site has no server that could receive it and nothing
// that could match it to you.

(function () {
  "use strict";

  var THEMES = [
    ["tokyo-night", "Tokyo Night"],
    ["gruvbox", "Gruvbox"],
    ["dracula", "Dracula"],
    ["nord", "Nord"],
    ["catppuccin", "Catppuccin Mocha"],
    ["everforest", "Everforest"],
    ["solarized", "Solarized Dark"],
    // Written as an escape, not a literal: this file is source the site
    // explicitly invites people to open and read, and a reader whose viewer
    // guesses the wrong encoding would see mojibake instead of a theme name.
    // The escape is ASCII on disk and the correct character on screen.
    ["rose-pine", "Ros\u00e9 Pine"],
    ["paper", "Paper (light)"]
  ];

  var KEY = "veilvoice-theme";
  var DEFAULT = "tokyo-night";

  function valid(name) {
    return THEMES.some(function (t) { return t[0] === name; });
  }

  function stored() {
    try {
      var v = localStorage.getItem(KEY);
      return valid(v) ? v : DEFAULT;
    } catch (e) {
      // Private browsing can throw on access rather than return null.
      return DEFAULT;
    }
  }

  function apply(name) {
    document.documentElement.setAttribute("data-theme", name);
    try { localStorage.setItem(KEY, name); } catch (e) { /* not fatal */ }
  }

  function build(select) {
    var current = stored();
    THEMES.forEach(function (t) {
      var opt = document.createElement("option");
      opt.value = t[0];
      opt.textContent = t[1];
      if (t[0] === current) { opt.selected = true; }
      select.appendChild(opt);
    });
    select.addEventListener("change", function () { apply(select.value); });
  }

  // Applied before DOMContentLoaded so the page never flashes the default
  // palette before switching to the reader's choice.
  apply(stored());

  // Marks the document as scripted, from a *blocking* head script, so the
  // class is set before the first paint. Scroll reveals hide themselves only
  // under `html.js`: without JavaScript the content is simply visible, rather
  // than transparent for ever waiting for an observer that will never run.
  document.documentElement.classList.add("js");

  // Upgrade the JavaScript switch from its honest default.
  //
  // The markup says `aria-checked="false"` because markup cannot know whether
  // scripts run, and a switch that claims "on" when nothing is running is
  // simply lying to whoever most needs the truth. Reaching this line proves
  // scripts run, so the attribute is corrected here -- the visual state is
  // handled by CSS through `html.js`, but assistive technology reads the
  // attribute and it has to agree.
  document.addEventListener("DOMContentLoaded", function () {
    var toggle = document.querySelector(".js-toggle[role=\"switch\"]");
    // Only on the full site: the no-JavaScript edition's switch is genuinely
    // off, and it does not load this file anyway.
    if (toggle && toggle.getAttribute("href") !== "../index.html") {
      toggle.setAttribute("aria-checked", "true");
    }
  });

  document.addEventListener("DOMContentLoaded", function () {
    var select = document.getElementById("theme");
    if (select) { build(select); }
  });
})();
