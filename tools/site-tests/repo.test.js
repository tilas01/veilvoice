// SPDX-License-Identifier: GPL-3.0-or-later
//
// `repo.js` is the only module on this site that puts data from a *third party*
// into the page. It fetches the GitHub API and raw README.md, and everything it
// does with those answers is a decision about how far a remote response is
// trusted.
//
// The audit's standing objection was "trusted by omission": download links were
// assigned straight from `browser_download_url` with no check, on the reasoning
// that GitHub's own API for this repository always returns a github.com URL.
// That reasoning is true and it is not a control. This suite is the control.
//
// # How the module is exercised without a browser
//
// `repo.js` is an IIFE that reaches for `document`, `fetch` and `window`.
// Rather than pull in a DOM library -- this project has no dependencies and
// that is deliberate -- the suite builds the smallest DOM and `fetch` the
// module actually touches and drives it through the same path a reader does:
// the module registers a `DOMContentLoaded` handler, that handler attaches a
// click listener to the button, and the click is what starts the fetches. All
// three are wired here rather than short-circuited, because a stub that models
// only the happy path cannot express a failure -- the mistake the reveal suite
// made once already, and recorded in the audit.

"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ROOT = path.resolve(__dirname, "..", "..");
const REPO_JS = fs.readFileSync(path.join(ROOT, "website", "js", "repo.js"), "utf8");
const MD_JS = fs.readFileSync(path.join(ROOT, "website", "js", "markdown.js"), "utf8");

// --- the smallest DOM the module uses ---------------------------------------

function makeElement(tag) {
  return {
    tagName: String(tag).toUpperCase(),
    children: [],
    attributes: Object.create(null),
    listeners: Object.create(null),
    style: { setProperty() {} },
    classList: { add() {}, remove() {} },
    disabled: false,
    _text: "",
    _html: null,
    offsetWidth: 0,
    get textContent() { return this._text; },
    set textContent(v) { this._text = String(v); this._html = null; },
    get innerHTML() { return this._html; },
    set innerHTML(v) { this._html = String(v); },
    setAttribute(name, value) {
      this.attributes[name] = String(value);
      if (name === "href") { this.href = String(value); }
    },
    getAttribute(name) {
      if (name === "href" && typeof this.href === "string") { return this.href; }
      return Object.prototype.hasOwnProperty.call(this.attributes, name)
        ? this.attributes[name]
        : null;
    },
    appendChild(child) { this.children.push(child); return child; },
    remove() {},
    querySelector(selector) {
      const found = this.querySelectorAll(selector);
      return found.length ? found[0] : null;
    },

    /**
     * Enough of a selector engine to see the anchors and images in whatever was
     * assigned to `innerHTML`, and to write attributes back into it.
     *
     * Returning an empty list here instead would have quietly skipped the
     * link-rewriting pass in `repo.js` altogether -- the test would have
     * "passed" while exercising nothing, which is precisely the failure the
     * reveal suite already made once (a test double must
     * model the platform, not the happy path). This is not a real parser and
     * does not pretend to be; it handles the double-quoted attributes this
     * renderer emits, which is all the renderer can produce.
     */
    querySelectorAll(selector) {
      const html = this._html;
      if (typeof html !== "string") { return []; }
      const tag = /^img/.test(selector) ? "img" : "a";
      const attr = tag === "img" ? "src" : "href";
      if (selector.indexOf(tag) !== 0) { return []; }

      const self = this;
      const found = [];
      const pattern = new RegExp("<" + tag + "\\b([^>]*)>", "g");
      let match;
      while ((match = pattern.exec(html)) !== null) {
        const attrs = match[1];
        const value = /(?:^|\s)(?:href|src)="([^"]*)"/.exec(attrs);
        if (!value) { continue; }
        found.push(makeAnchorView(self, match[0], attr, value[1]));
      }
      return found;
    },
    addEventListener(type, fn) { this.listeners[type] = fn; }
  };
}

/**
 * A live view onto one tag inside a parent's `innerHTML` string. Reading an
 * attribute reads the string; writing one rewrites it in place, so assertions
 * afterwards see what a browser would have.
 */
