<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# packaging/

Package definitions for each platform that has one.

**None of these has been built or installed yet.** They parse, and that is all
that is claimed. See [`docs/PACKAGING.md`](../docs/PACKAGING.md) for the status
table, the build commands, and what each format deliberately does not do.

The tested route to a verified install is [`install/`](../install/) and the
portable verifier — see [`docs/INSTALL.md`](../docs/INSTALL.md).

| | |
|---|---|
| `wix/` | Windows MSI. Optional components are off by default and install links, not software. |
| `debian/` | `.deb` for Debian and Ubuntu. |
| `rpm/` | `.spec` for Fedora, RHEL and openSUSE. |
| `flatpak/` | Flatpak manifest and AppStream metadata. No network permission. |
| `homebrew/` | A formula, not a cask: it builds from source. |
| `gentoo/` | A live ebuild that builds from this repository. |
| `veilvoice.desktop` | Shared desktop entry for the Linux packages. |
