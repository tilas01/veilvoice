// SPDX-License-Identifier: GPL-3.0-or-later
//! Amnesic secret storage: page-locked, zeroized, and never printed.
//!
//! # No `unsafe`, even here
//!
//! Locking pages out of the swap file is a raw syscall — `VirtualLock` on
//! Windows, `mlock` on Unix — and it is the one place a project like this
//! usually has to reach for `unsafe`. It does not here: the `region` crate
//! exposes a safe, cross-platform wrapper. VeilVoice therefore contains **no
//! `unsafe` code at all**, and every crate keeps `#![forbid(unsafe_code)]`.
//!
//! # What locking does and does not buy
//!
//! Locking keeps key material out of the page file, so a secret cannot be
//! recovered later by reading swap off the disk. It does **not** protect against
//! an attacker who can already read this process's memory, and it does not
//! survive hibernation, which writes RAM to disk wholesale. Locking can also
//! fail outright — unprivileged Linux users get a small `RLIMIT_MEMLOCK` budget
//! — so it is best-effort hardening and never a precondition.
//! [`Secret::is_locked`] reports what actually happened, so the UI can tell the
//! user the truth rather than imply a guarantee that was not obtained.
//!
//! Zeroization, by contrast, always happens.
//!
//! # Why each secret owns whole pages
//!
//! Locking has *page* granularity, not byte granularity. If two secrets share a
//! 4 KiB page, both lock it, and the first one dropped unlocks the page out from
//! under the second — which is still live and now swappable.
//!
//! Each [`Secret`] therefore over-allocates and locks a page-aligned,
//! page-sized span lying entirely within its own allocation. No other allocation
//! can occupy those bytes, so none can be inside those pages: lock and unlock
//! are exact, and locking a secret never drags unrelated data into physical
//! memory alongside it.
//!
//! # Why the lock is not held by an RAII guard
//!
//! `region::lock` hands back a guard that unlocks on drop — but its destructor
//! **panics** if unlocking fails, and unlocking can fail for reasons that are
//! nobody's fault: Windows does not reference-count `VirtualLock` and may drop
//! pages from a process working set on its own, after which `VirtualUnlock`
//! reports `ERROR_NOT_LOCKED`. A type whose entire job is holding key material
//! must not abort the process while being dropped. The lock is released
//! explicitly instead, and a failure to unlock is ignored: it leaves pages
//! pinned, which is harmless, rather than unwinding out of a destructor.
//!
//! # In plain words
//!
//! A place to hold a passphrase or a key while it is being used, which tries hard
//! to forget it afterwards.
//!
//! It asks the operating system not to write that memory out to disk, wipes it as
//! soon as it is finished with, and refuses to print itself. That last one matters
//! more than it sounds: secrets most often escape not by being stolen but by
//! appearing in an error message or a log that somebody later sends on.
//!
//! Comparisons take the same amount of time whether or not they match, so nothing
//! is given away by how long an answer took.

use zeroize::Zeroize;

use subtle::ConstantTimeEq;

/// A byte buffer holding key material.
///
/// Page-locked where the OS allows it, zeroized on drop, compared in constant
/// time, and deliberately opaque to `Debug` so a secret can never reach a log
/// line by accident.
pub struct Secret {
    /// Over-sized backing allocation. The usable secret is the `len` bytes at
    /// `offset`; the slack exists so the locked span can be page-aligned.
    backing: Vec<u8>,
    offset: usize,
    len: usize,
    /// Length of the locked span in bytes, or 0 if locking did not happen.
    locked_span: usize,
}

impl Secret {
    /// Wrap `bytes`, taking ownership and wiping the caller's copy.
    pub fn new(bytes: &mut [u8]) -> Self {
        let mut s = Self::zeroed(bytes.len());
        s.expose_mut().copy_from_slice(bytes);
        bytes.zeroize();
        s
    }

    /// Allocate `len` zero bytes, ready to be filled in place.
    pub fn zeroed(len: usize) -> Self {
        if len == 0 {
            return Self {
                backing: Vec::new(),
                offset: 0,
                len: 0,
                locked_span: 0,
            };
        }
        let page = region::page::size();
        // Round up to whole pages, plus one page of slack to align within.
        let span = len.div_ceil(page) * page;
        let backing = vec![0u8; span + page];

        // `align_offset` answers `usize::MAX` if it cannot satisfy the request;
        // then we simply do not lock. Correctness never depends on locking.
        let offset = backing.as_ptr().align_offset(page);
        let (offset, locked_span) = if offset == usize::MAX || offset + span > backing.len() {
            (0, 0)
        } else {
            // Best-effort. `forget` keeps the lock in place without keeping the
            // panicking guard around; `Drop` releases it explicitly.
            let ok = region::lock(backing[offset..].as_ptr(), span)
                .map(std::mem::forget)
                .is_ok();
            (offset, if ok { span } else { 0 })
        };
        Self {
            backing,
            offset,
            len,
            locked_span,
        }
    }

    /// Fill `len` bytes from the operating-system CSPRNG.
    pub fn random(len: usize) -> Result<Self, crate::Error> {
        let mut s = Self::zeroed(len);
        getrandom::getrandom(s.expose_mut()).map_err(|_| crate::Error::Random)?;
        Ok(s)
    }