function makeAnchorView(parent, originalTag, primary, primaryValue) {
  let currentTag = originalTag;
  return {
    tagName: primary === "src" ? "IMG" : "A",
    getAttribute(name) {
      const m = new RegExp('(?:^|\\s)' + name + '="([^"]*)"').exec(currentTag);
      return m ? m[1] : null;
    },
    setAttribute(name, value) {
      const escaped = String(value).replace(/"/g, "&quot;");
      const existing = new RegExp('(\\s' + name + '=")[^"]*(")');
      const updated = existing.test(currentTag)
        ? currentTag.replace(existing, "$1" + escaped + "$2")
        : currentTag.replace(/>$/, " " + name + '="' + escaped + '">');
      parent._html = parent._html.replace(currentTag, updated);
      currentTag = updated;
    },
    remove() {
      parent._html = parent._html.replace(currentTag, "");
    },
    get href() { return primary === "href" ? this.getAttribute("href") : undefined; },
    get src() { return primary === "src" ? this.getAttribute("src") : undefined; },
    _primaryValue: primaryValue
  };
}

function makeDocument() {
  const nodes = Object.create(null);
  for (const id of [
    "stars", "forks", "issues", "repo-desc", "repo-license",
    "latest-tag", "asset-list", "readme", "repo-status", "repo", "load-repo"
  ]) {
    nodes[id] = makeElement(id === "load-repo" ? "button" : "div");
  }
  const listeners = Object.create(null);
  return {
    nodes,
    listeners,
    getElementById(id) {
      return Object.prototype.hasOwnProperty.call(nodes, id) ? nodes[id] : null;
    },
    createElement: makeElement,
    createTextNode: (t) => ({ text: String(t) }),
    querySelectorAll: () => [],
    addEventListener(type, fn) { listeners[type] = fn; }
  };
}

function jsonReply(body) {
  return {
    ok: true,
    status: 200,
    json: () => Promise.resolve(body),
    text: () => Promise.resolve("")
  };
}

function textReply(body) {
  return {
    ok: true,
    status: 200,
    text: () => Promise.resolve(body),
    json: () => Promise.resolve({})
  };
}

/**
 * Run the panel end to end against scripted responses, and return the DOM it
 * produced along with anything that escaped as an unhandled rejection.
 */
async function drive(responses) {
  const document = makeDocument();
  const escaped = [];
  const sandbox = {
    window: {
      matchMedia: () => ({ matches: true }), // reduced motion: skip the animations
      requestAnimationFrame: (fn) => fn(0)
    },
    document,
    URL,
    Promise,
    Array,
    Math,
    isFinite,
    String,
    Error,
    console,
    setTimeout,
    fetch(url) {
      for (const [pattern, reply] of responses) {
        if (String(url).indexOf(pattern) !== -1) { return Promise.resolve(reply); }
      }
      return Promise.reject(new Error("no stub for " + url));
    }
  };
  sandbox.window.document = document;
  sandbox.window.fetch = sandbox.fetch;
  vm.createContext(sandbox);
  vm.runInContext(MD_JS, sandbox);   // repo.js reuses MD.safeUrl
  vm.runInContext(REPO_JS, sandbox);

  const onRejection = (e) => escaped.push(e instanceof Error ? e : new Error(String(e)));
  process.on("unhandledRejection", onRejection);
  try {
    // Same three steps a reader's browser takes.
    if (!document.listeners.DOMContentLoaded) {
      throw new Error("repo.js did not register a DOMContentLoaded handler");
    }
    document.listeners.DOMContentLoaded();
    const button = document.getElementById("load-repo");
    if (!button.listeners.click) {
      throw new Error("repo.js did not attach a click handler to the load button");
    }
    const started = Date.now();
    button.listeners.click();
    // `Promise.allSettled` over three stubbed fetches settles in a few ticks.
    for (let i = 0; i < 50; i++) { await new Promise((r) => setTimeout(r, 1)); }
    return { document, escaped, elapsed: Date.now() - started };
  } finally {
    process.removeListener("unhandledRejection", onRejection);
  }
}

const EMPTY = [
  ["api.github.com/repos/tilas01/veilvoice/releases", jsonReply({})],
  ["README.md", textReply("")],
  ["api.github.com/repos/tilas01/veilvoice", jsonReply({})]
];

/** Replace one stubbed response, leaving the rest empty. */
function only(pattern, reply) {
  return [[pattern, reply]].concat(EMPTY);
}

// --- the checks --------------------------------------------------------------

const CHECKS = [];
const check = (name, fn) => CHECKS.push({ name, fn });

check("a download URL that is not https is named but never made clickable", async () => {
  const hostile = [
    "javascript:alert(1)",
    "data:text/html,<script>x</script>",
    "//attacker.example/x",
    "http://insecure.example/x",
    "\\\\attacker.example\\x",
    "VBSCRIPT:msgbox 1",
    "jAvAsCrIpT:alert(1)",
    ""
  ];
  const assets = [{
    name: "veilvoice-x86_64-linux.tar.gz",
    browser_download_url:
      "https://github.com/tilas01/veilvoice/releases/download/v9.9.9/a.tar.gz",
    size: 1048576
  }];
  hostile.forEach((url, i) => {
    assets.push({ name: "hostile-" + i + ".bin", browser_download_url: url, size: 1 });
  });

  const { document } = await drive(
    only("/releases/latest", jsonReply({ tag_name: "v9.9.9", assets }))
  );

  const problems = [];
  let linked = 0;
  for (const li of document.getElementById("asset-list").children) {
    const label = li.children[0];
    const name = label.textContent;
    const href = typeof label.href === "string" ? label.href : null;
    if (name.indexOf("hostile-") === 0) {
      if (href !== null) { problems.push(`${name} became a link to ${href}`); }
      if (label.tagName === "A") { problems.push(`${name} was rendered as an anchor`); }
      if (!name) { problems.push("a refused asset lost its name"); }
    } else if (href) {
      linked++;
      if (href.indexOf("https://github.com/") !== 0) {
        problems.push("the legitimate asset was linked to " + href);
      }
    }
  }
  if (linked !== 1) {
    problems.push(`expected exactly one usable link, got ${linked}`);
  }
  return problems;
});

check("a malformed release payload is survived rather than thrown on", async () => {
  const problems = [];
  const payloads = [
    { tag_name: 42, assets: [{ name: 7, browser_download_url: 9 }] },
    { tag_name: null, assets: "not an array" },
    { assets: [null, undefined, {}, { name: "ok", browser_download_url: null }] },
    { assets: [{ name: "a", browser_download_url: "https://github.com/x", size: "big" }] },
    { assets: [{ name: "a", browser_download_url: "https://github.com/x", size: NaN }] },
    {}
  ];
  for (const payload of payloads) {
    const { escaped } = await drive(only("/releases/latest", jsonReply(payload)));
    for (const e of escaped) {
      problems.push("unhandled rejection: " + e.message);
    }
  }
  return problems;
});

check("the number of list items a single response can create is bounded", async () => {
  const assets = [];
  for (let i = 0; i < 5000; i++) {
    assets.push({
      name: "a" + i,
      browser_download_url: "https://github.com/tilas01/veilvoice/" + i,
      size: 1
    });
  }
  const { document } = await drive(
    only("/releases/latest", jsonReply({ tag_name: "v1", assets }))
  );
  const n = document.getElementById("asset-list").children.length;
  return n > 200 ? [`${n} list items were created from one response`] : [];
});

check("a README's own banner markup is not shown as text", async () => {
  // This is what was live on the site: the project's README opens with a
  // centred banner, GitHub's own idiom for one, and the panel rendered its
  // source code as a paragraph above the word VeilVoice.
  //
  // Nothing was broken in isolation. `markdown.js` escapes raw HTML because
  // that is what makes its output safe to hand to `innerHTML`, and a README is
  // entitled to contain presentational markup. The two correct behaviours met
  // and produced tag soup at the top of the page -- which is the same shape as
  // F-37, a thing that was wrong on every viewport for as long as it existed
  // and that every test passed straight through.
  const { document } = await drive(
    only("README.md", textReply(
      '<!-- a comment that must not appear either -->\n' +
      '<p align="center">\n' +
      '  <picture>\n' +
      '    <source srcset="assets/banner-animated.png">\n' +
      '    <img src="assets/banner.png" alt="VeilVoice">\n' +
      '  </picture>\n' +
      '</p>\n' +
      '\n' +
      '# VeilVoice\n' +
      '\n' +
      'Irreversible voice de-identification.\n' +
      '\n' +
      '```html\n' +
      '<picture>this one is an example and must survive</picture>\n' +
      '```\n'))
  );
  const html = document.getElementById("readme").innerHTML || "";
  const problems = [];
  if (/&lt;picture&gt;|&lt;source|&lt;p align/.test(html.replace(/<code[\s\S]*?<\/code>/g, ""))) {
    problems.push("raw HTML was rendered as escaped text outside a code block");
  }
  if (/a comment that must not appear/.test(html)) {
    problems.push("an HTML comment from the README was shown to the reader");
  }
  if (!/VeilVoice/.test(html)) {
    problems.push("the prose was stripped along with the markup");
  }
  // A fenced example is being shown on purpose and must not be swept up.
  if (!/this one is an example/.test(html)) {
    problems.push("markup inside a fenced block was stripped; it is an example");
  }
  return problems;
});

check("an enormous README is refused, and the reader is told why", async () => {
  const { document } = await drive(
    only("README.md", textReply("x".repeat(4 * 1024 * 1024)))
  );
  const readme = document.getElementById("readme");
  const problems = [];
  if (readme.innerHTML !== null) {
    problems.push("a four-megabyte document was passed to innerHTML");
  }
  if (!/unusually large/.test(readme.textContent)) {
    problems.push("no explanation was shown: " + JSON.stringify(readme.textContent.slice(0, 60)));
  }
  return problems;
});

check("a README shaped to hang the tab renders promptly", async () => {
  // This exact document took eight seconds before the renderer's two
  // quadratics were fixed, on the main thread, from text fetched over the
  // network.
  const { document, elapsed } = await drive(
    only("README.md", textReply("![a](" + "b".repeat(128000)))
  );
  const problems = [];
  if (elapsed > 3000) {
    problems.push(`rendering took ${elapsed} ms -- the quadratic is back`);
  }
  if (document.getElementById("readme").innerHTML === null) {
    problems.push("nothing was rendered at all");
  }
  return problems;
});

check("a repo-relative README link resolves to where it says it points", async () => {
  const { document } = await drive(
    only("README.md", textReply(
      "See [the whitepaper](docs/WHITEPAPER.md) and [up](../../../elsewhere).\n"
    ))
  );
  const html = document.getElementById("readme").innerHTML || "";
  const problems = [];
  if (html.indexOf("https://github.com/tilas01/veilvoice/blob/main/docs/WHITEPAPER.md") === -1) {
    problems.push("an ordinary repo-relative link was not rewritten: " + html.slice(0, 160));
  }
  // The climbing link must not have been rewritten into a URL that normalises
  // somewhere other than where it appears to point.
  if (/blob\/main\/\.\.\//.test(html)) {
    problems.push("a `..` segment was left in an href for the browser to resolve");
  }
  return problems;
});

check("every rendered href is http(s) or a fragment", async () => {
  const { document } = await drive(
    only("README.md", textReply(
      "[a](javascript:alert(1)) [b](data:text/html,x) [c](vbscript:x)\n\n" +
      "[d](https://example.org/) [e](#anchor) [f](docs/AUDIT.md)\n\n" +
      "![g](javascript:alert(2)) ![h](assets/banner.png)\n"
    ))
  );
  const html = document.getElementById("readme").innerHTML || "";
  const problems = [];
  for (const scheme of ["javascript:", "data:", "vbscript:"]) {
    // Present as escaped *text* is the renderer working; present inside an
    // attribute is the bug.
    const inAttribute = new RegExp('(?:href|src)="\\s*' + scheme, "i");
    if (inAttribute.test(html)) {
      problems.push(`${scheme} reached an attribute`);
    }
  }
  if (html.indexOf('href="https://example.org/"') === -1) {
    problems.push("an ordinary absolute link stopped working");
  }
  return problems;
});

// --- harness -----------------------------------------------------------------
//
// Asynchronous, unlike the other suites, because the module under test is. The
// runner awaits whatever `run` returns.

async function run() {
  let failures = 0;
  for (const c of CHECKS) {
    let problems;
    try {
      problems = (await c.fn()) || [];
    } catch (e) {
      problems = ["threw " + e.constructor.name + ": " + e.message];
    }
    if (problems.length) {
      failures++;
      console.log(`FAIL ${c.name}`);
      for (const p of problems) { console.log(`       ${p}`); }
    } else {
      console.log(`ok   ${c.name}`);
    }
  }
  return failures;
}

module.exports = { name: "repository panel, remote data", run };
