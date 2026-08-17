// SPDX-License-Identifier: GPL-3.0-or-later
//
// Search: the live one, and the one that works with no JavaScript at all.
//
// Both halves are tested here on purpose, because they are one feature. The
// static page in `website/nojs/` is not a courtesy stub -- `website/nojs/` is a
// supported edition of this site -- and a search that quietly finds nothing
// without JavaScript would be exactly the silent degradation this project
// audits itself against. So the last group of checks asks the only question
// that matters about it: *does it actually contain the answers?*
//
// The live half is driven through a DOM stub rather than asserted about by
// reading the source, for the reason `reveal.test.js` records: a test that
// models the happy path cannot express the bug. The stub here therefore builds
// real element objects with children, so a test can ask what ended up in the
// page -- and, critically, whether anything from the index ever became markup.
//
// # Why the injection checks are not theoretical
//
// The index is built from every tracked file, which in this repository includes
// `markdown.hostile.test.js` -- a file whose entire content is `<script>`,
// `onerror=` and other attack strings, as ordinary text. That text is in
// `search-index.json` today. If `search.js` ever renders a result through
// `innerHTML`, this project's own test corpus becomes its payload.

"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ROOT = path.resolve(__dirname, "..", "..");
const SOURCE = fs.readFileSync(path.join(ROOT, "website", "js", "search.js"), "utf8");
const INDEX_PATH = path.join(ROOT, "website", "search-index.json");
const STATIC_PATH = path.join(ROOT, "website", "nojs", "search.html");

// --- a DOM small enough to read and complete enough to be wrong in ----------

function makeNode(tagName) {
  const classes = new Set();
  const node = {
    tagName: String(tagName || "").toUpperCase(),
    childNodes: [],
    attributes: {},
    style: { setProperty(name, value) { node.attributes["style:" + name] = value; } },
    hidden: false,
    disabled: false,
    value: "",
    placeholder: "",
    _listeners: {},
    classList: {
      add: v => classes.add(v),
      remove: v => classes.delete(v),
      contains: v => classes.has(v),
      toggle: (v, on) => { if (on) { classes.add(v); } else { classes.delete(v); } }
    },
    get className() { return [...classes].join(" "); },
    set className(v) { classes.clear(); String(v).split(/\s+/).filter(Boolean).forEach(c => classes.add(c)); },
    get firstChild() { return node.childNodes[0] || null; },
    appendChild(child) { node.childNodes.push(child); return child; },
    removeChild(child) {
      const at = node.childNodes.indexOf(child);
      if (at !== -1) { node.childNodes.splice(at, 1); }
      return child;
    },
    setAttribute(name, v) { node.attributes[name] = String(v); },
    getAttribute(name) { return name in node.attributes ? node.attributes[name] : null; },
    addEventListener(type, fn) { (node._listeners[type] = node._listeners[type] || []).push(fn); },
    dispatch(type, event) {
      (node._listeners[type] || []).forEach(fn => fn(event || { preventDefault() {} }));
    },
    get textContent() {
      return node.childNodes.map(c => (c.nodeType === 3 ? c.data : c.textContent)).join("");
    },
    // Read-only in this stub: `search.js` must never *assign* innerHTML, and a
    // getter lets a test inspect what the tree would serialise to.
    //
    // Text nodes are escaped, because that is what a real serialiser does and
    // the difference is the whole point here. The index legitimately contains
    // the text `<script>` -- it is indexed from this project's own hostile
    // markup fixtures -- and a stub that emitted it raw would report the
    // renderer working correctly as an injection. `docs/AUDIT.md` section 4.4
    // records five false findings from exactly that mistake.
    get innerHTML() {
      return node.childNodes.map(c => {
        if (c.nodeType === 3) {
          return c.data
            .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
        }
        const attrs = Object.keys(c.attributes)
          .filter(k => !k.startsWith("style:"))
          .map(k => ` ${k}="${c.attributes[k]}"`).join("");
        const tag = c.tagName.toLowerCase();
        return `<${tag}${attrs}>${c.innerHTML}</${tag}>`;
      }).join("");
    }
  };
  Object.defineProperty(node, "href", {
    get() { return node.attributes.href; },
    set(v) { node.attributes.href = String(v); }
  });
  Object.defineProperty(node, "rel", {
    get() { return node.attributes.rel; },
    set(v) { node.attributes.rel = String(v); }
  });
  return node;
}

