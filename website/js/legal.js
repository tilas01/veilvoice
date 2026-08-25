// SPDX-License-Identifier: GPL-3.0-or-later
//
// The welcome dialog: licence terms, liability waiver, and the disclosure that
// this project was built with AI assistance.
//
// # Shown once per session, and never phoned home
//
// Acceptance is recorded in sessionStorage, so it survives navigation between
// pages of this site and is gone when the tab closes. It is not a cookie, so it
// is never attached to a request; there is no server here to receive it and no
// analytics to correlate it with. That is also why there is no "remember me
// forever" option -- a permanent record would be more data about you than this
// site has any business keeping.
//
// # Why it is a real gate and not a banner
//
// The waiver's section 4 is the part that matters: it says plainly that this
// software hides *who said it*, not *what was said*. Someone who assumes the
// opposite could send a recording believing its contents are protected. A
// dismissible strip at the bottom of the page does not carry that.
//
// The page underneath is inert while the dialog is open -- focus is trapped, and
// the content is hidden from assistive technology -- so the gate cannot be
// stepped around by tabbing past it.
//
// In plain words
//
// This is the notice you see the first time you open the site: the licence,
// what this project does not promise, and the fact that it was built with AI
// assistance.
//
// It remembers that you read it for as long as the tab is open, and forgets
// when you close it. Nothing about that is sent anywhere. There is no
// "remember me forever" option because keeping a permanent note about you
// would be more than a privacy tool's website has any business keeping.

(function () {
  "use strict";

  var KEY = "veilvoice-accepted-v1";

  function accepted() {
    try { return sessionStorage.getItem(KEY) === "yes"; } catch (e) { return false; }
  }

  function remember() {
    try { sessionStorage.setItem(KEY, "yes"); } catch (e) { /* private mode */ }
  }

  function build() {
    var overlay = document.createElement("div");
    overlay.className = "legal-overlay";
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-modal", "true");
    overlay.setAttribute("aria-labelledby", "legal-title");

    overlay.innerHTML = [
      '<div class="legal-box">',
      '  <h2 id="legal-title">BEFORE YOU USE THIS</h2>',

      // Deliberately not the AI notice: the first thing a reader sees is the
      // one misunderstanding that could actually harm them.
      '  <p>VeilVoice destroys the <b>biometric voiceprint</b> in a recording --',
      '  pitch, formants, timbre, and the melody of an accent -- so the speaker',
      '  cannot be identified or reconstructed. It is free software under the',
      '  <b>GNU General Public License v3 or later</b>, and it is provided with',
      '  non-commercially -- and it is provided with',
      '  <b>absolutely no warranty</b>.</p>',

      '  <p class="legal-warn"><b>It does not hide what you said.</b>',
      '  Intelligibility is preserved on purpose -- the words remain in the output',
      '  and can be transcribed. If the message itself is sensitive, encrypt it.</p>',

      '  <p>This project was developed with <b>AI assistance (Claude, by',
      '  Anthropic)</b> and has been reviewed and audited by <b>tilas01</b>. That',
      '  is a maintainer audit: no external firm or independent researcher has',
      '  reviewed this code. It is disclosed so you can judge for yourself how',
      '  much to verify before relying on it -- the whole project is published',
      '  under the GPL precisely so that you can read it.</p>',

      '  <details>',
      '    <summary>The rest of the terms, in short</summary>',
      '    <ul>',
      '      <li><b>You may</b> run it for any purpose, study it, change it,',
      '      and redistribute it -- including commercially. There is no',
      '      NonCommercial clause.</li>',
      '      <li><b>You must</b> pass on the source, keep the licence notices,',
      '      state your changes, and license derivatives under the GPL too.</li>',
      '      <li><b>No warranty, no liability.</b> The author disclaims all',
      '      liability for data loss, for a key or passphrase you destroy, and',
      '      for consequences of being identified despite using this.</li>',
      '      <li><b>Limits.</b> It does not remove a strong accent entirely, does',
      '      not sanitise background audio, and does not help against an attacker',
      '      already running code on your machine.</li>',
      '      <li><b>Destructive features are irreversible by design</b> and are',
      '      gated behind an explicit confirmation.</li>',
      '      <li><b>Privacy.</b> This site sets no cookies, runs no analytics and',
      '      loads nothing from a third party. Your choices stay in your browser.</li>',
      '    </ul>',
      '    <p>Full texts: ',
      '      <a href="user-agreements/LEGAL-WAIVER.txt">LEGAL-WAIVER.txt</a>, ',
      '      <a href="user-agreements/LICENCE-PLAIN-ENGLISH.txt">the licence in plain English</a>, ',
      '      <a href="user-agreements/LICENSE.txt">LICENSE.txt</a>.</p>',
      '  </details>',

      '  <label class="legal-check">',
      '    <input type="checkbox" id="legal-waiver">',
      '    <span>I have read and understood the disclaimer and liability waiver,',
      '    including what this software does <b>not</b> do.</span>',
      '  </label>',
      '  <label class="legal-check">',
      '    <input type="checkbox" id="legal-licence">',
      '    <span>I have read the licence (GPL-3.0-or-later) and will comply with it.</span>',
      '  </label>',

      // Stated here as well as in the page's <noscript>, because the two
      // reach different readers: this dialog is drawn by script, so somebody
      // with JavaScript off never sees a word of it.
      '  <p>There are <b>two editions</b> of this site. This one runs scripts.',
      '  The <b>JavaScript</b> switch in the header serves you the other:',
      '  <b>HTML and CSS only</b>, with no script running at all -- including a',
      "  complete search index your browser's own find-in-page can search.",
      '  The switch changes which edition you are sent, not any setting in your',
      '  browser; if you turn JavaScript off yourself it shows <b>off</b> and',
      '  locks, because a page cannot turn scripts back on.</p>',

      '  <p class="legal-fine">Using this website, the repository, the released',
      '  binaries or any output they produce constitutes your binding agreement',
      '  to these terms in full.</p>',

      '  <button class="btn primary" id="legal-go" disabled>continue</button>',
      '</div>'
    ].join("");

    return overlay;
  }

  function show() {
    var overlay = build();
    document.body.appendChild(overlay);
    document.body.classList.add("legal-locked");

    var main = document.querySelector("main");
    var header = document.querySelector("header.top");
    [main, header].forEach(function (el) {
      if (el) { el.setAttribute("aria-hidden", "true"); }
    });

    var waiver = overlay.querySelector("#legal-waiver");
    var licence = overlay.querySelector("#legal-licence");
    var go = overlay.querySelector("#legal-go");

    function sync() { go.disabled = !(waiver.checked && licence.checked); }
    waiver.addEventListener("change", sync);
    licence.addEventListener("change", sync);

    go.addEventListener("click", function () {
      remember();
      overlay.remove();
      document.body.classList.remove("legal-locked");
      [main, header].forEach(function (el) {
        if (el) { el.removeAttribute("aria-hidden"); }
      });
    });

    // Keep focus inside the dialog: the page behind it is not usable yet, and
    // tabbing into it would be a way around the gate.
    overlay.addEventListener("keydown", function (event) {
      if (event.key !== "Tab") { return; }
      var focusable = overlay.querySelectorAll(
        'a[href], button:not([disabled]), input, summary, [tabindex]:not([tabindex="-1"])'
      );
      if (!focusable.length) { return; }
      var first = focusable[0];
      var last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    });

    waiver.focus();
  }

  document.addEventListener("DOMContentLoaded", function () {
    if (!accepted()) { show(); }
  });
})();
