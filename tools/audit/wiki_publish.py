#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""A wiki with no repository behind it skips the publish, and does not fail it.

    python tools/audit/wiki_publish.py               # report, and fail on any gap
    python tools/audit/wiki_publish.py --self-test   # show the reading can fail

No `--check`, for the reason the other tools here have none: this only reads,
so reporting and checking are one run.

# What this is for

`.github/workflows/wiki.yml` publishes `wiki/` to the repository's GitHub wiki
from `main`. A GitHub wiki repository does not exist until somebody saves a
first page through the web interface, and until then the publish job could
only fail. It did, on every release merge, with a red cross on `main` for a
setting no change to the code could fix. That is F-259.

The job now answers the question in one step, `id: wiki`, which writes
`ready=true` when there is a wiki to push to and `ready=false`, with a warning
and a summary, when there is not. Every step after it reads that answer.

The shape is easy to lose in an edit: a new step added without the condition
runs on the skipped path and fails on a directory that is not there, and an
`exit 1` restored to the refusal puts the red cross back. Neither shows until a
release merge, which is the worst moment to find out, so this reads the
workflow and says so before then.

Pure standard library, like everything in `tools/`.
"""

import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORKFLOW = os.path.join(ROOT, ".github", "workflows", "wiki.yml")

STEP_ID = "wiki"
CONDITION = "steps.%s.outputs.ready == 'true'" % STEP_ID


def job_lines(text, job):
    """The lines of one job, from its key to the next job's key."""
    lines = text.splitlines()
    try:
        start = lines.index("  %s:" % job)
    except ValueError:
        return []
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if re.match(r"^  [A-Za-z0-9_-]+:\s*$", lines[index]):
            end = index
            break
    return lines[start:end]


def steps(lines):
    """Each step of a job, as the list of its lines.

    A step starts at a `- ` at the indentation of the first item under the
    job's `steps:` key. A comment between steps belongs to no step, because a
    condition in a comment is not a condition.
    """
    found, current, key, indent = [], None, None, None
    for line in lines:
        stripped = line.lstrip()
        depth = len(line) - len(stripped)
        if key is None:
            if re.match(r"^\s*steps:\s*$", line):
                key = depth
            continue
        if not stripped or stripped.startswith("#"):
            continue
        if depth <= key:
            break
        if indent is None and stripped.startswith("- "):
            indent = depth
        if stripped.startswith("- ") and depth == indent:
            current = [line]
            found.append(current)
        elif current is not None:
            current.append(line)
    return found


def gaps(text):
    """Every way the workflow fails to skip cleanly, as sentences."""
    problems = []
    publish = job_lines(text, "publish")
    if not publish:
        return ["wiki.yml has no `publish` job"]

    every = steps(publish)
    answering = [s for s in every
                 if any(re.match(r"^\s*id:\s*%s\s*$" % STEP_ID, l) for l in s)]
    if not answering:
        return ["no step in `publish` has `id: %s`, so nothing says whether "
                "there is a wiki to publish to" % STEP_ID]
    step = answering[0]
    body = "\n".join(step)

    for value in ("true", "false"):
        if not re.search(r'echo "ready=%s" >> "\$GITHUB_OUTPUT"' % value, body):
            problems.append("the `%s` step never writes `ready=%s`"
                            % (STEP_ID, value))

    # The refusal path is everything after the last `ready=false`. It must warn
    # and stop without failing.
    tail = body.rsplit("ready=false", 1)[-1] if "ready=false" in body else ""
    if re.search(r"^\s*exit\s+[1-9]", tail, re.M):
        problems.append("the path with no wiki repository still exits non-zero, "
                        "which fails `main` on every release merge")
    if "::error::" in tail:
        problems.append("the path with no wiki repository reports an error, "
                        "not a warning")
    if "::warning::" not in tail:
        problems.append("the path with no wiki repository skips without a "
                        "warning, which reads exactly like a publish")

    after = every[every.index(step) + 1:]
    if not after:
        problems.append("nothing follows the `%s` step, so nothing publishes"
                        % STEP_ID)
    for later in after:
        name = next((re.sub(r"^\s*-?\s*name:\s*", "", l) for l in later
                     if re.match(r"^\s*-?\s*name:", l)), later[0].strip())
        conditions = [l for l in later if re.match(r"^\s*if:", l)]
        if not any(CONDITION in l for l in conditions):
            problems.append("step %r runs whether or not there is a wiki; it "
                            "needs `if: %s`" % (name, CONDITION))
    return problems


def self_test():
    """Break the real workflow each way the guard claims to see, and look."""
    with open(WORKFLOW, encoding="utf-8") as handle:
        good = handle.read()
    cases = [
        ("a later step without the condition",
         lambda t: t.replace("        if: %s\n        working-directory"
                             % CONDITION, "        working-directory", 1)),
        ("the refusal failing again",
         lambda t: t.replace('} >> "$GITHUB_STEP_SUMMARY"\n',
                             '} >> "$GITHUB_STEP_SUMMARY"\n          exit 1\n', 1)),
        ("the refusal reported as an error",
         lambda t: t.replace("::warning::The wiki was not published",
                             "::error::The wiki was not published", 1)),
        ("the answer never given",
         lambda t: t.replace('echo "ready=false" >> "$GITHUB_OUTPUT"', "", 1)),
        ("the step that answers losing its id",
         lambda t: t.replace("        id: %s\n" % STEP_ID, "", 1)),
    ]
    failed = 0
    if gaps(good):
        print("  the real workflow fails, so the self-test proves nothing:")
        for problem in gaps(good):
            print("    - %s" % problem)
        return 1
    for label, mutate in cases:
        broken = mutate(good)
        if broken == good:
            print("  FAIL %s: the mutation did not apply" % label)
            failed += 1
        elif not gaps(broken):
            print("  FAIL %s: not caught" % label)
            failed += 1
        else:
            print("  ok   %s" % label)
    if failed:
        print("  %d self-test(s) failed: this guard does not catch what it "
              "claims to" % failed)
        return 1
    print("  every break was caught")
    return 0


def main():
    if "--self-test" in sys.argv:
        return self_test()
    with open(WORKFLOW, encoding="utf-8") as handle:
        problems = gaps(handle.read())
    if problems:
        for problem in problems:
            print("  %s" % problem)
        return 1
    print("  wiki.yml skips the publish cleanly when there is no wiki yet")
    return 0


if __name__ == "__main__":
    sys.exit(main())
