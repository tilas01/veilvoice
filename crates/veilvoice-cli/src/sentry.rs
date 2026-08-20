// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice sentry` -- canaries, baselines, and what changed since.
//!
//! The command-line front end to [`veilvoice_sentry`]. All of the logic is in
//! that crate; this file decides where the state lives, prints it, and chooses
//! an exit code.
//!
//! # Where the state lives
//!
//! Beside the app lock, under the platform's usual per-user configuration
//! directory:
//!
//! ```text
//! <config>/veilvoice/sentry/nest.txt      the planted canaries
//! <config>/veilvoice/sentry/<16 hex>.txt  one baseline per watched directory
//! ```
//!
//! Baselines are named from a digest of the directory they describe, so two
//! directories cannot silently overwrite each other's baseline and the state
//! directory's listing does not say what somebody is watching. Each file
//! records its own root, so `check` reads them rather than needing an index.
//!
//! # The exit code answers one question and not the other
//!
//! `veilvoice sentry check` exits non-zero when **a canary tripped**, because
//! that is a fact: a file nothing uses was changed, moved or removed. It exits
//! zero for churn at any level, however high, because churn is a question --
//! a backup restore produces the same numbers as anything else, and a command
//! that fails a scheduled task every time somebody copies a folder is a
//! command somebody removes from the scheduled task.
//!
//! # This detects, and stops nothing
//!
//! [`veilvoice_sentry::SCOPE`] is printed by `status` rather than paraphrased
//! here, so there is one wording and the tests guard it.

use crate::theme::{colour, err, field, heading, ok, paint, warn};
use std::path::{Path, PathBuf};
use veilvoice_sentry::canary::Nest;
use veilvoice_sentry::rate::{self, Concern, Limits, Snapshot, Threshold};

/// Where the canaries and baselines are kept.
///
/// Derived from the app lock's location rather than resolved again, so there
/// is one answer to "where does VeilVoice keep things" and it cannot drift.
pub fn state_dir() -> Option<PathBuf> {
    veilvoice_crypto::lock::default_path().map(|lock| lock.with_file_name("").join("sentry"))
}

fn nest_path() -> Result<PathBuf, String> {
    Ok(state_dir()
        .ok_or_else(|| {
            "this platform did not say where to keep configuration (no APPDATA, \
             XDG_CONFIG_HOME or HOME), so there is nowhere to record canaries"
                .to_string()
        })?
        .join("nest.txt"))
}

