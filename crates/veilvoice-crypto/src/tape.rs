// SPDX-License-Identifier: GPL-3.0-or-later
//! A recording held in locked, zeroizing memory while it is still being made.
//!
//! # The problem this exists for
//!
//! [`Secret`] holds a passphrase or a key: a few dozen bytes, known in full
//! before the allocation is made. A recording is neither. It arrives a few
//! milliseconds at a time, for as long as somebody keeps talking, and nobody
//! knows at the start how long that will be.
//!
//! Accumulating it in a `Vec<u8>` would undo the thing this crate is careful
//! about everywhere else. A `Vec` is not locked, so the operating system may
//! write it to the page file; it is not zeroized, so its contents outlive it in
//! freed memory; and it reallocates as it grows, which leaves the *previous*
//! buffer, still holding the recording so far, somewhere on the heap with
//! nothing wiping it. Every doubling leaves another copy behind.
//!
//! A `Tape` is the growable equivalent of a [`Secret`]: append-only, made of
//! page-locked chunks that are never reallocated or moved, and zeroized in full
//! when it goes out of scope.
//!
//! # Why chunks, rather than one buffer that grows
//!
//! Growing means reallocating, and reallocating a secret means copying it to a
//! new address and leaving the old bytes unwiped in memory the allocator is now
//! free to hand to anybody. The whole point is that no copy is ever left
//! behind, so nothing here is ever resized. A full chunk is kept exactly where
//! it is and a new one is added beside it.
//!
//! Chunks are [`CHUNK`] bytes, which is a deliberate compromise rather than a
//! round number picked for looks. Locking is charged against a per-process
//! budget (`RLIMIT_MEMLOCK` on Linux, often a few megabytes and sometimes far
//! less), and a chunk is the unit in which that budget is spent. Small chunks
//! spend it in fine increments, so a tape that outgrows the budget locks as
//! much as the budget allowed rather than losing a large request wholesale.
//!
//! # What this buys, and what it does not
//!
//! It buys the same thing [`Secret`] buys, over a buffer that grows: the
//! recording is kept out of the page file where the operating system permits
//! it, and it is wiped rather than abandoned.
//!
//! It does not defeat somebody who can already read this process's memory, and
//! locking does not survive hibernation, which writes RAM to disk wholesale.
//! Locking can also simply fail: the budget above is small and unprivileged
//! processes cannot raise it. That is reported rather than hidden.
//! [`Tape::fully_locked`] is false the moment one chunk could not be locked,
//! and [`Tape::locked_chunks`] says how many were, so a caller can tell the
//! user what was actually obtained instead of implying a guarantee.
//!
//! Zeroization, as in [`crate::amnesia`], always happens.
//!
//! # In plain words
//!
//! Somewhere to keep a recording while it is being made, which asks the
//! operating system not to write it out to disk and wipes it when it is
//! finished with.
//!
//! It is built out of fixed-size pieces so that it never has to move what it is
//! already holding. Moving it would leave a copy of your recording behind in
//! memory, which is exactly what this is for avoiding.

use crate::{Error, Secret};

/// Bytes per chunk.
///
/// See the module documentation: this is the unit in which the operating
/// system's lock budget is spent, so it is small enough that a tape which
/// outgrows that budget still locks most of what it holds, and large enough
/// that a long recording does not accumulate an absurd number of allocations.
/// At 48 kHz 16-bit mono, one chunk is about two thirds of a second.
pub const CHUNK: usize = 64 * 1024;

/// An append-only buffer of locked, zeroizing chunks.
///
/// Deliberately has no `Debug`, no `Clone` and no way to hand out an owned copy
/// of its contents: every route out of it writes into storage the caller has
/// already made safe. See [`Tape::copy_into`].
pub struct Tape {
    /// Full chunks, then the one being filled. Never reallocated or moved.
    chunks: Vec<Secret>,
    /// Bytes used in the final chunk. Every earlier chunk is full.
    filled: usize,
}

impl Default for Tape {
    fn default() -> Self {
        Self::new()
    }
}

