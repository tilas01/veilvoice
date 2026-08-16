// SPDX-License-Identifier: GPL-3.0-or-later
//! Timing measurement of the password paths.
//!
//! `docs/AUDIT.md` listed this as outstanding: Argon2id is inherently
//! constant-ish, but nobody had measured the code *around* it. The question is
//! whether the time an attempt takes leaks anything about the password —
//! classically, whether a byte-by-byte comparison returns early and turns
//! "how long did that take" into "how many characters were right".
//!
//! # These are ignored by default, and that is deliberate
//!
//! A timing test on a shared CI runner measures the neighbours, not the code.
//! Run them on a quiet machine and read the numbers:
//!
//! ```text
//! cargo test -p veilvoice-crypto --release --test timing -- --ignored --nocapture
//! ```
//!
//! The thresholds below are loose on purpose. They are there to catch a
//! *catastrophic* regression — someone replacing a constant-time comparison
//! with `==`, which shows up as a difference of orders of magnitude — not to
//! certify a bound in nanoseconds, which this method cannot honestly do.

use std::time::{Duration, Instant};
use veilvoice_crypto::{container, kdf, lock};

/// Cheap parameters on purpose: a fast KDF makes the *comparison* a larger
/// share of the total, so a non-constant-time one is easier to see. Measuring
/// with the 256 MiB default would bury any leak under Argon2's own noise, which
/// would be a comfortable way to prove nothing.
fn params() -> kdf::KdfParams {
    kdf::KdfParams {
        m_cost: 64,
        t_cost: 1,
        p_cost: 1,
    }
}

const SAMPLES: usize = 2_000;

/// What a run of samples looked like.
///
/// The headline figure is the **minimum**, not the mean or the median. Timing
/// noise on a real machine is one-sided: a scheduler, an interrupt or a cache
/// miss can only ever make a sample slower, never faster. The fastest sample is
/// therefore the closest estimate of the work the code actually does, and it is
/// far more stable across runs than any average — which on the first pass of
/// these tests moved the "ratio" by 50% purely from Windows scheduling.
struct Stats {
    min: Duration,
    median: Duration,
    iqr: Duration,
}

fn summarise(times: Vec<Duration>) -> Stats {
    let mut sorted = times;
    sorted.sort_unstable();
    Stats {
        min: sorted[0],
        median: sorted[sorted.len() / 2],
        iqr: sorted[sorted.len() * 3 / 4] - sorted[sorted.len() / 4],
    }
}

fn show(label: &str, s: &Stats) {
    println!(
        "    {label:<30} min {:>9.3?}   median {:>9.3?}   IQR {:>9.3?}",
        s.min, s.median, s.iqr
    );
}

fn time_it(runs: usize, mut body: impl FnMut()) -> Stats {
    // A warm-up pass, so the first sample is not measuring page faults and
    // branch predictors rather than the code.
    for _ in 0..50 {
        body();
    }
    summarise(
        (0..runs)
            .map(|_| {
                let start = Instant::now();
                body();
                start.elapsed()
            })
            .collect(),
    )
}

/// Time one call each against a batch of values prepared *outside* the clock.
///
/// Needed wherever a single measurement would otherwise have to include its own
/// setup: the app lock counts failures, so timing repeated wrong guesses on one
/// lock either trips the rate limiter or has to reset it inside the timed
/// region — which is how the first version of this test ended up comparing two
/// derivations against one and reporting a meaningless 2× "leak".
fn time_each<T>(items: &mut [T], mut body: impl FnMut(&mut T)) -> Stats {
    summarise(
        items
            .iter_mut()
            .map(|item| {
                let start = Instant::now();
                body(item);
                start.elapsed()
            })
            .collect(),
    )
}

fn ratio(a: &Stats, b: &Stats) -> f64 {
    a.min.as_secs_f64() / b.min.as_secs_f64().max(1e-12)
}

