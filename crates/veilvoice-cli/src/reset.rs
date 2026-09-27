// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice reset`: putting this machine back to a new install.
//!
//! # Why it is here as well as in the window
//!
//! A policy or a mandate set from the command line has to be removable from the
//! command line. Somebody who fixed a machine with `veilvoice policy` should not
//! have to open a window to undo it, and a machine that has no display -- a
//! server, a container, somebody's headless box -- has no window to open.
//!
//! The deciding and the removing are [`veilvoice_crypto::reset`]'s, so this and
//! the window remove the same things, in the same order, and describe them in
//! the same words. This module is the part that prints, asks and counts.
//!
//! # Nothing is removed until it has been read
//!
//! The plan is printed first, every time, with a line per thing and the size of
//! it. Then:
//!
//! - `--dry-run` prints and stops. The default for somebody who wants to look.
//! - `--yes` is enough where nothing in the plan is irreplaceable.
//! - Where something is, the word `RESET` has to be typed, exactly as
//!   `veilvoice shred` asks for `DESTROY`. `--yes` is deliberately not accepted
//!   in its place: a flag is what somebody leaves in a script and forgets, and
//!   this is the one command where forgetting costs recordings.
//! - `--confirm RESET` is that typed word, for a machine with nothing to type
//!   into. A server being rebuilt has no terminal and still has to be
//!   resettable. It is a separate flag carrying the word itself rather than a
//!   second meaning for `--yes`, so it cannot be arrived at by copying the
//!   flags off another command.
//!
//! # In plain words
//!
//! Puts VeilVoice back to how it was the day you installed it. It shows you
//! what it is about to remove before it removes anything, and `--keep-keys`
//! leaves your password and the key to your recordings alone.

use crate::theme::{colour, err, field, heading, ok, paint, warn};
use std::io::IsTerminal;
use veilvoice_crypto::reset::{self, Keep, Plan};

/// The word somebody types where the plan cannot be undone.
///
/// Data rather than an inline literal, so the test suite can assert that the
/// prompt still asks for it and that nothing else is accepted.
pub const TYPED: &str = "RESET";

/// Run the reset.
pub fn run(keep_keys: bool, dry_run: bool, yes: bool, confirm: Option<&str>) -> Result<(), String> {
    println!("{}", heading("Start again"));

    let keep = if keep_keys { Keep::Keys } else { Keep::Nothing };
    let plan = reset::plan(keep).map_err(|_| {
        "this platform did not say where to keep configuration, so there is no \
         folder to reset"
            .to_string()
    })?;

    println!("{}", field("folder", &plan.dir.display().to_string()));
    println!();

    if plan.is_empty() {
        println!(
            "{}",
            ok("there is nothing to remove: this is already a new install")
        );
        report_kept(&plan);
        return Ok(());
    }

    describe(&plan);

    if dry_run {
        println!();
        println!("{}", ok("nothing was touched: this was a dry run"));
        return Ok(());
    }

    if !agreed(&plan, yes, confirm)? {
        return Err("cancelled, and nothing was touched".to_string());
    }

    let report = reset::carry_out(&plan);
    println!();
    for label in &report.removed {
        println!("{}", ok(&format!("removed {label}")));
    }
    for line in &report.refused {
        println!("{}", err(&line.to_string()));
    }
    if !report.complete() {
        return Err("some of it is still there, and the lines above say which".to_string());
    }

    report_kept(&plan);
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  The next run starts as a first run. Nothing was sent anywhere and\n  \
             nothing outside that folder was touched: recordings you have saved\n  \
             elsewhere, and this program itself, are where they were."
        )
    );
    Ok(())
}

