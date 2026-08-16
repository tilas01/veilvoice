// SPDX-License-Identifier: GPL-3.0-or-later
//
// Rendering-correctness tests for the site's Markdown renderer.
//
// Separate from the hostile-input suite because the failure being guarded
// against here is different in kind. Nothing below is a security bug; they are
// all cases where the page would display something that is *not what the
// source says*. On a project whose entire pitch is "every claim is checkable,
// go and read the source", publishing altered source on the front page is its
// own kind of serious.

"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ROOT = path.resolve(__dirname, "..", "..");
const sandbox = { window: {} };
vm.createContext(sandbox);
vm.runInContext(fs.readFileSync(path.join(ROOT, "website", "js", "markdown.js"), "utf8"), sandbox);
const MD = sandbox.window.MD;

const CASES = [
  // --- the placeholder regression -----------------------------------------
  // Finished markup is parked while later passes run. The placeholder used to
  // be a NUL-delimited *decimal index*, which the number highlighter (\b\d+\b)
  // then wrapped in a span — after which the un-parking pass no longer
  // recognised it and the parked content was dropped entirely. Every string
  // literal in every code block on the site rendered as a stray digit.
  {
    name: "a string literal in a code block survives the number highlighter",
    source: '```rust\nlet s = "hello";\n```',
    want: ['tok-str', '&quot;hello&quot;'],
    reject: ['tok-num">0<']
  },
  {
    name: "a string and a number in one line both survive",
    source: '```rust\nlet n = 42; let s = "x";\n```',
    want: ['tok-num">42<', 'tok-str', '&quot;x&quot;']
  },
  {
    name: "a comment in a shell block survives",
    source: '```bash\ncargo build   # deliberately slow\n```',
    want: ['tok-com', 'deliberately slow']
  },
  {
    name: "several parked items in one block all come back",
    source: '```rust\nlet a = "one"; let b = "two"; let c = "three";\n```',
    want: ['&quot;one&quot;', '&quot;two&quot;', '&quot;three&quot;']
  },
  // Adjacent placeholders used to share a delimiter, so alternate items were
  // silently lost.
  {
    name: "adjacent inline code spans both survive",
    source: '`alpha``beta`',
    want: ['<code>alpha</code>', '<code>beta</code>']
  },
  {
    name: "a digit in ordinary prose is not mistaken for a placeholder",
    source: 'Section 0 has `code` in it, and section 1 does too.',
    want: ['Section 0 has', '<code>code</code>', 'section 1 does too']
  },
  {
    name: "a link, a bare digit and another link all render",
    source: '[a](http://x.test) 0 [b](http://y.test)',
    want: ['>a</a>', '>b</a>', '> 0 <']
  },

  // --- ordinary rendering --------------------------------------------------
  { name: "headings", source: '## Title', want: ['<h2>Title</h2>'] },
  { name: "bold and italic", source: 'a **b** and *c*', want: ['<strong>b</strong>', '<em>c</em>'] },
  { name: "an external link gets rel=noopener", source: '[x](https://example.com)',
    want: ['rel="noopener noreferrer"'] },
  { name: "a relative link does not", source: '[x](./docs/AUDIT.md)',
    want: ['href="./docs/AUDIT.md"'], reject: ['rel='] },
  { name: "a table renders as a table", source: 'a | b\n--- | ---\n1 | 2',
    want: ['<table>', '<th>a</th>', '<td>1</td>'] },
  { name: "a blockquote renders", source: '> quoted', want: ['<blockquote>', 'quoted'] },
  { name: "a list renders", source: '- one\n- two', want: ['<ul>', '<li>one</li>'] },
  { name: "an ordered list renders", source: '1. one\n2. two', want: ['<ol>', '<li>two</li>'] },
  { name: "an image renders", source: '![alt](assets/x.png)',
    want: ['<img src="assets/x.png" alt="alt">'] },

  // --- nested placeholders --------------------------------------------------
  // A link whose label is inline code parks the code, then parks an anchor
  // whose label *is* that placeholder. One un-parking pass left the inner one
  // in the output as a private-use character, which browsers draw as nothing:
  // every such link in the README rendered as an empty link, so
  // "see [`docs/AUDIT.md`](docs/AUDIT.md)." came out as "see .".
  {
    name: "a link whose label is inline code keeps its label",
    source: 'See [`docs/AUDIT.md`](docs/AUDIT.md).',
    want: ['<a href="docs/AUDIT.md"><code>docs/AUDIT.md</code></a>'],
    // The broken output was `<a href="docs/AUDIT.md"></a>`, so the quote has to
    // be part of the pattern — `></a>` alone also matches `</code></a>`.
    reject: ['"></a>']
  },
  {
    name: "an image nested in a link keeps both",
    source: '[![alt](a.png)](https://example.com)',
    want: ['<img src="a.png" alt="alt">', '<a href="https://example.com"']
  },
  {
    name: "inline code in a heading and a list item survives",
    source: '## The `veilvoice` binary\n- run `veilvoice lock set`',
    want: ['<code>veilvoice</code>', '<code>veilvoice lock set</code>']
  },
  {
    name: "bold around an inline-code link keeps everything",
    source: '**[`HANDOFF.md`](HANDOFF.md)**',
    want: ['<strong>', '<code>HANDOFF.md</code>', 'href="HANDOFF.md"']
  },

  // A protocol-relative target looks relative and behaves external, so it is
  // refused outright rather than emitted without rel="noopener noreferrer".
  { name: "a protocol-relative link is refused", source: '[label](//evil.example)',
    want: ['label'], reject: ['<a href'] },
  { name: "a backslash-prefixed target is refused", source: '[label](\\\\evil.example)',
    want: ['label'], reject: ['<a href'] },

  // Ordinary relative Markdown links, which the old scheme test rejected.
  { name: "a bare relative link renders", source: '[whitepaper](docs/WHITEPAPER.md)',
    want: ['href="docs/WHITEPAPER.md"', '>whitepaper</a>'] },
  { name: "an anchor-only link renders", source: '[top](#what)', want: ['href="#what"'] },
  { name: "a mailto link is still refused", source: '[mail](mailto:someone@example.com)',
    want: ['mail'], reject: ['<a href'] },

  // --- the real README ------------------------------------------------------
  { name: "the project README renders without losing its code", source: null }
];

