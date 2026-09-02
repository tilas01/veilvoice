#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Photograph every tab of the desktop application, on Linux, with no display.
#
#     tools/shots/gui.sh
#
# The counterpart to `gui.ps1`, which does the same job on Windows. Both exist
# because the captures have to be reproducible by whoever is holding the
# repository, and until this script there was exactly one machine in the world
# that could take them.
#
# What this does, and what it deliberately does not do
# ----------------------------------------------------
#
# It does not click, for the same reason `gui.ps1` does not: three earlier
# versions of that script drove the interface by clicking and each failed by
# photographing the wrong tab under the right name. `veilvoice-gui --tab <name>`
# opens the window on a tab, and the application is started once per tab. There
# are no coordinates, no scanning and no synthetic input.
#
# There is no window manager. That is a choice rather than a limitation:
# without one the window is mapped at the origin at exactly the size it asks
# for, so the X root window *is* the application window, pixel for pixel. The
# capture needs no cropping, cannot include a strip of desktop down one edge,
# and comes out the same size for all nine tabs by construction rather than by
# arithmetic afterwards. The Windows script has to ask the desktop compositor
# for the window's real bounds to get the same result, because there a window
# manager owns the frame.
#
# The screen is the window size for the same reason.
#
# Nothing here touches the reader's own configuration. `gui.ps1` has to save
# and restore `%APPDATA%\veilvoice\settings.conf`, because that is where the
# application looks on Windows. On Linux it looks under `XDG_CONFIG_HOME`,
# which this points at a temporary directory, so the settings the captures need
# exist only for as long as the captures take and the reader's own are never
# opened, let alone written.
#
# Requirements: Xvfb and xwd, and a release build of veilvoice-gui.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
exe="${VEILVOICE_GUI:-$here/target/release/veilvoice-gui}"
out="${1:-$here/assets/screenshots}"

# The size the captures are taken at.
#
# 1100x720 is what the window opens at, and it is too small for this: the tab
# strip does not fit and the longest tab is cut off partway down, so the
# picture shows a panel with its bottom missing. 1400 across is the same width
# `gui.ps1` uses, so the two scripts produce comparable images.
#
# The height is 1600 and that is not the height of the finished pictures.
#
# It used to be 1000, which is the height the window is meant to be, and two
# tabs are taller than that: `group` needs 1315 and `install` needs 1095. Those
# two came out with their bottoms cut off, and the committed pictures were the
# right size only because whoever ran this last happened to know to pass a
# bigger height. A tool that produces correct output only for somebody who
# already knows the answer is not a tool.
#
# So it captures tall enough for the longest tab and `fit.py` trims each one
# back to its own content afterwards, with a floor of 1000 so the short ones
# stay the size the window actually is. Every tab gets a picture that fits it,
# by measurement rather than by memory, and nothing here needs a per-tab table
# that would go stale the next time a panel grows a paragraph.
width="${SHOT_WIDTH:-1400}"
height="${SHOT_HEIGHT:-1600}"

for tool in Xvfb xwd; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "missing $tool -- install xvfb and x11-apps" >&2
    exit 1
  }
done
[ -x "$exe" ] || {
  echo "no build at $exe -- run: cargo build --release -p veilvoice-gui" >&2
  exit 1
}
mkdir -p "$out"

work="$(mktemp -d)"
display=":$(( 90 + RANDOM % 9 ))"
trap 'kill "${xvfb:-}" 2>/dev/null || true; rm -rf "$work"' EXIT

# `-nocursor` is the whole of the no-mouse-pointer guarantee on this side.
# The Windows script gets it from `PrintWindow`, which asks the window to
# draw itself and so cannot include anything the compositor draws on top of
# it. Here the capture is of the root window, which would include a pointer
# if one were drawn, so the server is told to draw none at all. Both are
# properties of how the capture is taken rather than a hope about where the
# mouse was, and `images.test.js` checks each script for its own.
Xvfb "$display" -screen 0 "${width}x${height}x24" -nocursor -nolisten tcp >/dev/null 2>&1 &
xvfb=$!
for _ in $(seq 40); do
  DISPLAY="$display" xwd -root -silent >/dev/null 2>&1 && break
  sleep 0.25
done

# The configuration these pictures are taken under. It exists only for this
# run, in a directory made a moment ago, so nothing here reads or writes the
# settings of whoever is running it -- and, just as importantly, no policy
# they have set applies: policies live beside this file, under the same
# `XDG_CONFIG_HOME`, and this one is empty. The captures are therefore of the
# application as it is, not as one machine has been told to restrict it.
#
#   configured      skips the first-run choice, which is a dialog over the
#                   window rather than a tab, and would be photographed
#                   instead of the tab that was asked for.
#   always_group    group mode is off by default, which is correct and makes
#                   for a picture of an empty panel.
#   animations      the mark in the header animates, so it is a different
#                   shape in every photograph and any two captures of the same
#                   tab differ. Stilled here, which is a setting the
#                   application already has for people who want it.
#   animated_icon   the same, for the window icon.
mkdir -p "$work/config/veilvoice"
cat > "$work/config/veilvoice/settings.conf" <<'CONF'
configured = true
always_group = true
animations = false
animated_icon = false
CONF

# The tab names the application answers to: `Tab::key` in
# crates/veilvoice-gui/src/app.rs, where a test keeps them unique and stable,
# because each one is also a file name the README links.
tabs=(file live group monitor lock verify settings install about)

problems=()
declare -A prints=()

for tab in "${tabs[@]}"; do
  DISPLAY="$display" XDG_CONFIG_HOME="$work/config" LIBGL_ALWAYS_SOFTWARE=1 \
    "$exe" --tab "$tab" --size "${width}x${height}" >"$work/$tab.log" 2>&1 &
  gui=$!

  # Long enough for the window to appear, the layout to settle and the first
  # frames to be drawn. Software rendering, so this is not quick.
  sleep 14

  if ! kill -0 "$gui" 2>/dev/null; then
    problems+=("$tab : the application exited before it was photographed")
    continue
  fi

  if ! DISPLAY="$display" xwd -root -silent > "$work/$tab.xwd" 2>/dev/null; then
    problems+=("$tab : xwd could not read the screen")
    kill "$gui" 2>/dev/null || true
    wait "$gui" 2>/dev/null || true
    continue
  fi

  kill "$gui" 2>/dev/null || true
  wait "$gui" 2>/dev/null || true

  if ! python3 "$here/tools/shots/xwd.py" "$work/$tab.xwd" "$out/gui-$tab.png"; then
    problems+=("$tab : the capture could not be converted")
    continue
  fi

  # A cheap fingerprint, so two tabs coming out identical is caught rather than
  # published. That is the failure the clicking versions of the Windows script
  # kept producing, and it is invisible in a directory listing.
  print="$(python3 "$here/tools/shots/xwd.py" --fingerprint "$out/gui-$tab.png")"
  if [ -n "${prints[$print]:-}" ]; then
    problems+=("$tab : identical to ${prints[$print]} -- the tab did not change")
  else
    prints[$print]="$tab"
  fi

  echo "wrote gui-$tab.png  (${width}x${height})"
done

if [ "${#problems[@]}" -gt 0 ]; then
  echo
  for p in "${problems[@]}"; do echo "  PROBLEM $p"; done
  exit 1
fi
echo
echo "${#tabs[@]} captures in $out"
