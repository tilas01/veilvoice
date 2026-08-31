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
            let verdict = if !path.exists() {
                Verdict::Missing
            } else {
                match crate::sha256_file(&path) {
                    Err(why) => Verdict::Unreadable(why.to_string()),
                    Ok(found) if crate::digests_match(&found, &member.digest) => Verdict::Matches,
                    Ok(found) => Verdict::Differs { found },
                }
            };
            Outcome {
                path: member.path.clone(),
                verdict,
            }
        })
        .collect()
}

/// Files sitting in the extracted directory that the release never published.
///
/// Reported rather than ignored. Everything else here answers "is what should
/// be there, there"; this answers the other half, and the other half is the one
/// an attacker uses. A directory holding every published file, unmodified, plus
/// one extra program, passes every check above and is not the release.
pub fn extras(root: &Path, archive: &ArchiveContents) -> Vec<PathBuf> {
    let published: BTreeSet<PathBuf> = archive
        .members
        .iter()
        .map(|member| root.join(&member.path))
        .collect();
    let mut found = Vec::new();
    for directory in archive.roots() {
        walk(&root.join(directory), &published, &mut found);
    }
    found.sort();
    found
}

/// Every file under `directory` that is not in `published`.
///
/// Depth is bounded by the tree on disk rather than by a counter, and the walk
/// never follows a symbolic link into a directory: an extracted release with a
/// link back to `/` in it would otherwise walk the whole filesystem, which is a
/// denial of service written by the person being checked.
fn walk(directory: &Path, published: &BTreeSet<PathBuf>, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            // A link is neither walked nor hashed through. It is not a file the
            // release published, so it is reported as an extra and left alone.
            if !published.contains(&path) {
                found.push(path);
            }
        } else if kind.is_dir() {
            walk(&path, published, found);
        } else if !published.contains(&path) {
            found.push(path);
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
        let extra = extras(&room, &all[0]);
        let names: Vec<String> = extra
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["extra.md".to_string(), "helper.sh".to_string()]);
        std::fs::remove_dir_all(&room).ok();
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
