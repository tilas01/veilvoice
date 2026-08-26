// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice privilege` — what VeilVoice is running with, and what it can see.
//!
//! # In plain words
//!
//! Most of VeilVoice needs no special permissions. The parts that watch your
//! machine see more when it is run as an administrator, and this says which of
//! those you are getting. It will not raise its own privileges for you — it
//! prints the command and you decide.

use crate::theme::{colour, field, heading, paint};

/// Report the privilege level and what it means.
pub fn show() -> Result<(), String> {
    let level = veilvoice_priv::level();

    println!("{}", heading("What VeilVoice is running with"));
    println!();
    println!("{}", field("running as", level.label()));
    println!();
    for line in crate::sentry::wrap(level.what_it_sees(), 72) {
        println!("  {line}");
    }

    if let Some(how) = level.how_to_raise() {
        println!();
        println!("{}", paint(colour::BLUE, "  To run it the other way"));
        for line in crate::sentry::wrap(how, 70) {
            println!("    {line}");
        }
    }

    println!();
    println!("{}", paint(colour::YELLOW, "WHAT THIS DOES NOT DO"));
    for note in [
        veilvoice_priv::NEVER_ELEVATES,
        veilvoice_priv::NO_SERVICE,
        veilvoice_priv::NO_KERNEL,
    ] {
        for line in crate::sentry::wrap(note, 72) {
            println!("  {line}");
        }
        println!();
    }
    Ok(())
}
