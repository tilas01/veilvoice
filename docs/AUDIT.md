<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice — internal audit

**Auditor:** tilas01 (maintainer). **Date:** 2026-08-16. **Version:** 0.1.5,
plus the at-rest-by-default, app-lock and audit work that landed after it.

This is a *maintainer* audit. It catches what the author can see, which is not
the same as what an adversary can. **No external firm or independent researcher
has reviewed this code.** Where that matters, it is said plainly rather than
papered over.

**The scope listed as outstanding in the previous revision is now done**, except
the one item that cannot be done from the inside: an independent review. Working
through it found **seven real defects**, four of them reachable from a file
somebody sends you. They are written up individually in §2 rather than
summarised, because a finding with the details filed off is not a finding.

The uncomfortable conclusion is recorded here rather than buried: the previous
revision said "no vulnerabilities found" about code that would abort on a
hostile container and silently destroy every recording it processed after
reading one NaN. It was not lying — nobody had looked with the right tools. That
is what "a maintainer audit is worth what a maintainer audit is worth" means in
practice, and it is the argument for the item that is still open.

---

## 1. Mechanical checks

| Check | Result |
|---|---|
| `unsafe` code | **None.** All 8 crates carry `#![forbid(unsafe_code)]`, enforced at compile time. |
| `cargo clippy --workspace --all-targets` | **0 warnings**, both with and without the `live` feature. |
| `cargo fmt --all --check` | Clean. |
| `cargo audit` | **0 vulnerabilities.** Two `unmaintained` advisories accepted with written reasoning in `.cargo/audit.toml`. |
| Test suite | 269 tests across 8 crates, plus doctests and 5 site-test suites in `tools/site-tests`. |
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

**F-2 — Argon2 parallelism overflow, reachable from any hostile file
(`veilvoice-crypto`). Denial of service.**
`argon2` 0.5.3 validates in the wrong order: `Params::new` evaluates
`m_cost < p_cost * 8` *before* it checks `p_cost > MAX_P_COST`. A `p_cost` above
`u32::MAX / 8` therefore overflows the multiplication. `p_cost` is read verbatim
from a `.veil` header — and from the app-lock file, which is parsed **before
anyone has authenticated**. With overflow checks on (every debug build, and any
project consuming these crates as libraries, which the README explicitly invites)
that is a panic on attacker-controlled input.

VeilVoice's own release profile sets `overflow-checks = false`, where the
multiplication wraps and the ceiling test then rejects it — so shipped binaries
were not affected. "Our release profile happens to make the panic unreachable"
is not a property to rely on and is not true for anyone building against these
crates. **Fixed** by validating in `KdfParams::checked()`, the single funnel
every derivation passes through, in arithmetic that cannot overflow. Found by
`tests/parser_fuzz.rs` within seconds of the campaign first running.

**F-3 — Unbounded Argon2 memory cost (`veilvoice-crypto`). Denial of service,
release builds included.**
`m_cost` is also read verbatim, and Argon2 allocates that many KiB before doing
anything else. A header claiming `u32::MAX` asks for **4 TiB**; the allocation
fails, and a failed allocation in Rust aborts the process. Merely *attempting to
open* a hostile container killed the program, in debug and release alike. For the
app lock it is worse — anything able to write that file could stop VeilVoice
from starting at all.

**Fixed** with a documented ceiling of 4 GiB (`KdfParams::MAX_M_COST`), chosen to
sit above RFC 9106's largest recommended profile (2 GiB) and this crate's own
default (256 MiB) while refusing absurd values. Found by the same campaign,
immediately after F-2 was fixed — which is the argument for running a fuzzer to
exhaustion rather than until the first green run.

**Residual, stated rather than fixed:** a container may still declare a
legitimate-but-expensive cost, so an attacker can make opening their file *slow*.
That is inherent to shipping the cost with the file, which is what lets old files
open after defaults rise. Slow is not crashing, the user chose to open that file,
and they can stop waiting.

