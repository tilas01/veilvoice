#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""The real window, compiled for a browser, and kept from going stale.

    python tools/site/window.py          # build the bundle into website/demo/window
    python tools/site/window.py --check  # it is of this window, or absent and unreferenced
    python tools/site/window.py --self-test

# What this is for

Roadmap item 177 puts the actual interface on the website rather than a model
of it. egui runs in a browser, so the same source that draws the desktop window
compiles to WebAssembly and draws the same window on the page, with its tabs,
its menus and its themes.

That is only worth having if it cannot go out of date. A picture of an old
window is at least obviously a picture; a *running* old window is the thing
itself, wrong, and there is nothing about it to notice. So the bundle carries
what it was built from, and `--check` compares that against the tree.

# What it is built from, exactly

`built-from.txt` beside the bundle records three things:

  * the workspace version, so a release cannot ship a bundle of an older one;
  * the `wasm-bindgen` version, for the reason in the next section;
  * a hash over every source file of every crate the window is built from,
    closed over the manifests rather than listed.

The hash is the useful one, and it is why this is a stronger check than the one
over the window's photographs in `tools/shots/taken.py`. There the honest check
is a version stamp, because a fresh capture differs from a committed one by
antialiasing and the comparison cannot be made exact. Here the input is text.
Hashing it answers exactly the question worth asking, which is whether the
window has changed since this bundle was made, and it answers it without a
build.

# Why the tool's version is pinned to the lockfile

`wasm-bindgen` is a crate and a command-line tool that have to agree. The crate
writes descriptors into the WebAssembly module and the tool reads them, and the
format is internal to the pair. A mismatch is **not** a build failure: it is a
bundle that is produced, committed, published, and then fails in somebody's
browser.

So the version is read out of `Cargo.lock` rather than written here, and a tool
that disagrees with it is refused with the line that installs the right one. A
version written in two places is a version that will disagree in two places.

# What `--check` does in each state

There is no bundle yet, because the window has no browser entry point: see the
end of this note. The check therefore has to be right about three states rather
than two, and the one that matters is the third.

  * **The bundle is absent and no page asks for it.** That is this tree today,
    and it passes. The absence is a roadmap item, not a drift.
  * **The bundle is absent and a page asks for it.** That fails. A page that
    loads a bundle which is not there is a blank rectangle where the interface
    should be, and it is the failure this check exists to make impossible.
  * **The bundle is there and was built from a different window.** That fails,
    which is the whole point.

Being absent is therefore never merely skipped. What makes that safe is the
second state: the only way the bundle can be missing without failing is for
nothing to reference it, and then there is nothing for a reader to find broken.

# What the window still needs before this can run

Measured rather than assumed, on 2026-09-26. Every crate in this workspace
except `veilvoice-gui` compiles for `wasm32-unknown-unknown`, and `ci.yml`
checks it. `veilvoice-gui` needs three things, all of them in its own source
and manifest:

  * `crate-type = ["cdylib", "rlib"]`. The crate has no `[lib]` section, so it
    builds as an rlib, and `wasm-bindgen` cannot make a bundle out of an rlib.
  * An entry point: `eframe::WebRunner` started against a canvas, under
    `#[wasm_bindgen]`, with `wasm-bindgen`, `wasm-bindgen-futures` and
    `web-sys` under a `wasm32` target table.
  * A browser path through two modules that reach for things a browser does not
    have: `graphics.rs` builds an `eframe::NativeOptions` and asks
    `egui_glow` for a `HardwareAcceleration`, and `dialog.rs` uses
    `rfd::FileDialog`, which `rfd` gates out on that target. Those are the
    eleven compile errors and they are all there is; every *dependency* of the
    crate already resolves for the browser.

The chain after that point is proven rather than hoped for: an `eframe` window
of the same version was built for `wasm32-unknown-unknown` with this
repository's pinned toolchain and bundled with this pinned `wasm-bindgen`. It
came to 5.2 MB, 1.9 MB compressed, almost all of it `egui`'s own drawing code
and default fonts. That is the floor for the real window and it is a fair size
for a page; the fonts are a candidate for removing, since this window ships
JetBrains Mono and uses nothing else.

Pure standard library.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))

CRATE = "veilvoice-gui"
# `wasm-bindgen --target web` writes `<crate>.js` and `<crate>_bg.wasm`, with
# the crate's name in Rust's own spelling rather than the package's.
STEM = CRATE.replace("-", "_")

