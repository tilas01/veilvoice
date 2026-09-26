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
pub mod progress;
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

/// Every module's source, for the guards that read the crate rather than run it.
///
/// One list, because there are two guards that need it and a second copy would
/// be a second thing to forget a module in. `include_str!` takes a literal, so
/// it cannot be built by walking the directory; what it can be is **checked**
/// against [`declared_modules`], which reads `lib.rs`'s own `mod` lines, so a
/// module added without a line here fails rather than going unscanned. That
/// check is the whole lesson of F-210, F-212 and F-216.
///
#[cfg(test)]
pub(crate) fn sources() -> Vec<(&'static str, String)> {
    vec![
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
            "progress.rs",
            include_str!("progress.rs").replace("\r\n", "\n"),
        ),
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
    ]
}

/// The modules `lib.rs` declares, as filenames.
///
/// Read from this file rather than listed, so that [`sources`] can be checked
/// against something that cannot be forgotten.
#[cfg(test)]
pub(crate) fn declared_modules() -> Vec<String> {
    include_str!("lib.rs")
        .replace("\r\n", "\n")
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line
                .strip_prefix("pub mod ")
                .or_else(|| line.strip_prefix("mod "))?;
            rest.strip_suffix(';').map(|name| format!("{name}.rs"))
        })
        .collect()
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

        let sources = crate::sources();

        let declared = crate::declared_modules();
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

#[cfg(test)]
mod draw_path_tests {
    //! Reading the crate for what the thread that draws is made to wait for.