    /// Whether the pages were successfully locked out of swap.
    ///
    /// False is not an error — see the module documentation — but it is worth
    /// surfacing to the user rather than claiming a guarantee that was not
    /// actually obtained.
    pub fn is_locked(&self) -> bool {
        self.locked_span > 0
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the secret is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Borrow the raw bytes. Callers must not copy them into unprotected
    /// storage.
    pub fn expose(&self) -> &[u8] {
        &self.backing[self.offset..self.offset + self.len]
    }

    /// Borrow mutably, for filling in place.
    pub fn expose_mut(&mut self) -> &mut [u8] {
        &mut self.backing[self.offset..self.offset + self.len]
    }

    /// Wipe the contents now, before the value goes out of scope.
    ///
    /// Zeroizes the slice rather than the `Vec`: `Zeroize for Vec` also
    /// truncates to length zero, which would throw away the allocation.
    pub fn wipe(&mut self) {
        self.backing[..].zeroize();
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // Wipe the whole backing allocation, not just the usable span.
        self.backing[..].zeroize();
        if self.locked_span > 0 {
            // Ignored on purpose: see the module note on not panicking here.
            let _ = region::unlock(self.backing[self.offset..].as_ptr(), self.locked_span);
        }
    }
}

impl Clone for Secret {
    fn clone(&self) -> Self {
        let mut s = Self::zeroed(self.len);
        s.expose_mut().copy_from_slice(self.expose());
        s
    }
}

/// Constant-time equality: comparison never leaks how much of the secret
/// matched through timing.
impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        self.expose().ct_eq(other.expose()).into()
    }
}

impl Eq for Secret {}

/// Deliberately opaque, so a secret cannot reach a log line through `{:?}`.
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Secret({} bytes, redacted)", self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_wipes_the_callers_copy() {
        let mut input = [7u8; 32];
        let s = Secret::new(&mut input);
        assert_eq!(s.expose(), &[7u8; 32]);
        assert_eq!(input, [0u8; 32], "source buffer must be wiped");
    }

    #[test]
    fn random_is_not_all_zeroes_and_differs_each_time() {
        let a = Secret::random(32).unwrap();
        let b = Secret::random(32).unwrap();
        assert_ne!(a.expose(), &[0u8; 32]);
        assert_ne!(a, b, "two draws should not collide");
    }

    #[test]
    fn equality_is_value_based() {
        let mut x = [1u8; 16];
        let mut y = [1u8; 16];
        let mut z = [2u8; 16];
        assert_eq!(Secret::new(&mut x), Secret::new(&mut y));
        assert_ne!(Secret::new(&mut z), Secret::random(16).unwrap());
    }

    #[test]
    fn debug_never_reveals_contents() {
        let mut bytes = [0xABu8; 8];
        let s = Secret::new(&mut bytes);
        let shown = format!("{s:?}");
        assert!(shown.contains("redacted"), "{shown}");
        assert!(!shown.contains("ab") && !shown.contains("AB"), "{shown}");
    }

    #[test]
    fn wipe_clears_in_place() {
        let mut s = Secret::random(64).unwrap();
        s.wipe();
        // Length must survive the wipe, or the "all zero" check below would
        // pass vacuously on an empty slice.
        assert_eq!(s.len(), 64, "wipe must not shrink the secret");
        assert_eq!(s.expose().len(), 64);
        assert!(s.expose().iter().all(|&b| b == 0));
    }

    #[test]
    fn clone_is_independent() {
        let a = Secret::random(32).unwrap();
        let mut b = a.clone();
        assert_eq!(a, b);
        b.wipe();
        assert_ne!(a, b, "wiping the clone must not touch the original");
    }

    /// Locking is best-effort, so this asserts only that it is reported and
    /// never panics — not that it succeeded, which depends on privileges.
    #[test]
    fn locking_is_reported_and_never_panics() {
        let s = Secret::zeroed(4096);
        let _ = s.is_locked();
    }

    /// Regression: locking has page granularity, and `region`'s lock guard
    /// panics if unlocking fails. Between them, sharing a page across secrets
    /// could unlock a live secret's memory or abort from inside a `Drop`.
    /// Creating and dropping many small secrets out of order must be boring.
    #[test]
    fn many_small_secrets_can_coexist_and_drop_in_any_order() {
        let mut live: Vec<Secret> = (0..64).map(|_| Secret::random(32).unwrap()).collect();
        while live.len() > 1 {
            drop(live.remove(live.len() / 2));
            assert!(live.iter().all(|s| s.len() == 32));
        }
    }

    /// The locked span must be page aligned and inside the secret's own
    /// allocation, so no other allocation can share a locked page with it.
    #[test]
    fn locked_span_is_page_aligned() {
        let page = region::page::size();
        for len in [1usize, 31, 32, page - 1, page, page + 1, 5 * page] {
            let s = Secret::zeroed(len);
            if !s.is_locked() {
                continue; // no memlock budget here; nothing to check
            }
            assert_eq!(
                s.expose().as_ptr() as usize % page,
                0,
                "len={len}: secret is not page aligned"
            );
            assert!(s.offset + s.locked_span <= s.backing.len());
        }
    }

    #[test]
    fn zero_length_is_handled() {
        let s = Secret::zeroed(0);
        assert!(s.is_empty());
        assert_eq!(s.expose(), b"");
        assert!(!s.is_locked());
    }

    #[test]
    fn contents_survive_the_alignment_offset() {
        // Regression guard for the offset arithmetic: what goes in comes out.
        for len in [1usize, 33, 4095, 4096, 4097] {
            let mut src = vec![0u8; len];
            for (i, b) in src.iter_mut().enumerate() {
                *b = (i % 251) as u8;
            }
            let expected = src.clone();
            let s = Secret::new(&mut src);
            assert_eq!(s.expose(), &expected[..], "len={len}");
        }
    }
}
