<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice — internal audit

**Auditor:** tilas01 (maintainer). **Date:** 2026-08-16. **Version:** 0.1.4.

This is a *maintainer* audit. It catches what the author can see, which is not
the same as what an adversary can. **No external firm or independent researcher
has reviewed this code.** Where that matters, it is said plainly rather than
papered over.

The audit is partial — it covers the mechanical checks, the cryptography, and a
read of the security-critical paths. The remaining scope is listed at the end so
whoever picks it up knows exactly where the line is.

---

## 1. Mechanical checks

| Check | Result |
|---|---|
| `unsafe` code | **None.** All 7 crates carry `#![forbid(unsafe_code)]`, enforced at compile time. |
| `cargo clippy --workspace --all-targets` | **0 warnings**, both with and without the `live` feature. |
| `cargo fmt --all --check` | Clean. |
| `cargo audit` | **0 vulnerabilities.** Two `unmaintained` advisories accepted with written reasoning in `.cargo/audit.toml`. |
| Test suite | 186 tests across 7 crates, plus doctests. |
| Networking crates in the graph | **None.** CI fails the build if `reqwest`/`hyper`/`curl`/`ureq`/`tungstenite`/`isahc`/`surf` appears. |
| `TODO`/`FIXME`/`HACK` markers | None. |
| Secrets in the repository | None. `gpg_secrets/` is gitignored; `*.asc` ignored by default with only the public key allowed back explicitly. |
| Dependency licences | All permissive (MIT / Apache-2.0 / BSD / ISC / BSL / CC0). No copyleft conflict with GPL-3.0-or-later. |

---

## 2. Findings

### 2.1 Fixed during this audit

**F-1 — Duplicate seeding path that panicked (`veilvoice-core`).**
`Modulator::from_os_rng` read the OS CSPRNG and `.expect()`-ed on failure. It
was public API, never called, and duplicated `Deidentifier::new`, which does the
same thing and returns a `Result`. Two paths for one job where the unused one
aborts the process is a footgun in a security crate. **Removed.**

### 2.2 Accepted, with reasoning

**A-1 — Remaining `expect()` sites (6).** Each was reviewed:

| Site | Assessment |
|---|---|
| `stft.rs` ×2 — FFT `expect` | Infallible given correct buffer sizes, which the engine owns and asserts on construction. A failure here is a programming error, not a runtime condition. |
| `hybrid.rs` — `OsRng::fill_bytes` | `rand_core::RngCore::fill_bytes` has no error return. Panicking on CSPRNG failure is the only option and is what every RustCrypto consumer does. Continuing with weak randomness would be far worse. |
| `hybrid.rs` — `NonZeroU32::new(1).unwrap()` | Compile-time constant, cannot fail. |
| `wav.rs` ×2 — `try_into().expect("4 bytes")` | Slices of statically known length 4, guarded by an explicit `pos + 8 <= end` bounds check immediately above. |

**A-2 — `paste` and `ttf-parser` unmaintained advisories.** Neither is a
vulnerability. `paste` is a compile-time proc macro that emits no runtime code.
`ttf-parser` does parse untrusted input, but the only fonts loaded are egui's
own embedded face and, optionally, a JetBrains Mono the user already installed
system-wide — no attacker-supplied font reaches it. Reviewed each release.

**A-3 — FreeBSD builds are not reproducibility-verified.** Built once in a VM
rather than twice in separate directories. Reported as `not-verified` in the
release notes rather than claimed.

**A-4 — `veilvoice-watch` on Linux sees only your own processes.** `/proc/<pid>/fd`
is readable by the owner and root. This is a kernel permission boundary, and
`support()` states it rather than letting an empty list imply an empty machine.

---

## 3. Cryptography

### 3.1 Primitives

| Purpose | Choice | Assessment |
|---|---|---|
| Password → key | Argon2id, RFC 9106 profile (256 MiB, t=3, p=4) | Current best practice. Memory-hard. Cost parameters travel with the file, so old files open after defaults rise. |
| Public-key | X25519 + ML-KEM-768 **hybrid** | Correct construction. Breaking it requires breaking *both*. |
| Payload | XChaCha20-Poly1305 | 192-bit nonce is randomly generated; collision risk negligible, and it removes the counter-management failure mode that sinks RFC 8439 ChaCha20 deployments. |
| KEM combiner | HKDF-SHA256 over both secrets **plus the full transcript** | Correct. Binding both ciphertexts and the recipient key prevents an attacker who substitutes one half from steering the derived key. |
| Header integrity | Authenticated as AEAD associated data | Prevents KDF-cost downgrade. Verified by test. |
| Modulation stream | ChaCha20, OS-seeded, ratcheted every 2 s | Forward-secure: ChaCha20 is not invertible, so a compromised current state cannot recover earlier segments. |

### 3.2 Post-quantum posture

**Sound.** ML-KEM-768 (FIPS 203) is NIST-standardised and targets category 3.
The hybrid is the right call: ML-KEM is young, lattice schemes have had
implementation breaks, and a pure-PQ deployment would be a single point of
failure. The harvest-now-decrypt-later threat is real for recordings, which is
precisely why this is not deferred.

**Honest gap:** the *signature* on releases is RSA-4096, which is **not**
post-quantum. This is deliberate and low-risk — a signature only needs to resist
forgery until the release is superseded, and no PQ signature scheme has the
verifier tooling to make it practical today. It is recorded here so nobody
mistakes "post-quantum encryption" for "post-quantum everything".

### 3.3 Key handling

Page-locked out of swap, zeroized on drop, constant-time comparison, `Debug`
redacted. Each `Secret` owns whole pages exclusively — a bug found and fixed
earlier, where two secrets sharing a page meant dropping one unlocked the
other's memory.

`Secret::is_locked()` reports whether locking actually succeeded rather than
assuming. Locking does not survive hibernation, which is stated in the docs.

---

## 4. Remaining audit scope

Not yet done. Whoever continues should treat this list as the definition of
"the audit is finished":

1. **Line-by-line review of `spectral.rs` and `accent.rs`** against the
   irreversibility claim. The argument is sound and tested, but the code has not
   been read adversarially end to end.
2. **Fuzzing the container parser** (`container::Header::parse`) and the WAV
   chunk walker (`wav::clean_wav_bytes`). Both parse untrusted input. `cargo
   fuzz` targets should be added; the WAV walker already survives a
   lying-chunk-size test but that is one case, not a campaign.
3. **Timing analysis** of the password path. Argon2id is inherently constant-ish
   but the surrounding container code has not been measured.
4. **Review of the website's JavaScript** for DOM-injection paths. The markdown
   renderer escapes first and only emits its own tags, but it has not been
   fuzzed with hostile markdown.
5. **A real independent audit.** Everything above is still the author checking
   the author's work.

---

## 5. Verdict

No vulnerabilities found. One genuine defect fixed (F-1). The cryptography uses
standard, well-reviewed primitives correctly, and the composition — particularly
the hybrid combiner and the authenticated container header — is done properly
rather than approximately.

The project's main security asset is not any single control but the fact that
**every claim it makes is checkable**: no `unsafe`, no network, reproducible
builds, generated artwork, and documentation that states limits rather than
hiding them.
