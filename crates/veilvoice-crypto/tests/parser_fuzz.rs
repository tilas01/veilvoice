// SPDX-License-Identifier: GPL-3.0-or-later
//! Randomised robustness testing for the two parsers that read untrusted input.
//!
//! # What is being defended
//!
//! [`container::Header::parse`] reads a file somebody sent you.
//! [`lock::AppLock::parse`] reads the app-lock file, which is worse: it is
//! parsed **before anyone has authenticated anything**, so it is the first
//! bytes the program touches on a locked machine.
//!
//! For a parser in a security tool the bar is not "usually returns the right
//! answer". It is:
//!
//! 1. **Never panic.** A panic on hostile input is a denial of service, and in
//!    a `panic = "abort"` release profile it is the whole process.
//! 2. **Never hang.** Every loop must be bounded by the input, not by a length
//!    field the input controls.
//! 3. **Never report success for something it did not fully understand.** An
//!    `Ok` must come with offsets that are actually inside the buffer.
//!
//! # Why this and not `cargo fuzz`
//!
//! `cargo fuzz` needs nightly and libFuzzer, which is a poor fit for a project
//! that pins a stable toolchain and wants every check runnable by anyone who
//! cloned it. This is a deterministic campaign instead: a seeded PRNG, so a
//! failure is reproducible from its seed rather than being a story about a run
//! nobody can repeat, and structure-aware mutation, so the bytes spend their
//! time near the interesting boundaries rather than being rejected at the magic
//! number.
//!
//! It is **not** a substitute for a coverage-guided fuzzer and `docs/AUDIT.md`
//! does not claim it is. It is the campaign that can actually be run on every
//! commit, on every platform, by everybody.
//!
//! Set `VEILVOICE_FUZZ_ROUNDS` to run it longer than the default.
//!
//! # In plain words
//!
//! Throws malformed and deliberately hostile encrypted files at the code that
//! reads them, in bulk.
//!
//! Reading a file somebody else made is where most security problems live. Every
//! one of these has to be refused with a reason: never accepted, and never able to
//! bring the program down.

use veilvoice_crypto::{container, kdf, lock};

/// xorshift32. Deterministic, seeded, and twenty lines — a dependency here
/// would be a dependency in the audit surface for no benefit.
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

