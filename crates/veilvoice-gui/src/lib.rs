// SPDX-License-Identifier: GPL-3.0-or-later
//! # veilvoice-gui
//!
//! The VeilVoice desktop application: an egui/eframe front-end, monospace
//! throughout: anonymise a file, scramble a microphone live, watch what is
//! listening, manage the app lock, choose how the app looks, and an about
//! panel that states the honest scope.
//!
//! The binary lives in `main.rs`; this library exists so the UI logic can be
//! unit tested without opening a window.
//!
//! That split is worth stating plainly, because it is the reason this crate has
//! tests at all: a binary crate cannot be unit tested, so everything with logic
//! in it -- the app lock's state machine, preference loading, palette
//! resolution, the reduced-motion decision -- lives here where a test can reach
//! it without a display server. `main.rs` holds only what genuinely needs a
//! window.
//!
//! # The modules
//!
//! | Module | What it owns |
//! |---|---|
//! | [`security`] | The unlock screen, the lock tab, and the at-rest controls |
//! | [`prefs`] | Preferences, and recovering from a corrupt preferences file |
//! | [`policy`] | Settings somebody has fixed, and the reason beside each one |
//! | [`settings`] | The settings tab |
//! | [`setup`] | Installing this copy, and the optional companions |
//! | [`theme`] | The palette, shared with the command-line front end |
//! | [`soundbar`] | The animated level meter |
//! | [`reduced_motion`] | Whether to animate at all |
//! | [`watchfeed`] | The device monitor, on a thread that is not this one |
//!
//! # Two rules this crate keeps
//!
//! **The user interface never softens a scope note.** Where a control has a
//! bound -- the app lock is a verifier and not disk encryption, tamper detection
//! detects rather than prevents -- the interface says so next to the control,
//! and tests fail the build if that text changes. Documentation nobody opens
//! does not protect anybody.
//!
//! **Animation is a preference that is honoured, not a decoration.**
//! [`reduced_motion`] resolves the platform's own setting alongside the user's
//! explicit choice, and the whole interface reads that answer rather than each
//! widget deciding for itself.
//!
//! # In plain words
//!
//! This is the window.
//!
//! Tabs down the top for the things the program does: disguise a file, scramble a
//! microphone as you talk, handle a recording with several people, watch for
//! anything using your microphone, put the app behind a password, check a
//! download, and change how it looks.
//!
//! Nine colour schemes, and your own if you write one. It does the slow work on
//! another thread, so the window keeps answering while it is busy.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod app;

/// The name of every tab the window shows, in the order it shows them.
///
/// Exported so that `veilvoice-gui --tabs` can print them and the screenshot
/// scripts can read them, rather than each carrying a copy of a list that goes
/// stale the first time a tab is added. When it went stale the failure was
/// silent: the run succeeded and the new tab simply had no picture.
pub fn tabs() -> impl Iterator<Item = &'static str> {
    app::Tab::ALL.iter().map(|tab| tab.key())
}

/// The name of every page of the Settings tab, in the order the menu shows
/// them.
///
/// Exported for `veilvoice-gui --settings-pages`, which the screenshot scripts
/// read so that neither of them carries a copy of a list that would go stale
/// the first time a page is added. That is the same failure `--tabs` exists to
/// prevent, one level down.
///
/// These are the deep-link names rather than the headings the menu draws:
/// lower case, and the same words a capture file is named after. A heading
/// rewritten does not rename a published picture, which is the reason the two
/// are separate at all.
pub fn settings_pages() -> impl Iterator<Item = &'static str> {
    settings::Page::ALL.iter().map(|(_, key, _, _)| *key)
}

/// Spawn a subprocess without a console window.
///
/// Every `Command` this crate creates goes through here, and a test asserts
/// it by reading the crate's own source.
///
/// On Windows a `Command` for a console program creates a console, and when
/// the parent is a GUI process with none of its own, Windows opens a
/// **window** for it, which appears and vanishes as the child runs. That is
/// what "a cmd prompt flashing randomly" was, and it is the half of roadmap
/// item 167 that has nothing to do with threads: a window that freezes and a
/// window that flashes a terminal are the same complaint from a reader, which
/// is why they are one item.
///
/// `creation_flags` is a **safe** API, so this costs nothing against this
/// crate's `#![forbid(unsafe_code)]`.
///
/// This used to live in [`reduced_motion`] as a private helper, with a test
/// that read that one file. It moved here when roadmap item 167 read the whole
/// crate and found the Studio's render and the group render each spawning
/// `ffmpeg` bare, six hundred lines from a comment explaining why that must
/// not happen. A wrapper only the module that wrote it can reach is a wrapper
/// the rest of the crate does not use.
pub(crate) fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    hide_console(std::process::Command::new(program))
}

