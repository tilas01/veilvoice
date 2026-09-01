// SPDX-License-Identifier: GPL-3.0-or-later
//
// Buttons line up with the buttons beside them, and a new feature cannot
// quietly add a row that does not.
//
// # The defect this exists to stop coming back
//
// Buttons that appear only after setup were not aligned with the ones that
// are there from the start. In the desktop application the cause was labels
// padded with trailing spaces to fake a column, which lines nothing up in a
// proportional font; that is guarded in `veilvoice-gui` by a test beside the
// code. On the website the cause was the other half of the same habit:
// `.row`, which holds the download buttons on the front page and the download
// page, declared `display: flex` and said nothing about `align-items`.
//
// A flex container that says nothing gets `stretch`. While every button in a
// row happens to be one line tall that is invisible, and the first button
// whose label wraps on a narrow viewport makes every button beside it taller.
// It is invisible right up until it is on somebody's phone.
//
// # Why this checks the containers and not the buttons
//
// The buttons are fine: `.btn` is an `inline-flex` that centres its own
// contents, and it was fine before this test existed. Alignment between
// siblings is a property of the thing that holds them, so that is what is
// read. A check that measured the buttons would pass while the row that
// arranges them said nothing at all.
//
// # What "holds buttons" means
//
// The pages are read to find out, rather than a list of class names being
// kept here. A list would go stale the first time somebody adds a row, which
// is precisely the case this is for: the point is to catch the *next* feature,
// not to describe the current ones.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");

/** Anything a reader would call a button. */
const BUTTON_CLASS = /\b(btn|demo-btn|demo-mode|demo-tab|demo-close)\b/;

function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), "utf8");
}

function pages() {
  const out = [];
  for (const dir of ["website", "website/nojs"]) {
    const full = path.join(ROOT, dir);
    if (!fs.existsSync(full)) { continue; }
    for (const name of fs.readdirSync(full)) {
      if (name.endsWith(".html")) { out.push(path.posix.join(dir, name)); }
    }
  }
  return out;
}

/**
 * Every class that directly wraps a button, per page.
 *
 * Deliberately crude: the nearest preceding opening tag that carries a class.
 * It is not a parser and does not need to be, because it is looking for
 * containers to ask the stylesheet about, and a wrong guess costs a lookup
 * that finds no flex rule and moves on.
 */
function containersHoldingButtons() {
  const found = new Map();
  const opener = /<(?:div|nav|section|p|header|footer|form)\b[^>]*class="([^"]*)"[^>]*>/g;
  const button = /<(?:a|button)\b[^>]*class="([^"]*)"[^>]*>/g;

  for (const page of pages()) {
    const html = read(page);
    let last = null;
    const marks = [];
    let m;
    opener.lastIndex = 0;
    while ((m = opener.exec(html)) !== null) {
      marks.push({ at: m.index, cls: m[1].trim() });
    }
    button.lastIndex = 0;
    while ((m = button.exec(html)) !== null) {
      if (!BUTTON_CLASS.test(m[1])) { continue; }
      last = null;
      for (const mark of marks) {
        if (mark.at < m.index) { last = mark.cls; } else { break; }
      }
      if (!last) { continue; }
      for (const cls of last.split(/\s+/).filter(Boolean)) {
        if (!found.has(cls)) { found.set(cls, new Set()); }
        found.get(cls).add(page);
      }
    }
  }
  return found;
}

/** Every CSS rule whose selector mentions this class, with its body. */
function rulesFor(css, cls) {
  const out = [];
  const rule = /([^{}]+)\{([^{}]*)\}/g;
  let m;
  while ((m = rule.exec(css)) !== null) {
    const selector = m[1].replace(/\/\*[\s\S]*?\*\//g, "").trim();
    if (new RegExp(`\\.${cls}(?![\\w-])`).test(selector)) {
      out.push({ selector, body: m[2] });
    }
  }
  return out;
}

function run() {
  let failures = 0;
  const fail = (why) => { failures += 1; console.log(`FAIL ${why}`); };
  const pass = (what) => console.log(`  ok  ${what}`);

  const css = read("website/css/main.css");
  const holders = containersHoldingButtons();

  if (holders.size === 0) {
    fail("no container holding a button was found on any page, so this suite " +
         "is reading the pages wrongly and is checking nothing");
    return failures;
  }

  let checked = 0;
  for (const [cls, where] of [...holders].sort()) {
    for (const { selector, body } of rulesFor(css, cls)) {
      if (!/display:\s*(inline-)?flex/.test(body)) { continue; }
      checked += 1;
      if (!/align-items:/.test(body)) {
        fail(`\`${selector}\` holds buttons on ${[...where].join(", ")} and is ` +
             "a flex container that does not declare `align-items`, so it " +
             "falls back to `stretch`: one button whose label wraps makes " +
             "every button beside it taller. Say what you mean, even if you " +
             "mean stretch.");
      } else {
        pass(`\`${selector}\` decides how its buttons line up`);
      }
    }
  }

  if (checked === 0) {
    fail("every container that holds a button was found, and not one of them " +
         "is a flex row, which is not credible: the selectors or the " +
         "stylesheet path have moved and this suite is passing without " +
         "reading anything");
  }

  return failures;
}

module.exports = { run, name: "buttons line up with the buttons beside them" };
