#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
The workflows and the Dependabot configuration pass zizmor, and stay hardened.

# What this adds to the guards beside it

`actions.py`, `checkout_tokens.py`, `job_limits.py` and `dependabot.py` each
hold one rule this project decided on. zizmor (https://docs.zizmor.sh) is a
static auditor for GitHub Actions that knows the rest of the published
attacks: script injection through `${{ }}`, dangerous triggers, credentials
left in artefacts, unpinned or impostor actions, cache poisoning, missing
Dependabot cooldowns and more. Running it here means a new kind of mistake is
caught by somebody else's research instead of after it has been made.

# Checking

By default this runs zizmor offline against the repository and fails on any
finding it reports at its regular level. A finding that has been reviewed and
must stay is silenced on its own line with

    # zizmor: ignore[rule] the reason it is safe here

and this check refuses an ignore comment that gives no reason, so an
exception can never be silent.

# Hardening automatically

`--fix` runs zizmor's safe fixes, then audits again, and keeps the result only
when there are fewer findings than before and the other workflow guards still
pass. Otherwise every file is put back exactly as it was. `tools/verify.py`
runs it before the checks, so a fix zizmor knows how to make is made, proved
and left in the working tree for the next commit.

zizmor is installed from `tools/audit/requirements.txt`, pinned by hash. When
it is missing this says so and passes, unless VEILVOICE_REQUIRE_ZIZMOR=1 is
set, as CI sets it, in which case a missing auditor is a failure.

Pure standard library apart from zizmor itself. `--self-test` checks the
reason rule and the summary without running zizmor.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
GITHUB = os.path.join(ROOT, ".github")

# The guards that must still pass after a fix is applied.
GUARDS = ("actions.py", "checkout_tokens.py", "job_limits.py", "dependabot.py")

IGNORE_RE = re.compile(r"#\s*zizmor:\s*ignore\[([^\]]*)\](.*)$")

INSTALL = ("python -m pip install --require-hashes "
           "-r tools/audit/requirements.txt")


def zizmor() -> str | None:
    return shutil.which("zizmor")


def audit(binary: str) -> list[dict]:
    """zizmor's findings for the repository, not counting ignored ones."""
    run = subprocess.run(
        [binary, "--offline", "--no-progress", "--format", "json", ROOT],
        capture_output=True, text=True, timeout=300)
    # zizmor exits non-zero when it has findings; only unreadable output is
    # an error.
    try:
        found = json.loads(run.stdout or "[]")
    except json.JSONDecodeError:
        sys.stderr.write(run.stderr)
        raise SystemExit("zizmor did not produce a report (exit %d)"
                         % run.returncode)
    return [f for f in found if not f.get("ignored")]


def summary(finding: dict) -> str:
    """`path:line  rule (severity): what` for one finding."""
    where = "?"
    for location in finding.get("locations", []):
        if location.get("symbolic", {}).get("kind") != "Primary":
            continue
        key = location["symbolic"].get("key", {}).get("Local", {})
        path = key.get("verbatim_path") or key.get("given_path") or "?"
        path = os.path.relpath(os.path.join(ROOT, path), ROOT) \
            if not os.path.isabs(path) else os.path.relpath(path, ROOT)
        row = location["concrete"]["location"]["start_point"]["row"] + 1
        where = "%s:%d" % (path.replace(os.sep, "/"), row)
        break
    severity = finding.get("determinations", {}).get("severity", "?")
    return "%s  %s (%s): %s" % (where, finding.get("ident", "?"),
                                severity.lower(), finding.get("desc", ""))


def silent_ignores(text: str) -> list[tuple[int, str]]:
    """(line, rule) for each `zizmor: ignore[...]` with no reason after it."""
    found = []
    for number, line in enumerate(text.splitlines(), 1):
        match = IGNORE_RE.search(line)
        if match and len(match.group(2).strip(" -:;,.")) < 10:
            found.append((number, match.group(1)))
    return found


def configuration_files() -> list[str]:
    files = []
    for folder, _dirs, names in os.walk(GITHUB):
        for name in names:
            if name.endswith((".yml", ".yaml")):
                files.append(os.path.join(folder, name))
    return sorted(files)


def check() -> int:
    bad = 0
    for path in configuration_files():
        with open(path, encoding="utf-8") as handle:
            for line, rule in silent_ignores(handle.read()):
                print("%s:%d  ignores %s without saying why"
                      % (os.path.relpath(path, ROOT), line, rule))
                bad += 1

    binary = zizmor()
    if binary is None:
        if os.environ.get("VEILVOICE_REQUIRE_ZIZMOR") == "1":
            print("zizmor is not installed, and this run requires it:\n    "
                  + INSTALL)
            return 1
        print("  zizmor is not installed, so the workflows were not audited "
              "here (CI does). To run it:\n    " + INSTALL)
        return 1 if bad else 0

    findings = audit(binary)
    for finding in findings:
        print(summary(finding))
    if findings:
        print("\n%d finding(s). Fix them, or run `python tools/audit/"
              "workflows.py --fix` for the ones zizmor can fix itself. A "
              "finding that is safe here is silenced on its line with "
              "`# zizmor: ignore[rule] reason`." % len(findings))
    if findings or bad:
        return 1
    print("  zizmor reports nothing, and every exception gives its reason")
    return 0


def guards_pass() -> bool:
    for guard in GUARDS:
        run = subprocess.run([sys.executable, os.path.join(HERE, guard)],
                             capture_output=True, text=True, cwd=ROOT)
        if run.returncode != 0:
            print("  %s fails after the fix:\n%s" % (guard, run.stdout[-2000:]))
            return False
    return True


def fix() -> int:
    binary = zizmor()
    if binary is None:
        print("  zizmor is not installed, so nothing was hardened here")
        return 0
    before = audit(binary)
    if not any(f.get("fixes") for f in before):
        print("  nothing zizmor can fix by itself")
        return 0

    saved = {}
    for path in configuration_files():
        with open(path, "rb") as handle:
            saved[path] = handle.read()

    subprocess.run([binary, "--offline", "--no-progress", "--fix=safe", ROOT],
                   capture_output=True, text=True, timeout=300)
    after = audit(binary)

    if len(after) < len(before) and guards_pass():
        changed = [os.path.relpath(p, ROOT) for p, data in saved.items()
                   if open(p, "rb").read() != data]
        print("  hardened %s: %d finding(s) fixed, %d left"
              % (", ".join(changed) or "nothing", len(before) - len(after),
                 len(after)))
        return 0

    for path, data in saved.items():
        with open(path, "wb") as handle:
            handle.write(data)
    print("  zizmor's fixes did not make the workflows safer, so every file "
          "is back as it was")
    return 0


def self_test() -> int:
    bad = 0
    text = "\n".join([
        "on:",
        "  workflow_run:  # zizmor: ignore[dangerous-triggers] only release triggers it",
        "  push:  # zizmor: ignore[dangerous-triggers]",
        "  x: 1  # zizmor: ignore[artipacked] ok",
        "  y: 2  # an ordinary comment",
    ])
    if silent_ignores(text) != [(3, "dangerous-triggers"), (4, "artipacked")]:
        print("  the reason rule is wrong: %r" % silent_ignores(text))
        bad += 1

    finding = {
        "ident": "artipacked", "desc": "credential persistence",
        "determinations": {"severity": "Medium"},
        "locations": [
            {"symbolic": {"kind": "Related",
                          "key": {"Local": {"verbatim_path": "./x.yml"}}},
             "concrete": {"location": {"start_point": {"row": 1}}}},
            {"symbolic": {"kind": "Primary",
                          "key": {"Local": {"verbatim_path":
                                            "./.github/workflows/a.yml"}}},
             "concrete": {"location": {"start_point": {"row": 39}}}},
        ],
    }
    expected = ".github/workflows/a.yml:40  artipacked (medium): credential persistence"
    if summary(finding) != expected:
        print("  the summary is wrong: %r" % summary(finding))
        bad += 1
    if bad:
        return 1
    print("  an ignore without a reason is refused, and findings name their line")
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        sys.exit(self_test())
    sys.exit(fix() if "--fix" in sys.argv else check())
