// SPDX-License-Identifier: GPL-3.0-or-later
//! How much of a directory tree changed, and how fast.
//!
//! # What this measures, and what it does not
//!
//! It counts files. Take a [`Snapshot`] of a tree, take another later, and
//! [`compare`] reports how many were added, removed or altered in between, and
//! over what interval. That is the whole of the measurement.
//!
//! It is **not** a ransomware detector, and [`Churn`] deliberately has no
//! `is_ransomware` on it. Restoring a backup, importing a camera card, a
//! synchronisation client catching up after a week offline, extracting an
//! archive, and a compiler writing a target directory all produce exactly the
//! shape this measures, because they are all mass rewrites. Anything claiming
//! to tell those apart by counting files is claiming something it cannot do.
//!
//! So the output is numbers plus a [`Concern`] level against a [`Threshold`]
//! **the user sets**, and the front end's job is to say "this many files
//! changed in this long, was that you?" — a question, which the person at the
//! keyboard can answer instantly and this crate never can.
//!
//! # Modification times can be set by whatever did the modifying
//!
//! A file counts as changed when its length or its modification time differs.
//! Both are attacker-controllable: anything that can rewrite a file can restore
//! its old timestamp, and on most filesystems its length is whatever it chose
//! to write. This measure therefore has a floor, not a guarantee, and something
//! careful will pass under it.
//!
//! [`crate::canary`] does not have that weakness, because it compares contents
//! against a recorded digest. It has a different weakness instead. That is why
//! there are two signals here rather than one.
//!
//! # Walking a tree costs real time, so the walk is bounded
//!
//! [`Limits`] caps how many files and how deep, and a snapshot that hit a cap
//! is marked [`Snapshot::truncated`]. A truncated snapshot compared against
//! another truncated snapshot is still useful — both walked in the same order —
//! but the count is of what was looked at, not of what is there, and every
//! report carries that flag so a front end cannot present it as complete.
//!
//! Symbolic links are recorded and never followed. Following them turns a tree
//! walk into a graph walk with cycles in it, and lets a link inside the watched
//! directory make this crate report on files outside it.
//!
//! # Format
//!
//! A snapshot has to survive between two runs of the program, or the only
//! comparison possible is one taken inside a single session — which measures
//! the minute you were looking and nothing else. So it is written to disk, in
//! text, one record per line, for the same reason the tamper manifest is text:
//! the point of the file is to be checkable without this crate.
//!
//! ```text
//! VEILSENTRY-SNAP1
//! root  /home/somebody/Documents
//! taken  1700000000
//! truncated  false
//! unreadable  /home/somebody/Documents/private: permission denied
//! file  4096  1699999000  /home/somebody/Documents/notes.txt
//! ```
//!
//! A modification time the platform did not report is written as `-`, which is
//! not the same as zero: zero is a real instant in 1970 and files claiming it
//! do exist.

use crate::Error;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Magic first line of a saved snapshot. The digit is a format version.
const MAGIC: &str = "VEILSENTRY-SNAP1";

/// What is recorded about one file. Deliberately cheap: no file is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    /// Length in bytes.
    pub len: u64,
    /// Modification time in seconds since the Unix epoch, where the platform
    /// reports one. `None` is not an error — some filesystems do not keep it —
    /// and a file whose time is unknown is compared on length alone.
    pub modified: Option<u64>,
}

/// How far a walk is allowed to go.
///
/// Defaults are a compromise: large enough for a documents folder, small enough
/// that walking one does not become the reason the interface stopped painting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Stop after this many files, and mark the snapshot truncated.
    pub max_files: usize,
    /// How many directories deep to descend. Zero means the root only.
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_depth: 12,
        }
    }
}

