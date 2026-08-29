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
const USAGE: &str = "\
veilvoice-gui - the VeilVoice desktop application

Usage:
  veilvoice-gui [--tab <NAME>]

Options:
  --tab <NAME>  Open on a named tab rather than the last one used.
                Names are the ones the tabs carry: file, live, group,
                monitor, lock, verify, settings, install, about.
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
        .with_inner_size([1100.0, 720.0])
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
