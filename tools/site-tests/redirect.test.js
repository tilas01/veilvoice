// SPDX-License-Identifier: GPL-3.0-or-later
//
// The old address must land every reader on the same page at the new one.
//
// tools/site/redirect.py writes what tilas01.github.io/veilvoice serves after
// the move. These checks build it into a temporary folder and then load its
// pages the way a browser would arrive at them: with a path, a query string
// and a fragment. Each must end at the matching address under
// tilas01.github.io/veilvoice-website with the query and fragment intact, and
// a path the old site never had must end on the new home page.
//
// They also hold the parts a script cannot cover: readers without JavaScript
// follow the refresh tag, search engines read the canonical link, the page
// shows no text, and the signing key is still served byte for byte for
// the curl command in docs/INSTALL.md.

"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const vm = require("vm");
const { execFileSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..", "..");
const NEW = "https://tilas01.github.io/veilvoice-website";

function build() {
  const out = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "veilvoice-redirect-")), "out");
  execFileSync("python3", [path.join(ROOT, "tools", "site", "redirect.py"), out], { stdio: "pipe" });
  return out;
}

/** Run a page's script as if the browser had opened `href`; return where it went. */
function land(html, href) {
  const script = html.match(/<script>([\s\S]*?)<\/script>/);
  if (!script) { return null; }
  const url = new URL(href);
  let went = null;
  const location = {
    pathname: url.pathname,
    search: url.search,
    hash: url.hash,
    replace(to) { went = to; }
  };
  vm.runInNewContext(script[1], { location, Set });
  return went;
}

function run() {
  let fails = 0;
  const check = (label, ok) => {
    console.log(`  ${ok ? "ok  " : "FAIL"} ${label}`);
    if (!ok) { fails += 1; }
  };

  const out = build();
  const read = rel => fs.readFileSync(path.join(out, rel), "utf8");

  const cases = [
    ["index.html", "https://tilas01.github.io/veilvoice/", `${NEW}/`],
    ["index.html", "https://tilas01.github.io/veilvoice", `${NEW}/`],
    ["index.html", "https://tilas01.github.io/veilvoice/?ref=old#download", `${NEW}/?ref=old#download`],
    ["faq.html", "https://tilas01.github.io/veilvoice/faq.html#offline", `${NEW}/faq.html#offline`],
    ["wiki/Doc-INSTALL.html", "https://tilas01.github.io/veilvoice/wiki/Doc-INSTALL.html?x=1&y=2#verify",
      `${NEW}/wiki/Doc-INSTALL.html?x=1&y=2#verify`],
    ["nojs/index.html", "https://tilas01.github.io/veilvoice/nojs/", `${NEW}/nojs/`],
    ["404.html", "https://tilas01.github.io/veilvoice/a/page/that/never/existed?q=%20#h", `${NEW}/`],
    ["404.html", "https://tilas01.github.io/veilvoice/search-index.json?v=2", `${NEW}/search-index.json?v=2`],
    ["404.html", "https://tilas01.github.io/veilvoice/assets/banner.png", `${NEW}/assets/banner.png`],
    ["404.html", "https://tilas01.github.io/veilvoice/reference/", `${NEW}/reference/`],
    ["404.html", "https://tilas01.github.io/veilvoice/assets/", `${NEW}/`],
    ["404.html", "https://tilas01.github.io/veilvoicefoo/x", `${NEW}/`]
  ];
  for (const [file, from, to] of cases) {
    check(`${from} lands on ${to}`, land(read(file), from) === to);
  }

  const faq = read("faq.html");
  check("a page without scripts refreshes to its own new address",
    faq.includes(`<meta http-equiv="refresh" content="0; url=${NEW}/faq.html">`));
  check("a page names its new address as canonical", faq.includes(`<link rel="canonical" href="${NEW}/faq.html">`));
  check("a folder index points at the folder", read("nojs/index.html").includes(`url=${NEW}/nojs/"`));

  const body = faq.match(/<body>([\s\S]*?)<\/body>/)[1];
  check("the page shows no text, only the redirect", body.trim() === "");

  for (const kept of ["assets/veilvoice-signing-key.asc", "assets/fingerprint.txt"]) {
    const a = fs.readFileSync(path.join(out, kept));
    const b = fs.readFileSync(path.join(ROOT, "website", kept));
    check(`${kept} is served byte for byte`, a.equals(b));
  }

  const listed = [];
  (function walk(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) { walk(full); } else if (entry.name.endsWith(".html")) { listed.push(full); }
    }
  })(path.join(ROOT, "website"));
  const covered = listed.every(full => fs.existsSync(path.join(out, path.relative(path.join(ROOT, "website"), full))));
  check(`every one of the ${listed.length} old pages has a redirect`, covered);

  fs.rmSync(path.dirname(out), { recursive: true, force: true });
  return fails;
}

module.exports = { run, name: "the old address after the move" };

if (require.main === module) { process.exit(run() ? 1 : 0); }
