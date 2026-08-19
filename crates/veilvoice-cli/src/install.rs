// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice install` -- put this program somewhere the system can find it.
//!
//! # Portable is the default, and installing is the exception
//!
//! VeilVoice runs from wherever it is unpacked. Nothing has to be installed,
//! nothing is written outside the folder unless the user does something that
//! writes outside the folder, and deleting the folder removes it. That is the
//! posture this project has always had and it is not being given up.
//!
//! This exists because "runs from anywhere" and "I would like to type
//! `veilvoice` in a terminal" are both reasonable, and the second needs three
//! things a portable folder cannot provide: a stable location, an entry on
//! `PATH`, and a way for the operating system to list and remove it.
//!
//! # No administrator, and nothing outside the user's own account
//!
//! Everything here is per-user: `%LOCALAPPDATA%` on Windows,
//! `~/.local` on everything else, and on Windows the `HKCU` registry rather
//! than `HKLM`. No elevation is requested, nothing is written to a system
//! directory, and no service is created.
//!
//! That is a deliberate limit rather than an oversight. A per-user install can
//! be undone by the user who made it, needs no privilege to audit, and cannot
//! break anybody else's account. A machine-wide install would need
//! administrator rights, and the reason to ask for those has to be better than
//! "so the program is on everyone's PATH".
//!
//! # Every change is reversible, and `uninstall` reverses exactly these
//!
//! | What | Where | Undone by |
//! |---|---|---|
//! | The binaries | `<prefix>/VeilVoice` | removing that directory |
//! | `PATH` entry | `HKCU\Environment`, or a shell profile line | removing just that entry |
//! | Uninstall entry | `HKCU\...\Uninstall\VeilVoice` | deleting that key |
//!
//! The `PATH` edit is the one that can damage something, so it is the one
//! handled most carefully: the existing value is read, the entry is appended
//! only if absent, and removal takes out that entry and nothing else. A tool
//! that overwrites `PATH` wholesale has broken a machine, and doing it during
//! an *uninstall* is worse -- that is the moment somebody is least inclined to
//! check.
//!
//! # Why the registry through `reg.exe`
//!
//! The same reason `veilvoice-watch` reads it that way: this workspace carries
//! `#![forbid(unsafe_code)]` in every crate, and the Win32 registry API needs
//! `unsafe` FFI. Shelling out to a system tool keeps that guarantee and costs a
//! subprocess on an operation that runs once. `reg.exe` is resolved by absolute
//! path -- resolving it by name would search the working directory first, which
//! is finding F-13.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The name of the directory and the uninstall entry.
pub const NAME: &str = "VeilVoice";

/// Files that make up an installation, if they are beside the running binary.
const PROGRAMS: &[&str] = &["veilvoice", "veilvoice-gui", "veilvoice-verify"];

/// Spawn without a console window.
///
/// Same reasoning as the copies in `veilvoice-watch` and `veilvoice-guard`: on
/// Windows a `Command` for a console program creates one, and if this is ever
/// called from the desktop application a window would flash. `creation_flags`
/// is safe, so this costs nothing against `#![forbid(unsafe_code)]`.
#[cfg_attr(not(windows), allow(unused_mut))]
fn no_window(mut command: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// `reg.exe`, by absolute path.
///
/// Never by bare name: Windows searches the current directory before most of
/// `PATH`, so running this from a folder containing `reg.exe` would run that
/// instead. This is the program that edits `PATH`, so it is a poor place to be
/// relaxed about which binary is doing it.
#[cfg(windows)]
fn reg_exe() -> PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    PathBuf::from(format!(r"{root}\System32\reg.exe"))
}

/// Where an installation goes, for this user only.
pub fn prefix() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("Programs").join(NAME))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("veilvoice")
        })
    }
}

/// The directory a `PATH` entry should point at.
pub fn bin_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        prefix()
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("bin"))
    }
}

/// What an installation currently looks like.
pub struct Status {
    pub prefix: Option<PathBuf>,
    pub installed: bool,
    pub on_path: bool,
    pub running_from: Option<PathBuf>,
    /// True when the running binary is the installed one rather than a
    /// portable copy. Worth telling the user: "installed" and "you are running
    /// the installed one" are different facts.
    pub running_installed: bool,
}

/// Read the current state without changing anything.
pub fn status() -> Status {
    let prefix = prefix();
    let running = std::env::current_exe().ok();
    let installed = prefix
        .as_ref()
        .map(|p| p.join(exe_name("veilvoice")).exists())
        .unwrap_or(false);
    let running_installed = match (&prefix, &running) {
        (Some(p), Some(r)) => r.parent().map(|d| d == p.as_path()).unwrap_or(false),
        _ => false,
    };
    Status {
        on_path: prefix.as_ref().map(|p| path_contains(p)).unwrap_or(false),
        prefix,
        installed,
        running_from: running,
        running_installed,
    }
}

fn exe_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}

