// SPDX-License-Identifier: GPL-3.0-or-later
//
// Renders every document in the hostile corpus, naming each one before it
// starts, and exits.
//
// A separate process for the same reason `complexity-probe.js` is one, and a
// sharper one. A JavaScript loop is synchronous and cannot be interrupted: if
// the renderer stops advancing there is no timer, signal or `await` that will
// get control back, so a suite that rendered these in its own process would
// **hang** rather than fail. That is the worst outcome available, because a
// hung job reads as an infrastructure problem and gets retried, while a
// failing one gets read. Four documents did exactly that before `markdown.js`
// was made to advance on every pass, and `tools/site/wiki_site.py` renders the
// same documents in CI, where a job with no `timeout-minutes` would have sat
// there for six hours.
//
// The name goes out before the render, with `fs.writeSync`, because
// `console.log` to a pipe is buffered and a buffer dies with the process. The
// parent kills this on a timeout and reads the last name it printed, which is
// the document that would not finish.
//
//   node render-probe.js

"use strict";

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ROOT = path.resolve(__dirname, "..", "..");
const sandbox = { window: {} };
vm.createContext(sandbox);
vm.runInContext(
  fs.readFileSync(path.join(ROOT, "website", "js", "markdown.js"), "utf8"), sandbox);
const MD = sandbox.window.MD;

const { DOCUMENTS } = require("./markdown.hostile.test.js");

for (const [name, source] of DOCUMENTS) {
  fs.writeSync(1, "rendering " + name + "\n");
  try {
    MD.render(source);
  } catch (e) {
    fs.writeSync(1, "threw " + name + ": " + e.message + "\n");
  }
}
fs.writeSync(1, "every document finished\n");