**F-4 — 32-bit overflow in the RIFF chunk walker (`veilvoice-meta`).**
`clean_wav_bytes` computed `declared + 8` where `declared` is a `u32` widened to
`usize`. On a 64-bit host that cannot overflow — which is why no amount of
fuzzing on this machine would ever have found it. **VeilVoice ships an ARMv7
build**, where `u32::MAX + 8` overflows `usize` and panics under overflow checks.
**Fixed** with `saturating_add`, which is also the correct semantics since the
value is clamped to the real length on the next line. Found by reading, and
recorded here as the counter-example to "the fuzzer is green, therefore the
parser is fine".

**F-5 — One NaN sample permanently destroyed the engine (`veilvoice-core`).
Silent, total, and reachable from an input file.**
The accent neutraliser's long-term spectrum is an exponential moving average, so
anything folded into it never washes out. A single non-finite input sample
reached it, and from then on **every output sample was NaN for the rest of the
session** — with nothing reported to the user. Measured: after one NaN, 0 of
48,000 subsequent samples were finite.

A 32-bit-float WAV can legally contain NaN, and `symphonia` decodes it
faithfully rather than sanitising. The realistic route is the ordinary one:
somebody sends you a recording and you veil it before passing it on. The failure
is silent, so the first sign is a recording that turned out to be silence.

**Fixed** at the single gate every sample passes through (`StftEngine::process`):
non-finite samples become zero, and magnitudes are bounded at ±1e6 — six orders
of magnitude above real audio, low enough that squaring and summing cannot reach
the float ceiling and produce a NaN by the other door.

Not a confidentiality failure: the output was garbage, not the original voice, so
it failed in the safe direction. It was still a total loss of function with no
error, and `tests/hostile_audio.rs` now covers it along with infinities,
full-scale DC, square waves, impulse trains and digital silence.

**F-6 — The site's Markdown renderer emitted unfiltered image URLs
(`website/js/markdown.js`).**
Links went through a scheme allowlist; images did not, so
`![x](javascript:...)` produced `<img src="javascript:...">`. `js/repo.js`
assigns the rendered README straight to `innerHTML`, so this was on the path from
a fetched file into the live page. No current browser executes a `javascript:`
image source, which is why it survived unnoticed — but "no browser we tested
still honours this" is not a security argument. **Fixed**: one `safeUrl` helper,
applied in both places.

**F-7 — The renderer silently deleted every string literal on the page
(`website/js/markdown.js`).**
Finished markup is parked while later passes run, and the placeholder was a
NUL-delimited decimal index. The number highlighter (`\b\d+\b`) matched the index
*inside the placeholder* and wrapped it in a span, after which the un-parking
pass no longer recognised it and **the parked content was discarded**. Every
string literal in every code block rendered as a stray highlighted digit:
`let s = "hello";` was published as `let s = 0 ;`. Adjacent placeholders shared a
delimiter as well, so alternate items were dropped.

Not a security bug. Arguably worse in context: the site's whole argument is "go
and read the source", and it was displaying source that was not the source.
**Fixed** with single-character private-use placeholders, which no pass matches
and which `escapeHtml` strips from the input so they cannot be forged.

**F-8 — The URL allowlist rejected ordinary relative links (same file).**
The scheme test was `^(?:https?:|[./#])`, so any target not beginning with `.`,
`/` or `#` was refused — which is most Markdown links. Every
`[whitepaper](docs/WHITEPAPER.md)` in the README rendered as plain text on the
site, and `js/repo.js`'s rewriting of repo-relative links could never fire.
Safe, wrong, and quietly wrong. **Fixed**: a scheme is required to be http(s);
no scheme means a relative path and is fine; protocol-relative `//host` and
leading backslashes are refused explicitly, since both look relative and behave
external.

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

### 3.2 The app lock

Reviewed as it was written, and the review is short because the design refuses
to be clever:

