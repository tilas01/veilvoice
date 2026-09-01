<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Screenshots

Pictures of VeilVoice running, for the README, the website, the wiki and the
reference pages.

## The two kinds here, and why they are different

**`gui-*.png` are photographs of the running application.** They are the one
class of committed binary in this repository that cannot be reproduced from
source, because they are pictures of a program drawing itself on somebody's
screen. `tools/shots/gui.ps1` takes them: it starts the release build, fixes
the window to one size and position, clicks each tab, and captures the window's
real frame bounds. Re-running it takes a minute, so a change to the interface
can be followed by a change to its pictures in the same commit.

That script fails rather than writing a wrong picture, and it does not remember
where the tabs are. It **finds** them: it scans the strip of pixels the labels
sit in and groups the lit columns into runs, one run per label, and stops if the
count is not the one it expects.

It did remember them, once, and they went stale the first time a tab was
inserted. Every click still landed on *a* tab, so every capture was different,
the duplicate check saw nothing wrong, and three tabs were quietly photographed
under the wrong names. Two guards remain from that: an identical pair of
consecutive captures stops the run, and after each click the pixel above the
label has to be the raised background a selected tab is drawn on.

**`cli-*.svg` are drawings, and they are generated.** Each one is a pure
function of the `cli-*.txt` beside it, which holds exactly what the command
printed. `python tools/shots/terminal.py --check` regenerates every drawing into
memory and compares, and CI fails on a difference, the same arrangement the
banners and the reference diagrams have. A picture of a command line that
disagrees with the command line is documentation that lies, and this makes it
impossible to commit by accident.

The `.txt` is the file to read in a diff. An SVG diff is unreadable; a diff of
what the program printed is the review.

## Redoing them

```
cargo build --release -p veilvoice-gui -p veilvoice-cli
powershell -ExecutionPolicy Bypass -File tools/shots/gui.ps1
python tools/shots/terminal.py --capture
python tools/shots/terminal.py
```

The first two steps need a machine and a person looking at the result. The
last one is the reproducible half, and it is the half CI checks.

## Two of the pictures are redacted, and here is exactly which

A screenshot of a working application is a screenshot of somebody's machine.
Two tabs put that on the page, so `tools/shots/gui.ps1` paints over those
regions before writing the file, in the colours the interface draws them
in, so the replacement reads as part of the application rather than as a
black bar.

| File | What was covered | What it says instead |
|---|---|---|
| `gui-live.png` | the two audio device dropdowns | `your microphone`, `your virtual cable` |
| `gui-install.png` | the running-from and install-to paths | the same paths, under a user called `you` |
| `gui-lock.png` | where the app lock file lives | the same path, under a user called `you` |

The device names are product names: a headset model and a particular
virtual-cable setup, which together describe the maintainer's hardware. The
paths contain the **account name**, and this project is published under a
pseudonym on purpose, and an account name is not that pseudonym.

Nothing else in this directory is altered, and no other picture is. **A tab
that starts showing a path or a device name needs adding to the tables in
that script.** Nothing can check it for you: the text is inside a PNG.

## What is deliberately not captured at all

`veilvoice devices` makes a good demonstration and a poor thing to publish,
so it is not in the command list. The monitor tab is photographed in whatever
state the machine is in, which is why it is photographed on a machine with
nothing running.
