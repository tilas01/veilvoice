// SPDX-License-Identifier: GPL-3.0-or-later
//! The WAV pre-flight in `veilvoice-audio`, coverage-guided.
//!
//! This check exists because a WAV declaring a sample rate of zero makes
//! `symphonia` panic inside its own probe, before this project sees anything it
//! could inspect -- and under the shipped `panic = "abort"` profile that is the
//! process ending, not an error a caller can handle.
//!
//! It is therefore a parser standing in front of a crash, which makes its own
//! robustness load-bearing: it walks chunk sizes taken from the file, so it has
//! exactly the termination-depends-on-a-length-field shape that F-4 had. If
//! this ever hangs or panics it has become the bug it was written to prevent.

#![no_main]

use libfuzzer_sys::fuzz_target;
use veilvoice_audio::io::preflight;

fuzz_target!(|data: &[u8]| {
    // The only assertion that matters is reaching the next line: no panic, and
    // -- since libFuzzer's timeout catches a spin -- no unbounded loop.
    let _ = preflight(data);

    // Every prefix must behave too. A truncated download is the ordinary case,
    // not an exotic one, and an off-by-one at the end of the buffer is the
    // classic way a chunk walker reads past it.
    for cut in [
        0,
        data.len() / 4,
        data.len() / 2,
        data.len().saturating_sub(1),
    ] {
        let _ = preflight(&data[..cut.min(data.len())]);
    }
});