OUT_REL = os.path.join("website", "demo", "window")
OUT = os.path.join(ROOT, OUT_REL)
RECORD = os.path.join(OUT, "built-from.txt")
RECORD_REL = "%s/built-from.txt" % OUT_REL.replace(os.sep, "/")

BUNDLE = ["%s.js" % STEM, "%s_bg.wasm" % STEM]

# Where a page would ask for it. Any page under `website/` naming the bundle
# counts, which is what makes "absent and unreferenced" a state rather than an
# excuse.
WEBSITE = os.path.join(ROOT, "website")

HEADER = [
    "# GENERATED by tools/site/window.py. Do not edit by hand.",
    "#",
    "# What the WebAssembly bundle beside this file was built from. The hash is",
    "# over every source file of every crate the window is built from, so it",
    "# changes when the window changes and not otherwise.",
    "#",
    "# tools/site/window.py --check fails when this disagrees with the tree,",
    "# which is what stops the website running a window the program no longer",
    "# has. See the module note in that file.",
]


# --------------------------------------------------------------- the versions


def read(path):
    with io.open(path, encoding="utf-8") as handle:
        return handle.read()


def workspace_version():
    """The version a release would carry."""
    manifest = read(os.path.join(ROOT, "Cargo.toml"))
    block = re.search(r"\[workspace\.package\]([\s\S]*?)(\n\[|$)", manifest)
    if not block:
        raise SystemExit("Cargo.toml has no [workspace.package]")
    found = re.search(r'^\s*version\s*=\s*"([^"]+)"', block.group(1), re.M)
    if not found:
        raise SystemExit("[workspace.package] declares no version")
    return found.group(1)


def bindgen_version(root=None):
    """The `wasm-bindgen` the lockfile resolved, which the tool must match."""
    lock = read(os.path.join(root or ROOT, "Cargo.lock"))
    found = re.search(
        r'\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"', lock)
    if not found:
        return None
    return found.group(1)


def installed_bindgen():
    """The version of the `wasm-bindgen` on the path, or None."""
    exe = shutil.which("wasm-bindgen")
    if exe is None:
        return None
    try:
        done = subprocess.run([exe, "--version"], capture_output=True,
                              check=False)
    except OSError:
        return None
    found = re.search(r"(\d+\.\d+\.\d+)", (done.stdout or b"").decode("utf-8",
                                                                     "replace"))
    return found.group(1) if found else None


# ------------------------------------------------------------ the fingerprint


WORKSPACE_DEP = re.compile(r"^\s*(veilvoice-[a-z0-9-]+)\s*[.=]", re.M)


def crates_of(package, root=None):
    """The workspace crates `package` is built from, including itself.

    Closed over the manifests rather than listed, for the same reason
    `tools/shots/terminal.py` does it for the command line: a list written down
    is a list that stops being true the next time somebody adds a dependency,
    and it stops being true silently.
    """
    root = root or ROOT
    seen, queue = set(), [package]
    while queue:
        name = queue.pop()
        if name in seen:
            continue
        seen.add(name)
        manifest = os.path.join(root, "crates", name, "Cargo.toml")
        if not os.path.exists(manifest):
            continue
        queue.extend(WORKSPACE_DEP.findall(read(manifest)))
    return sorted(seen)


def fingerprint(root=None):
    """A hash over everything a build of the window reads from this tree.

    The manifests and the lockfile are in it as well as the Rust, because the
    version of `egui` a window is drawn with decides what it looks like as
    surely as the code that calls it does.
    """
    root = root or ROOT
    files = []
    for name in ("Cargo.toml", "Cargo.lock"):
        path = os.path.join(root, name)
        if os.path.exists(path):
            files.append((name, path))
    for crate in crates_of(CRATE, root):
        base = os.path.join(root, "crates", crate)
        for here, dirs, names in os.walk(base):
            dirs[:] = sorted(entry for entry in dirs
                             if entry not in ("target", "__pycache__"))
            for entry in sorted(names):
                if not (entry.endswith(".rs") or entry == "Cargo.toml"):
                    continue
                path = os.path.join(here, entry)
                rel = os.path.relpath(path, root).replace(os.sep, "/")
                files.append((rel, path))

    digest = hashlib.sha256()
    for rel, path in sorted(files):
        # The name goes in as well as the bytes, so moving a file changes the
        # answer. A hash of contents alone calls a rename no change at all.
        digest.update(rel.encode("utf-8"))
        digest.update(b"\0")
        with io.open(path, "rb") as handle:
            digest.update(handle.read())
        digest.update(b"\0")
    return digest.hexdigest()


# ------------------------------------------------------------- the record