| Property | Assessment |
|---|---|
| Verifier, not a key | Correct choice. It encrypts nothing because there is nothing local it could usefully encrypt, and a key that protected nothing would only invite the belief that it did. |
| Domain separation | `Argon2id("veilvoice/app-lock/v1\0" ‖ password, salt)`. Asserted by test to differ from a container key over the same passphrase and salt. |
| Comparison | Constant time, through `Secret`'s `subtle::ConstantTimeEq`. |
| Rate limit | Three free attempts, then doubling from 5 s to a 15-minute cap. Persisted after every attempt, so a process restart does not reset it. The shift is guarded against overflow and asserted at `u32::MAX`. |
| Clock handling | A clock that moves backwards yields no credit against the wait rather than a negative elapsed time. |
| Lock file | Unauthenticated **on purpose**. Any key that could authenticate it would have to sit beside it in the same file; a MAC would look like tamper-proofing without being any. Parse errors are errors, never "no lock". |
| Failure modes | A wrong password on `remove` reopens the store so the recorded failure is not lost with it. |

**The honest limit, recorded here as clearly as in the UI:** this defends
against casual access, not against an attacker with the disk. The file can be
deleted, the counter edited, the clock moved, and the hash attacked offline.
`lock::SCOPE` states all four, is shown on the unlock screen itself, and a test
fails the build if it is ever softened into a boast.

### 3.3 Encryption at rest, by default

The `anonymise` path now encodes the WAV in memory and seals it there, so a
recording that is going to be encrypted never lands on disk in the clear. That
closes a real hole rather than a theoretical one: on flash storage, a plaintext
file that is written and then deleted is not recoverable *by the user* and may
well still be recoverable by someone else.

Both front-ends refuse to start a job with encryption on and no passphrase or
recipient key set, rather than quietly falling back to plaintext. The GUI's
`Plan::Missing` exists solely to make that fallback impossible to write by
accident, and is tested.

Opting out is preserved and gated behind a warning that has to be answered.
Tests assert both that the default is on and that the warning still contains the
uncomfortable sentences.

### 3.4 Post-quantum posture

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

### 3.5 Key handling

Page-locked out of swap, zeroized on drop, constant-time comparison, `Debug`
redacted. Each `Secret` owns whole pages exclusively — a bug found and fixed
earlier, where two secrets sharing a page meant dropping one unlocked the
other's memory.

`Secret::is_locked()` reports whether locking actually succeeded rather than
assuming. Locking does not survive hibernation, which is stated in the docs.

**A-5 — Typed passphrases exist as ordinary bytes while being typed.**
*Narrowed since first recorded; the residue is accepted.*

An egui text field owns a `String` and `rpassword` returns one, so a passphrase
is ordinary heap memory for as long as something is receiving keystrokes. That
window cannot be removed without a custom text widget, which would be far more
code and far less reviewed than the gap it closes.

What **was** fixed is the part that had no excuse: the GUI used to keep the
confirmed session passphrase as a plain `String` for the *entire session*, and
the CLI passed `Vec<u8>` and `String` copies around after the prompt. Both now
move the passphrase into a page-locked, zeroizing `Secret` the moment it is
confirmed, and wipe the buffer it came from. The window went from "until the app
closes" to "while the user is typing".

The remainder is accepted and stated rather than papered over: for those
moments the bytes are swappable, and none of this helps against an attacker who
can read the process's memory — who has already won, as `WHITEPAPER.md` §7
says.

---

## 4. The work that was outstanding, and what it found

The previous revision listed five items. Four are now done; the fifth cannot be
done from inside the project.

### 4.1 Adversarial read of `spectral.rs` and `accent.rs` — **done**

Read end to end against the irreversibility claim.

**The central claim holds.** `transform` reads `c.norm()` and nothing else from
the incoming spectrum, and every bin of `spec` is overwritten before it returns —
zeroed then rewritten on the comb path, assigned outright on the channel-vocoder
path. There is no branch on which measured phase survives, and none on which a
bin retains a previous frame's value.

Checked in detail, and correct:

- **`box_smooth`'s edge arithmetic.** The running sum seeds with `radius`
  copies of `src[0]` and slides with clamped indices; expanding the window by
  hand at `i = 0` and `i = 1` gives exactly the sums the code produces.
