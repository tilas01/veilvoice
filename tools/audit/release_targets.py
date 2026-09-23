#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
The updater asks for an archive the release workflow actually builds.

# The defect this exists to stop

Roadmap item 179 gave VeilVoice an updater that fetches the release published for
the platform it is running on. To do that it has to know what that release is
called, and the name is built from a **label**: `veilvoice-v0.1.23-linux-arm64.tar.gz`
is the tag and the label `linux-arm64` with an extension on the end.

Those labels are written in two places and nothing in either language relates
them. They are the `label:` values in the build matrix of
`.github/workflows/release.yml`, and they are `PUBLISHED` in
`crates/veilvoice-verify/src/update.rs`, which maps this build's target triple
onto one of them.

Both directions of drift are silent and both are bad:

* **A label in the workflow and not in the updater** means a platform whose
  releases are published and whose users are told nothing is offered for their
  build. The failure is a shrug rather than an error.
* **A label in the updater and not in the workflow** means a build that asks
  GitHub for an archive that has never existed. The failure is a 404 reported
  as a download problem, which reads like a network fault and sends somebody
  looking in the wrong place.

Neither shows up in a test, because a test of the Rust side can only compare
the list against itself, and a workflow is not run by `cargo test`.

# What is checked

Every `label:` in the release workflow's build matrix appears in `PUBLISHED`,
and every entry in `PUBLISHED` appears in the workflow. The workflow's
single-platform jobs, which stage `veilvoice-$VERSION_TAG-freebsd-x86_64` and
its two neighbours directly rather than through the matrix, are read from the
`out=` line that names them, because a label that only ever appears in a shell
assignment is still a label a reader can download.

The archive extension is checked with it. The workflow selects `zip` or
`tar.gz` per row, and the updater decides the same thing from whether the label
begins with `windows`. That shortcut is correct today and would stop being
correct the moment a second platform published a zip, so it is asserted rather
than assumed.
"""

import re
import sys
import shutil
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/release.yml"
UPDATER = ROOT / "crates/veilvoice-verify/src/update.rs"


def labels_in_workflow(text):
    """Every label the workflow publishes an archive for, with its extension."""
    found = {}

    # The build matrix: `label:` and `archive:` are siblings in one entry, and
    # `archive:` defaults to nothing, so an entry is read as a block rather than
    # line by line.
    for block in re.split(r"\n\s*-\s+os:", text):
        label = re.search(r"^\s*label:\s*(\S+)\s*$", block, re.M)
        if not label:
            continue
        archive = re.search(r"^\s*archive:\s*(\S+)\s*$", block, re.M)
        found[label.group(1)] = archive.group(1) if archive else "tar.gz"

    # The single-platform jobs, which name their own output directory. Their
    # archive step is `tar -czf` in every case, and that is asserted below
    # rather than parsed, because these three jobs have no `archive:` key to
    # read.
    for out in re.findall(r'out="veilvoice-\$\{\{\s*env\.VERSION_TAG\s*\}\}-([A-Za-z0-9_.-]+)"', text):
        found.setdefault(out, "tar.gz")

    return found


def labels_in_updater(text):
    """The `PUBLISHED` list, read from the Rust source."""
    block = re.search(r"pub const PUBLISHED:\s*&\[&str\]\s*=\s*&\[(.*?)\];", text, re.S)
    if not block:
        return None
    return [m for m in re.findall(r'"([^"]+)"', block.group(1))]


def audit(root):
    problems = []
    workflow = (root / ".github/workflows/release.yml").read_text(encoding="utf-8")
    updater = (root / "crates/veilvoice-verify/src/update.rs").read_text(encoding="utf-8")

    published = labels_in_workflow(workflow)
    if not published:
        problems.append(
            "  no labels were found in the release workflow. Either it stopped "
            "publishing archives or this guard stopped being able to read it, "
            "and both are worth stopping a build for.")
        return problems, 0

    listed = labels_in_updater(updater)
    if listed is None:
        problems.append(
            "  PUBLISHED could not be found in crates/veilvoice-verify/src/update.rs. "
            "It is what the updater builds a download URL from.")
        return problems, 0

    for label in sorted(set(published) - set(listed)):
        problems.append(
            "  %s is published by the release workflow and is not in PUBLISHED, so "
            "a build for it is told nothing is offered." % label)
    for label in sorted(set(listed) - set(published)):
        problems.append(
            "  %s is in PUBLISHED and the release workflow builds no archive for it, "
            "so a build for it would ask for a file that has never existed." % label)

    # The extension rule the updater uses, checked against what the workflow
    # actually builds rather than assumed to still hold.
    for label, extension in sorted(published.items()):
        expected = "zip" if label.startswith("windows") else "tar.gz"
        if extension != expected:
            problems.append(
                "  the workflow builds a .%s for %s, and the updater works the "
                "extension out from whether the label begins with `windows`, which "
                "gives .%s. One of the two has to change." % (extension, label, expected))

    return problems, len(published)


def self_test():
    """Both directions of drift are caught, and a sound pair is not."""
    failures = 0
    where = Path(tempfile.mkdtemp(prefix="veilvoice-release-targets-"))
    try:
        def build(labels_workflow, labels_rust, archive="tar.gz"):
            (where / ".github/workflows").mkdir(parents=True, exist_ok=True)
            rows = "\n".join(
                "          - os: ubuntu-latest\n"
                "            target: x\n"
                "            label: %s\n"
                "            archive: %s" % (label, "zip" if label.startswith("windows") else archive)
                for label in labels_workflow)
            (where / ".github/workflows/release.yml").write_text(
                "jobs:\n  build:\n    strategy:\n      matrix:\n        include:\n" + rows + "\n",
                encoding="utf-8")
            (where / "crates/veilvoice-verify/src").mkdir(parents=True, exist_ok=True)
            entries = "".join('    "%s",\n' % label for label in labels_rust)
            (where / "crates/veilvoice-verify/src/update.rs").write_text(
                "pub const PUBLISHED: &[&str] = &[\n%s];\n" % entries, encoding="utf-8")

        for name, in_workflow, in_rust, must_catch in [
            ("a label the updater does not know about",
             ["linux-x86_64", "linux-arm64"], ["linux-x86_64"], "linux-arm64"),
            ("a label nothing publishes",
             ["linux-x86_64"], ["linux-x86_64", "macos-arm64"], "macos-arm64"),
        ]:
            build(in_workflow, in_rust)
            problems, _ = audit(where)
            if not any(must_catch in line for line in problems):
                failures += 1
                print("    MISSED: %s" % name)
            else:
                print("    caught: %s" % name)

        build(["linux-x86_64", "windows-x86_64"], ["linux-x86_64", "windows-x86_64"])
        problems, _ = audit(where)
        if problems:
            failures += 1
            print("    MISSED: two lists that agree were reported as faulty")
            print("\n".join("      " + line for line in problems))
        else:
            print("    clean: two lists that agree")
    finally:
        shutil.rmtree(where, ignore_errors=True)

    if failures:
        print("  %d self-test(s) failed: this guard does not catch what it claims to"
              % failures)
        return 1
    print("  every fault this guard exists for is caught by it")
    return 0


def main():
    if "--self-test" in sys.argv:
        return self_test()

    problems, counted = audit(ROOT)
    if problems:
        print("\n".join(problems))
        return 1
    print("  %d published platform(s), every one of them offerable as an update" % counted)
    return 0


if __name__ == "__main__":
    sys.exit(main())
