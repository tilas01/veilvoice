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
//! audit found exactly that: one NaN, and every output sample for the rest of
//! the session was NaN, silently.
//!
//! # In plain words
//!
//! Feeds the engine audio designed to break it: silence, deafening noise, values
//! that are not numbers, files that lie about their own length.
//!
//! The question is not whether it sounds good. It is whether anything can make the
//! engine stop, hang, or quietly produce silence while reporting success. A
//! de-identifier that fails by outputting nothing is one somebody might not notice
//! had failed.

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

// ---------------------------------------------------------------------------
// Hostile *configuration*, as opposed to hostile samples.
//
// The section above covers a bad sample reaching the engine. These cover the
// second door onto the same failure, which the audit found still open: the
// engine was built from a configuration nobody had validated, and a `NaN`
// sample rate produced `NaN` output for every sample without a sample ever
// having been bad. The value is reachable from a file, because a WAV's `fmt `
// chunk carries a `u32` sample rate that `symphonia` passes straight through, and
// from any project using these crates as libraries, which the README invites.
// ---------------------------------------------------------------------------

/// A non-finite sample rate must be refused, not built.
///
/// Before the fix, `NaN` passed validation (because `NaN < 8_000.0` is false),
/// the engine built happily, and **every output sample was `NaN`**, silently
/// and for ever. `INFINITY` was worse in a different way: it reached an
/// `as usize` saturation followed by an addition, and panicked with an
/// arithmetic overflow in every build with overflow checks on.
#[test]
fn a_non_finite_sample_rate_is_refused_rather_than_built() {
    for rate in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let config = DeidConfig {
            sample_rate: rate,
            ..Default::default()
        };
        let err = config
            .checked()
            .expect_err(&format!("sample_rate {rate} was accepted"));
        assert!(err.contains("real number"), "{err}");
        assert!(
            Deidentifier::new(config).is_err(),
            "an engine was built at sample_rate {rate}"
        );
    }
}

/// A sample rate a file can legally declare, but no hardware produces, sizes
/// the reverb and chorus delay lines. `u32::MAX` asked for about two gigabytes
/// of buffers from a four-kilobyte file, and a failed allocation aborts.
#[test]
fn an_absurd_sample_rate_is_refused_rather_than_allocated() {
    for rate in [
        u32::MAX as f32,
        2e9,
        DeidConfig::MAX_SAMPLE_RATE * 2.0,
        DeidConfig::MAX_SAMPLE_RATE + 1.0,
    ] {
        let config = DeidConfig {
            sample_rate: rate,
            ..Default::default()
        };
        assert!(
            config.checked().is_err(),
            "sample_rate {rate} Hz was accepted"
        );
    }
    // The ceiling itself works, and sits above every real converter.
    assert!(DeidConfig {
        sample_rate: DeidConfig::MAX_SAMPLE_RATE,
        ..Default::default()
    }
    .checked()
    .is_ok());
    for real in [8_000.0, 16_000.0, 44_100.0, 48_000.0, 96_000.0, 384_000.0] {
        assert!(
            DeidConfig {
                sample_rate: real,
                ..Default::default()
            }
            .checked()
            .is_ok(),
            "{real} Hz is a real rate and must still build"
        );
    }
}

/// `frame_size` had no upper bound at all, and it sizes every internal buffer
/// and the FFT plan.
#[test]
fn an_absurd_frame_size_is_refused() {
    for frame_size in [
        DeidConfig::MAX_FRAME_SIZE + 2,
        1 << 26,
        (usize::MAX / 2) & !1,
    ] {
        assert!(
            DeidConfig {
                frame_size,
                overlap: 2,
                ..Default::default()
            }
            .checked()
            .is_err(),
            "frame_size {frame_size} was accepted"
        );
    }
    assert!(DeidConfig {
        frame_size: DeidConfig::MAX_FRAME_SIZE,
        ..Default::default()
    }
    .checked()
    .is_ok());
}

/// Every other float is either clamped to something meaningful or refused for
/// being `NaN`. None of them may reach the engine unexamined.
#[test]
fn non_finite_parameters_are_refused_and_wild_ones_are_clamped() {
    // A named function-pointer type rather than a boxed closure: no allocation,
    // no trait object, and a signature short enough to read.
    type Poison = (&'static str, fn(&mut DeidConfig));

    let named: [Poison; 8] = [
        ("intensity", |c| c.intensity = f32::NAN),
        ("mod_smooth", |c| c.mod_smooth = f32::NAN),
        ("distortion_drive", |c| c.distortion_drive = f32::NAN),
        ("distortion_mix", |c| c.distortion_mix = f32::NAN),
        ("chorus_mix", |c| c.chorus_mix = f32::NAN),
        ("reverb_mix", |c| c.reverb_mix = f32::NAN),
        ("pitch_bounds.0", |c| c.pitch_bounds.0 = f32::NAN),
        ("formant_bounds.1", |c| c.formant_bounds.1 = f32::INFINITY),
    ];
    for (name, poison) in named {
        let mut config = DeidConfig::default();
        poison(&mut config);
        let err = config
            .checked()
            .expect_err(&format!("{name} accepted a non-finite value"));
        assert!(
            err.contains(name),
            "error for {name} does not name it: {err}"
        );
    }

    // Finite but wild values are brought back to something the DSP can act on
    // rather than refused, because each has a sensible nearest legal value.
    let tamed = DeidConfig {
        intensity: 99.0,
        distortion_drive: 1e9,
        chorus_mix: -5.0,
        pitch_bounds: (500.0, 0.0001),
        ..Default::default()
    }
    .checked()
    .expect("finite-but-wild values should be clamped, not refused");
    assert_eq!(tamed.intensity, 1.0);
    assert_eq!(tamed.chorus_mix, 0.0);
    assert!(tamed.distortion_drive <= 64.0);
    assert!(
        tamed.pitch_bounds.0 <= tamed.pitch_bounds.1,
        "bounds came back inverted: {:?}",
        tamed.pitch_bounds
    );
}

/// The whole point, checked end to end: a configuration that survives
/// validation must produce finite audio from finite audio.
#[test]
fn every_configuration_that_builds_produces_finite_audio() {
    for rate in [8_000.0f32, 44_100.0, 48_000.0, 192_000.0] {
        let config = DeidConfig {
            sample_rate: rate,
            accent: AccentConfig::default(),
            ..Default::default()
        };
        let mut deid = Deidentifier::new(config).unwrap();
        let out = deid.process_vec(&speech(rate as u32, 0.1));
        assert!(
            out.iter().all(|v| v.is_finite()),
            "{rate} Hz produced non-finite output"
        );
    }
}
