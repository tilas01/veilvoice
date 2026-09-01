// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice accel` reports the graphics hardware here, and what it is good for.
//!
//! # In plain words
//!
//! Lists the graphics devices on this computer and says which of them can
//! encode video, so you can pick one when making a video of a conversation.
//!
//! It also says, with the measurement behind it, why the voice changing itself
//! does not use a graphics card: it is already about a hundred times faster
//! than real time, and moving that work onto a card would slow it down.

use crate::theme::{colour, field, heading, paint, warn};

/// Show what this machine has.
pub fn show() -> Result<(), String> {
    let found = veilvoice_accel::look();

    println!("{}", heading("Graphics hardware on this machine"));
    println!();
    println!(
        "{}",
        field(
            "threads available",
            &veilvoice_accel::usable_threads().to_string()
        )
    );
    println!();

    if found.adapters.is_empty() && found.problems.is_empty() {
        println!(
            "{}",
            paint(colour::MUTED, "  no graphics devices were listed")
        );
    }
    for adapter in &found.adapters {
        let tint = if adapter.encoder().is_some() {
            colour::BLUE
        } else {
            colour::MUTED
        };
        println!("{}", paint(tint, &format!("  {}", adapter.describe())));
        if let Some(encoder) = adapter.encoder() {
            println!("{}", field("    ffmpeg encoder", encoder));
        }
    }
    for problem in &found.problems {
        println!("{}", warn(problem));
    }

    println!();
    match found.why_recommended() {
        Some(why) => {
            for line in crate::sentry::wrap(&why, 70) {
                println!("  {line}");
            }
        }
        None => println!(
            "{}",
            paint(
                colour::MUTED,
                "  Nothing here has an encoder this build recognises, so videos are \
                 written by the software encoder.",
            )
        ),
    }

    if let Some(first) = found.adapters.first() {
        println!();
        for line in crate::sentry::wrap(first.caveat(), 70) {
            println!("  {line}");
        }
    }

    println!();
    println!(
        "{}",
        paint(colour::YELLOW, "WHAT A GRAPHICS CARD DOES NOT DO HERE")
    );
    for note in [
        veilvoice_accel::WHY_NOT_THE_ENGINE,
        veilvoice_accel::WHAT_IT_CHANGES,
    ] {
        for line in crate::sentry::wrap(note, 72) {
            println!("  {line}");
        }
        println!();
    }
    Ok(())
}
