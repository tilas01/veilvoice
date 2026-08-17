// SPDX-License-Identifier: GPL-3.0-or-later
//! The RIFF chunk walker in `veilvoice-meta`, coverage-guided.
//!
//! This one walks a flat list of chunks whose sizes come from the file, so its
//! termination depends on values an attacker chooses -- the shape F-4 had.
//!
//! The interesting property is not only "does not crash": a *cleaned* WAV is
//! handed back to the user as safe, so it has to actually be a WAV, and its
//! RIFF size field has to describe the bytes that were written. A cleaner that
//! returns a corrupt file has failed even though it did not panic.

#![no_main]

use libfuzzer_sys::fuzz_target;
use veilvoice_meta::{clean_wav_bytes, is_wav, Policy};

fuzz_target!(|data: &[u8]| {
    for policy in [Policy::Strip, Policy::Realistic] {
        let Ok((cleaned, _report)) = clean_wav_bytes(data, policy) else {
            continue;
        };

        assert!(
            is_wav(&cleaned),
            "the cleaner returned something that is not a WAV"
        );

        // The size field must describe the file, or the result does not open.
        let declared = u32::from_le_bytes([cleaned[4], cleaned[5], cleaned[6], cleaned[7]]) as u64;
        assert_eq!(
            declared + 8,
            cleaned.len() as u64,
            "RIFF size field does not match the length written"
        );

        // Cleaning is idempotent: a file this crate produced must survive being
        // cleaned again, and must not change. If it does not, the output was
        // not really clean.
        if policy == Policy::Strip {
            let (twice, report) =
                clean_wav_bytes(&cleaned, Policy::Strip).expect("a cleaned WAV must re-clean");
            assert!(!report.changed, "a cleaned WAV still had something to strip");
            assert_eq!(twice, cleaned, "re-cleaning changed the bytes");
        }
    }
});