/// Read the nest, treating "no file yet" as "nothing planted".
///
/// A missing file is the ordinary state before anything is planted and must
/// not read as an error. A file that exists and will not parse is a different
/// matter and is reported: quietly starting again from an empty nest would
/// lose the record of every canary and report none of them as gone.
fn load_nest() -> Result<Nest, String> {
    let path = nest_path()?;
    match Nest::load(&path) {
        Ok(nest) => Ok(nest),
        Err(veilvoice_sentry::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(Nest::new())
        }
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn save_nest(nest: &Nest) -> Result<(), String> {
    let path = nest_path()?;
    nest.save(&path)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

/// Every saved baseline, with the path it came from.
fn baselines() -> Result<Vec<(PathBuf, Snapshot)>, String> {
    let Some(dir) = state_dir() else {
        return Ok(Vec::new());
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read {}: {error}", dir.display())),
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .map(|name| name == "nest.txt")
            .unwrap_or(false)
        {
            continue;
        }
        if path.extension().map(|ext| ext != "txt").unwrap_or(true) {
            continue;
        }
        match Snapshot::load(&path) {
            Ok(snapshot) => found.push((path, snapshot)),
            // Reported, not skipped: a baseline that will not parse means the
            // next comparison for that directory silently does not happen.
            Err(error) => {
                println!("{}", err(&format!("{}: {error}", path.display())));
            }
        }
    }
    found.sort_by(|a, b| a.1.root.cmp(&b.1.root));
    Ok(found)
}

/// What is planted, what is watched, and what this is worth.
pub fn status() -> Result<(), String> {
    println!("{}", heading("Sentry"));
    let nest = load_nest()?;
    println!("{}", field("canaries planted", &nest.len().to_string()));
    let watched = baselines()?;
    println!(
        "{}",
        field("directories watched", &watched.len().to_string())
    );
    println!();

    if nest.is_empty() && watched.is_empty() {
        println!("  Nothing is set up yet.");
        println!();
        println!("    veilvoice sentry plant <DIR>      put a canary in a folder");
        println!("    veilvoice sentry baseline <DIR>   record what a folder holds now");
        println!("    veilvoice sentry check            look at both");
        println!();
    }

    for canary in nest.canaries() {
        println!("{}", field("canary", &canary.path));
    }
    for (_, snapshot) in &watched {
        println!(
            "{}",
            field(
                "watching",
                &format!("{} ({} files)", snapshot.root, snapshot.len())
            )
        );
    }

    println!();
    println!("{}", paint(colour::YELLOW, "WHAT THIS IS WORTH"));
    for line in wrap(veilvoice_sentry::SCOPE, 72) {
        println!("  {line}");
    }
    Ok(())
}

/// Put a canary in `dir`.
pub fn plant(dir: &Path, name: Option<&str>) -> Result<(), String> {
    println!("{}", heading("Plant a canary"));
    let mut nest = load_nest()?;
    let path = nest
        .plant(dir, name)
        .map_err(|error| format!("could not plant a canary: {error}"))?;
    save_nest(&nest)?;
    println!("{}", ok(&format!("planted {}", path.display())));
    println!();
    println!("  Nothing reads that file. If it ever changes, VeilVoice will say so.");
    println!("  Remove it with `veilvoice sentry pull-up` rather than deleting it,");
    println!("  or the deletion is itself reported as a change.");
    println!();
    println!(
        "{}",
        warn(
            "A canary only fires if whatever is running reaches that folder. A quiet \
             canary is not evidence that nothing happened."
        )
    );
    Ok(())
}

/// Stop watching a canary, and delete it.
pub fn pull_up(path: &Path) -> Result<(), String> {
    println!("{}", heading("Pull up a canary"));
    let mut nest = load_nest()?;
    nest.pull_up(path)
        .map_err(|error| format!("could not pull it up: {error}"))?;
    save_nest(&nest)?;
    println!("{}", ok(&format!("removed {}", path.display())));
    Ok(())
}

/// Record what `dir` holds now, as the thing to compare against later.
pub fn baseline(dir: &Path, limits: Limits) -> Result<(), String> {
    println!("{}", heading("Record a baseline"));
    let Some(state) = state_dir() else {
        return Err("this platform did not say where to keep configuration".to_string());
    };
    let snapshot = Snapshot::take(dir, limits)
        .map_err(|error| format!("could not read {}: {error}", dir.display()))?;
    let path = state.join(rate::baseline_name(dir));
    snapshot
        .save(&path)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;

    println!("{}", field("directory", &snapshot.root));
    println!("{}", field("files recorded", &snapshot.len().to_string()));
    if snapshot.truncated {
        println!(
            "{}",
            warn(
                "a limit was reached, so this records what was looked at rather than \
                 everything that is there"
            )
        );
    }
    for complaint in &snapshot.unreadable {
        println!("{}", warn(&format!("could not read {complaint}")));
    }
    println!();
    println!("{}", ok("baseline recorded"));
    println!("  Run `veilvoice sentry check` later to see what changed since.");
    Ok(())
}

/// Look at every canary and every baseline.
///
/// Returns `true` when a canary tripped, which is what the exit code reports.
/// Churn never sets it -- see the note at the top of this file.
pub fn check(threshold: Threshold, limits: Limits) -> Result<bool, String> {
    println!("{}", heading("Sentry check"));
    let nest = load_nest()?;
    let watched = baselines()?;

    if nest.is_empty() && watched.is_empty() {
        println!("{}", warn("nothing is planted and nothing is watched"));
        println!("  `veilvoice sentry status` says how to set either up.");
        return Ok(false);
    }

    let mut tripped = false;
    if !nest.is_empty() {
        println!("{}", paint(colour::BLUE, "CANARIES"));
        for sighting in nest.check() {
            let line = format!("{}: {}", sighting.canary.path, sighting.state.describe());
            if sighting.state.is_trip() {
                tripped = true;
                println!("{}", err(&line));
            } else {
                println!("{}", ok(&line));
            }
        }
        println!();
    }

    for (path, before) in &watched {
        println!("{}", paint(colour::BLUE, &before.root));
        let after = match Snapshot::take(Path::new(&before.root), limits) {
            Ok(after) => after,
            Err(error) => {
                println!("{}", err(&format!("could not read it now: {error}")));
                continue;
            }
        };
        let churn = rate::compare(before, &after);
        println!("  {}", churn.describe());
        let level = rate::concern(&churn, &threshold);
        let line = format!("  {}", rate::Concern::describe(&level));
        match level {
            Concern::Quiet => println!("{}", paint(colour::GREEN, &line)),
            Concern::Elevated => println!("{}", paint(colour::YELLOW, &line)),
            Concern::High => println!("{}", paint(colour::RED, &line)),
        }
        println!(
            "  {}",
            paint(
                colour::MUTED,
                &format!(
                    "baseline recorded {}s ago in {}",
                    churn.window_secs,
                    path.display()
                )
            )
        );
        println!();
    }

    if tripped {
        println!(
            "{}",
            err(
                "A canary changed. That means something wrote to a file nothing uses. \
                 It does not say what, and nothing here stopped it."
            )
        );
    }
    Ok(tripped)
}

/// Wrap `text` to `width` columns on spaces, for the scope note.
///
/// A paragraph printed as one line is a paragraph nobody reads in an
/// eighty-column terminal, and the scope note is the paragraph here that most
/// needs reading.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_directory_sits_beside_the_app_lock() {
        let Some(dir) = state_dir() else {
            return; // a platform with nowhere to keep configuration
        };
        assert!(dir.ends_with("sentry"), "{}", dir.display());
        let lock = veilvoice_crypto::lock::default_path().unwrap();
        assert_eq!(
            dir.parent(),
            lock.parent(),
            "the sentry directory must live beside the lock, not somewhere new"
        );
    }

    /// Baselines are found by their own recorded root, so the filename never
    /// has to be reversed back into a path.
    #[test]
    fn a_baseline_filename_is_derived_and_not_the_path() {
        let name = rate::baseline_name(Path::new("/home/somebody/Documents"));
        assert!(!name.contains("Documents"));
        assert!(name.ends_with(".txt"));
    }

    #[test]
    fn wrapping_keeps_every_word_and_respects_the_width() {
        let lines = wrap(veilvoice_sentry::SCOPE, 72);
        assert!(lines.len() > 1, "the scope note is longer than one line");
        for line in &lines {
            assert!(line.len() <= 72, "too long: {line:?}");
        }
        let rejoined = lines.join(" ");
        let original: Vec<&str> = veilvoice_sentry::SCOPE.split_whitespace().collect();
        assert_eq!(rejoined.split_whitespace().collect::<Vec<_>>(), original);
    }

    #[test]
    fn wrapping_handles_nothing_and_one_long_word() {
        assert!(wrap("", 10).is_empty());
        assert_eq!(
            wrap("supercalifragilistic", 5),
            vec!["supercalifragilistic"]
        );
    }
}
