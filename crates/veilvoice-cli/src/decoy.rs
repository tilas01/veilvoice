// SPDX-License-Identifier: GPL-3.0-or-later
//! `veilvoice decoy`, and what a second passphrase is worth and what it is not.
//!
//! # What it is worth is printed on the way in, not behind a flag
//!
//! Every path that reports on a decoy or sets one up prints
//! [`veilvoice_crypto::decoy::SCOPE`], and the two that set one print
//! [`veilvoice_crypto::decoy::WHY_NO_DESTRUCTION`] with it, for the same reason
//! [`crate::lock`] prints the lock's own: somebody who believes a decoy hides
//! that a second passphrase exists has been made **less** safe by it, not more.
//! They would rely on an argument they do not have.
//!
//! Removing one is the exception and prints neither. Nothing is being taken on
//! trust there, and a reader who has just asked for the feature to go does not
//! need a page on what it was worth.
//!
//! The window prints the same two passages from the same two constants, so
//! neither front end can soften it without the other.
//!
//! # In plain words
//!
//! Explains the decoy passphrase, and sets, changes or removes one.
//!
//! A decoy opens VeilVoice with nothing in it. It is there for the situation
//! where somebody is standing over you asking you to unlock your computer: it
//! gives you something true to say.
//!
//! It does not hide that a second passphrase might exist, and no passphrase
//! destroys your recordings. Both of those are printed every time, and the
//! second is deliberate: on modern storage, deleting a file does not reliably
//! remove it, so a feature that claimed to would be lying to you at the worst
//! possible moment.
//!
//! Setting or removing a decoy asks for your real passphrase first, so nobody
//! can do either at a window you left unlocked.

use crate::atrest::prompt_secret;
use crate::theme::{colour, field, heading, ok, paint, warn};
use clap::Subcommand;
use veilvoice_crypto::decoy::{self, State};
use veilvoice_crypto::{layout, LockStore, Secret};

/// What `veilvoice decoy` can be asked to do.
///
/// There is deliberately no variant that reads a decoy out, exactly as there is
/// none on [`crate::lock::Action`]: the only things that can be done to a
/// passphrase are setting it, proving it and removing it.
#[derive(Subcommand)]
pub enum Action {
    /// Report whether a decoy is set, and where it lives.
    Status,
    /// Set a decoy passphrase. Refuses if one is already set.
    Set,
    /// Replace the decoy passphrase, after proving the real one.
    Change,
    /// Remove the decoy passphrase, after proving the real one.
    Remove,
}

/// Dispatch `veilvoice decoy`, or explain the feature when nothing was asked.
///
/// A bare `veilvoice decoy` explains and changes nothing, which is what it has
/// always done and what the help screen promises. It is also the only path here
/// that neither reads nor writes a file, so it is the one somebody can run
/// while deciding.
pub fn run(action: Option<Action>) -> Result<(), String> {
    match action {
        None => explain(),
        Some(Action::Status) => status(),
        Some(Action::Set) => set(false),
        Some(Action::Change) => set(true),
        Some(Action::Remove) => remove(),
    }
}

/// Explain the feature and its limits.
pub fn explain() -> Result<(), String> {
    println!("{}", heading("A second passphrase"));
    println!();
    print_scope();

    println!();
    println!("{}", paint(colour::YELLOW, "WHAT NO PASSPHRASE DOES"));
    print_no_destruction();

    println!();
    println!("{}", heading("What a pair has to satisfy"));
    print_pair_rule();
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
    println!();
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Set one with:    veilvoice decoy set\n  \
             or in the window: Settings, then Lock",
        )
    );
    Ok(())
}

