// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//! Randomised robustness testing for the RIFF chunk walker.
//!
//! `clean_wav_bytes` exists because `lofty` cannot remove ID3v2 from a WAV, so
//! VeilVoice walks the chunk list itself. That means it parses a container
//! somebody else produced, with every length field under their control — the
//! textbook setting for an overrun or a loop that never ends.
//!
//! The properties asserted, for any input at all:
//!
//! 1. **It returns.** No panic, and no unbounded loop: every iteration must
//!    advance `pos`, whatever the chunk sizes claim.
//! 2. **A success is a valid WAV.** If it hands back bytes, those bytes must
//!    parse as RIFF/WAVE with a length field that matches what was written —
//!    it is not permitted to emit something the next tool chokes on.
//! 3. **It never invents audio.** Output length is bounded by input length.
//!
//! Set `VEILVOICE_FUZZ_ROUNDS` to run it longer than the default.

use veilvoice_meta::{clean_wav_bytes, is_wav, Policy};

struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed | 1)
    }
    fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            self.next_u32() as usize % n
        }
    }
    fn byte(&mut self) -> u8 {
        (self.next_u32() >> 24) as u8
    }
}

fn rounds() -> u32 {
    std::env::var("VEILVOICE_FUZZ_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000)
}

/// A minimal but genuinely valid WAV to mutate from.
fn seed_wav() -> Vec<u8> {
    let samples: Vec<u8> = (0..400u32).flat_map(|i| (i as u16).to_le_bytes()).collect();
    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes()); // PCM
    fmt.extend_from_slice(&1u16.to_le_bytes()); // mono
    fmt.extend_from_slice(&48_000u32.to_le_bytes());
    fmt.extend_from_slice(&96_000u32.to_le_bytes());
    fmt.extend_from_slice(&2u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());

    let mut body = Vec::new();
    body.extend_from_slice(b"fmt ");
    body.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    body.extend_from_slice(&fmt);
    // A tag chunk, so the cleaner has something to remove.
    body.extend_from_slice(b"LIST");
    body.extend_from_slice(&12u32.to_le_bytes());
    body.extend_from_slice(b"INFOIART");
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(b"data");
    body.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    body.extend_from_slice(&samples);

    let mut wav = Vec::from(*b"RIFF");
    wav.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(&body);
    wav
}