def recorded():
    """What the bundle says it was built from, as a dict, or {}."""
    if not os.path.exists(RECORD):
        return {}
    found = {}
    for line in read(RECORD).splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        key, _, value = line.partition(" ")
        found[key] = value.strip()
    return found


def write_record(bindgen):
    lines = list(HEADER)
    lines.append("")
    lines.append("version %s" % workspace_version())
    lines.append("wasm-bindgen %s" % bindgen)
    lines.append("window %s" % fingerprint())
    with io.open(RECORD, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines) + "\n")


def pages_asking():
    """Website pages that load the bundle, by path relative to the root."""
    asking = []
    for here, dirs, names in os.walk(WEBSITE):
        dirs[:] = [entry for entry in dirs if entry != "demo"]
        for entry in sorted(names):
            if not entry.endswith((".html", ".js")):
                continue
            path = os.path.join(here, entry)
            try:
                text = read(path)
            except (OSError, UnicodeDecodeError):
                continue
            if "demo/window/%s" % BUNDLE[0] in text:
                asking.append(os.path.relpath(path, ROOT).replace(os.sep, "/"))
    return asking


# ------------------------------------------------------------------ building


def build():
    want = bindgen_version()
    if want is None:
        print("  Cargo.lock does not resolve wasm-bindgen, so there is no")
        print("  version for the tool to match. Nothing in this workspace")
        print("  reaches it, which means the browser build is not set up yet.")
        return 1

    have = installed_bindgen()
    if have != want:
        print("  wasm-bindgen %s is what Cargo.lock resolved, and %s"
              % (want, "the tool on the path is %s" % have if have
                 else "there is no wasm-bindgen on the path"))
        print()
        print("    The crate writes descriptors into the module and the tool")
        print("    reads them. A mismatch is not a build failure, it is a")
        print("    bundle that fails in somebody's browser. Run:")
        print()
        print("      cargo install wasm-bindgen-cli --version %s --locked" % want)
        return 1

    module = os.path.join(ROOT, "target", "wasm32-unknown-unknown", "release",
                          "%s.wasm" % STEM)
    print("  building %s for wasm32-unknown-unknown" % CRATE)
    done = subprocess.run(
        ["cargo", "build", "--release", "-p", CRATE,
         "--target", "wasm32-unknown-unknown"],
        cwd=ROOT, check=False)
    if done.returncode != 0:
        print()
        print("    The window does not compile for a browser yet. What it")
        print("    needs is in this file's module note, under \"What the")
        print("    window still needs\": a cdylib, an eframe::WebRunner entry")
        print("    point, and a browser path through graphics.rs and")
        print("    dialog.rs. That is window work and it is roadmap item 177.")
        return 1
    if not os.path.exists(module):
        print("    the build reported success and wrote no %s.wasm, which"
              % STEM)
        print("    means the crate is not a cdylib. See the module note.")
        return 1

    os.makedirs(OUT, exist_ok=True)
    done = subprocess.run(
        ["wasm-bindgen", "--target", "web", "--no-typescript",
         "--out-dir", OUT, module], cwd=ROOT, check=False)
    if done.returncode != 0:
        return 1

    write_record(want)
    sizes = []
    for name in BUNDLE:
        path = os.path.join(OUT, name)
        sizes.append("%s %.2f MB" % (name, os.path.getsize(path) / 1048576.0))
    print("  bundled into %s: %s" % (OUT_REL.replace(os.sep, "/"),
                                     ", ".join(sizes)))
    return 0


# ------------------------------------------------------------------ checking