/// `veilvoice decoy status`: whether a decoy is set, and where it lives.
///
/// Says nothing about the passphrase itself. What a reader learns here is what
/// somebody holding the machine can work out anyway, and the one thing worth
/// telling them is the answer this command exists for: a record that will not
/// read is not a decoy, and they would find that out at the worst possible
/// moment.
fn status() -> Result<(), String> {
    println!("{}", heading("Decoy passphrase"));

    // Checked before anything opens the vault, because opening it writes the
    // index that names the lock's files. A machine with no lock has no decoy
    // and should be left exactly as it was found.
    let index = layout::path(layout::Item::AppLock)
        .ok_or_else(|| "cannot work out where this platform keeps configuration".to_string())?;
    if !index.is_file() {
        println!("{}", field("State", "not set"));
        println!("{}", field("App lock", "not set either"));
        println!();
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  A decoy is a second passphrase for the app lock, so the lock comes\n  \
                 first: veilvoice lock set",
            )
        );
        return Ok(());
    }

    let store = decoy::Store::here().map_err(|e| e.to_string())?;
    // The folder rather than the file. The decoy's own name is derived from the
    // same index the lock's two copies are, so printing it would tell a reader
    // nothing and tell anybody reading over their shoulder which file to take.
    let folder = match index.parent() {
        Some(dir) => dir.display().to_string(),
        None => index.display().to_string(),
    };
    println!("{}", field("Folder", &folder));
    let state = store.state();
    match state {
        State::Absent => {
            println!("{}", field("State", "not set"));
            println!();
            println!(
                "{}",
                paint(colour::MUTED, "  Set one with: veilvoice decoy set")
            );
        }
        State::Set => {
            println!("{}", field("State", "set"));
            println!();
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  Typing it opens VeilVoice with nothing in it, and does not count\n  \
                     against the app lock's failed attempts.",
                )
            );
        }
        State::Unreadable => {
            println!(
                "{}",
                field("State", "there is a record and it does not read")
            );
            println!();
            println!(
                "{}",
                warn(
                    "so this machine has no working decoy. The passphrase you think \
                     is a decoy would open nothing. Set it again with `veilvoice \
                     decoy change`, or remove it with `veilvoice decoy remove`"
                )
            );
            println!(
                "{}",
                paint(
                    colour::MUTED,
                    "  The real passphrase is unaffected: it lives in the app lock and\n  \
                     nothing here touches it.",
                )
            );
        }
    }
    if state != State::Absent {
        println!(
            "{}",
            paint(
                colour::MUTED,
                "  The record's own name in there is derived, like the lock's two\n  \
                 copies, so a folder listing does not say which file it is."
            )
        );
    }
    println!();
    print_scope();
    Ok(())
}

/// `veilvoice decoy set` and `veilvoice decoy change`, which are one path.
///
/// The same work either way, and deliberately not two functions: the only
/// difference is which state is refused, and a second copy of the prompts and
/// the printed scope is a second copy that can drift. Which one was asked for
/// still matters, because a `set` that silently replaced would take away a
/// decoy somebody is relying on without saying so.
fn set(replacing: bool) -> Result<(), String> {
    let lock = lock_or_explain()?;
    let store = decoy::Store::here().map_err(|e| e.to_string())?;
    if let Some(note) = allowed(store.state(), replacing)? {
        println!("{}", warn(note));
    }

    println!("{}", heading("A second passphrase"));
    println!();
    print_scope();
    println!();
    println!("{}", paint(colour::YELLOW, "WHAT NO PASSPHRASE DOES"));
    print_no_destruction();
    println!();
    print_pair_rule();
    println!();

    let real = prompt_secret("App lock passphrase: ")?;
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Checking it (Argon2id, deliberately slow)..."
        )
    );
    // Checked here as well as inside `Store::set`, which proves it too and has
    // to: the crate cannot rely on a front end to have asked. What this buys is
    // the order somebody experiences. Without it, a mistyped app-lock
    // passphrase is reported after they have chosen a decoy and typed it twice,
    // and the reason it failed looks like something they did wrong at the end.
    if !lock.would_open(real.expose()).map_err(|e| e.to_string())? {
        return Err(
            "that is not this machine's app lock passphrase, so nothing \
                    was changed. A decoy can only be set by whoever can already \
                    open VeilVoice"
                .into(),
        );
    }
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Now the decoy, twice. Do NOT make it a version of the real one."
        )
    );
    let decoyed = read_twice()?;
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Deriving (Argon2id, deliberately slow, twice)..."
        )
    );
    store
        .set(&lock, text(&real)?, text(&decoyed)?)
        .map_err(|e| e.to_string())?;

    println!(
        "{}",
        ok(if replacing {
            "decoy passphrase changed"
        } else {
            "decoy passphrase set"
        })
    );
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Typing it opens VeilVoice with nothing in it, and does not count\n  \
             against the app lock's failed attempts.",
        )
    );
    Ok(())
}

