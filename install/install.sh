#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# VeilVoice installer for Linux and macOS.
#
#   sh install.sh                 # interactive
#   sh install.sh --yes           # no prompts, no optional components
#   sh install.sh --version v0.1.9
#   sh install.sh --prefix ~/.local
#   sh install.sh --help
#
# ---------------------------------------------------------------------------
# What this script will and will not do
# ---------------------------------------------------------------------------
#
# It downloads a release, proves it is the one this project published, and puts
# it on your PATH. It refuses -- loudly, naming the check that failed -- rather
# than continuing past anything it could not verify. There is no "--force" and
# no "skip verification" switch, because an installer with one is an installer
# whose verification is decorative.
#
# It installs *nothing else* unless you say so. Audacity and GPG are offered,
# once, as an explicit question that defaults to **no**. With `--yes` they are
# not installed at all: `--yes` means "do not ask me", and answering an
# unasked question by installing software on somebody's machine is precisely
# the behaviour that makes install scripts untrustworthy.
#
# ---------------------------------------------------------------------------
# The order of the checks, which is the whole point
# ---------------------------------------------------------------------------
#
#   1. Fetch the public key and check its **fingerprint** against the constant
#      below, which is hardcoded in this file. A key that does not match is
#      refused. This is the only anchor in the whole process: everything after
#      it is only as trustworthy as this comparison.
#   2. Verify the detached signature over SHA256SUMS with that key.
#   3. Verify the archive's SHA-256 against the now-trusted SHA256SUMS.
#   4. Only then unpack, and only then install.
#
# Doing it in this order matters. Checking the hash first and the signature
# afterwards proves only that the file matches a list that might itself have
# been replaced. The signature is what makes the list worth checking against.
#
# If GPG is not installed, steps 1 and 2 cannot be done, and the script says so
# and stops rather than quietly falling back to "the hash matched". A hash
# checked against an unverified list is not a security check.

set -eu

# ---------------------------------------------------------------------------
# The trust anchor. Hardcoded on purpose: if this is fetched, it is not an
# anchor. Compare it against the fingerprint published in README.md, on
# https://tilas01.github.io/veilvoice/ and in every release's notes.
# ---------------------------------------------------------------------------
FINGERPRINT="8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A"

REPO="tilas01/veilvoice"
KEY_URL="https://tilas01.github.io/veilvoice/assets/veilvoice-signing-key.asc"
KEY_URL_FALLBACK="https://raw.githubusercontent.com/$REPO/main/website/assets/veilvoice-signing-key.asc"

VERSION=""
PREFIX=""
ASSUME_YES=0
WANT_AUDACITY=0
WANT_GPG=0

# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_OK=$(printf '\033[32m'); C_BAD=$(printf '\033[31m')
    C_DIM=$(printf '\033[2m');  C_OFF=$(printf '\033[0m')
else
    C_OK=''; C_BAD=''; C_DIM=''; C_OFF=''
fi

say()  { printf '%s\n' "$*"; }
step() { printf '%s\n' "${C_DIM}==>${C_OFF} $*"; }
good() { printf '%s\n' "  ${C_OK}ok${C_OFF}   $*"; }

# Every refusal goes through here, so every refusal names the check.
refuse() {
    printf '\n%s\n' "${C_BAD}REFUSED${C_OFF}: $1" >&2
    shift
    for line in "$@"; do printf '%s\n' "  $line" >&2; done
    printf '\n%s\n' "Nothing has been installed." >&2
    exit 1
}

usage() {
    sed -n '3,11p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
}

# ---------------------------------------------------------------------------
# Arguments
# ---------------------------------------------------------------------------
while [ $# -gt 0 ]; do
    case "$1" in
        --yes|-y)        ASSUME_YES=1 ;;
        --version)       shift; VERSION="${1:-}" ;;
        --version=*)     VERSION="${1#*=}" ;;
        --prefix)        shift; PREFIX="${1:-}" ;;
        --prefix=*)      PREFIX="${1#*=}" ;;
        --with-audacity) WANT_AUDACITY=1 ;;
        --with-gpg)      WANT_GPG=1 ;;
        --help|-h)       usage ;;
        *) refuse "unknown option: $1" "Run '$0 --help' for the options." ;;
    esac
    shift