- **The comb's energy normalisation.** `amp` is set so the sum of squares over
  the comb lines equals the original frame's, so replacing the excitation is
  level-neutral rather than approximately so.
- **Every accent correction is bounded and long-term.** Prosody is clamped to
  `[0.5, 2.0]`, VTLN to `[0.72, 1.40]`, and the tilt curve to ±12 dB; the time
  constants are 3 s (VTLN) and 2 s (LTAS), which is the mechanism behind the
  "never derived from the current frame" claim that keeps vowels intact.
- **`log_centroid` refuses to produce a value it cannot stand behind**,
  returning `None` on an empty band or a non-finite result rather than a number.

One defect found: **F-5**, above. Two observations kept rather than fixed:

- `resample_linear` silently substitutes a ratio of 1.0 for a non-finite one,
  which would pass the envelope through unwarped — a *weaker* transform arriving
  quietly. With F-5 fixed and the accent ratios clamped, no input can reach it,
  so it is now defence in depth rather than a live path. Left as it is, and
  written down here so it is a known belt rather than a forgotten one.
- With accent neutralisation switched off, voiced frames take the
  channel-vocoder path and pitch is *randomised* rather than *normalised* —
  strictly weaker, as `WHITEPAPER.md` §3.2 already says. Consistent, not a
  defect.

### 4.2 Parser campaigns — **done**, and they found F-2, F-3 and F-4

`crates/veilvoice-crypto/tests/parser_fuzz.rs` and
`crates/veilvoice-meta/tests/wav_fuzz.rs`. Deterministic seeded PRNG with
structure-aware mutation — bit flips, truncation, splices, and 32-bit fields
replaced with `u32::MAX`, `i32::MAX`, 0 and 1 — plus every length around each
boundary walked exhaustively.

Properties asserted, not just "it did not crash": a parsed header must
re-serialise to exactly the bytes it came from, reported offsets must lie inside
the buffer, and a cleaned WAV must itself be a valid WAV whose size field matches
what was written.

Run at **1,000,000 rounds per target in debug** (overflow checks *on* — the
release profile disables them, and an overflow was one of the bugs) and
2,000,000 in release. Clean after the fixes. CI runs 1,000,000 per target.

**This is not `cargo fuzz` and is not claimed to be.** It is not
coverage-guided, so it explores by construction rather than by feedback. It was
chosen because `cargo fuzz` needs nightly and libFuzzer, and a check that only
one person can run is a check that stops being run. A coverage-guided campaign
remains worth doing — see §5.

### 4.3 Timing analysis — **done**, no leak found

`crates/veilvoice-crypto/tests/timing.rs`, `#[ignore]`d by default because a
timing test on a shared runner measures the neighbours. Measured on an idle
Windows desktop, release build, 2,000 samples per case, reporting the
**minimum** — timing noise is one-sided, so the fastest sample is the closest
estimate of the work actually done.

| Comparison | Ratio |
|---|---|
| Container open: wrong at the first byte vs wrong at the last | **0.996** |
| Container open: wrong vs right password | **0.996** |
| App lock: wrong vs right password | **1.004** |
| App lock: one-byte vs full-length password | **1.004** |

Within half a percent. **No prefix correlation**, which is the property that
matters: an early-exit comparison turns the clock into a character-by-character
oracle and would show as a large factor, not a fraction of a percent.

The rate-limited path is a deliberate exception. It returns before touching the
KDF and is therefore **at least 23,600× cheaper** than a real attempt. That is
the point of a rate limit — refusing to spend the CPU — and it leaks only the
state the unlock screen displays on its face anyway.

*(A first pass reported a 1.49 ratio for wrong-vs-right. That was the harness,
not the code: it used medians on a noisy machine and, for the app lock, timed
two derivations against one. Recorded because a timing result that is not
reproducible is not a result.)*

### 4.4 Website JavaScript review — **done**, and it found F-6, F-7 and F-8

`js/repo.js` fetches README.md over the network and assigns the rendered output
to `innerHTML`. Everything on that path rests on one claim in `js/markdown.js`:
escape first, emit only your own tags.