/// Mutate `seed_bytes` in one of the ways that break parsers in practice.
///
/// Deliberately biased towards length fields and boundaries: uniformly random
/// bytes almost never get past a magic number, so a campaign made only of them
/// tests the first four bytes very thoroughly and nothing else at all.
fn mutate(rng: &mut Rng, seed_bytes: &[u8]) -> Vec<u8> {
    let mut out = seed_bytes.to_vec();
    match rng.below(8) {
        // Flip a bit.
        0 => {
            if !out.is_empty() {
                let i = rng.below(out.len());
                out[i] ^= 1 << rng.below(8);
            }
        }
        // Replace a byte outright.
        1 => {
            if !out.is_empty() {
                let i = rng.below(out.len());
                out[i] = rng.byte();
            }
        }
        // Truncate — the classic way a length field outruns the buffer.
        2 => {
            let n = rng.below(out.len() + 1);
            out.truncate(n);
        }
        // Extend with noise.
        3 => {
            for _ in 0..rng.below(64) {
                out.push(rng.byte());
            }
        }
        // Corrupt a 32-bit field with an extreme value.
        4 => {
            if out.len() >= 4 {
                let i = rng.below(out.len() - 3);
                let v: u32 = match rng.below(5) {
                    0 => u32::MAX,
                    1 => u32::MAX - 1,
                    2 => 0,
                    3 => i32::MAX as u32,
                    _ => rng.next_u32(),
                };
                out[i..i + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        // Splice a region onto itself.
        5 => {
            if out.len() > 8 {
                let from = rng.below(out.len());
                let len = rng.below(out.len() - from);
                let piece = out[from..from + len].to_vec();
                let at = rng.below(out.len());
                out.splice(at..at, piece);
            }
        }
        // Zero a run.
        6 => {
            if !out.is_empty() {
                let from = rng.below(out.len());
                let to = (from + rng.below(32)).min(out.len());
                for b in &mut out[from..to] {
                    *b = 0;
                }
            }
        }
        // Start from scratch: pure noise, occasionally with the right magic.
        _ => {
            let n = rng.below(200);
            out = (0..n).map(|_| rng.byte()).collect();
            if rng.below(2) == 0 && out.len() >= 8 {
                out[..8].copy_from_slice(container::MAGIC);
            }
        }
    }
    out
}

fn weak() -> kdf::KdfParams {
    kdf::KdfParams::weak_for_tests()
}

/// Whether it is worth actually running the KDF for these parameters.
///
/// Mutation cheerfully produces costs that are *valid* and enormous — a
/// 4 GiB-and-three-passes Argon2 is a perfectly legal header. Executing those
/// turns a campaign into a benchmark, so the KDF-running half of each round is
/// limited to cheap parameters and the expensive ones are covered by the unit
/// tests in `kdf.rs` instead.
///
/// Worth stating plainly, because it is a real property and not just a test
/// convenience: an attacker who hands you a container **can** make opening it
/// slow, because the cost travels with the file and that is the whole point of
/// the design. Slow is not the same as crashing, the user chose to open that
/// file, and they can stop waiting. The crash was the bug; the delay is the
/// documented trade.
fn cheap(params: kdf::KdfParams) -> bool {
    params.checked().is_ok() && params.m_cost <= 256 && params.t_cost <= 4 && params.p_cost <= 4
}

#[test]
fn the_container_header_parser_survives_hostile_input() {
    let valid = container::seal_with_password(b"pw", b"a recording", weak()).unwrap();
    let mut checked = 0u32;

    for seed in 1..=rounds() {
        let mut rng = Rng::new(seed);
        let bytes = mutate(&mut rng, &valid);

        // Property 1: it must return, and must not panic. Reaching the next
        // line at all is the assertion.
        if let Ok((header, body)) = container::Header::parse(&bytes) {
            // Property 3: a success must describe the buffer it was given.
            assert!(
                body <= bytes.len(),
                "seed {seed}: parse reported ciphertext starting at {body} \
                 in a {}-byte buffer",
                bytes.len()
            );
            assert!(
                body >= container::HEADER_LEN,
                "seed {seed}: body offset {body} is inside the fixed header"
            );
            assert_eq!(
                header.encapsulation.len(),
                body - container::HEADER_LEN,
                "seed {seed}: encapsulation length disagrees with the offset"
            );
            // Re-serialising a parsed header must reproduce the bytes it was
            // parsed from. If it does not, the parser accepted something it did
            // not fully account for.
            assert_eq!(
                header.to_bytes(),
                &bytes[..body],
                "seed {seed}: header does not round-trip"
            );

            // The guard itself must hold for every parsed header, cheap or not.
            // This is the check that stops an absurd cost reaching Argon2.
            let _ = header.kdf.checked();
            // And the full open path, which is what actually faces a hostile
            // file — only for costs cheap enough to run: see `cheap`.
            if cheap(header.kdf) {
                let _ = container::open_with_password(b"pw", &bytes);
            }
        }
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn the_app_lock_parser_survives_hostile_input() {
    let valid = lock::AppLock::create(b"pw", weak()).unwrap().to_bytes();

    for seed in 1..=rounds() {
        let mut rng = Rng::new(seed ^ 0x5EED);
        let bytes = mutate(&mut rng, &valid);

        if let Ok(mut parsed) = lock::AppLock::parse(&bytes) {
            // A parsed lock must re-serialise to exactly what it came from.
            // Anything else means a field was ignored, and an ignored field in
            // a file read before authentication is precisely the place not to
            // have one.
            assert_eq!(
                parsed.to_bytes(),
                bytes,
                "seed {seed}: lock file does not round-trip"
            );
            // The cooldown must be a finite answer whatever the stored counters
            // say — including a failure count of u32::MAX and a timestamp from
            // the far future, which mutation reaches often.
            let _ = parsed.cooldown();
            assert!(lock::delay_secs(parsed.failures()) <= 15 * 60);

            // And the path that actually runs on a locked machine. Parsing
            // alone missed the Argon2 parallelism overflow entirely, because
            // the corrupt cost parameters only reach the KDF here.
            if cheap(parsed.params()) {
                let _ = parsed.verify(b"whatever the user typed");
            }
            // Whatever the file said, the guard must have an opinion about it
            // and must not panic forming one.
            let _ = parsed.params().checked();
        }
    }
}

/// Pure noise, with no valid seed to start from. Cheap, and it covers the
/// "someone pointed it at an unrelated file" case that structured mutation
/// never reaches.
#[test]
fn both_parsers_survive_pure_noise() {
    for seed in 1..=rounds() {
        let mut rng = Rng::new(seed.wrapping_mul(2_654_435_761));
        let n = rng.below(300);
        let bytes: Vec<u8> = (0..n).map(|_| rng.byte()).collect();

        if let Ok((header, _)) = container::Header::parse(&bytes) {
            if cheap(header.kdf) {
                let _ = container::open_with_password(b"pw", &bytes);
            }
        }
        if let Ok(parsed) = lock::AppLock::parse(&bytes) {
            let _ = parsed.params().checked();
        }
    }
}

/// The lengths a parser gets wrong are the ones either side of a boundary, and
/// a random campaign hits them only by luck. This walks them deliberately.
#[test]
fn every_length_around_a_boundary_is_handled() {
    let container_bytes = container::seal_with_password(b"pw", b"x", weak()).unwrap();
    let lock_bytes = lock::AppLock::create(b"pw", weak()).unwrap().to_bytes();

    for len in 0..=container_bytes.len() {
        let _ = container::Header::parse(&container_bytes[..len]);
        let _ = container::open_with_password(b"pw", &container_bytes[..len]);
    }

    for len in 0..=lock_bytes.len() + 8 {
        let mut padded = lock_bytes.clone();
        padded.resize(len, 0);
        let _ = lock::AppLock::parse(&padded);
    }

    // Exactly the right length parses; one byte either side does not.
    assert!(lock::AppLock::parse(&lock_bytes).is_ok());
    assert!(lock::AppLock::parse(&lock_bytes[..lock_bytes.len() - 1]).is_err());
    let mut long = lock_bytes.clone();
    long.push(0);
    assert!(lock::AppLock::parse(&long).is_err());
}

/// **F-82.** The one input the coverage-guided campaign found, kept here.
///
/// `fuzz/README.md` says these two campaigns are different things and both are
/// kept, and this is what that means in practice: the nightly campaign found an
/// input, and the input lives here, where it is checked on every commit on
/// every platform by anybody who cloned the repository.
///
/// The bytes are a whole `.veil` header declaring `m_cost` 65535, `t_cost`
/// 4521984 and `p_cost` 1280. Nothing about them overflows, nothing allocates
/// beyond the memory ceiling, and `m_cost >= p_cost * 8` holds, so every check
/// this file's other tests make passed. The only thing wrong with them is that
/// the derivation would not finish: about 74 hours, measured in a release
/// build before `MAX_T_COST` existed.
///
/// Written out as bytes rather than built from `KdfParams`, because the point
/// is the file, and a test that constructs the parameters would keep passing if
/// the header ever stopped putting them where it puts them.
#[test]
fn the_header_the_coverage_guided_campaign_found_is_refused() {
    let mut header = vec![0u8; 69];
    header[..8].copy_from_slice(b"VEILVOX1");
    header[8] = 1; // format version
    header[9] = 1; // password mode
    header[12..16].copy_from_slice(&65_535u32.to_le_bytes()); // m_cost
    header[16..20].copy_from_slice(&4_521_984u32.to_le_bytes()); // t_cost
    header[20..24].copy_from_slice(&1280u32.to_le_bytes()); // p_cost

    // The header still parses: the numbers are structurally fine, and pretending
    // otherwise would be testing the wrong thing.
    let parsed = container::Header::parse(&header);
    assert!(
        parsed.is_ok(),
        "the header itself is well formed: {parsed:?}"
    );

    // What must not happen is that opening it runs. On a thread with a deadline,
    // because the defect this guards against is a *hang*: timing the call after
    // it returns cannot fail, it can only never finish, and a test that hangs
    // says less than one that fails and says why.
    let (done, answer) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let refused = container::open_with_password(b"not the password", &header).is_err();
        let _ = done.send(refused);
    });
    match answer.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(refused) => assert!(refused, "a header this expensive must be refused"),
        Err(_) => panic!(
            "open_with_password did not come back within five seconds, so it is \
             deriving rather than refusing. That is F-82: t_cost is unbounded."
        ),
    }
}
