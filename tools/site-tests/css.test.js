// SPDX-License-Identifier: GPL-3.0-or-later
//
// Cross-engine invariants for the stylesheets.
//
// The site is served to whatever browser somebody has, and this project has no
// build step -- no autoprefixer, no PostCSS, no browserslist. That is a
// deliberate choice (what is in `website/` is exactly what GitHub Pages serves,
// which is what makes "read the file yourself" a real invitation), and the
// price of it is that nothing adds vendor prefixes or fallbacks on your behalf.
// This suite is the thing that notices when one is missing.
//
// Every rule below corresponds to a real degradation found by reading the CSS
// against what each engine actually shipped, not to a general preference:
//
//   - `backdrop-filter` was unprefixed only. Safari did not support the
//     unprefixed property until version 18 (late 2024), so on every iPhone
//     running iOS 17 or earlier the translucent header had no blur at all.
//
//   - `color-mix()` arrived in Chrome 111, Safari 16.2 and Firefox 113, all in
//     2023. An engine older than that discards the whole declaration -- and
//     three of the four uses had no preceding fallback, so the element was left
//     with *no background*. The worst was the legal gate: a fixed overlay shown
//     with `body { overflow: hidden }`, which without its background is an
//     invisible modal that silently stops the page scrolling.
//
//   - `:focus-visible` arrived in Safari 15.4. An unsupported pseudo-class
//     makes the entire selector list invalid, so a single rule combining
//     `:focus-visible` selectors left older Safari with no focus ring anywhere
//     -- the page unnavigable by keyboard.
//
//   - `color-scheme` is the only way to reach the browser's *native* controls.
//     Without it the theme `<select>`'s dropdown and the verifier's
//     `<progress>` bar render light on a near-black page.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const CSS_DIR = path.join(ROOT, "website", "css");

function read(name) {
  return fs.readFileSync(path.join(CSS_DIR, name), "utf8");
}

/** Strip comments, so prose about a property is never mistaken for a use. */
function code(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, "");
}

/**
 * Split a stylesheet into declaration blocks, keeping the selector.
 * Crude, and sufficient: this stylesheet is hand-written and shallow.
 */
function blocks(css) {
  const found = [];
  const pattern = /([^{}]+)\{([^{}]*)\}/g;
  let match;
  while ((match = pattern.exec(css)) !== null) {
    found.push({ selector: match[1].trim().replace(/\s+/g, " "), body: match[2] });
  }
  return found;
}

