// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice failsafe` is the safety catch. What it can and cannot do.
//!
//! # In plain words
//!
//! Shows whether the safety catch is on, what it would do, and what is holding
//! a microphone right now. It is the same guard the desktop application runs;
//! this is how to look at it from a terminal.
//!
//! The limits are printed every time, because the one thing a reader must not
//! come away believing is that this stops their computer handing a microphone
//! to another program. It notices, and it acts, and there is a moment between
//! the two.

use crate::theme::{colour, field, heading, paint, warn};
use veilvoice_failsafe::{Finding, Guard, Holder, Posture};

/// Show what Failsafe would make of this machine right now.
pub fn show(veiling: Option<&str>) -> Result<(), String> {
    println!("{}", heading("Failsafe"));
    println!();
    println!("{}", field("posture", Posture::default().label()));
    for line in crate::sentry::wrap(Posture::default().note(), 70) {
        println!("    {line}");
    }
    println!();

    // What holds a microphone, through the same feed the application uses.
    let (holders, problems) = microphone_holders();
    let mut guard = Guard::new();
    guard.posture = Posture::default();
    guard.veiling = veiling.map(str::to_string);
    // Asked as though veiling were running, because the question a reader has
    // at a terminal is "what would happen", and answering `Idle` whenever live
    // mode is not open would tell them nothing at all.
    guard.live = true;

    println!("{}", heading("What is holding a microphone"));
    if holders.is_empty() && problems.is_empty() {
        println!("{}", paint(colour::MUTED, "  nothing"));
    }
    for holder in &holders {
        let mine = veiling
            .map(|ours| {
                holder
                    .device
                    .as_deref()
                    .map(|d| d.to_lowercase().contains(&ours.to_lowercase()))
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        let tint = if mine { colour::MUTED } else { colour::YELLOW };
        let where_ = holder.device.clone().unwrap_or_else(|| "?".into());
        println!("{}", paint(tint, &format!("  {}  ({where_})", holder.app)));
    }
    for problem in &problems {
        println!("{}", warn(problem));
    }

    println!();
    let finding = guard.look(&holders, &problems);
    let tint = if finding.is_alarming() {
        colour::YELLOW
    } else {
        colour::MUTED
    };
    for line in crate::sentry::wrap(&finding.phrasing(), 70) {
        println!("{}", paint(tint, &format!("  {line}")));
    }

    println!();
    println!("{}", paint(colour::YELLOW, "WHAT THIS CANNOT DO"));
    for note in [
        veilvoice_failsafe::CANNOT_PREVENT,
        veilvoice_failsafe::NEVER_CLOSES,
    ] {
        for line in crate::sentry::wrap(note, 72) {
            println!("  {line}");
        }
        println!();
    }
    Ok(())
}

/// Everything holding a microphone, through the shared watch layer.
fn microphone_holders() -> (Vec<Holder>, Vec<String>) {
    let feed = veilvoice_watch::scan();
    let mut problems = Vec::new();
    let uses = match feed {
        Ok(uses) => uses,
        Err(error) => {
            problems.push(error.to_string());
            Vec::new()
        }
    };
    let holders = uses
        .into_iter()
        .filter(|use_| use_.kind == veilvoice_watch::DeviceKind::Microphone)
        .map(|use_| Holder {
            app: use_.app,
            pid: use_.pid,
            device: use_.device,
        })
        .collect();
    (holders, problems)
}

/// A named finding, for the tests to reach without a machine.
#[allow(dead_code)]
pub fn describe(finding: &Finding) -> String {
    finding.phrasing()
}
