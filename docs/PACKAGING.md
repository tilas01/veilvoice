<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Packaging

Package definitions for the platforms that have one, in [`packaging/`](../packaging/).

## Status: two of them have been built

**Two of these have produced packages, and four have not.** The rest are
written against each format's documentation and validated as far as parsing
goes: the WiX source and the AppStream metadata are well-formed XML, the
Flatpak manifest is valid YAML. That is all that can honestly be claimed about
them.

A spec file that has never produced an RPM is a draft, and this table says
which is which:

| Format | File | Built? | Installed and run? |
|---|---|---|---|
| Windows MSI | `packaging/wix/veilvoice.wxs` | no | no |
| Debian/Ubuntu | `packaging/debian/` | **yes**, on Ubuntu 24.04, x86-64 | **yes**, installed and removed |
| Fedora/RHEL/SUSE | `packaging/rpm/veilvoice.spec` | **yes**, on Ubuntu 24.04, x86-64 | binaries run; **not** installed by `rpm` |
| Flatpak | `packaging/flatpak/` | no | no |
| Homebrew | `packaging/homebrew/veilvoice.rb` | no | no |
| Gentoo | `packaging/gentoo/` | no | no |

### What the RPM build proved, and what it did not

`rpmbuild -bs` produced `veilvoice-0.1.14-1.src.rpm` from a `git archive`
tarball, and `rpmbuild -bb` produced two binary packages:
`veilvoice-0.1.14-1.x86_64.rpm` and `veilvoice-gui-0.1.14-1.x86_64.rpm`.

The thing a build proves that a parse cannot is that `%files` and `%install`
agree. They do: the main package carries `/usr/bin/veilvoice`,
`/usr/bin/veilvoice-verify`, the licence and the whole of `docs/`; the `gui`
subpackage carries `/usr/bin/veilvoice-gui`, the desktop entry and the icon.
Nothing is listed that is not installed and nothing is installed that is not
listed, which is the classic spec defect and is the one that had never been
looked for. Extracted with `rpm2cpio`, the packaged `veilvoice --version`,
`veilvoice info` and `veilvoice-verify --version` all ran.

What it did not prove, and the gap is wider than the Debian one. **This was
built on Ubuntu, not on Fedora or RHEL or openSUSE**, so `%{dist}`, the system
RPM macros and the distribution's own Rust packaging are all untested. It
needed `--nodeps`, because `rpm` on a Debian machine cannot read `dpkg`'s
database and so cannot see that `cargo`, `gcc`, `alsa`, `gtk+-3.0` and
`xkbcommon` are in fact all installed; every one of them was confirmed present
by hand before the flag was used. It needed `--nocheck`, so the spec's own
`%check` stanza has never run as part of an RPM build, although the identical
command runs inside the Debian build. It has not been installed with `rpm -i`,
because doing that on a Debian machine tests nothing, and `rpmlint` has not
been run. Nothing has been uploaded anywhere.

### What the Debian build actually proved, and what it did not

`dpkg-buildpackage -us -uc -b` produced `veilvoice_0.1.14-1_amd64.deb` and
`veilvoice-gui_0.1.14-1_amd64.deb`. Both installed with `dpkg -i`, the
installed `veilvoice --version` reported 0.1.14, `veilvoice info` and
`veilvoice-verify --help` ran, and both packages removed cleanly. The release
build and `cargo test --release --workspace` both ran as part of it, because
that is what `debian/rules` does.

What it did not prove: this was one machine, on x86-64, with the toolchain from
rustup rather than from Debian's own `cargo` and `rustc` packages, which is why
the build needed `-d` to get past `dpkg-checkbuilddeps`. On a real Debian build
machine those packages are what `Build-Depends` names and the flag is not
wanted. Nothing has been uploaded anywhere.

**`lintian` has now been run**, over both packages. The first run found no
errors and five warnings; three of those were `no-manual-page`, one per binary,
and that one was a real gap rather than a formality: somebody who installs a
package on a Unix system types `man veilvoice` and got nothing.

Those three are fixed. `tools/release/manpage.py` derives each page from the
binary's own `--help` during `override_dh_auto_install`, so nothing is
committed and no page can drift from the command it describes. A second run of
`lintian` over the rebuilt packages reports **no errors and two warnings**,
both `initial-upload-closes-no-bugs`, which asks the changelog to close an ITP
bug and applies to a package being uploaded into Debian's own archive rather
than one a project publishes itself.

