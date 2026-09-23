// SPDX-License-Identifier: GPL-3.0-or-later
//! Put this program somewhere the system can find it.
//!
//! Reached as `veilvoice install` on the command line and as the setup tab
//! in the desktop application. Both call the functions below; neither has a
//! copy of them. See the crate documentation for why that mattered enough to
//! move this file out of the binary it used to live in.
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
//!
//! # In plain words
//!
//! Copies VeilVoice somewhere your system can find it, and adds that place to your
//! path so typing `veilvoice` works.
//!
//! It installs for you alone and needs no administrator rights. It also registers
//! with the system's own list of installed programs, so removing it works the way
//! removing anything else does.
//!
//! Running VeilVoice straight out of a folder is a perfectly good way to use it,
//! and the setup screen says so rather than treating portable as something
//! missing.

use std::path::{Path, PathBuf};

/// The name of the directory and the uninstall entry.
///
/// Windows-only: elsewhere the prefix follows the XDG convention and is
/// lower-case, so this constant has no reader.
#[cfg(windows)]
pub const NAME: &str = "VeilVoice";

/// Files that make up an installation, if they are beside the running binary.
///
/// The two the release publishes. A third, `veilvoice-verify`, existed until
/// 0.1.18 and is now inside both, so looking for it here would only ever find
/// a stale copy left behind by an older install.
const PROGRAMS: &[&str] = &["veilvoice", "veilvoice-gui"];

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

/// The directory VeilVoice owns on this machine, for this user only.
///
/// **Not where the binaries go.** That is [`bin_dir`], and the two are the same
/// directory on Windows and different ones everywhere else. This one is
/// VeilVoice's own: nothing else writes to it, it holds whatever VeilVoice
/// keeps beside itself such as `copies-seen.txt`, and it is the one directory
/// [`uninstall`] may remove whole.
///
/// The distinction is F-214. These two were used interchangeably, and on
/// Unix that meant the binaries were copied into one directory while a
/// different one was added to `PATH`.
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

/// Where the binaries go, and therefore the directory `PATH` must contain.
///
/// On Windows this is [`prefix`]: a per-application directory under
/// `%LOCALAPPDATA%\\Programs` is the platform's own convention and it is added
/// to `PATH` by name.
///
/// Everywhere else it is `~/.local/bin`, which is where the XDG layout and
/// systemd's file hierarchy both put a user's own executables, and which modern
/// distributions already have on `PATH`. It is **not** VeilVoice's directory:
/// other programs put their binaries there too, which is why [`uninstall`]
/// removes files from it by name and never removes the directory.
///
/// This used to be answered one way and acted on another. `install` copied into
/// [`prefix`], `~/.local/share/veilvoice`, and then added this directory to
/// `PATH`, so the binaries were never in the directory the user was told about
/// and typing `veilvoice` could not work: the entire point of installing. That
/// is F-214.
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
///
/// Read by [`status`], which changes nothing. Every field is a separate fact
/// on purpose: "something is installed" and "you are running the installed
/// copy" are different, and a front end that conflates them tells somebody
/// editing a portable folder that their changes took effect.
pub struct Status {
    /// Where the binaries go, or are. `None` when this system offers no
    /// per-user program directory at all.
    ///
    /// This is [`bin_dir`], and every other field here answers about that same
    /// directory. It reported [`prefix`] until F-214, while `install`
    /// copied into one directory and added another to `PATH`, so all three
    /// facts below were about three different places.
    pub prefix: Option<PathBuf>,
    /// A VeilVoice command line exists under [`Status::prefix`].
    pub installed: bool,
    /// The install directory is on **this process's** `PATH` -- which is what
    /// "will typing `veilvoice` work in the terminal I already have open"
    /// actually depends on.
    pub on_path: bool,
    /// The binary that is running right now, as the operating system reports
    /// it.
    pub running_from: Option<PathBuf>,
    /// True when the running binary is the installed one rather than a
    /// portable copy. Worth telling the user: "installed" and "you are running
    /// the installed one" are different facts.
    pub running_installed: bool,
}

