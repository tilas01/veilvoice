// SPDX-License-Identifier: GPL-3.0-or-later
//! The engine against input that is not well-behaved audio.
//!
//! A realistic use of VeilVoice is "someone sent me a recording and I want to
//! veil it before passing it on", so the input file is not necessarily
//! friendly. A 32-bit-float WAV can legally contain NaN and infinity, and
//! `symphonia` decodes those faithfully rather than sanitising them.
//!
//! That matters more than it might sound, because the engine keeps *persistent*
//! state: the accent neutraliser's long-term spectrum is an exponential moving
//! average, so a single non-finite sample folded into it never washes out. The
//! audit found exactly that — one NaN, and every output sample for the rest of
//! the session was NaN, silently.

use veilvoice_core::{AccentConfig, DeidConfig, Deidentifier};

fn speech(sample_rate: u32, secs: f32) -> Vec<f32> {
    let n = (sample_rate as f32 * secs) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (1..=12)
                .map(|h| (std::f32::consts::TAU * 150.0 * h as f32 * t).sin() / h as f32)
                .sum::<f32>()
                * 0.1
        })
        .collect()
}

fn engine() -> Deidentifier {
    Deidentifier::new(DeidConfig::default()).unwrap()
}

/// The regression. One bad sample must not end the session.
#[test]
fn a_single_nan_does_not_poison_the_engine_for_ever() {
    let mut deid = engine();
    let clean = speech(48_000, 0.5);

    let mut poisoned = clean.clone();
    poisoned[500] = f32::NAN;
    let during = deid.process_vec(&poisoned);
    assert!(
        during.iter().all(|v| v.is_finite()),
        "the frame containing the NaN produced non-finite output"
    );

    let after = deid.process_vec(&clean);
    assert!(
        after.iter().all(|v| v.is_finite()),
        "the engine did not recover: {} of {} samples are non-finite",
        after.iter().filter(|v| !v.is_finite()).count(),
        after.len()
    );
    let energy: f32 = after.iter().map(|v| v * v).sum();
    assert!(
        energy > 1e-6,
        "the engine recovered into silence, energy {energy:e}"
    );
}

#[test]
fn every_flavour_of_non_finite_is_survived() {
    for (name, poison) in [
        ("NaN", f32::NAN),
        ("+inf", f32::INFINITY),
        ("-inf", f32::NEG_INFINITY),
        ("huge", 1e38),
        ("tiny", -1e38),
    ] {
        let mut deid = engine();
        let clean = speech(48_000, 0.3);
        let mut bad = clean.clone();
        // Scatter it, rather than trusting one position to be reached.
        for i in (100..bad.len()).step_by(997) {
            bad[i] = poison;
        }
        let out = deid.process_vec(&bad);
        assert!(
            out.iter().all(|v| v.is_finite()),
            "{name}: output went non-finite"
        );
        let recovered = deid.process_vec(&clean);
        assert!(
            recovered.iter().all(|v| v.is_finite()),
            "{name}: engine did not recover"
        );
    }
}

/// An input made entirely of poison must not panic, hang, or emit garbage.
#[test]
fn an_entirely_non_finite_buffer_is_handled() {
    let mut deid = engine();
    let out = deid.process_vec(&vec![f32::NAN; 24_000]);
    assert!(out.iter().all(|v| v.is_finite()));

    let recovered = deid.process_vec(&speech(48_000, 0.5));
    assert!(recovered.iter().all(|v| v.is_finite()));
}

/// Silence must not drive the long-term averages anywhere strange, and must not
/// come out as anything but silence.
#[test]
fn digital_silence_stays_silent_and_leaves_the_state_usable() {
    let mut deid = engine();
    let out = deid.process_vec(&vec![0.0f32; 48_000]);
    assert!(out.iter().all(|v| v.is_finite()));
    assert!(
        out.iter().all(|v| v.abs() < 1e-3),
        "silence in, noise out: peak {}",
        out.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    );

    let after = deid.process_vec(&speech(48_000, 0.5));
    assert!(after.iter().all(|v| v.is_finite()));
    assert!(after.iter().map(|v| v * v).sum::<f32>() > 1e-6);
}

/// Full-scale square waves and DC are legal audio and unlike anything the
/// engine was tuned on.
#[test]
fn pathological_but_legal_audio_is_handled() {
    for (name, signal) in [
        ("DC", vec![1.0f32; 24_000]),
        (
            "square",
            (0..24_000)
                .map(|i| if (i / 24) % 2 == 0 { 1.0 } else { -1.0 })
                .collect(),
        ),
        (
            "impulses",
            (0..24_000)
                .map(|i| if i % 1000 == 0 { 1.0 } else { 0.0 })
                .collect(),
        ),
        (
            "alternating",
            (0..24_000)
                .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
                .collect(),
        ),
    ] {
        let mut deid = engine();
        let out = deid.process_vec(&signal);
        assert!(
            out.iter().all(|v| v.is_finite()),
            "{name}: non-finite output"
        );
        assert!(
            out.iter().all(|v| v.abs() <= 4.0),
            "{name}: output ran away to {}",
            out.iter().fold(0.0f32, |m, v| m.max(v.abs()))
        );
    }
}

/// The same, with accent neutralisation off, since that changes which paths in
/// `spectral.rs` run.
#[test]
fn hostile_input_is_survived_with_accent_neutralisation_off() {
    let cfg = DeidConfig {
        accent: AccentConfig {
            enabled: false,
            ..AccentConfig::default()
        },
        ..DeidConfig::default()
    };
    let mut deid = Deidentifier::new(cfg).unwrap();
    let clean = speech(48_000, 0.3);
    let mut bad = clean.clone();
    bad[77] = f32::NAN;
    bad[1_500] = f32::INFINITY;

    assert!(deid.process_vec(&bad).iter().all(|v| v.is_finite()));
    assert!(deid.process_vec(&clean).iter().all(|v| v.is_finite()));
}