/// Print the plan: a line per thing, and what it adds up to.
fn describe(plan: &Plan) {
    println!("{}", paint(colour::YELLOW, "THIS WOULD BE REMOVED"));
    for going in &plan.going {
        println!(
            "{}",
            field(
                &going.label,
                &format!("{}, {}", count(going.files), size(going.bytes))
            )
        );
        for line in crate::sentry::wrap(&going.note, 66) {
            println!("{}", paint(colour::MUTED, &format!("    {line}")));
        }
        if going.irreplaceable {
            println!("{}", paint(colour::YELLOW, "    nothing can put this back"));
        }
    }

    println!();
    println!(
        "{}",
        field(
            "in total",
            &format!("{}, {}", count(plan.files()), size(plan.bytes()))
        )
    );

    report_kept(plan);

    if let Some(path) = &plan.beyond_reach {
        println!();
        println!(
            "{}",
            warn("a second copy of the app lock is somewhere this cannot reach")
        );
        println!("{}", field("still there", &path.display().to_string()));
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  An administrator wrote it, so removing it needs the same privilege.\n  \
                 Left alone it restores the app lock by itself on the next launch,\n  \
                 which is the whole point of the second copy and is not what you\n  \
                 asked for here. Remove that file as an administrator, or run this\n  \
                 command as one."
            )
        );
    }
}

/// Print what stays, when anything does.
fn report_kept(plan: &Plan) {
    if plan.keeping.is_empty() {
        return;
    }
    println!();
    println!("{}", paint(colour::CYAN, "THIS STAYS"));
    for (label, path) in &plan.keeping {
        println!("{}", field(label, &path.display().to_string()));
    }
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Your password on VeilVoice, and the key to anything sealed with it,\n  \
             are exactly as they were."
        )
    );
}

/// Whether to go ahead: a flag where that is enough, a typed word where it is
/// not.
///
/// The distinction is the plan's own: [`Plan::irreplaceable`] is true where
/// something in it has no default to come back to and no copy anywhere. A
/// person resetting a folder of settings twice in an afternoon is not asked to
/// type anything; a person about to lose recordings is.
fn agreed(plan: &Plan, yes: bool, confirm: Option<&str>) -> Result<bool, String> {
    if !plan.irreplaceable() {
        if yes {
            return Ok(true);
        }
        println!();
        println!("  Nothing here is irreplaceable: every one of those has a default");
        println!("  to come back to. Re-run with --yes to proceed.");
        return Ok(false);
    }

    println!();
    println!("{}", err("THIS CANNOT BE UNDONE."));
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  The files are deleted rather than overwritten. On an SSD, SD card or\n  \
             USB stick, overwriting cannot promise the old blocks are gone either,\n  \
             which is why this does not spend an hour pretending to: full-volume\n  \
             encryption is the answer to somebody holding the disk."
        )
    );
    println!();

    // The word given as an argument, for a machine with no terminal. Compared
    // against the same constant the prompt asks for, so there is one word and
    // not two spellings of it.
    if let Some(given) = confirm {
        if given.trim() == TYPED {
            return Ok(true);
        }
        println!("{}", err(&format!("--confirm has to be exactly {TYPED}")));
        return Ok(false);
    }

    if !std::io::stdin().is_terminal() {
        // A pipe cannot be asked. `--yes` deliberately does not stand in for
        // the typed word: a flag is the thing somebody leaves in a script, and
        // this is the one command where that costs recordings. `--confirm`
        // carries the word itself, which is the way through for a machine that
        // genuinely has no terminal.
        println!(
            "{}",
            err(&format!(
                "nothing to type into: --yes is not enough where recordings are \
                 going, so run this in a terminal or pass --confirm {TYPED}"
            ))
        );
        return Ok(false);
    }

    print!("  Type {TYPED} to continue: ");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(answer.trim() == TYPED)
}

/// "one file" or "nine files".
///
/// Spelled out rather than "1 file(s)", because a parenthesis in a sentence a
/// person reads before deleting their recordings is a sign nobody read it back.
fn count(files: usize) -> String {
    if files == 1 {
        "one file".to_string()
    } else {
        format!("{files} files")
    }
}