#[test]
#[ignore = "timing measurement; run by hand on a quiet machine"]
fn opening_a_container_does_not_leak_how_much_of_the_password_was_right() {
    let sealed =
        container::seal_with_password(b"correct horse battery staple", b"audio", params()).unwrap();

    // Wrong from the first character, versus wrong only in the last. A
    // comparison that gave up early would make the second markedly slower.
    let early = time_it(SAMPLES, || {
        let _ = container::open_with_password(b"xorrect horse battery staple", &sealed);
    });
    let late = time_it(SAMPLES, || {
        let _ = container::open_with_password(b"correct horse battery stapl3", &sealed);
    });
    let right = time_it(SAMPLES, || {
        let _ = container::open_with_password(b"correct horse battery staple", &sealed);
    });

    println!("\ncontainer open, cheap Argon2 ({SAMPLES} samples each)");
    show("wrong at the first byte", &early);
    show("wrong at the last byte", &late);
    show("right password", &right);

    let prefix = ratio(&early, &late);
    let success = ratio(&early, &right);
    println!("    wrong-early / wrong-late   {prefix:.4}");
    println!("    wrong / right              {success:.4}");

    // The one that matters. A comparison that returns on the first differing
    // byte turns the clock into a character-by-character oracle, and shows up
    // here as a large factor rather than a few percent.
    assert!(
        (0.8..1.25).contains(&prefix),
        "prefix length changed the time by {prefix:.3}x — early-exit comparison?"
    );
    // Success versus failure is a weaker property, and is not an oracle in any
    // case: an attacker holding the container learns whether a guess worked
    // from the plaintext, not from the clock.
    assert!(
        (0.5..2.0).contains(&success),
        "success and failure differ by {success:.3}x"
    );
}

#[test]
#[ignore = "timing measurement; run by hand on a quiet machine"]
fn the_app_lock_takes_the_same_time_whether_or_not_the_password_is_right() {
    // A batch of pristine locks, built before the clock starts. Each is used
    // for exactly one attempt, so no measurement includes a reset and none of
    // them trips the rate limiter.
    let batch = |n: usize| -> Vec<lock::AppLock> {
        (0..n)
            .map(|_| lock::AppLock::create(b"the app lock password", params()).unwrap())
            .collect()
    };
    let samples = SAMPLES / 4; // each sample costs a `create` as well
    let mut for_right = batch(samples);
    let mut for_wrong = batch(samples);
    let mut for_length = batch(samples);

    let right = time_each(&mut for_right, |l| {
        let _ = l.verify(b"the app lock password");
    });
    let wrong = time_each(&mut for_wrong, |l| {
        let _ = l.verify(b"the app lock passwerd");
    });
    // A wildly different length, in case anything downstream is length-sensitive.
    let long = time_each(&mut for_length, |l| {
        let _ = l.verify(b"z");
    });

    println!("\napp lock verify, cheap Argon2 ({samples} fresh locks each)");
    show("right password", &right);
    show("wrong password, same length", &wrong);
    show("wrong password, one byte", &long);

    let same_length = ratio(&wrong, &right);
    let diff_length = ratio(&long, &right);
    println!("    wrong / right              {same_length:.4}");
    println!("    one-byte / right           {diff_length:.4}");

    assert!(
        (0.8..1.25).contains(&same_length),
        "right and wrong passwords differ by {same_length:.3}x"
    );
    assert!(
        (0.8..1.25).contains(&diff_length),
        "password length changed the time by {diff_length:.3}x"
    );
}

/// The rate limiter returns **before** touching the KDF, so a locked-out
/// attempt is obviously faster than a real one. That is deliberate — the point
/// of a rate limit is to refuse to spend the CPU — and it leaks only the state
/// the UI displays on screen anyway. Measured so the trade is a number rather
/// than an assumption.
#[test]
#[ignore = "timing measurement; run by hand on a quiet machine"]
fn a_rate_limited_attempt_is_visibly_cheaper_and_that_is_intended() {
    let mut locked = lock::AppLock::create(b"pw", params()).unwrap();
    for _ in 0..6 {
        let _ = locked.verify(b"nope");
    }
    assert!(locked.cooldown().is_some(), "should be rate limited by now");

    let refused = time_it(SAMPLES, || {
        let _ = locked.verify(b"pw");
    });

    let mut fresh: Vec<lock::AppLock> = (0..SAMPLES / 4)
        .map(|_| lock::AppLock::create(b"pw", params()).unwrap())
        .collect();
    let considered = time_each(&mut fresh, |l| {
        let _ = l.verify(b"pw");
    });

    println!("\nrate-limited refusal versus a real attempt");
    show("refused, KDF never consulted", &refused);
    show("a real derivation", &considered);
    println!(
        "    the refusal is at least {:.0}x cheaper, by design",
        considered.min.as_secs_f64() / refused.min.as_secs_f64().max(1e-9)
    );
    assert!(
        refused.median < considered.median,
        "a rate-limited attempt must not cost a full derivation"
    );
}