/// Read the current state without changing anything.
pub fn status() -> Status {
    let dir = bin_dir();
    let running = std::env::current_exe().ok();
    let installed = dir
        .as_ref()
        .map(|d| d.join(exe_name("veilvoice")).exists())
        .unwrap_or(false);
    let running_installed = match (&dir, &running) {
        (Some(d), Some(r)) => r
            .parent()
            .map(|parent| parent == d.as_path())
            .unwrap_or(false),
        _ => false,
    };
    Status {
        on_path: dir.as_ref().map(|d| path_contains(d)).unwrap_or(false),
        prefix: dir,
        installed,
        running_from: running,
        running_installed,
    }
}

/// A program's file name on this platform.
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
            // A portable folder may hold only one of the two: several
            // platforms publish a command-line archive with no window in it.
            // Copying what is there and saying so is more useful than refusing
            // because the GUI was not unpacked.
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
    let wanted = dir.display().to_string();
    let joined = match read_user_path()? {
        UserPath::Absent => wanted.clone(),
        UserPath::Value(current) => {
            if current
                .split(';')
                .any(|entry| entry.trim().eq_ignore_ascii_case(&wanted))
            {
                return Ok(false);
            }
            if current.trim().is_empty() {
                wanted.clone()
            } else {
                format!("{};{}", current.trim_end_matches(';'), wanted)
            }
        }
    };
    let output = crate::command(reg_exe())
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
enum UserPath {
    /// Read successfully. This is the value to append to.
    Value(String),
    /// `reg` said the value does not exist. Creating it is safe.
    Absent,
}

