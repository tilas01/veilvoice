#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Regenerate everything generated, then run every check, in the right order.

    python tools/verify.py            # regenerate, then check
    python tools/verify.py --check    # check only, exactly as CI does

# Why this exists

Four generators write into this tree, and three of them read files the others
write. Run them in the wrong order, or edit a source file after running them,
and the committed output is one change behind -- which is not a visible fault.
It is a green local run and a red CI run ten minutes later.

That happened twice in one afternoon, both times the same way: edit a source
file, regenerate, edit another source file, commit. The search index is built
from *every tracked file*, so it goes stale when anything at all changes after
it is written.

The order below is the dependency order, and it is the whole point of the file:

  1. `assets/generate.py`      -- artwork, from nothing but itself
  2. `tools/docs/generate.py`  -- reads the Rust doc comments, writes 371 files
  3. `tools/search-index/generate.py` -- reads *everything*, so it goes last
  4. the checks, which must all see the same tree

Anything that regenerates has to be staged before the index runs, because the
index walks `git ls-files` and a file git has never heard of is not in it.
"""

import os
import subprocess
import sys


def repo_root():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(here, ".."))


def run(root, label, command, capture=True):
    """Run one step. Returns (ok, output).

    Cargo steps are run with `RUSTFLAGS=-D warnings`, because that is what CI
    sets and a local check that does not match CI is worse than no local check:
    it passes, and then the push fails ten minutes later for something that was
    on screen the whole time. Three CI failures in one session came through
    exactly that gap -- an unused `mut`, a `needless_return`, a dead enum
    variant -- each a warning locally and an error there.
    """
    environment = dict(os.environ)
    if command and str(command[0]).startswith("cargo"):
        existing = environment.get("RUSTFLAGS", "")
        if "-D warnings" not in existing:
            environment["RUSTFLAGS"] = (existing + " -D warnings").strip()
    try:
        result = subprocess.run(
            command, cwd=root, shell=isinstance(command, str), env=environment,
            stdout=subprocess.PIPE if capture else None,
            stderr=subprocess.STDOUT if capture else None,
        )
    except OSError as error:
        return False, "%s: could not run (%s)" % (label, error)
    output = (result.stdout or b"").decode("utf-8", "replace")
    return result.returncode == 0, output


def stage(root):
    """Stage everything, so the index walk sees newly written files.

    `git ls-files` lists *tracked* files. A generator that has just written a
    new page leaves it untracked, so the index built immediately afterwards
    does not contain it -- and CI, regenerating from the committed tree, finds
    one more file and fails with a message about drift that says nothing about
    why.
    """
    subprocess.run(["git", "add", "-A"], cwd=root,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


GENERATORS = [
    # First. It depends on nothing that is generated -- it runs the test suite
    # and reads Cargo.toml -- and everything after it may quote what it
    # measures. It was written in last, which put it after the search index:
    # the index walked docs/ before this file had been rewritten and then
    # disagreed with it, on the very first run.
    ("measured numbers", [sys.executable, "tools/measured/generate.py"]),
    ("artwork", [sys.executable, "assets/generate.py"]),
    # Drawn from the command output committed beside them. `--capture`, which
    # actually runs `veilvoice`, is a separate and manual step: it needs a
    # build, a machine and a person deciding the output is right. Everything
    # after that is a pure function of those text files, which is why this half
    # can be regenerated and checked here.
    # Before the drawings, which mirror the window captures into the website
    # and record their sizes: a crop after that copy would leave the two out of
    # step, and the size check in `terminal.py --check` is what would notice.
    ("screenshot borders", [sys.executable, "tools/shots/crop.py"]),
    # After the border comes off and before the corners go on. A capture is
    # taken taller than any tab needs, so that the longest one is not cut in
    # half, and this trims each picture back to what it actually contains.
    # Rounding first would round the corners of a picture that is about to
    # lose its bottom half.
    ("screenshot height", [sys.executable, "tools/shots/fit.py"]),
    # After the crop and the fit, never before them: rounding the corners of a
    # picture that still has a capture border rounds the border.
    ("screenshot corners", [sys.executable, "tools/shots/round.py"]),
    # After everything that can change a picture's size, because it copies
    # those sizes into the pages that show them.
    ("screenshot sizes on the pages", [sys.executable, "tools/shots/attrs.py"]),
    ("terminal drawings", [sys.executable, "tools/shots/terminal.py"]),
    ("documentation", [sys.executable, "tools/docs/generate.py"]),
    # Before the wiki, which carries this document as a page of its own.
    ("every command, and its place in the window",
     [sys.executable, "tools/docs/commands.py"]),
    # Reads the workspace, tools/audit/ and this file's own lists, so it goes
    # after the documentation and before the wiki that carries it.
    ("the developer guide", [sys.executable, "tools/docs/developers.py"]),
    # After the documentation generator, which owns the wiki directory:
    # these are per-program views of the user guide and land in it too.
    ("per-program guides", [sys.executable, "tools/docs/guides.py"]),
    # After the guides, because the wiki's landing page links to them.
    ("wiki landing and documents", [sys.executable, "tools/docs/wiki.py"]),
    # Before the source pages below, which walk `website/js` and publish a page
    # per file in it: this writes `website/js/demo-data.js`, so a run with the
    # two the other way round publishes the previous demonstration and fails
    # its own check. The comment on this line has said "before the source
    # pages" since it was written; the order did not agree with it, and a
    # single pass over a changed demonstration went red until somebody ran it
    # twice. Nothing else here reads what this writes, so it moves rather than
    # the source pages moving past the wiki that walks them.
    ("demonstration data", [sys.executable, "tools/site/demo.py"]),
    # The website's own source, which `generate.py` does not cover: it reads
    # Rust doc comments, and these are JavaScript and CSS. Imports the same
    # module for the palette and the drawing code, so it goes after it, and
    # after every generator that writes into `website/js`.
    ("website source pages", [sys.executable, "tools/docs/sources.py"]),
    # After every page of the reference exists, including the source pages
    # above: this publishes the wiki on the website and works out which of its
    # pages are already here by walking `website/reference/`. Before the
    # addresses and the index, which walk what it writes.
    ("the wiki on the website", [sys.executable, "tools/site/wiki_site.py"]),
    # Derived from website/index.html, so it must run after anything that could
    # edit that file and before the index walks the result.
    ("section pages", [sys.executable, "tools/site/split.py"]),
    # After the split, because it borrows that tool's header, navigation and
    # footer from index.html, and before the index, which walks the result.
    ("roadmap page", [sys.executable, "tools/site/roadmap.py"]),
    ("questions page", [sys.executable, "tools/site/faq.py"]),
    # After the split, whose header this borrows, and before the index walks it.
    ("releases page", [sys.executable, "tools/site/releases.py"]),
    # After every generator that writes a page, because it reads each page's
    # own title and description and writes the addresses from them. Before
    # the index, which walks the result.
    ("addresses and the sitemap", [sys.executable, "tools/site/seo.py"]),
    ("search index", [sys.executable, "tools/search-index/generate.py"]),
]

# Which generator has to run before which, and why, for the pairs where one
# writes a file the other reads or walks. Only those pairs: the list is not a
# second copy of the order above, it is the handful of orderings that are load
# bearing, and `ordering_faults` checks the order against them.
#
# This exists because the comment saying "before the source pages" sat on the
# demonstration step while the demonstration step ran *after* the source pages,
# and both were true-looking English. A comment cannot be run. A single pass
# over a changed demonstration therefore failed, and passed on a second run,
# which is the worst shape a build failure can have: it looks like a flake and
# it is not one.
MUST_RUN_BEFORE = [
    ("demonstration data", "website source pages",
     "it writes website/js/demo-data.js, which the source pages walk and "
     "publish a page for"),
    ("website source pages", "the wiki on the website",
     "the wiki on the website works out which reference pages exist by "
     "walking website/reference/, which the source pages write into"),
    ("section pages", "roadmap page",
     "the roadmap page borrows the header, navigation and footer the split "
     "takes out of index.html"),
    ("addresses and the sitemap", "search index",
     "the index walks the pages the addresses step has finished writing"),
]


def ordering_faults():
    """Every pair in MUST_RUN_BEFORE that the generator list has back to front.

    Returns a list of sentences, empty when the order is right. Separate from
    `main` so it can be exercised without running a single generator.
    """
    at = {label: n for n, (label, _) in enumerate(GENERATORS)}
    faults = []
    for earlier, later, why in MUST_RUN_BEFORE:
        for label in (earlier, later):
            if label not in at:
                faults.append(
                    "%r is named in MUST_RUN_BEFORE and is not a generator, so "
                    "the ordering it describes is not being checked" % label)
        if earlier in at and later in at and at[earlier] > at[later]:
            faults.append(
                "%r runs after %r and must run before it: %s"
                % (earlier, later, why))
    return faults


CHECKS = [
    # First, because a release whose README tells people to download the
    # previous version is a failure nothing else here would notice: the
    # command works, the verifier passes, and the reader gets the wrong
    # program.
    ("every copy of the version agrees with Cargo.toml",
     [sys.executable, "tools/release/version.py", "--check"]),
    ("every package installs what the workspace builds",
     [sys.executable, "tools/release/packaging.py"]),
    # Beside it, and for the same reason. A release's notes used to be a
    # hundred and twenty lines of shell inside the release workflow, so they
    # were seen for the first time when the release was published: v0.1.21
    # went out with no notes at all because a changelog heading was written
    # `## 0.1.21 - 2026-09-10` and the extraction wanted `## v0.1.21`. The
    # notes are built by a tool now, and this runs that tool's reading of the
    # newest entry on every push, which is the whole gain. Roadmap item 180.
    ("the newest release notes can be built from the changelog",
     [sys.executable, "tools/release/notes.py", "--check"]),
    ("no state file is written one place and read another",
     [sys.executable, "tools/audit/state_paths.py"]),
    # Roadmap item 126. A dependency is a decision, and a decision with no sentence
    # beside it is one nobody can revisit. The day this was added it found
    # three that no line of code referred to.
    ("every dependency says what it is for",
     [sys.executable, "tools/audit/dependencies.py"]),
    # The same question asked of this project's own code rather than of the
    # code it imports. `dead_code` stops at the crate boundary, so nothing was
    # watching whether a public item had a caller; five had none, and one of
    # them was keeping a private field and a clone per vault open alive behind
    # it, which is exactly the shape the compiler cannot report.
    ("every public item is reached by something",
     [sys.executable, "tools/audit/reachable.py"]),
    # Three documents list the crates and they listed thirteen, twelve and
    # thirteen of the twenty-seven. The front page of the website renders the
    # README, so the shortest of the three was what the site said this project
    # was made of.
    ("every crate appears in every table that lists the crates",
     [sys.executable, "tools/audit/crate_tables.py"]),
    # F-185. A build directory inside the repository is invisible to git,
    # because `.gitignore` matches `target/` at any depth, so it is never
    # reported and never cleaned. One had been rebuilt by this very tool on
    # every non-Windows machine and had reached fifteen gigabytes.
    ("no build output lives outside the one directory at the root",
     [sys.executable, "tools/audit/build_output.py"]),
    # Roadmap item 151. The mutation campaign itself is weekly, because half an hour
    # does not fit a per-push job. What fits here is the half that does not need
    # a campaign: every argued-for survivor still points at a real file and a
    # real line. Those move whenever code above them moves, and without this the
    # list would quietly point at the wrong lines until the next weekly run.
    ("every argued-for surviving mutant still points at real code",
     [sys.executable, "tools/mutants/check.py", "--lint"]),
    # Beside it, and answering the other question. `dependencies.py` asks
    # whether somebody said why each dependency is here; this asks whether
    # anything is watching it. A manifest outside every Dependabot entry is
    # unmonitored quietly, which is worse than a dependency with a known
    # problem, because nobody is looking at it.
    ("every manifest is covered by a Dependabot entry",
     [sys.executable, "tools/audit/dependabot.py"]),
    # And the half of that guard which cannot be proved by running it here: a
    # check that reads only the file it guards is a check nobody has seen fail.
    # F-200 was an entry aimed at the released branch, and the reader in place
    # at the time stopped looking before `target-branch` and would have passed
    # either way. These cases are text, so the failing shapes are exercised.
    ("the Dependabot guard notices where a bump would land",
     [sys.executable, "tools/audit/dependabot.py", "--self-test"]),
    # And the third question about dependencies, which is whether the code
    # still compiles without the optional ones. Nine of the twelve release
    # jobs turn `veilvoice-audio`'s `live` feature off because `cpal` has no
    # backend for them, and nothing built that configuration until a module
    # lost its `#[cfg]` and failed all nine at once. Listed rather than built
    # here: `--build` is a compile, which belongs in CI beside the release
    # build rather than in a pass somebody runs before every commit.
    ("every feature selection a release builds is still declared",
     [sys.executable, "tools/audit/features.py"]),
    # The check that lets an accepted advisory stay accepted: RUSTSEC-2023-0071
    # is about RSA private-key operations, and this crate only ever verifies a
    # signature against a public key. It ran only in CI until now, which meant
    # the one guard behind a security argument was the one nobody could run
    # before pushing.
    ("no crate reaching pgp performs a private-key operation",
     [sys.executable, "tools/audit/rsa_usage.py"]),
    # A de-identifier whose randomness is predictable does not work, and a weak
    # draw produces output that looks exactly like a strong one. Rust puts the
    # three names that do not promise a cryptographic draw, `thread_rng`,
    # `SmallRng` and `StdRng`, in the most obvious crate.
    ("every random draw comes from the OS CSPRNG",
     [sys.executable, "tools/audit/randomness.py"]),
    # A tag is a label, not a version: whoever owns the action can move it, and
    # two of the ones here were branches rather than tags. An unpinned action
    # runs whatever it points at that morning, on a runner holding a checkout
    # and a token.
    ("every action a workflow runs is pinned to a commit",
     [sys.executable, "tools/audit/actions.py"]),
    # F-170. The tag a release publishes must name the commit that was built,
    # or the reproducibility every other check exists to support is a claim
    # about a different tree than the one somebody would check out.
    ("the release step tags the commit it built",
     [sys.executable, "tools/audit/publishing.py"]),
    # F-204. Every other check in this file reads the working tree, and the
    # working tree is not what anybody else clones. A file that is present and
    # untracked reads exactly like a file that is committed, which is how an
    # `include_str!` fixture went out without the file it names and took the
    # whole workspace down on every clone but the one that wrote it. This is
    # the only check here that asks git rather than the disk, which is why it
    # belongs in the routine before a push and not only in CI.
    ("every file the build reads is in the repository",
     [sys.executable, "tools/audit/fixtures.py"]),
    ("that guard catches what it claims to",
     [sys.executable, "tools/audit/fixtures.py", "--self-test"]),
    ("the app-manifest tooling works",
     [sys.executable, "tools/sign/selftest.py"]),
    ("artwork matches its generator", [sys.executable, "assets/generate.py", "--check"]),
    ("no screenshot has a capture border",
     [sys.executable, "tools/shots/crop.py", "--check"]),
    ("no screenshot has empty space below its content",
     [sys.executable, "tools/shots/fit.py", "--check"]),
    ("screenshots have rounded corners",
     [sys.executable, "tools/shots/round.py", "--check"]),
    ("every screenshot tag matches its file",
     [sys.executable, "tools/shots/attrs.py", "--check"]),
    ("terminal drawings match their output",
     [sys.executable, "tools/shots/terminal.py", "--check"]),
    # The recorded sessions, re-run and compared. This is the check that would
    # have caught the demonstration inventing the verifier's output, and it is
    # the only one here that runs the programs rather than reading about them.
    ("recorded sessions match the programs",
     [sys.executable, "tools/shots/sessions.py", "--check"]),
    ("documentation matches the source", [sys.executable, "tools/docs/generate.py", "--check"]),
    ("per-program guides match the user guide",
     [sys.executable, "tools/docs/guides.py", "--check"]),
    # Two questions in one: whether the wiki's prose pages still match the
    # documents they are converted from, and whether every `[[link]]` in the
    # whole wiki names a page that exists. The second matters because a wiki
    # link to a missing page does not fail loudly: GitHub renders it as an
    # invitation to create that page, so a typo looks like a feature.
    ("the wiki matches the documents, and every link in it resolves",
     [sys.executable, "tools/docs/wiki.py", "--check"]),
    ("website source pages match their files",
     [sys.executable, "tools/docs/sources.py", "--check"]),
    ("every command is documented, and its window location exists",
     [sys.executable, "tools/docs/commands.py", "--check"]),
    ("the developer guide matches the tree",
     [sys.executable, "tools/docs/developers.py", "--check"]),
    ("the website's wiki matches the wiki",
     [sys.executable, "tools/site/wiki_site.py", "--check"]),
    ("section pages match index.html", [sys.executable, "tools/site/split.py", "--check"]),
    ("the roadmap page matches ROADMAP.md",
     [sys.executable, "tools/site/roadmap.py", "--check"]),
    ("the demonstration matches the source",
     [sys.executable, "tools/site/demo.py", "--check"]),
    ("the questions page matches docs/FAQ.md",
     [sys.executable, "tools/site/faq.py", "--check"]),
    ("the releases page matches CHANGELOG.md",
     [sys.executable, "tools/site/releases.py", "--check"]),
    ("every page says where it lives, and the sitemap lists it",
     [sys.executable, "tools/site/seo.py", "--check"]),
    ("search index matches the tree", [sys.executable, "tools/search-index/generate.py", "--check"]),
    ("measured numbers match the tree",
     [sys.executable, "tools/measured/generate.py", "--check"]),
    ("the line counter is right about what a line is",
     [sys.executable, "tools/loc/count.py", "--self-test"]),
    ("website suites", ["node", "tools/site-tests/run.js"]),
    ("the local site serves every page", [sys.executable, "tools/site/serve.py", "--check"]),
]

CARGO = [
    ("formatting", ["cargo", "fmt", "--all", "--check"]),
    ("clippy", ["cargo", "clippy", "--workspace", "--all-targets"]),
    ("tests", ["cargo", "test", "--workspace"]),
]


def main():
    root = repo_root()

    # Before anything runs, because a generator order that is wrong wastes the
    # whole run and then fails a check that names the wrong thing: what goes
    # red is the file that was published stale, not the ordering that published
    # it.
    faults = ordering_faults()
    if faults:
        print("the generators are not in dependency order:")
        for fault in faults:
            print("  %s" % fault)
        print()
        print("Reorder GENERATORS, or correct MUST_RUN_BEFORE if the "
              "dependency it describes is no longer real.")
        return 1

    check_only = "--check" in sys.argv
    # `--quick` runs everything except the three cargo steps. Those are most of
    # the wall time and almost none of the failures: what actually reaches CI
    # red, twice in one afternoon, is a generated file left behind by a commit
    # that added a source file. The search index walks every text file in the
    # tree, so adding one and not regenerating is enough. This is the check to
    # run before a push that touches no Rust; `tools/verify.py` whole is still
    # the one to run before a release.
    quick = "--quick" in sys.argv
    failed = []

    if not check_only:
        print("regenerating, in dependency order")
        for label, command in GENERATORS:
            stage(root)   # so the walk sees anything written by the step before
            ok, output = run(root, label, command)
            print("  %-16s %s" % (label, "ok" if ok else "FAILED"))
            if not ok:
                print(output)
                failed.append(label)
        stage(root)
        print()

    print("checking" + (", without the cargo steps" if quick else ""))
    for label, command in CHECKS + ([] if quick else CARGO):
        ok, output = run(root, label, command)
        print("  %-34s %s" % (label, "ok" if ok else "FAILED"))
        if not ok:
            failed.append(label)
            for line in output.strip().splitlines()[-25:]:
                print("      " + line)

    print()
    if failed:
        print("%d check(s) failed: %s" % (len(failed), ", ".join(failed)))
        return 1
    print("everything regenerated and every check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
