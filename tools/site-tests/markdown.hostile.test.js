// SPDX-License-Identifier: GPL-3.0-or-later
//
// Hostile-input tests for the site's Markdown renderer.
//
// # Why this exists
//
// `js/repo.js` fetches README.md over the network and assigns the rendered
// result straight to `innerHTML`. Every byte of that path's safety rests on one
// claim in `js/markdown.js`: that the source is escaped first and only tags the
// renderer itself emits are ever introduced. `docs/AUDIT.md` listed that claim
// as *asserted but not tested*. This file is the test.
//
// The threat is not hypothetical hand-waving. The README is fetched from
// raw.githubusercontent.com; anyone who could alter it, whether a compromised
// token, a bad merge or a mistaken commit, would be writing directly into the page unless
// the renderer holds.
//
// Two kinds of check:
//
//   1. A corpus of deliberately hostile documents, each aimed at a specific
//      escape route.
//   2. A randomised campaign that builds documents from dangerous fragments, on
//      the theory that the bug worth finding is the one nobody thought to write
//      a case for.
//
// Both assert the same invariant, and it is an allowlist rather than a
// blocklist: the output may only contain tags and attributes this renderer is
// supposed to produce. A blocklist of "no <script>" would pass a document that
// found some other way in.

"use strict";

const { execFileSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ROOT = path.resolve(__dirname, "..", "..");

// Generous, because this is not a stopwatch on the renderer: it is the line
// between "slow machine" and "will never finish". The whole corpus renders in
// well under a second.
const PROBE_TIMEOUT_MS = 60000;

function loadRenderer() {
  const src = fs.readFileSync(path.join(ROOT, "website", "js", "markdown.js"), "utf8");
  const sandbox = { window: {} };
  vm.createContext(sandbox);
  vm.runInContext(src, sandbox);
  return sandbox.window.MD;
}

const MD = loadRenderer();

// Everything the renderer is allowed to emit. Anything else in the output is a
// failure by definition, whether or not it happens to be exploitable today.
const ALLOWED_TAGS = new Set([
  "p", "br", "hr", "h1", "h2", "h3", "h4", "h5", "h6",
  "strong", "em", "code", "pre", "blockquote",
  "ul", "ol", "li", "a", "img", "span",
  "table", "thead", "tbody", "tr", "th", "td",
  // `<details>` and `<summary>` are the one piece of raw HTML the documents
  // use and the renderer passes three exact shapes of them through. Leaving
  // them off this list is why no `<details>` case could be written here: the
  // suite would have called the renderer doing its job a violation. The audit
  // read that the other way round, as the renderer not being tested on the
  // markup the README uses eleven times, and it was right.
  "details", "summary"
]);

const ALLOWED_ATTRS = new Set(["href", "src", "alt", "class", "rel"]);

// The check has to parse the way a browser parses, or it reports things that
// are not true. `&lt;script&gt;` in the output is *text*, which is the renderer
// doing its job, and `src="a&quot;onerror=x"` is a single attribute whose value
// happens to contain a quote character, because entity references are decoded
// *after* the value has been delimited. A naive scan of the raw string calls
// both of those attacks and hides the real bug in the noise.

/** Decode the entities the renderer can emit, for inspecting a URL value. */
function decodeEntities(value) {
  return value
    .replace(/&quot;/g, '"')
    .replace(/&#(\d+);/g, (_, n) => String.fromCharCode(Number(n)))
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

/** Split a tag's attribute text the way an HTML parser would. */
function parseAttributes(text) {
  const attrs = [];
  const re = /([a-zA-Z_:][-a-zA-Z0-9_:.]*)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    if (!m[0].trim()) { break; }
    attrs.push({ name: m[1].toLowerCase(), value: m[2] ?? m[3] ?? m[4] ?? "" });
  }
  return attrs;
}

/** A URL scheme that can run code, checked after entity decoding. */
function dangerousUrl(value) {
  // Strip characters an HTML parser ignores before the scheme, which is the
  // classic way `java\tscript:` sneaks past a naive prefix test.
  const url = decodeEntities(value)
    .replace(new RegExp("[\\u0000-\\u0020]", "g"), "")
    .toLowerCase();
  return url.startsWith("javascript:") || url.startsWith("vbscript:") ||
         (url.startsWith("data:") && !url.startsWith("data:image/"));
}

function findViolations(html) {
  const bad = [];

  // No internal machinery may reach the page. Placeholders are private-use
  // characters, which browsers draw as nothing at all, so one escaping is not
  // a visible glitch, it is content that silently disappears. Exactly that
  // shipped: every README link whose label was inline code rendered empty,
  // because the un-parking pass did not recurse into its own output.
  const escaped = [...html].filter(c => c.charCodeAt(0) >= 0xe000 && c.charCodeAt(0) <= 0xf8ff);
  if (escaped.length) {
    bad.push(`${escaped.length} un-parked placeholder character(s) reached the output`);
  }
  // Control characters have no business in rendered HTML either.
  if (new RegExp("[\\u0000-\\u0008\\u000B\\u000C\\u000E-\\u001F]").test(html)) {
    bad.push("a control character reached the output");
  }
  // Built rather than typed: a file that checks for stray characters must not
  // contain one. `tools/site-tests/characters.test.js` enforces that repo-wide.
  if (html.includes(String.fromCharCode(0xfffd))) {
    bad.push("a Unicode replacement character reached the output");
  }

  const tagRe = /<\/?([a-zA-Z][a-zA-Z0-9-]*)((?:\s[^>]*)?)\/?>/g;
  let m;
  while ((m = tagRe.exec(html)) !== null) {
    const tag = m[1].toLowerCase();
    if (!ALLOWED_TAGS.has(tag)) { bad.push(`disallowed tag <${tag}>`); continue; }
    for (const attr of parseAttributes(m[2])) {
      if (attr.name.startsWith("on")) {
        bad.push(`event handler ${attr.name} on <${tag}>`);
      } else if (!ALLOWED_ATTRS.has(attr.name)) {
        bad.push(`disallowed attribute ${attr.name} on <${tag}>`);
      }
      if ((attr.name === "href" || attr.name === "src") && dangerousUrl(attr.value)) {
        bad.push(`executable URL in ${attr.name} on <${tag}>: ${attr.value.slice(0, 60)}`);
      }
    }
  }
  return bad;
}

// --- the corpus ------------------------------------------------------------
// Each entry names the escape route it is trying to take.

const HOSTILE = [
  ["a bare script tag", '<script>alert(1)</script>'],
  ["a script tag mid-paragraph", 'text before <script>alert(1)</script> text after'],
  ["an img with an error handler", '<img src=x onerror=alert(1)>'],
  ["an svg with an onload", '<svg onload=alert(1)></svg>'],
  ["an iframe", '<iframe src="https://evil.example"></iframe>'],
  ["a javascript: link", '[click me](javascript:alert(1))'],
  ["a javascript: link, mixed case", '[click me](JaVaScRiPt:alert(1))'],
  ["a data: URL link", '[click me](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)'],
  ["a javascript: image", '![alt](javascript:alert(1))'],
  ["a quote break-out in an image source", '![alt](http://a"onerror="alert(1))'],
  ["a quote break-out in image alt text", '![" onerror="alert(1)](http://example.com/a.png)'],
  ["a quote break-out in a link label", '[" onmouseover="alert(1)](http://example.com)'],
  ["an entity-encoded javascript URL", '[x](&#106;avascript:alert(1))'],
  ["a tab-split scheme", '[x](java\tscript:alert(1))'],
  ["a null-byte-split scheme", '[x](java\u0000script:alert(1))'],
  ["raw html inside a heading", '# <img src=x onerror=alert(1)>'],
  ["raw html inside a list item", '- <script>alert(1)</script>'],
  ["raw html inside a table cell", 'a | b\n--- | ---\n<script>alert(1)</script> | c'],
  ["raw html inside a blockquote", '> <script>alert(1)</script>'],
  ["raw html inside a fenced block", '```\n<script>alert(1)</script>\n```'],
  ["raw html inside inline code", '`<script>alert(1)</script>`'],
  ["a style block", '<style>body{display:none}</style>'],
  ["a base tag", '<base href="https://evil.example/">'],
  ["a form and input", '<form action="https://evil.example"><input name="p"></form>'],
  ["an unclosed tag swallowing the rest", '<div onclick="alert(1)"'],
  ["a comment that never closes", '<!-- ' + 'x'.repeat(200)],
  ["angle brackets in a table header", '<script> | b\n--- | ---\nc | d'],
  ["a link label containing a closing anchor", '[</a><script>alert(1)</script>](http://example.com)'],
  ["nested emphasis around markup", '**<script>alert(1)</script>**'],
  ["an image whose alt closes the tag", '![x><script>alert(1)</script>](http://example.com/a.png)'],
  ["a protocol-relative link", '[x](//evil.example/path)'],
  ["a very long link target", '[x](http://example.com/' + 'a'.repeat(5000) + ')'],
  ["deep blockquote nesting", '>'.repeat(60) + ' hello'],
  ["a fence that is never closed", '```rust\nfn main() { println!("hi"); }'],
  ["backtick soup", '`'.repeat(200)],
  ["asterisk soup", '*'.repeat(200)],
  ["pipe soup", '|'.repeat(200) + '\n' + '-|'.repeat(100)],
  ["mixed markers", '#'.repeat(50) + ' <script>x</script>'],
  ["an anchor inside inline code inside a link", '[`</code><script>alert(1)</script>`](http://example.com)'],

  // Collapsed sections. The README writes eleven of them and this file had no
  // case for one, which is how four documents that stopped the renderer dead
  // reached a published page. Each of these looped forever: the block
  // dispatch recognised three exact shapes of the tags and nothing else,
  // while the paragraph collector refused any line that started with one, so
  // a line neither would take left the index where it was. They are kept as
  // hostile input because that is what they are over the network: `repo.js`
  // renders README.md as fetched from GitHub, so a document that freezes this
  // renderer is a denial of service against the reader.
  ["a details and summary on one line", '<details><summary>x</summary>'],
  ["a summary that is never closed", '<summary>x'],
  ["text after a closing details", '</details>junk'],
  ["a heading holding a line separator", '# head\u2028ing'],
  ["a summary with no details around it", '<summary>alone</summary>\n\ntext'],
  ["a details with an attribute", '<details open>\nbody\n</details>'],
  ["a summary carrying markup", '<details>\n<summary><img src=x onerror=alert(1)></summary>\n</details>'],
  ["a details tag that never closes", '<details>\n<summary>s</summary>\nbody'],
  ["a closing details with nothing open", '</details>\n\ntext'],
  ["a line separator at every position", 'a\u2028'.repeat(200)]
];

// Every document this suite knows about. `render-probe.js` renders this list
// in a process of its own so that a renderer which stops advancing is a
// reported failure rather than a hung job.
const DOCUMENTS = HOSTILE;

// --- randomised campaign ---------------------------------------------------

const FRAGMENTS = [
  "<script>", "</script>", "<img", "src=x", "onerror=alert(1)", "javascript:",
  "data:text/html", '"', "'", "<", ">", "&", "&quot;", "&#106;", "`", "```",
  "[", "]", "(", ")", "!", "*", "**", "_", "#", ">", "|", "---", "\n", " ",
  "0", " 1 ", " 2 ", "\t", "\u0000", "\\", "%", "%22", "//evil.example",
  "http://example.com", "</a>", "</code>", "<svg", "onload=", "-->", "<!--",
  // The collapsed-section tags, and the line separator that `.` does not
  // match. Without these the campaign could not reach the shapes that hung
  // the renderer: it can produce `<` and `>` but never happens on the exact
  // word between them.
  "<details>", "</details>", "<summary>", "</summary>", "<details open>",
  "\u2028", "\u2029"
];

// A tiny deterministic PRNG, so a failure can be reproduced from its seed
// rather than being a story about a run nobody can repeat.
function rng(seed) {
  let s = seed >>> 0;
  return function () {
    s ^= s << 13; s >>>= 0;
    s ^= s >> 17;
    s ^= s << 5;  s >>>= 0;
    return s / 4294967296;
  };
}

function generate(random) {
  const pieces = 3 + Math.floor(random() * 40);
  let doc = "";
  for (let i = 0; i < pieces; i++) {
    doc += FRAGMENTS[Math.floor(random() * FRAGMENTS.length)];
  }
  return doc;
}

// --- runner ----------------------------------------------------------------

/**
 * Every document finishes rendering, measured from outside.
 *
 * This runs first and deliberately so. The checks below render in this
 * process, and a document the renderer cannot finish would take the suite with
 * it, so the thing that proves the renderer still advances cannot itself be
 * one of them. A timeout here names the document that would not finish, from
 * the last name the probe managed to write.
 */
function everyDocumentFinishes() {
  const probe = path.join(__dirname, "render-probe.js");
  let out;
  try {
    out = execFileSync(process.execPath, [probe],
                       { encoding: "utf8", timeout: PROBE_TIMEOUT_MS });
  } catch (e) {
    const printed = String(e.stdout || "").trim().split("\n");
    const last = printed[printed.length - 1] || "nothing at all";
    if (e.killed || e.signal) {
      console.log(`FAIL the renderer did not finish within ` +
                  `${PROBE_TIMEOUT_MS / 1000}s, and stopped advancing on: ${last}`);
      console.log("     every pass of the block loop in markdown.js must move");
      console.log("     the line index. A pass that consumes nothing emits the");
      console.log("     line as escaped text and advances.");
    } else {
      console.log(`FAIL the render probe failed: ${e.message.split("\n")[0]}`);
    }
    return 1;
  }
  if (!out.includes("every document finished")) {
    console.log("FAIL the render probe stopped early:");
    console.log(`     ${out.trim().split("\n").slice(-2).join(" / ")}`);
    return 1;
  }
  const threw = out.split("\n").filter((l) => l.startsWith("threw "));
  if (threw.length) {
    console.log(`FAIL ${threw.length} document(s) made the renderer throw:`);
    threw.slice(0, 5).forEach((l) => console.log(`     ${l}`));
    return 1;
  }
  console.log(`  every one of the ${DOCUMENTS.length} documents finished, ` +
              "measured in a process of its own");
  return 0;
}

function run() {
  // Nothing below this line can run if the renderer cannot finish: the checks
  // that follow render in this process, so they would hang on the same
  // document the probe has just reported, and the reported failure would
  // never be printed. Inverting the fix to prove this check catches it is
  // exactly how that was found.
  if (everyDocumentFinishes()) {
    console.log("     the checks that render in this process are skipped, " +
                "because they would hang too");
    return 1;
  }
  let failures = 0;

  for (const [name, source] of HOSTILE) {
    let html;
    try {
      html = MD.render(source);
    } catch (e) {
      console.log(`FAIL ${name}: renderer threw ${e.message}`);
      failures++;
      continue;
    }
    const bad = findViolations(html);
    if (bad.length) {
      failures++;
      console.log(`FAIL ${name}`);
      console.log(`     ${bad.join("; ")}`);
      console.log(`     output: ${html.slice(0, 200)}`);
    }
  }
  console.log(`  corpus: ${HOSTILE.length} hostile documents`);

  const ROUNDS = Number(process.env.MD_FUZZ_ROUNDS || 20000);
  let firstBad = null;
  for (let seed = 1; seed <= ROUNDS; seed++) {
    const source = generate(rng(seed));
    let html;
    try {
      html = MD.render(source);
    } catch (e) {
      firstBad = { seed, source, why: `threw ${e.message}` };
      break;
    }
    const bad = findViolations(html);
    if (bad.length) {
      firstBad = { seed, source, why: bad.join("; ") };
      break;
    }
  }
  if (firstBad) {
    failures++;
    console.log(`FAIL randomised campaign at seed ${firstBad.seed}: ${firstBad.why}`);
    console.log(`     source: ${JSON.stringify(firstBad.source).slice(0, 300)}`);
  }
  console.log(`  randomised: ${ROUNDS} generated documents`);

  return failures;
}

module.exports = { run, name: "markdown renderer, hostile input", DOCUMENTS };

if (require.main === module) { process.exit(run() ? 1 : 0); }