function run() {
  let failures = 0;
  const fail = (message) => { console.log("FAIL " + message); failures++; };
  const pass = (message) => console.log("ok   " + message);

  const files = fs.readdirSync(CSS_DIR).filter((f) => f.endsWith(".css"));

  // --- 1. backdrop-filter must always be paired with the WebKit prefix ------
  let backdropUses = 0;
  for (const file of files) {
    for (const block of blocks(code(read(file)))) {
      const unprefixed = /(^|[;\s])backdrop-filter\s*:/.test(block.body);
      const prefixed = /-webkit-backdrop-filter\s*:/.test(block.body);
      if (unprefixed) {
        backdropUses++;
        if (!prefixed) {
          fail(
            `${file}: \`${block.selector}\` uses backdrop-filter without ` +
              "-webkit-backdrop-filter, so it does nothing on Safari before 18"
          );
        }
      }
      if (prefixed && !unprefixed) {
        fail(`${file}: \`${block.selector}\` has only the prefixed backdrop-filter`);
      }
    }
  }
  if (backdropUses > 0) {
    pass(`all ${backdropUses} backdrop-filter uses carry the -webkit- prefix`);
  }

  // --- 2. every color-mix() must have a plain fallback before it ------------
  let mixUses = 0;
  for (const file of files) {
    for (const block of blocks(code(read(file)))) {
      // Declarations in order, so "before" is meaningful.
      const declarations = block.body
        .split(";")
        .map((d) => d.trim())
        .filter(Boolean);
      declarations.forEach((declaration, index) => {
        if (!/color-mix\s*\(/.test(declaration)) { return; }
        mixUses++;
        const property = declaration.split(":")[0].trim();
        const hasFallback = declarations
          .slice(0, index)
          .some((earlier) =>
            earlier.split(":")[0].trim() === property && !/color-mix\s*\(/.test(earlier)
          );
        if (!hasFallback) {
          fail(
            `${file}: \`${block.selector}\` sets ${property} with color-mix() and ` +
              "no earlier plain fallback -- engines before 2023 drop it entirely"
          );
        }
      });
    }
  }
  if (mixUses > 0) {
    pass(`all ${mixUses} color-mix() declarations have a plain fallback first`);
  }

  // --- 3. :focus-visible must not be the only focus rule -------------------
  //
  // And it must not share a selector list with plain `:focus`, since one
  // unsupported pseudo-class invalidates the whole list.
  for (const file of files) {
    const css = code(read(file));
    const usesFocusVisible = /:focus-visible/.test(css);
    if (!usesFocusVisible) { continue; }
    for (const block of blocks(css)) {
      if (!/:focus-visible/.test(block.selector)) { continue; }
      // `:focus:not(:focus-visible)` is the standard progressive-enhancement
      // form and is *meant* to be dropped by an engine that does not know
      // `:focus-visible` -- being dropped is what leaves the plain `:focus`
      // ring in place. Only a list that mixes a bare `:focus` selector with a
      // `:focus-visible` one is a problem, because then the fallback and the
      // enhancement fall together.
      const parts = block.selector.split(",").map((s) => s.trim());
      const bare = parts.filter(
        (s) => /:focus(?![-a-z])/.test(s) && !/:not\(\s*:focus-visible\s*\)/.test(s)
      );
      const enhanced = parts.filter((s) => /:focus-visible/.test(s) && !/:not\(/.test(s));
      if (bare.length && enhanced.length) {
        fail(
          `${file}: \`${block.selector}\` mixes a bare :focus selector with a ` +
            ":focus-visible one -- an engine that knows neither drops both"
        );
      }
    }
    const hasPlainFocusRule = blocks(css).some(
      (b) => /:focus(?![-a-z])/.test(b.selector) && /outline\s*:/.test(b.body) &&
             !/outline\s*:\s*none/.test(b.body)
    );
    if (!hasPlainFocusRule) {
      fail(
        `${file}: :focus-visible is used with no plain :focus fallback, so Safari ` +
          "before 15.4 shows no focus ring at all"
      );
    } else {
      pass(`${file}: :focus-visible has a plain :focus fallback`);
    }
  }

  // --- 4. every theme declares a color-scheme ------------------------------
  const themes = code(read("themes.css"));
  const themeBlocks = blocks(themes).filter((b) => /--bg\s*:/.test(b.body));
  if (themeBlocks.length < 5) {
    fail(`only ${themeBlocks.length} theme blocks found -- the parser is wrong`);
  }
  for (const block of themeBlocks) {
    if (!/color-scheme\s*:\s*(light|dark)/.test(block.body)) {
      fail(
        `themes.css: \`${block.selector}\` defines colours but no color-scheme, so ` +
          "native controls will not match it"
      );
      continue;
    }
    // And the declared scheme must agree with the background it sits beside,
    // or the native controls are wrong in the other direction.
    const declared = /color-scheme\s*:\s*(light|dark)/.exec(block.body)[1];
    const bg = /--bg\s*:\s*#([0-9a-fA-F]{6})/.exec(block.body);
    if (bg) {
      const v = bg[1];
      const luminance =
        parseInt(v.slice(0, 2), 16) * 0.299 +
        parseInt(v.slice(2, 4), 16) * 0.587 +
        parseInt(v.slice(4, 6), 16) * 0.114;
      const expected = luminance > 127 ? "light" : "dark";
      if (declared !== expected) {
        fail(
          `themes.css: \`${block.selector}\` says color-scheme: ${declared} but its ` +
            `background #${v} is ${expected}`
        );
      }
    }
  }
  pass(`all ${themeBlocks.length} themes declare a color-scheme matching their background`);

  // --- 5. no fixed viewport-height units -----------------------------------
  //
  // `100vh` on iOS Safari is the height of the viewport *without* the browser
  // chrome, so a full-height element is taller than the screen and its bottom
  // is unreachable. Not currently used; this keeps it that way.
  //
  // A `vh` value is allowed only as the *fallback* immediately before the same
  // property in `dvh`, which is the documented way to support both.
  let vhProblems = 0;
  for (const file of files) {
    for (const block of blocks(code(read(file)))) {
      const declarations = block.body.split(";").map((d) => d.trim()).filter(Boolean);
      declarations.forEach((declaration, index) => {
        if (!/(?:^|[\s:(])\d+(?:\.\d+)?vh\b/.test(declaration)) { return; }
        const property = declaration.split(":")[0].trim();
        const upgraded = declarations
          .slice(index + 1)
          .some((later) => later.split(":")[0].trim() === property && /dvh\b/.test(later));
        if (!upgraded) {
          vhProblems++;
          fail(
            `${file}: \`${block.selector}\` sets ${property} in vh with no dvh after ` +
              "it -- on iOS Safari vh is not the height you can see, so the bottom " +
              "of the element can be unreachable"
          );
        }
      });
    }
  }
  if (vhProblems === 0) {
    pass("every vh length is followed by a dvh upgrade");
  }

  // --- 6. the mobile header must not be allowed to grow ---------------------
  //
  // Nine navigation links wrapped onto four rows at 375 px and, because the
  // header is sticky, cost 165 px of an 812 px screen at every scroll position.
  // The fix is a single scrolling row; this asserts the parts of it that a
  // future edit could remove without anyone noticing on a desktop.
  const main = code(read("main.css"));
  const mobileHeader = /@media[^{]*max-width:\s*7\d\dpx[^{]*\{([\s\S]*?)\n\}/.exec(main);
  if (!mobileHeader) {
    fail("main.css: no narrow-viewport media query for the header was found");
  } else {
    const body = mobileHeader[1];
    const required = [
      [/nav\.links[\s\S]*?overflow-x:\s*auto/, "the nav must scroll rather than wrap"],
      [/nav\.links[\s\S]*?min-width:\s*0/, "min-width: 0 is what lets a flex item scroll"],
      [/nav\.links[\s\S]*?white-space:\s*nowrap/, "links must not break mid-word"]
    ];
    for (const [pattern, why] of required) {
      if (!pattern.test(body)) { fail(`main.css, narrow viewport: ${why}`); }
    }
    pass("the narrow-viewport header keeps its single scrolling nav row");
  }

  // --- 7. tap targets ------------------------------------------------------
  if (!/nav\.links a\s*\{[^}]*min-height:\s*2[4-9]px/.test(main)) {
    fail("main.css: nav links must be at least 24px tall (WCAG 2.5.8)");
  } else {
    pass("nav links declare a 24px minimum tap target");
  }

  return failures;
}

module.exports = { name: "stylesheets, cross-engine invariants", run };
