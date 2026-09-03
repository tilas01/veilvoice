#!/usr/bin/env python3
"""Find state files that one part of VeilVoice writes and another reads.

F-141 was this: the desktop application created app locks through
`LockStore::create`, which writes one file, and loaded them through
`open_default`, which reads another. Both were correct on their own. Nothing
compared them, so a lock set in the window was written somewhere the window
never looked, and the app lock had never worked from the GUI.

F-142 was the same shape a week later, in new code: three files were migrated
into the obfuscated store and shredded, and only one of them was ever read back
out of it.

The shape is: **two spellings of where something lives.** This looks for it.

What it checks
--------------

1. Every literal state filename (`something.conf`, `something.manifest`) that
   appears in more than one crate is derived the same way in each. Two crates
   spelling one file's location differently is the F-141 defect exactly.

2. No state filename is written in one crate and never read in any.

What it cannot check
--------------------

It reads text, so it sees spellings rather than behaviour: two derivations that
differ textually but agree at runtime are reported, and two that agree
textually while a function underneath them disagrees are not. It is a net with
a known mesh size, not a proof, and the tests beside each feature are what
prove the behaviour.

SPDX-License-Identifier: GPL-3.0-or-later
"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

#: Filenames that are state rather than source: things VeilVoice keeps between
#: runs, which is where this class of defect lives.
STATE = re.compile(r'"([a-z0-9][a-z0-9._-]*\.(?:conf|manifest|bin|dat|txt|json))"')

#: How a path is built around that filename, on the same line.
DERIVATION = re.compile(r"(default_path\(\)|default_dir\(\)|with_file_name|join)")


def crate_of(path: Path) -> str:
    parts = path.relative_to(ROOT).parts
    return parts[1] if parts[0] == "crates" else parts[0]


def functions(text: str) -> list[str]:
    """Split a Rust source into function bodies, roughly.

    Roughly is enough and precise would be a parser. What matters is that a
    derivation split over several lines -- which is what `rustfmt` does to a
    long one -- is read as one thing. The first version of this scanned line by
    line and reported `integrity.manifest` as built two ways across the command
    line and the window, when both build it identically and one of them simply
    wrapped. A detector whose first finding is its own formatting is a detector
    nobody will keep.
    """
    out: list[str] = []
    current: list[str] = []
    for line in text.splitlines():
        if re.match(r"\s*(pub(\([a-z]+\))?\s+)?(async\s+)?fn\s", line):
            if current:
                out.append("\n".join(current))
            current = [line]
        elif current:
            current.append(line)
    if current:
        out.append("\n".join(current))
    return out


def scan() -> tuple[dict, dict]:
    """Where each state filename appears, and how its path is built."""
    places: dict[str, set[str]] = defaultdict(set)
    shapes: dict[str, set[str]] = defaultdict(set)

    for source in sorted((ROOT / "crates").rglob("*.rs")):
        text = source.read_text(encoding="utf-8", errors="replace")
        # Test modules build paths in temporary directories on purpose.
        at = text.find("mod tests")
        if at != -1:
            text = text[:at]
        # Comments describe paths without building them.
        text = "\n".join(
            line for line in text.splitlines() if not line.strip().startswith("//")
        )
        for body in functions(text):
            names = set(STATE.findall(body))
            if not names:
                continue
            shape = "+".join(sorted(set(DERIVATION.findall(body)))) or "literal"
            for name in names:
                places[name].add(crate_of(source))
                shapes[name].add(shape)
    return places, shapes


def check() -> list[str]:
    problems: list[str] = []
    places, shapes = scan()

    for name, crates in sorted(places.items()):
        if len(crates) < 2:
            continue
        built = shapes[name]
        if len(built) > 1:
            problems.append(
                f"{name}: built {len(built)} different ways across "
                f"{', '.join(sorted(crates))} -- {sorted(built)}. "
                f"Two spellings of one file's location is F-141."
            )
    return problems


def main() -> int:
    problems = check()
    places, _ = scan()
    shared = {n: c for n, c in places.items() if len(c) > 1}
    if problems:
        for line in problems:
            print(f"  {line}")
        print(f"\n{len(problems)} state path(s) spelled more than one way.")
        return 1
    print(
        f"every shared state file is derived one way "
        f"({len(shared)} shared across crates, {len(places)} in total)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
