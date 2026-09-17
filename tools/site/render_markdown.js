// SPDX-License-Identifier: GPL-3.0-or-later
//
// Render Markdown to HTML with the renderer the website already has.
//
//     node tools/site/render_markdown.js < {"name": "markdown"} > {"name": "html"}
//
// Why this exists rather than a renderer in Python
// ------------------------------------------------
//
// `tools/site/wiki_site.py` publishes every page of the GitHub wiki as part of
// the website, and those pages are Markdown. There are two renderers in this
// repository already: `website/js/markdown.js`, which draws the README on the
// front page and has been through two rounds of hostile-input auditing, and
// the small one inside `tools/docs/generate.py`, whose own doc comment says in
// as many words that it is not a Markdown engine and must not become one.
//
// A third would be a third thing to audit and a third set of rules for what a
// heading or a table means. So the Python generator hands its text to this,
// which loads the audited renderer and hands the HTML back. One renderer, one
// behaviour, and a page on the website that is laid out exactly as the same
// text is on the front page.
//
// Input and output are JSON on the standard streams: one object of names to
// Markdown in, the same names to HTML out. One process for every page rather
// than one per page, because starting node is the expensive part.
"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");

// `markdown.js` is a browser script: it assigns `window.MD` and returns
// nothing. Given a `window` to assign to, it works unchanged, which is the
// point -- a copy edited to be importable would be a copy.
global.window = {};
new Function(fs.readFileSync(path.join(ROOT, "website", "js", "markdown.js"),
                             "utf8")).call(global);
const MD = global.window.MD;
if (!MD || typeof MD.render !== "function") {
  process.stderr.write("website/js/markdown.js did not define window.MD.render\n");
  process.exit(1);
}

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => { input += chunk; });
process.stdin.on("end", () => {
  const pages = JSON.parse(input);
  const out = {};
  for (const name of Object.keys(pages)) {
    out[name] = MD.render(pages[name]);
  }
  process.stdout.write(JSON.stringify(out));
});