/// Mutations aimed at the chunk walker specifically: the interesting bytes are
/// the 32-bit sizes, so a fifth of the rounds corrupt one deliberately.
fn mutate(rng: &mut Rng, seed_bytes: &[u8]) -> Vec<u8> {
    let mut out = seed_bytes.to_vec();
    match rng.below(9) {
        0 => {
            if !out.is_empty() {
                let i = rng.below(out.len());
                out[i] ^= 1 << rng.below(8);
            }
        }
        1 => {
            let n = rng.below(out.len() + 1);
            out.truncate(n);
        }
        2 => {
            for _ in 0..rng.below(80) {
                out.push(rng.byte());
            }
        }
        // Lie about a chunk size — the whole point of the exercise.
        3 | 4 => {
            if out.len() >= 8 {
                let i = rng.below(out.len() - 3);
                let v: u32 = match rng.below(6) {
                    0 => u32::MAX,
                    1 => u32::MAX - 7,
                    2 => 0,
                    3 => 1,
                    4 => i32::MAX as u32,
                    _ => rng.next_u32(),
                };
                out[i..i + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        // Lie about the RIFF size specifically.
        5 => {
            if out.len() >= 8 {
                let v = if rng.below(2) == 0 {
                    u32::MAX
                } else {
                    rng.next_u32()
                };
                out[4..8].copy_from_slice(&v.to_le_bytes());
            }
        }
        // Rename a chunk, so unknown ids are exercised.
        6 => {
            if out.len() >= 16 {
                let i = 12 + rng.below(out.len().saturating_sub(16));
                for k in 0..4 {
                    out[i + k] = rng.byte();
                }
            }
        }
        7 => {
            if !out.is_empty() {
                let from = rng.below(out.len());
                let to = (from + rng.below(48)).min(out.len());
                for b in &mut out[from..to] {
                    *b = 0;
                }
            }
        }
        _ => {
            let n = rng.below(300);
            out = (0..n).map(|_| rng.byte()).collect();
            if out.len() >= 12 {
                out[..4].copy_from_slice(b"RIFF");
                out[8..12].copy_from_slice(b"WAVE");
            }
        }
    }
    out
}

fn check_output(seed: u32, input: &[u8], out: &[u8]) {
    assert!(
        is_wav(out),
        "seed {seed}: cleaner produced something that is not a WAV"
    );
    assert!(
        out.len() >= 12,
        "seed {seed}: output too short to be a container"
    );
    let declared = u32::from_le_bytes([out[4], out[5], out[6], out[7]]) as usize;
    assert_eq!(
        declared + 8,
        out.len(),
        "seed {seed}: the RIFF size field does not match the bytes written"
    );
    assert!(
        out.len() <= input.len() + 256,
        "seed {seed}: output grew from {} to {} bytes",
        input.len(),
        out.len()
    );
}

#[test]
fn the_chunk_walker_survives_hostile_input() {
    let valid = seed_wav();
    for seed in 1..=rounds() {
        let mut rng = Rng::new(seed);
        let bytes = mutate(&mut rng, &valid);

        // Reaching the next line is the no-panic, no-hang assertion.
        if let Ok((out, _report)) = clean_wav_bytes(&bytes, Policy::Strip) {
            check_output(seed, &bytes, &out);
        }
    }
}

/// `Policy::Realistic` appends a chunk of its own, which is the one path that
/// can make the output larger than the input.
#[test]
fn the_realistic_policy_survives_hostile_input() {
    let valid = seed_wav();
    for seed in 1..=rounds() {
        let mut rng = Rng::new(seed ^ 0xC0FFEE);
        let bytes = mutate(&mut rng, &valid);
        if let Ok((out, report)) = clean_wav_bytes(&bytes, Policy::Realistic) {
            check_output(seed, &bytes, &out);
            assert!(
                report.changed,
                "seed {seed}: realistic policy always rewrites"
            );
        }
    }
}

#[test]
fn pure_noise_is_rejected_or_handled() {
    for seed in 1..=rounds() {
        let mut rng = Rng::new(seed.wrapping_mul(2_246_822_519));
        let n = rng.below(400);
        let bytes: Vec<u8> = (0..n).map(|_| rng.byte()).collect();
        if let Ok((out, _)) = clean_wav_bytes(&bytes, Policy::Strip) {
            check_output(seed, &bytes, &out);
        }
    }
}

/// A cleaned file must clean again to itself. If a second pass changes
/// anything, the first pass left something behind.
#[test]
fn cleaning_is_idempotent() {
    let valid = seed_wav();
    let (once, first) = clean_wav_bytes(&valid, Policy::Strip).unwrap();
    assert!(first.changed, "the seed file has a LIST chunk to remove");
    let (twice, second) = clean_wav_bytes(&once, Policy::Strip).unwrap();
    assert_eq!(once, twice, "a second pass changed the bytes");
    assert!(!second.changed, "a second pass claimed to remove something");
}

/// Every truncation of a valid file, which is what a partial download or an
/// interrupted recording actually looks like.
#[test]
fn every_truncation_of_a_valid_file_is_handled() {
    let valid = seed_wav();
    for len in 0..=valid.len() {
        if let Ok((out, _)) = clean_wav_bytes(&valid[..len], Policy::Strip) {
            check_output(len as u32, &valid[..len], &out);
        }
    }
}

/// The RIFF size field is a `u32` widened to `usize` and then had 8 added to
/// it. On a 32-bit target — VeilVoice ships an ARMv7 build — `u32::MAX + 8`
/// overflows `usize` and panics under overflow checks. A 64-bit host cannot
/// reach that, so no amount of fuzzing *here* would have found it; it is
/// asserted anyway so the saturating arithmetic is not quietly removed later.
#[test]
fn a_riff_size_of_u32_max_does_not_overflow_the_length_arithmetic() {
    let mut wav = seed_wav();
    wav[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let (out, _) = clean_wav_bytes(&wav, Policy::Strip).expect("should still parse");
    check_output(0, &wav, &out);

    // And the smallest possible file, where the clamp is doing all the work.
    let mut tiny = Vec::from(*b"RIFF");
    tiny.extend_from_slice(&u32::MAX.to_le_bytes());
    tiny.extend_from_slice(b"WAVE");
    assert!(clean_wav_bytes(&tiny, Policy::Strip).is_err());
}

/// A chunk that declares a size of zero must still advance the walker. If it
/// did not, this would never return — which is why it is asserted rather than
/// assumed.
#[test]
fn zero_sized_chunks_do_not_stall_the_walker() {
    let mut wav = Vec::from(*b"RIFF");
    let mut body = Vec::new();
    for _ in 0..500 {
        body.extend_from_slice(b"junk");
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    body.extend_from_slice(b"data");
    body.extend_from_slice(&4u32.to_le_bytes());
    body.extend_from_slice(&[0u8; 4]);
    wav.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(&body);

    let (out, report) = clean_wav_bytes(&wav, Policy::Strip).unwrap();
    check_output(0, &wav, &out);
    assert!(report.changed);
}