/// Read this user's `PATH`, distinguishing "not set" from "could not tell".
///
/// The first version returned an empty string for both, and the caller treats
/// empty as "there is no PATH yet, write a fresh one" -- so a query that failed
/// for any reason would have replaced the user's entire `PATH` with a single
/// entry. The comment at the top of this file already said that was the thing
/// to avoid; the code did not implement it.
///
/// `reg query` exits non-zero for a missing value *and* for every other
/// failure, so the two are told apart by what it says. Anything not
/// recognisably "value does not exist" is an error, and an error refuses the
/// write rather than guessing.
#[cfg(windows)]
fn read_user_path() -> Result<UserPath, String> {
    let output = crate::command(reg_exe())
        .args(["query", r"HKCU\Environment", "/v", "PATH"])
        .output()
        .map_err(|e| format!("could not run reg.exe: {e}"))?;
    if !output.status.success() {
        let complaint = String::from_utf8_lossy(&output.stderr).to_lowercase();
        if complaint.contains("unable to find") {
            // Genuinely no value. Creating one is safe.
            return Ok(UserPath::Absent);
        }
        return Err(format!(
            "could not read your PATH ({}). Refusing to change it: writing a \
             PATH that could not first be read would replace whatever is there.",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
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
        return Ok(UserPath::Value(after[space..].trim().to_string()));
    }
    // `reg` succeeded and printed something this cannot parse. That is not
    // "there is no PATH" -- it is "I do not understand the answer", and the
    // difference is the whole point of this function.
    Err("could not parse the PATH value reg.exe printed. Refusing to change it.".to_string())
}

/// The Unix half: report that nothing was written, because nothing was.
///
/// Answering `Ok(false)` rather than doing it is the decision, and the body
/// says why. The caller prints the line for the person to add themselves.
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
        let output = crate::command(reg_exe())
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

/// The Unix half: nothing to register. There is no Add/Remove Programs here,
/// and `veilvoice uninstall` is the reversal on these platforms.
#[cfg(not(windows))]
fn register_uninstall(_prefix: &Path) -> Result<(), String> {
    Ok(())
}

/// Take this directory back out of the user's `PATH`, leaving the rest of it
/// exactly as it was.
///
/// The same care as [`add_to_path`], for the same reason: the variable is read
/// and rewritten rather than replaced, and a `PATH` that never had this
/// directory in it is left untouched rather than rewritten to itself.
#[cfg(windows)]
fn remove_from_path(dir: &Path) -> Result<bool, String> {
    let current = match read_user_path()? {
        // Nothing to remove from, and nothing to write.
        UserPath::Absent => return Ok(false),
        UserPath::Value(value) => value,
    };
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
    let output = crate::command(reg_exe())
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

/// The Unix half: nothing was written to a profile, so nothing is taken out.
#[cfg(not(windows))]
fn remove_from_path(_dir: &Path) -> Result<bool, String> {
    Ok(false)
}

/// Take the Add/Remove Programs entry away again.
///
/// A failure is ignored on purpose: the entry not being there is the outcome
/// wanted, and refusing to finish an uninstall because the registry key was
/// already gone would leave the person with a half-removed program.
#[cfg(windows)]
fn unregister_uninstall() -> Result<(), String> {
    let key = format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{NAME}");
    let _ = crate::command(reg_exe())
        .args(["delete", &key, "/f"])
        .output();
    Ok(())
}

/// The Unix half: nothing was registered, so nothing is unregistered.
#[cfg(not(windows))]
fn unregister_uninstall() -> Result<(), String> {
    Ok(())
}

/// Install for this user. Returns the lines to report.
pub fn install() -> Result<Vec<String>, String> {
    // One directory, and it is the one that goes on `PATH`. Copying into a
    // second and adding this one is F-214, and it made installing a
    // no-op on every platform but Windows.
    let dir = bin_dir()
        .ok_or_else(|| "no per-user program directory could be found on this system".to_string())?;
    let mut report = Vec::new();

    let copied = copy_programs(&dir)?;
    report.push(format!(
        "copied {} into {}",
        copied.join(", "),
        dir.display()
    ));

    match add_to_path(&dir) {
        Ok(true) => report.push(format!("added {} to your PATH", dir.display())),
        Ok(false) => report.push(format!("{} was already on your PATH", dir.display())),
        Err(error) => report.push(format!("PATH was not changed: {error}")),
    }

    register_uninstall(&dir)?;
    if cfg!(windows) {
        report.push("registered in Apps & features, so Windows can remove it".to_string());
    }
    Ok(report)
}

/// Remove what `install` added.
pub fn uninstall() -> Result<Vec<String>, String> {
    let dir = bin_dir()
        .ok_or_else(|| "no per-user program directory could be found on this system".to_string())?;
    let mut report = Vec::new();

    match remove_from_path(&dir) {
        Ok(true) => report.push(format!("removed {} from your PATH", dir.display())),
        Ok(false) => report.push("PATH did not mention it".to_string()),
        Err(error) => report.push(format!("PATH was not changed: {error}")),
    }

    unregister_uninstall()?;

    // The binaries, by name. See `remove_programs` for why this is not a
    // recursive delete of the directory they are in.
    let running = std::env::current_exe().ok();
    let (removed, in_use) = remove_programs(&dir, running.as_deref())?;
    if removed.is_empty() && in_use.is_empty() {
        report.push("nothing was installed".to_string());
    } else if !removed.is_empty() {
        report.push(format!(
            "removed {} from {}",
            removed.join(", "),
            dir.display()
        ));
    }
    for path in &in_use {
        report.push(format!(
            "left {} in place: this program is running from it, and a running \
             program cannot delete itself. Remove that file by hand.",
            path.display()
        ));
    }

    // VeilVoice's own directory, which is a different one everywhere but
    // Windows, and which it created. Only this directory may go whole.
    if let Some(owned) = removable_prefix() {
        // On Windows that is the directory the binaries were just in, so a
        // program still running from it has already been reported and there is
        // nothing to add by failing to delete the folder around it.
        let already_said = !in_use.is_empty() && owned == dir;
        if owned.exists() && !already_said {
            match std::fs::remove_dir_all(&owned) {
                Ok(()) => report.push(format!("removed {}", owned.display())),
                // Not fatal. The binaries are gone, `PATH` is clean and the
                // uninstall entry is gone; failing the whole operation over a
                // data directory would leave somebody with a half-removed
                // program and an error they cannot act on.
                Err(e) => report.push(format!("could not remove {}: {e}", owned.display())),
            }
        }
    }

    Ok(report)
}

/// Remove the programs `install` wrote, by name, and say which are still in
/// use.
///
/// **By name, never as a directory.** On every platform but Windows the
/// binaries live in `~/.local/bin`, which belongs to the user and holds other
/// programs' binaries too. A `remove_dir_all` there would take every one of
/// them, during an uninstall, which is the moment somebody is least likely to
/// be watching closely. The previous version of this function did exactly that
/// to [`prefix`], and it was only ever safe because `prefix` happened not to be
/// a shared directory: one line moving the binaries to where they belonged
/// would have turned it into a command that empties `~/.local/bin`. That is
/// half of F-214, and the answer is a function that cannot do it rather
/// than a comment asking the next person not to.
///
/// A file that is the running program is reported rather than deleted: Windows
/// will not unlink a running executable, and the honest report is which file is
/// still there and why.
fn remove_programs(
    dir: &Path,
    running: Option<&Path>,
) -> Result<(Vec<String>, Vec<PathBuf>), String> {
    let mut removed = Vec::new();
    let mut in_use = Vec::new();
    for stem in PROGRAMS {
        let name = exe_name(stem);
        let path = dir.join(&name);
        if !path.exists() {
            continue;
        }
        if running == Some(path.as_path()) {
            in_use.push(path);
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => removed.push(name),
            // Anything else is a real failure and is reported as one: a
            // binary left behind is a `veilvoice` that still runs after an
            // uninstall said it was gone.
            Err(e) => return Err(format!("could not remove {}: {e}", path.display())),
        }
    }
    Ok((removed, in_use))
}

/// [`prefix`], but only when it is genuinely a directory VeilVoice made for
/// itself.
///
/// The check is the point rather than paranoia. `remove_dir_all` is the one
/// call in this crate that can destroy something a user cares about, and it is
/// reached during an uninstall, so it is given a directory that has passed two
/// tests: it is not the directory the binaries live in, and its last component
/// names VeilVoice. Either one alone would have been enough to stop
/// F-214 turning into a command that empties `~/.local/bin`; both are
/// here because the cost of the check is nothing and the cost of being wrong
/// is somebody's machine.
fn removable_prefix() -> Option<PathBuf> {
    let owned = prefix()?;
    if Some(&owned) == bin_dir().as_ref() && !cfg!(windows) {
        // On Windows these are deliberately the same directory, and it is
        // VeilVoice's own, so the equality is not a warning there.
        return None;
    }
    let last = owned.file_name()?.to_str()?.to_ascii_lowercase();
    if !last.contains("veilvoice") {
        return None;
    }
    Some(owned)
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

    /// **F-214, the half that could have damaged a machine.**
    ///
    /// `uninstall` removes the programs it wrote **by name** and leaves
    /// everything else in the directory alone. The directory the binaries live
    /// in is `~/.local/bin` on every platform but Windows, and that belongs to
    /// the user: it holds whatever else they have installed. The previous
    /// version removed a directory recursively, and was safe only because it
    /// happened to be pointed at a directory nobody else wrote to.
    ///
    /// Written against `remove_programs` directly rather than `uninstall`,
    /// because `uninstall` reads the real environment and a test that could
    /// empty the developer's own `~/.local/bin` is not a test worth having.
    #[test]
    fn uninstalling_removes_its_own_files_and_nothing_else() {
        let dir = std::env::temp_dir().join(format!(
            "veilvoice-uninstall-{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // What VeilVoice put there, and what somebody else did.
        for stem in PROGRAMS {
            std::fs::write(dir.join(exe_name(stem)), b"ours").unwrap();
        }
        let theirs = ["ripgrep", "fd", "a-script-somebody-wrote"];
        for name in theirs {
            std::fs::write(dir.join(name), b"not ours").unwrap();
        }
        std::fs::create_dir(dir.join("a-directory-of-their-own")).unwrap();

        let (removed, in_use) = remove_programs(&dir, None).expect("it removes");
        assert_eq!(removed.len(), PROGRAMS.len(), "{removed:?}");
        assert!(in_use.is_empty());

        for stem in PROGRAMS {
            assert!(!dir.join(exe_name(stem)).exists(), "{stem} was not removed");
        }
        for name in theirs {
            assert!(
                dir.join(name).exists(),
                "uninstalling VeilVoice removed {name}, which is not VeilVoice"
            );
        }
        assert!(
            dir.join("a-directory-of-their-own").is_dir(),
            "uninstalling VeilVoice removed a directory that was not its own"
        );
        assert!(
            dir.is_dir(),
            "uninstalling VeilVoice removed the directory itself"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The running program is reported rather than deleted, and nothing else
    /// is skipped because of it.
    #[test]
    fn the_running_program_is_left_and_named() {
        let dir = std::env::temp_dir().join(format!(
            "veilvoice-uninstall-running-{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for stem in PROGRAMS {
            std::fs::write(dir.join(exe_name(stem)), b"ours").unwrap();
        }
        let running = dir.join(exe_name(PROGRAMS[0]));

        let (removed, in_use) = remove_programs(&dir, Some(&running)).expect("it removes");
        assert_eq!(in_use, vec![running.clone()]);
        assert!(running.exists(), "the running program was deleted");
        assert_eq!(
            removed.len(),
            PROGRAMS.len() - 1,
            "the others should still go: {removed:?}"
        );
    }

    /// The one recursive delete in this crate is only ever given a directory
    /// VeilVoice made for itself.
    ///
    /// The guard is what keeps F-214 from being able to come back as
    /// something much worse than an install that did not work. If `prefix` is
    /// ever pointed at the directory the binaries live in, on a platform where
    /// that directory is shared, this returns `None` and the recursive delete
    /// does not happen at all.
    #[test]
    fn only_a_directory_named_for_veilvoice_may_be_removed_whole() {
        let Some(owned) = removable_prefix() else {
            // Nothing will be removed recursively on this machine, which is the
            // safe answer and not a failure.
            return;
        };
        let last = owned
            .file_name()
            .expect("a final component")
            .to_str()
            .expect("a readable name")
            .to_ascii_lowercase();
        assert!(
            last.contains("veilvoice"),
            "{} would be removed whole and is not VeilVoice's own directory",
            owned.display()
        );
        if !cfg!(windows) {
            assert_ne!(
                Some(owned.clone()),
                bin_dir(),
                "the directory holding other programs' binaries must never be \
                 removed whole"
            );
        }
    }

    /// Everything `status` reports is about one directory, and it is the one
    /// the binaries actually go into.
    ///
    /// Three facts about three different places is what F-214 was:
    /// `install` copied into one, added a second to `PATH`, and `status`
    /// answered about a third, so an install that worked reported itself as an
    /// install that had not happened.
    #[test]
    fn everything_reported_is_about_the_directory_the_binaries_go_into() {
        let state = status();
        assert_eq!(
            state.prefix,
            bin_dir(),
            "status reports a directory the binaries are not copied into"
        );
        if let (Some(dir), Some(from)) = (&state.prefix, &state.running_from) {
            assert_eq!(
                state.running_installed,
                from.parent() == Some(dir.as_path()),
                "`running_installed` is answered about a different directory"
            );
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
