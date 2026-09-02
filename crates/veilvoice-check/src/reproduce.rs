// SPDX-License-Identifier: GPL-3.0-or-later
//! A script that rebuilds a release and compares it with what was published.
//!
//! # What this is for, and why it is stronger than checking a hash
//!
//! Checking a download against `SHA256SUMS` proves the file is the one whose
//! hash was signed. It says nothing about what is *in* it. The signature and
//! the hash are both made by whoever published the release, and if that
//! machine was compromised, or the person was, both are made over a binary
//! nobody wants.
//!
//! Rebuilding closes that. Compile the source at the tag, hash what came out,
//! and compare it with the published hash for this platform. If they match,
//! the published binary is what that source compiles to, and the question
//! moves from "do I trust the publisher" to "do I trust the source", which is
//! a question anybody can act on because the source is here to read.
//!
//! This is the check the verify tab describes as the one worth more than any
//! of the others and says it cannot perform for you. This writes the script
//! that performs it.
//!
//! # Why the toolchain version is not typed in here
//!
//! Reproducibility depends on the exact compiler, and this project pins it in
//! `rust-toolchain.toml`. A script carrying its own copy of that version would
//! be wrong the first time the pin moved, and wrong in the worst possible way:
//! it would report a mismatch on a genuine release and tell somebody their
//! download had been tampered with.
//!
//! So the script does not name a version at all. It relies on `rustup`
//! reading the file in the checkout, which is what `rustup` does with no
//! encouragement, and it says so where the reader can see it.
//!
//! # What it will not do
//!
//! It does not install a compiler, and it does not fetch the release. It says
//! what to run. A script that installed a toolchain would be asking somebody
//! to let an unfamiliar program put a compiler on their machine as part of
//! deciding whether to trust that program.

/// The system a script is being written for.
///
/// The split is by what the shell and the tools are called, not by processor:
/// an Intel Mac and an Apple Silicon Mac run the same commands, and the
/// difference between them shows up in the *name of the archive*, which the
/// script works out at run time rather than being told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    /// Linux, and WSL, and anything else with GNU coreutils.
    Linux,
    /// macOS, on Intel or Apple Silicon.
    MacOs,
    /// FreeBSD, OpenBSD, NetBSD.
    Bsd,
    /// Windows, as a `.cmd` file for `cmd.exe`.
    Windows,
}

impl System {
    /// The name to save the script under.
    pub fn file_name(self) -> &'static str {
        match self {
            System::Linux => "reproduce-veilvoice.sh",
            System::MacOs => "reproduce-veilvoice-macos.sh",
            System::Bsd => "reproduce-veilvoice-bsd.sh",
            System::Windows => "reproduce-veilvoice.cmd",
        }
    }

    /// The name this is called on the command line.
    pub fn key(self) -> &'static str {
        match self {
            System::Linux => "linux",
            System::MacOs => "macos",
            System::Bsd => "bsd",
            System::Windows => "windows",
        }
    }

    /// The system with this name, if it is one.
    pub fn from_key(key: &str) -> Option<System> {
        match key.trim().to_ascii_lowercase().as_str() {
            "linux" | "wsl" => Some(System::Linux),
            "macos" | "mac" | "darwin" => Some(System::MacOs),
            "bsd" | "freebsd" | "openbsd" | "netbsd" => Some(System::Bsd),
            "windows" | "win" | "cmd" => Some(System::Windows),
            _ => None,
        }
    }

    /// Every system, in the order a front end should offer them.
    pub const ALL: &'static [System] =
        &[System::Linux, System::MacOs, System::Bsd, System::Windows];

    /// The one this program is running on, when it is one of these.
    pub fn here() -> System {
        if cfg!(windows) {
            System::Windows
        } else if cfg!(target_os = "macos") {
            System::MacOs
        } else if cfg!(any(
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd"
        )) {
            System::Bsd
        } else {
            System::Linux
        }
    }

    /// How this system checks a folder against a `SHA256SUMS`.
    fn hash_check_command(self) -> &'static str {
        match self {
            System::Linux => "sha256sum -c SHA256SUMS --ignore-missing",
            System::MacOs => "shasum -a 256 -c SHA256SUMS --ignore-missing",
            // The BSDs' `sha256` has `-c`, and it does not take
            // `--ignore-missing`; `-q` keeps the output to the verdict.
            System::Bsd => "sha256 -c SHA256SUMS",
            System::Windows => "certutil -hashfile",
        }
    }

    /// How this system spells "hash this file".
    fn hash_tool(self) -> &'static str {
        match self {
            // Coreutils. Also what WSL has.
            System::Linux => "sha256sum",
            // macOS ships `shasum`, and `sha256sum` is not there.
            System::MacOs => "shasum -a 256",
            // FreeBSD, OpenBSD and NetBSD all ship `sha256`, whose `-q` prints
            // the digest alone.
            System::Bsd => "sha256 -q",
            System::Windows => "certutil -hashfile",
        }
    }
}