/// `veilvoice decoy remove`: take the decoy off, after proving the real one.
///
/// Asks for the real passphrase for the same reason setting one does. A decoy
/// anybody could remove is a decoy that can be taken away from the person
/// relying on it, and they would not find out until they needed it.
fn remove() -> Result<(), String> {
    let lock = lock_or_explain()?;
    let store = decoy::Store::here().map_err(|e| e.to_string())?;
    if store.state() == State::Absent {
        return Err("no decoy is set here, so there is nothing to remove".into());
    }

    println!("{}", heading("Decoy passphrase"));
    let real = prompt_secret("App lock passphrase: ")?;
    store
        .remove(&lock, text(&real)?)
        .map_err(|e| e.to_string())?;
    println!("{}", ok("decoy passphrase removed"));
    println!(
        "{}",
        paint(
            colour::MUTED,
            "  Only the real passphrase opens VeilVoice now. Nothing was deleted:\n  \
             a decoy never held any of your recordings.",
        )
    );
    Ok(())
}

/// Whether the state on disk allows what was asked, and what to say first.
///
/// `Ok(None)` proceeds, `Ok(Some(note))` proceeds after a warning, and an error
/// refuses with the command the reader wanted instead. Pulled out of [`set`] so
/// the four combinations can be tested, which the prompts themselves cannot be:
/// they need a terminal.
fn allowed(state: State, replacing: bool) -> Result<Option<&'static str>, String> {
    match (state, replacing) {
        (State::Set, false) => Err("a decoy is already set here. Use `veilvoice decoy change` \
                                    to replace it, which asks for the real passphrase first"
            .into()),
        (State::Absent, true) => {
            Err("no decoy is set here, so use `veilvoice decoy set` instead".into())
        }
        (State::Unreadable, _) => Ok(Some(
            "the record here does not read, so this machine has no working \
             decoy. Setting one replaces it",
        )),
        _ => Ok(None),
    }
}

/// The app lock, or a message saying what to do about there not being one.
///
/// A decoy is a second passphrase for the app lock, so there is nothing for it
/// to be second to until a lock exists. Said in those words rather than as a
/// bare failure, because "no app lock" does not tell somebody what to run.
fn lock_or_explain() -> Result<LockStore, String> {
    let (store, _restored) = veilvoice_crypto::lock::open_default().map_err(|e| e.to_string())?;
    store.ok_or_else(|| {
        "there is no app lock on this machine, so there is nothing for a decoy to \
         be a second passphrase to. Set one with `veilvoice lock set` first"
            .to_string()
    })
}

/// Read the decoy twice, without echoing it, and check the two agree.
///
/// [`crate::atrest::read_new_password`] does this for the app lock and prompts
/// with words that would be wrong here: somebody being asked for "Passphrase:"
/// while setting a decoy has every reason to type the real one. The comparison
/// is not constant time and does not need to be, because both sides are what
/// the same person typed a moment ago.
fn read_twice() -> Result<Secret, String> {
    let first = prompt_secret("Decoy passphrase: ")?;
    if first.expose().is_empty() {
        return Err("a decoy passphrase must not be empty".into());
    }
    let again = prompt_secret("Repeat: ")?;
    if first.expose() != again.expose() {
        return Err("the two do not match, so nothing was set".into());
    }
    Ok(first)
}

/// A typed passphrase as text.
///
/// The difference rule counts characters rather than bytes, so the crypto crate
/// takes text. Anything a terminal hands back is already text; this is the arm
/// for the case that cannot arrive rather than an `unwrap` that says so.
fn text(secret: &Secret) -> Result<&str, String> {
    std::str::from_utf8(secret.expose())
        .map_err(|_| "that passphrase is not valid text, so it cannot be used here".to_string())
}

/// What a decoy is worth, wrapped, from the one constant both front ends read.
fn print_scope() {
    print_wrapped(decoy::SCOPE);
}

/// Why no passphrase destroys anything, from the same place.
fn print_no_destruction() {
    print_wrapped(decoy::WHY_NO_DESTRUCTION);
}

/// Print an indented, wrapped passage, with the blank lines left blank.
///
/// An indented empty line is two spaces of trailing whitespace, which every
/// generated copy of this output would then carry.
fn print_wrapped(text: &str) {
    for line in paragraphs(text, 72) {
        if line.is_empty() {
            println!();
        } else {
            println!("  {line}");
        }
    }
}

