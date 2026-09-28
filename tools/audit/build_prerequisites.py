#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""What a fresh machine needs before it can build this, said once.

    python tools/audit/build_prerequisites.py
    python tools/audit/build_prerequisites.py --self-test

# The defect this exists to stop coming back

A reader who wants to build VeilVoice themselves is sent to the "From source"
section of `docs/INSTALL.md`. On Linux, `cargo build --release --workspace`
cannot succeed there without the ALSA headers `cpal` links against. Without
them the reader does not get a sentence saying what to install. They get a
linker error about a missing `alsa.pc`, from a crate they have never heard of,
three levels down somebody else's dependency tree.

`docs/INSTALL.md` named no packages at all, and no minimum Rust version either,
while `rust-toolchain.toml` pins one and `eframe` does not compile on an older
compiler. `docs/CONTRIBUTING.md`, which is not the page a user is sent to, named
`libasound2-dev`, `libgtk-3-dev` and `libxdo-dev`. Nothing in this tree has ever
depended on libxdo, and `Cargo.lock` has never contained a crate that binds it.

# The list is not the workflows' list, and here is why

`ci.yml`, `promote.yml` and `release.yml` install seven packages before
building on Linux. That list was measured and four of them are not needed:

  * `libgtk-3-dev`. `crates/veilvoice-gui/Cargo.toml` declares
    `rfd = { default-features = false, features = ["xdg-portal", "tokio"] }`,
    which is the XDG desktop portal and deliberately not the GTK backend. The
    dependency that gets built is `ashpd`, and it needs no GTK headers.
  * `libxkbcommon-dev` and `libwayland-dev`. `eframe` is declared with
    `default-features = false` and the `wayland` and `x11` features, which
    reach those libraries through Rust crates and by loading them at run time,
    not by linking against headers at build time.
  * `libudev-dev`. There is no crate in `Cargo.lock` that binds udev at all.

Measured rather than reasoned: on a machine with only `libasound2-dev` and
`pkg-config` installed, `cargo build --release --workspace` completes and links
both binaries, `veilvoice` and `veilvoice-gui`.

So this file does not simply copy the workflows into the documentation. A
workflow installing more than it needs costs a runner a few seconds. A document
doing it costs a reader trust: they cannot tell which of the names in the line
is the one that was guessed, and one of them was.

Narrowing the workflows themselves is not done here. It would have to be proven
on a GitHub runner rather than in this container, and being wrong about it turns
every thread's build red.

# What it compares

  * the three workflows must install the same set as each other. They exist to
    be the same, and a release built with fewer packages than CI tested with is
    a release built differently from the thing that was tested.
  * every document telling a person to install these must name the same set as
    every other document. Two pages that disagree mean one of them is wrong and
    a reader cannot tell which.
  * that documented set must be contained in what the workflows install.
    Telling a reader to install something no machine here has ever installed is
    how `libxdo-dev` survived.
  * and it must contain what the build actually needs, which is `REQUIRED`
    below. That list is short enough to be a judgement, so the judgement is
    re-checked rather than trusted: this also reads the window's manifest and
    fails if the backend declarations that make GTK unnecessary have changed.
  * the from-source section of `docs/INSTALL.md` must name the pinned toolchain
    version, because "install Rust" is not an instruction that succeeds on a
    machine with last year's Rust on it.

Nothing here checks macOS or Windows, and that is deliberate rather than an
omission: CI builds and tests on `windows-latest` and `macos-latest` with no
package installation step at all, so "the toolchain and nothing else" is not a
claim that needs a guard. It is a claim a green build makes several times a day.

# Why the step name, and not a line number

The list is found by the name of the step that installs it, which is the same
name in all three workflows. Renaming that step in one of them fails here, which
is right: the three copies exist to be the same, and a rename in one is how they
stop being comparable.

