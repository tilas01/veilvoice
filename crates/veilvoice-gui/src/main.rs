// SPDX-License-Identifier: GPL-3.0-or-later
//! Entry point for the desktop application: open a window, hand it to
//! [`veilvoice_gui::VeilVoiceApp`], and get out of the way.
//!
//! Everything of substance is in the library beside this file. That split is
//! the point: a binary crate cannot be unit tested, so the whole user interface
//! lives in `lib.rs` and its modules where tests can reach it, and this file
//! holds only what genuinely needs a window to exist.
//!
//! Three decisions are made here and nowhere else.
//!
//! **No console window on Windows, in release only.** A release build sets
//! `windows_subsystem = "windows"`, so double-clicking the application does not
//! flash up a terminal behind it. A debug build deliberately keeps the console,
//! because that is where panics and `eprintln!` go and losing them while
//! developing costs far more than the flash of a window is worth.
//!
//! **The icon is raw RGBA, not a PNG.** `assets/generate.py` writes
//! `icon-32.rgba` beside the PNG it generates from the same pixels, so the
//! application can set its own title-bar icon without linking an image decoder.
//! A decoder is a parser, a parser is an attack surface, and this one would
//! exist solely to draw a 32x32 square. The length is checked before use, and a
//! mismatch means the window simply opens without an icon rather than panicking
//! at startup.
//!
//! **The window has a minimum size, and it is about width.** Every tab is
//! inside one scroll area, so anything taller than the window can be reached by
//! scrolling to it and a short window loses nothing. Width is different: the
//! layout is monospace and column-based, and below roughly 720 across, columns
//! start overlapping rather than reflowing. So the floor is enforced here
//! rather than left to produce an unreadable window on somebody else's machine.
//!
//! It opens at 1100 by 720, which is large enough to read without resizing and
//! still fits a 1366 by 768 laptop with its taskbar. Anything bigger opens
//! partly off the bottom of a common screen, which looks like a broken
//! application rather than a generous one.
//!
//! # In plain words
//!
//! Opens the window and hands over to the rest of the application.
//!
//! Almost nothing happens here. Everything of substance lives beside it in code
//! that can be tested without a screen, and this file holds only the few things
//! that genuinely need a window to exist: its size, its icon, and making sure a
//! failure to open leaves a message behind.
// No console window on Windows for a release build; a debug build keeps it so
// panics and `eprintln!` stay visible while developing.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![forbid(unsafe_code)]

/// The window icon, as raw 32x32 RGBA produced by `assets/generate.py`.
///
/// Raw rather than PNG so the application needs no image decoder just to draw
/// its own title bar.
const ICON_RGBA: &[u8] = include_bytes!("../../../assets/icon-32.rgba");
const ICON_SIZE: u32 = 32;

/// What `--help` prints, on the platforms where printing works.
///
/// Kept beside the argument parsing rather than in a file somewhere, because a
/// second description of an interface drifts from the first: `tools/release/
/// manpage.py` turns this text into the installed manual page at package build
/// time, so the page and the program cannot disagree.
///
/// Declared only where it is read. A Windows release build has no console to
/// print to, so the reader below is Unix only, and a constant nothing reads is
/// dead code: the Windows job failed on it under `-D warnings` while every
/// other platform passed. The test that checks this text against the tab names
/// reads the file rather than the constant, so it still runs everywhere.
#[cfg(unix)]
const USAGE: &str = "\
veilvoice-gui - the VeilVoice desktop application

Usage:
  veilvoice-gui [--tab <NAME>] [--size <W>x<H>]

Options:
  --tab <NAME>  Open on a named tab rather than the last one used.
                Names are the ones the tabs carry: file, live, group,
                monitor, lock, verify, settings, install, about.
  --size <W>x<H>  Open at this size in logical pixels rather than the
                default 1100x720. Both are clamped to the window's
                minimum of 720x520.
  -h, --help    Print this message.
  -V, --version Print the version.

The command line is `veilvoice`, and `veilvoice gui` opens this window too.
Everything this window does, that command can do without one.
";

/// Answer `--help` and `--version` before a window is opened.
///
/// Unix only, and the restriction is the honest part rather than an oversight.
/// A release build on Windows declares `windows_subsystem = "windows"` and has
/// no console attached, so `println!` there writes to nothing: the program
/// would appear to do nothing at all, which is worse than the window it
/// currently opens. Where a console is guaranteed, this answers; where it is
/// not, behaviour is unchanged.
///
/// It exists because `veilvoice-gui --help` used to try to open a window and,
/// on a machine with no display, failed with a winit error naming
/// `WAYLAND_DISPLAY`. That is the reply to a reasonable question, and it is
/// also what `lintian` was pointing at with `no-manual-page`: a binary with no
/// help text has no page to derive.
#[cfg(unix)]
fn answered_without_a_window() -> bool {
    for arg in std::env::args().skip(1) {
        if arg == "-h" || arg == "--help" {
            print!("{USAGE}");
            return true;
        }
        if arg == "-V" || arg == "--version" {
            println!("veilvoice-gui {}", env!("CARGO_PKG_VERSION"));
            return true;
        }
    }
    false
}

#[cfg(not(unix))]
fn answered_without_a_window() -> bool {
    false
}

/// The size asked for by `--size <W>x<H>`, if one was and it parses.
///
/// A malformed value opens the default window rather than refusing to start.
/// This is a convenience for somebody who wants a bigger window and for the
/// screenshot harness; neither is worth a program that will not open, and a
/// window that is the wrong size says so by being the wrong size.
///
/// Clamped to the minimum the window enforces anyway, so `--size 1x1` gives a
/// usable window instead of one whose columns overlap.
fn requested_size() -> Option<[f32; 2]> {
    size_from(std::env::args().skip(1))
}

