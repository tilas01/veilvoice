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
pub mod reset;
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

/// Motion resolved to "nothing moves", for tests that drive a panel.
///
/// Every panel that draws takes the resolved setting since roadmap item 169, so
/// the tests need one. Written once: five copies of the same three fields is
/// five places for the default to disagree with itself.
#[cfg(test)]
pub(crate) fn no_motion() -> crate::prefs::Motion {
    crate::prefs::Motion {
        enabled: false,
        icon: false,
        system_reduced: true,
    }
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
        ("reset.rs", include_str!("reset.rs").replace("\r\n", "\n")),
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
                    let motion = crate::no_motion();
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
                            run.panel(ui, &mut prefs, &mut security, (0, 0), crate::no_motion());
                        });
                    });
                }),
            ),
            (
                "the verify tab",
                Box::new(|ctx: &egui::Context| {
                    let mut verify = crate::verify::Verify::default();
                    let _ = crate::headless_frame(ctx, Default::default(), |ui| {
                        egui::CentralPanel::default()
                            .show(ui, |ui| verify.tab(ui, crate::no_motion()));
                    });
                }),
            ),
            (
                "the studio tab, shut",
                Box::new(|ctx: &egui::Context| {
                    let mut studio = crate::studio::Studio::default();
                    let _ = crate::headless_frame(ctx, Default::default(), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            studio.tab(ui, Default::default(), None, None, crate::no_motion());
                        });
                    });
                }),
            ),
            (
                "the recordings browser, shut",
                Box::new(|ctx: &egui::Context| {
                    let mut studio = crate::studio::Studio::default();
                    let _ = crate::headless_frame(ctx, Default::default(), |ui| {
                        egui::CentralPanel::default()
                            .show(ui, |ui| studio.browser(ui, crate::no_motion()));
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

#[cfg(test)]
mod row_tests {
    //! **Roadmap item 168.** Measuring the interface rather than reading it.
    //!
    //! # Why this measures the paint list
    //!
    //! egui knows every widget's rectangle: it keeps them in `WidgetRects` for
    //! its own hit testing. That is `pub(crate)`, so from outside the crate the
    //! only honest record of where a control ended up is what was painted.
    //!
    //! Every interactive control in this interface paints a background: the
    //! theme gives `inactive`, `hovered`, `active` and `open` a fill, a stroke
    //! and a corner radius, so a button, a text field, a dropdown and a
    //! selectable label each contribute a rectangle. A label does not, which is
    //! exactly the set this item is about. Reading the paint list therefore
    //! finds the controls and nothing else, without a list of them to keep up to
    //! date.
    //!
    //! # What a row is, without naming any
    //!
    //! Two controls are in the same row when their vertical extents overlap.
    //! That is the definition a reader uses, it needs no list of rows, and it
    //! keeps working when a panel is rearranged. A row of one is not a row and is
    //! ignored.
    //!
    //! Written this way for the reason F-210, F-212, F-216, F-220 and F-224 each
    //! gave from a different direction: a rule enforced over an enumerated list
    //! is a rule that stops holding the moment somebody adds to the interface
    //! without adding to the list.
    //!
    //! # Width is measured and deliberately not compared across a row
    //!
    //! Roadmap item 168 names four measurements, and three of them are checked
    //! here: height, vertical centre and left edge. Width is not, because the
    //! rule would be false. Measured: the Settings tab has a row of five options
    //! 67, 74.8, 82.6, 90.4 and 98.3 points wide, because they are labelled with
    //! words of different lengths, and every panel has controls of different
    //! widths starting at the left margin, because a button and a row of options
    //! have no reason to be the same size. Both are right.
    //!
    //! Width matters where two controls are two halves of one idea, which is
    //! `layout::LOCK_WIDTH` and roadmap item 160, and that is guarded where the
    //! number lives. A guard here would have to be suppressed in six panels on
    //! its first run, and a guard with six exceptions is a guard nobody trusts.

    use egui::Rect;

    /// How much two rectangles may differ and still be called equal, in points.
    ///
    /// Not zero, because a real interface has controls that are genuinely
    /// different shapes on purpose, and not generous either: half a point is
    /// below what anybody can see and far below the pixel this item is about.
    const SLACK: f32 = 0.5;

    /// Vertical overlap needed before two controls are in one row, in points.
    ///
    /// A few points rather than any overlap at all: two rows eight points apart
    /// with a one-point stroke leaking into each other must not read as one row.
    const OVERLAP: f32 = 4.0;

    /// A window this size, so a panel has somewhere to lay a row out.
    ///
    /// The default test context offers nearly ten thousand points of width,
    /// which is not a window anybody has and makes centring meaningless. The
    /// same reasoning as `layout`'s own tests, and the same number.
    fn input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        }
    }

    /// The same window, with the pointer at `at`.
    fn input_pointing(at: egui::Pos2) -> egui::RawInput {
        let mut raw = input();
        raw.events.push(egui::Event::PointerMoved(at));
        raw
    }

    /// The tallest a control is, in points, and the shortest.
    ///
    /// A card, a group frame and the panel's own fill are all painted
    /// rectangles too, and none of them is a control anybody expects to line up
    /// with its neighbour: the first-run tour's cards are 213 points tall. The
    /// band is what separates a control from the scenery it sits in, and it is
    /// generous at both ends: the smallest control here is a 14-point tick box
    /// and the largest a 27-point button.
    const CONTROL_HEIGHT: std::ops::RangeInclusive<f32> = 6.0..=40.0;

    /// Every control background one frame painted.
    ///
    /// Three kinds of rectangle are dropped, and each for its own reason.
    ///
    /// **The scenery**: the panel's own fill covers the window, and a card is
    /// taller than any control. [`CONTROL_HEIGHT`] and a width limit take both.
    ///
    /// **The parts of a control**: a checkbox paints its 14-point tick box
    /// *inside* its 96-point widget rectangle, and the two are one control, not
    /// two controls in a row. A rectangle wholly inside another is a part of it.
    /// Two controls side by side never contain one another, so nothing this rule
    /// drops is a thing this test is about.
    ///
    /// **The hairlines**: separators and strokes, which are under six points.
    fn controls(raw: egui::RawInput, draw: impl FnMut(&mut egui::Ui)) -> Vec<Rect> {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        // Twice: the first frame of anything that remembers a width has nothing
        // remembered, which `layout`'s tests document at length. The second is
        // the frame a reader sees.
        let mut painted = Vec::new();
        let mut draw = draw;
        for _ in 0..2 {
            let output = crate::headless_frame(&ctx, raw.clone(), &mut draw);
            painted.clear();
            for clipped in output.shapes {
                if let egui::Shape::Rect(rect) = &clipped.shape {
                    let rect = rect.rect;
                    if rect.width() > 800.0
                        || rect.width() < 6.0
                        || !CONTROL_HEIGHT.contains(&rect.height())
                    {
                        continue;
                    }
                    painted.push(rect);
                }
            }
        }
        // A part of a control rather than a control: see the note above. Done
        // after the loop because it is a question about the whole frame.
        let whole = painted.clone();
        painted.retain(|rect| {
            !whole.iter().any(|other| {
                other != rect && other.contains_rect(*rect) && other.area() > rect.area()
            })
        });
        painted
    }

    /// The painted controls gathered into rows, top to bottom.
    fn rows(mut painted: Vec<Rect>) -> Vec<Vec<Rect>> {
        painted.sort_by(|a, b| a.top().total_cmp(&b.top()));
        let mut rows: Vec<Vec<Rect>> = Vec::new();
        for rect in painted {
            match rows.last_mut() {
                // Compared against the row's first member rather than its last,
                // so a run of slightly taller controls cannot walk a row down
                // the panel one comparison at a time.
                Some(row)
                    if (row[0].bottom() - rect.top()).min(rect.bottom() - row[0].top())
                        > OVERLAP =>
                {
                    row.push(rect)
                }
                _ => rows.push(vec![rect]),
            }
        }
        for row in &mut rows {
            row.sort_by(|a, b| a.left().total_cmp(&b.left()));
        }
        rows
    }

    /// The panels this drives, named for the failure message.
    type Panel = (&'static str, Box<dyn Fn(&mut egui::Ui)>);

    fn panels() -> Vec<Panel> {
        vec![
            (
                "the verify tab",
                Box::new(|ui: &mut egui::Ui| {
                    let mut verify = crate::verify::Verify::default();
                    verify.tab(ui, crate::no_motion());
                }),
            ),
            (
                "the setup tab",
                Box::new(|ui: &mut egui::Ui| {
                    let mut setup = crate::setup::Setup::new();
                    setup.tab(ui, crate::no_motion());
                }),
            ),
            (
                "the security tab",
                Box::new(|ui: &mut egui::Ui| {
                    let mut security = crate::security::Security::default();
                    security.tab(ui, crate::no_motion());
                }),
            ),
            (
                "the lock screen",
                Box::new(|ui: &mut egui::Ui| {
                    let mut security = crate::security::Security::default();
                    security.unlock_screen(ui, crate::no_motion());
                }),
            ),
            (
                "the recordings browser",
                Box::new(|ui: &mut egui::Ui| {
                    let mut studio = crate::studio::Studio::default();
                    studio.browser(ui, crate::no_motion());
                }),
            ),
            (
                "the studio tab",
                Box::new(|ui: &mut egui::Ui| {
                    let mut studio = crate::studio::Studio::default();
                    studio.tab(ui, Default::default(), None, None, crate::no_motion());
                }),
            ),
            (
                "the settings tab",
                Box::new(|ui: &mut egui::Ui| {
                    let mut settings = crate::settings::Settings::default();
                    let ctx = ui.ctx().clone();
                    // A reset of its own, rather than one shared with another
                    // panel: this harness measures where controls land, and a
                    // reset part-way through a confirmation draws a different
                    // set of them.
                    let mut reset = crate::reset::Reset::default();
                    settings.tab(ui, &ctx, &mut reset);
                }),
            ),
            (
                "the group tab",
                Box::new(|ui: &mut egui::Ui| {
                    let mut group = crate::group::Group::default();
                    let mut settings = crate::settings::Settings::default();
                    group.tab(ui, &mut settings, crate::no_motion());
                }),
            ),
            (
                "the storage panel",
                Box::new(|ui: &mut egui::Ui| {
                    let mut storage = crate::storage::Storage::default();
                    let _ = crate::storage::panel(&mut storage, ui);
                }),
            ),
            (
                "the update check",
                Box::new(|ui: &mut egui::Ui| {
                    let mut updates = crate::updates::Updates::default();
                    updates.section(ui, "0.0.0", crate::no_motion());
                }),
            ),
            (
                "the first-run tour",
                Box::new(|ui: &mut egui::Ui| {
                    let mut run = crate::firstrun::FirstRun::default();
                    let mut prefs = crate::settings::Settings::default();
                    let mut security = crate::security::Security::default();
                    let _ = run.panel(ui, &mut prefs, &mut security, (0, 0), crate::no_motion());
                }),
            ),
        ]
    }

    /// Every row of controls agrees on height and on vertical centre.
    ///
    /// The two together are what "lined up" means: equal heights with different
    /// centres is a row where one control sits low, and equal centres with
    /// different heights is a row where one is squat. Roadmap item 168 asks for
    /// both, and for the left edge, which is the test below.
    #[test]
    fn every_row_of_controls_agrees_on_height_and_centre() {
        let mut wrong = Vec::new();
        let mut compared = 0;
        for (name, draw) in panels() {
            for row in rows(controls(input(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| draw(ui));
            })) {
                if row.len() < 2 {
                    continue;
                }
                compared += row.len();
                let tallest = row.iter().fold(0.0_f32, |a, r| a.max(r.height()));
                let shortest = row.iter().fold(f32::MAX, |a, r| a.min(r.height()));
                if tallest - shortest > SLACK {
                    wrong.push(format!(
                        "{name}: a row at y={:.0} holds controls {:.1} and {:.1} high",
                        row[0].top(),
                        shortest,
                        tallest
                    ));
                }
                let highest = row.iter().fold(f32::MAX, |a, r| a.min(r.center().y));
                let lowest = row.iter().fold(0.0_f32, |a, r| a.max(r.center().y));
                if lowest - highest > SLACK {
                    wrong.push(format!(
                        "{name}: a row at y={:.0} has centres {:.1} apart",
                        row[0].top(),
                        lowest - highest
                    ));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "these rows do not line up. Give the controls one height, the way \
             `layout::button_height` and `layout::LOCK_WIDTH` do for the lock \
             and unlock buttons:\n{}",
            wrong.join("\n")
        );
        // Not vacuous. Every rectangle here is found by a filter, and a filter
        // that is one line from finding nothing is how a guard comes to pass by
        // drawing no conclusions: the first version of this counted a
        // checkbox's tick box as a control beside its own label, and tightening
        // that could as easily have left nothing to compare.
        assert!(
            compared > 30,
            "only {compared} controls were in a row with anything, which is too \
             few for this to have checked the interface"
        );
    }

    /// Rows that nearly share a left edge share it.
    ///
    /// The left edge is the fourth of the four measurements roadmap item 168
    /// asks for, and it is the one where "close" is the whole defect. Two rows
    /// starting at the same x read as a column; two rows starting 40 points
    /// apart read as a heading and an indented list, which is deliberate. Two
    /// rows starting three points apart read as a mistake, because that is what
    /// it is: nobody indents by three points on purpose.
    ///
    /// So the rule is about near misses only, and says so rather than pretending
    /// to check alignment in general, which from a paint list it cannot: whether
    /// two rows belong to one column is a question about the panel's structure,
    /// and the answer is not in the shapes.
    #[test]
    fn rows_that_nearly_share_a_left_edge_share_it() {
        /// How far apart two left edges have to be before they are meant to
        /// differ rather than failing to match.
        const DELIBERATE: f32 = 12.0;

        let mut wrong = Vec::new();
        let mut pairs = 0;
        for (name, draw) in panels() {
            let rows = rows(controls(input(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| draw(ui));
            }));
            let lefts: Vec<(f32, f32)> = rows
                .iter()
                .map(|row| (row[0].left(), row[0].top()))
                .collect();
            for (index, (left, top)) in lefts.iter().enumerate() {
                for (other, other_top) in lefts.iter().skip(index + 1) {
                    pairs += 1;
                    let gap = (left - other).abs();
                    if gap > SLACK && gap < DELIBERATE {
                        wrong.push(format!(
                            "{name}: a row at y={top:.0} starts at x={left:.1} and one \
                             at y={other_top:.0} at x={other:.1}, {gap:.1} apart"
                        ));
                    }
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "these rows almost line up, which reads worse than not lining up at \
             all. Give them one left edge, with `layout::column` where a label \
             has to hold the space:\n{}",
            wrong.join("\n")
        );
        // Not vacuous, for the reason the test above gives: a rule that finds
        // nothing and a rule that looks at nothing read the same from here.
        assert!(
            pairs > 20,
            "only {pairs} pairs of rows were compared, which is too few for this \
             to have looked at the interface"
        );
    }

    /// Hovering a control changes its colour and not its shape.
    ///
    /// Driven with the pointer over the middle of each control in turn, and the
    /// whole panel re-measured each time: a control that grows on hover pushes
    /// everything after it along, so the assertion is about the panel and not
    /// only about the control under the pointer.
    ///
    /// This is the half of roadmap item 168 that a reader cannot check by
    /// looking, because a one-point shift is only visible as a flicker while the
    /// pointer moves, and it is the half a future theme change is most likely to
    /// undo: `egui::style::WidgetVisuals::expansion` is one field, it is zero in
    /// this version of egui and it has not always been, and setting it is how
    /// somebody would make hover "feel nicer".
    #[test]
    fn hovering_a_control_does_not_move_anything() {
        let mut moved = Vec::new();
        for (name, draw) in panels() {
            let draw = &draw;
            let at_rest = controls(input(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| draw(ui));
            });
            for target in &at_rest {
                let hovered = controls(input_pointing(target.center()), |ui| {
                    egui::CentralPanel::default().show(ui, |ui| draw(ui));
                });
                // Every rectangle that was there at rest must still be there,
                // in the same place. Compared as a set rather than pairwise,
                // because hover is allowed to paint *more*: a highlight behind
                // the control under the pointer is a colour change, and it
                // arrives as an extra shape that would offset every later
                // comparison in a pairwise walk.
                for rest in &at_rest {
                    let still_there = hovered.iter().any(|over| {
                        (rest.min - over.min).length() <= SLACK
                            && (rest.max - over.max).length() <= SLACK
                    });
                    if !still_there {
                        let nearest = hovered
                            .iter()
                            .min_by(|a, b| {
                                (a.min - rest.min)
                                    .length()
                                    .total_cmp(&(b.min - rest.min).length())
                            })
                            .copied()
                            .unwrap_or(*rest);
                        moved.push(format!(
                            "{name}: with the pointer at y={:.0}, the control at \
                             {:?} {:.1}x{:.1} became {:?} {:.1}x{:.1}",
                            target.top(),
                            rest.min,
                            rest.width(),
                            rest.height(),
                            nearest.min,
                            nearest.width(),
                            nearest.height()
                        ));
                        break;
                    }
                }
            }
        }
        assert!(
            moved.is_empty(),
            "hover moves something, so text shifts under the pointer. Hover \
             changes colour and nothing else: no weight, no padding, no \
             `expansion`:\n{}",
            moved.join("\n")
        );
    }
}
