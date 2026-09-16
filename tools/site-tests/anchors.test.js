// SPDX-License-Identifier: GPL-3.0-or-later
//
// Following a link into a page lands on the thing it names, with that thing
// visible.
//
// # The defect this exists to stop coming back
//
// The header is `position: sticky`, so whatever a fragment jump puts at the
// top of the viewport is underneath it. `scroll-margin-top` is the property
// for that, and the stylesheet set it to a flat `90px`.
//
// The header is not 90px tall, at any width. Measured in a browser it is
// 133px at desktop widths, 171px where the navigation wraps to three rows,
// 115px on a phone, 143px at 320px, and 81px on the reference pages. At the
// common desktop width that is 43px in the direction that puts the heading
// behind the header: the reader follows `download` and arrives at a page that
// appears to start mid-sentence.
//
// The rule also named `section`, `h2` and `h3` and nothing else, so an `h4`
// or a list item with an id got no offset at all. That is every entry on the
// releases page and every entry on the roadmap.
//
// # What is checked, and why it is these things
//
// A written-down height is the defect, so the check is that the offset is not
// written down: the stylesheet has to hold a variable, `js/teleport.js` has
// to measure the header, and every page whose header is sticky has to load
// it. None of that can be measured without a browser, which this suite does
// not have, but all of it can be read.
//
// The selector is checked too. A list of element names is how the releases
// page and the roadmap ended up with no offset at all, and the next id to be
// added to something not on the list would go the same way.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const SITE = path.join(ROOT, "website");

function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), "utf8");
}

/** Every page under `website/`, repository-relative, sorted. */
function pages(dir = SITE, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : 1)) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) { pages(full, out); }
    else if (entry.name.endsWith(".html")) { out.push(path.relative(ROOT, full)); }
  }
  return out;
}

function run() {
  let failures = 0;
  const fail = (why) => { failures += 1; console.log(`FAIL ${why}`); };
  const pass = (what) => console.log(`  ok  ${what}`);

  const css = read("website/css/main.css");

  // Every `scroll-margin-top` that exists to clear the header.
  const offsets = [...css.matchAll(/scroll-margin-top:\s*([^;]+);/g)].map(m => m[1].trim());
  if (!offsets.length) {
    fail("main.css declares no scroll-margin-top, so every fragment jump " +
         "lands underneath the sticky header");
  } else {
    const written = offsets.filter(value => !value.includes("var(--anchor-offset)"));
    if (written.length) {
      fail(`main.css clears the header with ${written.join(", ")} rather than ` +
           "the measured offset. The header is 81, 115, 133, 143 or 171px " +
           "tall depending on the page and the width, so any number written " +
           "here is wrong somewhere.");
    } else {
      pass(`all ${offsets.length} anchor offsets come from the measured variable`);
    }
  }

  // The selector. Anything with an id can be a fragment target.
  if (!/\[id\]\s*\{\s*scroll-margin-top:/.test(css)) {
    fail("main.css applies the anchor offset to a list of element names " +
         "rather than to `[id]`. An h4 or a list item with an id is a " +
         "fragment target too, and every roadmap and release entry is one.");
  } else {
    pass("the anchor offset applies to every id, not to a list of tag names");
  }

  // The starting value exists, so a page lands somewhere sensible in the frame
  // before the measurement lands.
  if (!/--anchor-offset:\s*\d+px/.test(css)) {
    fail("main.css declares no starting value for --anchor-offset, so the " +
         "offset is zero until js/teleport.js has run");
  } else {
    pass("--anchor-offset has a starting value for the frame before it is measured");
  }

  // The script measures rather than assumes.
  const js = read("website/js/teleport.js");
  if (!/getBoundingClientRect\(\)\.height/.test(js)) {
    fail("js/teleport.js no longer measures the header's height, which is " +
         "the whole reason it exists");
  } else {
    pass("js/teleport.js takes the header's height from the header");
  }

  // Nothing above a landing may change size after the jump.
  //
  // An image with no width and height has no size until it arrives, so
  // everything below it moves down when it does. Measured in a browser, a
  // link to the demonstration section landed and then kept moving for over a
  // second while six drawings of help screens and a screen photograph loaded
  // above it. The offset was right the whole time; the page was not finished.
  const unsized = [];
  for (const rel of pages()) {
    const html = read(rel);
    for (const tag of html.match(/<img\s[^>]*>/g) || []) {
      if (!/\swidth=/.test(tag) || !/\sheight=/.test(tag)) {
        unsized.push(`${rel}: ${tag.slice(0, 70)}`);
      }
    }
  }
  if (unsized.length) {
    fail(`${unsized.length} image(s) declare no width and height, so everything ` +
         `below them moves when they load: ${unsized.slice(0, 5).join("; ")}` +
         (unsized.length > 5 ? ", ..." : ""));
  } else {
    pass("every image declares its size, so nothing below it moves when it loads");
  }

  // Every page with a sticky header loads it.
  const missing = pages().filter(rel => {
    const html = read(rel);
    return /<header class="top">/.test(html) && !/js\/teleport\.js/.test(html);
  });
  if (missing.length) {
    fail(`${missing.length} page(s) carry the sticky header without loading ` +
         `js/teleport.js, so their anchors land at whatever the starting ` +
         `value happens to be: ${missing.slice(0, 5).join(", ")}` +
         (missing.length > 5 ? ", ..." : ""));
  } else {
    pass("every page with a sticky header loads the script that measures it");
  }

  return failures;
}

module.exports = { name: "fragment links land on what they name", run };

if (require.main === module) { process.exit(run() === 0 ? 0 : 1); }
