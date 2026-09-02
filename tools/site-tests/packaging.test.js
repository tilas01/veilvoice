// SPDX-License-Identifier: GPL-3.0-or-later
//
// The package definitions, against the version the workspace is actually at.
//
// # The defect this exists to stop coming back
//
// F-81. Every definition in `packaging/` named v0.1.9 while the workspace was
// at v0.1.14. Five releases had gone by and nothing noticed, because nothing
// was looking: the Homebrew formula would have fetched and built the v0.1.9
// tarball, the Flatpak manifest would have checked out the v0.1.9 tag, and the
// AppStream metadata told a software centre that 0.1.9 was the newest release
// there is.
//
// That is exactly the shape this repository keeps finding and keeps writing
// guards for: a claim in a file, correct on the day it was typed, with nothing
// watching it. F-71 was two hand-typed numbers agreeing with each other; this
// is six files agreeing with a number that had moved.
//
// # Why a version and not a build
//
// A build would be better and is not available here. `rpmbuild`, `wix`,
// `flatpak-builder` and `brew` are four different platforms' toolchains, and
// `docs/PACKAGING.md` says plainly which of these has been built and which has
// not. What this suite can do without any of them is make sure the number in
// each file is the number the workspace is at, which is the part that went
// wrong.
//
// # The Debian package is the one with a rule of its own
//
// F-80. `dpkg-buildpackage` refuses to start without `debian/changelog`, and
// runs `debian/rules` directly, so that file has to be executable. Neither was
// true, so the recipe printed in `docs/PACKAGING.md` could not run at all.
// Both are checked here, the mode through `git ls-files -s` rather than
// through the filesystem, because a checkout on a filesystem with no
// permission bits still records the mode in the index and that is what other
// people clone.

"use strict";

const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..", "..");

function read(rel) {
  return fs.readFileSync(path.join(ROOT, rel), "utf8");
}

/** The version every one of these has to agree with. */
function workspaceVersion() {
  const manifest = read("Cargo.toml");
  const block = /\[workspace\.package\]([\s\S]*?)(\n\[|$)/.exec(manifest);
  if (!block) { return null; }
  const found = /^\s*version\s*=\s*"([^"]+)"/m.exec(block[1]);
  return found ? found[1] : null;
}