/// What a tree looked like at one moment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// The directory that was walked, normalised with forward slashes.
    pub root: String,
    /// When the walk finished, in seconds since the Unix epoch.
    pub taken: u64,
    /// A cap was hit, so this describes what was looked at rather than what is
    /// there. Carried into every [`Churn`] derived from it.
    pub truncated: bool,
    /// Directories that could not be read, with the reason. Reported rather
    /// than skipped silently: a folder that became unreadable between two
    /// snapshots is itself something that happened.
    pub unreadable: Vec<String>,
    files: BTreeMap<String, Facts>,
}

/// The difference between two snapshots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Churn {
    /// Present in the later snapshot and not the earlier.
    pub added: usize,
    /// Present in the earlier and not the later.
    pub removed: usize,
    /// In both, with a different length or modification time.
    pub modified: usize,
    /// In both and identical on both counts.
    pub unchanged: usize,
    /// Seconds between the two snapshots.
    pub window_secs: u64,
    /// Either snapshot hit a cap, so these counts are of what was looked at.
    pub truncated: bool,
}

impl Churn {
    /// Added plus removed plus modified: everything that is not unchanged.
    pub fn touched(&self) -> usize {
        self.added + self.removed + self.modified
    }

    /// Files touched per minute.
    ///
    /// `None` when the two snapshots have the same timestamp. A rate over a
    /// zero-length window is not a large number, it is not a number, and
    /// returning infinity would sail past every threshold in this module.
    pub fn per_minute(&self) -> Option<f64> {
        if self.window_secs == 0 {
            return None;
        }
        Some(self.touched() as f64 * 60.0 / self.window_secs as f64)
    }

    /// The proportion of the earlier snapshot that was touched, from 0.0 to
    /// 1.0. `None` when the earlier snapshot was empty.
    ///
    /// A rate alone says nothing about scale: twenty files a minute is nothing
    /// in a source tree and most of a folder holding thirty photographs.
    pub fn share(&self) -> Option<f32> {
        let before = self.removed + self.modified + self.unchanged;
        if before == 0 {
            return None;
        }
        Some((self.removed + self.modified) as f32 / before as f32)
    }

    /// One line for a terminal or a log. States the counts, never a cause.
    pub fn describe(&self) -> String {
        let rate = match self.per_minute() {
            Some(rate) => format!("{rate:.0}/min"),
            None => "rate unknown (no time passed between the two looks)".to_string(),
        };
        let mut line = format!(
            "{} touched in {}s ({} added, {} removed, {} modified, {} unchanged) -- {rate}",
            self.touched(),
            self.window_secs,
            self.added,
            self.removed,
            self.modified,
            self.unchanged
        );
        if self.truncated {
            line.push_str(" -- counts are of what was looked at, not of the whole tree");
        }
        line
    }
}

/// When to raise the level, set by the user rather than guessed at here.
///
/// Both conditions must be met for [`Concern::High`]: a rate on its own has no
/// sense of scale, and a share on its own has no sense of time. Twenty files a
/// minute is nothing in a source tree; half a folder rewritten over a fortnight
/// is a fortnight of ordinary work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Threshold {
    /// Files touched per minute, above which this is worth mentioning.
    pub files_per_minute: f64,
    /// Proportion of the watched files touched, from 0.0 to 1.0.
    pub share: f32,
}

impl Default for Threshold {
    fn default() -> Self {
        // Deliberately unexciting numbers. They are a starting point for the
        // user to move, not a claim about what ransomware looks like -- there
        // is no such number, because the same figure is produced by a backup
        // restore.
        Self {
            files_per_minute: 60.0,
            share: 0.25,
        }
    }
}

/// How much of the threshold a [`Churn`] met.
///
/// Three levels rather than a boolean, and none of them is a verdict. The
/// highest is called [`Concern::High`] and not `Ransomware` on purpose: what
/// has been established is that a lot of files changed quickly, which is a
/// question for the user and not an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Concern {
    /// Neither condition met.
    Quiet,
    /// One of the two met.
    Elevated,
    /// Both met: a large share of the watched files, quickly.
    High,
}