    /// Comment and string contents replaced by spaces, offsets kept.
    ///
    /// Two guards in this crate name the call they forbid in order to forbid
    /// it, and a check that trips over its own explanation is a check somebody
    /// deletes. Blanking rather than removing keeps every byte offset, so a
    /// failure can name the real line.
    ///
    /// **Newlines survive the blanking.** The first version of this replaced
    /// them too, inside multi-line string literals, and every line number after
    /// the first long message in a file was reported five short.
    fn code_only(source: &str) -> String {
        let bytes: Vec<char> = source.chars().collect();
        let mut out = bytes.clone();
        let blank = |out: &mut Vec<char>, from: usize, to: usize, src: &[char]| {
            for k in from..to.min(src.len()) {
                if src[k] != '\n' {
                    out[k] = ' ';
                }
            }
        };
        let n = bytes.len();
        let mut i = 0;
        while i < n {
            let c = bytes[i];
            if c == '/' && i + 1 < n && bytes[i + 1] == '/' {
                let mut j = i;
                while j < n && bytes[j] != '\n' {
                    j += 1;
                }
                blank(&mut out, i, j, &bytes);
                i = j;
            } else if c == '/' && i + 1 < n && bytes[i + 1] == '*' {
                let mut depth = 1;
                let mut j = i + 2;
                while j < n && depth > 0 {
                    if bytes[j] == '/' && j + 1 < n && bytes[j + 1] == '*' {
                        depth += 1;
                        j += 2;
                    } else if bytes[j] == '*' && j + 1 < n && bytes[j + 1] == '/' {
                        depth -= 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                blank(&mut out, i, j, &bytes);
                i = j;
            } else if c == 'r' && i + 1 < n && (bytes[i + 1] == '"' || bytes[i + 1] == '#') {
                let mut hashes = 0;
                let mut j = i + 1;
                while j < n && bytes[j] == '#' {
                    hashes += 1;
                    j += 1;
                }
                if j < n && bytes[j] == '"' {
                    let mut k = j + 1;
                    loop {
                        if k >= n {
                            break;
                        }
                        if bytes[k] == '"' {
                            let closed = (1..=hashes).all(|h| k + h < n && bytes[k + h] == '#');
                            if closed {
                                k += hashes + 1;
                                break;
                            }
                        }
                        k += 1;
                    }
                    blank(&mut out, i + 1, k, &bytes);
                    i = k;
                } else {
                    i += 1;
                }
            } else if c == '"' {
                let mut j = i + 1;
                while j < n {
                    if bytes[j] == '\\' {
                        j += 2;
                        continue;
                    }
                    if bytes[j] == '"' {
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                blank(&mut out, i + 1, j.saturating_sub(1), &bytes);
                i = j;
            } else {
                i += 1;
            }
        }
        out.into_iter().collect()
    }

    /// Blank the argument of every call named here, so what runs inside it is
    /// not read as running on the drawing thread.
    ///
    /// These are the two sanctioned ways off it: `std::thread::spawn`, and the
    /// closure handed to [`crate::offthread::Answer::ask`]. Anything else that
    /// wants to get off the thread goes through one of them, which is the point
    /// of naming only two.
    fn blank_workers(code: &str) -> String {
        let chars: Vec<char> = code.chars().collect();
        let mut out = chars.clone();
        for opener in ["thread::spawn(", ".ask(", ".ask_once("] {
            let mut from = 0;
            while let Some(at) = find_from(&chars, opener, from) {
                let open = at + opener.chars().count() - 1;
                let mut depth = 0;
                let mut j = open;
                while j < chars.len() {
                    match chars[j] {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    j += 1;
                }
                for slot in out
                    .iter_mut()
                    .take(j.min(chars.len()))
                    .skip(open + 1)
                    .filter(|c| **c != '\n')
                {
                    *slot = ' ';
                }
                from = at + 1;
            }
        }
        out.into_iter().collect()
    }

    /// `str::find` over a `char` slice, so offsets stay in `char`s throughout.
    fn find_from(haystack: &[char], needle: &str, from: usize) -> Option<usize> {
        let needle: Vec<char> = needle.chars().collect();
        if needle.is_empty() || haystack.len() < needle.len() {
            return None;
        }
        (from..=haystack.len() - needle.len())
            .find(|&i| haystack[i..i + needle.len()] == needle[..])
    }

    /// One function: its name, its signature, and where its body starts and ends.
    struct Function {
        name: String,
        signature: String,
        from: usize,
        to: usize,
        /// Whether it is written outside every `impl` block, so that a bare
        /// call by its name is unambiguously a call of it.
        free: bool,
    }

    /// Every function in one module's code.
    fn functions(code: &str) -> Vec<Function> {
        let chars: Vec<char> = code.chars().collect();
        let impls = impl_spans(&chars);
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(at) = find_from(&chars, "fn ", from) {
            from = at + 1;
            // A word boundary before it, so `#[cfg(test)] fn` counts and
            // `asdf fn` in the middle of an identifier does not.
            if at > 0 && (chars[at - 1].is_alphanumeric() || chars[at - 1] == '_') {
                continue;
            }
            let mut i = at + 3;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let name: String = chars[at + 3..i].iter().collect();
            if name.is_empty() {
                continue;
            }
            // The parameter list. Generics may sit between, and may hold
            // parentheses of their own, so this finds the first `(` and matches
            // from there.
            while i < chars.len() && chars[i] != '(' && chars[i] != ';' && chars[i] != '{' {
                i += 1;
            }
            if i >= chars.len() || chars[i] != '(' {
                continue;
            }
            let mut depth = 0;
            let mut j = i;
            while j < chars.len() {
                match chars[j] {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let signature: String = chars[at..(j + 1).min(chars.len())].iter().collect();
            // The body, or nothing at all for a declaration in a trait.
            let mut k = j + 1;
            while k < chars.len() && chars[k] != '{' && chars[k] != ';' {
                k += 1;
            }
            if k >= chars.len() || chars[k] == ';' {
                continue;
            }
            let end = match_brace(&chars, k);
            out.push(Function {
                name,
                signature,
                from: k,
                to: end + 1,
                free: !impls.iter().any(|(a, b)| *a < k && k < *b),
            });
        }
        out
    }

    /// The index of the `}` closing the `{` at `open`.
    fn match_brace(chars: &[char], open: usize) -> usize {
        let mut depth = 0;
        for (i, c) in chars.iter().enumerate().skip(open) {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
        }
        chars.len().saturating_sub(1)
    }

    /// Where every `impl` block's braces are.
    fn impl_spans(chars: &[char]) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(at) = find_from(chars, "impl ", from) {
            from = at + 1;
            if at > 0 && chars[at - 1] != '\n' && !chars[at - 1].is_whitespace() {
                continue;
            }
            let mut i = at;
            while i < chars.len() && chars[i] != '{' && chars[i] != ';' {
                i += 1;
            }
            if i < chars.len() && chars[i] == '{' {
                out.push((i, match_brace(chars, i)));
            }
        }
        out
    }

    /// The line a character offset is on, counting from one.
    fn line_of(code: &str, offset: usize) -> usize {
        code.chars().take(offset).filter(|c| *c == '\n').count() + 1
    }

    /// Everything the drawing thread reaches, per module.
    ///
    /// A function is on the drawing thread when its parameters include a
    /// `&mut Ui`, or when it is the `update` that `eframe` calls. So is anything
    /// it calls **by name within its own module**: `self.name(`, `Self::name(`,
    /// or a bare `name(` that is a free function of that module.
    ///
    /// Same-module only, and deliberately. The first version of this followed
    /// calls across the whole crate by name and reported 495 of 523 functions
    /// as being on the drawing thread, because `run`, `write`, `load`, `status`
    /// and `poll` are each the name of several unrelated things here. A guard
    /// that answers "everything" answers nothing.
    fn reached(module: &str, code: &str) -> Vec<(String, usize, usize)> {
        let all = functions(code);
        let seeds: Vec<usize> = all
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                f.signature.contains("&mut Ui")
                    || f.signature.contains("&mut egui::Ui")
                    || (f.name == "update" && f.signature.contains("egui::Context"))
            })
            .map(|(i, _)| i)
            .collect();
        assert!(
            !seeds.is_empty() || !code.contains("&mut Ui"),
            "{module} mentions a `&mut Ui` and no function was recognised as \
             drawing, so this guard is reading nothing there"
        );

        let mut seen = std::collections::BTreeSet::new();
        let mut stack = seeds;
        while let Some(i) = stack.pop() {
            if !seen.insert(i) {
                continue;
            }
            let body: String = code
                .chars()
                .skip(all[i].from)
                .take(all[i].to - all[i].from)
                .collect();
            for (j, candidate) in all.iter().enumerate() {
                if seen.contains(&j) {
                    continue;
                }
                let called = body.contains(&format!("self.{}(", candidate.name))
                    || body.contains(&format!("Self::{}(", candidate.name))
                    || (candidate.free && calls_bare(&body, &candidate.name));
                if called {
                    stack.push(j);
                }
            }
        }
        seen.into_iter()
            .map(|i| (all[i].name.clone(), all[i].from, all[i].to))
            .collect()
    }

    /// Whether `body` calls `name` without a receiver or a path before it.
    ///
    /// `foo(` counts; `bar.foo(`, `Bar::foo(`, and `notfoo(` do not. Without
    /// this, `plan_for(` was read as a call of `for(` and every module with a
    /// loop in it acquired a phantom callee.
    fn calls_bare(body: &str, name: &str) -> bool {
        let needle = format!("{name}(");
        let chars: Vec<char> = body.chars().collect();
        let mut from = 0;
        while let Some(at) = find_from(&chars, &needle, from) {
            from = at + 1;
            if at == 0 {
                return true;
            }
            let before = chars[at - 1];
            if !before.is_alphanumeric() && before != '_' && before != '.' && before != ':' {
                return true;
            }
        }
        false
    }

    /// **Roadmap item 79, and roadmap item 167.** Nothing that waits happens on
    /// the thread that draws.
    ///
    /// A window stutters for one of two reasons: it is asked to draw too
    /// rarely, or it is doing something slow between frames. The second cannot
    /// be tuned away and is invisible in a screenshot: the window simply stops
    /// for as long as the call takes.
    ///
    /// # Why this reads the crate and not one file
    ///
    /// Because the version that read one file passed for months while five
    /// separate calls waited on the drawing thread in four other modules. It
    /// lived in `app.rs`, read `app.rs`, and was correct about `app.rs`. What it
    /// could not see: the setup tour forking `df` on every frame it drew, the
    /// setup tab re-reading the machine the moment an install finished, the
    /// Verify tab stat'ing the disk per frame, the program folder being opened
    /// and migrated and shredded inside one frame, and an export waiting for
    /// `ffmpeg` to render a video. F-216, F-218 and F-219.
    ///
    /// Four crates learned this in one week: F-210 in `veilvoice-setup`, F-212
    /// for this crate's spawns, and then these. **The scope a guard is written
    /// at is the scope of the fix that prompted it, and the defect is always
    /// somewhere else.** So this one is written at the crate.
    ///
    /// # What it cannot see, said plainly
    ///
    /// Calls that leave the module. `poll` reaching into another module's type
    /// which then reads a file is invisible here, because following names across
    /// the crate makes every function a hit. The frame-timing test is what
    /// covers that gap, and this covers what a name can prove.
    #[test]
    fn the_drawing_thread_never_waits_on_anything() {
        // Each of these waits. The stats are here because one per frame at
        // 60 Hz is sixty syscalls a second for an answer that changed when a
        // file was dropped: cheap on a warm local disk and not cheap on a
        // network share or a drive that has spun down.
        const WAITS: [&str; 24] = [
            "Command::new",
            ".output()",
            ".wait()",
            "std::fs::read",
            "std::fs::write",
            "std::fs::copy",
            "std::fs::remove",
            "std::fs::create_dir",
            "read_to_string",
            "thread::sleep",
            "devices::list",
            ".exists()",
            ".is_file()",
            ".is_dir()",
            "fs::metadata",
            "read_dir(",
            "canonicalize(",
            "shred_file",
            // Named one by one, because from this crate's text a call into
            // another crate is just a call. Each of these is expensive and the
            // name is the only thing that says so: `free_bytes` spawns `df` or
            // `fsutil.exe`, `install::status` walks the install folder and the
            // `PATH`, `detect_all` runs a command per companion, the two
            // `look`s query the graphics driver, and `ffmpeg::found` searches
            // the `PATH`. A guard reading one crate cannot work this out; it
            // can be told.
            "install::status",
            "space::free_bytes",
            "detect_all(",
            "probe::look()",
            "accel::look()",
            "ffmpeg::found()",
        ];

        let declared = crate::declared_modules();
        assert!(!declared.is_empty(), "no modules were read out of lib.rs");
        let sources = crate::sources();
        for module in &declared {
            assert!(
                sources.iter().any(|(name, _)| name == module),
                "{module} is declared in lib.rs and is not read for what the \
                 drawing thread waits on. Add it to `crate::sources`: a module \
                 left out of that list is a module where this rule is not \
                 enforced, which is F-216."
            );
        }

        let mut faults: Vec<String> = Vec::new();
        for (module, source) in &sources {
            let shipped = match source.find("\n#[cfg(test)]") {
                Some(at) => &source[..at],
                None => source.as_str(),
            };
            let code = blank_workers(&code_only(shipped));
            for (name, from, to) in reached(module, &code) {
                let body: String = code.chars().skip(from).take(to - from).collect();
                for waits in WAITS {
                    let mut at = 0;
                    while let Some(found) = body[at..].find(waits) {
                        let absolute = from + body[..at + found].chars().count();
                        faults.push(format!(
                            "{module}:{} in {name}() calls {waits}",
                            line_of(&code, absolute)
                        ));
                        at += found + 1;
                    }
                }
            }
        }
        assert!(
            faults.is_empty(),
            "the drawing thread is made to wait here. Move each to a worker and \
             report back through a channel, as `crate::offthread::Answer` and \
             `crate::dialog::Pending` do:\n{}",
            faults.join("\n")
        );
    }

    /// **Roadmap item 167's other half.** Every panel draws a frame quickly.
    ///
    /// The guard above reads names, and says so about what it cannot see: a
    /// drawing function calling into another module, which then reads a file,
    /// is invisible to it. This covers that gap from the other end. It drives
    /// each panel headlessly and fails if a frame takes a length of time that
    /// only a syscall can explain.
    ///
    /// # The threshold, and what it can and cannot catch
    ///
    /// These panels draw in 20 to 215 **microseconds** when nothing in them
    /// waits, measured on the machine this was written on. The limit is 25
    /// milliseconds, which is more than a hundred times the slowest of them, so
    /// the gap between a clean frame and a failing one is not a matter of how
    /// fast the machine is. A tighter limit would fail on a loaded build runner,
    /// and a timing test that fails for a reason nobody can act on is a timing
    /// test somebody deletes.
    ///
    /// **What it catches, and what it does not, measured rather than assumed.**
    /// It catches the gross stalls: a video render waited on, an audio-device
    /// enumeration on Windows, a file decrypted, a folder of twenty-five
    /// candidate vaults opened one after another, three passes of overwriting.
    /// Each of those is hundreds of milliseconds and fails this by a wide
    /// margin.
    ///
    /// It does **not** catch a single cheap syscall. A `stat` is micro-seconds.
    /// Spawning `df` was measured at 1.5 milliseconds on the machine this was
    /// written on, which is fifteen times a clean frame and still well inside
    /// the limit; on a slower disk it would fail this, and the limit cannot be
    /// lowered to depend on that. **The guard above is what catches those, by
    /// name**, and that is why there are two tests rather than one. Neither
    /// covers the other's blind spot and both are needed.
    ///
    /// The first frames are drawn and thrown away, because the first one builds
    /// the font atlas and is slow for a reason that is not a defect.
    #[test]
    fn every_panel_draws_a_frame_without_stopping() {
        /// A hundred times the slowest clean frame. See the note above.
        const LIMIT: std::time::Duration = std::time::Duration::from_millis(25);
        /// Frames drawn and discarded first, for the font atlas.
        const WARMUP: usize = 3;

        /// One panel: a name, and something that draws it into a context.
        type Panel = (&'static str, Box<dyn Fn(&egui::Context)>);

        let mut slow: Vec<String> = Vec::new();
        let panels: Vec<Panel> = vec![
            (
                "the setup tab",
                Box::new(|ctx: &egui::Context| {
                    let mut setup = crate::setup::Setup::new();
                    let motion = crate::prefs::Motion {
                        enabled: false,
                        icon: false,
                        system_reduced: true,
                    };
                    let _ = crate::headless_frame(ctx, Default::default(), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| setup.tab(ui, motion));
                    });
                }),
            ),
            (
                "the first-run tour",
                Box::new(|ctx: &egui::Context| {
                    let mut run = crate::firstrun::FirstRun::default();
                    // The machine card is the one that read the machine.
                    run.step = crate::firstrun::Step::Machine;
                    let mut prefs = crate::settings::Settings::default();
                    let mut security = crate::security::Security::default();
                    let _ = crate::headless_frame(ctx, Default::default(), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            run.panel(ui, &mut prefs, &mut security, (0, 0));
                        });
                    });
                }),
            ),
            (
                "the verify tab",
                Box::new(|ctx: &egui::Context| {
                    let mut verify = crate::verify::Verify::default();
                    let _ = crate::headless_frame(ctx, Default::default(), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| verify.tab(ui));
                    });
                }),
            ),
            (
                "the studio tab, shut",
                Box::new(|ctx: &egui::Context| {
                    let mut studio = crate::studio::Studio::default();
                    let _ = crate::headless_frame(ctx, Default::default(), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            studio.tab(ui, Default::default(), None, None);
                        });
                    });
                }),
            ),
            (
                "the recordings browser, shut",
                Box::new(|ctx: &egui::Context| {
                    let mut studio = crate::studio::Studio::default();
                    let _ = crate::headless_frame(ctx, Default::default(), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| studio.browser(ui));
                    });
                }),
            ),
        ];

        for (name, draw) in &panels {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            for _ in 0..WARMUP {
                draw(&ctx);
            }
            // The best of three, not the mean. A test runner that descheduled
            // the process for 300 milliseconds during one of them has said
            // nothing about the code, and a defect of this kind is in every
            // frame rather than one of them.
            let best = (0..3)
                .map(|_| {
                    let at = std::time::Instant::now();
                    draw(&ctx);
                    at.elapsed()
                })
                .min()
                .expect("three frames were drawn");
            if best > LIMIT {
                slow.push(format!("{name} took {best:?}"));
            }
        }
        assert!(
            slow.is_empty(),
            "a frame took long enough that something in it waited on the \
             operating system. Nothing on the drawing thread may read a file, \
             spawn a process or ask the hardware anything:\n{}",
            slow.join("\n")
        );
    }

    /// The channels on the draw path are drained without blocking.
    ///
    /// Counted rather than searched for, because `try_recv()` contains
    /// `recv()` and the first version of this reported the correct call as the
    /// fault.
    #[test]
    fn no_channel_on_the_draw_path_is_waited_on() {
        let mut faults = Vec::new();
        for (module, source) in &crate::sources() {
            let shipped = match source.find("\n#[cfg(test)]") {
                Some(at) => &source[..at],
                None => source.as_str(),
            };
            let code = blank_workers(&code_only(shipped));
            for (name, from, to) in reached(module, &code) {
                let body: String = code.chars().skip(from).take(to - from).collect();
                let blocking = body.matches("recv()").count() - body.matches("try_recv()").count();
                if blocking > 0 {
                    faults.push(format!("{module}: {name}()"));
                }
            }
        }
        assert!(
            faults.is_empty(),
            "a channel on the draw path is read with a blocking recv; use \
             try_recv:\n{}",
            faults.join("\n")
        );
    }
}
