# Copyright 2026 tilas01
# Distributed under the terms of the GNU General Public License v3 or later
# SPDX-License-Identifier: GPL-3.0-or-later

EAPI=8

# A live ebuild that builds from this repository the way every other Gentoo
# package does: fetch the source, compile it on the user's machine, install it
# through the package manager. No binary is downloaded and nothing is trusted
# that the machine did not compile itself.
#
# For a fixed release, copy this to veilvoice-0.1.9.ebuild, drop the git-r3
# inherit and the EGIT_ lines, and set SRC_URI to the release tarball.

CRATES=""

inherit cargo git-r3

DESCRIPTION="Irreversible voice de-identification, fully offline"
HOMEPAGE="https://tilas01.github.io/veilvoice/"
EGIT_REPO_URI="https://github.com/tilas01/veilvoice.git"
EGIT_BRANCH="main"

# The crate itself is GPL-3+. Its dependencies are all permissive, which is
# compatible in that direction; the second list is what Gentoo expects for the
# vendored crates.
LICENSE="GPL-3+"
LICENSE+=" Apache-2.0 BSD BSD-2 ISC MIT Unicode-3.0 ZLIB"
SLOT="0"
KEYWORDS="~amd64 ~arm64"

IUSE="+cli gui live"
REQUIRED_USE="|| ( cli gui )"

DEPEND="
	live? ( media-libs/alsa-lib )
	gui? (
		x11-libs/gtk+:3
		x11-libs/libxkbcommon
		media-libs/libglvnd
	)
"
RDEPEND="${DEPEND}"
BDEPEND=">=virtual/rust-1.96"

QA_FLAGS_IGNORED="usr/bin/veilvoice usr/bin/veilvoice-gui"

src_configure() {
	local myfeatures=()

	# `live` is a default-on cargo feature, so it has to be switched off
	# explicitly rather than merely not requested.
	if ! use live; then
		myfeatures+=( --no-default-features )
	fi

	cargo_src_configure "${myfeatures[@]}"
}

src_compile() {
	local packages=()
	use cli && packages+=( -p veilvoice-cli )
	use gui && packages+=( -p veilvoice-gui )

	cargo_src_compile "${packages[@]}"
}

src_test() {
	cargo_src_test --workspace
}

src_install() {
	if use cli; then
		dobin "$(cargo_target_dir)/veilvoice"
	fi
	if use gui; then
		dobin "$(cargo_target_dir)/veilvoice-gui"
		newicon -s 256 assets/icon.png veilvoice.png
		domenu packaging/veilvoice.desktop
	fi

	dodoc README.md
	dodoc -r docs/.
	einstalldocs
}

pkg_postinst() {
	elog "VeilVoice destroys the voiceprint, not the words."
	elog "Intelligibility is preserved on purpose: the words remain in the"
	elog "output and can be transcribed. If the message itself is sensitive,"
	elog "encrypt it -- that is a separate problem with a separate answer."
	elog ""
	elog "Limits, stated rather than hidden:"
	elog "  - a strong regional accent may still be audible"
	elog "  - the application lock names and encrypts VeilVoice's own files,"
	elog "    but cannot hide that VeilVoice is installed, and anybody who"
	elog "    can read the folder can still delete it"
	elog "  - secure erase is unreliable on flash storage"
	elog ""
	if use live; then
		elog "Live mode needs a virtual audio device: media-sound/pulseaudio"
		elog "or a JACK/PipeWire loopback."
	else
		elog "Built without USE=live: file processing only, no microphone."
	fi
	elog ""
	# The verifier used to be its own binary and was installed unconditionally.
	# It is part of `veilvoice` now, so this line is only true where the command
	# line was actually built; USE=-cli gets pointed at the window instead.
	if use cli; then
		elog "Verify a release you did not build:  veilvoice verify --help"
	else
		elog "Verify a release you did not build: open VeilVoice and use the"
		elog "Verify tab. Same check, same code underneath."
	fi
}