impl Concern {
    /// A line for a front end, phrased as the question it actually is.
    pub fn describe(&self) -> &'static str {
        match self {
            Concern::Quiet => "nothing above the thresholds you set",
            Concern::Elevated => {
                "one of your two thresholds was passed. That is common during a \
                 backup, a large copy or a build; it is worth a glance, not an alarm."
            }
            Concern::High => {
                "a large share of the watched files changed quickly. If that was you \
                 -- a restore, an import, an extraction -- this is expected. If it \
                 was not, stop and look now."
            }
        }
    }
}

/// Judge a [`Churn`] against a [`Threshold`].
///
/// A churn whose window is zero cannot meet the rate condition at all, because
/// it has no rate. It can still meet the share condition, so the answer is at
/// most [`Concern::Elevated`] — which is the honest ceiling for a measurement
/// missing half its inputs.
pub fn concern(churn: &Churn, threshold: &Threshold) -> Concern {
    let fast = churn
        .per_minute()
        .map(|rate| rate >= threshold.files_per_minute)
        .unwrap_or(false);
    let broad = churn
        .share()
        .map(|share| share >= threshold.share)
        .unwrap_or(false);
    match (fast, broad) {
        (true, true) => Concern::High,
        (true, false) | (false, true) => Concern::Elevated,
        (false, false) => Concern::Quiet,
    }
}