Pure standard library. No YAML parser, because what is being read is a shell
command inside a `run:` block, which a YAML parser hands back as a string to be
parsed anyway.
"""

from __future__ import annotations

import io
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

# The same step name in all three workflows. Anchoring on the name rather than
# on a package means this file has no opinion about what the packages are.
STEP = "Install Linux audio and windowing headers"

AUTHORITY = ".github/workflows/ci.yml"
FOLLOWERS = [".github/workflows/promote.yml", ".github/workflows/release.yml"]

TOOLCHAIN = "rust-toolchain.toml"

# The documents that tell a person to install these by hand, and the heading
# each one's instructions live under. A document with no such heading fails,
# because the instructions have then been deleted or renamed past recognition.
GUIDES = [
    ("docs/INSTALL.md", "From source"),
    ("docs/CONTRIBUTING.md", None),
]

# Every place that names the pinned compiler to a reader. Both of these say
# the version in prose, so both are places for it to go stale the day
# rust-toolchain.toml moves.
VERSIONED = [("docs/INSTALL.md", "From source"),
             ("docs/CONTRIBUTING.md", None)]

# What a Linux machine needs to get from a clone to two binaries. Measured, not
# reasoned: see the module note. `pkg-config` is how the ALSA headers are found
# at all, so it is not optional even though it is not a header.
REQUIRED = {"libasound2-dev", "pkg-config"}

# The window loads this at run time and aborts at startup without it, in a way
# that reads like a crash rather than a missing package. Not needed to build,
# which is why it is named separately here and separately in the guide.
AT_RUNTIME = {"libxkbcommon-x11-0"}

# REQUIRED is a judgement, so the facts it rests on are re-read rather than
# trusted. Each entry is a fragment that must still appear in the window's
# manifest, with the package that would come back if it stopped appearing.
BACKENDS = [
    ("crates/veilvoice-gui/Cargo.toml", 'features = ["xdg-portal", "tokio"]',
     "libgtk-3-dev",
     "the file panel would be GTK's again rather than the desktop portal's"),
    ("crates/veilvoice-gui/Cargo.toml",
     'eframe = { version = "0.36", default-features = false',
     "libgtk-3-dev and the xkbcommon headers",
     "eframe's default features pull in backends this list does not cover"),
]

# A shell continuation is joined before this is applied, so the command is one
# line by the time it is matched. Matching the continuation in the pattern
# itself does not work: the line's own characters consume the trailing
# backslash, the optional group is satisfied by matching nothing, and a regex
# that has already succeeded does not go back to try harder.
INSTALL = re.compile(r"apt(?:-get)?\s+install\b([^\n]*)")
CONTINUED = re.compile(r"\\\n\s*")
FLAG = re.compile(r"^-")


def read(rel):
    with io.open(os.path.join(ROOT, rel), encoding="utf-8") as handle:
        return handle.read()


def step_block(text, name):
    """The lines of the named workflow step, or None.

    Ends at the next `- name:` at the same indentation, which is how a step
    ends in every workflow here.
    """
    lines = text.split("\n")
    start = None
    indent = 0
    for at, line in enumerate(lines):
        if line.strip() == "- name: %s" % name:
            start = at
            indent = len(line) - len(line.lstrip())
            break
    if start is None:
        return None
    for at in range(start + 1, len(lines)):
        line = lines[at]
        if not line.strip():
            continue
        here = len(line) - len(line.lstrip())
        if here <= indent and line.strip().startswith("- "):
            return "\n".join(lines[start:at])
    return "\n".join(lines[start:])


def packages(text):
    """Every `apt install` invocation in `text`, as a list of sets."""
    found = []
    for run in INSTALL.findall(CONTINUED.sub(" ", text)):
        names = set()
        for word in run.split():
            if FLAG.match(word) or word == "\\":
                continue
            # A workflow expression is a list this file cannot read, and the
            # cross-compilation steps use one. Those are not this list.
            if "${{" in word:
                names = set()
                break
            names.add(word)
        if names:
            found.append(names)
    return found


def section(text, heading):
    """The part of `text` under a heading containing `heading`, or None."""
    if heading is None:
        return text
    lines = text.split("\n")
    start = None
    depth = 0
    for at, line in enumerate(lines):
        if line.startswith("#") and heading in line:
            start = at
            depth = len(line) - len(line.lstrip("#"))
            break
    if start is None:
        return None
    for at in range(start + 1, len(lines)):
        line = lines[at]
        if not line.startswith("#"):
            continue
        if len(line) - len(line.lstrip("#")) <= depth:
            return "\n".join(lines[start:at])
    return "\n".join(lines[start:])


def channel(text):
    """The pinned toolchain version from rust-toolchain.toml."""
    found = re.search(r'^\s*channel\s*=\s*"([^"]+)"', text, re.M)
    return found.group(1) if found else None


def relevant(sets, wanted):
    """The invocation that is about these packages, out of possibly several.

    A document may install something else for another purpose, so the one to
    compare against is the one that overlaps. No overlap at all means the
    instructions do not mention these packages, which is the F-222 shape and
    is reported by the caller.
    """
    for names in sets:
        if names & wanted:
            return names
    return None


def problems(files):
    """`files` maps a relative path to its text. Returns a list of sentences."""
    found = []

    for rel, fragment, package, consequence in BACKENDS:
        if fragment not in files[rel]:
            found.append(
                "%s no longer says `%s`, so %s. The package list below was "
                "measured against that declaration and has to be measured "
                "again: %s may be needed now."
                % (rel, fragment, consequence, package))
    if found:
        return found

    block = step_block(files[AUTHORITY], STEP)
    if block is None:
        return ['%s has no step called "%s", so there is no list to compare '
                "against. If it was renamed, rename it here too."
                % (AUTHORITY, STEP)]
    sets = packages(block)
    if not sets:
        return ['%s\'s "%s" step installs nothing this file could read.'
                % (AUTHORITY, STEP)]
    installed = sets[0]

    for rel in FOLLOWERS:
        other = step_block(files[rel], STEP)
        if other is None:
            found.append('%s has no step called "%s". A release must be built '
                         "with what CI tested with." % (rel, STEP))
            continue
        theirs = packages(other)
        theirs = theirs[0] if theirs else set()
        if theirs != installed:
            found.append("%s installs a different set from %s: %s" % (
                rel, AUTHORITY, difference(installed, theirs)))

    documented = None
    first = None
    for rel, heading in GUIDES:
        body = section(files[rel], heading)
        if body is None:
            found.append('%s has no heading containing "%s", so the '
                         "instructions this checks are gone." % (rel, heading))
            continue
        theirs = relevant(packages(body), REQUIRED | installed)
        if theirs is None:
            found.append(
                "%s tells a reader to build on Linux and never says which "
                "headers to install. On a fresh machine that is a linker error "
                "about a package they have not heard of." % rel)
            continue
        if documented is None:
            documented, first = theirs, rel
        elif theirs != documented:
            found.append("%s and %s tell a reader to install different things: "
                         "%s" % (first, rel, difference(documented, theirs)))

    if documented is not None:
        extra = documented - installed
        if extra:
            found.append(
                "the documentation tells a reader to install %s, which no "
                "workflow here installs and no build here has needed. If it is "
                "genuinely needed, the workflows are missing it too."
                % ", ".join(sorted(extra)))
        missing = REQUIRED - documented
        if missing:
            found.append(
                "the documentation does not name %s, without which the build "
                "does not link." % ", ".join(sorted(missing)))
        # Named somewhere in the guide rather than in the install line, because
        # it is needed to run the window and not to build it.
        for rel, _ in GUIDES:
            if rel == "docs/INSTALL.md":
                for name in sorted(AT_RUNTIME):
                    if name not in files[rel]:
                        found.append(
                            "%s never mentions %s, which the window loads at "
                            "startup and aborts without, in a way that reads "
                            "like a crash." % (rel, name))

    pinned = channel(files[TOOLCHAIN])
    if pinned is None:
        found.append("%s pins no channel this file could read." % TOOLCHAIN)
    else:
        for rel, heading in VERSIONED:
            body = section(files[rel], heading)
            if body is not None and pinned not in body:
                found.append(
                    "%s does not name the pinned toolchain %s, so a reader "
                    "with an older Rust gets a compiler error rather than a "
                    "version to install." % (rel, pinned))
    return found


def difference(wanted, theirs):
    bits = []
    missing = sorted(wanted - theirs)
    extra = sorted(theirs - wanted)
    if missing:
        bits.append("missing %s" % ", ".join(missing))
    if extra:
        bits.append("and names %s, which nothing here needs"
                    % ", ".join(extra))
    return "; ".join(bits) if bits else "an unexplained difference"


def main():
    if "--self-test" in sys.argv:
        return self_test()

    files = {}
    wanted = [AUTHORITY, TOOLCHAIN] + FOLLOWERS + [g[0] for g in GUIDES]
    wanted += [rel for rel, _, _, _ in BACKENDS]
    for rel in dict.fromkeys(wanted):
        files[rel] = read(rel)

    found = problems(files)
    if found:
        print("  the build prerequisites are not said the same way everywhere:")
        for line in found:
            print("    %s" % line)
        print()
        print("    A person following the documentation should need exactly")
        print("    what the build needs, and nothing more. %s"
              % ", ".join(sorted(REQUIRED)))
        print("    to build, %s to run the window. The workflows"
              % ", ".join(sorted(AT_RUNTIME)))
        print("    install more than that on purpose; see this file's note.")
        return 1

    block = step_block(files[AUTHORITY], STEP)
    installed = sorted(packages(block)[0])
    body = section(files["docs/INSTALL.md"], VERSIONED[0][1])
    documented = sorted(relevant(packages(body), REQUIRED))
    print("  to build on Linux: %s, named the same way in every guide"
          % ", ".join(documented))
    print("  the %d workflow package(s) cover them: %s"
          % (len(installed), ", ".join(installed)))
    return 0


# ------------------------------------------------------------- the self-test


WORKFLOW = """jobs:
  test:
    steps:
      - uses: actions/checkout@v7

      # cpal needs ALSA headers, and eframe needs the X11/Wayland stack.
      - name: Install Linux audio and windowing headers
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends \\
            libasound2-dev libudev-dev pkg-config \\
            libgtk-3-dev libxkbcommon-x11-0

      - name: Install the pinned toolchain
        run: rustup show

      - name: Something else entirely
        run: sudo apt-get install -y xvfb strace