Verified rather than assumed: `dpkg -c` shows `veilvoice.1.gz` and
`veilvoice-verify.1.gz` in the main package and `veilvoice-gui.1.gz` in the
`gui` one, and each was installed and rendered with `groff`. That took one
detour worth recording, because it looked like a packaging bug and was not:
this build machine is a *minimized* Ubuntu image, which carries
`path-exclude=/usr/share/man/*` in `/etc/dpkg/dpkg.cfg.d/excludes` and throws
manual pages away as it installs them. The pages were in the packages the whole
time.

Doing it found two defects, F-80 and F-81, both recorded in
[`AUDIT.md`](AUDIT.md). The first was that the recipe below could not run at
all. The second was that every definition here still named v0.1.9 while the
workspace was at v0.1.14, and nothing was watching:
`tools/site-tests/packaging.test.js` is watching now.

The install scripts and the portable verifier are tested — see
[INSTALL.md](INSTALL.md) — and they are the supported route until the rest of
this table changes. If you build one of these and it works, or does not, saying
so in an issue is the most useful thing you could contribute.

---

## The rule every one of them follows

**Optional third-party software is never installed silently and never by
default.** It is the same rule the install scripts follow, and each format
expresses it differently:

- **WiX** puts VB-CABLE and Audacity in the feature tree at `Level="1000"`,
  above the install level, so neither is selected unless the user turns it on.
  Even then they install a *shortcut to the download page*, not the software:
  VB-CABLE is proprietary donationware with its own licence, and this installer
  has no business accepting somebody else's terms on a user's behalf.
- **Debian, RPM, Gentoo** do not mention them at all. A distribution package
  that pulled in proprietary donationware as a dependency would rightly be
  rejected, and should be.
- **Flatpak** cannot install them, which is the sandbox working as intended.

## Why these build from source

Every definition here compiles the tagged source on the machine that will run
it, rather than repackaging a published binary. For a project whose argument is
that you can check it yourself, that is the more honest default — and each one
passes `--locked`, so it builds the dependency versions the project actually
tested rather than whatever resolves on the day. A package that silently drifts
from the tested graph is not the software that was audited.

Homebrew is a **formula**, not a cask, for the same reason. It also sidesteps
macOS notarisation, which this project cannot obtain: notarisation requires a
certificate issued to a verified legal identity, and this is published under a
pseudonym.

## What each one deliberately does not do

No package installs a service, a scheduled task, a driver, a shell hook, or
anything that runs at startup. A privacy tool with a background service is a
privacy tool with a background service. There is nothing here that needs one.

The Flatpak's permissions are the clearest statement of this, because they are
public and checkable without reading any code:

```yaml
# no --share=network
```

VeilVoice has no networking code and CI fails the build if an HTTP client enters
the dependency graph. The absence of that line is the form of that claim a user
can verify in one command:

```bash
flatpak info --show-permissions io.github.tilas01.VeilVoice
```

## Signing, and what is not signed

| Artefact | Signed? |
|---|---|
| `SHA256SUMS` in each release | **yes**, detached OpenPGP, key `8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A` |
| The release archives themselves | no — see below |
| The MSI (Authenticode) | no, and cannot be |
| The macOS binaries (notarisation) | no, and cannot be |

Binaries are never signed in place. The signature is over `SHA256SUMS` only, so
that signing and reproducibility do not conflict: a signature embedded in a
binary changes the binary, and two people building the same source would then
produce different files for a reason that has nothing to do with the source.

Authenticode and notarisation both require a certificate tied to a verified
legal identity. Windows will show "unknown publisher" and macOS Gatekeeper will
refuse to run the binaries until allowed explicitly. Both are stated in
[INSTALL.md](INSTALL.md) rather than worked around, and both are why the OpenPGP
signature over the hash list remains the real check: **verify the archive, then
install it.**

## Building each one

```bash
# Windows MSI (needs: dotnet tool install -g wix)
wix build packaging/wix/veilvoice.wxs -arch x64 \
    -d Version=0.1.14 -d BinDir=dist/veilvoice-v0.1.14-windows-x86_64 \
    -o dist/VeilVoice-0.1.14-x64.msi
```

