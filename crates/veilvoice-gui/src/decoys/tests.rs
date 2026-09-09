// SPDX-License-Identifier: GPL-3.0-or-later
//! What the panel offers, checked without a disk or a window.

use super::*;

/// A vault of `n` recordings of `each` bytes, with an index to match.
fn shape(n: usize, each: usize) -> Shape {
    Shape {
        recordings: n,
        each,
        index: 0,
    }
}

#[test]
fn the_count_comes_from_the_free_space_and_not_from_a_number_picked_here() {
    let s = shape(4, 1024 * 1024);
    let one = advise(s, Some(0)).each;

    // Twenty times the room for three, at the twentieth this keeps to.
    let a = advise(s, Some(one * 3 * SHARE));
    assert_eq!(a.room, Some(3));
    assert_eq!(a.count, 3);

    // Twice the space, twice the offer. If the count were a constant this
    // would not move.
    let b = advise(s, Some(one * 6 * SHARE));
    assert_eq!(b.room, Some(6));
    assert_eq!(b.count, 6);
    assert_ne!(a.count, b.count);
}

#[test]
fn a_disk_that_will_not_say_is_not_reported_as_a_measurement() {
    let a = advise(shape(2, 4096), None);
    assert_eq!(a.room, None, "silence became a number");
    assert_eq!(a.count, 1, "an unmeasured default should be the floor");
    assert_eq!(a.most, MOST);
}

#[test]
fn a_full_disk_offers_nothing_rather_than_offering_one_anyway() {
    let a = advise(shape(3, 8192), Some(1024));
    assert_eq!(a.room, Some(0));
    assert_eq!(a.count, 0);
    // Nothing is offered rather than one being offered anyway: the panel
    // disables the button on this, and says why.
    assert_eq!(a.most, 0);
}

#[test]
fn an_enormous_disk_stops_at_the_stated_ceiling() {
    let a = advise(shape(1, 1024), Some(u64::MAX));
    assert_eq!(a.count, MOST);
    assert_eq!(a.most, MOST);
}

#[test]
fn an_empty_vault_still_has_a_size_and_does_not_divide_by_zero() {
    // A vault of no recordings is still an index file. If `each` were zero the
    // division working out how many fit would panic.
    let a = advise(shape(0, 0), Some(1024 * 1024 * 1024));
    assert!(a.each >= 1);
    assert!(a.room.is_some());
}

#[test]
fn the_size_offered_is_the_size_the_vault_format_says() {
    // Read from the crypto crate rather than worked out here. Two copies of
    // this arithmetic would be two answers the first time the format changed.
    let s = shape(5, 300_000);
    assert_eq!(advise(s, None).each, s.bytes_on_disk());
}

#[test]
fn the_count_is_said_in_words_that_read_properly() {
    assert_eq!(counted_vaults(0), "no vaults");
    assert_eq!(counted_vaults(1), "one vault");
    assert_eq!(counted_vaults(2), "2 vaults");
}