def check():
    present = [name for name in BUNDLE
               if os.path.exists(os.path.join(OUT, name))]
    asking = pages_asking()

    if not present:
        if asking:
            print("  a page loads the window bundle and the bundle is not there:")
            for rel in asking:
                print("    %s asks for %s/%s" % (rel,
                                                 OUT_REL.replace(os.sep, "/"),
                                                 BUNDLE[0]))
            print()
            print("    That is a blank rectangle where the interface should be.")
            print("    Run tools/site/window.py, or take the reference out.")
            return 1
        print("  no window bundle yet, and no page asks for one: roadmap item 177")
        return 0

    problems = []
    for name in BUNDLE:
        if name not in present:
            problems.append("%s/%s is missing, so the bundle is half there"
                            % (OUT_REL.replace(os.sep, "/"), name))

    said = recorded()
    if not said:
        problems.append("%s does not exist, so nothing says which window the "
                        "bundle is of" % RECORD_REL)
    else:
        want_version = workspace_version()
        if said.get("version") != want_version:
            problems.append(
                "the bundle was built from v%s and this tree is v%s"
                % (said.get("version"), want_version))
        want_bindgen = bindgen_version()
        if want_bindgen and said.get("wasm-bindgen") != want_bindgen:
            problems.append(
                "the bundle was written by wasm-bindgen %s and Cargo.lock now "
                "resolves %s, which do not share a descriptor format"
                % (said.get("wasm-bindgen"), want_bindgen))
        if said.get("window") != fingerprint():
            problems.append(
                "the window's source has changed since the bundle was built, "
                "so the page would run an interface this program no longer has")

    if problems:
        print("  the window bundle does not match this tree:")
        for line in problems:
            print("    %s" % line)
        print()
        print("    Build it again, which writes the record as part of doing it:")
        print()
        print("      python tools/site/window.py")
        print()
        print("    A running old window is worse than a photograph of one:")
        print("    there is nothing about it to notice.")
        return 1

    print("  the window bundle is of this tree, v%s, and %d page(s) load it"
          % (said.get("version"), len(asking)))
    return 0


# ----------------------------------------------------------- the self-test


def _write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with io.open(path, "w", encoding="utf-8") as handle:
        handle.write(text)


def self_test():
    import tempfile

    failures = []

    def expect(what, got, want):
        if got != want:
            failures.append("%s: got %r, wanted %r" % (what, got, want))

    root = tempfile.mkdtemp(prefix="veilvoice-window-")
    try:
        _write(os.path.join(root, "Cargo.toml"), "[workspace.package]\n")
        _write(os.path.join(root, "Cargo.lock"),
               '[[package]]\nname = "wasm-bindgen"\nversion = "0.2.128"\n'
               'source = "registry+x"\n')
        _write(os.path.join(root, "crates", CRATE, "Cargo.toml"),
               "[dependencies]\nveilvoice-core.workspace = true\n")
        _write(os.path.join(root, "crates", CRATE, "src", "app.rs"), "// one\n")
        _write(os.path.join(root, "crates", "veilvoice-core", "Cargo.toml"), "\n")
        _write(os.path.join(root, "crates", "veilvoice-core", "src", "lib.rs"),
               "// two\n")
        # Not reachable from the window, so not part of its fingerprint.
        _write(os.path.join(root, "crates", "veilvoice-cli", "Cargo.toml"), "\n")
        _write(os.path.join(root, "crates", "veilvoice-cli", "src", "main.rs"),
               "// three\n")

        expect("the lockfile's wasm-bindgen is read",
               bindgen_version(root), "0.2.128")
        expect("the crate set is closed over the manifests",
               crates_of(CRATE, root), sorted([CRATE, "veilvoice-core"]))

        was = fingerprint(root)

        _write(os.path.join(root, "crates", "veilvoice-cli", "src", "main.rs"),
               "// changed\n")
        expect("a crate the window is not built from does not change it",
               fingerprint(root), was)

        _write(os.path.join(root, "crates", "veilvoice-core", "src", "lib.rs"),
               "// changed\n")
        if fingerprint(root) == was:
            failures.append("a crate the window is built from did not change it")

        _write(os.path.join(root, "crates", "veilvoice-core", "src", "lib.rs"),
               "// two\n")
        expect("and putting it back puts the hash back", fingerprint(root), was)

        # A rename with identical contents has to change the answer, or moving
        # a module past this check is free.
        os.rename(os.path.join(root, "crates", CRATE, "src", "app.rs"),
                  os.path.join(root, "crates", CRATE, "src", "window.rs"))
        if fingerprint(root) == was:
            failures.append("renaming a source file did not change the hash")

        _write(os.path.join(root, "Cargo.lock"),
               '[[package]]\nname = "wasm-bindgen"\nversion = "0.2.129"\n'
               'source = "registry+x"\n')
        expect("a lockfile bump is read as a new version",
               bindgen_version(root), "0.2.129")
    finally:
        shutil.rmtree(root, ignore_errors=True)

    if failures:
        print("  %d self-test(s) failed: this guard does not catch what it "
              "claims to" % len(failures))
        for line in failures:
            print("    %s" % line)
        return 1
    print("  the window bundle's staleness check catches every case it claims to")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--check", action="store_true",
                        help="the bundle is of this window, or absent and unreferenced")
    parser.add_argument("--self-test", action="store_true",
                        help="prove the staleness check catches its cases")
    args = parser.parse_args()

    if args.self_test:
        return self_test()
    if args.check:
        return check()
    return build()


if __name__ == "__main__":
    sys.exit(main())
