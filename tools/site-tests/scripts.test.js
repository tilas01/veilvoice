// SPDX-License-Identifier: GPL-3.0-or-later
//
// A page that carries a feature's markup loads the feature's code.
//
// # The defect this exists to stop coming back
//
// `website/verify.html` is the page this project points people to when it
// tells them to check a download before running it. It had the drop zone, the
// expected-hash field, the progress bar and the verdict line, all of it, and
// it did not load `js/verify.js`. Dropping a file on it did nothing at all:
// the digest line sat at "no file hashed yet" for ever, on the page whose
// entire subject is proving a download is genuine (finding F-111).
//
// The cause was in `tools/site/split.py`, which generates the section pages
// out of `index.html`. It copied the head, and the six scripts `index.html`
// loads in its head came along inside it. The three it loads at the *end of
// its body* did not. `shell()` even collected all nine into a `scripts` key,
// and nothing ever read it -- a dead variable is why nobody noticed.
//
// # Why this reads the scripts instead of keeping a list
//
// A table here saying "verify.html needs verify.js" would be a second copy of
// a fact, which is the shape of half the findings in this repository. So each
// module is read for the ids it will not start without, and each page for the
// ids it has, and the two are compared. A section that grows a feature is
// covered the day it grows it, by nobody doing anything.
//
// # Activation ids, not every id
//
// Which ids count is the whole difficulty. `repo.js` touches `#asset-list`,
// and the download page has an `#asset-list` -- but `repo.js` returns
// immediately unless `#load-repo` is on the page, and that button lives on
// the front page only. So the download page holds that markup legitimately
// and needs no script, while the verify page holds `#drop` and `#file`, which
// `verify.js` does demand, and needed one badly.
//
// Every module here is written the same way: a `DOMContentLoaded` handler
// looks its elements up and returns early if the ones it cannot work without
// are missing. That guard is the answer, and it is read out of the module
// rather than restated here. Counting ids gets the download page wrong;
// reading the guard gets both right.
//
// # The second check
//
// Splitting a section onto its own page also falsifies the words "above" and
// "below" in it. "The verifier below" was true on the front page and became a
// link to a different page. A claim about where something sits on this page
// cannot be made about a link that leaves it, so that pairing is refused.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");

function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), "utf8");
}

function htmlIn(dir) {
  const full = path.join(ROOT, dir);
  if (!fs.existsSync(full)) { return []; }
  return fs.readdirSync(full)
    .filter((name) => name.endsWith(".html"))
    .map((name) => path.posix.join(dir, name))
    .sort();
}

/** `function byId(id) { return document.getElementById(id); }` and its callers. */
const ALIAS = /function (\w+)\(\w+\)\s*\{\s*return document\.getElementById\(\w+\);\s*\}/g;
const LOOKUP = /getElementById\("([^"]+)"\)|querySelector(?:All)?\("#([^"]+)"\)/g;
const BOUND = /var (\w+) = document\.getElementById\("([^"]+)"\)/g;
const GUARD = /if \(([^)]*!\w+[^)]*)\)\s*\{\s*return;\s*\}/g;
const HANDLER = 'document.addEventListener("DOMContentLoaded"';

function idsUsed(js) {
  const out = new Set();
  for (const m of js.matchAll(LOOKUP)) { out.add(m[1] || m[2]); }
  for (const m of js.matchAll(ALIAS)) {
    const call = new RegExp(`\\b${m[1]}\\("([^"]+)"\\)`, "g");
    for (const hit of js.matchAll(call)) { out.add(hit[1]); }
  }
  return out;
}

/**
 * The ids a module will not start without, and how to read them.
 *
 * `all` means the module needs every one of them, which is what an early
 * return on a missing element says. `any` is the answer when there is no
 * guard to read: cautious rather than precise, so a module this cannot
 * understand is demanded wherever its markup appears rather than nowhere.
 */
function activationIds(js) {
  const start = js.indexOf(HANDLER);
  if (start < 0) { return { ids: idsUsed(js), how: "any" }; }
  const body = js.slice(start);
  for (const guard of body.matchAll(GUARD)) {
    const bound = new Map();
    for (const m of body.slice(0, guard.index).matchAll(BOUND)) {
      bound.set(m[1], m[2]);
    }
    const names = [...guard[1].matchAll(/!(\w+)/g)].map((m) => m[1]);
    // A guard naming something the handler did not look up is testing a
    // parsed value or a browser capability, and says nothing about which
    // page the module belongs on.
    if (names.length && names.every((n) => bound.has(n))) {
      return { ids: new Set(names.map((n) => bound.get(n))), how: "all" };
    }
  }
  return { ids: idsUsed(js), how: "any" };
}