/// A size somebody can judge at a glance.
///
/// Binary units, named as binary units. A vault of recordings is the thing in
/// this list whose size decides whether somebody agrees, and rounding it to
/// "0.0 MiB" would be the one number worth getting right, got wrong.
fn size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    let bytes = bytes as f64;
    if bytes == 1.0 {
        return "1 byte".to_string();
    }
    if bytes < KIB {
        return format!("{bytes:.0} bytes");
    }
    for (limit, unit) in [
        (KIB * KIB, "KiB"),
        (KIB * KIB * KIB, "MiB"),
        (KIB * KIB * KIB * KIB, "GiB"),
    ] {
        if bytes < limit {
            return format!("{:.1} {unit}", bytes / (limit / KIB));
        }
    }
    format!("{:.1} TiB", bytes / (KIB * KIB * KIB * KIB))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_readable_at_every_scale() {
        assert_eq!(size(0), "0 bytes");
        assert_eq!(size(1), "1 byte");
        assert_eq!(size(512), "512 bytes");
        assert_eq!(size(2048), "2.0 KiB");
        assert_eq!(size(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(size(3 * 1024 * 1024 * 1024), "3.0 GiB");
        // The number that decides whether somebody agrees. A vault of
        // recordings must not round to nothing.
        assert!(size(1024 * 1024 + 1).starts_with("1.0 MiB"));
    }

    #[test]
    fn one_file_is_not_one_files() {
        assert_eq!(count(1), "one file");
        assert_eq!(count(0), "0 files");
        assert_eq!(count(12), "12 files");
    }

    /// A folder of settings needs a flag. A folder with recordings in it needs
    /// the word typed.
    ///
    /// The distinction the whole command turns on, checked against real plans
    /// rather than against the flag: `--yes` must not be a way past the typed
    /// confirmation, because that is exactly what somebody's script would do.
    #[test]
    fn yes_is_enough_for_settings_and_not_for_recordings() {
        use veilvoice_crypto::layout::{entry, Item};
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(entry(Item::Settings).name), b"a palette").unwrap();

        let settings_only = reset::plan_in(dir.path(), Keep::Nothing);
        assert!(!settings_only.irreplaceable());
        assert!(
            agreed(&settings_only, true, None).unwrap(),
            "a folder of settings asked for more than a flag"
        );
        assert!(
            !agreed(&settings_only, false, None).unwrap(),
            "a reset went ahead with neither a flag nor a typed word"
        );

        std::fs::create_dir(dir.path().join(entry(Item::Vaults).name)).unwrap();
        std::fs::write(
            dir.path().join(entry(Item::Vaults).name).join("a.veil"),
            b"a recording",
        )
        .unwrap();
        let with_recordings = reset::plan_in(dir.path(), Keep::Nothing);
        assert!(with_recordings.irreplaceable());
        // Not a terminal under `cargo test`, so this is the "nothing to type
        // into" path, and what it must not do is return true because `--yes`
        // was passed.
        assert!(
            !agreed(&with_recordings, true, None).unwrap(),
            "--yes was enough to delete recordings"
        );
        assert!(
            agreed(&with_recordings, false, Some(TYPED)).unwrap(),
            "the word given outright was not accepted, so a machine with no \
             terminal cannot be reset at all"
        );
        for wrong in ["reset", "RESETT", "", "yes"] {
            assert!(
                !agreed(&with_recordings, false, Some(wrong)).unwrap(),
                "{wrong:?} was accepted in place of the word"
            );
        }
    }

    /// The shared list names the same files the command line's own modules do.
    ///
    /// The window checks its half of this in `paths::tests`, and neither crate
    /// can see the other, so the two halves are checked where they can be. What
    /// fails here is a reset that names `capture` while `veilvoice capture`
    /// writes somewhere else: the allowlist would survive a reset that said it
    /// had removed it.
    #[test]
    fn the_shared_list_names_the_same_files_this_crate_does() {
        use veilvoice_crypto::layout::{self, Item};
        if layout::dir().is_none() {
            return; // a platform that does not say where configuration goes
        }
        for (item, mine) in [
            (Item::Captures, crate::capture::capture_dir()),
            (Item::Sentry, crate::sentry::state_dir()),
            (Item::Policies, crate::policy::policy_dir()),
            (Item::Mandate, veilvoice_policy::mandate_path()),
            (Item::Integrity, veilvoice_guard::record_path()),
        ] {
            assert_eq!(
                layout::path(item),
                mine,
                "{item:?} is in two places at once: the shared list and the \
                 module that writes it disagree"
            );
        }
    }

    /// The word the prompt asks for is the word the code compares against.
    #[test]
    fn the_typed_word_is_asked_for_by_name() {
        let source = include_str!("reset.rs");
        assert!(
            source.contains("Type {TYPED} to continue"),
            "the prompt does not ask for the word it compares against"
        );
        assert_eq!(
            TYPED,
            TYPED.to_uppercase(),
            "a word typed in anger is typed in capitals"
        );
    }
}
