// SPDX-License-Identifier: GPL-3.0-or-later
//
// Theme switching. Nine palettes, Tokyo Night default.
//
// The choice is kept in localStorage, which never leaves the browser. No
// cookie, so nothing is attached to a request and there is nothing to consent
// to — a preference the server never sees is not tracking.

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
    ["rose-pine", "Rosé Pine"],
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

  document.addEventListener("DOMContentLoaded", function () {
    var select = document.getElementById("theme");
    if (select) { build(select); }
  });
})();
