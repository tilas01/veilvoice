# SPDX-License-Identifier: GPL-3.0-or-later
#
# RPM spec for VeilVoice (Fedora, RHEL, openSUSE).
#
#   rpmbuild -ba packaging/rpm/veilvoice.spec \
#            --define "_sourcedir $PWD/dist" --define "vv_version 0.1.15"
#
# Builds from the published source tarball rather than repackaging a binary,
# which is what a distribution package is supposed to do: the person installing
# it gets something their own machine compiled from source they can read.

%global vv_version %{?vv_version}%{!?vv_version:0.1.15}

Name:           veilvoice
Version:        %{vv_version}
Release:        1%{?dist}
Summary:        Irreversible voice de-identification, fully offline

# The whole work is GPL-3.0-or-later. Dependencies are permissive and are
# statically linked by cargo, which is compatible in that direction.
License:        GPL-3.0-or-later
URL:            https://github.com/tilas01/veilvoice
Source0:        https://github.com/tilas01/veilvoice/archive/refs/tags/v%{version}.tar.gz#/%{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust >= 1.96
BuildRequires:  pkgconfig(alsa)
BuildRequires:  gcc
BuildRequires:  python3
# The desktop app needs the windowing stack; the CLI does not.
BuildRequires:  pkgconfig(gtk+-3.0)
BuildRequires:  pkgconfig(xkbcommon)

%description
VeilVoice destroys the biometric voiceprint of a speaker -- pitch, formants,
timbre, micro-timing and the melody of an accent -- so that neither software nor
a human listener can re-identify the speaker or reconstruct the original voice,
while the words themselves stay clean and transcribable.

It does not hide what was said. Intelligibility is preserved on purpose, and the
words remain in the output and can be transcribed. If the message itself is
sensitive, encrypt it; that is a separate problem with a separate answer.

Fully offline by construction: there is no networking code in the project and
the build fails if an HTTP client enters the dependency graph.

%package        gui
Summary:        Desktop application for VeilVoice
Requires:       %{name} = %{version}-%{release}

%description    gui
The VeilVoice desktop application.

%prep
%autosetup -n %{name}-%{version}

%build
# --locked: build exactly the dependency versions the project tested, rather
# than whatever resolves today. A package that silently drifts from the tested
# graph is not the software that was audited.
cargo build --release --locked --workspace

%install
install -Dpm 0755 target/release/veilvoice        %{buildroot}%{_bindir}/veilvoice
install -Dpm 0755 target/release/veilvoice-verify %{buildroot}%{_bindir}/veilvoice-verify
install -Dpm 0755 target/release/veilvoice-gui    %{buildroot}%{_bindir}/veilvoice-gui
install -Dpm 0644 assets/icon.png                 %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/veilvoice.png
install -Dpm 0644 packaging/veilvoice.desktop     %{buildroot}%{_datadir}/applications/veilvoice.desktop

# Manual pages, generated from each binary's own `--help`. Written out by hand
# they would be a second description of the interface, kept in step with the
# first by nothing but attention.
python3 tools/release/manpage.py target/release/veilvoice \
    %{buildroot}%{_mandir}/man1/veilvoice.1
python3 tools/release/manpage.py target/release/veilvoice-verify \
    %{buildroot}%{_mandir}/man1/veilvoice-verify.1 \
    --summary "check a VeilVoice release against its signature"
python3 tools/release/manpage.py target/release/veilvoice-gui \
    %{buildroot}%{_mandir}/man1/veilvoice-gui.1 \
    --summary "the VeilVoice desktop application"

%check
cargo test --release --locked --workspace

%files
%license LICENSE
%doc README.md docs/
%{_bindir}/veilvoice
%{_bindir}/veilvoice-verify
%{_mandir}/man1/veilvoice.1*
%{_mandir}/man1/veilvoice-verify.1*

%files gui
%{_bindir}/veilvoice-gui
%{_mandir}/man1/veilvoice-gui.1*
%{_datadir}/icons/hicolor/256x256/apps/veilvoice.png
%{_datadir}/applications/veilvoice.desktop

%changelog
* Sun Aug 31 2026 tilas01 <tilas01@users.noreply.github.com> - 0.1.15-1
- See CHANGELOG.md in the source for what changed. The newest entry here is
- compared against the workspace version by the site suite.
* Fri Aug 28 2026 tilas01 <tilas01@users.noreply.github.com> - 0.1.14-1
- See CHANGELOG.md in the source for what changed. The newest entry here is
- compared against the workspace version by the site suite, so it cannot go
- five releases stale again without the build failing.
* Tue Aug 18 2026 tilas01 <tilas01@users.noreply.github.com> - 0.1.9-1
- Search across the repository and website, a portable verifier, install scripts.