function modules() {
  const dir = path.join(ROOT, "website", "js");
  const out = new Map();
  for (const name of fs.readdirSync(dir).sort()) {
    if (!name.endsWith(".js")) { continue; }
    out.set(name, activationIds(read(path.posix.join("website/js", name))));
  }
  return out;
}

/** Text with comments and script bodies removed, so prose is read as prose. */
function prose(html) {
  return html
    .replace(/<!--[\s\S]*?-->/g, " ")
    .replace(/<script[\s\S]*?<\/script>/g, " ");
}

function run() {
  let failures = 0;
  const fail = (why) => { failures += 1; console.log(`FAIL ${why}`); };
  const pass = (what) => console.log(`  ok  ${what}`);

  const known = modules();
  const gated = [...known].filter(([, m]) => m.how === "all" && m.ids.size);
  if (gated.length === 0) {
    fail("not one module in website/js was found to guard its own start, " +
         "which is not credible: the guard pattern has changed and this " +
         "suite is reading nothing");
    return failures;
  }

  // --- every page loads the code for the markup it carries -----------------
  let demanded = 0;
  for (const page of htmlIn("website")) {
    const html = read(page);
    const loaded = new Set(
      [...html.matchAll(/<script src="js\/([^"]+)"/g)].map((m) => m[1]));
    const present = new Set([...html.matchAll(/id="([^"]+)"/g)].map((m) => m[1]));

    for (const [name, { ids, how }] of known) {
      if (!ids.size) { continue; }
      const hit = [...ids].filter((id) => present.has(id));
      const needed = how === "all" ? hit.length === ids.size : hit.length > 0;
      if (!needed) { continue; }
      demanded += 1;
      if (loaded.has(name)) {
        pass(`${page} carries ${name}'s markup and loads it`);
      } else {
        fail(`${page} has ${hit.map((i) => "#" + i).join(", ")}, which is ` +
             `what js/${name} works on, and does not load js/${name}. The ` +
             "markup is on the page and the code behind it is not, so the " +
             "feature is furniture: it looks present and does nothing.");
      }
    }
  }

  if (demanded === 0) {
    fail("no page was found to carry any module's markup, so this suite " +
         "passed without comparing a single page to a single module");
  }

  // --- "below" is a claim about this page ----------------------------------
  //
  // Anchor text, or the words just before the link, describing a link that
  // goes to another page as being above or below on this one.
  const CROSS = /<a\b[^>]*href="(?!#)([^"]*\.html)#?[^"]*"[^>]*>([^<]*)<\/a>/g;
  let spatial = 0;
  for (const page of htmlIn("website")) {
    const text = prose(read(page));
    for (const m of text.matchAll(CROSS)) {
      const before = text.slice(Math.max(0, m.index - 60), m.index);
      const where = `${m[2]} ${before}`;
      if (!/\b(above|below)\b/.test(m[2]) &&
          !/\b(above|below)\b[^.]{0,20}$/.test(before)) { continue; }
      spatial += 1;
      fail(`${page} describes a link to ${m[1]} as "${
        /\babove\b/.test(where) ? "above" : "below"}" (${m[2].trim()}). ` +
        "Above and below are claims about where something sits on this " +
        "page, and that link leaves it.");
    }
  }
  if (spatial === 0) { pass("no cross-page link is described as above or below"); }

  // --- the no-JavaScript pages are exactly that ----------------------------
  const nojs = htmlIn("website/nojs");
  if (nojs.length === 0) {
    fail("website/nojs has no pages, so the no-JavaScript variant of the " +
         "site has gone missing");
  }
  for (const page of nojs) {
    if (/<script\b/.test(read(page))) {
      fail(`${page} is in the no-JavaScript variant of the site and loads a ` +
           "script, which is the one thing those pages exist not to do");
    } else {
      pass(`${page} loads no scripts`);
    }
  }

  return failures;
}

module.exports = { run, name: "pages load the code for the markup they carry" };
