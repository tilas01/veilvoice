// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice decoy` — what a second passphrase is worth, and what it is not.
//!
//! # In plain words
//!
//! Explains the decoy passphrase: what it does, the two things it cannot do,
//! and the rules a pair has to satisfy. It does not set one up. Choosing a
//! passphrase is done where passphrases are already handled, and printing one
//! into a terminal's history would be a poor start.

use crate::theme::{colour, field, heading, paint};

/// Explain the feature and its limits.
pub fn explain() -> Result<(), String> {
    println!("{}", heading("A second passphrase"));
    println!();
    for line in crate::sentry::wrap(veilvoice_decoy::SCOPE, 72) {
        println!("  {line}");
    }

    println!();
    println!("{}", paint(colour::YELLOW, "WHAT NO PASSPHRASE DOES"));
    for line in crate::sentry::wrap(veilvoice_decoy::WHY_NO_DESTRUCTION, 72) {
        println!("  {line}");
    }

    println!();
    println!("{}", heading("What a pair has to satisfy"));
    println!(
        "{}",
        field(
            "different in at least",
            &format!("{} places", veilvoice_decoy::LEAST_DIFFERENCE)
        )
    );
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  A decoy that is nearly the real passphrase is not a decoy. Somebody\n  \
             watching you type learns both at once, and somebody typing under\n  \
             pressure gives away the wrong one. Length counts as difference, so\n  \
             adding characters to the end does not make a second passphrase.",
        )
    );
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Both passphrases are checked with the same Argon2id cost and compared\n  \
             in constant time, and both are always derived even when the first one\n  \
             matches. Returning early would make the real passphrase measurably\n  \
             faster, which is enough to tell an observer which one was typed.",
        )
    );
    Ok(())
}
