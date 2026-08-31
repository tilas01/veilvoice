// SPDX-License-Identifier: GPL-3.0-or-later
//! What came out of the archive, and the GnuPG somebody already has.
//!
//! **Marker 91.** Two halves of the same request: check the extracted copy as
//! well as the archive, and offer the check through GnuPG for anybody who would
//! rather trust their own tools than this binary.
//!
//! # The honest limit on checking an extracted copy
//!
//! This is the part worth reading before the code. A release signs
//! `SHA256SUMS`, and `SHA256SUMS` covers the **archives**. It says nothing
//! about a directory somebody unzipped last week.
//!
//! So verifying `veilvoice-0.1.14-linux-x86_64.zip` proves that archive is the
//! one that was signed. It does **not** prove that the folder sitting beside it
//! came out of that archive. The folder may predate the download, may have been
//! extracted from a different copy, may have been edited since. Nothing on disk
//! records which archive an extracted directory came from, and no amount of
//! hashing the loose files can invent that link, because there is no signed
//! list of what those files should be.
//!
//! A verifier that reported "archive good, extracted files present" as one
//! green result would be telling somebody their installed copy is verified when
//! it is not. So this reports the two separately, in those words, and says the
//! one thing that does resolve it: extract the archive that was just checked,
//! now, and use that.
//!
//! What it can honestly say about the extracted copy is whether the programs
//! are there and whether the operating system will run them, which is the other
//! half of what was asked and is a real thing to get wrong: an archive
//! extracted by a tool that drops the executable bit leaves somebody with files
//! that look right and will not start.
//!
//! # GnuPG
//!
//! VeilVoice checks the signature itself, with the key compiled into this
//! binary, so that somebody with no GnuPG installed is not stuck. That is a
//! convenience and it has an obvious circularity: the program telling you the
//! download is genuine is a program from the same download.
//!
//! [`gnupg_commands`] is the answer to that. It prints the exact commands to
//! run with a GnuPG this project did not write, against a key fingerprint
//! published somewhere this project does not control. Anybody who wants the
//! independent check has it, spelled out, with nothing to work out.
//!
//! # In plain words
//!
//! Checks the folder you unzipped, as well as the zip.
//!
//! It will tell you the programs are there and that your system will run them.
//! It will not tell you the folder came out of the zip it just checked, because
//! nothing on your disk records that. If you want to be certain, unzip the
//! checked file again and use what comes out.

use std::path::{Path, PathBuf};

/// The programs a release archive carries.
pub const PROGRAMS: &[&str] = &["veilvoice", "veilvoice-verify", "veilvoice-gui"];

/// One program found in an extracted directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// Where it is.
    pub path: PathBuf,
    /// Whether the operating system will run it.
    ///
    /// On Unix, the owner execute bit. On Windows there is no such bit and the
    /// extension decides, so this is true for a file that is there at all and
    /// the report says which platform's answer it is giving.
    pub runnable: bool,
}

/// What an extracted directory turned out to hold.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Extracted {
    /// The directory itself.
    pub directory: PathBuf,
    /// The programs found in it.
    pub programs: Vec<Program>,
}

impl Extracted {
    /// Whether anything was found at all.
    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }

    /// The programs the operating system will not run.
    pub fn not_runnable(&self) -> Vec<&Program> {
        self.programs.iter().filter(|p| !p.runnable).collect()
    }
}

/// The directory an archive would extract into, by this project's naming.
///
/// `veilvoice-0.1.14-linux-x86_64.zip` extracts into
/// `veilvoice-0.1.14-linux-x86_64`. Returns `None` for a name that is not one
/// of ours rather than stripping whatever happens to be after the last dot.
pub fn directory_for(archive: &Path) -> Option<PathBuf> {
    let name = archive.file_name()?.to_str()?;
    let stem = [".tar.gz", ".tar.xz", ".tgz", ".zip"]
        .iter()
        .find_map(|suffix| name.strip_suffix(suffix))?;
    if !stem.starts_with("veilvoice") {
        return None;
    }
    Some(archive.with_file_name(stem))
}

/// Look in `directory` for the programs a release carries.
pub fn look_in(directory: &Path) -> Extracted {
    let mut found = Extracted {
        directory: directory.to_path_buf(),
        programs: Vec::new(),
    };
    for program in PROGRAMS {
        for name in [program.to_string(), format!("{program}.exe")] {
            let path = directory.join(&name);
            if path.is_file() {
                found.programs.push(Program {
                    runnable: runnable(&path),
                    path,
                });
                break;
            }
        }
    }
    found
}

/// Whether the operating system will run this file.
fn runnable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            // Any execute bit. Checking only the owner's would report a file
            // as unrunnable for somebody running it as a different user in the
            // same group, which is a real way to install these.
            Ok(meta) => meta.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        // No execute bit exists. The extension decides, and the caller is told
        // that is the answer being given.
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            || path.extension().is_none()
    }
}