/// The Windows half of [`command`].
///
/// A named function per platform rather than a `cfg` block inside one, for the
/// reason `veilvoice-setup`'s copy gives at length: the block version needs
/// `let mut` on Windows and no `mut` anywhere else, which compiles on Windows
/// and fails the Linux and macOS runners on `unused_mut`. Two signatures the
/// compiler checks beat one body that means different things per platform.
#[cfg(windows)]
fn hide_console(mut command: std::process::Command) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// The everywhere-else half of [`command`]: no console is created by spawning
/// a process in the first place, so there is nothing to suppress.
#[cfg(not(windows))]
fn hide_console(command: std::process::Command) -> std::process::Command {
    command
}

/// Where JetBrains Mono is on this machine, if it is anywhere.
///
/// Re-exported so `--typeface` can answer without a window and without the
/// binary reaching into a module the rest of it does not use.
pub fn jetbrains_mono_path() -> Option<std::path::PathBuf> {
    theme::jetbrains_mono_path()
}
pub mod autolock;
pub mod avnotice;
pub mod crashlog;
pub mod crashreport;
pub mod decoys;
pub mod dialog;
pub mod firstrun;
pub mod graphics;
pub mod group;
pub mod integrity;
pub mod layout;
pub mod monitor;
pub mod notify;
pub mod offthread;
pub mod pace;
pub mod palettes;
pub mod paths;
pub mod policy;
pub mod prefs;
pub mod probe;
pub mod reduced_motion;
pub mod security;
pub mod settings;
pub mod setup;
pub mod soundbar;
pub mod storage;
pub mod studio;
pub mod theme;
pub mod tour;
pub mod updates;
pub mod vault_store;
pub mod verify;
pub mod watchfeed;
pub mod window;

pub use app::VeilVoiceApp;

/// Crate version string, surfaced in the About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Draw one frame with no window, and discard what a real backend would have
/// uploaded.
///
/// Every test in this crate that renders headlessly goes through here.
/// `egui` 0.36 asserts that a frame's texture deltas were handled: dropping a
/// `FullOutput` that still carries one panics, which is exactly right for a
/// backend that forgot to upload a font atlas and exactly wrong for a test
/// that only wants the shapes back. Clearing it in one place beats repeating
/// the same two lines at fifteen call sites and forgetting it at the
/// sixteenth.
#[cfg(test)]
pub(crate) fn headless_frame(
    ctx: &egui::Context,
    input: egui::RawInput,
    add_contents: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let mut output = ctx.run_ui(input, add_contents);
    output.textures_delta.clear();
    output
}

