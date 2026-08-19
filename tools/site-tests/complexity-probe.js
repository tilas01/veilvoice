// SPDX-License-Identifier: GPL-3.0-or-later
//
// Renders one adversarial document and prints how long it took, in
// milliseconds, on stdout.
//
// A separate process on purpose. A regular-expression match is *synchronous*
// and cannot be interrupted: once the engine starts backtracking there is no
// timer, no signal and no `await` that will get control back. A suite that
// measured these in-process would therefore hang rather than fail when a
// quadratic reappeared -- which is the worst outcome, because a hung CI job
// looks like an infrastructure problem and gets retried, while a failing one
// gets read. Measured here and killed from outside, a catastrophic case becomes
// a timeout the parent can report as the failure it is.
//
//   node complexity-probe.js <shape> <size>

"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ROOT = path.resolve(__dirname, "..", "..");
const sandbox = { window: {} };
vm.createContext(sandbox);
vm.runInContext(fs.readFileSync(path.join(ROOT, "website", "js", "markdown.js"), "utf8"), sandbox);
const MD = sandbox.window.MD;

/**
 * Every one of these is a real shape, found by reading the patterns for two
 * runs that can both match the same character, or for an unbounded scan
 * repeated at every position in the document.
 */
const SHAPES = {
  "image target that never closes": (n) => "![a](" + "b".repeat(n),
  "link target that never closes": (n) => "[a](" + "b".repeat(n),
  "an opening bracket at every position": (n) => "[".repeat(n) + "]".repeat(n),
  "a bracket then an open parenthesis, repeated": (n) => "[".repeat(n) + "](".repeat(n),
  "inline code that never closes": (n) => "`" + "a".repeat(n),
  "a backtick at every position": (n) => "`x".repeat(Math.floor(n / 2)),
  "an unterminated string in a code block": (n) => '```rust\nlet s = "' + "a".repeat(n) + "\n```",
  "a quote at every position in a code block": (n) => "```rust\n" + '"x'.repeat(Math.floor(n / 2)) + "\n```",
  "emphasis markers only": (n) => "*".repeat(n),
  "an underscore at every position": (n) => "_x".repeat(Math.floor(n / 2)),
  "a table row at every line": (n) => "|a|b|\n|-|-|\n" + "|x|y|\n".repeat(Math.floor(n / 6)),
  "a list item at every line": (n) => "- x\n".repeat(Math.floor(n / 4)),
  "a heading at every line": (n) => "# x\n".repeat(Math.floor(n / 4)),
  "a parenthesis at every position": (n) => "](".repeat(Math.floor(n / 2)),
  "an exclamation bracket at every position": (n) => "![".repeat(Math.floor(n / 2))
};

module.exports = { SHAPES };

if (require.main === module) {
  const shape = process.argv[2];
  const size = Number(process.argv[3]);
  const build = Object.prototype.hasOwnProperty.call(SHAPES, shape) ? SHAPES[shape] : null;
  if (!build || !isFinite(size)) {
    console.error("usage: complexity-probe.js <shape> <size>");
    process.exit(2);
  }
  const source = build(size);

  // Median of three: one sample on a shared machine is noise.
  const samples = [];
  for (let i = 0; i < 3; i++) {
    const started = process.hrtime.bigint();
    MD.render(source);
    samples.push(Number(process.hrtime.bigint() - started) / 1e6);
  }
  samples.sort((a, b) => a - b);
  process.stdout.write(String(samples[1]));
}
