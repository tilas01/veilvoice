// SPDX-License-Identifier: GPL-3.0-or-later
//
// Runs every site test. No framework, no dependencies, no package.json — the
// same rule the site itself follows, for the same reason: a test suite that
// pulls a hundred packages off a registry is a supply chain nobody has read.
//
//   node tools/site-tests/run.js
//
// `MD_FUZZ_ROUNDS` sets the size of the randomised Markdown campaign; the
// default is small enough to run on every commit and the audit runs it far
// larger by hand.

"use strict";

const SUITES = [
  require("./characters.test.js"),
  require("./html.test.js"),
  require("./css.test.js"),
  require("./markdown.render.test.js"),
  require("./markdown.hostile.test.js"),
  require("./markdown.complexity.test.js"),
  require("./repo.test.js"),
  require("./reveal.test.js"),
  require("./search.test.js")
];

// `run` may be synchronous or return a promise: the repository-panel suite
// drives an async module, and awaiting a number is harmless for the rest.
(async function () {
  let failures = 0;
  for (const suite of SUITES) {
    console.log(`\n${suite.name}`);
    failures += await suite.run();
  }

  console.log(failures === 0 ? "\nall site tests passed" : `\n${failures} failing check(s)`);
  process.exit(failures === 0 ? 0 : 1);
})();