done

# ---------------------------------------------------------------------------
# Tools this script needs
# ---------------------------------------------------------------------------
have() { command -v "$1" >/dev/null 2>&1; }

# Defined here rather than beside the optional-extras section further down,
# because the GnuPG prompt above may call it. A function used before its
# definition is simply not defined yet -- the shell reads top to bottom.
install_optional() {
    package="$1"
    if   have apt-get; then sudo apt-get install -y "$package"
    elif have dnf;     then sudo dnf install -y "$package"
    elif have pacman;  then sudo pacman -S --noconfirm "$package"
    elif have zypper;  then sudo zypper install -y "$package"
    elif have apk;     then sudo apk add "$package"
    elif have brew;    then brew install "$package"
    elif have pkg;     then sudo pkg install -y "$package"
    else
        say "  Could not find a package manager to install '$package'."
        say "  Install it yourself if you want it."
        return 1
    fi
}

if have curl; then
    fetch() { curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1"; }
elif have wget; then
    fetch() { wget -q -O "$2" "$1"; }
else
    refuse "neither curl nor wget is installed" \
        "One of them is needed to download anything at all." \
        "Debian/Ubuntu: sudo apt install curl" \
        "Fedora:        sudo dnf install curl" \
        "macOS:         curl is already present; check your PATH"
fi

# SHA-256, from whichever tool this system has.
if have sha256sum; then
    sha256_of() { sha256sum "$1" | cut -d' ' -f1; }
elif have shasum; then
    sha256_of() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
    refuse "no SHA-256 tool found" \
        "Looked for 'sha256sum' and 'shasum'." \
        "Without one, the download cannot be checked, so it will not be installed."
fi

# ---------------------------------------------------------------------------
# Which build
# ---------------------------------------------------------------------------
detect_label() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Linux)
            case "$arch" in
                x86_64|amd64)  echo "linux-x86_64" ;;
                aarch64|arm64) echo "linux-arm64" ;;
                armv7l|armv7)  echo "linux-armv7-pi" ;;
                *) echo "" ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                arm64)        echo "macos-arm64" ;;
                x86_64)       echo "macos-x86_64" ;;
                *) echo "" ;;
            esac
            ;;
        FreeBSD)
            case "$arch" in
                amd64|x86_64) echo "freebsd-x86_64" ;;
                *) echo "" ;;
            esac
            ;;
        *) echo "" ;;
    esac
}

LABEL=$(detect_label)
[ -n "$LABEL" ] || refuse "no published build for $(uname -s) $(uname -m)" \
    "The releases page lists what is published:" \
    "  https://github.com/$REPO/releases" \
    "Building from source works on any platform Rust supports:" \
    "  cargo install --path crates/veilvoice-cli"

# The archive suffix. Everything except Windows is a tarball.
EXT="tar.gz"

# ---------------------------------------------------------------------------
# Which version
# ---------------------------------------------------------------------------
if [ -z "$VERSION" ]; then
    step "Asking GitHub for the latest release"
    TMP_TAG=$(mktemp)
    if ! fetch "https://api.github.com/repos/$REPO/releases/latest" "$TMP_TAG"; then
        rm -f "$TMP_TAG"
        refuse "could not reach the GitHub API to find the latest release" \
            "Pass one explicitly instead:  $0 --version v0.1.9"
    fi
    VERSION=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$TMP_TAG" | head -1)
    rm -f "$TMP_TAG"
    [ -n "$VERSION" ] || refuse "could not read a tag name from the GitHub API reply" \
        "Pass one explicitly instead:  $0 --version v0.1.9"
fi

ARCHIVE="veilvoice-$VERSION-$LABEL.$EXT"
BASE="https://github.com/$REPO/releases/download/$VERSION"

say ""
say "  VeilVoice installer"
say "  version    $VERSION"
say "  build      $LABEL"
say "  key        $FINGERPRINT"
say ""

WORK=$(mktemp -d)
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------
# 1. Download
# ---------------------------------------------------------------------------
step "Downloading"
fetch "$BASE/$ARCHIVE" "$WORK/$ARCHIVE" \
    || refuse "could not download $ARCHIVE" \
        "Checked: $BASE/$ARCHIVE" \
        "If that version or platform is not published, the releases page lists what is:" \
        "  https://github.com/$REPO/releases"
