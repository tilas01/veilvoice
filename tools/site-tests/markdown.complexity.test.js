// SPDX-License-Identifier: GPL-3.0-or-later
//
// The renderer must stay *linear* in the size of its input.
//
// Separate from both the correctness and the hostile-input suites, because
// neither could see this class of bug: the output was perfectly correct and
// contained nothing dangerous. It just took eight seconds to produce.
//
// That matters here specifically. `repo.js` fetches README.md over the network
// and renders it on the main thread, so a document that makes the renderer
// quadratic is a frozen tab -- a denial of service against the reader, from
// text this page went and asked for. Two separate quadratics were measured
// during the audit and both are fixed; this suite is what stops a third
// arriving unnoticed.
//
// # Why a ratio rather than a stopwatch
//
// Asserting "renders in under N milliseconds" fails on a slow or busy machine,
// which teaches people to ignore the suite. What is actually being checked is
// the *shape* of the curve: four times the input must not take sixteen times
// the work. Each case is therefore run at two sizes and the ratio compared
// against a generous allowance -- linear predicts 4, quadratic predicts 16, and
// the allowance sits between them at 12. A small absolute ceiling is checked
// too, since a ratio alone would pass something uniformly slow.
//
// # Why each measurement is a separate process
//
// A regular-expression match is synchronous and cannot be interrupted. Once the
// engine starts backtracking, no timer, signal or `await` gets control back --
// so a suite that measured in-process would **hang** rather than fail when a
// quadratic reappeared. That was not theoretical: reverting the fixes and
// running an earlier, in-process version of this file produced no output at all
// for over fifteen minutes, where the fixed renderer finishes the same work in
// about a second. A hung CI job looks like an infrastructure problem and gets
// retried; a failing one gets read. Each measurement is therefore spawned and
// killed from outside, so catastrophe arrives as a reportable timeout.

"use strict";

const { execFileSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const PROBE = path.join(__dirname, "complexity-probe.js");
const { SHAPES } = require("./complexity-probe.js");

const ROOT = path.resolve(__dirname, "..", "..");
const sandbox = { window: {} };
vm.createContext(sandbox);
vm.runInContext(fs.readFileSync(path.join(ROOT, "website", "js", "markdown.js"), "utf8"), sandbox);
const MD = sandbox.window.MD;

const SMALL = 8000;
const LARGE = 32000; // four times SMALL
const RATIO_ALLOWANCE = 12; // linear predicts 4, quadratic 16
const CEILING_MS = 4000;
const PROBE_TIMEOUT_MS = 30000;

/** Render one shape at one size in a child process. `null` means it timed out. */
function measure(shape, size) {
  try {
    const out = execFileSync(process.execPath, [PROBE, shape, String(size)], {
      timeout: PROBE_TIMEOUT_MS,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"]
    });
    return Number(out);
  } catch (e) {
    // `killed` is a timeout; anything else is a genuine crash in the probe.
    if (e.killed || e.code === "ETIMEDOUT") { return null; }
    throw new Error("probe failed for " + shape + ": " + (e.stderr || e.message));
  }
}

function run() {
  let failures = 0;

  for (const shape of Object.keys(SHAPES)) {
    const small = measure(shape, SMALL);
    if (small === null) {
      console.log(`FAIL ${shape}: ${SMALL} characters did not finish in ${PROBE_TIMEOUT_MS} ms`);
      failures++;
      continue;
    }
    const large = measure(shape, LARGE);
    if (large === null) {
      console.log(
        `FAIL ${shape}: ${LARGE} characters did not finish in ${PROBE_TIMEOUT_MS} ms ` +
          `(${SMALL} took ${small.toFixed(1)} ms) -- this is worse than quadratic`
      );
      failures++;
      continue;
    }

    // A floor on the denominator: dividing by a 0.02 ms measurement gives a
    // meaningless ratio and a flaky test.
    const ratio = large / Math.max(small, 0.5);

    if (large > CEILING_MS) {
      console.log(`FAIL ${shape}: ${LARGE} characters took ${large.toFixed(0)} ms`);
      failures++;
    } else if (ratio > RATIO_ALLOWANCE) {
      console.log(
        `FAIL ${shape}: 4x the input took ${ratio.toFixed(1)}x the time ` +
          `(${small.toFixed(1)} ms -> ${large.toFixed(1)} ms) -- this looks quadratic`
      );
      failures++;
    } else {
      console.log(`ok   ${shape} (${small.toFixed(1)} ms -> ${large.toFixed(1)} ms)`);
    }
  }

  // --- recursion depth -----------------------------------------------------
  //
  // A blockquote strips one `>` and calls `render` again, so the *document*
  // chose the recursion depth. Five thousand `>` characters overflowed the
  // stack and threw a RangeError -- and because `repo.js` reports any rejection
  // from the README fetch as "could not reach api.github.com", the reader was
  // given a confident, wrong explanation for a page that had loaded fine and
  // then broken while rendering.
  for (const depth of [100, 5000, 50000]) {
    try {
      const out = MD.render(">".repeat(depth) + " hello");
      if (typeof out !== "string" || out.indexOf("hello") === -1) {
        console.log(`FAIL nesting ${depth}: the quoted text did not survive`);
        failures++;
      } else {
        console.log(`ok   ${depth} nested blockquotes render without overflowing`);
      }
    } catch (e) {
      console.log(`FAIL nesting ${depth}: threw ${e.constructor.name}: ${e.message}`);
      failures++;
    }
  }

  // --- the fence language must not reach Object.prototype -------------------
  //
  // The info string is matched with `\w*`, and both `constructor` and
  // `__proto__` are `\w*`. On a plain object literal, `KEYWORDS[lang]` resolved
  // through the prototype chain to `Object` and to `Object.prototype`. Neither
  // did any harm as the code stood, which is exactly the point: that is a
  // description of a bug that has not gone off, not of a safe lookup.
  const plain = MD.render("```\nlet x = 1;\n```");
  for (const lang of ["constructor", "__proto__", "valueOf", "hasOwnProperty", "toString"]) {
    let out;
    try {
      out = MD.render("```" + lang + "\nlet x = 1;\n```");
    } catch (e) {
      console.log(`FAIL fence language ${lang}: threw ${e.message}`);
      failures++;
      continue;
    }
    if (out !== plain) {
      console.log(`FAIL fence language ${lang}: highlighted unlike an unknown language`);
      failures++;
    } else {
      console.log(`ok   a fence language of "${lang}" reaches no prototype property`);
    }
  }

  // --- the bounds stay generous enough for real documents -------------------
  //
  // The repetition bounds that make the link patterns linear also decide what
  // still renders as a link. Tightened too far they would refuse ordinary
  // Markdown, which is F-8 all over again: safe, wrong, and quietly wrong.
  const realistic = [
    "[the whitepaper](docs/WHITEPAPER.md)",
    "[a link](https://tilas01.github.io/veilvoice/#verify)",
    "![banner](assets/banner.png)",
    '[titled](https://example.org/a "with a title")',
    "[" + "long label ".repeat(20) + "](docs/AUDIT.md)",
    "[deep](a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/r/s/t/u/v/w/x/y/z.md)"
  ];
  for (const source of realistic) {
    const html = MD.render(source);
    if (html.indexOf("<a ") === -1 && html.indexOf("<img ") === -1) {
      console.log(`FAIL ordinary Markdown stopped rendering as a link: ${source.slice(0, 50)}`);
      failures++;
    } else {
      console.log(`ok   still renders: ${source.slice(0, 46)}`);
    }
  }

  return failures;
}

module.exports = { name: "markdown renderer, complexity and recursion", run };
