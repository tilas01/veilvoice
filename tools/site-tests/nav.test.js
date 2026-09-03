// SPDX-License-Identifier: GPL-3.0-or-later
//
// Every page on the site carries the same header navigation, and every link in
// it points at something that exists.
//
// # Why this is a test
//
// The header was not the same on every page. Nine hand-written pages carried
// the full thirteen links. `wiki.html` carried six, `search.html` nine,
// `404.html` four, and the twelve hundred generated pages under
// `website/reference/` carried five.
//
// So clicking `reference` -- the link whose whole job is to send a reader into
// the source -- landed them on a page whose header had silently dropped
// `what`, `download`, `guide`, `verify`, `security`, `faq`, `roadmap` and
// `releases`. Eight ways out of the page, gone, with no way back to any of them
// except `home`. Nothing was broken in the sense a link checker understands:
// every link that was there worked. The fault was the links that were not
// there, which is exactly the kind of thing no existing check could see.
//
// A header that changes shape as a reader moves through the site is worse than
// a long one. It reads as though they have left the site, and on a project
// whose argument is "go and read it yourself" the reference pages are the last
// place that should feel like somewhere else.
//
// # What is checked
//
// `index.html` is the definition: whatever nav it has is what every other page
// must have, label for label, in order. That way the test cannot drift out of
// date with a deliberate change to the menu -- add a link to `index.html` and
// every other page is required to grow it too, which is the property actually
// wanted.
//
// Links are then resolved for real, relative to the page that carries them, so
// the `../../` prefixes the generator computes for depth are checked rather
// than assumed. Fragments are resolved too, against the `id` attributes of the
// target page: `index.html#crypto` from a page two directories down has three
// separate ways to be wrong, and F-37's shape was exactly a depth prefix that
// no test looked at.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const SITE = path.join(ROOT, "website");

/** Every `.html` file under `website/`, repository-relative, sorted. */
function pages(dir = SITE, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : 1)) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) { pages(full, out); }
    else if (entry.name.endsWith(".html")) { out.push(path.relative(ROOT, full)); }
  }
  return out;
}

const NAV = /<nav class="links">([\s\S]*?)<\/nav>/;
const LINK = /<a\s+href="([^"]+)"[^>]*>([^<]*)<\/a>/g;

/** The [href, label] pairs of a page's header nav, or null if it has none. */
function navOf(text) {
  const block = NAV.exec(text);
  if (!block) { return null; }
  return [...block[1].matchAll(LINK)].map(m => [m[1], m[2].trim()]);
}

const idsCache = new Map();
function idsOf(rel) {
  if (!idsCache.has(rel)) {
    const text = fs.readFileSync(path.join(ROOT, rel), "utf8");
    idsCache.set(rel, new Set([...text.matchAll(/\bid="([^"]+)"/g)].map(m => m[1])));
  }
  return idsCache.get(rel);
}

function run() {
  let fails = 0;
  const check = (name, ok) => {
    console.log(`  ${ok ? "ok  " : "FAIL"} ${name}`);
    if (!ok) { fails++; }
    return ok;
  };

  const all = pages();

  // `nojs/` is the deliberately scriptless mirror. It has its own minimal
  // shell and no header nav at all, which is the point of it, so it is not
  // held to the shape of the main site's header.
  const withNav = all.filter(rel => !rel.startsWith(path.join("website", "nojs")));

  const home = "website/index.html";
  const want = navOf(fs.readFileSync(path.join(ROOT, home), "utf8"));
  check("index.html defines a header nav", want !== null && want.length > 0);
  if (!want) { return fails; }

  const wantLabels = want.map(([, label]) => label).join(" ");

  const wrong = [];
  const broken = [];
  let resolved = 0;

  for (const rel of withNav) {
    const text = fs.readFileSync(path.join(ROOT, rel), "utf8");
    const nav = navOf(text);
    if (nav === null) { wrong.push(`${rel}: no header nav at all`); continue; }

    const labels = nav.map(([, label]) => label).join(" ");
    if (labels !== wantLabels) {
      wrong.push(`${rel}: [${labels}]`);
      continue;
    }

    for (const [href] of nav) {
      if (/^https?:/i.test(href)) { continue; }
      const [target, frag] = href.split("#");
      // A bare `#section` is a link into the page that carries it, which is
      // how the home page's own header is written.
      // Anything else resolves relative to the directory of the page carrying
      // it, which is what makes the generator's depth prefixes testable.
      const targetRel = target === ""
        ? rel
        : path.relative(ROOT, path.resolve(path.dirname(path.join(ROOT, rel)), target));
      if (targetRel.startsWith("..")) { broken.push(`${rel} -> ${href} (escapes the repository)`); continue; }
      const abs = path.join(ROOT, targetRel);
      if (!fs.existsSync(abs) || !fs.statSync(abs).isFile()) { broken.push(`${rel} -> ${href} (no such page)`); continue; }
      if (frag && !idsOf(targetRel).has(frag)) { broken.push(`${rel} -> ${href} (no such anchor)`); continue; }
      resolved++;
    }
  }

  check(`all ${withNav.length} pages carry the same header as index.html ` +
        `(${want.length} links)`, wrong.length === 0);
  for (const line of wrong.slice(0, 10)) { console.log("     differs: " + line); }
  if (wrong.length > 10) { console.log(`     ...and ${wrong.length - 10} more`); }

  check(`every header link resolves, anchors included (${resolved} checked)`, broken.length === 0);
  for (const line of broken.slice(0, 10)) { console.log("     broken: " + line); }
  if (broken.length > 10) { console.log(`     ...and ${broken.length - 10} more`); }

  // The generated reference tree is the half that regressed, and it is the
  // half a hand-written fix cannot reach. If the walk stopped finding it, both
  // checks above would pass while testing only the pages that were never
  // wrong.
  const refs = withNav.filter(rel => rel.includes(path.join("website", "reference")));
  check(`the generated reference pages are covered (${refs.length} found)`, refs.length > 100);

  // Non-vacuity: a nav regex that matched nothing anywhere would report a
  // clean run over zero links.
  check("header links were actually resolved", resolved > 1000);

  return fails;
}

module.exports = { run, name: "header navigation" };

if (require.main === module) { process.exit(run() ? 1 : 0); }
