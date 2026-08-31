// SPDX-License-Identifier: GPL-3.0-or-later
//! What came out of the archive, and the GnuPG somebody already has.
//!
//! **Marker 91.** Two halves of the same request: check the extracted copy as
//! well as the archive, and offer the check through GnuPG for anybody who would
//! rather trust their own tools than this binary.
//!
//! # This used to be the honest limit, and it is not the limit any more
//!
//! Worth reading before the code, because the module changed shape around it.
//!
//! A release signs `SHA256SUMS`, and `SHA256SUMS` covers the **archives**. So
//! verifying `veilvoice-0.1.14-linux-x86_64.zip` proved that archive was the
//! one that was signed, and proved nothing at all about the folder sitting
//! beside it. The folder may predate the download, may have come out of a
//! different copy, may have been edited since. Nothing on disk records which
//! archive a directory was extracted from.
//!
//! That was written here as a limit that could not be lifted, and it could not
//! be lifted **from this side**. It was lifted from the other one. A release
//! now also publishes `CONTENTS.sha256`, listing every file inside every
//! archive with its SHA-256, staged before `SHA256SUMS` is computed so that the
//! signature covers it too. `veilvoice_check::contents` reads it and `main.rs`
//! checks the extracted folder against it, file by file, and reports anything
//! in that folder the release never published.
//!
//! The lesson is worth keeping beside the code: "no signed list covers loose
//! files" was a true statement about the release format, and it was being
//! treated as a fact about the world. Publishing one more file changed it.
//!
//! What is left here is the part no hash can answer. A file can be byte for
//! byte correct and still not start, because the tool that unpacked it dropped
//! the execute bit, and somebody in that position has a folder that looks
//! perfect and does nothing. That is what [`look_in`] and [`Program::runnable`]
//! are for, and they are still asked after every hash has matched.
//!
//! Releases published before v0.1.15 carry no contents list, and for those the
//! old report and the old caveat are exactly what is printed, because they were
//! honest then and still are.
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
//! From v0.1.15 a release publishes a signed list of everything inside each
//! archive, so every file in that folder is checked against it, and anything in
//! there that was not part of the release is named. For older releases, which
//! carry no such list, it can only tell you the programs are there and that
//! your system will run them, and it says so rather than implying more.

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
/// Marker 90 moved the body out of this binary so the desktop application's
/// verify tab prints the same commands; marker 97 moved it again, into
/// `veilvoice-gnupg`, which also *runs* them. Re-exported here rather than
/// called through at every site, which keeps this module the one place the
/// verifier looks for anything about extracted releases and GnuPG.
pub use veilvoice_gnupg::commands as gnupg_commands;

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
