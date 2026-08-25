<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Coverage-guided fuzzing

## In plain words

This throws deliberately broken files at the program to see whether it falls
over.

Not the ordinary tests, which check that correct input gives correct output.
These generate nonsense -- truncated recordings, impossible headers, files that
lie about their own length -- and keep going, on the theory that anything a
person can be sent, somebody will eventually send.


Six targets, one for each parser in VeilVoice that reads bytes somebody else
produced:

| Target | What it reads | Why it matters |
|---|---|---|
| `container_header` | the `.veil` header | a file somebody sent you; carries the Argon2 cost parameters |
| `lock_file` | the app-lock file | parsed **before anyone has authenticated**; also carries cost parameters |
| `wav_chunks` | the RIFF chunk walker | termination depends on length fields taken from the file |
| `wav_preflight` | the WAV header check | stands in front of a decoder crash, so its own robustness is load-bearing |
| `guard_manifest` | the integrity manifest | text, sliced by byte offset for display |
| `hybrid_keys` | `.pub` keys and encapsulations | a public key arrives from elsewhere by definition |

## Running it

`cargo fuzz` needs a nightly toolchain and libFuzzer, which is why this lives
outside the workspace and outside `rust-toolchain.toml`. Nothing else in the
repository needs nightly, and installing it is not a prerequisite for anything
but this directory.

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
cargo +nightly fuzz run container_header
```

One target for a fixed time, which is the useful form for a session:

```bash
cargo +nightly fuzz run wav_chunks -- -max_total_time=600
```

All six in turn:

```bash
for t in container_header lock_file wav_chunks wav_preflight guard_manifest hybrid_keys; do
  cargo +nightly fuzz run "$t" -- -max_total_time=300 -max_len=65536 || exit 1
done
```

A crash is written to `fuzz/artifacts/<target>/`, and is reproducible with:

```bash
cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<file>
```

## Status, stated plainly

**These targets have been written and type-checked against the real APIs. They
have not been run to convergence by the maintainer.** libFuzzer needs a
clang-based toolchain, which the `x86_64-pc-windows-msvc` host this was
developed on does not provide, so the campaign is currently something a
contributor on Linux or macOS can run and the maintainer cannot.

That is recorded here rather than glossed, because "we have a fuzzing setup" and
"we have fuzzed this" are different claims and only the first one is true. What
*has* been run to exhaustion is the deterministic campaign described at the
bottom of this file — a million rounds per target, on every commit, on three
platforms. If you have a Linux box and ten minutes, running the loop above is
the single most useful contribution available to this project right now.

## Overflow checks are on, deliberately

`fuzz/Cargo.toml` sets `overflow-checks = true` and `debug-assertions = true` in
an otherwise optimised profile. This is not an oversight and must not be
"tidied up" to match the workspace release profile, which sets
`overflow-checks = false`.

Two of the defects this project has shipped — F-2 (an Argon2 parallelism
overflow) and F-4 (a 32-bit RIFF overflow) — were *arithmetic overflows*. With
overflow checks off they wrap silently, so a fuzzer running under the release
profile would have explored those exact inputs and reported nothing. A campaign
that cannot see the class of bug you have already shipped twice is not a
campaign.

## What this does not cover

- **32-bit targets.** F-4 existed only on ARMv7, and F-11 (the non-terminating
  erase loop) is also 32-bit only. Neither is reachable from a fuzzer on an
  x86-64 host, whatever it does. Building the targets for `i686` or `armv7`
  under emulation would help; reading the code is what actually found both.
- **The decoders themselves.** `symphonia`, `lofty` and `img-parts` parse
  untrusted input and are not fuzzed here — they have their own suites, and
  duplicating them badly would be worse than pointing at them. `wav_preflight`
  covers the one place VeilVoice stands in front of a decoder crash it cannot
  otherwise survive.
- **Anything stateful.** Every target is a pure function of its input, so a
  crash is reproducible from the artefact alone. `Manifest::check` is
  deliberately not driven against the real filesystem for that reason.

## Relationship to the deterministic campaign

`crates/veilvoice-crypto/tests/parser_fuzz.rs` and
`crates/veilvoice-meta/tests/wav_fuzz.rs` are a *different* thing and both are
kept. They are seeded, deterministic, need no nightly, and run on every commit
in CI on every platform — so they are the check that actually gets run. This
directory explores by feedback rather than by construction, and is the deeper
but less frequent pass.

Neither replaces the other, and neither replaces reading the code: F-4 and F-11
were both found by reading, and no campaign on a 64-bit machine could have
reached either.