/// The rule about the pair, and why there is one.
fn print_pair_rule() {
    println!(
        "{}",
        field(
            "different in at least",
            &format!("{} places", decoy::LEAST_DIFFERENCE)
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
}

/// Wrap text that has paragraphs in it, and keep the paragraphs.
///
/// [`crate::sentry::wrap`] splits on whitespace, which runs a blank line into
/// the sentence before it. That is fine for the one-paragraph notes it was
/// written for and wrong for these two: [`decoy::WHY_NO_DESTRUCTION`] is three
/// paragraphs and read as one wall of text, with the plain statement that
/// nothing is destroyed buried in the middle of it.
fn paragraphs(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for (i, para) in text.split("\n\n").enumerate() {
        if i > 0 {
            lines.push(String::new());
        }
        lines.extend(crate::sentry::wrap(para, width));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decoy_that_is_already_there_is_not_replaced_by_set() {
        let refusal = allowed(State::Set, false).unwrap_err();
        assert!(
            refusal.contains("veilvoice decoy change"),
            "the refusal has to name what to run instead: {refusal}"
        );
    }

    #[test]
    fn changing_a_decoy_that_is_not_there_says_to_set_one() {
        let refusal = allowed(State::Absent, true).unwrap_err();
        assert!(refusal.contains("veilvoice decoy set"), "{refusal}");
    }

    #[test]
    fn setting_a_first_decoy_and_changing_one_that_exists_both_proceed() {
        assert_eq!(allowed(State::Absent, false).unwrap(), None);
        assert_eq!(allowed(State::Set, true).unwrap(), None);
    }

    /// A record that will not read is the case this state exists for: whoever
    /// relies on it has no decoy and does not know. Setting one over it is
    /// allowed, and saying nothing about it is not.
    #[test]
    fn a_record_that_does_not_read_is_replaced_and_said_out_loud() {
        for replacing in [false, true] {
            let note = allowed(State::Unreadable, replacing)
                .unwrap()
                .expect("a broken record has to be mentioned");
            assert!(note.contains("does not read"), "{note}");
        }
    }

    #[test]
    fn wrapping_keeps_the_paragraphs_and_every_word() {
        let lines = paragraphs(decoy::WHY_NO_DESTRUCTION, 60);
        assert!(
            lines.iter().filter(|l| l.is_empty()).count() >= 2,
            "three paragraphs need two blank lines between them: {lines:?}"
        );
        assert!(lines.iter().all(|l| l.len() <= 60 || !l.contains(' ')));
        assert_eq!(
            lines.join(" ").split_whitespace().collect::<Vec<_>>(),
            decoy::WHY_NO_DESTRUCTION
                .split_whitespace()
                .collect::<Vec<_>>(),
            "a word was lost or moved"
        );
    }

    /// The uncomfortable half is the half a front end is tempted to drop.
    #[test]
    fn what_is_printed_still_says_the_uncomfortable_part() {
        let scope = paragraphs(decoy::SCOPE, 72).join(" ").to_lowercase();
        assert!(scope.contains("does not hide"), "{scope}");
        let destruction = paragraphs(decoy::WHY_NO_DESTRUCTION, 72)
            .join(" ")
            .to_lowercase();
        assert!(
            destruction.contains("no passphrase that deletes your recordings"),
            "{destruction}"
        );
    }

    /// Printed by the path that explains, the one that reports and the one
    /// that sets. Read out of this file because the prompts in between need a
    /// terminal: what this catches is a path that quietly stops printing it.
    ///
    /// `remove` is not in the list on purpose, and the module note says why.
    #[test]
    fn every_path_that_reports_or_sets_prints_what_a_decoy_is_worth() {
        let source = include_str!("decoy.rs");
        for function in ["fn explain()", "fn status()", "fn set("] {
            let body = source
                .split(function)
                .nth(1)
                .unwrap_or_else(|| panic!("{function} has to be findable"));
            let body = body.split("\n/// ").next().unwrap();
            assert!(
                body.contains("print_scope()"),
                "{function} does not print what a decoy is worth"
            );
        }
        let setting = source.split("fn set(").nth(1).unwrap();
        assert!(
            setting
                .split("\n/// ")
                .next()
                .unwrap()
                .contains("print_no_destruction()"),
            "setting a decoy has to say that no passphrase destroys anything"
        );
    }
}
