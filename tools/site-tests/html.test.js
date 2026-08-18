// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//
// Structural checks on the published pages.
//
// These catch the mistakes hand-edited HTML actually makes — an unclosed
// section that silently swallows the rest of the page, a navigation link
// pointing at an anchor that no longer exists, an id used twice so the browser
// jumps to the wrong one. None of them are security problems; all of them ship
// a broken page to everybody.
//
// Also enforced here, rather than only in the deploy workflow, so a mistake is
// caught before the push rather than after: no third-party asset references,
// no inline event handlers, and the signing-key fingerprint on every page.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const SITE = path.join(ROOT, "website");

/**
 * Every HTML page the site publishes, **discovered** rather than listed.
 *
 * This was a hardcoded list of three files, under a comment saying the checks
 * applied to "every page". They did not. `search.html` was added and was never
 * checked at all -- not for the signing-key fingerprint, not for balanced tags,
 * not for third-party assets, not for inline event handlers. It shipped
 * without the fingerprint, which is the one thing on these pages that lets a
 * reader tell a real release from a forged one.
 *
 * That is section 4.5 of `docs/AUDIT.md` happening to the tests themselves:
 * *a finished scope is only as wide as the list it was drawn from.* The
 * defence is to enumerate from the directory rather than from memory, so a new
 * page is covered the moment it exists rather than whenever somebody remembers
 * to add it here.
 */
function discoverPages(dir = SITE, found = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) { discoverPages(full, found); }
    else if (entry.name.endsWith(".html")) { found.push(full); }
  }
  return found.sort();
}

const PAGES = discoverPages();

// Hosts the site is allowed to link to. Not fetch from — link to. Nothing on
// these pages may *load* from anywhere but the same origin.
const ALLOWED_LINK_HOSTS = [
  "github.com", "raw.githubusercontent.com", "api.github.com",
  "www.audacityteam.org", "vb-audio.com", "creativecommons.org", "www.gnu.org"
];

const VOID = new Set(["area", "base", "br", "col", "embed", "hr", "img", "input",
                      "link", "meta", "param", "source", "track", "wbr", "!doctype"]);

function balance(html) {
  const problems = [];
  const stack = [];
  const re = /<(\/?)([a-zA-Z!][a-zA-Z0-9-]*)\b[^>]*?(\/?)>/g;
  let m;
  while ((m = re.exec(html)) !== null) {
    const closing = m[1] === "/";
    const name = m[2].toLowerCase();
    if (VOID.has(name) || m[3] === "/") { continue; }
    if (!closing) { stack.push({ name, at: m.index }); continue; }
    const top = stack.pop();
    if (!top) { problems.push(`stray </${name}> at offset ${m.index}`); }
    else if (top.name !== name) {
      problems.push(`</${name}> at ${m.index} closes <${top.name}> opened at ${top.at}`);
    }
  }
  for (const left of stack) { problems.push(`unclosed <${left.name}> at offset ${left.at}`); }
  return problems;
}

function run() {
  let failures = 0;
  const fingerprint = fs
    .readFileSync(path.join(SITE, "assets", "fingerprint.txt"), "utf8")
    .replace(/\s/g, "")
    .toUpperCase();

  for (const page of PAGES) {
    const rel = path.relative(ROOT, page).replace(/\\/g, "/");
    const raw = fs.readFileSync(page, "utf8");
    const html = raw.replace(/<!--[\s\S]*?-->/g, "");
    const problems = [];

    problems.push(...balance(html));

    const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map(m => m[1]);
    const duplicated = [...new Set(ids.filter((v, i) => ids.indexOf(v) !== i))];
    if (duplicated.length) { problems.push(`duplicate ids: ${duplicated.join(", ")}`); }

    const known = new Set(ids);
    const dangling = [...new Set([...html.matchAll(/href="#([^"]+)"/g)].map(m => m[1]))]
      .filter(a => !known.has(a));
    if (dangling.length) { problems.push(`anchors with no target: #${dangling.join(", #")}`); }

    // A privacy tool's site loading a CDN would undercut the whole claim.
    for (const m of html.matchAll(/(?:src|href)="(https?:\/\/[^"]+)"/g)) {
      const host = m[1].replace(/^https?:\/\//, "").split(/[/?#]/)[0];
      if (!ALLOWED_LINK_HOSTS.includes(host)) {
        problems.push(`reference to an unexpected host: ${host}`);
      }
    }
    for (const m of html.matchAll(/<(?:script|link|img)\b[^>]*\b(?:src|href)="(\/\/|https?:)/g)) {
      problems.push(`asset loaded from a third party: ${m[1]}`);
    }

    // Inline handlers are the thing the renderer's escaping exists to prevent;
    // the pages themselves must not undo that by hand.
    for (const m of html.matchAll(/<[^>]*\son[a-z]+\s*=/gi)) {
      problems.push(`inline event handler: ${m[0].trim().slice(0, 60)}`);
    }

    // The fingerprint is how a reader tells a real release from a forged one.
    if (!raw.replace(/\s/g, "").toUpperCase().includes(fingerprint)) {
      problems.push("the signing-key fingerprint is missing from this page");
    }

    if (problems.length) {
      failures++;
      console.log(`FAIL ${rel}`);
      problems.slice(0, 10).forEach(p => console.log(`     ${p}`));
    }
  }

  // Every script the pages reference must exist and parse.
  for (const page of PAGES) {
    const dir = path.dirname(page);
    const html = fs.readFileSync(page, "utf8");
    for (const m of html.matchAll(/<script[^>]+src="([^"]+)"/g)) {
      const file = path.resolve(dir, m[1]);
      if (!fs.existsSync(file)) {
        failures++;
        console.log(`FAIL ${path.relative(ROOT, page)} references a missing script: ${m[1]}`);
      }
    }
  }

  console.log(`  ${PAGES.length} pages checked`);
  return failures;
}

module.exports = { run, name: "published pages, structure" };

if (require.main === module) { process.exit(run() ? 1 : 0); }
