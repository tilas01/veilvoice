// SPDX-License-Identifier: GPL-3.0-or-later
//! **Roadmap item 164.** Remembering where copies of VeilVoice were found, so a
//! later pass can say what has changed since.
//!
//! # Why a machine pass that forgets is worth less than one that does not
//!
//! Finding every copy and checking each one answers the question on the day.
//! It cannot answer the more useful question, which is what is different from
//! last time: a copy that has appeared, a copy that has gone, and above all a
//! copy that was the published one in March and is not now. That last case is
//! the one a reader most wants told, and no amount of care with a single run
//! can produce it.
//!
//! So each pass writes down what it saw, and the next one says what moved.
//!
//! # What is written, and what deliberately is not
//!
//! A path, a hash and the verdict reached, one per line, and when the pass
//! ran. Nothing else. No user name, no machine name, and nothing from inside
//! any file: this is a record of what was checked, in a project whose whole
//! subject is not keeping more about somebody than the job needs.
//!
//! It is also **not evidence**, and the file says so in its own first line. It
//! is written by this program, to an ordinary file, with no signature over it;
//! anything that could tamper with a copy of VeilVoice could tamper with this.
//! Its value is entirely in noticing a change worth looking into by hand, and
//! a reader who treats it as proof has been misled by it.
//!
//! # In plain words
//!
//! Writes down where the copies of VeilVoice on this computer were, so that
//! next time it can tell you what has changed.
//!
//! It holds paths and fingerprints and nothing else, and it is not proof of
//! anything: it is a note to yourself about where to look.

use std::path::{Path, PathBuf};

use crate::check::machine::{Checked, Verdict};

/// The name the record is kept under.
pub const RECORD: &str = "copies-seen.txt";

/// The line every record opens with, so the file cannot be mistaken for proof.
const PREAMBLE: &str = "\
# VeilVoice: where copies of this program were found, and what they hashed to.
# Written by `veilvoice verify machine`. This is a note, NOT evidence: nothing
# signs it, and whatever could change a copy of VeilVoice could change this.
# Its only job is to let a later pass say what has changed since.
";

/// Where the record lives, if this system offers anywhere for it.
///
/// Beside the installation rather than in a configuration directory of its
/// own, because `veilvoice-setup` is the one crate this binary already depends
/// on for platform paths and adding a second way of answering "where does
/// state go" is exactly the defect `tools/audit/state_paths.py` exists to
/// catch (F-141).
///
/// `None` when the environment does not say, and a caller that gets `None`
/// reports that it could not remember rather than writing into whatever
/// directory it happens to be standing in.
pub fn path() -> Option<PathBuf> {
    veilvoice_setup::install::prefix().map(|prefix| prefix.join(RECORD))
}

/// One copy as a previous pass saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remembered {
    /// Where it was.
    pub path: String,
    /// What it hashed to, or empty if it could not be read.
    pub digest: String,
    /// The verdict reached, in the same words this file writes.
    pub verdict: String,
}

/// How one line is written: three fields, tab separated.
///
/// Tabs rather than anything cleverer because a path can contain spaces,
/// commas and quotes, and cannot contain a tab on any system this runs on. A
/// format that needs escaping is a format with an escaping defect in it.
fn line_of(checked: &Checked) -> String {
    format!(
        "{}\t{}\t{}",
        checked.copy.path.display(),
        checked.digest.clone().unwrap_or_default(),
        shortly(&checked.verdict),
    )
}

/// The verdict in one word, for the record and for a reader's eye.
pub fn shortly(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Published { release, .. } => format!("published-in-{release}"),
        Verdict::Changed { release, .. } => format!("not-published-in-{release}"),
        Verdict::NoSignedList => "no-signed-list".to_string(),
        Verdict::Unreadable(_) => "unreadable".to_string(),
    }
}

/// Read the previous pass, if there was one.
///
/// A record that cannot be read or cannot be parsed comes back empty rather
/// than as an error. The comparison it feeds is an extra: a machine pass whose
/// answers depended on a file this program wrote last time would be a machine
/// pass that could be silenced by deleting it.
pub fn recall(path: &Path) -> Vec<Remembered> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            let mut fields = line.split('\t');
            Some(Remembered {
                path: fields.next()?.to_string(),
                digest: fields.next().unwrap_or_default().to_string(),
                verdict: fields.next().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

/// Seconds since the Unix epoch, or 0 if the clock is unreadable.
///
/// No date formatting and no dependency for it, which is the answer
/// `veilvoice-gui`'s crash log already gives to the same question and is worth
/// giving the same way twice. A reader comparing two passes needs to know
/// which is the later one, and an epoch second answers that; pulling a
/// calendar library into the binary whose smallness is a feature, to render a
/// number nobody reads aloud, would be a poor trade in a project whose
/// argument is that its dependency graph can be read.
pub fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Write down what this pass saw.
///
/// Returns where it was written, or why it could not be. Failing to remember
/// is reported and is never a failure of the check: the copies were still
/// found and still checked, and a read-only home is not a statement about any
/// of them.
pub fn remember(path: &Path, checked: &[Checked], when: u64) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot make {}: {e}", parent.display()))?;
    }
    let mut text = String::from(PREAMBLE);
    text.push_str(&format!(
        "# last pass: {when} seconds after 1970-01-01 UTC\n"
    ));
    for one in checked {
        text.push_str(&line_of(one));
        text.push('\n');
    }
    std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// What is different between the last pass and this one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Copies this pass found that the last one did not.
    pub appeared: Vec<String>,
    /// Copies the last pass found that this one did not.
    pub gone: Vec<String>,
    /// Copies in both passes whose hash is not the same.
    ///
    /// The line this whole file exists for. A program that was what was
    /// published and now is not is the finding no single pass can make.
    pub altered: Vec<String>,
}

