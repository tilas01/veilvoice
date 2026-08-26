// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice input` — what running programs can see your keyboard and mouse.
//!
//! The whole command is arranged around one risk: that somebody runs it, sees
//! nothing listed, and concludes their machine is clean. That conclusion is
//! wrong and acting on it makes them less safe than not having looked, so the
//! limits are printed **with** the result rather than behind a flag, and they
//! are printed whether anything was found or not.
//!
//! # In plain words
//!
//! This tells you which of the programs currently open on your computer are
//! able to see what you type or where you click. Most of them will be things
//! you installed on purpose — a password manager, a screen reader, remote
//! support software you use for work.
//!
//! It cannot tell you whether anything is actually recording your typing. No
//! program can, and this one says so every time rather than letting a short
//! list look like good news.

use crate::theme::{colour, field, heading, paint, warn};
use veilvoice_input::Reach;

/// Show what can see input on this machine.
pub fn look() -> Result<(), String> {
    let report = veilvoice_input::look();

    println!("{}", heading("What could see your keyboard and mouse"));
    println!();

    if !report.problems.is_empty() {
        for problem in &report.problems {
            println!("{}", warn(problem));
        }
        println!();
    }

    if report.findings.is_empty() {
        println!(
            "{}",
            paint(colour::MUTED, "  nothing this build recognises is running")
        );
        println!();
    }

    for finding in &report.findings {
        let watcher = finding.watcher;
        let tint = match watcher.reach {
            // Not red. A remote-support tool being open is information, not an
            // alarm, and colouring it as an alarm is how somebody learns to
            // ignore the colour.
            Reach::Purpose => colour::YELLOW,
            Reach::Incidental => colour::MUTED,
        };
        println!("{}", paint(tint, &format!("  {}", finding.phrasing())));
        println!("{}", field("    what it is", watcher.what));
        for line in crate::sentry::wrap(watcher.how, 66) {
            println!("      {line}");
        }
        println!();
    }

    println!("{}", paint(colour::YELLOW, "WHAT THIS CANNOT TELL YOU"));
    for line in crate::sentry::wrap(veilvoice_input::LIMITS, 72) {
        println!("  {line}");
    }
    println!();
    for line in crate::sentry::wrap(veilvoice_input::WHY_NOT_HOOKING, 72) {
        println!("  {line}");
    }
    println!();
    println!(
        "{}",
        paint(colour::MUTED, &format!("  {}", report.summary()))
    );
    Ok(())
}

/// Everything this build knows how to recognise, whether running or not.
///
/// Separate from [`look`] because they answer different questions, and because
/// a reader who gets an empty result deserves to be able to see *what* was
/// looked for rather than taking the emptiness on trust.
pub fn known() -> Result<(), String> {
    println!(
        "{}",
        heading("Programs this build can recognise as able to see input")
    );
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  This is not a list of keyloggers. Nearly all of it is software\n  \
             somebody installed on purpose."
        )
    );
    println!();

    for reach in [Reach::Purpose, Reach::Incidental] {
        let heading_text = match reach {
            Reach::Purpose => "Reading input is what these are for",
            Reach::Incidental => "These reach input to do their own job",
        };
        println!("{}", paint(colour::BLUE, &format!("  {heading_text}")));
        for watcher in veilvoice_input::ALL.iter().filter(|w| w.reach == reach) {
            println!("{}", field(&format!("    {}", watcher.name), watcher.what));
        }
        println!();
    }

    println!("{}", paint(colour::YELLOW, "WHAT THIS CANNOT TELL YOU"));
    for line in crate::sentry::wrap(veilvoice_input::LIMITS, 72) {
        println!("  {line}");
    }
    Ok(())
}
