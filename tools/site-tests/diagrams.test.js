// SPDX-License-Identifier: GPL-3.0-or-later
//
// The generated flowcharts must be drawn at a size a person can read.
//
// # The defect this exists to stop coming back
//
// `tools/docs/generate.py` laid a graph out by rank and put every node of one
// rank on a single line, so the canvas was as wide as the busiest rank:
// `veilvoice-core/chain.rs` reached **4490 px**. The drawing then went into the
// page as `width="100%"` with no intrinsic size, inside a 630 px column. The
// browser did the only thing it could and scaled it to **0.147**, which renders
// a 13 px label at under two pixels tall.
//
// That was measured, not guessed: driving the reference page over the DevTools
// protocol reported the SVG's rendered box against its `viewBox`. The same
// measurement on a 390 px viewport reported a `scrollWidth` of 561, so the wide
// drawings were also part of what pushed the reference pages sideways on a
// phone.
//
// So there are three things to hold, and each is checked below:
//
//   1. A rank **wraps**, so the canvas never grows past what a column can show.
//   2. The drawing carries its **own** `width` and `height`, so it renders at
//      its own size rather than being scaled to whatever box it lands in.
//   3. `max-width: 100%` with `height: auto`, so a narrow screen scales it
//      *down* -- the direction that keeps the aspect ratio and loses nothing
//      but room.
//
// A ceiling in pixels is a blunt instrument, and it is the right one here: the
// number is what the reference column actually is, and a diagram wider than the
// column is the bug.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");
const REFERENCE = path.join(ROOT, "website", "reference");
const DIAGRAMS = path.join(ROOT, "assets", "diagrams");

// The reference pages put the drawing in a 630 px column, measured. The
// generator lays out to 640 and a single box may be wider than the budget when
// one name is very long, so the ceiling has headroom for that and for nothing
// else. Before the fix the widest was 4490.
const MAX_CANVAS_W = 900;

function walk(dir, ext, found = []) {
  if (!fs.existsSync(dir)) return found;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, ext, found);
    else if (entry.name.endsWith(ext)) found.push(full);
  }
  return found;
}

/** Every `<svg …>` opening tag that carries `aria-label="flowchart"`. */
function flowchartTags(text) {
  return [...text.matchAll(/<svg\b[^>]*>/g)]
    .map((m) => m[0])
    .filter((tag) => /aria-label="flowchart"/.test(tag));
}

function attr(tag, name) {
  const m = tag.match(new RegExp(`\\b${name}="([^"]*)"`));
  return m ? m[1] : null;
}

function run() {
  let failures = 0;
  const fail = (message) => { failures++; console.log(`  FAIL  ${message}`); };
  const pass = (message) => console.log(`  ok    ${message}`);

  const pages = walk(REFERENCE, ".html");
  if (pages.length === 0) {
    fail("no reference pages were found at all");
    return failures;
  }

  let widest = { width: 0, where: null };
  let drawings = 0;
  const problems = [];

  const check = (tag, where) => {
    drawings++;
    const viewBox = attr(tag, "viewBox");
    if (!viewBox) {
      problems.push(`${where}: a flowchart has no viewBox`);
      return;
    }
    const width = Number(viewBox.split(/\s+/)[2]);
    const height = Number(viewBox.split(/\s+/)[3]);
    if (!Number.isFinite(width) || !Number.isFinite(height)) {
      problems.push(`${where}: viewBox "${viewBox}" is not four numbers`);
      return;
    }
    if (width > widest.width) widest = { width, where };
    if (width > MAX_CANVAS_W) {
      problems.push(
        `${where}: the canvas is ${width}px, past the ${MAX_CANVAS_W}px ceiling` +
        ` -- a rank is not wrapping, and the drawing will be scaled down to fit`);
    }

    // An intrinsic size, or the browser has nothing to render it at but the
    // width of whatever box it lands in. This is the exact attribute whose
    // absence caused the 0.147 scale.
    const w = attr(tag, "width");
    const h = attr(tag, "height");
    if (w === null || h === null || w.endsWith("%") || h === null) {
      problems.push(
        `${where}: a flowchart has no intrinsic width and height` +
        ` (width=${w}, height=${h}), so it is scaled to its container`);
    } else if (Number(w) !== width || Number(h) !== height) {
      problems.push(
        `${where}: width/height (${w}x${h}) disagree with the viewBox` +
        ` (${width}x${height}), so the drawing is scaled before it is drawn`);
    }

    const style = attr(tag, "style") || "";
    if (!/max-width:\s*100%/.test(style) || !/height:\s*auto/.test(style)) {
      problems.push(
        `${where}: a flowchart needs "max-width:100%;height:auto" or it cannot` +
        ` scale down on a narrow screen (style="${style}")`);
    }
  };

  for (const page of pages) {
    const rel = path.relative(ROOT, page).replace(/\\/g, "/");
    for (const tag of flowchartTags(fs.readFileSync(page, "utf8"))) check(tag, rel);
  }

  // The same drawings are written out as files for the repository and the wiki,
  // which is what makes those two show the same picture the site does rather
  // than leaving the layout to GitHub's Mermaid.
  const files = walk(DIAGRAMS, ".svg");
  if (files.length === 0) {
    fail("assets/diagrams/ is empty, so the repository has no drawing to show");
  } else {
    for (const file of files) {
      const rel = path.relative(ROOT, file).replace(/\\/g, "/");
      const text = fs.readFileSync(file, "utf8");
      const tags = flowchartTags(text);
      if (tags.length === 0) {
        // A file with nothing to draw is legitimate and says so.
        if (!/aria-label="no items"/.test(text)) {
          problems.push(`${rel}: neither a flowchart nor an empty one`);
        }
        continue;
      }
      tags.forEach((tag) => check(tag, rel));
    }
  }

  if (problems.length) {
    problems.slice(0, 20).forEach(fail);
    if (problems.length > 20) fail(`… and ${problems.length - 20} more`);
  } else {
    pass(`${drawings} flowcharts carry their own size and scale down, not up`);
    pass(`the widest canvas is ${widest.width}px (${widest.where}), ` +
         `under the ${MAX_CANVAS_W}px ceiling`);
  }

  // The stylesheet's half of the deal: a container that scrolls rather than a
  // page that does. Without it a single box wider than the column takes the
  // whole reference page sideways with it.
  const css = fs.readFileSync(
    path.join(ROOT, "website", "css", "main.css"), "utf8")
    .replace(/\/\*[\s\S]*?\*\//g, "");
  if (!/\.diagram\s*\{[^}]*overflow-x:\s*auto/.test(css)) {
    fail(".diagram must scroll on its own, or a wide drawing scrolls the page");
  } else {
    pass(".diagram scrolls on its own rather than widening the page");
  }

  return failures;
}

module.exports = { name: "flowcharts, drawn at a readable size", run };
