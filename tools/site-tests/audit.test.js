// SPDX-License-Identifier: GPL-3.0-or-later
//
// The audit's own arithmetic, and whether every finding it hands out a number
// to actually has an entry.
//
// # The two defects this exists to stop coming back
//
// F-105. The verdict opened with "One hundred and four defects found and fixed
// across eighteen audit rounds (F-1 to F-104)" and then broke that down as
// "eight in the first two, twenty-eight in the third, eleven in the fourth,
// twelve in the fifth, one in the sixth, five in the seventh". Those sum to
// sixty-five. The headline had been maintained round after round; the
// explanation underneath it had not been touched in eleven rounds. Two numbers
// about the same thing, one of them being kept.
//
// F-93 and F-94. Both were real, both were fixed, and both were described in
// full in the commit that fixed them. Neither was ever given an entry in
// `docs/AUDIT.md`, so the finding numbers ran 1 to 104 with a hole at 94 and
// nothing said so. A later round then referred a reader back to "F-93" and
// there was nothing to refer them to.
//
// # Why this compares against a measured number and not against prose
//
// F-71 is the reason. A guard already existed that compared the front page's
// test count against `docs/AUDIT.md`, and it passed the whole time both
// numbers were wrong, because both were hand-typed and drifted together. A
// check that compares one copy of a claim against another copy agrees with
// itself.
//
// So the count comes from `docs/MEASURED.md`, which
// `tools/measured/generate.py` derives by reading the audit's finding headings
// out of the document. The documents that make claims are then checked against
// that, never against each other.
//
// # What a gap means, and why it is a failure rather than a note
//
// The generator records two numbers: how many findings have an entry, and the
// highest number handed out. They are equal exactly when no number has been
// skipped. A gap means one of two things, and both are worth failing a build
// over: a finding was fixed and never written up, which is F-94, or a number
// was allocated to something that turned out not to be a finding and the
// document does not say so. Either way the audit is claiming a completeness it
// does not have.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..", "..");

function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), "utf8");
}

/** A row out of the measured table, as a number. */
function measured(label) {
  const table = read("docs/MEASURED.md");
  const row = new RegExp(`^\\| ${label} \\| (\\d+) \\|$`, "m").exec(table);
  return row ? Number(row[1]) : null;
}

/**
 * "One hundred and five" as 105.
 *
 * The verdict states its total in words, and a check that skipped it because
 * parsing words is fiddly would be checking the easy half of the sentence and
 * reporting on the whole of it. That is the shape of mistake this file exists
 * to catch, so the words are parsed. Scale is deliberately limited to hundreds:
 * a project claiming a thousand findings has a different problem.
 */
function wordsToNumber(words) {
  const UNITS = {
    zero: 0, one: 1, two: 2, three: 3, four: 4, five: 5, six: 6, seven: 7,
    eight: 8, nine: 9, ten: 10, eleven: 11, twelve: 12, thirteen: 13,
    fourteen: 14, fifteen: 15, sixteen: 16, seventeen: 17, eighteen: 18,
    nineteen: 19
  };
  const TENS = {
    twenty: 20, thirty: 30, forty: 40, fifty: 50, sixty: 60, seventy: 70,
    eighty: 80, ninety: 90
  };

  let total = 0;
  let current = 0;
  let saw = false;
  for (const word of words.toLowerCase().split(/[\s-]+/)) {
    if (word === "and" || word === "") { continue; }
    if (word === "hundred") { current = (current || 1) * 100; saw = true; continue; }
    if (Object.prototype.hasOwnProperty.call(UNITS, word)) { current += UNITS[word]; saw = true; continue; }
    if (Object.prototype.hasOwnProperty.call(TENS, word)) { current += TENS[word]; saw = true; continue; }
    return null;
  }
  total += current;
  return saw ? total : null;
}

