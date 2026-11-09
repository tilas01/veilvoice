// SPDX-License-Identifier: GPL-3.0-or-later
//
// The pause button for the front-page banner.
//
// For most readers the banner animates and there is no button: `.bn-pause` is
// hidden by CSS. A reader whose system asks for reduced motion sees the banner
// keep a slow sweep running (see "calmer, not frozen" in main.css), and anything
// that moves for more than five seconds should be possible to stop, so for them
// the button appears and this file makes it work.
//
// The state lives in one class on the banner, `bn-paused`, which the reduced
// motion block in main.css reads. The button's `aria-pressed` and its label
// follow that class, so a screen reader hears what the button will do next.
//
// In plain words
//
// If you have asked your computer for less movement, a small button under the
// banner lets you stop its slow glow, and press it again to start it.

(function () {
  "use strict";

  document.addEventListener("DOMContentLoaded", function () {
    var button = document.querySelector(".bn-pause");
    var banner = document.querySelector(".bn");
    if (!button || !banner) { return; }

    function show(paused) {
      banner.classList.toggle("bn-paused", paused);
      button.setAttribute("aria-pressed", paused ? "true" : "false");
      button.textContent = paused ? "Play the banner" : "Pause the banner";
    }

    button.addEventListener("click", function () {
      show(!banner.classList.contains("bn-paused"));
    });
  });
})();