impl Changes {
    /// Whether anything at all moved.
    pub fn any(&self) -> bool {
        !self.appeared.is_empty() || !self.gone.is_empty() || !self.altered.is_empty()
    }
}

/// Compare the last pass against this one.
pub fn compare(before: &[Remembered], now: &[Checked]) -> Changes {
    let mut changes = Changes::default();
    for one in now {
        let path = one.copy.path.display().to_string();
        match before.iter().find(|was| was.path == path) {
            None => changes.appeared.push(path),
            Some(was) => {
                let digest = one.digest.clone().unwrap_or_default();
                // An empty digest on either side means a file that could not
                // be read then or cannot be read now, which is not the same as
                // one whose contents changed and must not be reported as it.
                if !was.digest.is_empty() && !digest.is_empty() && was.digest != digest {
                    changes.altered.push(path);
                }
            }
        }
    }
    for was in before {
        if !now
            .iter()
            .any(|one| one.copy.path.display().to_string() == was.path)
        {
            changes.gone.push(was.path.clone());
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::machine::{Copy, Where};
    use std::path::PathBuf;

    fn checked(path: &str, digest: &str) -> Checked {
        Checked {
            copy: Copy {
                path: PathBuf::from(path),
                found: Where::Installed,
                claims: Some("v0.1.22".to_string()),
            },
            digest: Some(digest.to_string()),
            verdict: Verdict::Published {
                release: "v0.1.22".to_string(),
                inside: "veilvoice-v0.1.22-linux-x86_64/veilvoice".to_string(),
            },
        }
    }

    /// A record written and read back is the same record.
    #[test]
    fn what_is_written_down_reads_back_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RECORD);
        let now = vec![checked("/usr/local/bin/veilvoice", "aa")];
        remember(&path, &now, 1_758_499_200).expect("the record is written");
        let back = recall(&path);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].path, "/usr/local/bin/veilvoice");
        assert_eq!(back[0].digest, "aa");
        assert_eq!(back[0].verdict, "published-in-v0.1.22");
    }

    /// A path with spaces in it survives, because paths have spaces in them.
    #[test]
    fn a_path_with_spaces_in_it_is_not_split() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RECORD);
        let awkward = "/home/a b/My Downloads/veilvoice";
        remember(&path, &[checked(awkward, "bb")], 1_758_499_200).unwrap();
        assert_eq!(recall(&path)[0].path, awkward);
    }

    /// The three things a comparison has to be able to say.
    #[test]
    fn a_second_pass_says_what_appeared_went_and_changed() {
        let before = vec![
            Remembered {
                path: "/old/veilvoice".to_string(),
                digest: "aa".to_string(),
                verdict: "published-in-v0.1.21".to_string(),
            },
            Remembered {
                path: "/kept/veilvoice".to_string(),
                digest: "bb".to_string(),
                verdict: "published-in-v0.1.22".to_string(),
            },
        ];
        let now = vec![
            checked("/kept/veilvoice", "cc"),
            checked("/new/veilvoice", "dd"),
        ];
        let changes = compare(&before, &now);
        assert_eq!(changes.appeared, vec!["/new/veilvoice"]);
        assert_eq!(changes.gone, vec!["/old/veilvoice"]);
        assert_eq!(
            changes.altered,
            vec!["/kept/veilvoice"],
            "a copy whose hash moved is the finding this file exists for"
        );
        assert!(changes.any());
    }

    /// A file that could not be read is not reported as one that changed.
    ///
    /// The two are different facts, and telling somebody their program has
    /// been altered when what happened is a permission error is the shape of
    /// mistake this whole crate is written to avoid.
    #[test]
    fn a_file_that_could_not_be_read_is_not_called_altered() {
        let before = vec![Remembered {
            path: "/kept/veilvoice".to_string(),
            digest: "bb".to_string(),
            verdict: "published-in-v0.1.22".to_string(),
        }];
        let mut unreadable = checked("/kept/veilvoice", "");
        unreadable.digest = None;
        unreadable.verdict = Verdict::Unreadable("permission denied".to_string());
        let changes = compare(&before, &[unreadable]);
        assert!(changes.altered.is_empty(), "{changes:?}");
        assert!(!changes.any());
    }

    /// No previous record is not a machine full of new copies.
    ///
    /// The first run has nothing to compare against, and a report that listed
    /// every copy as newly appeared would be noise on the one run where the
    /// reader has least idea what is normal.
    #[test]
    fn the_first_pass_has_nothing_to_compare_against() {
        let dir = tempfile::tempdir().unwrap();
        assert!(recall(&dir.path().join(RECORD)).is_empty());
    }
}
