// SPDX-License-Identifier: GPL-3.0-or-later
//! The signed list of what is inside each release archive.
//!
//! **Marker 97.** `SHA256SUMS` covers the archives. That proves a download is
//! the one that was published, and it says nothing at all about the folder
//! somebody unzipped it into, which is the copy they actually run.
//!
//! # The gap this closes
//!
//! Until this existed, a verifier could check the archive and could not check
//! the extracted directory beside it, and the reason was not laziness: nothing
//! on disk records which archive a directory came from, and no signed list
//! covered the loose files. The honest report was therefore two separate
//! answers and the advice to extract the checked archive again.
//!
//! That advice is still correct and it is a poor substitute for an answer. A
//! release now publishes `CONTENTS.sha256`, which lists every file inside every
//! archive with its SHA-256, and it is staged **before** `SHA256SUMS` is
//! computed, so the hash list covers it and the signature therefore covers it
//! too. The chain is complete and every link in it is checkable:
//!
//! ```text
//! SHA256SUMS.asc -> SHA256SUMS -> CONTENTS.sha256 -> each file on disk
//! ```
//!
//! So the question "is the program I am about to run the one that was
//! published" now has an arithmetic answer rather than an inference.
//!
//! # What it still does not prove
//!
//! The same limit as everywhere else in this crate, and it is worth repeating
//! rather than assuming somebody read it in [`crate`]: this proves the files
//! are the ones the holder of the key published. It does not prove they are
//! safe, and it does not prove they were built from the source you can read.
//!
//! # Why the paths are checked before they are used
//!
//! The manifest is signed, and a caller is told in the plainest terms to verify
//! the signature before parsing it. That is a rule a caller can get wrong, and
//! the cost of getting it wrong here would be a file of somebody else's
//! choosing deciding which paths this reads. So [`parse`] refuses an absolute
//! path, a path with a `..` component, a Windows drive letter and a backslash,
//! rather than trusting the order the caller did things in.
//!
//! Refusing rather than sanitising, because a manifest containing such a path
//! is not a manifest with one bad line in it: it is a file that did not come
//! from this project's release job, and the useful thing to do with it is say
//! so.
//!
//! # In plain words
//!
//! The list of every file inside a release, with its fingerprint, signed along
//! with everything else.
//!
//! It is what lets a checker tell you that the program sitting in your folder
//! is the one that was published, rather than only that the zip you downloaded
//! was.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::Error;

/// The name a release publishes its contents list under.
pub const CONTENTS: &str = "CONTENTS.sha256";

/// One file inside a release archive, and the hash published for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Where it sits inside the archive, including the release directory.
    pub path: String,
    /// Its SHA-256, lowercase hex.
    pub digest: String,
}

/// Everything one archive carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveContents {
    /// The archive's file name, as published.
    pub archive: String,
    /// Its files, in the order the manifest lists them.
    pub members: Vec<Member>,
}

impl ArchiveContents {
    /// The top-level directories the archive extracts into.
    ///
    /// One, for every archive this project publishes. Returned as a set rather
    /// than assumed, because an archive with two roots is a thing that can
    /// exist and quietly walking only the first would leave files unchecked.
    pub fn roots(&self) -> BTreeSet<String> {
        self.members
            .iter()
            .filter_map(|member| member.path.split('/').next())
            .filter(|first| !first.is_empty())
            .map(str::to_string)
            .collect()
    }
}

/// What checking one published file against the disk found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The file is there and its hash is the published one.
    Matches,
    /// The file is there and it is not the published one.
    Differs {
        /// What it actually hashes to.
        found: String,
    },
    /// The file is not there.
    Missing,
    /// The file is there and could not be read.
    Unreadable(String),
    /// Something is there under that name and it is not an ordinary file.
    ///
    /// **F-99.** A symbolic link at a published path used to hash whatever it
    /// pointed at and report `Matches`, which is wrong twice over. The release
    /// published a file, not a link; and a link is a name that somebody else
    /// may be able to repoint after this has looked, which is the one
    /// substitution a hash check cannot notice. The sweep for extra files
    /// already refuses to walk through links, so accepting one here was the
    /// two halves of this module disagreeing about what a link is.
    NotAFile(String),
}

/// One published file, checked against the disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The path as the manifest gives it.
    pub path: String,
    /// What was found.
    pub verdict: Verdict,
}

