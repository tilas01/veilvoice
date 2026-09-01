// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice appctl` learns what normally runs, so it can notice what does not.
//!
//! Every subcommand prints the scope note. Not once at setup, not behind a
//! flag: **every time**, because the one thing a reader must not come away
//! believing is that this stopped something. It did not and it cannot, and a
//! warning shown once is a warning forgotten by the second week.
//!
//! # In plain words
//!
//! Run `learn` for a while and VeilVoice writes down which programs you
//! normally use. Run `check` afterwards and it tells you about anything running
//! that was not on that list.
//!
//! It does not block anything. It is a way of noticing.

use crate::theme::{colour, field, heading, paint, warn};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use veilvoice_appctl::{Baseline, Grant, Verdict};

/// Where the baseline is kept.
pub fn baseline_path() -> Result<PathBuf, String> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
    };
    let base = base.ok_or_else(|| {
        "this system offers no per-user configuration directory, so there is nowhere \
         to keep a baseline"
            .to_string()
    })?;
    Ok(base.join("veilvoice").join("appctl.conf"))
}

fn load(path: &Path) -> Result<Baseline, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Baseline::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Baseline::new()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn save(path: &Path, baseline: &Baseline) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    // F-75. Readable by this account and no other.
    //
    // This file decides what counts as ordinary on this machine, which makes it
    // a security setting rather than a convenience. Written with the default
    // permissions, another local account could add a line and have a program of
    // their choosing treated as unremarkable for ever, or read the list to learn
    // exactly what runs here and when.
    //
    // The project already has one place that gets file permissions right, and
    // the important part is that it sets them **as the file is created** rather
    // than afterwards: a file that exists for even a moment with the wrong
    // permissions is a file somebody else's program may have read in that
    // moment.
    veilvoice_crypto::privatefile::write_owner_only(path, baseline.to_text().as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The note that goes with every answer.
fn scope() {
    println!();
    println!("{}", paint(colour::YELLOW, "WHAT THIS DOES NOT DO"));
    for line in crate::sentry::wrap(veilvoice_appctl::SCOPE, 72) {
        println!("  {line}");
    }
}

/// What is running now, through the shared listing.
fn running() -> (Vec<String>, Vec<String>) {
    veilvoice_proc::running()
}

/// Record what is running as ordinary.
pub fn learn(finish: bool) -> Result<(), String> {
    let path = baseline_path()?;
    let mut baseline = load(&path)?;

    if finish {
        let count = baseline.freeze().map_err(|e| format!("{e}"))?;
        save(&path, &baseline)?;
        println!("{}", heading("Baseline closed"));
        println!("{}", field("programs recorded", &count.to_string()));
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Nothing joins the baseline by running from now on. That is the point \
                 of the phase having an end: a baseline that is always learning has \
                 learned nothing, because whatever starts becomes part of the picture \
                 the moment it starts.",
            )
        );
        scope();
        return Ok(());
    }

    if !baseline.is_learning() {
        let mut fresh = Baseline::learning();
        std::mem::swap(&mut baseline, &mut fresh);
    }

    let (names, problems) = running();
    for problem in &problems {
        println!("{}", warn(problem));
    }
    let seen = baseline.observe(&names, SystemTime::now());
    save(&path, &baseline)?;

    println!("{}", heading("Learning what is ordinary here"));
    println!("{}", field("seen this run", &seen.len().to_string()));
    println!("{}", field("in the baseline", &baseline.len().to_string()));
    println!("{}", field("kept in", &path.display().to_string()));
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Run this again over the next few days while you work normally, then\n  \
             `veilvoice appctl learn --finish` to close it.",
        )
    );
    scope();
    Ok(())
}

