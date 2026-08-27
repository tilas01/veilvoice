// SPDX-License-Identifier: GPL-3.0-or-later
//
// The smallest useful thing you can do with VeilVoice as a library: build the
// engine, push samples through it, get de-identified samples back.
//
//     cargo run -p veilvoice-core --example veil_a_buffer
//
// This file exists so that the corresponding example in
// `docs/USING_THE_CRATES.md` is compiled on every commit rather than written
// out by hand and left to rot. `cargo clippy --workspace --all-targets` builds
// examples, so a signature change breaks CI here before it misleads a reader.
//
// In plain words
// --------------
//
// The shortest possible example of using VeilVoice's engine from your own
// program: hand it some audio, get veiled audio back.
//
// It is here for somebody who wants the voice changing without the rest of
// VeilVoice, and needs to see the whole thing on one screen before deciding.

use veilvoice_core::{DeidConfig, Deidentifier};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Struct-update syntax rather than assigning afterwards: clippy's
    // `field_reassign_with_default` is right that the two-step form reads as
    // if the default mattered when it does not.
    let config = DeidConfig {
        sample_rate: 48_000.0,
        ..DeidConfig::default()
    };

    // `new` validates the whole configuration. Every float is checked for
    // finiteness before range, because `NaN` compares false against every
    // bound and would otherwise build an engine that emits `NaN` for the rest
    // of the session with nothing reported (finding F-10).
    let mut deid = Deidentifier::new(config)?;

    // One second of silence is enough to show the shape of the call. Real
    // input is mono `f32`, nominally in [-1, 1].
    let input: Vec<f32> = vec![0.0; 48_000];
    let veiled = deid.process_vec(&input);

    assert_eq!(veiled.len(), input.len());
    assert!(
        veiled.iter().all(|s| s.is_finite()),
        "the engine must never emit a non-finite sample"
    );

    println!("in  {} samples", input.len());
    println!("out {} samples, all finite", veiled.len());
    println!("last block took {:.3} ms", deid.stats().last_block_ms());
    Ok(())
}
