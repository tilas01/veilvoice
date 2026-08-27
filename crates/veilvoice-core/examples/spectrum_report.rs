// SPDX-License-Identifier: GPL-3.0-or-later
//! Where do the output partials actually land?
//!
//! Prints the strongest partials of a synthetic speaker before and after
//! processing, as multiples of the STFT bin grid. It is the quickest way to see
//! the two synthesis modes described in `spectral.rs`:
//!
//! * **accent off** — the legacy channel-vocoder path. Each harmonic smears into
//!   a cluster of neighbouring grid frequencies (187.5 *and* 234.4 Hz around one
//!   partial), which is what makes it sound metallic.
//! * **accent on** — an exact harmonic series at the canonical register
//!   (140.6 Hz and its multiples), whatever pitch went in.
//!
//! Run with `cargo run -p veilvoice-core --example diag_spectrum`.
//!
//! # In plain words
//!
//! A small program that runs the engine over a recording and prints what changed
//! about its frequencies.
//!
//! It is here so that the claims about what VeilVoice does to a voice can be
//! checked by anybody, rather than taken on trust from a paragraph of prose.

use realfft::RealFftPlanner;
use veilvoice_core::{AccentConfig, DeidConfig, Deidentifier};

fn speaker(f0: f32, secs: f32) -> Vec<f32> {
    let sr = 48_000.0f32;
    (0..(sr * secs) as usize)
        .map(|i| {
            let t = i as f32 / sr;
            let mut s = 0.0;
            for h in 1..=24 {
                let f = f0 * h as f32;
                if f > sr * 0.45 {
                    break;
                }
                let mut g = 1.0 / h as f32;
                for &cf in &[700.0f32, 1220.0, 2600.0] {
                    g += 0.9 / (1.0 + ((f - cf) / 110.0).powi(2)) / h as f32;
                }
                s += g * (std::f32::consts::TAU * f * t).sin();
            }
            s * 0.1
        })
        .collect()
}

fn peaks(sig: &[f32], label: &str) {
    let n = 16384;
    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(n);
    let mut buf: Vec<f32> = sig[..n]
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
            x * w
        })
        .collect();
    let mut spec = r2c.make_output_vec();
    r2c.process(&mut buf, &mut spec).unwrap();
    let mag: Vec<f32> = spec.iter().map(|c| c.norm()).collect();
    let bin_hz = 48_000.0 / n as f32;

    let mut idx: Vec<usize> = (2..mag.len() - 2)
        .filter(|&k| mag[k] > mag[k - 1] && mag[k] >= mag[k + 1])
        .collect();
    idx.sort_by(|&a, &b| mag[b].partial_cmp(&mag[a]).unwrap());
    println!("\n{label}: strongest partials (Hz)");
    let mut top: Vec<f32> = idx.iter().take(10).map(|&k| k as f32 * bin_hz).collect();
    top.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for f in &top {
        // 46.875 Hz is the synthesis bin grid at n=1024 / 48 kHz.
        println!("   {f:8.1}   grid multiple = {:6.2}", f / 46.875);
    }
}

fn main() {
    let input = speaker(210.0, 3.0);
    peaks(&input, "input (210 Hz)");

    let cfg = DeidConfig {
        intensity: 0.0,
        distortion_mix: 0.0,
        chorus_mix: 0.0,
        reverb_mix: 0.0,
        accent: AccentConfig {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let out = Deidentifier::from_seed(cfg, [3u8; 32])
        .unwrap()
        .process_vec(&input);
    peaks(&out[out.len() / 2..], "output (accent off, intensity 0)");

    let cfg_on = DeidConfig {
        accent: AccentConfig::default(),
        ..cfg
    };
    let out_on = Deidentifier::from_seed(cfg_on, [3u8; 32])
        .unwrap()
        .process_vec(&input);
    peaks(&out_on[out_on.len() / 2..], "output (accent on)");
}