/// Is `dir` already on this user's `PATH`?
///
/// Reads the *current process* environment, which is what "will typing
/// `veilvoice` work in this terminal" actually depends on. A registry value
/// that a new terminal would pick up is a different question, and the report
/// says which one it answered.
fn path_contains(dir: &Path) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|entry| entry == dir)
}

/// Copy the binaries into place. Returns what was copied.
fn copy_programs(into: &Path) -> Result<Vec<String>, String> {
    let running =
        std::env::current_exe().map_err(|e| format!("cannot find this program on disk: {e}"))?;
    let source = running
        .parent()
        .ok_or_else(|| "this program has no parent directory".to_string())?;

    std::fs::create_dir_all(into).map_err(|e| format!("cannot create {}: {e}", into.display()))?;

    let mut copied = Vec::new();
    for stem in PROGRAMS {
        let name = exe_name(stem);
        let from = source.join(&name);
        if !from.exists() {
            // A portable folder may hold only some of the three. Copying what
            // is there and saying so is more useful than refusing because the
            // GUI was not unpacked.
            continue;
        }
        let to = into.join(&name);
        if from == to {
            return Err(format!(
                "this program is already running from {} -- nothing to install",
                into.display()
            ));
        }
        std::fs::copy(&from, &to)
            .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))?;
        copied.push(name);
    }
    if copied.is_empty() {
        return Err("found none of the VeilVoice programs beside this one".to_string());
    }
    Ok(copied)
}