"""

FOLLOWER = """jobs:
  release:
    steps:
      - name: Install Linux audio and windowing headers
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends \\
            libasound2-dev libudev-dev pkg-config \\
            libgtk-3-dev libxkbcommon-x11-0
"""

GUIDE = """## 3. From source

Rust 1.96.0 or newer.

```bash
sudo apt-get install -y libasound2-dev pkg-config
cargo build --release --workspace
```

To run the window you also need libxkbcommon-x11-0.

## 4. After you have it
"""

CONTRIB = """# Contributing

Needs the toolchain rust-toolchain.toml pins, 1.96.0.

```bash
sudo apt-get install -y libasound2-dev pkg-config
cargo build --workspace
```
"""

PIN = '[toolchain]\nchannel = "1.96.0"\n'

MANIFEST = """[dependencies]
eframe = { version = "0.36", default-features = false, features = ["glow"] }
rfd = { version = "0.16", default-features = false, features = ["xdg-portal", "tokio"] }
"""


def tree(**changes):
    files = {AUTHORITY: WORKFLOW, TOOLCHAIN: PIN,
             "docs/INSTALL.md": GUIDE, "docs/CONTRIBUTING.md": CONTRIB,
             "crates/veilvoice-gui/Cargo.toml": MANIFEST}
    for rel in FOLLOWERS:
        files[rel] = FOLLOWER
    files.update(changes)
    return files


def self_test():
    failures = []

    def expect(what, got, want):
        if got != want:
            failures.append("%s: got %r, wanted %r" % (what, got, want))

    def one(what, files, must_name=None):
        found = problems(files)
        if len(found) != 1:
            failures.append("%s: got %r, wanted one problem" % (what, found))
            return
        if must_name and must_name not in found[0]:
            failures.append("%s: must name %r, said %r"
                            % (what, must_name, found[0]))

    expect("a tree that agrees with itself has nothing to report",
           problems(tree()), [])

    expect("the list is read from the named step and not from the file",
           sorted(packages(step_block(WORKFLOW, STEP))[0]),
           ["libasound2-dev", "libgtk-3-dev", "libudev-dev",
            "libxkbcommon-x11-0", "pkg-config"])

    # The step ends before the next one, so an unrelated apt line further down
    # is not part of the list. Without that, xvfb would be a prerequisite.
    block = step_block(WORKFLOW, STEP)
    if "xvfb" in block or "rustup" in block:
        failures.append("the step must end at the next step")

    # The shape this was found by: a guide that gives a build command and never
    # says what to install first.
    one("a guide that names no packages at all is reported",
        tree(**{"docs/INSTALL.md": GUIDE.replace(
            "sudo apt-get install -y libasound2-dev pkg-config\n", "")}),
        "linker error")

    # The real defect in docs/CONTRIBUTING.md: a package nothing needs.
    one("a package no workflow installs is reported",
        tree(**{"docs/CONTRIBUTING.md": CONTRIB.replace(
            "libasound2-dev pkg-config",
            "libasound2-dev pkg-config libxdo-dev")}),
        "libxdo-dev")

    # And the other direction, which is what a linker error actually is.
    one("a guide missing something the build needs is reported",
        tree(**{"docs/INSTALL.md": GUIDE.replace(
            "libasound2-dev pkg-config", "pkg-config"),
                "docs/CONTRIBUTING.md": CONTRIB.replace(
            "libasound2-dev pkg-config", "pkg-config")}),
        "libasound2-dev")

    # Two guides that disagree: one of them is wrong and a reader cannot tell
    # which, so this fails before asking which set is right.
    one("two guides that disagree with each other are reported",
        tree(**{"docs/CONTRIBUTING.md": CONTRIB.replace(
            "libasound2-dev pkg-config",
            "libasound2-dev pkg-config libgtk-3-dev")}),
        "different things")

    one("a workflow installing less than CI is reported",
        tree(**{FOLLOWERS[0]: FOLLOWER.replace(" libgtk-3-dev", "")}))

    one("a renamed step in a follower is reported rather than skipped",
        tree(**{FOLLOWERS[1]: FOLLOWER.replace(STEP, "Deps")}))

    one("a renamed step in the authority stops everything",
        tree(**{AUTHORITY: WORKFLOW.replace(STEP, "Deps")}))

    one("a guide with no from-source heading is reported",
        tree(**{"docs/INSTALL.md":
                GUIDE.replace("3. From source", "3. Elsewhere")}))

    one("a guide with no minimum Rust version is reported",
        tree(**{"docs/INSTALL.md": GUIDE.replace("Rust 1.96.0 or newer.\n", "")}),
        "1.96.0")

    one("and so is the other one, since both say the version in prose",
        tree(**{"docs/CONTRIBUTING.md": CONTRIB.replace(
            "Needs the toolchain rust-toolchain.toml pins, 1.96.0.\n", "")}),
        "docs/CONTRIBUTING.md")

    one("a guide never naming the library the window loads is reported",
        tree(**{"docs/INSTALL.md": GUIDE.replace(
            "To run the window you also need libxkbcommon-x11-0.\n", "")}),
        "libxkbcommon-x11-0")

    # The judgement behind REQUIRED is re-read rather than trusted. Turning the
    # GTK file panel back on must stop this file, not be quietly absorbed.
    one("the GTK backend coming back stops the check",
        tree(**{"crates/veilvoice-gui/Cargo.toml": MANIFEST.replace(
            'features = ["xdg-portal", "tokio"]', 'features = ["gtk3"]')}),
        "libgtk-3-dev")

    one("and so does eframe getting its default features back",
        tree(**{"crates/veilvoice-gui/Cargo.toml": MANIFEST.replace(
            'eframe = { version = "0.36", default-features = false',
            'eframe = { version = "0.36"')}),
        "measured")

    # A cross-compilation step installs a list this file cannot read, and must
    # be ignored rather than compared against, or every release fails here.
    if not problems(tree(**{FOLLOWERS[0]: FOLLOWER.replace(
            "libasound2-dev libudev-dev pkg-config \\\n            "
            "libgtk-3-dev libxkbcommon-x11-0", "${{ matrix.packages }}")})):
        failures.append("an unreadable package expression must not pass as a "
                        "matching list")

    if failures:
        print("  %d self-test(s) failed: this guard does not catch what it "
              "claims to" % len(failures))
        for line in failures:
            print("    %s" % line)
        return 1
    print("  the build-prerequisite check catches every case it claims to")
    return 0


if __name__ == "__main__":
    sys.exit(main())