/// The commands that check this release with somebody else's GnuPG.
///
/// Marker 90 moved the body into [`veilvoice_check::gnupg_commands`] so the
/// desktop application's verify tab prints the same commands. Re-exported here
/// rather than called through at every site, which keeps this module the one
/// place the verifier looks for anything about extracted releases and GnuPG.
pub use veilvoice_check::{gnupg_commands, gnupg_on_path};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_archive_name_yields_the_folder_it_extracts_into() {
        for (archive, folder) in [
            (
                "veilvoice-0.1.14-linux-x86_64.zip",
                "veilvoice-0.1.14-linux-x86_64",
            ),
            ("veilvoice-0.1.14-macos.tar.gz", "veilvoice-0.1.14-macos"),
            ("veilvoice-0.1.14-win.tar.xz", "veilvoice-0.1.14-win"),
        ] {
            assert_eq!(
                directory_for(Path::new(archive)),
                Some(PathBuf::from(folder)),
                "{archive}"
            );
        }
    }

    /// Anything not ours produces nothing, rather than a folder name invented
    /// by stripping whatever came after the last dot.
    #[test]
    fn a_name_that_is_not_ours_yields_nothing() {
        for other in [
            "holiday.zip",
            "notes.tar.gz",
            "veilvoice",
            "somethingelse-1.0.zip",
        ] {
            assert_eq!(directory_for(Path::new(other)), None, "{other}");
        }
    }

    #[test]
    fn an_empty_directory_holds_no_programs() {
        let dir = tempfile::tempdir().unwrap();
        let found = look_in(dir.path());
        assert!(found.is_empty());
        assert!(found.not_runnable().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_program_without_its_execute_bit_is_reported_as_such() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let runnable = dir.path().join("veilvoice");
        let inert = dir.path().join("veilvoice-verify");
        std::fs::write(&runnable, b"#!/bin/sh\n").unwrap();
        std::fs::write(&inert, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&runnable, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&inert, std::fs::Permissions::from_mode(0o644)).unwrap();

        let found = look_in(dir.path());
        assert_eq!(found.programs.len(), 2);
        let stuck = found.not_runnable();
        assert_eq!(stuck.len(), 1, "one of the two is not runnable");
        assert!(stuck[0].path.ends_with("veilvoice-verify"));
    }

    #[test]
    fn the_gnupg_commands_check_the_signature_and_then_the_hashes() {
        let lines = gnupg_commands(
            Path::new("SHA256SUMS"),
            Path::new("SHA256SUMS.asc"),
            Some(Path::new("veilvoice-signing-key.asc")),
        );
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("gpg --import"));
        assert!(lines[1].starts_with("gpg --verify"));
        assert!(
            lines[1].find("SHA256SUMS.asc").unwrap()
                < lines[1].find(" SHA256SUMS").unwrap_or(usize::MAX)
                || lines[1].contains("SHA256SUMS.asc SHA256SUMS"),
            "the signature comes first: {}",
            lines[1]
        );
        assert!(lines[2].contains("sha256sum -c"));

        // Without a key file, there is nothing to import.
        let short = gnupg_commands(Path::new("SHA256SUMS"), Path::new("SHA256SUMS.asc"), None);
        assert_eq!(short.len(), 2);
        assert!(short[0].starts_with("gpg --verify"));
    }

    /// Marker 91. The extracted report must never run when the archive itself
    /// failed. "The archive is bad, and here are the programs in the folder
    /// beside it" reads as reassurance, and there is none to give: an archive
    /// that failed its signature says nothing good about anything unpacked
    /// from it.
    #[test]
    fn a_failed_archive_stops_before_the_extracted_report() {
        let source = include_str!("main.rs").replace("\r\n", "\n");
        let start = source
            .find("fn command_auto(")
            .expect("command_auto exists");
        let end = source[start..]
            .find("\n/// Marker 91. What is in the folder")
            .map(|at| start + at)
            .unwrap_or(source.len());
        let body = &source[start..end];

        let failure = body
            .find("if failures > 0 {")
            .expect("the failure branch exists");
        let report = body
            .find("report_extracted(")
            .expect("the extracted report is called");
        assert!(
            failure < report,
            "the extracted report runs before the failure check"
        );
        assert!(
            body[failure..report].contains("return worst;"),
            "a failed archive falls through into the extracted report"
        );
    }

    /// This module must not run GnuPG. A verifier that shells out to `gpg` and
    /// reports what it said has not escaped the circularity it exists to
    /// escape, because the thing running `gpg` is the binary under suspicion.
    #[test]
    fn it_prints_the_commands_rather_than_running_them() {
        let source = include_str!("extracted.rs").replace("\r\n", "\n");
        let shipped = source.split("#[cfg(test)]").next().unwrap_or("");
        let body: String = shipped
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for forbidden in ["Command::new", "process::Command", ".output()", ".status()"] {
            assert!(
                !body.contains(forbidden),
                "this module calls {forbidden:?}: the independent check is \
                 independent because the person runs it"
            );
        }
    }
}
