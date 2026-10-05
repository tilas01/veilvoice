#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""No PowerShell function adds to a script's variable and keeps the result to itself.

    python tools/audit/powershell.py               # read every .ps1 here
    python tools/audit/powershell.py --self-test   # and prove it can fail

# What this is for

PowerShell scopes a variable to the function that assigns it. Reading a name
inside a function finds the script's variable when the function has none of
its own, and assigning to it makes one. So a line like

    $problems += "the window never appeared"

inside a function reads the script's list, appends to it, and stores the result
in a new variable local to the function, which is thrown away when the function
returns. The script's `$problems` never changes. Nothing reports it: the line
runs, and it even looks as though it worked if the function prints the list.

That is F-241. `tools/shots/gui.ps1` gathers what went wrong while photographing
the window into `$problems`, and fails at the end if the list is not empty.
When the capture moved into a function, `Shoot`, so that the Settings pages
could be photographed by the same code as the tabs, all four of its
`$problems +=` lines went on writing to the function's own copy. A window that
never appeared, a capture that measured nothing, a refused `PrintWindow` and two
tabs coming out identical were each noticed and then forgotten, and the script
said how many captures it had taken and exited 0.

# What it reads

Every `.ps1` file in the repository. Inside each `function` it looks for a
compound assignment, `+=`, `-=`, `*=`, `/=`, `++` or `--`, to a name the script
also assigns at its own level, written without `$script:` or `$global:`. Those
are the ones that are always a mistake: the operator reads the outer value, so
the author meant the outer variable. A plain `=` inside a function is left
alone, because a local of the same name is sometimes the point: `install.ps1`
sets `$ErrorActionPreference` inside `Invoke-Gpg` for exactly as long as that
function runs.

It reads text and does not parse PowerShell, so it is deliberately narrow:
braces are counted to find where a function ends, and anything after a `#` is
a comment. That is enough for the scripts here, and the self-test holds it to
the shapes that matter.

Pure standard library, like everything in `tools/`.
"""

import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# `$name` then a compound operator, or `$name++` / `$name--`. The lookbehind
# keeps `$script:name` and `$global:name` out, since those are the fix.
COMPOUND = re.compile(r"(?<![\w:])\$([A-Za-z_]\w*)\s*(?:[-+*/]=|\+\+|--)")
# Any assignment, for learning what the script sets at its own level.
ASSIGN = re.compile(r"(?<![\w:])\$([A-Za-z_]\w*)\s*(?:[-+*/]?=(?!=)|\+\+|--)")
FUNCTION = re.compile(r"^\s*function\s+([\w-]+)", re.IGNORECASE)


def code_of(line):
    """The part of a line that is code: everything before an unquoted `#`."""
    quote = None
    for index, char in enumerate(line):
        if quote:
            if char == quote:
                quote = None
        elif char in "\"'":
            quote = char
        elif char == "#":
            return line[:index]
    return line


def findings(text):
    """Every compound assignment inside a function to a script-level name.

    Returns `(line number, function, name)` for each. Separate from `main` so
    the self-test can drive it on text.
    """
    script_level = set()
    inside = []
    depth = 0
    function, opened_at = None, None
    in_block_comment = False
    for number, line in enumerate(text.splitlines(), 1):
        # `<# ... #>` is PowerShell's block comment, and the scripts here use
        # it for their help text.
        if in_block_comment:
            if "#>" in line:
                in_block_comment = False
            continue
        if line.lstrip().startswith("<#") and "#>" not in line:
            in_block_comment = True
            continue
        code = code_of(line)
        started = FUNCTION.match(code)
        if started and function is None:
            function, opened_at = started.group(1), depth
        for match in ASSIGN.finditer(code):
            if function is None:
                script_level.add(match.group(1).lower())
        if function is not None:
            for match in COMPOUND.finditer(code):
                inside.append((number, function, match.group(1)))
        depth += code.count("{") - code.count("}")
        if function is not None and depth <= opened_at and "}" in code:
            function = None
    return [(number, function, name) for number, function, name in inside
            if name.lower() in script_level]


def scripts():
    """Every `.ps1` git tracks, so a stray local file is not read."""
    done = subprocess.run(["git", "ls-files", "*.ps1"], cwd=ROOT,
                          capture_output=True, text=True, check=True)
    return [line for line in done.stdout.splitlines() if line]


def main():
    found = []
    names = scripts()
    for name in names:
        with open(os.path.join(ROOT, name), "r", encoding="utf-8-sig") as handle:
            for number, function, variable in findings(handle.read()):
                found.append("  %s:%d: %s changes $%s, which is the script's, "
                             "without $script:, so the change stays in the "
                             "function" % (name, number, function, variable))
    if found:
        print("a PowerShell function writes to its own copy of a script "
              "variable:")
        print("\n".join(found))
        return 1
    print("  %d PowerShell script(s), and no function keeps a script "
          "variable's change to itself" % len(names))
    return 0


# Each case is a script, how many findings it should give, and why.
CASES = [
    ("""
$problems = @()
function Shoot([string]$name) {
  if ($true) {
    $problems += "$name : the window never appeared"
  }
}
""", 1, "F-241 itself: a list the script owns, appended to inside a function"),

    ("""
$problems = @()
function Shoot([string]$name) {
  $script:problems += "$name : the window never appeared"
}
""", 0, "the fix: the script's variable, named as the script's"),

    ("""
$taken = 0
function Shoot {
  $taken++
}
""", 1, "an increment is a compound assignment too"),

    ("""
$ErrorActionPreference = "Stop"
function Invoke-Gpg {
  $previous = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  $ErrorActionPreference = $previous
}
""", 0, "a plain assignment makes a deliberate local, as install.ps1 does"),

    ("""
function Count {
  $seen = 0
  $seen += 1
}
""", 0, "a function's own variable, which the script never sets"),

    ("""
$problems = @()
function Shoot {
  Write-Output "done"  # $problems += "a comment, not code"
}
$problems += "at the script's own level, which is fine"
""", 0, "a comment is not code, and the script's own level is not a function"),

    ("""
$problems = @()
<#
  $problems += "inside help text"
#>
function Shoot {
  Write-Output 'a # inside quotes is not a comment'; $problems += "x"
}
""", 1, "help text is skipped, and a # in quotes does not hide what follows"),
]


def self_test():
    """Drive `findings` over the cases above. Returns the exit status."""
    wrong = []
    for text, expected, why in CASES:
        got = len(findings(text))
        if got != expected:
            wrong.append("  expected %d, got %d  (%s)" % (expected, got, why))
    if wrong:
        print("the PowerShell scope check is wrong on %d case(s):" % len(wrong))
        print("\n".join(wrong))
        return 1
    print("  the PowerShell scope check is right on all %d cases" % len(CASES))
    return 0


if __name__ == "__main__":
    if "--self-test" in sys.argv[1:]:
        sys.exit(self_test())
    sys.exit(main())
