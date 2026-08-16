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
  require("./html.test.js"),
  require("./markdown.render.test.js"),
  require("./markdown.hostile.test.js"),
  require("./reveal.test.js")
];

let failures = 0;
for (const suite of SUITES) {
  console.log(`\n${suite.name}`);
  failures += suite.run();
}

console.log(failures === 0 ? "\nall site tests passed" : `\n${failures} failing check(s)`);
process.exit(failures === 0 ? 0 : 1);