impl Tape {
    /// An empty tape. No allocation happens until the first byte is appended.
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            filled: 0,
        }
    }

    /// Append `bytes`.
    ///
    /// Fills the current chunk before adding another, so a tape of `n` bytes
    /// holds `n.div_ceil(CHUNK)` chunks and no more: the count is a function of
    /// the length alone, never of how the appends were split up. A caller
    /// pushing one sample at a time and a caller pushing a whole buffer end up
    /// with byte-for-byte the same tape.
    pub fn push(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            if self.chunks.is_empty() || self.filled == CHUNK {
                self.chunks.push(Secret::zeroed(CHUNK));
                self.filled = 0;
            }
            let room = CHUNK - self.filled;
            let take = room.min(bytes.len());
            let at = self.filled;
            // `expect` cannot fire: a chunk was just pushed if none existed.
            let last = self
                .chunks
                .last_mut()
                .expect("a chunk exists after the check above");
            last.expose_mut()[at..at + take].copy_from_slice(&bytes[..take]);
            self.filled += take;
            bytes = &bytes[take..];
        }
    }

    /// Total bytes held.
    pub fn len(&self) -> usize {
        match self.chunks.len() {
            0 => 0,
            n => (n - 1) * CHUNK + self.filled,
        }
    }

    /// Whether nothing has been appended.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many chunks the operating system agreed to lock out of swap.
    pub fn locked_chunks(&self) -> usize {
        self.chunks.iter().filter(|c| c.is_locked()).count()
    }

    /// How many chunks the tape holds.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Whether every chunk is locked.
    ///
    /// An empty tape is trivially fully locked: there is nothing unlocked in
    /// it. Callers reporting this to a user should check [`Tape::is_empty`]
    /// first if "nothing to protect" and "protected" should read differently.
    pub fn fully_locked(&self) -> bool {
        self.chunks.iter().all(|c| c.is_locked())
    }

    /// Copy the whole tape into `out`, which must be exactly [`Tape::len`].
    ///
    /// Takes a destination rather than returning a `Vec` on purpose. Returning
    /// one would put the entire recording into unlocked, unzeroized memory and
    /// undo this type in a single line, and it would do it at the one moment
    /// the recording is complete and therefore at its most worth protecting.
    /// The caller allocates a [`Secret`] and passes its
    /// [`expose_mut`](Secret::expose_mut) here instead.
    ///
    /// Returns [`Error::TapeLength`] if `out` is the wrong size, rather than
    /// copying what fits: a partial recording written into a buffer sized for a
    /// whole one is a truncated file that nothing would flag.
    pub fn copy_into(&self, out: &mut [u8]) -> Result<(), Error> {
        if out.len() != self.len() {
            return Err(Error::TapeLength);
        }
        let mut at = 0;
        for (index, chunk) in self.chunks.iter().enumerate() {
            let take = if index + 1 == self.chunks.len() {
                self.filled
            } else {
                CHUNK
            };
            out[at..at + take].copy_from_slice(&chunk.expose()[..take]);
            at += take;
        }
        Ok(())
    }

    /// Wipe and release everything held, leaving an empty tape.
    ///
    /// Each chunk is a [`Secret`], so dropping it wipes it and unlocks its
    /// pages. This exists so a caller can do that at a chosen moment rather
    /// than waiting for the tape itself to go out of scope, which matters when
    /// the tape lives inside a long-running session.
    pub fn wipe(&mut self) {
        self.chunks.clear();
        self.filled = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_tape_holds_nothing_and_allocates_nothing() {
        let tape = Tape::new();
        assert_eq!(tape.len(), 0);
        assert!(tape.is_empty());
        assert_eq!(tape.chunk_count(), 0);
    }

    #[test]
    fn what_goes_in_comes_out_byte_for_byte() {
        let mut tape = Tape::new();
        let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        tape.push(&data);
        assert_eq!(tape.len(), data.len());

        let mut out = vec![0u8; tape.len()];
        tape.copy_into(&mut out).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn a_tape_spanning_many_chunks_reassembles_in_order() {
        // The failure this guards is an off-by-one at a chunk boundary, which
        // would corrupt audio in a way that still plays: the length would be
        // right and the samples would be shuffled.
        let mut tape = Tape::new();
        let data: Vec<u8> = (0..(CHUNK * 3 + 1234)).map(|i| (i % 253) as u8).collect();
        tape.push(&data);
        assert_eq!(tape.len(), data.len());
        assert_eq!(tape.chunk_count(), 4);

        let mut out = vec![0u8; tape.len()];
        tape.copy_into(&mut out).unwrap();
        assert_eq!(out, data, "the tape did not reassemble in order");
    }

    #[test]
    fn how_the_appends_were_split_up_does_not_change_the_tape() {
        // One sample at a time and one buffer at a time have to produce the
        // same bytes and the same chunk count, or an audio callback delivering
        // odd-sized buffers would produce a different file from the same sound.
        let data: Vec<u8> = (0..(CHUNK + 777)).map(|i| (i % 249) as u8).collect();

        let mut whole = Tape::new();
        whole.push(&data);

        let mut dribbled = Tape::new();
        for byte in &data {
            dribbled.push(std::slice::from_ref(byte));
        }

        let mut a = vec![0u8; whole.len()];
        let mut b = vec![0u8; dribbled.len()];
        whole.copy_into(&mut a).unwrap();
        dribbled.copy_into(&mut b).unwrap();
        assert_eq!(a, b);
        assert_eq!(whole.chunk_count(), dribbled.chunk_count());
    }

    #[test]
    fn a_chunk_boundary_lands_exactly_where_the_arithmetic_says() {
        let mut tape = Tape::new();
        tape.push(&vec![7u8; CHUNK]);
        assert_eq!(tape.len(), CHUNK);
        assert_eq!(tape.chunk_count(), 1, "a full chunk is not yet a second one");

        tape.push(&[9]);
        assert_eq!(tape.len(), CHUNK + 1);
        assert_eq!(tape.chunk_count(), 2);
    }

    #[test]
    fn pushing_nothing_changes_nothing() {
        let mut tape = Tape::new();
        tape.push(&[]);
        assert_eq!(tape.chunk_count(), 0, "an empty push must not allocate");
        tape.push(&[1, 2, 3]);
        tape.push(&[]);
        assert_eq!(tape.len(), 3);
        assert_eq!(tape.chunk_count(), 1);
    }

    #[test]
    fn a_destination_of_the_wrong_size_is_refused_rather_than_part_filled() {
        let mut tape = Tape::new();
        tape.push(&[1, 2, 3, 4]);

        let mut too_small = vec![0u8; 3];
        assert!(matches!(
            tape.copy_into(&mut too_small),
            Err(Error::TapeLength)
        ));
        let mut too_large = vec![0u8; 5];
        assert!(matches!(
            tape.copy_into(&mut too_large),
            Err(Error::TapeLength)
        ));
        // Neither attempt wrote anything.
        assert_eq!(too_small, vec![0, 0, 0]);
        assert_eq!(too_large, vec![0, 0, 0, 0, 0]);
    }

    #[test]
    fn wiping_leaves_an_empty_tape_that_still_works() {
        let mut tape = Tape::new();
        tape.push(&vec![3u8; CHUNK * 2]);
        assert_eq!(tape.chunk_count(), 2);

        tape.wipe();
        assert!(tape.is_empty());
        assert_eq!(tape.chunk_count(), 0);

        // And it is reusable rather than poisoned.
        tape.push(&[8, 8]);
        let mut out = vec![0u8; 2];
        tape.copy_into(&mut out).unwrap();
        assert_eq!(out, vec![8, 8]);
    }

    #[test]
    fn the_lock_count_never_claims_more_than_the_tape_holds() {
        // Locking is best-effort and the budget is small, so this asserts the
        // relationship rather than a number: claiming more locked chunks than
        // exist would be a report that cannot be true.
        let mut tape = Tape::new();
        tape.push(&vec![1u8; CHUNK * 2 + 10]);
        assert_eq!(tape.chunk_count(), 3);
        assert!(tape.locked_chunks() <= tape.chunk_count());
        assert_eq!(
            tape.fully_locked(),
            tape.locked_chunks() == tape.chunk_count()
        );
    }

    #[test]
    fn an_empty_tape_reports_no_unlocked_chunks() {
        let tape = Tape::new();
        assert!(tape.fully_locked());
        assert_eq!(tape.locked_chunks(), 0);
        assert_eq!(tape.chunk_count(), 0);
    }
}
