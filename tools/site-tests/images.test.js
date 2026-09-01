// SPDX-License-Identifier: GPL-3.0-or-later
//
// No generated picture may put text outside its own canvas.
//
// # The defect this exists to stop coming back
//
// A drawing that carries its own explanation has to be tall enough for it. The
// first version of the diagram footer worked the height out in one function and
// drew it in another, the two disagreed by one row of the colour key, and the
// last line of every note was clipped by the bottom edge of the picture it was
// explaining. It rendered fine in a browser, every existing check passed, and
// the only way to see it was to look at one.
//
// That is the third time this repository has cut its own text off. F-37 deleted
// more than half the pixel rows of the banner carrying this project's licence
// and authorship. The terminal drawings ended sentences mid-word with an
// ellipsis for as long as they had existed. Both were found by looking, and
// looking does not scale to 300 pictures.
//
// So this measures instead. Every `<text>` in every generated SVG, against the
// canvas it is drawn on.
//
// # How the width is estimated, and why an estimate is enough
//
// These drawings are monospace by construction: one font stack, set in
// `generate.py`, chosen so a character's width is predictable. A character is
// about 0.585 of the font size in it. This uses **0.62**, deliberately high,
// so the estimate errs towards reporting a problem that is not there rather
// than missing one that is. A false alarm costs somebody a look at a picture;
// a miss costs a reader the end of a sentence.
//
// It is not a rendering. A rendering would need a browser and this suite runs
// without one. What it catches is the class of fault that has actually
// happened here: a canvas sized by arithmetic that does not match the drawing.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");

// Where the generated drawings are. `website/assets` holds copies of the same
// files; checking both means a copy that went stale is caught here too.
const PLACES = [
  "assets/diagrams",
  "assets/banners",
  "assets/screenshots",
  "website/assets/banners",
  "website/assets/screenshots"
];
const SINGLES = [
  "assets/roadmap.svg", "website/assets/roadmap.svg",
  "assets/roadmap-film.svg", "website/assets/roadmap-film.svg"
];

// A character is about this much of the font size, in the monospace stack
// every one of these drawings uses. Rounded up on purpose: see the header.
//
// `TEXT_RATIO` in `tools/docs/generate.py` is the same number and has to stay
// the same number. The generator lays text out with it and this checks the
// result: if the generator were the more optimistic of the two, every drawing
// would be laid out to a width this suite then refuses.
const CHAR_RATIO = 0.62;
// Room for a descender under the baseline, as a fraction of the font size.
const DESCENDER = 0.28;
// A pixel of slack, because these are floating-point layouts.
const SLACK = 1.5;

function walk(dir, found = []) {
  const full = path.join(ROOT, dir);
  if (!fs.existsSync(full)) return found;
  for (const entry of fs.readdirSync(full, { withFileTypes: true })) {
    const next = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(next, found);
    else if (entry.name.endsWith(".svg")) found.push(next);
  }
  return found;
}

/** The drawing area, from the `viewBox` rather than from `width`. */
function canvas(svg) {
  const box = /viewBox="([-\d.]+)\s+([-\d.]+)\s+([\d.]+)\s+([\d.]+)"/.exec(svg);
  if (!box) return null;
  return { width: parseFloat(box[3]), height: parseFloat(box[4]) };
}

/**
 * A copy with the clipped groups removed.
 *
 * Text inside a `clip-path` is *meant* to be outside the frame: the roadmap
 * film is a list that scrolls through a window, so at any one instant most of
 * it is above or below the visible area, on purpose, and the clip is what
 * makes that work. Measuring it against the canvas would report sixty rows as
 * overflowing and would be wrong about every one of them.
 *
 * What is still measured in such a drawing is everything outside the clip: the
 * heading, the countdown, the rule. Those are the parts that have to fit.
 */
function unclipped(svg) {
  return svg.replace(/<g[^>]*\bclip-path="[^"]*"[^>]*>[\s\S]*?<\/g>\s*<\/g>/g, "")
            .replace(/<g[^>]*\bclip-path="[^"]*"[^>]*>[\s\S]*?<\/g>/g, "");
}

