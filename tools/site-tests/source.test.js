// SPDX-License-Identifier: GPL-3.0-or-later
//
// The source pages, and the links that lead to them.
//
// # What this is for
//
// Marker 27 asks for diagrams that open the relevant source, highlighted, in
// the site's palette. Every box in a flowchart on a reference page is now a
// link to a page of this site rather than to a blob on GitHub, and every one
// of those links carries a fragment naming the function it drew.
//
// Three ways that can rot without anybody noticing, and one check each.
//
//   1. **The fragment stops resolving.** A renamed function, a changed anchor
//      scheme, a page that no longer emits the wrapper: the link still works,
//      lands at the top of the file, and marks nothing. `html.test.js` checks
//      anchors that point within one page and cannot see this one, because the
//      target is a different file.
//   2. **The page stops showing the file.** These pages are generated from the
//      `.rs` files and nothing else reads them, so a generator that dropped
//      the last line, or the first, would be invisible. The line count is
//      compared against the file on disk.
//   3. **A box goes back to leaving the site.** That is the whole of what the
//      marker asked for, so it is asserted rather than assumed.
//
// The stylesheet's half is checked too. The mark is `:target` and nothing
// else, so a reader with JavaScript off gets it; a rule that quietly went
// missing would leave every link landing in an unmarked file.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const SITE = path.join(ROOT, "website");
const REFERENCE = path.join(SITE, "reference");

function walk(dir, found = []) {
  if (!fs.existsSync(dir)) return found;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, found);
    else if (entry.name.endsWith(".html")) found.push(full);
  }
  return found;
}

function run() {
  let failures = 0;
  const fail = (message) => { failures++; console.log(`FAIL ${message}`); };
  const pass = (message) => console.log(`ok   ${message}`);

  const pages = walk(SITE).sort();
  const idsOf = new Map();
  const identifiers = (file) => {
    if (!idsOf.has(file)) {
      const html = fs.readFileSync(file, "utf8");
      idsOf.set(file, new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1])));
    }
    return idsOf.get(file);
  };

  // ---- 1. every fragment that names another page resolves -----------------
  let crossPage = 0;
  const broken = [];
  for (const page of pages) {
    const html = fs.readFileSync(page, "utf8");
    const here = path.dirname(page);
    for (const m of html.matchAll(/href="([^"#:][^"]*\.html)#([^"]+)"/g)) {
      const target = path.resolve(here, m[1]);
      const rel = path.relative(ROOT, page).replace(/\\/g, "/");
      crossPage++;
      if (!fs.existsSync(target)) {
        broken.push(`${rel} links to ${m[1]}, which does not exist`);
      } else if (!identifiers(target).has(m[2])) {
        broken.push(`${rel} links to ${m[1]}#${m[2]}, and that page has no such id`);
      }
    }
  }
  if (broken.length) {
    broken.slice(0, 15).forEach(fail);
    if (broken.length > 15) fail(`and ${broken.length - 15} more`);
  } else {
    pass(`all ${crossPage} fragments naming another page resolve`);
  }

  // ---- 2. a source page shows the whole file, and the same file -----------
  const sourcePages = pages.filter((p) => p.endsWith(".src.html"));
  if (sourcePages.length === 0) {
    fail("no source pages were generated at all");
  }
  const wrong = [];
  for (const page of sourcePages) {
    const html = fs.readFileSync(page, "utf8");
    const named = html.match(/<h1><code>([^<]+)<\/code><\/h1>/);
    const rel = path.relative(ROOT, page).replace(/\\/g, "/");
    if (!named) {
      wrong.push(`${rel} does not say which file it shows`);
      continue;
    }
    const file = path.join(ROOT, named[1]);
    if (!fs.existsSync(file)) {
      wrong.push(`${rel} claims to show ${named[1]}, which is not in the tree`);
      continue;
    }
    const text = fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n");
    const expected = text.endsWith("\n")
      ? text.split("\n").length - 1
      : text.split("\n").length;
    const drawn = (html.match(/<span class="ln" id="L\d+">/g) || []).length;
    if (drawn !== expected) {
      wrong.push(`${rel} draws ${drawn} lines of ${named[1]}, which has ${expected}`);
    }
  }
  if (wrong.length) {
    wrong.slice(0, 15).forEach(fail);
    if (wrong.length > 15) fail(`and ${wrong.length - 15} more`);
  } else {
    pass(`${sourcePages.length} source pages each show their whole file`);
  }

  // ---- 3. a box on a reference page stays on this site --------------------
  const leaving = [];
  let boxes = 0;
  for (const page of walk(REFERENCE)) {
    if (page.endsWith(".src.html")) continue;
    const html = fs.readFileSync(page, "utf8");
    for (const svg of html.match(/<svg\b[\s\S]*?<\/svg>/g) || []) {
      if (!/aria-label="flowchart"/.test(svg)) continue;
      for (const m of svg.matchAll(/<a\s+href="([^"]+)"/g)) {
        boxes++;
        if (/^[a-z][a-z0-9+.-]*:/i.test(m[1]) || m[1].startsWith("//")) {
          leaving.push(
            `${path.relative(ROOT, page).replace(/\\/g, "/")} has a box linking off ` +
            `this site: ${m[1]}`);
        }
      }
    }
  }
  if (leaving.length) {
    leaving.slice(0, 10).forEach(fail);
  } else if (boxes === 0) {
    fail("no flowchart box on any reference page carries a link");
  } else {
    pass(`all ${boxes} flowchart boxes open the source on this site`);
  }

  // ---- 4. the mark is in the stylesheet, and needs no script --------------
  const css = fs.readFileSync(path.join(SITE, "css", "main.css"), "utf8");
  const bare = css.replace(/\/\*[\s\S]*?\*\//g, "");
  const wanted = [
    [/\.src\s+\.src-item:target\s+\.ln\s*\{|\.src\s+\.ln:target[\s\S]{0,120}?\.src-item:target/,
     "the mark on a whole function must be a :target rule"],
    [/\.src\s+\.ln\s*\{[^}]*white-space:\s*pre/,
     "each line must keep its own whitespace, or the file renders as one paragraph"],
    [/pre\.src\s*\{[^}]*white-space:\s*normal/,
     "the block must not, or every line is drawn twice"],
    [/\.src\s+\.ln\s*\{[^}]*min-width:\s*100%/,
     "a line must span the full width, or the mark stops where the column does"]
  ];
  let missing = 0;
  for (const [pattern, why] of wanted) {
    if (!pattern.test(bare)) { fail(`main.css: ${why}`); missing++; }
  }
  if (!missing) pass("the mark is CSS, works with no script, and covers the line");

  return failures;
}

module.exports = { run, name: "source pages, and the boxes that open them" };