/// The parsing half, separated from the environment so it can be tested.
///
/// `main` cannot be called from a test and `std::env::args` cannot be set by
/// one, so a parser that reads the environment directly is a parser nothing
/// checks. This takes the arguments instead.
fn size_from<I: Iterator<Item = String>>(args: I) -> Option<[f32; 2]> {
    let mut args = args;
    while let Some(arg) = args.next() {
        let value = if let Some(rest) = arg.strip_prefix("--size=") {
            rest.to_string()
        } else if arg == "--size" {
            args.next()?
        } else {
            continue;
        };
        let (width, height) = value.split_once(['x', 'X'])?;
        let width: f32 = width.trim().parse().ok()?;
        let height: f32 = height.trim().parse().ok()?;
        if !width.is_finite() || !height.is_finite() {
            return None;
        }
        return Some([width.max(720.0), height.max(520.0)]);
    }
    None
}

fn main() -> eframe::Result<()> {
    if answered_without_a_window() {
        return Ok(());
    }

    // First, before anything that can fail.
    //
    // This binary has no console (`windows_subsystem = "windows"`) and the
    // workspace aborts on panic, so without this every failure produces
    // literally nothing: no message, no dialog, no log, just a window that
    // never appears. A user has nothing to report but "it crashed", which is
    // exactly the report that arrived against v0.1.10.
    veilvoice_gui::crashlog::install();

    let mut viewport = egui::ViewportBuilder::default()
        // Opens large enough to read without resizing, and still fits a
        // 1366x768 laptop with its taskbar. Bigger than this and the window
        // would open partly off the bottom of a common screen, which looks
        // like a broken application rather than a generous one.
        //
        // `--size` overrides it. That exists because the screenshot harness
        // needs a window bigger than the default -- at 1100x720 the longest
        // tab does not fit and the picture shows a panel cut off partway --
        // and the only way to get one used to be resizing the window after it
        // opened, through `SetWindowPos`, which is Windows and nothing else.
        // Asking for the size up front works on every platform, and it is a
        // reasonable thing for a person with a large display to want too.
        .with_inner_size(requested_size().unwrap_or([1100.0, 720.0]))
        // The floor. Everything below it is reachable by scrolling -- every
        // tab is inside one scroller now -- so the only thing this has to
        // protect is the horizontal layout, which is monospace and
        // column-based and starts overlapping rather than reflowing.
        .with_min_inner_size([720.0, 520.0])
        .with_title("VeilVoice");

    if ICON_RGBA.len() == (ICON_SIZE * ICON_SIZE * 4) as usize {
        viewport = viewport.with_icon(std::sync::Arc::new(egui::IconData {
            rgba: ICON_RGBA.to_vec(),
            width: ICON_SIZE,
            height: ICON_SIZE,
        }));
    }

    let result = eframe::run_native(
        "VeilVoice",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(veilvoice_gui::VeilVoiceApp::new(cc)))),
    );

    // `eframe` reports a failed start by returning, not by panicking, so the
    // hook above never sees it -- and the `Err` a `main` returns is printed to
    // a stderr that does not exist here. Creating the window is also the most
    // likely thing to fail on somebody else's machine: this renders through
    // glow, which is OpenGL, and a virtual machine, a remote desktop session or
    // hybrid graphics handing over the wrong adapter can all refuse a context.
    if let Err(error) = &result {
        veilvoice_gui::crashlog::record_startup_failure(&format!("{error}"));
    }
    result
}
#[cfg(test)]
mod tests {
    use super::size_from;

    fn parse(args: &[&str]) -> Option<[f32; 2]> {
        size_from(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn a_size_is_read_in_either_spelling() {
        assert_eq!(parse(&["--size", "1400x1000"]), Some([1400.0, 1000.0]));
        assert_eq!(parse(&["--size=1400x1000"]), Some([1400.0, 1000.0]));
        assert_eq!(parse(&["--size", "1400X1000"]), Some([1400.0, 1000.0]));
    }

    #[test]
    fn the_size_is_found_beside_other_arguments() {
        assert_eq!(
            parse(&["--tab", "verify", "--size", "1400x1000"]),
            Some([1400.0, 1000.0])
        );
    }

    /// The window enforces a floor of 720x520, so a smaller request is raised
    /// to it rather than producing a window whose columns overlap.
    #[test]
    fn a_size_below_the_minimum_is_raised_to_it() {
        assert_eq!(parse(&["--size", "1x1"]), Some([720.0, 520.0]));
        assert_eq!(parse(&["--size", "1400x10"]), Some([1400.0, 520.0]));
    }

    /// A value that makes no sense opens the default window. The alternative
    /// is a program that will not start over a convenience, and a window of
    /// the wrong size reports itself by being the wrong size.
    #[test]
    fn nonsense_falls_back_to_the_default() {
        for bad in ["", "1400", "widexhigh", "1400x", "x1000", "nanxnan", "infxinf"] {
            assert_eq!(parse(&["--size", bad]), None, "{bad:?} should not parse");
        }
        assert_eq!(parse(&["--tab", "file"]), None);
        assert_eq!(parse(&[]), None);
    }

    /// The help text has to name the option, because the manual page is
    /// generated from that text: an option the program accepts and the help
    /// does not mention is one nobody can find.
    #[test]
    fn the_help_text_mentions_the_size_option() {
        let source = include_str!("main.rs");
        let usage = source
            .split("const USAGE: &str = \"\\\n")
            .nth(1)
            .and_then(|rest| rest.split("\";").next())
            .expect("the usage text has to be findable");
        assert!(
            usage.contains("--size"),
            "`--help` does not mention --size, which the program accepts"
        );
    }
}