`tools/site-tests/` now tests it: 39 hand-written hostile documents, each aimed
at a specific escape route, plus a randomised campaign run to **200,000
generated documents**. The check is an **allowlist** — output may contain only
tags and attributes the renderer is supposed to produce — because a blocklist of
"no `<script>`" passes anything that finds another door.

The check parses attributes the way a browser does. An early version did not,
and reported six findings of which five were false: `&lt;script&gt;` in the
output is the renderer working, and `src="a&quot;onerror=x"` is one attribute
whose value contains a quote, because entity references are decoded *after* the
value is delimited. A naive scan calls both of those attacks and buries the real
one in the noise.

After the fixes: clean at 200,000 rounds. The suite also covers rendering
correctness (F-7 was a *correctness* bug with no security dimension, and would
never have been caught by a security-only test), page structure, and the
scroll-reveal effect's fallbacks.

Also reviewed by reading:

- `repo.js` assigns `link.href = asset.browser_download_url` from the GitHub API
  without a scheme check. The value comes from GitHub's own release API for this
  repository and is always a `github.com` URL, so it is not currently reachable;
  it is the kind of assignment worth a guard anyway, and is listed in §5.
- `verify.js`, `theme.js`, `legal.js` and `reveal.js` write through
  `textContent`, `classList` and typed DOM properties throughout. No `innerHTML`
  outside the two places noted, no `eval`, no `new Function`, no
  `document.write`.

### 4.5 An independent audit — **still outstanding, and still the point**

Everything above is the author checking the author's work with better tools than
last time. Better tools found seven real defects in code that a previous pass of
the same author had called clean. That is evidence for the tools, and it is also
evidence for the limit: the next class of bug is the one the author does not know
to look for.

---

## 5. Still open

1. **A real independent audit.** See §4.5.
2. **A coverage-guided fuzzing campaign** (`cargo fuzz`, libFuzzer) against the
   same three parsers, to explore by feedback rather than by construction.
3. **A scheme check on `repo.js`'s asset links**, so the GitHub API is not
   trusted by omission.
4. **A `.veil` cost policy for unattended use.** Opening a hostile container can
   be made slow by design (see F-3's residual). A caller processing files
   without a human present may want a cost ceiling lower than 4 GiB; there is
   currently no way to pass one.
5. **32-bit targets are not exercised in CI.** F-4 existed only on ARMv7, and
   nothing in the test matrix would have caught it. The matrix is Windows,
   macOS and Linux on x86-64.

---

## 6. Verdict

**Eight defects found and fixed across two audit rounds (F-1 to F-8).** Four of
F-2 to F-8 were reachable from a file somebody sends you; two of those aborted
the process, and one silently destroyed every recording processed afterwards.
None was a confidentiality failure — no finding let an attacker recover a
voiceprint, read a sealed recording, or bypass a password — and the two crashes
failed closed rather than open. That distinction is worth drawing, and it is not
a reason to be pleased: a privacy tool that aborts when handed a crafted file, or
that returns silence and says nothing, has failed the person relying on it.

The cryptography itself stands up. The primitives are standard and well
reviewed, and the composition — the hybrid combiner, the authenticated container
header, the domain-separated app-lock verifier — is done properly rather than
approximately. Every defect found was at a **boundary**: parameters read from a
file and passed to a library without a bound, samples read from a decoder and
folded into persistent state without a check, text read from the network and
rendered without a complete allowlist. That is where to look next, and it is
where an outside reviewer should start.

The app lock adds a control whose value is real but bounded, and the bound is
stated everywhere it appears rather than only here. A lock a user over-trusts
makes them less safe, not more; that is the failure mode this design was written
to avoid, and it is the one to keep watching for in any future change to it.

The project's main security asset is not any single control but the fact that
**every claim it makes is checkable**: no `unsafe`, no network, reproducible
builds, generated artwork, and documentation that states limits rather than
hiding them. This revision is part of that. The previous one said "no
vulnerabilities found" in good faith about code containing at least four; the
honest response is to say so in the same document, rather than to quietly
improve the score.