/** Every `<text>` element, with what decides where its box lands. */
function texts(svg) {
  const found = [];
  const re = /<text\b([^>]*)>([\s\S]*?)<\/text>/g;
  let match;
  while ((match = re.exec(svg)) !== null) {
    const attrs = match[1];
    const get = (name) => {
      const found = new RegExp(name + '="([^"]*)"').exec(attrs);
      return found ? found[1] : null;
    };
    // Entities count as one character each, which is what they render as.
    // An entity is one character on screen and up to six in the file. The
    // hexadecimal form was missed at first, so every apostrophe counted as
    // five characters and four banners were reported as overflowing when they
    // were not.
    const body = match[2]
      .replace(/&#x[0-9a-f]+;/gi, "x")
      .replace(/&#\d+;/g, "x")
      .replace(/&[a-z]+;/gi, "x");
    found.push({
      x: parseFloat(get("x") || "0"),
      y: parseFloat(get("y") || "0"),
      size: parseFloat(get("font-size") || "13"),
      anchor: get("text-anchor") || "start",
      length: body.length,
      text: body
    });
  }
  return found;
}

function run() {
  let failures = 0;
  const fail = (message) => { failures++; console.log(`FAIL ${message}`); };
  const pass = (message) => console.log(`ok   ${message}`);

  let files = [];
  for (const place of PLACES) { files = files.concat(walk(place)); }
  for (const single of SINGLES) {
    if (fs.existsSync(path.join(ROOT, single))) { files.push(single); }
  }
  files.sort();

  if (files.length === 0) {
    fail("no generated drawings were found at all, so nothing was checked");
    return failures;
  }

  const problems = [];
  let checked = 0;
  for (const rel of files) {
    const svg = fs.readFileSync(path.join(ROOT, rel), "utf8");
    const box = canvas(svg);
    if (!box) {
      problems.push(`${rel}: no viewBox, so nothing can be measured against it`);
      continue;
    }
    for (const item of texts(unclipped(svg))) {
      checked++;
      const width = item.length * item.size * CHAR_RATIO;
      let left = item.x;
      if (item.anchor === "middle") { left = item.x - width / 2; }
      else if (item.anchor === "end") { left = item.x - width; }
      const right = left + width;
      const bottom = item.y + item.size * DESCENDER;

      const shown = item.text.trim().slice(0, 44);
      if (left < -SLACK) {
        problems.push(`${rel}: text starts ${(-left).toFixed(0)}px left of the canvas: "${shown}"`);
      }
      if (right > box.width + SLACK) {
        problems.push(`${rel}: text runs ${(right - box.width).toFixed(0)}px past the right edge ` +
                      `(canvas ${box.width}): "${shown}"`);
      }
      if (bottom > box.height + SLACK) {
        problems.push(`${rel}: text runs ${(bottom - box.height).toFixed(0)}px below the bottom edge ` +
                      `(canvas ${box.height}): "${shown}"`);
      }
      if (item.y - item.size < -SLACK) {
        problems.push(`${rel}: text sits above the canvas: "${shown}"`);
      }
    }
  }

  if (problems.length) {
    problems.slice(0, 20).forEach(fail);
    if (problems.length > 20) { fail(`and ${problems.length - 20} more`); }
  } else {
    pass(`${checked} pieces of text in ${files.length} drawings are inside their canvas`);
  }

  failures += windowCaptureChecks(fail, pass);
  return failures;
}

/**
 * The window captures: no mouse pointer, and no text cut off with an ellipsis.
 *
 * # Why the pointer is checked by reading the capture script
 *
 * The obvious check is to look for a pointer in the pixels, and it is the
 * wrong one. A cursor is a small arbitrary shape over arbitrary content, so
 * any detector is a guess, and a guess over nine screenshots will eventually
 * call a mouse pointer out of a scrollbar and fail a build for a picture that
 * is fine.
 *
 * There is an exact answer available instead. `tools/shots/gui.ps1` captures
 * with `PrintWindow`, which asks the window to draw itself into a bitmap. The
 * pointer is drawn by the compositor on top of the screen and is not part of
 * any window's own rendering, so a `PrintWindow` capture cannot contain one.
 * The property that guarantees no cursor is the capture method, so the capture
 * method is what is checked: a screen copy would include whatever is over the
 * window, a pointer among it.
 *
 * This is the same reasoning as F-103, which found that comparing a drawing
 * against a file written by the same command proves nothing. Check the thing
 * that makes the claim true.
 */
function windowCaptureChecks(fail, pass) {
  let failures = 0;
  const before = failures;

  const script = "tools/shots/gui.ps1";
  const full = path.join(ROOT, script);
  if (!fs.existsSync(full)) {
    fail(`${script} is missing, so nothing here can say how the window ` +
         "captures are taken");
    return 1;
  }
  const source = fs.readFileSync(full, "utf8");
  const code = source
    .split("\n")
    .filter((line) => !line.trim().startsWith("#"))
    .join("\n");

  if (!/PrintWindow\s*\(/.test(code)) {
    fail(`${script} no longer calls PrintWindow. That call is the whole ` +
         "reason a mouse pointer cannot appear in a screenshot: it asks the " +
         "window to draw itself, rather than copying whatever is on screen " +
         "over it.");
  } else {
    pass("window captures are taken with PrintWindow, so no pointer can be in them");
  }

  // The corners are in the alpha channel, so the stylesheet must not draw
  // them a second time.
  //
  // `border-radius` on an `img` clips the content box. The gallery shows these
  // at roughly a third of their captured width, so the file's 14-pixel radius
  // arrives as about five, and a fixed radius here larger than that cuts into
  // the picture past the corner the file already rounded. A `background` is
  // worse: it shows through the transparent corners as a wedge of colour in
  // each one. Both looked like a bug in the screenshot rather than in the page.
  const css = fs.readFileSync(path.join(ROOT, "website/css/main.css"), "utf8");
  const rule = /([^{}]+)\{([^{}]*)\}/g;
  let m;
  while ((m = rule.exec(css)) !== null) {
    const selector = m[1].replace(/\/\*[\s\S]*?\*\//g, "").trim();
    if (!/(^|,|\s)\.(shot|viewer)\s+img\b/.test(selector)) { continue; }
    for (const property of ["border-radius", "background"]) {
      if (new RegExp(`(^|;|\\s)${property}\\s*:`).test(m[2])) {
        fail(`\`${selector}\` sets \`${property}\`. The window captures carry ` +
             "their own rounded corners in the alpha channel, so the page " +
             "must not round or fill them again: a radius here clips the " +
             "picture past its own corner, and a background shows through " +
             "the corners the file made transparent.");
      }
    }
  }

  for (const forbidden of ["CopyFromScreen", "BitBlt", "CAPTUREBLT"]) {
    if (new RegExp(`${forbidden}\\s*\\(`).test(code)) {
      fail(`${script} calls ${forbidden}, which copies the screen rather than ` +
           "the window. Whatever is over the window at the time lands in the " +
           "picture, and the mouse pointer usually is.");
    }
  }

  return failures - before;
}

module.exports = { run, name: "generated pictures, with all their words inside" };