good "$ARCHIVE"

fetch "$BASE/SHA256SUMS" "$WORK/SHA256SUMS" \
    || refuse "could not download SHA256SUMS" \
        "Without the hash list the download cannot be checked, so it will not be installed."
good "SHA256SUMS"

SIGNED=1
fetch "$BASE/SHA256SUMS.asc" "$WORK/SHA256SUMS.asc" 2>/dev/null || SIGNED=0
if [ "$SIGNED" = "1" ]; then
    good "SHA256SUMS.asc"
else
    refuse "this release has no signature (SHA256SUMS.asc)" \
        "Every signed release publishes one. Its absence means either that this" \
        "release was built without the signing key, or that you are not looking" \
        "at the release you think you are." \
        "" \
        "This script will not install an unsigned build."
fi

# ---------------------------------------------------------------------------
# 2. The key, and its fingerprint
# ---------------------------------------------------------------------------
if ! have gpg && ! have gpg2; then
    say ""
    say "  GnuPG is not installed, and it is what verifies the signature."
    say "  Without it this script can check that the download matches a hash"
    say "  list, but not that the hash list is the one VeilVoice published --"
    say "  which is not a security check at all."
    say ""
    if [ "$ASSUME_YES" = "1" ] || [ "$WANT_GPG" = "1" ]; then
        [ "$WANT_GPG" = "1" ] || refuse "GnuPG is not installed" \
            "Install it and run this again, or pass --with-gpg to have this" \
            "script install it for you:" \
            "  Debian/Ubuntu: sudo apt install gnupg" \
            "  Fedora:        sudo dnf install gnupg2" \
            "  macOS:         brew install gnupg"
    else
        printf '  Install GnuPG now? [y/N] '
        read -r answer </dev/tty || answer=""
        case "$answer" in [yY]*) WANT_GPG=1 ;; esac
    fi

    if [ "$WANT_GPG" = "1" ]; then
        install_optional gnupg || true
    fi

    have gpg || have gpg2 || refuse "GnuPG is still not available" \
        "The signature cannot be verified without it, and this script does not" \
        "install software it could not verify."
fi

GPG=$(command -v gpg 2>/dev/null || command -v gpg2)

step "Checking the signing key's fingerprint"
if ! fetch "$KEY_URL" "$WORK/key.asc"; then
    fetch "$KEY_URL_FALLBACK" "$WORK/key.asc" \
        || refuse "could not download the public key" \
            "Tried: $KEY_URL" \
            "  and: $KEY_URL_FALLBACK"
fi

# A throwaway keyring: importing into the user's own is a side effect nobody
# asked for, and it would also mean a previously-imported key could satisfy
# this check instead of the one just downloaded.
export GNUPGHOME="$WORK/gnupg"
mkdir -p "$GNUPGHOME"
chmod 700 "$GNUPGHOME"

"$GPG" --batch --quiet --import "$WORK/key.asc" 2>/dev/null \
    || refuse "the downloaded public key could not be imported" \
        "The file at $KEY_URL is not a valid OpenPGP public key."

GOT=$("$GPG" --batch --with-colons --fingerprint 2>/dev/null \
      | awk -F: '$1 == "fpr" { print $10; exit }')

if [ "$GOT" != "$FINGERPRINT" ]; then
    refuse "the signing key's fingerprint does not match" \
        "expected  $FINGERPRINT" \
        "found     ${GOT:-(none)}" \
        "" \
        "This is the check that anchors every other one, so nothing further" \
        "was attempted. Either the key you fetched is not VeilVoice's, or the" \
        "fingerprint in this script has been altered. Compare it against the" \
        "one published in README.md and on the website before going further."
fi
good "fingerprint matches $FINGERPRINT"

# ---------------------------------------------------------------------------
# 3. The signature over the hash list
# ---------------------------------------------------------------------------
step "Verifying the signature over SHA256SUMS"
"$GPG" --batch --quiet --verify "$WORK/SHA256SUMS.asc" "$WORK/SHA256SUMS" 2>/dev/null \
    || refuse "the signature over SHA256SUMS is not valid" \
        "The hash list is not the one signed by $FINGERPRINT." \
        "Do not use this download."
good "signature is good"

