// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-setup
//!
//! Everything that puts VeilVoice on a machine, and everything that reports
//! what is already on it. [`install`] does the per-user install and its exact
//! reversal, [`companions`] finds the optional third-party software that live
//! mode is easier with and says who makes each piece, [`update`] reports
//! whether a newer release exists, [`space`] asks the system how much room is
//! free, and [`volumes`] reports the encrypted volumes that are mounted.
//!
//! This paragraph said "two modules" while five were declared below it, which
//! is the same class of thing as the spawn test naming three files out of six.
//!
//! # Why this is a library and not part of the command line
//!
//! It was part of the command line. `install.rs` lived inside the
//! `veilvoice-cli` **binary** crate, which meant the desktop application could
//! not call a single line of it, because a binary crate has no consumers. The choice
//! was to reimplement the installer behind the graphical front end, or to move
//! the logic somewhere both front ends can reach. Reimplementing it would have
//! produced two programs that edit `PATH`, drifting apart at whatever rate
//! nobody noticed, and the `PATH` edit is the one operation here that can
//! damage a machine.
//!
//! So this crate is the installer, and both front ends are front ends. The
//! command line calls [`install::install`]; so does the desktop app's setup
//! tab. There is one implementation of the careful part, and one set of tests
//! covering it.
//!
//! # What it will not do
//!
//! **It never runs somebody else's installer.** [`companions`] reports what is
//! present and prepares an exact command for what is not, and where that
//! command is "open the vendor's download page" it says so rather than
//! fetching and executing an unverified binary. A project whose entire subject
//! is verifying what you run has no business being casual about that.
//!
//! **It never asks for administrator rights.** Everything [`install`] does is
//! inside the user's own account. Where a companion genuinely needs privilege,
//! such as a system package manager or an audio driver, that fact is reported and the
//! command is handed over rather than run, because a graphical program cannot
//! honestly collect a `sudo` password and this one does not try.
//!
//! **Nothing is ticked by default.** There are no checkboxes at all: each
//! companion is a separate, deliberate action. The rule that predates this
//! crate, which is to detect, say what it is and who makes it, and act only on
//! an explicit yes, is unchanged by the interface getting prettier.
//!
//! # No `unsafe`, and therefore some subprocesses
//!
//! `#![forbid(unsafe_code)]` holds here as everywhere else in the workspace,
//! so the Windows registry is reached through `reg.exe` rather than the Win32
//! API. [`command`] wraps every spawn so that none of them flashes a console
//! window when the desktop application is the caller, which is the defect that
//! shipped in v0.1.10, and a test reads this crate's own source to catch a spawn that
//! forgets.
//!
//! # In plain words
//!
//! This installs the program, if you want it installed.
//!
//! You do not have to. Unzipping it and running it is a perfectly normal way to
//! use it, and this says so rather than treating it as a mistake. Installing does
//! three small things -- copies the program into your own folder, adds it to your
//! PATH so typing its name works, and adds an entry so Windows can remove it --
//! and nothing else. No administrator rights, no service.
//!
//! It also looks for the few other programs VeilVoice can work alongside, tells
//! you who makes each one, and installs none of them unless you say so.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod companions;
pub mod install;
pub mod space;
pub mod update;
pub mod volumes;

use std::ffi::OsStr;
use std::process::Command;

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Spawn without a console window.
///
/// On Windows a `Command` for a console program creates a console, and when
/// the caller is the desktop application that console flashes on screen. That
/// is not cosmetic: three of them at startup were most of what "it flashes a
/// command prompt and loads in an unusable state" meant in the v0.1.10 report.
///
/// `creation_flags` is a **safe** API, so this costs nothing against
/// `#![forbid(unsafe_code)]`. Every spawn in this crate goes through here, and
/// a test in this file reads the source to prove it.
pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    let command = Command::new(program);
    hide_console(command)
}

