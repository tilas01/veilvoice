#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""
Every CodeQL finding, written where somebody reading the run can see it.

# Why the run says what it found

CodeQL sends its findings to the Security tab and nowhere else. The tab is
only open to people with write access to the repository, it shows one alert at
a time, and an automated reviewer working from the run's log sees nothing at
all. This prints each finding of a SARIF file as one line in the job's log,
rule, level, place and message, and adds a count per rule to the job summary,
so a run can be read and two runs compared without the tab.

It reads the files the analysis already wrote and changes nothing: the upload
to the Security tab is the analysis step's own and happens before this runs.

Usage: `sarif_summary.py <file-or-folder>...`. Folders are searched for
`*.sarif`. A missing path is an error, because a summary of nothing would read
as a clean run. Pure standard library. `--self-test` checks it against a known
SARIF shape.
"""

from __future__ import annotations

import collections
import json
import os
import sys

# A message is cut here in the log. The Security tab has the whole of it.
MESSAGE_LIMIT = 240


def sarif_files(paths):
    """Every SARIF file named, or found under a folder named, in order."""
    found = []
    for path in paths:
        if os.path.isdir(path):
            for name in sorted(os.listdir(path)):
                if name.endswith(".sarif"):
                    found.append(os.path.join(path, name))
        elif os.path.isfile(path):
            found.append(path)
        else:
            raise FileNotFoundError("no SARIF file or folder at %s" % path)
    return found


def findings(sarif):
    """(rule, level, place, message) for every result in a parsed SARIF log."""
    out = []
    for run in sarif.get("runs", []):
        levels = {}
        for rule in run.get("tool", {}).get("driver", {}).get("rules", []):
            default = rule.get("defaultConfiguration", {}).get("level")
            if default:
                levels[rule.get("id")] = default
        for result in run.get("results", []):
            rule = result.get("ruleId") or result.get("rule", {}).get("id", "?")
            level = result.get("level") or levels.get(rule, "warning")
            place = "?"
            locations = result.get("locations") or []
            if locations:
                physical = locations[0].get("physicalLocation", {})
                uri = physical.get("artifactLocation", {}).get("uri", "?")
                line = physical.get("region", {}).get("startLine")
                place = "%s:%s" % (uri, line) if line else uri
            message = " ".join(result.get("message", {}).get("text", "").split())
            if len(message) > MESSAGE_LIMIT:
                message = message[: MESSAGE_LIMIT - 3] + "..."
            out.append((rule, level, place, message))
    return out


def summarise(paths, log=sys.stdout, summary=None):
    """Print every finding to `log` and a count per rule to `summary`."""
    every = []
    for path in sarif_files(paths):
        with open(path, encoding="utf-8") as handle:
            every.extend(findings(json.load(handle)))
    every.sort()
    for rule, level, place, message in every:
        log.write("%s\t%s\t%s\t%s\n" % (rule, level, place, message))
    counts = collections.Counter(rule for rule, _, _, _ in every)
    log.write("%d finding(s) under %d rule(s)\n" % (len(every), len(counts)))
    if summary is not None:
        summary.write("### CodeQL findings: %d\n\n" % len(every))
        if counts:
            summary.write("| Rule | Findings |\n|---|---:|\n")
            for rule, count in sorted(counts.items(), key=lambda kv: (-kv[1], kv[0])):
                summary.write("| `%s` | %d |\n" % (rule, count))
            summary.write("\n")
    return every


def self_test():
    import io

    sarif = {
        "runs": [{
            "tool": {"driver": {"rules": [
                {"id": "rust/path-injection", "defaultConfiguration": {"level": "error"}},
            ]}},
            "results": [
                {
                    "ruleId": "rust/path-injection",
                    "message": {"text": "This path depends on\na user-provided value."},
                    "locations": [{"physicalLocation": {
                        "artifactLocation": {"uri": "crates/a/src/lib.rs"},
                        "region": {"startLine": 7},
                    }}],
                },
                {"ruleId": "py/clear-text-logging", "level": "note",
                 "message": {"text": "x" * 400}},
            ],
        }],
    }
    got = findings(sarif)
    assert got[0] == ("rust/path-injection", "error", "crates/a/src/lib.rs:7",
                      "This path depends on a user-provided value."), got[0]
    assert got[1][1] == "note" and got[1][2] == "?", got[1]
    assert len(got[1][3]) == MESSAGE_LIMIT and got[1][3].endswith("..."), got[1]
    assert findings({}) == []
    try:
        sarif_files(["/nonexistent/results"])
    except FileNotFoundError:
        pass
    else:
        raise AssertionError("a missing results folder must not read as a clean run")
    log, summary = io.StringIO(), io.StringIO()
    import tempfile
    with tempfile.TemporaryDirectory() as folder:
        with open(os.path.join(folder, "rust.sarif"), "w", encoding="utf-8") as handle:
            json.dump(sarif, handle)
        summarise([folder], log, summary)
    assert "2 finding(s) under 2 rule(s)" in log.getvalue(), log.getvalue()
    assert "| `rust/path-injection` | 1 |" in summary.getvalue(), summary.getvalue()
    print("sarif_summary: self-test passed")


def main(argv):
    if argv[1:] == ["--self-test"]:
        self_test()
        return 0
    if len(argv) < 2:
        sys.stderr.write(__doc__)
        return 2
    target = os.environ.get("GITHUB_STEP_SUMMARY")
    if target:
        with open(target, "a", encoding="utf-8") as summary:
            summarise(argv[1:], summary=summary)
    else:
        summarise(argv[1:])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