#[cfg(test)]
mod spawn_tests {
    /// Every subprocess in this crate is spawned through [`super::command`].
    ///
    /// This reads the crate's own source rather than exercising the
    /// behaviour, because "no console window appeared" cannot be observed
    /// from a test, which is precisely why the defect reached a release. A
    /// bare spawn added later fails here rather than on somebody's desktop.
    ///
    /// # Why it reads the whole crate
    ///
    /// It used to read one file. `reduced_motion.rs` carried the wrapper and
    /// a test that scanned `reduced_motion.rs`, and that test passed for as
    /// long as it existed while `studio.rs` and `group.rs` each spawned
    /// `ffmpeg` bare: rendering a take to video, and rendering a group
    /// conversation, both flashed a console on Windows. A guard that reads
    /// the file it lives in only ever proves something about that file, and
    /// the module that bothered to write the rule is the least likely one to
    /// break it.
    ///
    /// `veilvoice-setup` learned the same lesson separately and at the same
    /// time, as F-210: its list named three modules of six and said in its own
    /// comment that it covered them all.
    ///
    /// # The list is checked rather than kept up to date
    ///
    /// `include_str!` takes a literal, so the list below cannot be built by
    /// walking the directory. What it can be is checked: every `mod` this
    /// file declares has to appear in it, and a module added without a line
    /// here fails this test instead of quietly going unscanned. That check is
    /// the part worth having, because a hand-kept list is wrong from the first
    /// module nobody thought about.
    ///
    /// Each file is cut at its first `#[cfg(test)]`. Two guards in this crate
    /// name the forbidden call in order to forbid it, and a check that trips
    /// over its own source is a check somebody deletes.
    #[test]
    fn every_subprocess_is_spawned_without_a_console_window() {
        // Split so this line is not itself a match: the needle cannot appear
        // whole in a file the scan reads, and this file is one of them.
        const NEEDLE: &str = concat!("Command", "::new");

        let sources = [
            // Normalised where they are read, for the reason F-72 gives: a
            // checkout with CRLF makes a line-by-line search quietly match
            // nothing, on Windows runners and nowhere else.
            ("app.rs", include_str!("app.rs").replace("\r\n", "\n")),
            (
                "autolock.rs",
                include_str!("autolock.rs").replace("\r\n", "\n"),
            ),
            (
                "avnotice.rs",
                include_str!("avnotice.rs").replace("\r\n", "\n"),
            ),
            (
                "crashlog.rs",
                include_str!("crashlog.rs").replace("\r\n", "\n"),
            ),
            (
                "crashreport.rs",
                include_str!("crashreport.rs").replace("\r\n", "\n"),
            ),
            ("decoys.rs", include_str!("decoys.rs").replace("\r\n", "\n")),
            ("dialog.rs", include_str!("dialog.rs").replace("\r\n", "\n")),
            (
                "firstrun.rs",
                include_str!("firstrun.rs").replace("\r\n", "\n"),
            ),
            (
                "graphics.rs",
                include_str!("graphics.rs").replace("\r\n", "\n"),
            ),
            ("group.rs", include_str!("group.rs").replace("\r\n", "\n")),
            (
                "integrity.rs",
                include_str!("integrity.rs").replace("\r\n", "\n"),
            ),
            ("layout.rs", include_str!("layout.rs").replace("\r\n", "\n")),
            (
                "monitor.rs",
                include_str!("monitor.rs").replace("\r\n", "\n"),
            ),
            ("notify.rs", include_str!("notify.rs").replace("\r\n", "\n")),
            (
                "offthread.rs",
                include_str!("offthread.rs").replace("\r\n", "\n"),
            ),
            ("pace.rs", include_str!("pace.rs").replace("\r\n", "\n")),
            (
                "palettes.rs",
                include_str!("palettes.rs").replace("\r\n", "\n"),
            ),
            ("paths.rs", include_str!("paths.rs").replace("\r\n", "\n")),
            ("policy.rs", include_str!("policy.rs").replace("\r\n", "\n")),
            ("prefs.rs", include_str!("prefs.rs").replace("\r\n", "\n")),
            ("probe.rs", include_str!("probe.rs").replace("\r\n", "\n")),
            (
                "reduced_motion.rs",
                include_str!("reduced_motion.rs").replace("\r\n", "\n"),
            ),
            (
                "security.rs",
                include_str!("security.rs").replace("\r\n", "\n"),
            ),
            (
                "settings.rs",
                include_str!("settings.rs").replace("\r\n", "\n"),
            ),
            ("setup.rs", include_str!("setup.rs").replace("\r\n", "\n")),
            (
                "soundbar.rs",
                include_str!("soundbar.rs").replace("\r\n", "\n"),
            ),
            (
                "storage.rs",
                include_str!("storage.rs").replace("\r\n", "\n"),
            ),
            ("studio.rs", include_str!("studio.rs").replace("\r\n", "\n")),
            ("theme.rs", include_str!("theme.rs").replace("\r\n", "\n")),
            ("tour.rs", include_str!("tour.rs").replace("\r\n", "\n")),
            (
                "updates.rs",
                include_str!("updates.rs").replace("\r\n", "\n"),
            ),
            (
                "vault_store.rs",
                include_str!("vault_store.rs").replace("\r\n", "\n"),
            ),
            ("verify.rs", include_str!("verify.rs").replace("\r\n", "\n")),
            (
                "watchfeed.rs",
                include_str!("watchfeed.rs").replace("\r\n", "\n"),
            ),
            ("window.rs", include_str!("window.rs").replace("\r\n", "\n")),
        ];

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
        assert!(!declared.is_empty(), "no modules were read out of lib.rs");
        for module in &declared {
            assert!(
                sources.iter().any(|(name, _)| name == module),
                "{module} is declared in lib.rs and is not scanned for bare \
                 spawns. Add it to the list above: a module left out of it is \
                 a module where this rule is not enforced, which is the shape \
                 of the defect this test exists for."
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
                bare.push(format!("{}:{}: {}", name, number + 1, trimmed));
            }
        }
        assert!(
            bare.is_empty(),
            "these spawns bypass `crate::command`, so each flashes a console \
             window on Windows. Route them through it:\n{}",
            bare.join("\n")
        );
    }
}
