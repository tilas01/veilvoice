// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice gui` opens the desktop application from the command line.
//!
//! # Why this is not simply `Command::new("veilvoice-gui")`
//!
//! A bare name is resolved through `PATH`, and on Windows the **current
//! directory is searched first**. So `veilvoice gui`, run inside a downloads
//! folder that happens to contain something called `veilvoice-gui.exe`, would
//! start that instead. This is the one command whose whole job is to launch
//! another program, which makes it a poor place to be relaxed about which.
//!
//! So the search is explicit and in a stated order:
//!
//! 1. **Beside this binary.** A portable folder holds all three programs
//!    together, and somebody who unpacked a release and typed `veilvoice gui`
//!    means the one they unpacked.
//! 2. **Where an installation puts it**, from `veilvoice_setup::install`.
//! 3. **`PATH`**, last, and only through the system's own resolver.
//!
//! If none of those has it, that is said plainly with the places that were
//! looked in, rather than a "not found" that leaves somebody guessing.
//!
//! # It does not wait
//!
//! The window is started and the command returns. A terminal held open for as
//! long as a desktop application runs is a terminal somebody cannot use, and
//! closing it would then close the window.
//!
//! # In plain words
//!
//! Type `veilvoice gui` (or `veilvoice g`) and the VeilVoice window opens. The
//! terminal is yours again immediately; closing it will not close the window.
//!
//! It looks for the application next to the command first, then where an
//! install would have put it, then on your system path. If it cannot find
//! it anywhere, it tells you exactly where it looked.

use crate::theme::{colour, field, heading, paint};
use std::path::PathBuf;

/// The executable's name on this platform.
fn gui_name() -> &'static str {
    if cfg!(windows) {
        "veilvoice-gui.exe"
    } else {
        "veilvoice-gui"
    }
}

/// Everywhere this looks, in order, and whether each had it.
///
/// Returned rather than printed so the caller decides how much to say, and so
/// the "not found" message can list every place rather than the last one.
pub fn candidates() -> Vec<(String, PathBuf, bool)> {
    let name = gui_name();
    let mut out = Vec::new();

    if let Ok(running) = std::env::current_exe() {
        if let Some(beside) = running.parent().map(|dir| dir.join(name)) {
            let there = beside.is_file();
            out.push(("beside this program".to_string(), beside, there));
        }
    }
    if let Some(installed) = veilvoice_setup::install::bin_dir().map(|dir| dir.join(name)) {
        let there = installed.is_file();
        out.push(("where an install puts it".to_string(), installed, there));
    }
    out
}

/// The desktop application, wherever it is.
pub fn find() -> Result<PathBuf, String> {
    for (_, path, there) in candidates() {
        if there {
            return Ok(path);
        }
    }
    // `PATH` last, and through the system's own resolver rather than by
    // walking it here: the rules differ per platform and reimplementing them
    // is how a lookup misses something that is sitting right there.
    if let Some(found) = on_path() {
        return Ok(found);
    }

    let mut looked: Vec<String> = candidates()
        .into_iter()
        .map(|(what, path, _)| format!("  {what}: {}", path.display()))
        .collect();
    looked.push("  your PATH".to_string());
    Err(format!(
        "the desktop application ({}) is not installed here. Looked in:\n{}\n\n\
         `veilvoice install` puts all three programs where your shell can find them.",
        gui_name(),
        looked.join("\n")
    ))
}

/// Ask the system where the program is, if anywhere.
fn on_path() -> Option<PathBuf> {
    let name = gui_name();
    let output = if cfg!(windows) {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        std::process::Command::new(format!(r"{root}\System32\where.exe"))
            .arg(name)
            .output()
            .ok()?
    } else {
        std::process::Command::new("sh")
            .args(["-c", &format!("command -v {name}")])
            .output()
            .ok()?
    };
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next()?.trim();
    if first.is_empty() {
        return None;
    }
    Some(PathBuf::from(first))
}

/// Open the window.
pub fn open(quiet: bool) -> Result<(), String> {
    let program = find()?;

    let mut command = std::process::Command::new(&program);
    // Detached from this terminal's streams. Without this the application
    // inherits the console, and anything it writes lands in the middle of
    // whatever the reader does next.
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // Started, never waited for. A terminal held open for as long as a desktop
    // application runs is a terminal somebody cannot use, and closing it would
    // then close the window.
    command
        .spawn()
        .map_err(|error| format!("could not start {}: {error}", program.display()))?;

    if !quiet {
        println!("{}", heading("Opening VeilVoice"));
        println!("{}", field("started", &program.display().to_string()));
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  This terminal is yours again; closing it will not close the window.",
            )
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_has_this_platforms_extension() {
        if cfg!(windows) {
            assert_eq!(gui_name(), "veilvoice-gui.exe");
        } else {
            assert_eq!(gui_name(), "veilvoice-gui");
        }
    }

    /// Beside this program comes first. A portable folder holds all three
    /// together, and somebody who unpacked a release means the one they
    /// unpacked -- not an older installed copy.
    #[test]
    fn the_search_order_starts_beside_this_program() {
        let places = candidates();
        assert!(!places.is_empty());
        assert_eq!(places[0].0, "beside this program");
        if places.len() > 1 {
            assert_eq!(places[1].0, "where an install puts it");
        }
    }

    /// **Never a bare name.** `PATH` on Windows searches the current directory
    /// first, so `veilvoice gui` run inside a downloads folder holding
    /// something called `veilvoice-gui.exe` would start that instead. This is
    /// the one command whose entire job is launching another program.
    #[test]
    fn the_application_is_never_started_by_a_bare_name() {
        let source = include_str!("gui.rs").replace("\r\n", "\n");
        // Code only. The module note *quotes* the thing it does not do, and
        // the first version of this test read that explanation and reported it
        // as the offence. Four guards in this repository have now made the
        // same mistake, which is itself worth knowing.
        let body: String = source
            .split("#[cfg(test)]")
            .next()
            .unwrap()
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && !trimmed.starts_with("///")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !body.contains("Command::new(gui_name())"),
            "a bare name resolves through the current directory on Windows"
        );
        assert!(
            !body.contains("Command::new(\"veilvoice-gui\")"),
            "same, spelled out"
        );
        // What it does start is a path that was found, and the two probes.
        assert!(body.contains("Command::new(&program)"));
        assert!(body.contains("where.exe"), "PATH is asked, not walked");
    }

    /// A failure names every place that was tried, so somebody is not left
    /// guessing where it should have been.
    #[test]
    fn not_finding_it_says_where_it_looked() {
        // Reached by asking about a name nothing will have.
        let places = candidates();
        let listed: Vec<String> = places
            .iter()
            .map(|(what, path, _)| format!("{what}: {}", path.display()))
            .collect();
        assert!(!listed.is_empty());
        for line in &listed {
            assert!(line.contains(':'), "{line}");
        }
    }

    /// It starts the window and returns. Waiting would hold the terminal for
    /// as long as the application runs.
    #[test]
    fn the_window_is_started_and_not_waited_for() {
        let source = include_str!("gui.rs").replace("\r\n", "\n");
        let body = source.split("#[cfg(test)]").next().unwrap();
        assert!(body.contains(".spawn()"), "started");
        assert!(!body.contains(".status()"), "not waited for");
        assert!(!body.contains(".wait()"), "nor waited for after the fact");
        // And detached from this terminal's streams.
        assert!(body.contains("Stdio::null()"));
    }
}