impl Outcome {
    /// Whether this one is as published.
    pub fn is_good(&self) -> bool {
        self.verdict == Verdict::Matches
    }
}

/// Whether a path from the manifest is safe to join onto a directory.
///
/// See the module note. Empty, absolute, `..`, a drive letter and a backslash
/// are all refused; everything else is an ordinary relative path.
fn safe(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return false;
    }
    // `C:` and the like. Checked on the whole string rather than per component,
    // because a drive letter can only lead.
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        return false;
    }
    path.split('/').all(|part| part != ".." && part != ".")
}

/// Whether a string is 64 lowercase hex characters.
fn looks_like_a_digest(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Read a `CONTENTS.sha256`.
///
/// **Verify the signature over `SHA256SUMS`, and this file against
/// `SHA256SUMS`, before calling this.** Parsing an unverified manifest and
/// reporting on it would be checking a download against a list that came with
/// it, which proves nothing.
///
/// Strict on purpose. A line this cannot make sense of fails the whole file
/// rather than being skipped: a signed manifest is either the one the release
/// job wrote or it is not, and half-understanding one is how a verifier reports
/// a pass over files it never looked at.
pub fn parse(text: &str) -> Result<Vec<ArchiveContents>, Error> {
    let mut all: Vec<ArchiveContents> = Vec::new();
    for (number, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches(['\r', ' ', '\t']);
        if line.is_empty() {
            continue;
        }
        let at = number + 1;
        if let Some(name) = line.strip_prefix("# ") {
            let name = name.trim();
            if name.is_empty() {
                return Err(Error::Malformed(format!(
                    "line {at} of the contents list names no archive"
                )));
            }
            all.push(ArchiveContents {
                archive: name.to_string(),
                members: Vec::new(),
            });
            continue;
        }
        let Some((digest, path)) = line.split_once("  ") else {
            return Err(Error::Malformed(format!(
                "line {at} of the contents list is not a hash and a path"
            )));
        };
        if !looks_like_a_digest(digest) {
            return Err(Error::Malformed(format!(
                "line {at} of the contents list does not begin with a SHA-256"
            )));
        }
        let path = path.trim();
        if !safe(path) {
            return Err(Error::Malformed(format!(
                "line {at} of the contents list names a path outside the release: {path}"
            )));
        }
        let Some(current) = all.last_mut() else {
            return Err(Error::Malformed(format!(
                "line {at} of the contents list comes before any archive is named"
            )));
        };
        current.members.push(Member {
            path: path.to_string(),
            digest: digest.to_ascii_lowercase(),
        });
    }
    Ok(all)
}

/// The section of a manifest covering one archive.
pub fn for_archive<'a>(all: &'a [ArchiveContents], archive: &str) -> Option<&'a ArchiveContents> {
    all.iter().find(|entry| entry.archive == archive)
}

/// Check every file the archive published, against `root`.
///
/// `root` is the directory the archive sits in, because the manifest's paths
/// already carry the release directory name. So an archive and the folder it
/// was extracted into, side by side in a downloads folder, need no argument
/// beyond the folder itself.
pub fn check(root: &Path, archive: &ArchiveContents) -> Vec<Outcome> {
    archive
        .members
        .iter()
        .map(|member| {
            let path = root.join(&member.path);
            // `symlink_metadata`, not `metadata`: the question is what is at
            // this name, not what it leads to. See `Verdict::NotAFile`.
            let verdict = match std::fs::symlink_metadata(&path) {
                Err(_) => Verdict::Missing,
                Ok(meta) if meta.file_type().is_symlink() => {
                    Verdict::NotAFile("a symbolic link".to_string())
                }
                Ok(meta) if meta.is_dir() => Verdict::NotAFile("a directory".to_string()),
                Ok(meta) if !meta.is_file() => {
                    Verdict::NotAFile("not an ordinary file".to_string())
                }
                Ok(_) => match crate::sha256_file(&path) {
                    Err(why) => Verdict::Unreadable(why.to_string()),
                    Ok(found) if crate::digests_match(&found, &member.digest) => Verdict::Matches,
                    Ok(found) => Verdict::Differs { found },
                },
            };
            Outcome {
                path: member.path.clone(),
                verdict,
            }
        })
        .collect()
}