/// The project's signing fingerprint, from the one place it is written.
fn veilvoice_check_fingerprint() -> &'static str {
    crate::FINGERPRINT
}

/// The script for this system.
pub fn script(system: System) -> String {
    match system {
        System::Windows => windows(),
        other => posix(other),
    }
}

/// The POSIX script, which covers Linux, macOS and the BSDs.
///
/// One script rather than three, because they differ in exactly one place:
/// the name of the program that computes a SHA-256. Writing three would mean
/// three places for the *logic* to drift, which is the part that matters.
fn posix(system: System) -> String {
    let hash = system.hash_tool();
    // `sha256sum` and `shasum` print "digest  name"; BSD `sha256 -q` prints
    // the digest alone.
    let take_digest = match system {
        System::Bsd => "",
        _ => " | cut -d' ' -f1",
    };
    format!(
        r##"#!/bin/sh
# Rebuild a VeilVoice release and compare it with the published binary.
#
#     sh {file} <version>          e.g.  sh {file} v0.1.15
#
# What this proves, and what checking a hash does not
#
# Checking a download against SHA256SUMS proves the file is the one whose hash
# was signed. It says nothing about what is inside it: the same person signed
# both. This rebuilds the release from source and compares the result, so a
# match means the published binary is what this source compiles to.
#
# That moves the question from "do I trust the publisher" to "do I trust the
# source", and the source is here to read.
#
# What this needs, and what it will not do for you
#
#   git, and a rustup-managed Rust toolchain.
#
# It will not install either. Being asked to let an unfamiliar program put a
# compiler on your machine, as part of deciding whether to trust that program,
# is not a reasonable thing to be asked.
#
# The compiler version is deliberately not written down here. It is pinned in
# rust-toolchain.toml in the checkout, and rustup reads that by itself. A
# version typed into this script would be wrong the first time the pin moved,
# and would report a genuine release as tampered with.

set -eu

VERSION=${{1:-}}
[ -n "$VERSION" ] || {{ echo "usage: sh $0 <version>   e.g. $0 v0.1.15" >&2; exit 2; }}

say() {{ printf '%s\n' "$*"; }}
die() {{ say "FAILED: $*"; exit 1; }}

for tool in git cargo; do
    command -v "$tool" >/dev/null 2>&1 || die "$tool is not installed."
done
command -v rustup >/dev/null 2>&1 \
    || say "note: rustup was not found, so the pinned compiler is not guaranteed.
    A different compiler produces a different binary, and a mismatch below
    would then say nothing about the release."

# Where the release files are, if any are here. The build happens elsewhere
# and this is where the comparison comes back to.
here=$(pwd)

# The project's signing key, from the same constant the programs compile in.
FINGERPRINT={fingerprint}

# The published side of the comparison, checked before anything is built.
#
# Before, deliberately. This used to run after the build, so an unsigned
# hash list was reported ten minutes after the only thing it needed was a
# signature check taking under a second. The cheap check that can fail
# goes first.
#
# The script is useful without it -- it prints what this machine built, which
# somebody can compare by eye -- but doing the comparison is the point, so it
# is done whenever the files are present. All three come from the release:
#
#     SHA256SUMS, SHA256SUMS.asc, and the archive for this platform.
#
# The signature first, and refused if it is not by the project's key. A hash
# list nobody signed is a hash list an attacker can write, and comparing a
# rebuild against it would prove that the rebuild matches the attacker.
published=""
if [ -f SHA256SUMS ] && [ -f SHA256SUMS.asc ] && command -v gpg >/dev/null 2>&1; then
    if [ -f veilvoice-signing-key.asc ]; then
        gpg --quiet --import veilvoice-signing-key.asc 2>/dev/null || true
    fi
    if status=$(gpg --status-fd 1 --verify SHA256SUMS.asc SHA256SUMS 2>&1) \
       && printf '%s\n' "$status" | awk -v want="$FINGERPRINT" '
            /^\[GNUPG:\] VALIDSIG / {{ if ($3 == want || $NF == want) hit = 1 }}
            END                     {{ exit !hit }}'
    then
        published=SHA256SUMS
        say "hash list     signed by $FINGERPRINT"
    else
        die "SHA256SUMS is here and is not signed by $FINGERPRINT.
    Nothing was compared against it: an unsigned hash list is one anybody
    could have written, and a rebuild matching it would prove nothing."
    fi
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
say "building in   $work"

git clone --quiet --depth 1 --branch "$VERSION" \
    https://github.com/tilas01/veilvoice "$work/src" \
    || die "no such version: $VERSION"
cd "$work/src"

# The commit date, which is what the release build used. Without this the
# binary carries the time of this build and can never match.
SOURCE_DATE_EPOCH=$(git log -1 --pretty=%ct)
export SOURCE_DATE_EPOCH

# The same remapping the release build applies, so paths inside the binary do
# not depend on where it was built. Hardcoding one person's directory would
# make the build reproducible only for them.
RUSTFLAGS="--remap-path-prefix=$work/src=/veilvoice --remap-path-prefix=$HOME=/home"
export RUSTFLAGS

say "compiling     this takes a while, and it is meant to"
cargo build --release --locked --quiet || die "the build did not succeed."

say ""
say "binary                          rebuilt here"
mismatch=0
built=0
for binary in veilvoice veilvoice-gui veilvoice-verify; do
    [ -f "$work/src/target/release/$binary" ] || continue
    built=$((built + 1))
    mine=$({hash} "$work/src/target/release/$binary"{take})
    say "$(printf '%-30s' "$binary") $mine"

    # Against the copy that shipped, when the archive has been unpacked here.
    theirs=""
    for candidate in "./$binary" "./bin/$binary" veilvoice-*/"$binary"; do
        [ -f "$candidate" ] || continue
        theirs=$({hash} "$candidate"{take})
        break
    done
    [ -n "$theirs" ] || continue
    if [ "$mine" = "$theirs" ]; then
        say "$(printf '%-30s' "") matches the published binary"
    else
        say "$(printf '%-30s' "") DIFFERS from the published binary"
        say "$(printf '%-30s' "") published: $theirs"
        mismatch=1
    fi
done

[ "$built" -gt 0 ] || die "the build produced no binaries, which should not happen."

say ""
if [ -n "$published" ]; then
    # The archive itself, against the signed list. Separate from the rebuild:
    # this says the download is what was published, and the rebuild says what
    # was published is what the source compiles to. Both are worth knowing and
    # they are not the same claim.
    if {hash_check} >/dev/null 2>&1; then
        say "archive       matches the signed hash list"
    else
        say "archive       does NOT match the signed hash list"
        mismatch=1
    fi
fi

if [ "$mismatch" -eq 0 ]; then
    say ""
    say "Everything that could be compared here agreed."
else
    say ""
    say "Something did not match. That is worth reporting, and it is not by"
    say "itself proof of anything: a different compiler version or a different"
    say "set of build flags produces a different binary from identical source."
fi

say ""
say "What this proves, and what it does not: a match means the published"
say "binary is what this source compiles to. It does not make the source"
say "correct, and it does not make it safe. It moves the question from"
say "trusting the publisher to reading the code, which is a question you can"
say "act on."
exit $mismatch
"##,
        file = system.file_name(),
        hash = hash,
        take = take_digest,
        hash_check = system.hash_check_command(),
        fingerprint = veilvoice_check_fingerprint(),
    )
}

