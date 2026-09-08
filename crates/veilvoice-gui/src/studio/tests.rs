// SPDX-License-Identifier: GPL-3.0-or-later
//! What the Studio has to get right, tested without a window.
//!
//! The drawing needs a running egui context and is checked by the screenshot
//! run. Everything here is the part that would still be wrong if the drawing
//! were perfect: the state machine, the wiping, and the two formatters that
//! turn stored numbers into what a person reads.

use super::*;

#[test]
fn a_new_studio_is_shut_and_stays_shut_until_both_are_given() {
    let studio = Studio::default();
    assert!(!studio.is_open(), "a fresh Studio must not be open");
    assert!(!studio.is_recording());
    assert!(matches!(studio.phase(), Phase::Shut));
}

#[test]
fn one_passphrase_alone_never_derives_a_key() {
    // The property the whole vault rests on, asserted here as well as in the
    // crypto crate, because this is the layer a user actually reaches: if this
    // tab ever passed an empty half through, the vault would degrade to one
    // secret and nothing in the window would say so.
    let mut app = String::from("app-lock");
    let mut nothing = String::new();
    let one = into_secret(&mut app);
    let none = into_secret(&mut nothing);
    assert!(StudioKey::derive(&one, &none).is_err());
    assert!(StudioKey::derive(&none, &one).is_err());
}

#[test]
fn taking_a_passphrase_wipes_the_buffer_it_came_from() {
    let mut typed = String::from("a passphrase somebody typed");
    let secret = into_secret(&mut typed);
    assert_eq!(secret.expose(), b"a passphrase somebody typed");
    assert!(
        typed.is_empty(),
        "the typing buffer still holds the passphrase: {typed:?}"
    );
}

#[test]
fn closing_forgets_the_vault_the_listing_and_both_entries() {
    let mut studio = Studio::default();
    studio.app_entry.push_str("app");
    studio.rest_entry.push_str("rest");
    studio.entries.push(Entry {
        id: "abc".into(),
        name: "a name".into(),
        made: 0,
        bytes: 10,
    });
    studio.selected = Some("abc".into());

    studio.close();

    assert!(
        studio.app_entry.is_empty(),
        "the app entry survived closing"
    );
    assert!(
        studio.rest_entry.is_empty(),
        "the at-rest entry survived closing"
    );
    assert!(studio.entries.is_empty(), "the listing survived closing");
    assert!(studio.selected.is_none());
    assert!(!studio.is_open());
}

#[test]
fn a_length_reads_as_minutes_and_seconds() {
    assert_eq!(length(0.0), "0:00");
    assert_eq!(length(9.4), "0:09");
    assert_eq!(length(60.0), "1:00");
    assert_eq!(length(61.9), "1:01");
    assert_eq!(length(3_600.0), "60:00");
    // A negative length is not a thing, and must not print as one.
    assert_eq!(length(-5.0), "0:00");
}

#[test]
fn a_size_never_reads_as_zero() {
    // A recording that exists is never "0 KiB". Somebody reading that in a
    // listing concludes the recording is empty, and it is not.
    assert_eq!(size(1), "1 KiB");
    assert_eq!(size(0), "1 KiB");
    assert_eq!(size(2048), "2 KiB");
    assert_eq!(size(1024 * 1024), "1.0 MiB");
    assert_eq!(size(3 * 1024 * 1024 / 2), "1.5 MiB");
}

#[test]
fn the_count_is_written_rather_than_printed_with_an_s() {
    assert_eq!(counted(1), "one recording");
    assert_eq!(counted(0), "0 recordings");
    assert_eq!(counted(7), "7 recordings");
}

#[test]
fn a_stored_date_reads_as_the_date_it_was() {
    // Known instants, checked against the calendar rather than against this
    // function's own output.
    assert_eq!(made_on(0), "1970-01-01");
    assert_eq!(made_on(1_700_000_000), "2023-11-14");
    // A leap day, which is where a hand-written civil-date conversion goes
    // wrong if it is going to.
    assert_eq!(made_on(1_709_164_800), "2024-02-29");
    assert_eq!(made_on(1_709_251_200), "2024-03-01");
    // The turn of a century that is not a leap year.
    assert_eq!(made_on(4_102_444_800), "2100-01-01");
    // Before the epoch, which a naive division gets wrong by a day.
    assert_eq!(made_on(-1), "1969-12-31");
}

#[test]
fn the_vault_lives_beside_the_lock_rather_than_somewhere_of_its_own() {
    // Where it is matters: a vault in a second location is a second thing to
    // find, back up and lose.
    if let (Some(vault), Some(lock)) = (default_dir(), veilvoice_crypto::lock::default_path()) {
        assert_eq!(
            vault.parent(),
            lock.parent(),
            "the vault is not beside the lock file"
        );
        assert_eq!(vault.file_name().unwrap(), "studio");
    }
}
