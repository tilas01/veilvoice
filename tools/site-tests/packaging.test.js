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

  return failures;
}

module.exports = { run, name: "package definitions, against the workspace version" };