/// What a sweep of the extracted directory found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sweep {
    /// Files that are there and were never published.
    pub extras: Vec<PathBuf>,
    /// Directories the sweep could not read, and so could not clear.
    ///
    /// **F-98.** This used to be nothing: an unreadable directory ended the
    /// walk and the caller was handed an empty list, which reads as "there is
    /// nothing else in the folder" and means "I could not look". Measured: a
    /// directory tree deep enough that its absolute path passes `PATH_MAX`
    /// stops `read_dir` at about 1988 levels on Linux, and a file below that
    /// point was reported as absent rather than as unreachable. Permissions do
    /// the same thing far more easily.
    ///
    /// That is the failure this project has now made in several places and
    /// named each time: a check that cannot see must not answer "clear". The
    /// callers treat a non-empty list here as a reason to withhold the pass.
    pub unreadable: Vec<PathBuf>,
}

impl Sweep {
    /// Whether the folder is exactly what the release published.
    ///
    /// False when anything extra was found **and** false when anything could
    /// not be read, which is the distinction F-98 was about.
    pub fn is_clean(&self) -> bool {
        self.extras.is_empty() && self.unreadable.is_empty()
    }
}

/// Files sitting in the extracted directory that the release never published.
///
/// Reported rather than ignored. Everything else here answers "is what should
/// be there, there"; this answers the other half, and the other half is the one
/// an attacker uses. A directory holding every published file, unmodified, plus
/// one extra program, passes every check above and is not the release.
pub fn extras(root: &Path, archive: &ArchiveContents) -> Sweep {
    let published: BTreeSet<PathBuf> = archive
        .members
        .iter()
        .map(|member| root.join(&member.path))
        .collect();
    let mut sweep = Sweep::default();
    for directory in archive.roots() {
        walk(&root.join(directory), &published, &mut sweep);
    }
    sweep.extras.sort();
    sweep.unreadable.sort();
    sweep
}