/// Normalise a path for storage: forward slashes, as everywhere else here.
fn normalise(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn seconds(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

fn now_seconds() -> u64 {
    seconds(SystemTime::now()).unwrap_or(0)
}

impl Snapshot {
    /// Walk `root` and record what is there.
    ///
    /// Opens nothing: only directory entries and their metadata are read, which
    /// is what keeps this cheap enough to run on a timer. Unreadable
    /// directories are recorded in [`Snapshot::unreadable`] rather than failing
    /// the walk — a snapshot of nine folders out of ten is more useful than
    /// none, and the tenth is itself reported.
    pub fn take(root: &Path, limits: Limits) -> Result<Self, Error> {
        let mut files = BTreeMap::new();
        let mut unreadable = Vec::new();
        let mut truncated = false;

        // An explicit stack rather than recursion: a deep tree must not be able
        // to end this process by exhausting its stack, and a depth limit that
        // relies on the stack surviving to enforce it is not a limit.
        let mut stack = vec![(root.to_path_buf(), 0usize)];
        while let Some((directory, depth)) = stack.pop() {
            let entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) => {
                    unreadable.push(format!("{}: {error}", normalise(&directory)));
                    continue;
                }
            };
            for entry in entries {
                let Ok(entry) = entry else { continue };
                let path = entry.path();
                // `symlink_metadata`, never `metadata`: a link is recorded as
                // the link it is and never followed. See the module note.
                let Ok(meta) = std::fs::symlink_metadata(&path) else {
                    unreadable.push(format!("{}: metadata unavailable", normalise(&path)));
                    continue;
                };
                if meta.is_dir() {
                    if depth < limits.max_depth {
                        stack.push((path, depth + 1));
                    } else {
                        truncated = true;
                    }
                    continue;
                }
                if files.len() >= limits.max_files {
                    truncated = true;
                    continue;
                }
                files.insert(
                    normalise(&path),
                    Facts {
                        len: meta.len(),
                        modified: meta.modified().ok().and_then(seconds),
                    },
                );
            }
        }

        unreadable.sort();
        Ok(Self {
            root: normalise(root),
            taken: now_seconds(),
            truncated,
            unreadable,
            files,
        })
    }

    /// How many files were recorded.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The recorded paths and facts, in a stable order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &Facts)> {
        self.files
            .iter()
            .map(|(path, facts)| (path.as_str(), facts))
    }

    /// Replace the recorded time. For tests, which cannot wait a minute.
    #[doc(hidden)]
    pub fn with_taken(mut self, taken: u64) -> Self {
        self.taken = taken;
        self
    }

    /// Serialise to the text format described at the top of this module.
    pub fn to_text(&self) -> String {
        let mut out = String::from(MAGIC);
        out.push('\n');
        out.push_str(&format!("root  {}\n", self.root));
        out.push_str(&format!("taken  {}\n", self.taken));
        out.push_str(&format!("truncated  {}\n", self.truncated));
        for complaint in &self.unreadable {
            out.push_str(&format!("unreadable  {complaint}\n"));
        }
        for (path, facts) in &self.files {
            let modified = match facts.modified {
                Some(seconds) => seconds.to_string(),
                None => "-".to_string(),
            };
            out.push_str(&format!("file  {}  {}  {}\n", facts.len, modified, path));
        }
        out
    }

    /// Parse the text format.
    ///
    /// A line whose keyword is not recognised is an error rather than
    /// something to skip. A snapshot is a baseline: quietly dropping half of
    /// one produces a comparison against a tree that never existed, and every
    /// missing file reads as a deletion.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut lines = text.lines();
        match lines.next() {
            Some(first) if first.trim() == MAGIC => {}
            Some(other) => {
                return Err(Error::Malformed(format!(
                    "expected {MAGIC} on the first line, found {other:?}"
                )))
            }
            None => return Err(Error::Malformed("the snapshot file is empty".into())),
        }

        let mut root = None;
        let mut taken = None;
        let mut truncated = None;
        let mut unreadable = Vec::new();
        let mut files = BTreeMap::new();

        for (index, line) in lines.enumerate() {
            let number = index + 2;
            if line.trim().is_empty() {
                continue;
            }
            let Some((keyword, rest)) = line.split_once("  ") else {
                return Err(Error::Malformed(format!(
                    "line {number}: no keyword, found {line:?}"
                )));
            };
            match keyword {
                "root" => root = Some(rest.trim().replace('\\', "/")),
                "taken" => {
                    taken = Some(rest.trim().parse::<u64>().map_err(|_| {
                        Error::Malformed(format!("line {number}: bad time {rest:?}"))
                    })?)
                }
                "truncated" => {
                    truncated = Some(match rest.trim() {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(Error::Malformed(format!(
                                "line {number}: truncated is true or false, not {other:?}"
                            )))
                        }
                    })
                }
                "unreadable" => unreadable.push(rest.trim().to_string()),
                "file" => {
                    let mut parts = rest.splitn(3, "  ");
                    let len = parts.next().unwrap_or_default().trim();
                    let modified = parts.next().unwrap_or_default().trim();
                    let path = parts.next().unwrap_or_default().trim();
                    let len: u64 = len.parse().map_err(|_| {
                        Error::Malformed(format!("line {number}: bad length {len:?}"))
                    })?;
                    let modified = if modified == "-" {
                        None
                    } else {
                        Some(modified.parse::<u64>().map_err(|_| {
                            Error::Malformed(format!("line {number}: bad time {modified:?}"))
                        })?)
                    };
                    if path.is_empty() {
                        return Err(Error::Malformed(format!("line {number}: no path")));
                    }
                    // Normalised on the way in exactly as on the way out, so a
                    // file recorded on Windows is not reported as removed and
                    // re-added the first time the record is read back.
                    files.insert(path.replace('\\', "/"), Facts { len, modified });
                }
                other => {
                    return Err(Error::Malformed(format!(
                        "line {number}: unknown keyword {other:?}"
                    )))
                }
            }
        }

        Ok(Self {
            root: root.ok_or_else(|| Error::Malformed("no root line".into()))?,
            taken: taken.ok_or_else(|| Error::Malformed("no taken line".into()))?,
            truncated: truncated.ok_or_else(|| Error::Malformed("no truncated line".into()))?,
            unreadable,
            files,
        })
    }

    /// Write the snapshot to `path`.
    pub fn save(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.to_text())?;
        Ok(())
    }

    /// Read a snapshot written by [`Snapshot::save`].
    pub fn load(path: &Path) -> Result<Self, Error> {
        Self::parse(&std::fs::read_to_string(path)?)
    }
}