function run() {
  let failures = 0;

  for (const test of CASES) {
    if (test.source === null) { continue; }
    const html = MD.render(test.source);
    const missing = (test.want || []).filter(w => !html.includes(w));
    const present = (test.reject || []).filter(r => html.includes(r));
    if (missing.length || present.length) {
      failures++;
      console.log(`FAIL ${test.name}`);
      if (missing.length) { console.log(`     missing: ${missing.join(" | ")}`); }
      if (present.length) { console.log(`     should not contain: ${present.join(" | ")}`); }
      console.log(`     output: ${html.slice(0, 240)}`);
    }
  }

  // The renderer's actual job: the project's own README, which is what
  // js/repo.js fetches and injects. Every fenced block in the source must come
  // back with its contents intact.
  const readme = fs.readFileSync(path.join(ROOT, "README.md"), "utf8");
  const html = MD.render(readme);

  // Nothing internal may survive into the page, and no link may come out with
  // no text in it. Both were true of the deployed site.
  const stray = [...html].filter(c => c.charCodeAt(0) >= 0xe000 && c.charCodeAt(0) <= 0xf8ff);
  if (stray.length) {
    failures++;
    console.log(`FAIL the README leaves ${stray.length} placeholder character(s) in the output`);
  }
  const empty = html.match(/<a [^>]*><\/a>/g) || [];
  if (empty.length) {
    failures++;
    console.log(`FAIL the README renders ${empty.length} link(s) with no text: ${empty[0]}`);
  }

  const fences = readme.match(/```[\s\S]*?```/g) || [];
  let lost = 0;
  for (const fence of fences) {
    for (const literal of fence.match(/"[^"\n]{3,40}"/g) || []) {
      const escaped = literal.replace(/"/g, "&quot;");
      if (!html.includes(escaped)) {
        if (lost === 0) { console.log("FAIL the README loses code-block content"); }
        if (lost < 5) { console.log(`     dropped: ${literal}`); }
        lost++;
      }
    }
  }
  if (lost) { failures++; }
  console.log(`  ${CASES.length - 1} rendering cases, ${fences.length} README code blocks`);

  return failures;
}

module.exports = { run, name: "markdown renderer, rendering correctness" };

if (require.main === module) { process.exit(run() ? 1 : 0); }
