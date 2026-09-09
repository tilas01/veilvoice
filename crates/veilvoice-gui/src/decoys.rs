// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoy vaults: how many there is room for, and the panel that offers them.
//!
//! # What a decoy is worth, said plainly
//!
//! A decoy is a directory that is the same shape as the real vault, holding
//! encrypted nonsense under a key that was thrown away the moment it was
//! written. Somebody who takes the disk sees several vaults and cannot tell
//! which one holds anything: the names say nothing, the sizes are the same, and
//! cracking one yields bytes that parse as nothing, which looks exactly like a
//! wrong passphrase.
//!
//! What that buys is the cost of searching. It does **not** hide the real vault
//! from somebody watching the screen while it is opened, from somebody who has
//! already got into the running program, or from a backup taken before the
//! decoys were made. The panel says all three where the button is, because a
//! defence somebody misjudges is worse than one they do not have.
//!
//! # Why the count is measured
//!
//! A number chosen here would be a guess dressed as advice: nine decoys is
//! careless on a nearly full laptop and timid on a four-terabyte disk. So the
//! count comes from the room actually free where the vault lives, and where the
//! system will not say how much that is, the panel says so instead of quietly
//! inventing a figure.

use egui::{RichText, Ui};
use veilvoice_crypto::studio::Shape;

use crate::theme::palette as p;

/// Never more than this share of what is free.
///
/// One twentieth. Decoys are not the only thing that wants the disk, and a
/// feature that filled it would cost somebody their next recording to protect
/// the ones they had already made.
const SHARE: u64 = 20;

/// Never more than this many, however much room there is.
///
/// Past here the work of searching stops rising in any way that matters: an
/// attacker willing to attack thirty-two vaults is willing to attack a hundred,
/// and the folder becomes something its owner cannot look at and understand.
const MOST: usize = 32;

/// What to offer, and on what evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Advice {
    /// What one decoy will occupy on the disk, in bytes.
    pub each: u64,
    /// How many the free space allows, or `None` where it could not be read.
    ///
    /// `None` is a measurement that did not happen, not a measurement of zero,
    /// and the panel says which.
    pub room: Option<usize>,
    /// The most this offers at once.
    pub most: usize,
    /// What the count starts at.
    pub count: usize,
}

/// Work out what to offer from the vault's shape and whatever the disk says.
///
/// Pure, so the arithmetic can be tested without a disk and without a window.
pub fn advise(shape: Shape, free: Option<u64>) -> Advice {
    // Never zero: a vault of no recordings is still an index file, so a decoy
    // of it still occupies something and the division below is always safe.
    let each = shape.bytes_on_disk().max(1);

    // `try_from` rather than `as`: on a 32-bit build a count that does not fit
    // in a `usize` must come back as the largest one that does, not as whatever
    // the low bits of it happen to be. It is capped again below in any case,
    // and relying on that would be relying on the wrong thing.
    let room = free.map(|free| usize::try_from((free / SHARE) / each).unwrap_or(usize::MAX));
    let most = room.unwrap_or(MOST).min(MOST);

    Advice {
        each,
        room,
        most,
        // From the measurement where there is one. Where there is not, one:
        // the smallest thing that is still a decoy, offered as a floor rather
        // than as advice, because advice needs a measurement behind it.
        count: match room {
            Some(room) => room.min(MOST),
            None => 1,
        },
    }
}

/// The panel, under the listing in the Browser.
///
/// Returns how many decoys were asked for, or `None` when nothing was asked.
/// Drawing and doing are separate because making a decoy writes files and the
/// caller owns the vault this is measuring.
pub fn panel(
    ui: &mut Ui,
    shape: Shape,
    free: Option<u64>,
    existing: usize,
    wanted: &mut usize,
) -> Option<usize> {
    let advice = advise(shape, free);
    *wanted = (*wanted).clamp(1, advice.most.max(1));
    let mut asked = None;

    egui::CollapsingHeader::new("Decoy vaults")
        .id_salt("studio-decoys")
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "A decoy is a vault whose contents never existed. It is filled with \
                     encrypted nonsense under a key made here and dropped before this \
                     finishes, so there is no key to find, to leak, or to be made to hand \
                     over. Cracked, it yields bytes that parse as nothing.",
                )
                .color(p::muted())
                .small(),
            );
            ui.add_space(6.0);

            ui.label(
                RichText::new(format!(
                    "This folder holds {}. Each new one will take {}.",
                    counted_vaults(existing),
                    crate::studio::size(advice.each as usize)
                ))
                .color(p::blue())
                .small(),
            );
            match advice.room {
                Some(room) => ui.label(
                    RichText::new(format!(
                        "There is room for {room} at a twentieth of the free space, so that \
                         is what this starts on.",
                    ))
                    .color(p::muted())
                    .small(),
                ),
                None => ui.label(
                    RichText::new(
                        "This system would not say how much room is free, so the number \
                         below is a starting point rather than a measurement. Check there \
                         is room before making many.",
                    )
                    .color(p::yellow())
                    .small(),
                ),
            };

            ui.add_space(8.0);
            // Room for none is offered as none rather than as one anyway. A
            // decoy written onto a disk with nothing left on it is how somebody
            // loses the recording they make next.
            let possible = advice.most > 0;
            ui.add_enabled_ui(possible, |ui| {
                ui.horizontal(|ui| {
                    ui.label("how many  ");
                    ui.add(
                        egui::Slider::new(wanted, 1..=advice.most.max(1))
                            .clamping(egui::SliderClamping::Always),
                    );
                });
                ui.add_space(6.0);
                if ui.button(RichText::new("  make them  ").strong()).clicked() {
                    asked = Some(*wanted);
                }
            });
            if !possible {
                ui.label(
                    RichText::new(
                        "There is not room here for one, so none is offered. Free some \
                         space and open this again.",
                    )
                    .color(p::red())
                    .small(),
                );
            }

            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "What this buys is the cost of a search. It does not hide the real \
                     vault from somebody watching you open it, from something already \
                     running inside this computer, or from a backup taken before the \
                     decoys were made.",
                )
                .color(p::yellow())
                .small(),
            );
            ui.label(
                RichText::new(
                    "Decoys cannot be told from the real vault by looking, which means \
                     you cannot tell them apart either. Removing one is removing a \
                     directory you cannot open to check first.",
                )
                .color(p::muted())
                .small(),
            );
        });

    asked
}

/// "one vault" or "four vaults", so the panel does not say "1 vaults".
fn counted_vaults(n: usize) -> String {
    match n {
        0 => "no vaults".to_string(),
        1 => "one vault".to_string(),
        n => format!("{n} vaults"),
    }
}

#[cfg(test)]
#[path = "decoys/tests.rs"]
mod tests;