function run() {
  let failures = 0;
  const fail = (why) => { failures += 1; console.log(`FAIL ${why}`); };
  const pass = (what) => console.log(`  ok  ${what}`);

  const writtenUp = measured("Findings written up in the audit");
  const highest = measured("Highest finding number used");

  if (writtenUp === null || highest === null) {
    fail("docs/MEASURED.md has no finding rows; regenerate it with " +
         "tools/measured/generate.py");
    return failures;
  }

  // A gap in the numbering. F-94's whole life as a defect was that nothing
  // said this.
  if (writtenUp !== highest) {
    const missing = highest - writtenUp;
    fail(`the audit hands out numbers up to F-${highest} but writes up only ` +
         `${writtenUp} of them, so ${missing} finding number(s) have no entry. ` +
         "A finding fixed in code and described only in its commit message is " +
         "the case this checks for: see F-94.");
  } else {
    pass(`every finding from F-1 to F-${highest} has an entry of its own`);
  }

  // The README's headline, in digits.
  //
  // It used to carry the range as well, as "(F-1 to F-112)", and this checked
  // both halves. The numbers have come out of the README: they are working
  // references for the audit and for the code, and a reader of the front page
  // is being told an index number for a defect they cannot look up from
  // there. So only the count is claimed, and only the count is checked.
  //
  // The range is still checked, in the audit's own verdict below, which is
  // where somebody who wants a finding by number is already standing.
  const readme = read("README.md");
  const claim = /\*\*(\d+) defects\*\*/.exec(readme);
  if (!claim) {
    fail("README.md no longer states its defect count as '**N defects**', " +
         "so nothing here can check it against the audit");
  } else if (Number(claim[1]) !== writtenUp) {
    fail(`README.md claims ${claim[1]} defects; the audit writes up ${writtenUp}`);
  } else {
    pass(`README.md's ${claim[1]} matches the audit`);
  }

  // And no finding numbers anywhere in it. They are for the audit.
  const numbered = readme.match(/F-\d+/g);
  if (numbered) {
    fail(`README.md carries finding numbers (${[...new Set(numbered)].join(", ")}). ` +
         "Those are working references for the audit and for the code; on the " +
         "front page they are an index into a document the reader is not in.");
  } else {
    pass("README.md carries no finding numbers");
  }

  // The audit's own verdict, which states the total in words and the range in
  // digits. Both halves are checked: F-105 was a sentence whose two halves
  // disagreed with each other.
  const audit = read("docs/AUDIT.md");
  const verdict =
    /\*\*([A-Za-z][A-Za-z\s-]*?) defects found and fixed \(F-1 to (\d+|F-\d+)\)/.exec(audit);
  if (!verdict) {
    fail("docs/AUDIT.md's verdict no longer opens with " +
         "'**<words> defects found and fixed (F-1 to F-N)', so nothing here " +
         "can check it");
  } else {
    const spelled = wordsToNumber(verdict[1]);
    const end = Number(String(verdict[2]).replace(/^F-/, ""));
    if (spelled === null) {
      fail(`the verdict's total, "${verdict[1]}", is not a number this suite ` +
           "can read; write it in words it can, or the check is decorative");
    } else if (spelled !== writtenUp) {
      fail(`the verdict says ${spelled} defects ("${verdict[1]}"); the audit ` +
           `writes up ${writtenUp}`);
    } else {
      pass(`the verdict's "${verdict[1]}" is ${writtenUp}, which is what is written up`);
    }
    if (end !== highest) {
      fail(`the verdict's range ends at F-${end}; the last number used is F-${highest}`);
    } else {
      pass(`the verdict's range ends at F-${highest}`);
    }
  }

  // The breakdown that caused F-105. It is gone, and this keeps it gone: any
  // per-round split written there is unmaintainable, because sixty of the
  // findings sit in a shared section rather than under a round each.
  //
  // Scoped to the verdict section rather than the whole file, and that is not
  // a detail. The first version of this check searched the document and failed
  // on F-105's own write-up, which quotes the breakdown it is about. A guard
  // that cannot tell a quotation from a reintroduction fails honest edits and
  // teaches people to route around it.
  const section = /\n## 6\. Verdict\n([\s\S]*?)(?=\n## |$)/.exec(audit);
  const verdictText = section ? section[1] : "";
  if (!section) {
    fail("docs/AUDIT.md has no '## 6. Verdict' section, so the breakdown " +
         "check has nothing to read");
  } else if (/\b(?:eight|twelve|eleven|five|one) in the (?:first|second|third|fourth|fifth|sixth|seventh)\b/.test(verdictText)) {
    fail("the verdict has grown a per-round breakdown again. There is nothing " +
         "in the document to derive one from, which is why F-105 went " +
         "unnoticed for eleven rounds.");
  } else {
    pass("the verdict states a total and no hand-maintained per-round split");
  }

  return failures;
}

module.exports = { run, name: "the audit's arithmetic and its finding numbers" };