The WiX source refers to `packaging/wix/LICENSE.rtf` for the licence dialog,
which is not committed: WiX needs RTF, and converting `LICENSE` to RTF is a
build step rather than a source file. Any converter will do; the text must be
the unmodified GPL-3.0.

```bash
# Debian / Ubuntu  (copy packaging/debian to ./debian first)
#
# `debian/source/format` is a file in a directory, and this repository keeps it
# as `source-format` so that `packaging/debian/` stays a flat directory of
# files. The move below is what turns one into the other.
#
# Add `-d` if your Rust came from rustup rather than from Debian's `cargo` and
# `rustc` packages: `dpkg-checkbuilddeps` looks for the packages named in
# Build-Depends and cannot see a rustup toolchain. On a real Debian build
# machine, leave it off.
cp -r packaging/debian debian
mkdir -p debian/source && mv debian/source-format debian/source/format
dpkg-buildpackage -us -uc -b

# Fedora / RHEL / openSUSE
rpmbuild -ba packaging/rpm/veilvoice.spec \
    --define "_sourcedir $PWD/dist" --define "vv_version 0.1.14"

# Flatpak  (regenerate cargo-sources.json from Cargo.lock first)
python flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
flatpak-builder --user --install build \
    packaging/flatpak/io.github.tilas01.VeilVoice.yml

# Homebrew
brew install --build-from-source packaging/homebrew/veilvoice.rb

# Gentoo  (from a local overlay)
mkdir -p /var/db/repos/local/media-sound
cp -r packaging/gentoo/media-sound/veilvoice /var/db/repos/local/media-sound/
ebuild /var/db/repos/local/media-sound/veilvoice/veilvoice-9999.ebuild manifest
emerge -av media-sound/veilvoice
```

`packaging/flatpak/cargo-sources.json` is not committed either: it is a
mechanical transform of `Cargo.lock` and regenerating it is a single command,
whereas a committed copy is one more thing that can silently fall out of step
with the lock file.

## Platform coverage

Eleven targets are built and published today, OpenBSD among them. It had failed
for two releases because of a declared toolchain floor that turned out to be
wrong; v0.1.11 is the first release to carry an OpenBSD archive.

| Platform | Built | Reproducibility checked |
|---|---|---|
| Windows x86_64 | yes | yes, twice in separate directories |
| macOS Apple Silicon | yes | yes |
| macOS Intel | yes | yes |
| Linux x86_64 (gnu, musl) | yes | yes |
| Linux arm64 (gnu, musl) | yes | yes |
| Linux armv7 (Raspberry Pi) | yes | yes |
| FreeBSD x86_64 | yes | **no** — built once in a VM |
| OpenBSD x86_64 | yes, since v0.1.11 | not-verified (built once, in a VM) |
| NetBSD x86_64 | yes | **no** — built once in a VM |

Windows 10 and 11 share one executable. They are not split, and will not be
unless a measurement says they should be: shipping two identical binaries under
different names is a way of looking thorough rather than being it.

**OpenBSD failed to build for two releases, and the cause was on this side.**

Its packaged Rust is 1.94.1, and this workspace declared `rust-version =
"1.96"` -- so `cargo` refused with "rustc 1.94.1 is not supported by the
following packages" before compiling a single line. The documentation here
described that as OpenBSD's ports being behind, and an earlier revision of this
section recorded that lowering the floor had been "considered and rejected"
because "the toolchain floor is a property of the code".

That reasoning was sound and the premise was never checked. When it finally
was -- by installing 1.94.0 and compiling every crate in the workspace,
including the GUI -- **everything built without a single error**. The declared
floor was not a property of the code. It was the version that happened to be
current on the day somebody typed it, and it cost two releases of OpenBSD
coverage.

`rust-version` is now `1.94`, measured rather than assumed.
`rust-toolchain.toml` still pins a newer toolchain for development and CI; the
two are different things, and only the first is a claim about what the code
needs. If something in the tree ever does need a newer feature, cargo will say
so by name, which is a better guard than a number nobody re-tests.

The three BSD builds run in emulated VMs on a Linux runner, are the most
fragile jobs in the workflow, and are allowed to fail without blocking a
release. When one fails the release simply ships without that archive. Each is
marked `not-verified` in its reproducibility report, because it is built once
rather than twice — that is a statement about what was checked, not a suspicion
about the binary.
