// SPDX-License-Identifier: GPL-3.0-or-later
//
// The front-page banner under reduced motion.
//
// The global reduced-motion rule froze the banner for every reader whose
// system asks for less movement, and frozen it looked broken. The fix keeps a
// slow sweep running for them, with a pause button, and changes nothing for
// anyone else. These tests hold all three halves of that: the stylesheet keeps
// the calmer motion and hides the button by default, the page carries the
// button outside the aria-hidden banner, and the script toggles the pause.

"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ROOT = path.resolve(__dirname, "..", "..");
const read = (p) => fs.readFileSync(path.join(ROOT, p), "utf8");

/** The body of the first `@media (prefers-reduced-motion: reduce)` block holding `needle`. */
function reducedBlockWith(css, needle) {
  const head = "@media (prefers-reduced-motion: reduce) {";
  let at = css.indexOf(head);
  while (at !== -1) {
    let depth = 0;
    for (let i = at + head.length - 1; i < css.length; i++) {
      if (css[i] === "{") { depth++; }
      if (css[i] === "}" && --depth === 0) {
        const body = css.slice(at, i + 1);
        if (body.includes(needle)) { return body; }
        break;
      }
    }
    at = css.indexOf(head, at + 1);
  }
  return null;
}

/** Run banner.js against a stub page and return the button and banner. */
function page({ withButton = true } = {}) {
  const classes = new Set(["bn"]);
  const attrs = {};
  let onClick = null;
  const banner = {
    classList: {
      toggle: (c, on) => (on ? classes.add(c) : classes.delete(c)),
      contains: (c) => classes.has(c)
    }
  };
  const button = {
    textContent: "Pause the banner",
    setAttribute: (k, v) => { attrs[k] = v; },
    addEventListener: (ev, fn) => { if (ev === "click") { onClick = fn; } }
  };
  let ready = null;
  const sandbox = {
    document: {
      addEventListener: (ev, fn) => { if (ev === "DOMContentLoaded") { ready = fn; } },
      querySelector: (s) => (s === ".bn" ? banner : withButton && s === ".bn-pause" ? button : null)
    }
  };
  vm.createContext(sandbox);
  vm.runInContext(read("website/js/banner.js"), sandbox);
  ready();
  return { banner, button, attrs, click: () => onClick && onClick() };
}

function run() {
  let fails = 0;
  const check = (name, ok) => {
    console.log((ok ? "ok   " : "FAIL ") + name);
    if (!ok) { fails++; }
  };

  const css = read("website/css/main.css");
  const block = reducedBlockWith(css, ".bn .bn-sweep");
  check("reduced motion keeps the banner sweep and rule running", !!block
    && /animation-iteration-count:\s*infinite\s*!important/.test(block)
    && /\.bn \.bn-rule/.test(block));
  check("the calmer sweep is slower than the normal one", !!block
    && /animation-duration:\s*24s/.test(block) && /bn-sweep 9s linear infinite/.test(css));
  check("the blurred word copy is not exempted from reduced motion", !!block && !block.includes("bn-word"));
  check("the pause button is hidden unless motion is reduced",
    /\.bn-pause\s*\{[^}]*display:\s*none/.test(css) && !!block && block.includes(".bn-pause { display: inline-block; }"));

  const html = read("website/index.html");
  const bannerEnd = html.indexOf('<span class="bn-rule"></span>');
  const buttonAt = html.indexOf('class="bn-pause"');
  const closeAfterRule = html.indexOf("</div>", bannerEnd);
  check("the page loads banner.js", html.includes('<script src="js/banner.js" defer></script>'));
  check("the button sits outside the aria-hidden banner", buttonAt > closeAfterRule && closeAfterRule > 0);

  const p = page();
  p.click();
  check("pressing pauses the banner", p.banner.classList.contains("bn-paused") && p.attrs["aria-pressed"] === "true"
    && p.button.textContent === "Play the banner");
  p.click();
  check("pressing again plays it", !p.banner.classList.contains("bn-paused") && p.attrs["aria-pressed"] === "false"
    && p.button.textContent === "Pause the banner");

  let threw = false;
  try { page({ withButton: false }); } catch (e) { threw = true; }
  check("a page without the button is a no-op", !threw);

  return fails;
}

module.exports = { run, name: "banner under reduced motion" };

if (require.main === module) { process.exit(run() ? 1 : 0); }