/// Compare what is running against the baseline.
pub fn check() -> Result<(), String> {
    let path = baseline_path()?;
    let mut baseline = load(&path)?;
    let now = SystemTime::now();

    if baseline.is_empty() && !baseline.is_learning() {
        println!("{}", heading("No baseline yet"));
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  `veilvoice appctl learn` records what normally runs here. Until then \
                 there is nothing to compare against, and this will not guess.",
            )
        );
        scope();
        return Ok(());
    }

    let (names, problems) = running();
    for problem in &problems {
        println!("{}", warn(problem));
    }
    baseline.observe(&names, now);
    save(&path, &baseline)?;

    println!(
        "{}",
        heading("What is running that the baseline does not know")
    );
    if baseline.is_learning() {
        println!(
            "{}",
            paint(
                colour::YELLOW,
                "  The baseline is still learning, so nothing can be called unknown yet.",
            )
        );
        scope();
        return Ok(());
    }

    let unknown = baseline.unknown(&names, now);
    if unknown.is_empty() {
        println!(
            "{}",
            paint(colour::MUTED, "  nothing outside the baseline is running")
        );
    }
    for program in &unknown {
        println!("{}", paint(colour::YELLOW, &format!("  {program}")));
        println!("      {}", Verdict::Unknown.phrasing());
    }

    // Grants in force, so somebody can see what they allowed and until when.
    let granted: Vec<&String> = names
        .iter()
        .filter(|name| baseline.verdict(name, now) == Verdict::Granted)
        .collect();
    if !granted.is_empty() {
        println!();
        println!("{}", paint(colour::BLUE, "  Allowed by you, and running"));
        for program in granted {
            let how_long = baseline
                .grant(program)
                .map(|g| g.describe(now))
                .unwrap_or_default();
            println!("{}", field(&format!("    {program}"), &how_long));
        }
    }
    scope();
    Ok(())
}

/// Allow a program, for a while or for good.
pub fn allow(program: &str, hours: Option<u64>) -> Result<(), String> {
    let path = baseline_path()?;
    let mut baseline = load(&path)?;
    let now = SystemTime::now();

    let grant = match hours {
        Some(hours) => {
            Grant::for_duration(now, Duration::from_secs(hours * 3_600)).ok_or_else(|| {
                format!("{hours} hours runs off the end of the clock; use --forever if you mean it")
            })?
        }
        None => Grant::forever(),
    };
    baseline.allow(program, grant).map_err(|e| format!("{e}"))?;
    save(&path, &baseline)?;

    println!("{}", heading("Allowed"));
    println!("{}", field(program, &grant.describe(now)));
    if hours.is_none() {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  Permanent. A list that only ever grows stops meaning anything, so \
                 prefer `--hours` unless you mean this one for good.",
            )
        );
    }
    scope();
    Ok(())
}

/// Withdraw a grant.
pub fn revoke(program: &str) -> Result<(), String> {
    let path = baseline_path()?;
    let mut baseline = load(&path)?;
    baseline.revoke(program);
    save(&path, &baseline)?;
    println!("{}", heading("Withdrawn"));
    println!("{}", field(program, "no longer allowed"));
    scope();
    Ok(())
}

/// Show the decision log.
pub fn log() -> Result<(), String> {
    let path = baseline_path()?;
    let baseline = load(&path)?;
    println!("{}", heading("Every decision this baseline has made"));
    if baseline.log().is_empty() {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  nothing yet. Only the decisions worth reading are recorded: a line \
                 for every ordinary program every time it is seen is a log nobody \
                 reads, and a log nobody reads is not a control.",
            )
        );
    }
    for entry in baseline.log() {
        let verdict = match entry.verdict {
            Verdict::Unknown => paint(colour::YELLOW, "unknown"),
            Verdict::Granted => paint(colour::MUTED, "allowed"),
            other => paint(
                colour::MUTED,
                if other == Verdict::Known {
                    "known"
                } else {
                    "learning"
                },
            ),
        };
        println!("  {}  {verdict}  {}", entry.at, entry.program);
    }
    scope();
    Ok(())
}