/// The Windows script, for `cmd.exe`.
///
/// A `.cmd` rather than PowerShell on purpose: PowerShell's execution policy
/// refuses unsigned scripts by default, so a script somebody downloads to
/// check a download is the exact case it blocks.
fn windows() -> String {
    r##"@echo off
rem Rebuild a VeilVoice release and compare it with the published binary.
rem
rem     reproduce-veilvoice.cmd <version>      e.g.  reproduce-veilvoice.cmd v0.1.15
rem
rem Checking a download against SHA256SUMS proves the file is the one whose
rem hash was signed, and says nothing about what is inside it: the same person
rem signed both. This rebuilds the release from source, so a match means the
rem published binary is what this source compiles to.
rem
rem Needs git and a rustup-managed Rust toolchain. It installs neither. Being
rem asked to let an unfamiliar program put a compiler on your machine, as part
rem of deciding whether to trust that program, is not a reasonable thing to be
rem asked.
rem
rem The compiler version is not written down here on purpose: rust-toolchain.toml
rem in the checkout pins it and rustup reads that by itself. A version typed
rem into this file would be wrong the first time the pin moved, and would
rem report a genuine release as tampered with.

setlocal enabledelayedexpansion

if "%~1"=="" (
  echo usage: %~n0 ^<version^>    e.g. %~n0 v0.1.15
  exit /b 2
)
set VERSION=%~1

where git >nul 2>&1 || (echo FAILED: git is not installed. & exit /b 1)
where cargo >nul 2>&1 || (echo FAILED: cargo is not installed. & exit /b 1)
where rustup >nul 2>&1 || echo note: rustup was not found, so the pinned compiler is not guaranteed.

set WORK=%TEMP%\veilvoice-reproduce-%RANDOM%
mkdir "%WORK%" || (echo FAILED: could not make a working directory. & exit /b 1)
echo building in   %WORK%

git clone --quiet --depth 1 --branch %VERSION% https://github.com/tilas01/veilvoice "%WORK%\src"
if errorlevel 1 (echo FAILED: no such version: %VERSION% & rmdir /s /q "%WORK%" & exit /b 1)
cd /d "%WORK%\src"

rem The commit date, which is what the release build used. Without it the
rem binary carries the time of this build and can never match.
for /f %%d in ('git log -1 --pretty=%%ct') do set SOURCE_DATE_EPOCH=%%d

set RUSTFLAGS=--remap-path-prefix=%WORK%\src=/veilvoice

echo compiling     this takes a while, and it is meant to
cargo build --release --locked --quiet
if errorlevel 1 (echo FAILED: the build did not succeed. & cd /d %TEMP% & rmdir /s /q "%WORK%" & exit /b 1)

echo.
echo binary                          rebuilt here
for %%b in (veilvoice.exe veilvoice-gui.exe veilvoice-verify.exe) do (
  if exist "target\release\%%b" (
    for /f "skip=1 tokens=*" %%h in ('certutil -hashfile "target\release\%%b" SHA256') do (
      if not defined DONE_%%b (
        echo %%b   %%h
        set DONE_%%b=1
      )
    )
  )
)

echo.
echo Compare those against SHA256SUMS in the release, which lists the archives
echo rather than the binaries: unpack the archive for this platform and hash
echo the binaries inside it with:
echo.
echo     certutil -hashfile ^<the unpacked binary^> SHA256
echo.
echo A match means the published binary is what this source compiles to. A
echo mismatch is worth reporting, and is not by itself proof of anything: a
echo different compiler version or different build flags produce a different
echo binary from identical source.

cd /d %TEMP%
rmdir /s /q "%WORK%"
endlocal
"##
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The version pinned in `rust-toolchain.toml` must not appear in any
    /// script.
    ///
    /// This is the one thing that would turn a helpful script into a harmful
    /// one. A script carrying its own copy of the compiler version keeps
    /// working until the pin moves, and then reports a *genuine* release as
    /// not matching, which is the most alarming thing this project could tell
    /// somebody and would be false.
    #[test]
    fn no_script_carries_its_own_copy_of_the_toolchain_version() {
        let pinned = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../rust-toolchain.toml"
        ));
        let channel = pinned
            .lines()
            .find_map(|line| line.trim().strip_prefix("channel = "))
            .map(|value| value.trim().trim_matches('"').to_string())
            .expect("the toolchain file pins a channel");
        assert!(!channel.is_empty());
        for system in System::ALL {
            assert!(
                !script(*system).contains(&channel),
                "the {} script names the pinned compiler version ({channel}). \
                 When the pin moves, that script reports a genuine release as \
                 a mismatch.",
                system.key()
            );
        }
    }

    /// Each system gets the hash tool it actually has. `sha256sum` is not on
    /// macOS and is not on the BSDs, and a script that calls one there fails
    /// at the last step, after appearing to work for several minutes.
    #[test]
    fn each_system_uses_a_hash_tool_it_has() {
        assert!(script(System::Linux).contains("sha256sum "));
        assert!(script(System::MacOs).contains("shasum -a 256"));
        assert!(!script(System::MacOs).contains("sha256sum "));
        assert!(script(System::Bsd).contains("sha256 -q"));
        assert!(!script(System::Bsd).contains("sha256sum "));
        assert!(script(System::Windows).contains("certutil -hashfile"));
    }

    /// The commit date has to be exported, or the binary carries the time of
    /// this build and can never match anything.
    #[test]
    fn every_script_pins_the_build_date_to_the_commit() {
        for system in System::ALL {
            assert!(
                script(*system).contains("SOURCE_DATE_EPOCH"),
                "the {} script does not pin the build date, so its output can \
                 never match a published binary",
                system.key()
            );
        }
    }

    /// It builds; it does not install a compiler.
    #[test]
    fn no_script_installs_a_toolchain() {
        for system in System::ALL {
            let text = script(*system);
            for forbidden in ["rustup install", "rustup toolchain install", "rustup-init"] {
                assert!(
                    !text.contains(forbidden),
                    "the {} script runs {forbidden:?}",
                    system.key()
                );
            }
        }
    }

    /// A locked build, or the dependency versions are whatever resolved today
    /// and the result cannot match.
    #[test]
    fn every_script_builds_from_the_committed_lockfile() {
        for system in System::ALL {
            assert!(
                script(*system).contains("--locked"),
                "the {} script does not build with --locked",
                system.key()
            );
        }
    }

    #[test]
    fn every_system_survives_being_written_down_and_read_back() {
        for system in System::ALL {
            assert_eq!(System::from_key(system.key()), Some(*system));
        }
        assert_eq!(System::from_key("nonsense"), None);
        assert_eq!(System::from_key("wsl"), Some(System::Linux));
    }

    /// The system this is running on is one of the four, whatever it is.
    #[test]
    fn this_machine_is_one_of_the_systems() {
        assert!(System::ALL.contains(&System::here()));
    }
}