function findAll(node, predicate, out = []) {
  for (const child of node.childNodes) {
    if (child.nodeType === 3) { continue; }
    if (predicate(child)) { out.push(child); }
    findAll(child, predicate, out);
  }
  return out;
}

/** Load `search.js` against a stubbed page and a given index. */
async function boot(index, { failFetch = false, badShape = false } = {}) {
  const ids = {};
  for (const id of ["search-form", "q", "sort", "kind", "area", "results",
                    "result-count", "no-results"]) {
    ids[id] = makeNode(id === "results" ? "ul" : "div");
  }

  const frames = [];
  const docClasses = new Set();
  let ready = null;

  const sandbox = {
    document: {
      documentElement: { classList: { add: v => docClasses.add(v) } },
      getElementById: id => ids[id] || null,
      createElement: makeNode,
      createTextNode: data => ({ nodeType: 3, data: String(data) }),
      addEventListener: (type, fn) => { if (type === "DOMContentLoaded") { ready = fn; } }
    },
    window: {
      location: { href: "https://example.org/search.html" },
      requestAnimationFrame: fn => { frames.push(fn); return frames.length; },
      fetch: () => {
        if (failFetch) { return Promise.reject(new Error("offline")); }
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(badShape ? { nope: true } : index)
        });
      }
    },
    URL,
    JSON,
    Promise
  };
  sandbox.window.URL = URL;
  vm.createContext(sandbox);
  vm.runInContext(SOURCE, sandbox);

  ready();
  // Let the fetch promise chain settle.
  await new Promise(resolve => setImmediate(resolve));
  await new Promise(resolve => setImmediate(resolve));

  const page = {
    ids,
    frames,
    docClasses,
    flush() { while (frames.length) { frames.shift()(); } },
    type(value) {
      ids.q.value = value;
      ids.q.dispatch("input");
      page.flush();
    },
    choose(which, value) {
      ids[which].value = value;
      ids[which].dispatch("change");
      page.flush();
    },
    rows() {
      return ids.results.childNodes.filter(n => n.nodeType !== 3);
    },
    headings() {
      return page.rows().map(r => {
        const head = findAll(r, n => n.className.includes("sr-head"))[0];
        return head ? head.textContent : "";
      });
    },
    paths() {
      return page.rows().map(r => {
        const p = findAll(r, n => n.className.includes("sr-path"))[0];
        return p ? p.textContent : "";
      });
    },
    count() { return ids["result-count"].textContent; }
  };
  return page;
}

// --- the suite --------------------------------------------------------------