# ---------------------------------------------------------------------------
# 4. The archive's hash, against the now-trusted list
# ---------------------------------------------------------------------------
step "Verifying the archive against SHA256SUMS"
WANT=$(awk -v want="$ARCHIVE" '$2 == want || $2 == "*" want { print $1; exit }' "$WORK/SHA256SUMS")
[ -n "$WANT" ] || refuse "$ARCHIVE is not listed in SHA256SUMS" \
    "The signature was good, so the list is genuine -- it simply does not" \
    "mention this file. That means this archive was not part of this release."

GOT=$(sha256_of "$WORK/$ARCHIVE")
if [ "$WANT" != "$GOT" ]; then
    refuse "the archive's SHA-256 does not match the signed hash list" \
        "expected  $WANT" \
        "found     $GOT" \
        "" \
        "The download does not match what was signed. It is corrupt, truncated," \
        "or not the file that was published."
fi
good "sha256 matches ($GOT)"

# ---------------------------------------------------------------------------
# 5. Unpack and install
# ---------------------------------------------------------------------------
step "Unpacking"
tar -xzf "$WORK/$ARCHIVE" -C "$WORK" \
    || refuse "the archive could not be unpacked" "It verified, but tar could not read it."

SRC="$WORK/veilvoice-$VERSION-$LABEL"
[ -d "$SRC" ] || SRC=$(find "$WORK" -maxdepth 1 -type d -name 'veilvoice-*' | head -1)
[ -d "$SRC" ] || refuse "the archive did not contain the directory expected"

if [ -z "$PREFIX" ]; then
    # ~/.local/bin needs no root and is on the default PATH of every current
    # distribution. Installing to /usr/local by default would mean asking for
    # a password, which an installer should do only when it must.
    PREFIX="$HOME/.local"
fi
BIN="$PREFIX/bin"
mkdir -p "$BIN"

INSTALLED=""
for name in veilvoice veilvoice-gui; do
    if [ -f "$SRC/$name" ]; then
        cp "$SRC/$name" "$BIN/$name"
        chmod 755 "$BIN/$name"
        INSTALLED="$INSTALLED $name"
    fi
done
[ -n "$INSTALLED" ] || refuse "no VeilVoice binary was found inside the archive"

good "installed$INSTALLED to $BIN"

# ---------------------------------------------------------------------------
# 6. Optional extras -- asked once, defaulting to no
# ---------------------------------------------------------------------------
#
# Deliberately after the install, and deliberately not part of it. These are
# other people's software; VeilVoice recommends them and does not bundle them.
# Audacity in particular is GPL-2.0-or-later, which is incompatible with this
# project's GPL-3.0-or-later for combining code -- recommending it is fine,
# shipping it inside anything is not.

if [ "$ASSUME_YES" = "0" ] && [ -t 0 ]; then
    say ""
    say "  Optional, and nothing depends on them:"
    say ""
    say "    Audacity  -- a free audio editor. Useful for recording and for"
    say "                 trimming a file before veiling it. Not bundled: it is"
    say "                 GPL-2.0-or-later, which cannot be combined with this"
    say "                 project's GPL-3.0-or-later."
    say ""
    printf '  Install Audacity? [y/N] '
    read -r answer </dev/tty || answer=""
    case "$answer" in [yY]*) WANT_AUDACITY=1 ;; esac
fi

if [ "$WANT_AUDACITY" = "1" ]; then
    step "Installing Audacity"
    install_optional audacity && good "Audacity installed" || \
        say "  Audacity was not installed. VeilVoice does not need it."
fi

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
say ""
say "${C_OK}Installed.${C_OFF}"
say ""
say "  Every check passed: the key's fingerprint, the signature over the hash"
say "  list, and the archive's hash against that list."
say ""

case ":$PATH:" in
    *":$BIN:"*) ;;
    *)
        say "  $BIN is not on your PATH. Add it:"
        say ""
        say "    echo 'export PATH=\"\$PATH:$BIN\"' >> ~/.profile"
        say ""
        ;;
esac

say "  Start with:  veilvoice --help"
say "               veilvoice info        # what this build supports"
say ""
say "  What it does and does not do:"
say "    https://github.com/$REPO/blob/main/docs/WHITEPAPER.md"
say ""