/// Every file under `directory` that is not in `published`.
///
/// Iterative rather than recursive. Measured on Linux, the deepest directory
/// an absolute path can name is about 1988 levels, so the recursion this
/// replaced would not in fact have overflowed a stack -- but the bound came
/// from `PATH_MAX` rather than from this code, and a bound nobody here chose is
/// not a bound this code can rely on. An explicit stack has one.
///
/// A symbolic link is never walked into and never hashed through. An extracted
/// release with a link back to `/` in it would otherwise walk the whole
/// filesystem, which is a denial of service written by the person being
/// checked. The link is reported as an extra, because it is not a file the
/// release published.
fn walk(start: &Path, published: &BTreeSet<PathBuf>, sweep: &mut Sweep) {
    let mut pending = vec![start.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            // F-98. Recorded rather than skipped. A directory that could not be
            // opened is a directory whose contents are unknown, and unknown is
            // not the same as empty.
            Err(_) => {
                sweep.unreadable.push(directory);
                continue;
            }
        };
        for entry in entries {
            let Ok(entry) = entry else {
                // The listing itself failed part way through, so what is left
                // in this directory is unknown for the same reason.
                sweep.unreadable.push(directory.clone());
                continue;
            };
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                sweep.unreadable.push(path);
                continue;
            };
            if kind.is_symlink() {
                if !published.contains(&path) {
                    sweep.extras.push(path);
                }
            } else if kind.is_dir() {
                pending.push(path);
            } else if !published.contains(&path) {
                sweep.extras.push(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# veilvoice-v0.1.15-linux-x86_64.tar.gz
0000000000000000000000000000000000000000000000000000000000000001  veilvoice-v0.1.15-linux-x86_64/veilvoice
0000000000000000000000000000000000000000000000000000000000000002  veilvoice-v0.1.15-linux-x86_64/docs/A.md

# veilvoice-v0.1.15-windows-x86_64.zip
0000000000000000000000000000000000000000000000000000000000000003  veilvoice-v0.1.15-windows-x86_64/veilvoice.exe
";

    #[test]
    fn a_manifest_reads_back_as_its_archives_and_their_files() {
        let all = parse(SAMPLE).expect("the sample parses");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].archive, "veilvoice-v0.1.15-linux-x86_64.tar.gz");
        assert_eq!(all[0].members.len(), 2);
        assert_eq!(
            all[1].members[0].path,
            "veilvoice-v0.1.15-windows-x86_64/veilvoice.exe"
        );
        assert_eq!(
            all[0].roots().into_iter().collect::<Vec<_>>(),
            vec!["veilvoice-v0.1.15-linux-x86_64".to_string()]
        );
    }

    #[test]
    fn an_archive_is_found_by_its_published_name() {
        let all = parse(SAMPLE).unwrap();
        assert!(for_archive(&all, "veilvoice-v0.1.15-linux-x86_64.tar.gz").is_some());
        assert!(for_archive(&all, "veilvoice-v0.1.15-linux-aarch64.tar.gz").is_none());
    }

    /// A line that is not a hash and a path fails the file. See [`parse`].
    #[test]
    fn a_line_that_makes_no_sense_fails_the_whole_manifest() {
        for bad in [
            "# a.tar.gz\nnot a hash at all\n",
            "# a.tar.gz\nbeef  x\n",
            "0000000000000000000000000000000000000000000000000000000000000001  x\n",
            "# \n",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should not parse");
        }
    }

    /// The paths this will later read are refused before they are used, not
    /// after. See the module note.
    #[test]
    fn a_path_that_leaves_the_release_is_refused() {
        for escape in [
            "veilvoice/../../../etc/shadow",
            "/etc/shadow",
            "C:/Windows/System32/config/SAM",
            "veilvoice\\..\\..\\secrets",
            "./veilvoice",
        ] {
            let text = format!("# a.tar.gz\n{}  {escape}\n", "0".repeat(64));
            let refused = parse(&text).unwrap_err().to_string();
            assert!(
                refused.contains("outside the release"),
                "{escape}: {refused}"
            );
        }
    }

    /// The hash is compared, not the presence of the file.
    #[test]
    fn a_file_that_is_there_and_wrong_is_not_a_pass() {
        let room = tempdir();
        let release = room.join("veilvoice-v0.1.15-linux-x86_64");
        std::fs::create_dir_all(release.join("docs")).unwrap();
        std::fs::write(release.join("veilvoice"), b"the real one").unwrap();
        std::fs::write(release.join("docs/A.md"), b"tampered").unwrap();

        let real = crate::sha256_bytes(b"the real one");
        let published = crate::sha256_bytes(b"the published one");
        let text = format!(
            "# a.tar.gz\n{real}  veilvoice-v0.1.15-linux-x86_64/veilvoice\n\
             {published}  veilvoice-v0.1.15-linux-x86_64/docs/A.md\n\
             {published}  veilvoice-v0.1.15-linux-x86_64/gone\n"
        );
        let all = parse(&text).unwrap();
        let outcomes = check(&room, &all[0]);
        assert_eq!(outcomes[0].verdict, Verdict::Matches);
        assert!(matches!(outcomes[1].verdict, Verdict::Differs { .. }));
        assert_eq!(outcomes[2].verdict, Verdict::Missing);
        assert!(outcomes[0].is_good() && !outcomes[1].is_good());
        std::fs::remove_dir_all(&room).ok();
    }

    /// A directory holding every published file plus one more is not the
    /// release, and saying so is the whole reason [`extras`] exists.
    #[test]
    fn a_file_nobody_published_is_reported() {
        let room = tempdir();
        let release = room.join("veilvoice-v0.1.15-linux-x86_64");
        std::fs::create_dir_all(release.join("docs")).unwrap();
        std::fs::write(release.join("veilvoice"), b"the real one").unwrap();
        std::fs::write(release.join("helper.sh"), b"#!/bin/sh\n").unwrap();
        std::fs::write(release.join("docs/extra.md"), b"hello").unwrap();

        let real = crate::sha256_bytes(b"the real one");
        let text = format!("# a.tar.gz\n{real}  veilvoice-v0.1.15-linux-x86_64/veilvoice\n");
        let all = parse(&text).unwrap();
        let sweep = extras(&room, &all[0]);
        let names: Vec<String> = sweep
            .extras
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["extra.md".to_string(), "helper.sh".to_string()]);
        assert!(sweep.unreadable.is_empty(), "{sweep:?}");
        assert!(!sweep.is_clean(), "a folder with extras in it is not clean");
        std::fs::remove_dir_all(&room).ok();
    }

    /// **F-99.** A link where a file should be is not the published file, even
    /// when what it points at hashes correctly.
    ///
    /// The release published a file. A link is a name somebody else may be
    /// able to repoint after this has looked, which is the one substitution a
    /// hash check cannot notice, and the sweep for extra files already refuses
    /// to walk through links: accepting one here was the two halves of this
    /// module disagreeing about what a link is.
    #[test]
    #[cfg(unix)]
    fn a_link_pointing_at_the_right_bytes_is_still_not_the_published_file() {
        let room = tempdir();
        let release = room.join("veilvoice-v0.1.15-linux-x86_64");
        std::fs::create_dir_all(&release).unwrap();
        // The genuine bytes, somewhere else entirely, with a link to them
        // standing where the program should be.
        let elsewhere = room.join("elsewhere");
        std::fs::write(&elsewhere, b"the real one").unwrap();
        std::os::unix::fs::symlink(&elsewhere, release.join("veilvoice")).unwrap();

        let real = crate::sha256_bytes(b"the real one");
        let text = format!("# a.tar.gz\n{real}  veilvoice-v0.1.15-linux-x86_64/veilvoice\n");
        let all = parse(&text).unwrap();
        let outcomes = check(&room, &all[0]);
        assert!(
            matches!(outcomes[0].verdict, Verdict::NotAFile(_)),
            "a link hashing to the right value is still not the file: {:?}",
            outcomes[0].verdict
        );
        assert!(!outcomes[0].is_good());
        std::fs::remove_dir_all(&room).ok();
    }

    /// A directory standing where a file should be is refused for the same
    /// reason, and without hashing anything.
    #[test]
    fn a_directory_where_a_file_should_be_is_not_the_published_file() {
        let room = tempdir();
        let release = room.join("veilvoice-v0.1.15-linux-x86_64");
        std::fs::create_dir_all(release.join("veilvoice")).unwrap();
        let text = format!(
            "# a.tar.gz\n{}  veilvoice-v0.1.15-linux-x86_64/veilvoice\n",
            "0".repeat(64)
        );
        let all = parse(&text).unwrap();
        let outcomes = check(&room, &all[0]);
        assert!(
            matches!(outcomes[0].verdict, Verdict::NotAFile(_)),
            "{:?}",
            outcomes[0].verdict
        );
        std::fs::remove_dir_all(&room).ok();
    }

    /// **F-98.** A directory that cannot be read is reported, never treated as
    /// empty.
    ///
    /// Measured with a permission bit here, because it is the way this happens
    /// to ordinary people: a folder extracted by another account, or one whose
    /// mode came out of the archive wrong. The deep-tree case that found it is
    /// the same failure with a different cause.
    ///
    /// Skipped where the test can read anything regardless, which is what
    /// running as root means, since the case cannot be created there.
    #[test]
    #[cfg(unix)]
    fn a_directory_that_cannot_be_read_is_not_reported_as_empty() {
        use std::os::unix::fs::PermissionsExt as _;

        let room = tempdir();
        let release = room.join("veilvoice-v0.1.15-linux-x86_64");
        let shut = release.join("shut");
        std::fs::create_dir_all(&shut).unwrap();
        std::fs::write(shut.join("something"), b"hidden").unwrap();
        std::fs::write(release.join("veilvoice"), b"the real one").unwrap();
        std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();

        let readable = std::fs::read_dir(&shut).is_ok();
        if !readable {
            let real = crate::sha256_bytes(b"the real one");
            let text = format!("# a.tar.gz\n{real}  veilvoice-v0.1.15-linux-x86_64/veilvoice\n");
            let all = parse(&text).unwrap();
            let sweep = extras(&room, &all[0]);
            assert_eq!(
                sweep.unreadable.len(),
                1,
                "the shut directory must be reported: {sweep:?}"
            );
            assert!(
                !sweep.is_clean(),
                "a folder with a door this could not open is not clean"
            );
        }
        std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o700)).ok();
        std::fs::remove_dir_all(&room).ok();
    }

    /// The walk keeps its own stack, so its depth is not the call stack's.
    #[test]
    fn the_sweep_does_not_recurse() {
        let source = include_str!("contents.rs");
        let body = source.split("#[cfg(test)]").next().unwrap();
        let walk = body.split("fn walk(").nth(1).unwrap();
        assert!(
            !walk.contains("walk("),
            "the sweep calls itself, so its depth is the call stack's"
        );
        assert!(walk.contains("while let Some("), "and it must have its own");
    }

    /// An empty manifest is not an error and is not a pass either: it lists no
    /// archives, so no caller can find theirs in it.
    #[test]
    fn an_empty_manifest_lists_nothing() {
        assert!(parse("").unwrap().is_empty());
        assert!(parse("\n\n").unwrap().is_empty());
    }

    /// Somewhere to put files, without a dependency for it.
    fn tempdir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        path.push(format!(
            "veilvoice-contents-{stamp}-{:?}",
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