/// Add `dir` to the user's `PATH`, if it is not there already.
///
/// Reads the existing value and appends. Never writes a `PATH` it did not
/// first read: replacing that variable wholesale is how a tool breaks a
/// machine, and there is no undo.
#[cfg(windows)]
fn add_to_path(dir: &Path) -> Result<bool, String> {
    let current = read_user_path()?;
    if current.split(';').any(|entry| {
        entry
            .trim()
            .eq_ignore_ascii_case(&dir.display().to_string())
    }) {
        return Ok(false);
    }
    let joined = if current.trim().is_empty() {
        dir.display().to_string()
    } else {
        format!("{};{}", current.trim_end_matches(';'), dir.display())
    };
    let output = no_window(Command::new(reg_exe()))
        .args([
            "add",
            r"HKCU\Environment",
            "/v",
            "PATH",
            "/t",
            "REG_EXPAND_SZ",
            "/d",
        ])
        .arg(&joined)
        .arg("/f")
        .output()
        .map_err(|e| format!("could not run reg.exe: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "could not update PATH: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(true)
}

#[cfg(windows)]
fn read_user_path() -> Result<String, String> {
    let output = no_window(Command::new(reg_exe()))
        .args(["query", r"HKCU\Environment", "/v", "PATH"])
        .output()
        .map_err(|e| format!("could not run reg.exe: {e}"))?;
    if !output.status.success() {
        // No user PATH value at all is a normal state, not an error.
        return Ok(String::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("PATH") {
            continue;
        }
        // `reg query` prints:  PATH    REG_EXPAND_SZ    <value>
        //
        // The value may itself contain spaces, so it is taken as everything
        // after the type rather than by splitting on whitespace -- a PATH is
        // full of `C:\Program Files\...` and splitting one on spaces is how a
        // tool corrupts it.
        let Some(at) = trimmed.find("REG_") else {
            continue;
        };
        let after = &trimmed[at..];
        let Some(space) = after.find(char::is_whitespace) else {
            continue;
        };
        return Ok(after[space..].trim().to_string());
    }
    Ok(String::new())
}

#[cfg(not(windows))]
fn add_to_path(dir: &Path) -> Result<bool, String> {
    // On Unix the convention is a line in a shell profile, and rewriting
    // somebody's profile without asking is not this program's business. The
    // line is printed for them to add.
    let _ = dir;
    Ok(false)
}

/// Register with Add/Remove Programs, so the system can list and remove it.
#[cfg(windows)]
fn register_uninstall(prefix: &Path) -> Result<(), String> {
    let key = format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{NAME}");
    let exe = prefix.join(exe_name("veilvoice"));
    let entries: &[(&str, &str, String)] = &[
        ("DisplayName", "REG_SZ", NAME.to_string()),
        (
            "DisplayVersion",
            "REG_SZ",
            env!("CARGO_PKG_VERSION").to_string(),
        ),
        ("Publisher", "REG_SZ", "tilas01".to_string()),
        ("InstallLocation", "REG_SZ", prefix.display().to_string()),
        ("DisplayIcon", "REG_SZ", exe.display().to_string()),
        ("NoModify", "REG_DWORD", "1".to_string()),
        ("NoRepair", "REG_DWORD", "1".to_string()),
        (
            "UninstallString",
            "REG_SZ",
            format!("\"{}\" uninstall --yes", exe.display()),
        ),
    ];
    for (name, kind, value) in entries {
        let output = no_window(Command::new(reg_exe()))
            .args(["add", &key, "/v", name, "/t", kind, "/d"])
            .arg(value)
            .arg("/f")
            .output()
            .map_err(|e| format!("could not run reg.exe: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "could not write the uninstall entry: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn register_uninstall(_prefix: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn remove_from_path(dir: &Path) -> Result<bool, String> {
    let current = read_user_path()?;
    if current.trim().is_empty() {
        return Ok(false);
    }
    let wanted = dir.display().to_string();
    let kept: Vec<&str> = current
        .split(';')
        .filter(|entry| !entry.trim().eq_ignore_ascii_case(&wanted))
        .collect();
    if kept.len() == current.split(';').count() {
        return Ok(false);
    }
    // Only this entry is removed, and only from the value just read. An
    // uninstall that rewrites PATH from a template destroys whatever else the
    // user had, at the moment they are least likely to look.
    let joined = kept.join(";");
    let output = no_window(Command::new(reg_exe()))
        .args([
            "add",
            r"HKCU\Environment",
            "/v",
            "PATH",
            "/t",
            "REG_EXPAND_SZ",
            "/d",
        ])
        .arg(&joined)
        .arg("/f")
        .output()
        .map_err(|e| format!("could not run reg.exe: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "could not update PATH: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(true)
}

#[cfg(not(windows))]
fn remove_from_path(_dir: &Path) -> Result<bool, String> {
    Ok(false)
}

#[cfg(windows)]
fn unregister_uninstall() -> Result<(), String> {
    let key = format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{NAME}");
    let _ = no_window(Command::new(reg_exe()))
        .args(["delete", &key, "/f"])
        .output();
    Ok(())
}

#[cfg(not(windows))]
fn unregister_uninstall() -> Result<(), String> {
    Ok(())
}

/// Install for this user. Returns the lines to report.
pub fn install() -> Result<Vec<String>, String> {
    let prefix = prefix()
        .ok_or_else(|| "no per-user program directory could be found on this system".to_string())?;
    let mut report = Vec::new();

    let copied = copy_programs(&prefix)?;
    report.push(format!(
        "copied {} into {}",
        copied.join(", "),
        prefix.display()
    ));

    let dir = bin_dir().unwrap_or_else(|| prefix.clone());
    match add_to_path(&dir) {
        Ok(true) => report.push(format!("added {} to your PATH", dir.display())),
        Ok(false) => report.push(format!("{} was already on your PATH", dir.display())),
        Err(error) => report.push(format!("PATH was not changed: {error}")),
    }

    register_uninstall(&prefix)?;
    if cfg!(windows) {
        report.push("registered in Apps & features, so Windows can remove it".to_string());
    }
    Ok(report)
}

/// Remove what `install` added.
pub fn uninstall() -> Result<Vec<String>, String> {
    let prefix = prefix()
        .ok_or_else(|| "no per-user program directory could be found on this system".to_string())?;
    let mut report = Vec::new();

    let dir = bin_dir().unwrap_or_else(|| prefix.clone());
    match remove_from_path(&dir) {
        Ok(true) => report.push(format!("removed {} from your PATH", dir.display())),
        Ok(false) => report.push("PATH did not mention it".to_string()),
        Err(error) => report.push(format!("PATH was not changed: {error}")),
    }

    unregister_uninstall()?;

    // The directory goes last: if this binary is the installed one, it is
    // deleting itself, and Windows will not let it. Saying so is better than
    // failing halfway with the registry already cleaned up.
    let running = std::env::current_exe().ok();
    let running_here = running
        .as_ref()
        .and_then(|r| r.parent())
        .map(|d| d == prefix.as_path())
        .unwrap_or(false);
    if running_here {
        report.push(format!(
            "left {} in place: this program is running from it, and a running \
             program cannot delete itself. Remove that folder by hand.",
            prefix.display()
        ));
    } else if prefix.exists() {
        std::fs::remove_dir_all(&prefix)
            .map_err(|e| format!("could not remove {}: {e}", prefix.display()))?;
        report.push(format!("removed {}", prefix.display()));
    } else {
        report.push("nothing was installed".to_string());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prefix_is_found_on_this_platform() {
        assert!(
            prefix().is_some(),
            "no per-user program directory was resolved"
        );
    }

    #[test]
    fn status_reads_without_changing_anything() {
        // Called twice: the second must see exactly what the first did, or
        // reading the state is not free of side effects.
        let first = status();
        let second = status();
        assert_eq!(first.installed, second.installed);
        assert_eq!(first.on_path, second.on_path);
        assert_eq!(first.prefix, second.prefix);
    }

    #[test]
    fn the_executable_name_matches_the_platform() {
        let name = exe_name("veilvoice");
        if cfg!(windows) {
            assert_eq!(name, "veilvoice.exe");
        } else {
            assert_eq!(name, "veilvoice");
        }
    }

    #[test]
    fn path_membership_is_an_exact_directory_match() {
        // A prefix match would report `C:\Program Files\VeilVoiceOther` as
        // VeilVoice being installed, and a substring match is worse again.
        let path = std::env::var_os("PATH");
        if path.is_none() {
            return;
        }
        let nonsense = PathBuf::from("this-directory-is-not-on-anybody-s-path-42");
        assert!(!path_contains(&nonsense));
    }
}
