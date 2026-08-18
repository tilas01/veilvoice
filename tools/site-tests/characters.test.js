// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//
// No stray characters, anywhere in the repository.
//
// # Why this is a test and not a tidy-up
//
// Four separate defects in this project were stray characters nobody could see:
//
//   - The Markdown renderer parked fragments behind **NUL bytes**, which lived
//     as literal control characters in the source. They were invisible in every
//     editor, made the file read as binary to `grep`, and could not be matched
//     by ordinary string-replacing tools.
//   - When those were replaced with **private-use characters**, one escaped
//     into the rendered page, where browsers draw it as nothing at all. Every
//     README link whose label was inline code came out empty: "see
//     [`docs/AUDIT.md`](docs/AUDIT.md)." was published as "see .".
//   - A **NUL from hostile input** passed straight through the renderer.
//   - Writing the fix for that one put literal control characters straight back
//     into the source, because the character class was typed out instead of
//     escaped. Twice.
//
// The pattern is identical every time: a character that is invisible causes a
// fault that is also invisible. Ordinary review does not catch these, so they
// are checked mechanically.
//
// # This file is deliberately pure ASCII
//
// Every pattern below is built with `new RegExp` from a string of `\uXXXX`
// escapes, rather than written as a literal character class. A checker for
// invisible characters that contains invisible characters is not a joke worth
// making twice: the first two attempts at this file did exactly that, and one
// of them turned the checker itself into a file `grep` reported as binary.
//
// # What counts as stray
//
// Not "unusual". This repository is full of deliberate non-ASCII prose - em
// dashes, arrows, accented words - and all of that is wanted. Stray means
// characters carrying no meaning a reader can see.

"use strict";

const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..", "..");
const NUL = String.fromCharCode(0);
const BOM = String.fromCharCode(0xfeff);

/** Every file git tracks, which is exactly the set that ships. */
function trackedFiles() {
  return execFileSync("git", ["ls-files", "-z"], { cwd: ROOT, encoding: "buffer" })
    .toString("utf8")
    .split(NUL)
    .filter(Boolean);
}

/** Legitimately binary, and not text anyone reads. */
const BINARY = /\.(png|jpg|jpeg|gif|ico|rgba|woff2?|ttf|otf|pdf|zip|gz|asc|wav|mp3|flac)$/i;

const RULES = [
  {
    // Tab and newline are fine, and so is carriage return: this repository is
    // developed on Windows and git may hand back CRLF.
    name: "control character",
    pattern: "[\\u0000-\\u0008\\u000B\\u000C\\u000E-\\u001F\\u007F-\\u009F]"
  },
  {
    name: "Unicode replacement character (always a decoding accident)",
    pattern: "\\uFFFD"
  },
  {
    name: "private-use character (invisible, no agreed appearance)",
    pattern: "[\\uE000-\\uF8FF]"
  },
  {
    // Trojan Source: these reorder how text *displays* without changing what it
    // says, so source can read one way and mean another. A project asking
    // people to read its source has a specific reason to refuse them.
    name: "bidirectional override or isolate",
    pattern: "[\\u202A-\\u202E\\u2066-\\u2069]"
  },
  {
    name: "zero-width character",
    pattern: "[\\u200B-\\u200D\\u2060]"
  },
  {
    // UTF-8 read as CP1252 and written back out. An em dash becomes the three
    // perfectly valid characters "a-hat, euro, quote", so nothing here is
    // *invalid* - which is exactly why it survives review and why it needs a
    // pattern of its own rather than a validity check.
    name: "mojibake (UTF-8 decoded as CP1252)",
    pattern: "[\\u00C2\\u00C3\\u00E2\\u00C5\\u00C4][\\u0080-\\u00BF\\u20AC\\u2019\\u201C"
      + "\\u201D\\u2013\\u2014\\u2026\\u02DC\\u2122\\u0161\\u0153\\u017E]"
  }
].map(rule => ({ name: rule.name, re: new RegExp(rule.pattern, "g") }));

/**
 * Files that must be pure ASCII, and why.
 *
 * These are the files the site hands over **raw**, to be read on their own
 * terms rather than rendered inside a page:
 *
 *   - `website/js/*.js`, because the site tells readers to open `verify.js` and
 *     confirm for themselves that nothing is uploaded.
 *   - `website/user-agreements/*.txt`, the licence and the liability waiver,
 *     which are linked directly and are the documents someone reads before
 *     deciding whether to trust any of this.
 *
 * GitHub Pages sends `charset=utf-8` for all of them, so a browser following
 * the header is fine. The trouble is everything that does not: an editor, a
 * downloaded copy, a terminal with a CP1252 locale. There, one prose em dash
 * becomes "a-hat, euro, quote" in the middle of the sentence making the
 * promise - which was reported twice, from two different files, before this
 * rule existed.
 *
 * ASCII removes the question. Non-ASCII that genuinely has to survive is
 * written as a `\\uXXXX` escape: ASCII on disk, correct on screen. See the
 * theme name in `theme.js`.
 *
 * Markdown and HTML are deliberately exempt. They are prose, they declare their
 * own encoding, and em dashes in them are wanted.
 */
const ASCII_ONLY = /^website[/\\](js[/\\].*\.js|user-agreements[/\\].*\.txt)$/;

function describe(ch) {
  return "U+" + ch.codePointAt(0).toString(16).toUpperCase().padStart(4, "0");
}

function positionOf(text, index) {
  const before = text.slice(0, index);
  return before.split("\n").length + ":" + (index - before.lastIndexOf("\n"));
}

function run() {
  let failures = 0;
  let scanned = 0;

  for (const rel of trackedFiles()) {
    const full = path.join(ROOT, rel);
    if (BINARY.test(rel) || !fs.existsSync(full) || fs.statSync(full).isDirectory()) { continue; }

    const text = fs.readFileSync(full, "utf8");
    scanned++;
    const problems = [];

    for (const rule of RULES) {
      rule.re.lastIndex = 0;
      let m;
      while ((m = rule.re.exec(text)) !== null) {
        problems.push(rule.name + " " + describe(m[0]) + " at " + positionOf(text, m.index));
        if (problems.length > 6) { break; }
      }
    }

    // A byte-order mark is unwanted anywhere: at the start it is noise in a
    // repository that is UTF-8 throughout, and anywhere else it is a mistake.
    if (text.charCodeAt(0) === 0xfeff) {
      problems.push("byte-order mark at the start of the file");
    }
    const stray = text.indexOf(BOM, 1);
    if (stray !== -1) {
      problems.push("byte-order mark mid-file at " + positionOf(text, stray));
    }

    if (ASCII_ONLY.test(rel)) {
      for (let i = 0; i < text.length; i++) {
        if (text.charCodeAt(i) > 127) {
          problems.push(
            "non-ASCII " + describe(text[i]) + " at " + positionOf(text, i) +
            " - this file is served raw and must be ASCII, so no viewer can" +
            " mis-decode it; use a \\uXXXX escape if the character must survive"
          );
          break;
        }
      }
    }

    if (problems.length) {
      failures++;
      console.log("FAIL " + rel);
      problems.slice(0, 6).forEach(p => console.log("     " + p));
    }
  }

  console.log("  " + scanned + " tracked text files scanned");
  return failures;
}

module.exports = { run, name: "no stray characters in the repository" };

if (require.main === module) { process.exit(run() ? 1 : 0); }
