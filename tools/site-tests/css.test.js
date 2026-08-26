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

  // --- 8. the cycling fact line -------------------------------------------
  //
  // The keyframe percentages are one slot of N, so `--fact-count` in the
  // stylesheet and the number of `.fact` elements in the page must agree.
  // Adding a fact without widening the cycle overlaps two messages and makes
  // both unreadable -- on the strip carrying this project's own claims, which
  // is the neighbourhood finding F-37 lived in.
  const indexHtml = fs.readFileSync(
    path.join(ROOT, "website", "index.html"), "utf8");
  const declared = /--fact-count:\s*(\d+)/.exec(main);
  const present = (indexHtml.match(/class="fact"/g) || []).length;
  if (!declared) {
    fail("main.css: --fact-count is not declared, so nothing pins the cycle");
  } else if (Number(declared[1]) !== present) {
    fail(`the fact line disagrees with itself: main.css says ${declared[1]}, ` +
         `index.html has ${present}`);
  } else if (present < 20) {
    fail(`only ${present} facts; the strip was specified as at least 20`);
  } else {
    pass(`the fact line's ${present} entries match --fact-count`);
  }

  // Every fact needs its own index and colour, or they all animate together.
  const indices = [...indexHtml.matchAll(/class="fact"\s+style="--f:(\d+);--c:([^"]+)"/g)];
  if (indices.length !== present) {
    fail(`${present - indices.length} fact(s) are missing --f or --c`);
  } else {
    const seen = new Set(indices.map(m => m[1]));
    if (seen.size !== present) {
      fail("two facts share an --f index, so they appear at the same moment");
    } else if (indices.some(m => !/^var\(--[a-z0-9-]+\)$/.test(m[2]))) {
      fail("a fact's colour is not a palette token, so it will not follow the theme");
    } else {
      pass("each fact has a distinct slot and a palette colour");
    }
  }

  // The resting state of a `.fact` is invisible, and the global
  // reduced-motion rule collapses animations to 0.01ms -- which would leave
  // the strip permanently blank for a reader who asked for less motion. That
  // needs handling explicitly rather than by the blanket rule.
  const reduced = main.slice(main.indexOf(".facts"));
  if (!/@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{[^}]*\.fact\s*\{[^}]*animation:\s*none/.test(reduced)) {
    fail("main.css: .fact must stop animating under prefers-reduced-motion");
  } else if (!/\.fact:first-child\s*\{[^}]*opacity:\s*1/.test(reduced)) {
    fail("main.css: with motion reduced, one fact must remain visible " +
         "-- otherwise the strip is blank and nobody can tell");
  } else {
    pass("with motion reduced, the cycle stops and the first fact stays");
  }

  // --- 9. the fact strip states numbers, and numbers go stale ------------
  //
  // It said "336 tests" and "47 defects across four audit rounds" while the
  // tree had 354 and 59. Both were true when written. Everything else in this
  // repository that makes a claim is generated and checked; this was the one
  // place claims were hand-typed with nothing watching them.
  //
  // `docs/AUDIT.md` is the authority: it is the document that has to be
  // correct for any of the rest to mean anything.
  const audit = fs.readFileSync(path.join(ROOT, "docs", "AUDIT.md"), "utf8");

  // F-71. This used to compare the front page against docs/AUDIT.md, and it
  // passed for four rounds while both were wrong: the page said 354 tests and
  // "the nine crates", the audit said 354 across 9, and the tree held 890
  // across 19. Two hand-typed copies of a claim agreeing with each other is
  // not a check -- it is the same defect as F-61 and F-63, in a third place.
  //
  // Both are now compared against docs/MEASURED.md, which is generated by
  // running the tests and reading Cargo.toml. Neither number is typed by
  // anybody, so neither can drift.
  const measuredPath = path.join(ROOT, "docs", "MEASURED.md");
  if (!fs.existsSync(measuredPath)) {
    fail("docs/MEASURED.md is missing; run python tools/measured/generate.py");
  } else {
    const measured = fs.readFileSync(measuredPath, "utf8");
    const number = (label) => {
      const row = new RegExp("\\|\\s*" + label + "[^|]*\\|\\s*(\\d+)\\s*\\|").exec(measured);
      return row ? row[1] : null;
    };

    const trueTests = number("Tests, measured by running them");
    const trueCrates = number("Crates in the workspace");
    const trueSuites = number("Website suites");

    if (!trueTests || !trueCrates || !trueSuites) {
      fail("docs/MEASURED.md does not carry the three numbers it should");
    } else {
      // Every place each number is claimed, against the measurement.
      const claims = [
        ["the front page's test count", /(\d+) tests, and \d+ more suites/.exec(indexHtml), trueTests],
        ["the front page's suite count", /\d+ tests, and (\d+) more suites/.exec(indexHtml), trueSuites],
        ["the front page's crate count", /in any of the (\d+) crates/.exec(indexHtml), trueCrates],
        // Anchored on the "Test suite" row, not on the first loose match in the
        // document. The audit *discusses* past numbers -- F-71's own write-up
        // quotes "354 tests across 9 crates" -- and a check that takes the
        // first match reads the history instead of the claim. It did exactly
        // that on the first run, which is the second time in this file that a
        // regex has found an older number further up the page.
        ["the audit's test count", /\| Test suite \| (\d+) tests across \d+ crates/.exec(audit), trueTests],
        ["the audit's crate count", /\| Test suite \| \d+ tests across (\d+) crates/.exec(audit), trueCrates],
        ["the audit's suite count", /\| Test suite \|[^|]*?and (\d+) site-test suites/.exec(audit), trueSuites]
      ];

      let drifted = 0;
      for (const [what, found, truth] of claims) {
        if (!found) {
          fail(`${what} could not be found, so nothing is watching it`);
          drifted += 1;
        } else if (found[1] !== truth) {
          fail(`${what} says ${found[1]}, the tree measures ${truth}`);
          drifted += 1;
        }
      }
      if (drifted === 0) {
        pass(`every stated count matches the tree (${trueTests} tests, ` +
             `${trueCrates} crates, ${trueSuites} suites)`);
      }

      // A spelled-out number cannot be compared, so it is not allowed. "the
      // nine crates" is exactly how this drifted without anything noticing.
      const spelled = /in any of the (nine|ten|eleven|twelve|nineteen|twenty) crates/.exec(indexHtml);
      if (spelled) {
        fail(`the crate count is spelled out ("${spelled[1]}"), which no check can compare`);
      } else {
        pass("counts on the page are digits, so they can be checked");
      }
    }
  }

  // Anchored on the verdict line rather than on "F-1 to F-": section 2.1's
  // heading is also "(F-1 to F-8)", and matching that would compare the page
  // against the count from three rounds ago.
  const auditFindings = /audit rounds \(F-1 to F-(\d+)\)/.exec(audit);
  const pageFindings = /(\d+) defects found and fixed across (\w+) audit rounds/.exec(indexHtml);
  if (!auditFindings || !pageFindings) {
    fail("the defect-count claim could not be found in the audit or on the page");
  } else if (auditFindings[1] !== pageFindings[1]) {
    fail(`the front page claims ${pageFindings[1]} defects, ` +
         `docs/AUDIT.md's findings run to F-${auditFindings[1]}`);
  } else {
    pass(`the front page's defect count (${pageFindings[1]}) matches the audit`);
  }

  // --- 10. tooltips --------------------------------------------------------
  //
  // Three rules, each of which is a way tooltips are routinely got wrong and
  // none of which shows up as a visible fault on the developer's own desktop.
  const tips = main.slice(main.indexOf("[data-tip]"));
  if (main.indexOf("[data-tip]") === -1) {
    fail("main.css: the tooltip styles are gone");
  } else {
    // Keyboard reachable. `:hover` alone means a pointer is required, which
    // is invisible to anybody testing with a mouse in their hand.
    if (!/\[data-tip\]:focus-visible::after/.test(tips)) {
      fail("tooltips must appear on :focus-visible, not only on :hover");
    } else {
      pass("tooltips appear on keyboard focus as well as hover");
    }

    // `visibility` has to be in the transition. Without it the box stays in
    // the accessibility tree and stays hoverable while fully transparent, so a
    // pointer crossing empty space triggers a tooltip that is not visible.
    if (!/transition:[^;]*visibility/.test(tips)) {
      fail("the tooltip transition must include visibility, or an invisible " +
           "box stays hoverable and stays in the accessibility tree");
    } else {
      pass("the tooltip transition covers visibility");
    }

    // There must be an affordance. A hover-only annotation with nothing
    // indicating it exists is a secret, not a tooltip.
    if (!/\[data-tip\]\s*\{[^}]*text-decoration:\s*underline dotted/.test(tips)) {
      fail("[data-tip] needs a visible affordance, or nothing suggests there " +
           "is anything to hover");
    } else {
      pass("tooltips carry a visible affordance");
    }
  }

  // Every tooltip on the pages must also be announced once, and only once:
  // `data-tip` for sighted readers, `aria-label` carrying the same words, and
  // no `title` on the same element -- which some screen readers announce in
  // addition, and some desktops draw as a second box.
  const tipUses = [...indexHtml.matchAll(/<span([^>]*\bdata-tip="([^"]*)"[^>]*)>/g)];
  if (tipUses.length === 0) {
    fail("no tooltip is actually used on the front page");
  } else {
    const problems = [];
    for (const [, attrs, text] of tipUses) {
      if (!/\baria-label="/.test(attrs)) {
        problems.push(`a tooltip has no aria-label: ${text.slice(0, 40)}`);
      }
      if (/\btitle="/.test(attrs)) {
        problems.push(`a tooltip also carries title=, so it is announced twice`);
      }
    }
    if (problems.length) { problems.forEach(fail); }
    else { pass(`${tipUses.length} tooltips are announced exactly once`); }
  }

  // ---- the page must not scroll sideways ----------------------------------
  //
  // Every rule below was written because a viewport was measured with
  // `tools/render/probe.py overflow` and found to scroll horizontally. A phone
  // that scrolls sideways is not a cosmetic complaint: it moves the text out
  // from under the reader on every swipe. None of it is visible from the source
  // -- each looked correct until a number came back -- so each is pinned here.
  const mobile = [
    [/\.wiki-layout\s*>\s*\*\s*\{[^}]*min-width:\s*0/,
     "a grid item defaults to min-width:auto, so the reference column refused " +
     "to be narrower than its widest table (658px) and took the page with it"],
    [/(^|\})\s*table\s*\{[^}]*display:\s*block[^}]*overflow-x:\s*auto/m,
     "a table has to be its own sideways scroller, or a column of code names " +
     "wider than the page drags the whole page along"],
    [/td code,\s*th code\s*\{[^}]*overflow-wrap:\s*anywhere/,
     "break-word does not shrink a table's intrinsic width; anywhere does, " +
     "and without it the items table scrolled inside a desktop column too"],
    [/\bcode\s*\{[^}]*overflow-wrap:\s*break-word/,
     "one identifier can be 385px of unbreakable word in a 300px column"],
    [/pre code\s*\{[^}]*overflow-wrap:\s*normal/,
     "a code block scrolls on purpose; breaking its lines changes what it says"],
    [/\.hero\s*\{[^}]*padding:\s*\d+px\s+[1-9]/,
     "the hero is the one section not inside .wrap, so it needs its own gutter " +
     "or the tagline touches both edges of a phone"],
    [/\.search-page\s*\{\s*padding:\s*\d+px\s+[1-9]/,
     "a padding shorthand on .wrap.search-page replaces .wrap's side padding " +
     "rather than adding to it, which took the gutters away"],
    [/\.diagram\s*\{[^}]*overflow-x:\s*auto/,
     "a drawing wider than its column must scroll inside itself"]
  ];
  for (const [pattern, why] of mobile) {
    if (!pattern.test(main)) fail(`${why} — the rule for it is gone`);
  }
  if (mobile.every(([pattern]) => pattern.test(main))) {
    pass(`${mobile.length} measured horizontal-overflow fixes are still in place`);
  }

  // The tooltip is the subtle one. It is `position: absolute`, anchored to the
  // left of the word it annotates, and `visibility: hidden` still takes part in
  // layout -- so a closed tooltip near the right of a narrow column pushed the
  // front page 82px sideways with nobody hovering anything. The fix pins it to
  // the viewport below 900px, which is where the columns stop narrowing; a
  // query written at the site's usual 760 left a tablet at 768 still 75px over.
  const pinned = main.match(
    /@media \(max-width:\s*(\d+)px\)\s*\{\s*\[data-tip\]::after\s*\{([^}]*)\}/);
  if (!pinned) {
    fail("[data-tip]::after is not pinned to the viewport on a narrow screen, " +
         "so a closed tooltip can push the page sideways");
  } else if (Number(pinned[1]) < 900) {
    fail(`the tooltip is only pinned below ${pinned[1]}px; a tablet at 768 was ` +
         `still 75px over, so this has to reach 900`);
  } else if (!/position:\s*fixed/.test(pinned[2])) {
    fail("the pinned tooltip must be position:fixed — an absolute box still " +
         "counts toward the page's scrollable width");
  } else {
    pass(`tooltips are pinned to the viewport below ${pinned[1]}px`);
  }

  // ---- `inset` needs its longhands, and one place needs them badly --------
  //
  // `inset` arrived in Safari 14.1 (early 2021). An engine older than that
  // treats the declaration as invalid and drops it, and a `position: fixed`
  // element with no offsets sits wherever it fell in the flow at its own
  // content size.
  //
  // For the legal gate that is not a cosmetic failure. It is shown with
  // `body { overflow: hidden }`, so an overlay that does not cover the page
  // leaves a reader unable to scroll with nothing visible stopping them --
  // exactly the shape of the `color-mix` degradation this file already guards.
  {
    const insetRules = [...main.matchAll(/\{[^}]*\}/g)]
      .map((m) => m[0])
      .filter((block) => /(^|[\s;{])inset\s*:/.test(block));
    if (insetRules.length === 0) {
      pass("no bare `inset` to guard");
    } else {
      const bare = insetRules.filter(
        (block) => !(/(^|[\s;{])top\s*:/.test(block) && /(^|[\s;{])left\s*:/.test(block))
      );
      if (bare.length) {
        bare.forEach((block) =>
          fail(
            "`inset` with no longhand fallback (Safari 14.0 and earlier drop " +
              "it): " + block.replace(/\s+/g, " ").slice(0, 70)
          )
        );
      } else {
        pass(`${insetRules.length} \`inset\` rules carry longhand fallbacks`);
      }
    }
  }

  // ---- the page must not describe its own layout by direction -------------
  //
  // The demonstration's caption said "the bars on the left" and "on the
  // right". Below 640 px `.demo-flow` stacks, so on every phone those words
  // named the wrong thing -- and they never meant anything to a reader using a
  // screen reader at any width.
  //
  // The fix is not a second sentence behind a media query. It is to name the
  // thing rather than where it happens to be, which is true in every layout
  // and to every reader. This checks nobody puts the directions back.
  {
    const pages = fs
      .readdirSync(path.join(ROOT, "website"))
      .filter((name) => name.endsWith(".html"));
    const directions = /\b(?:on|to) the (?:left|right)\b|\b(?:left|right)-hand (?:column|side|panel)\b/i;
    const guilty = [];
    for (const name of pages) {
      const html = fs.readFileSync(path.join(ROOT, "website", name), "utf8");
      // Prose only: a `float: left` in an inline style is not a claim about
      // where something is on a phone.
      const prose = html.replace(/<style[\s\S]*?<\/style>/g, "").replace(/<[^>]+>/g, " ");
      const found = prose.match(directions);
      if (found) guilty.push(`${name}: "${found[0]}"`);
    }
    if (guilty.length) {
      guilty.forEach((where) =>
        fail(
          "the page describes its own layout by direction, which is wrong " +
            "wherever it stacks and meaningless to a screen reader: " + where
        )
      );
    } else {
      pass(`${pages.length} pages name things rather than directions`);
    }
  }

  return failures;
}

module.exports = { name: "stylesheets, cross-engine invariants", run };