function run() {
  let failures = 0;
  const fail = (message) => { failures++; console.log(`FAIL ${message}`); };
  const pass = (message) => console.log(`ok   ${message}`);

  const version = workspaceVersion();
  if (!version) {
    fail("Cargo.toml has no [workspace.package] version, so nothing can be checked");
    return failures;
  }

  // Each entry: the file, a pattern whose first group is a version, and what
  // that version means to somebody who runs the definition.
  const claims = [
    ["packaging/debian/changelog",
     /^veilvoice \(([0-9]+\.[0-9]+\.[0-9]+)-[0-9]+\)/m,
     "the version dpkg-buildpackage stamps into the .deb"],
    ["packaging/rpm/veilvoice.spec",
     /%global vv_version %\{\?vv_version\}%\{!\?vv_version:([0-9]+\.[0-9]+\.[0-9]+)\}/,
     "what rpmbuild builds when no version is passed"],
    ["packaging/rpm/veilvoice.spec",
     /%changelog\n\* [^\n]*? - ([0-9]+\.[0-9]+\.[0-9]+)-[0-9]+/,
     "the newest entry in the spec's own changelog"],
    ["packaging/homebrew/veilvoice.rb",
     /url "https:\/\/github\.com\/tilas01\/veilvoice\/archive\/refs\/tags\/v([0-9]+\.[0-9]+\.[0-9]+)\.tar\.gz"/,
     "the tarball brew downloads and compiles"],
    ["packaging/flatpak/io.github.tilas01.VeilVoice.yml",
     /^\s*tag: v([0-9]+\.[0-9]+\.[0-9]+)\s*$/m,
     "the tag flatpak-builder checks out"],
    ["packaging/flatpak/io.github.tilas01.VeilVoice.metainfo.xml",
     /<release version="([0-9]+\.[0-9]+\.[0-9]+)"/,
     "the newest release a software centre will show"],
    ["packaging/wix/veilvoice.wxs",
     /-d Version=([0-9]+\.[0-9]+\.[0-9]+)/,
     "the version in the documented wix command"],
    // The commands a reader copies out of the documentation are claims too,
    // and two of them were stale in exactly the same way the files were.
    ["docs/PACKAGING.md",
     /--define "vv_version ([0-9]+\.[0-9]+\.[0-9]+)"/,
     "the version in the rpmbuild command somebody will copy"],
    ["docs/PACKAGING.md",
     /-d Version=([0-9]+\.[0-9]+\.[0-9]+)/,
     "the version in the wix command somebody will copy"]
  ];

  let drifted = 0;
  for (const [file, pattern, what] of claims) {
    let text;
    try {
      text = read(file);
    } catch (error) {
      fail(`${file} is missing, so ${what} cannot be checked`);
      drifted++;
      continue;
    }
    const found = pattern.exec(text);
    if (!found) {
      fail(`${file}: no version found where one is expected (${what})`);
      drifted++;
    } else if (found[1] !== version) {
      fail(`${file} says ${found[1]}, the workspace is at ${version}. That is ${what}.`);
      drifted++;
    }
  }
  if (drifted === 0) {
    pass(`all ${claims.length} version claims in packaging/ are at ${version}`);
  }

  // ---- one version per release, in order, with no gaps --------------------
  //
  // Marker 95. The version claims above only check that everything agrees with
  // the workspace. They say nothing about whether the workspace number is the
  // right *next* one, and a release that skips 0.1.15 or repeats 0.1.14 is a
  // release nobody can reason about afterwards: a user asking "have I got the
  // one with the lock fix" is asking a question about ordering.
  const changelog = read("CHANGELOG.md");
  const released = [...changelog.matchAll(/^## v(\d+)\.(\d+)\.(\d+)\s*$/gm)]
    .map((m) => [Number(m[1]), Number(m[2]), Number(m[3])]);

  if (released.length < 2) {
    fail("CHANGELOG.md lists fewer than two releases, so nothing can be ordered");
  } else {
    // Newest first is how the file is written and how anybody reads it.
    let outOfOrder = 0;
    for (let i = 1; i < released.length; i++) {
      const [aMaj, aMin, aPat] = released[i - 1];
      const [bMaj, bMin, bPat] = released[i];
      const newer = aMaj > bMaj
        || (aMaj === bMaj && aMin > bMin)
        || (aMaj === bMaj && aMin === bMin && aPat > bPat);
      if (!newer) {
        fail(`CHANGELOG.md has v${aMaj}.${aMin}.${aPat} above ` +
             `v${bMaj}.${bMin}.${bPat}; releases are listed newest first`);
        outOfOrder++;
      }
    }
    if (outOfOrder === 0) {
      pass(`${released.length} releases in CHANGELOG.md are in order, newest first`);
    }

    // The workspace is either the newest release, or exactly one patch past
    // it while that release is still being prepared under Unreleased.
    const [maj, min, pat] = version.split(".").map(Number);
    const [nMaj, nMin, nPat] = released[0];
    const isReleased = maj === nMaj && min === nMin && pat === nPat;
    const isNextPatch = maj === nMaj && min === nMin && pat === nPat + 1;
    const isNextMinor = maj === nMaj && min === nMin + 1 && pat === 0;
    const isNextMajor = maj === nMaj + 1 && min === 0 && pat === 0;
    if (isReleased || isNextPatch || isNextMinor || isNextMajor) {
      pass(`the workspace at ${version} follows v${nMaj}.${nMin}.${nPat} without a gap`);
    } else {
      fail(`the workspace is at ${version} and the newest release is ` +
           `v${nMaj}.${nMin}.${nPat}. A release goes up by one: ` +
           `${nMaj}.${nMin}.${nPat + 1}, ${nMaj}.${nMin + 1}.0 or ${nMaj + 1}.0.0.`);
    }
  }

  // ---- every archive the release builds is linked on the releases page ----
  //
  // **F-101.** The page hand-listed five archives and the workflow built
  // eleven. Two of the five names had never existed -- `macos-aarch64` and
  // `linux-aarch64`, where the workflow says `arm64` -- so every release entry
  // carried two dead links, and six published platforms had no link at all.
  //
  // `tools/site/releases.py` derives the list from the workflow now, which is
  // the fix. This is the check that the derivation still works: a workflow
  // rewritten into a shape the generator cannot read would produce a page with
  // fewer downloads on it and nothing else would notice, because a missing
  // link looks exactly like a platform that was never built.
  const workflow = read(".github/workflows/release.yml");
  const releasesPage = read("website/releases.html");
  const labels = new Set();
  for (const match of workflow.matchAll(/^\s*label:\s*(\S+)\s*$/gm)) {
    labels.add(match[1]);
  }
  for (const match of workflow.matchAll(/out="veilvoice-\$\{\{[^}]*\}\}-(\S+?)"/g)) {
    labels.add(match[1]);
  }
  if (labels.size < 5) {
    fail(`only ${labels.size} archive labels could be read out of ` +
         "release.yml, so this check is not checking anything");
  } else {
    const missing = [...labels].filter(
      (label) => !releasesPage.includes(`-${label}.`)
    );
    if (missing.length) {
      fail(`the releases page links no file for ${missing.join(", ")}, ` +
           "which the release workflow builds");
    } else {
      pass(`all ${labels.size} archives the workflow builds are linked on ` +
           "the releases page");
    }
  }

  // ---- the two things dpkg-buildpackage needs before it will start --------
  const index = execFileSync("git", ["ls-files", "-s", "packaging/debian/"], {
    cwd: ROOT, encoding: "utf8", maxBuffer: 16 * 1024 * 1024
  });
  const modes = new Map(
    index.split("\n").filter(Boolean).map((line) => {
      const parts = line.split("\t");
      return [parts[1], parts[0].split(" ")[0]];
    })
  );

  if (!modes.has("packaging/debian/changelog")) {
    fail("packaging/debian/changelog is not tracked, and dpkg-buildpackage " +
         "refuses to start without it");
  } else {
    pass("packaging/debian/changelog is there, so dpkg-buildpackage can start");
  }

  const rules = modes.get("packaging/debian/rules");
  if (rules !== "100755") {
    fail(`packaging/debian/rules is mode ${rules || "absent"} in the index; ` +
         "dpkg-buildpackage runs it directly, so it has to be 100755");
  } else {
    pass("packaging/debian/rules is executable in the index");
  }

  failures += changelogDates(fail, pass);
  failures += releaseDatesAgree(fail, pass);
  // --- the site is published after a release ---------------------------------
  //
  // The site is not only documentation. The download page names the current
  // version, the releases page is generated from CHANGELOG.md, and the verify
  // page describes files a release publishes. Cutting a release changed all of
  // that and deployed none of it: a tag push touches no path in the pages
  // workflow's filter, so the site stayed on the previous release until
  // somebody happened to edit a file under `website/`.
  //
  // Checked here rather than trusted, because the failure is silent and slow:
  // the site is simply out of date, and looks fine.
  const pages = "\.github/workflows/pages.yml";
  const pagesPath = path.join(ROOT, ".github", "workflows", "pages.yml");
  if (!fs.existsSync(pagesPath)) {
    fail(`${pages} is missing, so nothing publishes the site`);
    failures += 1;
  } else {
    const workflow = fs.readFileSync(pagesPath, "utf8");
    if (!/workflow_run:/.test(workflow) || !/workflows:\s*\[?\s*release/.test(workflow)) {
      fail(`${pages} does not run after the release workflow, so a release ` +
           "publishes new archives and leaves the site describing the previous " +
           "one until somebody edits a page by hand");
      failures += 1;
    } else if (!/conclusion\s*==\s*'success'/.test(workflow)) {
      fail(`${pages} runs after the release workflow without checking that it ` +
           "succeeded, so a release that failed halfway would still redeploy " +
           "the site");
      failures += 1;
    } else {
      pass("the site is published again after a release succeeds");
    }
  }

  return failures;
}

/**
 * A release is dated the same day in every file that dates it.
 *
 * Three files record when each version came out, by hand and separately: the
 * Debian changelog, the RPM spec's `%changelog`, and the AppStream metainfo
 * the Flatpak ships. Nothing derives any of them from the others.
 *
 * They agree today. This is here because the weekday check above exists: that
 * defect was one wrong date copied into two of these three files, which is
 * proof that they are edited together, by hand, and can part company. A
 * software centre shows the metainfo date and a distribution shows the
 * changelog, so a reader can be told two different days a release happened.
 *
 * Only versions a file actually mentions are compared. The RPM spec's history
 * reaches further back than the others, and that is not drift.
 */
function releaseDatesAgree(fail, pass) {
  let failures = 0;
  const MONTHS = {
    Jan: "01", Feb: "02", Mar: "03", Apr: "04", May: "05", Jun: "06",
    Jul: "07", Aug: "08", Sep: "09", Oct: "10", Nov: "11", Dec: "12"
  };
  const dated = new Map();
  const note = (version, where, date) => {
    if (!dated.has(version)) { dated.set(version, new Map()); }
    dated.get(version).set(where, date);
  };

  // AppStream: <release version="0.1.16" date="2026-09-01"/>
  const meta = read("packaging/flatpak/io.github.tilas01.VeilVoice.metainfo.xml");
  for (const m of meta.matchAll(/<release\s+version="([\d.]+)"\s+date="([\d-]+)"/g)) {
    note(m[1], "the Flatpak metainfo", m[2]);
  }

  // RPM: * Tue Sep 01 2026 Name <mail> - 0.1.16-1
  const spec = read("packaging/rpm/veilvoice.spec");
  const changelog = spec.slice(spec.indexOf("%changelog"));
  for (const m of changelog.matchAll(
    /^\* \w{3} (\w{3}) (\d{2}) (\d{4})[^\n]*?-\s*([\d.]+)-\d/gm
  )) {
    note(m[4], "the RPM spec", `${m[3]}-${MONTHS[m[1]] || "??"}-${m[2]}`);
  }

  // Debian: the version heading, then the ` -- ` line that closes that entry.
  const deb = read("packaging/debian/changelog");
  const versions = [...deb.matchAll(/^veilvoice \(([\d.]+)-\d\)/gm)].map((m) => m[1]);
  const stamps = [...deb.matchAll(/^ -- .*?>\s+\w{3}, (\d{2}) (\w{3}) (\d{4})/gm)];
  versions.forEach((version, at) => {
    const m = stamps[at];
    if (m) { note(version, "the Debian changelog", `${m[3]}-${MONTHS[m[2]] || "??"}-${m[1]}`); }
  });

  let compared = 0;
  for (const [version, places] of dated) {
    if (places.size < 2) { continue; }
    compared += 1;
    const days = new Set(places.values());
    if (days.size > 1) {
      failures += 1;
      const said = [...places]
        .map(([where, day]) => `${where} says ${day}`)
        .join(", and ");
      fail(`version ${version} is dated differently in each file: ${said}. A ` +
           "software centre shows one of these and a distribution shows " +
           "another, so a reader is told two days a release happened.");
    }
  }

  if (compared === 0) {
    failures += 1;
    fail("no version is dated in more than one packaging file, so this check " +
         "is comparing nothing");
  } else if (failures === 0) {
    pass(`${compared} release(s) are dated the same day in every file that dates them`);
  }
  return failures;
}

/**
 * Every changelog entry's weekday matches its date.
 *
 * # The defect this exists to stop coming back
 *
 * `Sun, 31 Aug 2026` in the Debian changelog and `Sun Aug 31 2026` in the RPM
 * spec. 31 August 2026 was a Monday. One wrong weekday, copied into two
 * packaging files, sitting in both for as long as they had existed.
 *
 * Neither is cosmetic. `rpmbuild` prints `bogus date in %changelog` and
 * lintian carries `debian-changelog-has-wrong-day-of-week`, so both would be
 * bounced by a distribution's review. Nothing here noticed, because nothing
 * here built the packages: `docs/PACKAGING.md` said these formats "parse", and
 * a date with the wrong weekday parses perfectly.
 *
 * # Why this check and not the build
 *
 * Building both packages takes a full release compile and two toolchains, and
 * it already happens by hand and is written up. This is the part that can run
 * on every commit, on any machine, in milliseconds. It does not replace the
 * build; it catches the one class of defect that a build found and that
 * nothing else was looking for.
 */
function changelogDates(fail, pass) {
  let failures = 0;
  const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const MONTHS = {
    Jan: 0, Feb: 1, Mar: 2, Apr: 3, May: 4, Jun: 5,
    Jul: 6, Aug: 7, Sep: 8, Oct: 9, Nov: 10, Dec: 11
  };

  const sources = [
    {
      file: "packaging/debian/changelog",
      // ` -- Name <mail>  Tue, 01 Sep 2026 00:00:00 +0000`
      pattern: /^ -- .*?>\s+(\w{3}), (\d{2}) (\w{3}) (\d{4})/gm,
      order: (m) => [m[1], m[2], m[3], m[4]]
    },
    {
      file: "packaging/rpm/veilvoice.spec",
      // `* Tue Sep 01 2026 Name <mail> - 0.1.16-1`
      pattern: /^\* (\w{3}) (\w{3}) (\d{2}) (\d{4})/gm,
      order: (m) => [m[1], m[3], m[2], m[4]]
    }
  ];

  let checked = 0;
  for (const { file, pattern, order } of sources) {
    const text = read(file);
    let m;
    pattern.lastIndex = 0;
    while ((m = pattern.exec(text)) !== null) {
      const [dow, day, mon, year] = order(m);
      if (!(mon in MONTHS)) {
        failures += 1;
        fail(`${file}: "${mon}" is not a month name`);
        continue;
      }
      checked += 1;
      const date = new Date(Date.UTC(Number(year), MONTHS[mon], Number(day)));
      const actual = DAYS[date.getUTCDay()];
      if (actual !== dow) {
        failures += 1;
        fail(`${file}: "${dow}, ${day} ${mon} ${year}" is wrong; that date is ` +
             `a ${actual}. rpmbuild calls this a bogus date and lintian has ` +
             "debian-changelog-has-wrong-day-of-week, so a distribution's " +
             "review would bounce it.");
      }
    }
  }

  if (checked < 2) {
    failures += 1;
    fail("no changelog dates were found in either packaging file, so this " +
         "check is reading nothing");
  } else if (failures === 0) {
    pass(`${checked} packaging changelog dates have the right weekday`);
  }
  return failures;
}

module.exports = { run, name: "package definitions, against the workspace version" };