/// A stable filename for the saved snapshot of `root`.
///
/// Sixteen hex characters of the SHA-256 of the normalised path, plus `.txt`.
/// Two different roots colliding is then implausible rather than merely
/// unlikely -- and a collision would be one baseline silently overwriting
/// another, which shows up later as a whole tree appearing to have been
/// replaced.
///
/// Deriving the name rather than sanitising the path also keeps the watched
/// directory's name out of the state directory's listing, which is a small
/// courtesy on a shared machine.
pub fn baseline_name(root: &Path) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(normalise(root).as_bytes());
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{}.txt", &digest[..16])
}

/// Compare two snapshots of the same tree.
///
/// The order of the arguments is not checked against their timestamps, and the
/// window is a saturating subtraction, so passing them the wrong way round
/// gives a window of zero rather than a nonsense rate. A clock that went
/// backwards between the two produces the same thing, and for the same reason:
/// no rate is better than an invented one.
pub fn compare(before: &Snapshot, after: &Snapshot) -> Churn {
    let mut churn = Churn {
        added: 0,
        removed: 0,
        modified: 0,
        unchanged: 0,
        window_secs: after.taken.saturating_sub(before.taken),
        truncated: before.truncated || after.truncated,
    };
    for (path, was) in &before.files {
        match after.files.get(path) {
            None => churn.removed += 1,
            Some(now) if now == was => churn.unchanged += 1,
            Some(_) => churn.modified += 1,
        }
    }
    for path in after.files.keys() {
        if !before.files.contains_key(path) {
            churn.added += 1;
        }
    }
    churn
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    fn churn(before: usize, removed: usize, modified: usize, window: u64) -> Churn {
        Churn {
            added: 0,
            removed,
            modified,
            unchanged: before - removed - modified,
            window_secs: window,
            truncated: false,
        }
    }

    #[test]
    fn an_untouched_tree_shows_no_churn() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", b"one");
        write(dir.path(), "b.txt", b"two");
        let before = Snapshot::take(dir.path(), Limits::default()).unwrap();
        let after = Snapshot::take(dir.path(), Limits::default()).unwrap();
        let churn = compare(&before, &after);
        assert_eq!(churn.touched(), 0);
        assert_eq!(churn.unchanged, 2);
        assert!(!churn.truncated);
    }

    #[test]
    fn additions_removals_and_rewrites_are_counted_separately() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "keep.txt", b"unchanged");
        write(dir.path(), "gone.txt", b"about to go");
        write(dir.path(), "edit.txt", b"short");
        let before = Snapshot::take(dir.path(), Limits::default()).unwrap();

        std::fs::remove_file(dir.path().join("gone.txt")).unwrap();
        write(dir.path(), "edit.txt", b"a good deal longer than before");
        write(dir.path(), "new.txt", b"appeared");
        let after = Snapshot::take(dir.path(), Limits::default()).unwrap();

        let churn = compare(&before, &after);
        assert_eq!(churn.added, 1, "{churn:?}");
        assert_eq!(churn.removed, 1, "{churn:?}");
        assert_eq!(churn.modified, 1, "{churn:?}");
        assert_eq!(churn.unchanged, 1, "{churn:?}");
        assert_eq!(churn.touched(), 3);
    }

    #[test]
    fn subdirectories_are_walked_and_the_depth_limit_holds() {
        let dir = tempfile::tempdir().unwrap();
        let deep = dir.path().join("one").join("two").join("three");
        std::fs::create_dir_all(&deep).unwrap();
        write(&deep, "buried.txt", b"down here");
        write(dir.path(), "top.txt", b"up here");

        let all = Snapshot::take(dir.path(), Limits::default()).unwrap();
        assert_eq!(all.len(), 2, "the whole tree should be walked");
        assert!(!all.truncated);

        let shallow = Snapshot::take(
            dir.path(),
            Limits {
                max_depth: 1,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(shallow.len(), 1, "only the top file is within depth 1");
        assert!(shallow.truncated, "hitting the depth cap must be reported");
    }

    #[test]
    fn the_file_cap_truncates_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..10 {
            write(dir.path(), &format!("{index}.txt"), b"x");
        }
        let capped = Snapshot::take(
            dir.path(),
            Limits {
                max_files: 4,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(capped.len(), 4);
        assert!(capped.truncated);

        // And the flag survives into every report derived from it, so a front
        // end cannot present a partial count as a complete one.
        let churn = compare(&capped, &capped);
        assert!(churn.truncated);
        assert!(churn.describe().contains("what was looked at"));
    }

    #[test]
    fn a_zero_window_has_no_rate_rather_than_an_infinite_one() {
        let churn = churn(100, 50, 0, 0);
        assert_eq!(churn.per_minute(), None);
        assert!(churn.describe().contains("rate unknown"));
        // With no rate, the rate condition cannot be met, so the ceiling is
        // Elevated even though half the tree went.
        assert_eq!(
            concern(&churn, &Threshold::default()),
            Concern::Elevated,
            "a missing input must not produce the top level"
        );
    }

    #[test]
    fn the_rate_is_per_minute() {
        // 30 files in 30 seconds is 60 a minute.
        let churn = churn(100, 30, 0, 30);
        assert_eq!(churn.per_minute(), Some(60.0));
    }

    #[test]
    fn the_share_is_of_what_was_there_before() {
        let churn = churn(80, 10, 10, 60);
        assert_eq!(churn.share(), Some(0.25));
        // Added files are not part of the share: appearing is not the same as
        // being overwritten, and a download of a thousand files must not read
        // as a thousand files lost.
        let mut with_additions = churn;
        with_additions.added = 1000;
        assert_eq!(with_additions.share(), Some(0.25));
    }

    #[test]
    fn an_empty_before_has_no_share() {
        let churn = Churn {
            added: 5,
            removed: 0,
            modified: 0,
            unchanged: 0,
            window_secs: 1,
            truncated: false,
        };
        assert_eq!(churn.share(), None);
        assert_eq!(concern(&churn, &Threshold::default()), Concern::Elevated);
    }

    /// Both conditions, or it is not the top level.
    #[test]
    fn high_needs_both_speed_and_breadth() {
        let threshold = Threshold::default();
        // Fast, but a tiny share of a big tree.
        let fast_only = churn(10_000, 100, 0, 60);
        assert!(fast_only.per_minute().unwrap() >= threshold.files_per_minute);
        assert!(fast_only.share().unwrap() < threshold.share);
        assert_eq!(concern(&fast_only, &threshold), Concern::Elevated);

        // Broad, but over a fortnight.
        let broad_only = churn(100, 90, 0, 14 * 24 * 3600);
        assert!(broad_only.per_minute().unwrap() < threshold.files_per_minute);
        assert_eq!(concern(&broad_only, &threshold), Concern::Elevated);

        // Both.
        let both = churn(200, 150, 0, 60);
        assert_eq!(concern(&both, &threshold), Concern::High);

        // Neither.
        assert_eq!(concern(&churn(200, 1, 0, 3600), &threshold), Concern::Quiet);
    }

    /// The top level is a question put to the user, not a verdict about the
    /// world. If this wording ever becomes an accusation, the build fails.
    #[test]
    fn no_concern_level_names_a_cause() {
        for level in [Concern::Quiet, Concern::Elevated, Concern::High] {
            let text = level.describe().to_lowercase();
            for word in ["ransomware", "malware", "virus", "attack", "infected"] {
                assert!(!text.contains(word), "{level:?} says {word:?}: {text}");
            }
        }
        assert!(Concern::High.describe().contains("If that was you"));
        assert!(
            Concern::Elevated.describe().contains("backup"),
            "the innocent explanation must be offered first"
        );
    }

    /// A churn line reports counts and never a cause.
    #[test]
    fn the_churn_line_states_counts_and_no_conclusion() {
        let line = churn(100, 40, 10, 30).describe().to_lowercase();
        assert!(line.contains("50 touched"));
        for word in ["ransomware", "malware", "attack"] {
            assert!(!line.contains(word), "{line}");
        }
    }

    /// Swapped arguments, or a clock that stepped backwards, must not invent a
    /// rate out of a negative window.
    #[test]
    fn a_backwards_window_is_zero_rather_than_enormous() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", b"one");
        let early = Snapshot::take(dir.path(), Limits::default())
            .unwrap()
            .with_taken(1000);
        let late = Snapshot::take(dir.path(), Limits::default())
            .unwrap()
            .with_taken(2000);
        assert_eq!(compare(&late, &early).window_secs, 0);
        assert_eq!(compare(&early, &late).window_secs, 1000);
    }

    /// A directory that cannot be read is recorded, not skipped in silence: a
    /// folder that became unreadable is itself something that happened.
    #[test]
    fn a_snapshot_of_a_missing_root_reports_it_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("not-here");
        let snapshot = Snapshot::take(&missing, Limits::default()).unwrap();
        assert!(snapshot.is_empty());
        assert_eq!(snapshot.unreadable.len(), 1);
        assert!(snapshot.unreadable[0].contains("not-here"));
    }

    #[test]
    fn a_touched_timestamp_alone_counts_as_a_change() {
        let facts_now = Facts {
            len: 10,
            modified: Some(1000),
        };
        let later = Facts {
            len: 10,
            modified: Some(2000),
        };
        assert_ne!(facts_now, later, "same length, different time, is a change");
    }

    #[test]
    fn a_snapshot_survives_a_round_trip_through_text() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", b"one");
        write(dir.path(), "b b.txt", b"a name with a space in it");
        let snapshot = Snapshot::take(dir.path(), Limits::default()).unwrap();
        let text = snapshot.to_text();
        let read_back = Snapshot::parse(&text).expect("its own output must parse");
        assert_eq!(snapshot, read_back);
        assert_eq!(read_back.to_text(), text, "and byte for byte");
        // The whole point: a baseline read back compares as unchanged.
        assert_eq!(compare(&read_back, &snapshot).touched(), 0);
    }

    #[test]
    fn a_snapshot_survives_a_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.txt", b"one");
        let snapshot = Snapshot::take(dir.path(), Limits::default()).unwrap();
        let file = dir.path().join("state").join("baseline.txt");
        snapshot.save(&file).unwrap();
        assert_eq!(Snapshot::load(&file).unwrap(), snapshot);
    }

    /// A time the platform did not report is `-`, and comes back as `None`
    /// rather than as the first instant of 1970.
    #[test]
    fn an_unknown_modification_time_round_trips_as_unknown() {
        let text = format!("{MAGIC}\nroot  /x\ntaken  5\ntruncated  false\nfile  10  -  /x/a\n");
        let snapshot = Snapshot::parse(&text).unwrap();
        let (_, facts) = snapshot.files().next().unwrap();
        assert_eq!(facts.modified, None);
        assert!(snapshot.to_text().contains("file  10  -  /x/a"));
    }

    #[test]
    fn a_truncated_snapshot_says_so_after_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..5 {
            write(dir.path(), &format!("{index}.txt"), b"x");
        }
        let capped = Snapshot::take(
            dir.path(),
            Limits {
                max_files: 2,
                ..Limits::default()
            },
        )
        .unwrap();
        assert!(Snapshot::parse(&capped.to_text()).unwrap().truncated);
    }

    #[test]
    fn unreadable_directories_survive_the_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let snapshot = Snapshot::take(&dir.path().join("gone"), Limits::default()).unwrap();
        assert_eq!(snapshot.unreadable.len(), 1);
        assert_eq!(Snapshot::parse(&snapshot.to_text()).unwrap(), snapshot);
    }

    /// A baseline that half-parsed would compare against a tree that never
    /// existed, and every file it dropped would read as a deletion.
    #[test]
    fn a_malformed_snapshot_is_refused_rather_than_half_read() {
        assert!(Snapshot::parse("").is_err(), "empty");
        assert!(Snapshot::parse("NOT-THE-MAGIC\n").is_err(), "wrong magic");
        for bad in [
            "root  /x\ntaken  5",                                   // no truncated line
            "root  /x\ntruncated  false",                           // no taken line
            "taken  5\ntruncated  false",                           // no root line
            "root  /x\ntaken  x\ntruncated  false",                 // unparseable time
            "root  /x\ntaken  5\ntruncated  maybe",                 // not a boolean
            "root  /x\ntaken  5\ntruncated  false\nwhat  1",        // unknown keyword
            "root  /x\ntaken  5\ntruncated  false\nfile  x  1  /a", // bad length
            "root  /x\ntaken  5\ntruncated  false\nfile  1  1  ",   // no path
            "root  /x\ntaken  5\ntruncated  false\nnokeyword",      // no separator
        ] {
            let text = format!("{MAGIC}\n{bad}\n");
            assert!(
                Snapshot::parse(&text).is_err(),
                "should have been refused: {bad:?}"
            );
        }
    }

    #[test]
    fn a_windows_path_written_by_hand_reads_back_normalised() {
        let text = format!(
            "{MAGIC}\nroot  C:\\Users\\somebody\ntaken  5\ntruncated  false\n\
             file  10  20  C:\\Users\\somebody\\My Documents\\notes.txt\n"
        );
        let snapshot = Snapshot::parse(&text).unwrap();
        assert!(!snapshot.root.contains('\\'));
        let (path, _) = snapshot.files().next().unwrap();
        assert!(!path.contains('\\'), "{path}");
        assert!(path.contains("My Documents"), "{path}");
    }

    #[test]
    fn a_baseline_name_is_stable_and_distinct_per_root() {
        let one = baseline_name(Path::new("/home/somebody/Documents"));
        let two = baseline_name(Path::new("/home/somebody/Pictures"));
        assert_eq!(one, baseline_name(Path::new("/home/somebody/Documents")));
        assert_ne!(one, two);
        assert!(one.ends_with(".txt"));
        assert_eq!(one.len(), 20, "sixteen hex characters plus .txt");
        assert!(one[..16].bytes().all(|b| b.is_ascii_hexdigit()));
    }

    /// The same directory written with either separator is the same baseline,
    /// or a Windows user gets a fresh one every time the path is spelled the
    /// other way and never sees a comparison at all.
    #[test]
    fn a_baseline_name_ignores_the_separator_style() {
        assert_eq!(
            baseline_name(Path::new("C:/Users/somebody")),
            baseline_name(Path::new(r"C:\Users\somebody"))
        );
    }

    /// The watched directory's own name must not appear in the filename: on a
    /// shared machine the listing of the state directory would otherwise say
    /// what somebody is watching.
    #[test]
    fn a_baseline_name_does_not_leak_the_path() {
        let name = baseline_name(Path::new("/home/somebody/Very Private Folder"));
        assert!(!name.to_lowercase().contains("private"), "{name}");
        assert!(!name.contains("somebody"), "{name}");
    }

    #[test]
    fn the_defaults_are_the_documented_ones() {
        let limits = Limits::default();
        assert_eq!(limits.max_files, 20_000);
        assert_eq!(limits.max_depth, 12);
        let threshold = Threshold::default();
        assert_eq!(threshold.files_per_minute, 60.0);
        assert_eq!(threshold.share, 0.25);
    }
}
