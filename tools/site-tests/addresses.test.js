// SPDX-License-Identifier: GPL-3.0-or-later
//
// Every page says where it lives, and a crawler is told it may read the site.
//
// # The defect this exists to stop coming back
//
// `og:image` was `assets/banner.png` on every page: a relative URL, typed by
// hand. The crawlers that read these tags do not resolve relative URLs against
// the page, so every link to this site, in every chat application and every
// social network, showed no picture at all. Nothing on the page looked wrong,
// which is why it survived: the tag was there, it was spelled correctly, and
// it named a file that exists.
//
// There was no `og:url` and no canonical link either, so a page reachable at
// more than one address is more than one page as far as an index is concerned,
// and two of them, `404.html` and `wiki.html`, carried no tags at all.
//
// # Why this is here as well as `tools/site/seo.py --check`
//
// That check asks "is each page exactly what the generator would write". This
// one asks "is what the generator writes correct". They fail differently: a
// generator that starts emitting relative image URLs passes the first check on
// every page and fails this one on every page.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const SITE = path.join(ROOT, "website");

function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), "utf8");
}

/** Every page of the site, relative to `website/`, sorted. */
function pages(dir = SITE, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : 1)) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) { pages(full, out); }
    else if (entry.name.endsWith(".html")) {
      out.push(path.relative(SITE, full).replace(/\\/g, "/"));
    }
  }
  return out;
}

function run() {
  let failures = 0;
  const fail = (why) => { failures += 1; console.log(`FAIL ${why}`); };
  const pass = (what) => console.log(`  ok  ${what}`);

  const all = pages();

  // robots.txt: readable, and it says where the sitemap is.
  let robots = null;
  try { robots = read("website/robots.txt"); } catch (e) { /* reported below */ }
  if (robots === null) {
    fail("website/robots.txt is missing, so a crawler is told nothing at all");
  } else if (!/^\s*User-agent:\s*\*/m.test(robots) || !/^\s*Allow:\s*\/\s*$/m.test(robots)) {
    fail("website/robots.txt does not allow every crawler to read the whole site");
  } else if (!/^Sitemap:\s*https:\/\/\S+\/sitemap\.xml\s*$/m.test(robots)) {
    fail("website/robots.txt names no sitemap, so a crawler has to guess at the pages");
  } else {
    pass("robots.txt allows the whole site and names the sitemap");
  }

  // The sitemap lists every page, and nothing that is not one.
  let sitemap = null;
  try { sitemap = read("website/sitemap.xml"); } catch (e) { /* reported below */ }
  if (sitemap === null) {
    fail("website/sitemap.xml is missing");
  } else {
    const listed = [...sitemap.matchAll(/<loc>([^<]+)<\/loc>/g)].map(m => m[1]);
    const base = listed.length ? listed[0].replace(/[^/]*$/, "") : "";
    const tails = new Set(listed.map(url => url.slice(base.length) || "index.html"));
    const missing = all.filter(rel => !tails.has(rel) && !tails.has(rel.replace(/index\.html$/, "")));
    if (!listed.length) {
      fail("website/sitemap.xml lists no pages");
    } else if (missing.length) {
      fail(`${missing.length} page(s) are not in the sitemap, so nothing points a ` +
           `crawler at them: ${missing.slice(0, 5).join(", ")}` +
           (missing.length > 5 ? ", ..." : ""));
    } else {
      pass(`the sitemap lists all ${all.length} pages`);
    }
  }

  // Each page: one canonical, one og:url, and an absolute preview picture.
  const noCanonical = [];
  const manyCanonical = [];
  const relativeImage = [];
  const noTitle = [];
  for (const rel of all) {
    const html = read("website/" + rel);
    const canonical = [...html.matchAll(/<link rel="canonical" href="([^"]*)"/g)];
    if (!canonical.length) { noCanonical.push(rel); }
    else if (canonical.length > 1) { manyCanonical.push(rel); }
    else if (!/^https:\/\//.test(canonical[0][1])) { relativeImage.push(`${rel} (canonical)`); }

    for (const m of html.matchAll(/<meta (?:property|name)="(?:og:image|twitter:image|og:url)" content="([^"]*)"/g)) {
      if (!/^https:\/\//.test(m[1])) { relativeImage.push(`${rel}: ${m[1]}`); }
    }
    if (!/<meta property="og:title" content="[^"]+"/.test(html)) { noTitle.push(rel); }
  }

  if (noCanonical.length) {
    fail(`${noCanonical.length} page(s) have no canonical link, so an index cannot ` +
         `tell which address is the page's own: ${noCanonical.slice(0, 5).join(", ")}`);
  } else if (manyCanonical.length) {
    fail(`${manyCanonical.length} page(s) declare more than one canonical address: ` +
         manyCanonical.slice(0, 5).join(", "));
  } else {
    pass(`all ${all.length} pages name one canonical address`);
  }

  if (relativeImage.length) {
    fail(`${relativeImage.length} address(es) are relative. A crawler reading these ` +
         `tags does not resolve them against the page, so the link preview has no ` +
         `picture: ${relativeImage.slice(0, 5).join("; ")}`);
  } else {
    pass("every address and preview picture is absolute");
  }

  if (noTitle.length) {
    fail(`${noTitle.length} page(s) carry no og:title, so a link to them shows the ` +
         `URL: ${noTitle.slice(0, 5).join(", ")}`);
  } else {
    pass("every page carries a preview title");
  }

  return failures;
}

module.exports = { name: "every page says where it lives", run };

if (require.main === module) { process.exit(run() === 0 ? 0 : 1); }
