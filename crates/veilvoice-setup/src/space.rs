// SPDX-License-Identifier: GPL-3.0-or-later
//! How much room is actually free where VeilVoice keeps things.
//!
//! # Why this is measured rather than assumed
//!
//! Decoy vaults are only worth making if there is somewhere to put them, and
//! how many to make is a question with a real answer on each machine. A number
//! picked here would be a guess dressed as advice: nine decoys is careless on a
//! nearly full laptop and timid on a four-terabyte disk.
//!
//! So this asks the operating system, and where the operating system will not
//! say, it returns [`None`] and the interface says the count is a suggestion
//! rather than a measurement. **Not measuring and not saying so** is the answer
//! this module exists to avoid.
//!
//! # No `unsafe`, and no new dependency
//!
//! The free-space call is `statvfs` on Unix and `GetDiskFreeSpaceEx` on
//! Windows, and reaching either from Rust means an `unsafe` block or a crate.
//! This workspace forbids the first everywhere and weighs the second against a
//! dependency graph people are invited to read, and neither is worth paying for
//! one integer.
//!
//! So it runs the tool the platform already ships. On Unix that is `df` with
//! `-Pk`, whose output format is specified by POSIX rather than left to the
//! implementation, which is the whole reason that flag is there: the columns
//! are fixed, the block size is 1024, and one line describes the filesystem
//! asked about.
//!
//! # In plain words
//!
//! Asks the system how much space is free where VeilVoice is putting files, so
//! it can suggest a sensible number of decoy vaults instead of inventing one.
//! If the system will not say, it says so rather than guessing.

use std::path::Path;
use std::process::Command;

/// Free space at `path`, in bytes, or `None` if the platform would not say.
///
/// `None` is a real answer and callers must treat it as one. It happens on a
/// system with no `df`, in a sandbox that refuses to start processes, and on
/// any platform this has no branch for. Every one of those is "we do not know",
/// which is different from zero and must not be shown as a measurement.
pub fn free_bytes(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        unix_free(path)
    }
    #[cfg(windows)]
    {
        windows_free(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        None
    }
}

/// `df -Pk`, parsed from the format POSIX specifies.
#[cfg(unix)]
fn unix_free(path: &Path) -> Option<u64> {
    // Absolute, and never a bare name. `Command` resolves a bare program
    // through the platform search order, and this project has a rule about
    // that: a file called `df` in the working directory must not become the
    // thing that runs. Both locations are tried because the BSDs put it in
    // `/bin` and some Linux layouts only have `/usr/bin`.
    let program = ["/bin/df", "/usr/bin/df"]
        .into_iter()
        .find(|p| Path::new(p).is_file())?;

    let output = Command::new(program).arg("-Pk").arg(path).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_df(&text)
}

/// The available column of a `df -Pk` report.
///
/// Split out so it can be tested against real output from each platform
/// without running anything, which is the only way to check a parser against
/// systems this is not running on.
///
/// POSIX fixes the columns as filesystem, 1024-blocks, used, available,
/// capacity, mount point. The **fourth** field is the one wanted, and it is
/// counted from the left rather than the right because a mount point may
/// contain spaces and a device name may not.
pub fn parse_df(text: &str) -> Option<u64> {
    // The last data line, not the second: `df` reporting on a path may print a
    // header and exactly one row, but some implementations wrap a long device
    // name onto its own line, leaving the numbers on the next.
    for line in text.lines().rev() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 6 {
            continue;
        }
        // A header line has no digits where the blocks go.
        let Ok(available) = fields[3].parse::<u64>() else {
            continue;
        };
        // `-k` means 1024-byte blocks. Saturating rather than wrapping: a
        // filesystem reporting an absurd block count should give a huge number
        // rather than a small one, because a small one would suggest there is
        // no room when there is.
        return Some(available.saturating_mul(1024));
    }
    None
}

/// `fsutil volume diskfree`, which is on every Windows since Vista.
#[cfg(windows)]
fn windows_free(path: &Path) -> Option<u64> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let program = [
        format!(r"{root}\System32\fsutil.exe"),
        format!(r"{root}\Sysnative\fsutil.exe"),
    ]
    .into_iter()
    .find(|p| Path::new(p).is_file())?;

    let output = Command::new(program)
        .arg("volume")
        .arg("diskfree")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_fsutil(&String::from_utf8_lossy(&output.stdout))
}

/// The free-bytes figure from `fsutil volume diskfree`.
///
/// # Why this reads the numbers rather than the labels
///
/// `fsutil` translates its output, so an English machine says "Total # of free
/// bytes" and a German one does not. Matching the label would work on the
/// machine it was written on and nowhere else, which is a worse failure than
/// not measuring at all because it looks like it worked.
///
/// The three figures it prints are free bytes, total bytes and available bytes,
/// and free is never larger than total. Taking the **smallest** of the numbers
/// found is therefore the conservative reading in any language: it can suggest
/// fewer decoys than there is room for, and never more.
#[cfg(windows)]
pub fn parse_fsutil(text: &str) -> Option<u64> {
    let mut smallest: Option<u64> = None;
    for line in text.lines() {
        for word in line.split(|c: char| !c.is_ascii_digit()) {
            if word.is_empty() {
                continue;
            }
            if let Ok(n) = word.parse::<u64>() {
                // Below a megabyte it is a version number or a column width
                // rather than a disk figure.
                if n >= 1024 * 1024 {
                    smallest = Some(smallest.map_or(n, |s: u64| s.min(n)));
                }
            }
        }
    }
    smallest
}

#[cfg(test)]
#[path = "space/tests.rs"]
mod tests;
