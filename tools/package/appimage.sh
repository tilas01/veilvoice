#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build the AppImage from two programs that were already compiled.
#
#   tools/package/appimage.sh BIN_DIR OUT_DIR VERSION APPIMAGETOOL RUNTIME
#   tools/package/appimage.sh BIN_DIR OUT_DIR VERSION --appdir-only
#
# BIN_DIR holds veilvoice and veilvoice-gui. Build them on the oldest Linux
# the AppImage should run on: a program built against a C library runs on
# that version and every later one, never an earlier one. The release builds
# them on Ubuntu 22.04, so the AppImage runs on any distribution whose C
# library is 2.35 or newer.
#
# APPIMAGETOOL and RUNTIME are the AppImage project's own tools, downloaded
# and checked against a pinned hash by the workflow. With --appdir-only this
# stops after laying out the folder, which is what the self-test and anybody
# curious about the layout want.
#
# In plain words: puts both programs, their icon and three small libraries
# into one file that runs on most Linux systems without installing anything.

set -eu

[ $# -ge 4 ] || { echo "usage: $0 BIN_DIR OUT_DIR VERSION APPIMAGETOOL RUNTIME | --appdir-only" >&2; exit 2; }
bin="$1"; out="$2"; version="$3"; tool="$4"; runtime="${5:-}"
root="$(cd "$(dirname "$0")/../.." && pwd)"

for name in veilvoice veilvoice-gui; do
    [ -x "$bin/$name" ] || { echo "$bin/$name is missing or not executable" >&2; exit 1; }
done

mkdir -p "$out"
appdir="$out/VeilVoice.AppDir"
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/lib" "$appdir/usr/share/applications" \
         "$appdir/usr/share/doc/veilvoice"

install -m 0755 "$root/packaging/appimage/AppRun" "$appdir/AppRun"
install -m 0755 "$bin/veilvoice" "$bin/veilvoice-gui" "$appdir/usr/bin/"
install -m 0644 "$root/LICENSE" "$root/README.md" "$appdir/usr/share/doc/veilvoice/"
mkdir -p "$appdir/usr/share/icons"
cp -r "$root/assets/linux/hicolor" "$appdir/usr/share/icons/hicolor"

# The desktop entry and the icon must also sit at the top of the folder: that
# is where the AppImage tools and desktop integration look for them.
install -m 0644 "$root/packaging/veilvoice.desktop" "$appdir/usr/share/applications/veilvoice.desktop"
install -m 0644 "$root/packaging/veilvoice.desktop" "$appdir/veilvoice.desktop"
install -m 0644 "$root/assets/linux/hicolor/256x256/apps/veilvoice.png" "$appdir/veilvoice.png"
ln -s veilvoice.png "$appdir/.DirIcon"

# The window opens libxkbcommon-x11 by name at startup rather than linking
# it, so nothing that reads the program's links knows it is needed, and a
# minimal install of several distributions does not have it. It and the two
# libraries it needs travel with the AppImage; AppRun uses them only when the
# computer has none. Everything else the window needs (the C library, ALSA,
# OpenGL, X11, Wayland) is part of every Linux desktop and must come from the
# computer itself, because it is tied to that computer's drivers.
for lib in libxkbcommon-x11.so.0 libxkbcommon.so.0 libxcb-xkb.so.1; do
    path="$(/sbin/ldconfig -p | awk -v l="$lib" '$1 == l && /x86-64/ {print $NF; exit}')"
    if [ -z "$path" ]; then
        if [ "$tool" = "--appdir-only" ]; then
            echo "note: $lib is not on this machine, so it is not carried" >&2
            continue
        fi
        echo "$lib is not installed on the build machine" >&2
        exit 1
    fi
    cp -L "$path" "$appdir/usr/lib/$lib"
done

if [ "$tool" = "--appdir-only" ]; then
    echo "laid out $appdir"
    exit 0
fi

[ -n "$runtime" ] || { echo "the runtime is needed unless --appdir-only" >&2; exit 2; }
target="$out/veilvoice-v$version-linux-x86_64.AppImage"
# No update information is embedded: VeilVoice's own updater, or the next
# release page, is how a newer version arrives.
ARCH=x86_64 VERSION="$version" "$tool" --no-appstream --runtime-file "$runtime" \
    "$appdir" "$target"
rm -rf "$appdir"
echo "wrote $target"
