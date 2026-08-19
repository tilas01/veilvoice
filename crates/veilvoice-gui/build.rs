// SPDX-License-Identifier: GPL-3.0-or-later
//! Embed the Windows application icon and version information.
//!
//! Without this the executable has no icon: Explorer draws the generic glyph,
//! the taskbar shows it, and a pinned shortcut looks like nothing in
//! particular. The icon used to be shipped *beside* the binary as a loose
//! `.ico` -- a file Windows never reads.
//!
//! `assets/icon.ico` already carries all six sizes Windows asks for, generated
//! by `assets/generate.py` from the same pixels as everything else.
//!
//! # Two different `cfg`s, and confusing them broke the build
//!
//! `winresource` is declared under `[target.'cfg(windows)'.build-dependencies]`,
//! and for a **build** dependency that `cfg` describes the **host** doing the
//! compiling -- not the target being compiled for. So on a Linux runner the
//! crate is simply absent, and a build script that named it unconditionally
//! failed to compile before it could check anything. That is what the first
//! version of this file did, and CI caught it.
//!
//! Hence both gates:
//!
//! * `#[cfg(windows)]` on the code, so it exists only where the dependency
//!   does -- a question about the host;
//! * `CARGO_CFG_TARGET_OS` at run time, so a Windows host cross-compiling for
//!   Linux does not staple a Windows resource onto an ELF binary -- a question
//!   about the target.
//!
//! The consequence worth stating plainly: **cross-compiling to Windows from a
//! non-Windows host produces a binary with no icon.** The release workflow
//! builds Windows on a Windows runner, so shipped binaries have one, and the
//! release fails if they do not -- `tools/release/check-windows-icons.py` reads
//! the built PE rather than trusting this file.

fn main() {
    #[cfg(windows)]
    embed();
}

#[cfg(windows)]
fn embed() {
    // The host is Windows; this asks whether the *target* is too.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=../../assets/icon.ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../../assets/icon.ico");
    resource.set("ProductName", "VeilVoice");
    resource.set(
        "FileDescription",
        "VeilVoice - irreversible voice de-identification",
    );
    resource.set("LegalCopyright", "tilas01 - GPL-3.0-or-later");
    resource.set("OriginalFilename", "veilvoice-gui.exe");

    if let Err(error) = resource.compile() {
        panic!(
            "could not embed the Windows icon: {error}\n\
             The icon is part of what ships, so this stops the build rather \
             than producing an executable with no icon and a green CI run."
        );
    }
}