/// The Windows half of [`command`].
///
/// A named function per platform rather than a `cfg` block inside one, because
/// the block version needs `let mut` on Windows and no `mut` anywhere else --
/// which compiles here and fails the Linux and macOS runners on `unused_mut`.
/// That exact shape has cost this project three CI failures, so the difference
/// is expressed as two signatures the compiler can check instead of one body
/// that means different things.
#[cfg(windows)]
fn hide_console(mut command: Command) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// The everywhere-else half of [`command`]: nothing to hide, and no console
/// is created by spawning a process in the first place.
#[cfg(not(windows))]
fn hide_console(command: Command) -> Command {
    command
}

#[cfg(test)]
mod tests {
    /// Every subprocess in this crate must be spawned through [`super::command`].
    ///
    /// This reads the crate's own source rather than exercising the behaviour,
    /// because "no console window appeared" cannot be observed from a test --
    /// which is precisely why the defect reached a release. A `Command::new`
    /// added later without the wrapper fails here rather than on a desktop.
    ///
    /// # The list is checked rather than kept up to date
    ///
    /// It scans every module, because the wrapper lives in one place and the
    /// spawns do not. The list below used to be written by hand and said in its
    /// own comment that it covered every module while naming three of the six.
    /// `update.rs` spawned curl and `space.rs` spawned `df` and `fsutil.exe`
    /// outside the wrapper for as long as they had existed, so the button that
    /// checks for an update and the Storage tab's free-space reading each
    /// flashed a console window on Windows: the exact defect of v0.1.10, in a
    /// crate whose test for it passed. See F-210.
    ///
    /// `include_str!` takes a literal, so the list cannot be built from the
    /// directory. What it can be is **checked**: every `mod` declared in this
    /// file must appear below, and a module added without a line here fails
    /// this test rather than going unscanned.
    ///
    /// Each file is cut at its `cfg(test)` marker first. A test module that
    /// searches for the name of a type necessarily contains that name, and a
    /// check that trips over its own source is a check somebody deletes.
    #[test]
    fn every_subprocess_is_spawned_without_a_console_window() {
        const NEEDLE: &str = concat!("Command", "::new");
        let sources = [
            // Normalised where they are read, for the reason in F-72: a
            // checkout with CRLF makes a search for "\n}\n" match nothing.
            ("lib.rs", include_str!("lib.rs").replace("\r\n", "\n")),
            (
                "install.rs",
                include_str!("install.rs").replace("\r\n", "\n"),
            ),
            (
                "companions.rs",
                include_str!("companions.rs").replace("\r\n", "\n"),
            ),
            ("space.rs", include_str!("space.rs").replace("\r\n", "\n")),
            ("update.rs", include_str!("update.rs").replace("\r\n", "\n")),
            (
                "volumes.rs",
                include_str!("volumes.rs").replace("\r\n", "\n"),
            ),
        ];

        // Every module this crate declares is in the list above. Written as a
        // check because the hand-kept version of that claim was false.
        let lib = include_str!("lib.rs").replace("\r\n", "\n");
        let declared: Vec<String> = lib
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let rest = line
                    .strip_prefix("pub mod ")
                    .or_else(|| line.strip_prefix("mod "))?;
                rest.strip_suffix(';').map(|name| format!("{name}.rs"))
            })
            .collect();
        for module in &declared {
            assert!(
                sources.iter().any(|(name, _)| name == module),
                "{module} is declared in lib.rs but is not scanned for bare \
                 spawns. Add it to the list above: a module left out of it is \
                 a module where this rule is not enforced, which is F-210."
            );
        }

        let mut bare = Vec::new();
        for (name, source) in &sources {
            let shipped = match source.find("#[cfg(test)]") {
                Some(at) => &source[..at],
                None => source.as_str(),
            };
            for (number, line) in shipped.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || trimmed.starts_with('*') {
                    continue; // prose: this rule is discussed in the comments
                }
                if !line.contains(NEEDLE) {
                    continue;
                }
                // The one permitted occurrence is the wrapper's own.
                if *name == "lib.rs" && trimmed.starts_with("let command = ") {
                    continue;
                }
                bare.push(format!("{}:{}: {}", name, number + 1, trimmed));
            }
        }
        assert!(
            bare.is_empty(),
            "these spawns bypass `command()`, so each flashes a console window \
             on Windows:\n{}",
            bare.join("\n")
        );
    }
}