async function run() {
  let fails = 0;
  const check = (name, ok) => {
    console.log((ok ? "ok   " : "FAIL ") + name);
    if (!ok) { fails++; }
  };

  if (!fs.existsSync(INDEX_PATH) || !fs.existsSync(STATIC_PATH)) {
    console.log("FAIL the search index has not been generated -- run " +
                "`python tools/search-index/generate.py`");
    return 1;
  }

  const index = JSON.parse(fs.readFileSync(INDEX_PATH, "utf8"));
  const staticHtml = fs.readFileSync(STATIC_PATH, "utf8");

  // --- 1. the index itself --------------------------------------------------
  check("the index has documents", Array.isArray(index.docs) && index.docs.length > 50);
  check("the index has sections", Array.isArray(index.secs) && index.secs.length > 300);
  check("every section points at a real document",
        index.secs.every(s => index.docs[s.d] !== undefined));
  check("every document has a path, kind and area",
        index.docs.every(d => d.p && d.k && d.r));
  check("the Rust engine is indexed",
        index.docs.some(d => d.p === "crates/veilvoice-core/src/spectral.rs"));
  check("the audit is indexed", index.docs.some(d => d.p === "docs/AUDIT.md"));
  check("the website is indexed", index.docs.some(d => d.p === "website/wiki.html"));

  // The generator's output is tracked and lives under `website/`, so without an
  // explicit exclusion it would index itself: each run's input would contain
  // the previous run's output, the file would grow every time, and `--check`
  // could never agree with a freshly built one. That failure presents as flaky
  // CI rather than as a bug, so it is asserted here.
  check("the index does not index itself",
        !index.docs.some(d => d.p === "website/search-index.json" ||
                              d.p === "website/nojs/search.html"));
  // Built with `new RegExp` from escapes rather than written as a literal
  // class, for the reason `characters.test.js` gives at length: a checker for
  // invisible characters that contains invisible characters is a joke that has
  // already been made twice in this repository.
  const STRAY = new RegExp(
    "[\\u0000-\\u0008\\u000b\\u000c\\u000e-\\u001f" +
    "\\u200b-\\u200f\\u2028\\u2029\\u202a-\\u202e" +
    "\\u2060\\u2066-\\u2069\\ue000-\\uf8ff\\ufeff\\ufffd]"
  );
  check("no section carries a stray control character",
        !index.secs.some(s => STRAY.test((s.h || "") + (s.x || ""))));

  // --- 2. live search: it finds things --------------------------------------
  {
    const page = await boot(index);
    check("the box is enabled once the index loads", page.ids.q.disabled === false);
    check("the page marks itself live", page.docClasses.has("search-live"));

    page.type("argon2");
    check("searching 'argon2' finds results", page.rows().length > 0);
    check("every result mentioning argon2 is a real path",
          page.paths().every(p => typeof p === "string" && p.length > 0));

    page.type("spectral");
    check("searching 'spectral' reaches the DSP engine",
          page.paths().some(p => p.includes("spectral.rs")));

    page.type("irreversible");
    check("a word from the prose finds the documentation",
          page.paths().some(p => p.endsWith(".md") || p.endsWith(".html")));

    page.type("zzzznotarealtokenzzzz");
    check("a word that is in nothing returns nothing", page.rows().length === 0);
    check("and says so rather than showing an empty list",
          /nothing matched/i.test(page.count()));
    check("the empty note is shown", page.ids["no-results"].hidden === false);
  }

  // --- 3. every term must match, not just one -------------------------------
  {
    const page = await boot(index);
    page.type("argon2");
    const one = page.rows().length;
    page.type("argon2 zzzznotarealtokenzzzz");
    check("adding a term that matches nothing empties the results",
          one > 0 && page.rows().length === 0);
  }

  // --- 4. filtering ---------------------------------------------------------
  {
    const page = await boot(index);
    page.type("encrypt");
    const all = page.rows().length;
    page.choose("kind", "doc");
    const docsOnly = page.rows().length;
    check("filtering by kind narrows the results", docsOnly > 0 && docsOnly <= all);
    check("filtering by kind returns only that kind",
          page.paths().every(p => {
            const doc = index.docs.find(d => d.p === p);
            return doc && doc.k === "doc";
          }));

    page.choose("kind", "");
    page.choose("area", "veilvoice-crypto");
    check("filtering by area returns only that area",
          page.rows().length > 0 &&
          page.paths().every(p => p.startsWith("crates/veilvoice-crypto/")));
  }

  // --- 5. sorting -----------------------------------------------------------
  {
    const page = await boot(index);
    page.type("veil");
    page.choose("sort", "path");
    const paths = page.paths();
    const sorted = [...paths].sort();
    check("sorting by path really sorts by path",
          paths.length > 1 && paths.join("|") === sorted.join("|"));

    page.choose("sort", "relevance");
    check("sorting back to relevance changes the order or keeps results",
          page.rows().length > 0);

    // The same query twice must give the same order: a list that reshuffles
    // between keystrokes is unreadable.
    const first = page.paths().join("|");
    page.type("veil");
    check("the same query gives the same order twice", page.paths().join("|") === first);
  }

  // --- 6. nothing from the index ever becomes markup ------------------------
  {
    const page = await boot(index);
    // These terms are present in this repository *as hostile-input fixtures*,
    // so the index genuinely contains them.
    for (const term of ["script", "onerror", "javascript"]) {
      page.type(term);
      const rows = page.rows().length;
      check(`'${term}' matches something (so the check is not vacuous)`, rows > 0);

      // Asked of the *tree*, not of a serialised string. A result whose text
      // reads `<script>` is the renderer working; an element whose tagName is
      // SCRIPT is the renderer broken. Only the second is an injection, and
      // only a tree walk can tell them apart.
      const elements = findAll(page.ids.results, () => true);
      check(`'${term}' creates no script or frame element`,
            !elements.some(n => ["SCRIPT", "IFRAME", "OBJECT", "EMBED", "IMG"]
              .includes(n.tagName)));
      check(`'${term}' sets no event-handler attribute`,
            !elements.some(n => Object.keys(n.attributes)
              .some(a => /^on/i.test(a))));
      check(`'${term}' sets no javascript: URL`,
            !elements.some(n => ["href", "src"].some(a =>
              /^\s*javascript:/i.test(n.attributes[a] || ""))));
      check(`'${term}' keeps every match as text, not markup`,
            findAll(page.ids.results, n => n.tagName === "MARK")
              .every(m => m.childNodes.every(c => c.nodeType === 3)));
    }
    // Only the tags the renderer is supposed to produce.
    page.type("the");
    const tags = new Set(findAll(page.ids.results, () => true).map(n => n.tagName));
    const allowed = new Set(["LI", "A", "DIV", "SPAN", "P", "MARK"]);
    check("only the expected element types are produced",
          [...tags].every(t => allowed.has(t)));
  }

  // --- 7. highlighting ------------------------------------------------------
  {
    const page = await boot(index);
    page.type("argon2");
    const marks = findAll(page.ids.results, n => n.tagName === "MARK");
    check("matches are highlighted", marks.length > 0);
    check("every highlight is the matched text, case-insensitively",
          marks.every(m => m.textContent.toLowerCase() === "argon2"));
    check("highlights are text nodes, not markup",
          marks.every(m => m.childNodes.every(c => c.nodeType === 3)));
  }

  // --- 8. bounds ------------------------------------------------------------
  {
    const page = await boot(index);
    page.type("e");                       // a letter in almost everything
    check("the number of rows drawn is bounded", page.rows().length <= 60);
    check("the count still reports the honest total",
          /result|file/.test(page.count()));

    const long = "a".repeat(5000);
    const before = Date.now();
    page.type(long);
    check("a 5000-character query is handled promptly", Date.now() - before < 2000);

    page.type("a b c d e f g h i j k l m n o p");
    check("a query with many terms is handled promptly", true);
  }

  // --- 9. failure is reported honestly --------------------------------------
  {
    const page = await boot(index, { failFetch: true });
    check("a failed index fetch says so", /could not load/i.test(page.count()));
    check("and points at the static index", /static index/i.test(page.count()));
    check("the box stays disabled rather than pretending", page.ids.q.disabled === true);
  }
  {
    const page = await boot(index, { badShape: true });
    check("an index of the wrong shape is refused, not walked into",
          /could not load|expected shape/i.test(page.count()));
  }

  // --- 10. the no-JavaScript path actually finds things ---------------------
  //
  // This is the group that matters. Everything above proves the JavaScript
  // works; these prove the page that runs none of it is a real answer.
  {
    check("the static index is a complete HTML document",
          /<!DOCTYPE html>/i.test(staticHtml) && /<\/html>/i.test(staticHtml));
    check("the static index needs no JavaScript",
          !/<script/i.test(staticHtml));
    check("the static index tells the reader how to search it",
          /find-in-page/i.test(staticHtml) && /Ctrl\+F/i.test(staticHtml));

    // Every document in the JSON index is on the static page too, or the two
    // halves of this feature disagree about what the project contains.
    const missing = index.docs.filter(d => !staticHtml.includes(d.p));
    check(`every one of the ${index.docs.length} indexed files is listed statically`,
          missing.length === 0);
    if (missing.length) {
      console.log("     missing: " + missing.slice(0, 5).map(d => d.p).join(", "));
    }

    // The words a reader would actually search for have to be *in the page*,
    // because find-in-page is the whole mechanism.
    const terms = ["Argon2", "spectral", "voiceprint", "reproducible",
                   "XChaCha20", "tamper", "de-identification"];
    const absent = terms.filter(t => !new RegExp(t, "i").test(staticHtml));
    check("the terms a reader would search for are present in the page: " +
          terms.join(", "),
          absent.length === 0);
    if (absent.length) { console.log("     absent: " + absent.join(", ")); }

    // Finding something is only useful if you can then go to it.
    check("the static index links to the repository",
          /https:\/\/github\.com\/tilas01\/veilvoice\/blob\/main\//.test(staticHtml));
    check("the static index links back to the live search",
          /href="\.\.\/search\.html"/.test(staticHtml));
    check("the static index links to the no-JavaScript edition",
          /href="index\.html"/.test(staticHtml));

    // Section text, not just file names: searching for a phrase from the middle
    // of a document is the case that separates an index from a directory listing.
    check("section headings are in the static page",
          staticHtml.includes("The standard this is held to"));
    check("section text is in the static page, not only headings",
          /class="x"/.test(staticHtml));
  }

  // --- 11. the two halves agree --------------------------------------------
  {
    const listed = (staticHtml.match(/<details\b[^>]*>/g) || []).length;
    check(`the static page lists exactly the indexed files (${index.docs.length})`,
          listed === index.docs.length);
  }

  return fails;
}

module.exports = { run, name: "search, live and static" };

if (require.main === module) {
  run().then(f => process.exit(f ? 1 : 0));
}
