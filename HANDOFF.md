<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice — session handover

**Read this first.** It is written to be lossless: a new session should be able
to continue from here without the previous conversation.

**State as of 2026-08-18:** **v0.1.9 on `main`.** Eleven platforms (OpenBSD and
NetBSD join FreeBSD), GPG-signed, reproducible on eight -- the three BSDs are
built once in VMs and are honestly marked `not-verified`. Live site.
**336 tests** across 9 crates plus **ten** website suites, clippy clean, no
`unsafe`.

v0.1.9 is about **getting a genuine copy, and finding your way around one**:
a search index over the whole repository and website with a real no-JavaScript
answer, a portable verifier that checks a release without GnuPG installed,
install scripts that refuse rather than continue, and packaging definitions for
six formats. A fourth audit round found ten defects (F-37 to F-46); three had
shipped, seven were caught in code written during the round and are written up
anyway.

v0.1.8 is a **security release**. A third audit round -- run against the
*classes* of defect rather than a list of things that seemed worth checking --
found and fixed **twenty-eight** (F-9 to F-36), thirteen in the Rust and fifteen
in the website. None was a confidentiality failure; several were failures of the
thing being relied on, which is worse in a different way. `CHANGELOG.md` has the
summary and `docs/AUDIT.md` has one write-up per finding.

**The lesson of this round, in one line:** F-37 had the website rendering the
text inside its own banner illegibly -- on every viewport, for as long as the
banner existed, with every test passing. It was found by *looking at the page*.
Section 8 has said to do this since v0.1.7 and it has now paid twice.

Two things changed in how this project describes itself, and both matter:

1. **`docs/AUDIT.md` no longer names "an independent audit" as the outstanding
   item.** That framing let a great deal of unexamined code sit behind a single
   caveat. The standard is now stated positively and is about the code: every
   vulnerability class, walked across the whole tree, with a section saying what
   was found -- including the classes that found nothing. The absence of an
   outside reviewer is still recorded, as a fact rather than as a task.
2. **`fuzz/` exists but has not been run to convergence.** Six libFuzzer
   targets, built and type-checked; libFuzzer needs a clang toolchain the
   Windows host does not have. "We have a fuzzing setup" and "we have fuzzed
   this" are different claims and the docs only make the first.

---

## 1. Read these, in this order

| # | File | Why |
|---|---|---|
| 0 | `docs/USER_GUIDE.md` | What it is like to *use*. Skim it — knowing the user-facing behaviour first makes the rest read faster. |
| 1 | `README.md` | What the project is and claims. |
| 2 | `docs/WHITEPAPER.md` | The de-identification argument, threat model, and the limits that must never be overclaimed. |
| 3 | `docs/AUDIT.md` | Audit status, findings, and **exactly what audit work remains**. |
| 4 | This file, sections 4–7 | Locked decisions, current state, next work. |
| 5 | `crates/veilvoice-core/src/spectral.rs` + `accent.rs` | The security-critical heart. Long doc comments explain *why*, not just what. |

Then: `cargo test --workspace` and `cargo run -p veilvoice-core --example spectrum_report`.

---

## 2. Build and verify

Building inside a cloud-synced folder makes the sync client fight the compiler,
so redirect the target directory first:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\veilvoice\target"
cargo test --workspace                    # 269 tests
node tools/site-tests/run.js              # website: characters, structure, renderer, reveal
cargo clippy --workspace --all-targets    # must be zero warnings
cargo clippy -p veilvoice-cli --no-default-features   # the no-live build
cargo audit                               # policy in .cargo/audit.toml
cargo fmt --all
python assets/generate.py --check         # artwork must match its generator
```

### The website, locally

The site is plain static files, so serving the folder is the whole of it:

```powershell
python -m http.server 8787 --bind 127.0.0.1 --directory website
```

`.claude/launch.json` declares the same thing as a named `website` config, so an
editor or agent that reads it starts the identical server on port 8787. There is
no build step, no bundler and no `package.json` — what is in `website/` is
exactly what GitHub Pages serves, which is what makes "read the file yourself"
a real invitation rather than a slogan.

**Render it before believing it.** Three of the walkthrough's paragraphs were
invisible on the live site — including the box stating the app lock is not
tamper-proof — and every unit test passed the whole time, because the stub
modelled the observer firing and the bug was the observer *not* firing. The
site tests are much better now, and they are still not a substitute for looking
at the page.

---

## 3. Where everything lives

```
crates/
  veilvoice-core     DSP engine. spectral.rs + accent.rs are the heart.
  veilvoice-crypto   Argon2id, X25519+ML-KEM-768, XChaCha20-Poly1305,
                     amnesia.rs (page-locked secrets), shred.rs (self-destruct),
                     lock.rs (app-lock verifier + persisted rate limit)
  veilvoice-audio    cpal devices + live path, symphonia decode, hound WAV.
                     `live` is a DEFAULT-ON FEATURE — off where cpal has no backend.
  veilvoice-meta     lofty tags, wav.rs (chunk-level RIFF cleaner), img-parts EXIF
  veilvoice-watch    mic/camera monitor. linux.rs + windows.rs, zero dependencies
  veilvoice-guard    integrity manifest + tamper detection. blame.rs attributes
                     a change where system auditing allows, and says so when not
  veilvoice-cli      the `veilvoice` binary. atrest.rs (seal-by-default policy
                     + passphrase prompts), lock.rs (`veilvoice lock` subcommand)
  veilvoice-gui      egui desktop app, Tokyo Night. security.rs (unlock screen,
                     lock tab, at-rest controls, the write Plan)
website/             the published site (GitHub Pages, deployed via Actions)
tools/site-tests/    node, no dependencies. Hostile-markdown and structure
                     tests for the site; `node tools/site-tests/run.js`
assets/generate.py   generates every icon and the banner; --check verifies
gpg_secrets/         GITIGNORED. signing key, passphrase, generator script
.github/workflows/   ci.yml, release.yml, pages.yml
```

**Platform-specific code convention:** one file per platform, named for it, with
a `cfg` gate in the parent (`veilvoice-watch/src/{linux,windows}.rs`). Follow
this for any new platform work — it is what makes the codebase contributable.

---

## 4. Locked decisions — do not relitigate

1. **Goal = irreversible speaker de-identification with intelligibility
   preserved.** Not white-noise fill. The words are kept *on purpose*; the
   voiceprint is what is destroyed.
2. **Licence is GPL-3.0-or-later.** Confirmed after explicitly considering
   CC BY-NC-SA to match `unix-guides-dynamic`. Rejected: CC is not a software
   licence, NonCommercial is not free software, and it would contradict every
   "libre" claim. All dependencies are permissive, so this is clean.
3. **Identity is the pseudonym `tilas01`.** **No e-mail anywhere** — not in
   commits (which use `tilas01@users.noreply.github.com`), not in the GPG key
   UID, and never the maintainer's real address. Grep for `@gmail`, the real
   local-part and the OS username before every commit; an absolute path in a
   doc leaked the username once already and had to be scrubbed pre-publication.
   Do not write the real address down anywhere in this repository, including in
   a warning not to use it.
4. **GUI = egui/eframe**, Tokyo Night, monospace.
5. **Offline by construction.** CI fails if an HTTP client enters the graph.
6. **Accent neutralisation on by default.** Suprasegmental cues are removed;
   segmental ones cannot be, and the docs say so.
7. **Binaries are never signed in place.** Detached signature over `SHA256SUMS`
   only, so reproducibility and signing do not conflict.
8. **"Audited by tilas01"** is the agreed wording. Docs must also state that no
   external firm has reviewed it. Do not upgrade this to "independently audited".
9. **Audacity is recommended, not embedded.** It is GPL-2.0-or-later, which is
   *incompatible* with this project's GPL-3.0-or-later for copying code **in**.
   Recommend and integrate; never lift its source.
10. **Recordings are encrypted at rest by default**, and `anonymise` therefore
    writes `<out>.veil`, not a bare WAV. Opting out stays possible and is gated
    behind a warning that must be answered. Do not quietly restore the old
    default because it surprised someone — the surprise is the point, and the
    wiki explains where the WAV went.
11. **The app lock is a verifier, never a key, and is never called
    tamper-proof.** It is Argon2id + a persisted rate limit, and every surface
    that shows it also shows `lock::SCOPE`. Tests fail the build if that text is
    softened. A lock a user over-trusts leaves them worse off than no lock.
12. **The two passwords stay separate.** App lock and recording passphrase are
    different secrets, domain-separated in the KDF. Never derive one from the
    other, and never make unlocking the app unseal recordings.

---

## 5. Signing and release

- Repository secrets `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE` are **already set**.
- Key: RSA-4096, UID exactly `tilas01`, no e-mail, expires 2029-08-14.
- **Fingerprint `8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A`** — published in the
  README, the site, the wiki and the release notes. CI checks every page carries it.
- Regenerate with `python gpg_secrets/generate-signing-key.py` (gitignored).
- `gpg_secrets/` currently sits inside a OneDrive-synced folder. **It is being
  synced to Microsoft.** Moving it out is still outstanding.

Release: bump `version` in the root `Cargo.toml`, commit, tag `vX.Y.Z`, push the
tag. The workflow builds nine targets, double-builds each and compares, signs
`SHA256SUMS`, and publishes with full GPG verification instructions.

---

## 6. What is done

- **Engine** — phase discard, many-to-one normalisation of pitch/vocal-tract/tilt,
  CSPRNG modulation with a **2-second forward-secure ratchet** (configurable,
  inaudible), harmonic-comb voiced resynthesis, decimated-YIN pitch tracker.
- **Crypto** — Argon2id, X25519+ML-KEM-768 hybrid, XChaCha20-Poly1305, `.veil`
  container with authenticated header, page-locked zeroizing secrets, secure erase.
- **At rest** — every recording sealed by default, encoded in memory so the
  plaintext never touches the disk; both front-ends refuse to run with
  encryption on and nothing to encrypt with rather than falling back.
- **App lock** — Argon2id verifier, constant-time compare, domain separated,
  three free attempts then a doubling wait to a 15-minute cap, persisted across
  restarts. GUI unlock screen and lock tab, `veilvoice lock` subcommand.
- **Audio** — device enumeration with virtual-cable detection, live path over a
  lock-free ring, symphonia decode, WAV write.
- **Metadata** — tags, EXIF/GPS, chunk-level RIFF cleaner.
- **Monitor** — which apps hold the mic/camera, Windows + Linux, alerts, GUI overlay.
- **CLI + GUI**, **website + wiki + no-JS edition**, **legal gate**, CI, signed
  reproducible releases on nine platforms.

## 7. Next work, in order

**Item 1 (post-quantum recording encryption + app lock) is done** and sits
unreleased in the tree. What landed, and what deliberately did not:

- `veilvoice-crypto/src/lock.rs` — the verifier, the persisted rate limit, the
  84-byte lock file, `default_path()` resolved from environment variables (no
  new dependency), and `SCOPE`, the single-sourced honesty note.
- `container::veil_path`, `io::wav_bytes` — the two small primitives that let a
  recording be sealed without ever being written in the clear.
- CLI: `anonymise --encrypt` (default true) / `--encrypt-to` / `--yes`, and
  `veilvoice lock set|status|change|remove [--path]`.
- GUI: a `lock` tab, a full-window unlock screen, a header `lock` button, at-rest
  controls in the file tab, and a modal that must be answered before encryption
  can be switched off.
- **Not done on purpose:** the lock file is not authenticated (any key would sit
  beside it — see the note in `lock.rs`), and typed passphrases are not
  page-locked while they are in a text field (audit A-5).

Remaining, in order:

1. **The privileged half of tamper detection.** *(the unprivileged half shipped
   in v0.1.7 as `veilvoice-guard`)*

   What exists now: a SHA-256 manifest, a check that reports modified, removed
   and added files, an optional passphrase-sealed record, and best-effort
   attribution that reports honestly that it cannot see. No privileges needed.

   What a privileged helper would add, and the reasons not to rush it:

   - **A helper running as the user protects nothing from that user**, and
     anything running as them can kill it. To mean anything it must run as
     root/SYSTEM, which brings an installer, a privileged service and a much
     larger attack surface into a project that currently needs no privileges.
   - Even as root it can **detect and alert, not prevent** — another root
     process can do as it likes. "Tamper-proof" stays the overclaim this
     project refuses to make (§4.11); `guard::SCOPE` is the wording, and a test
     fails the build if it softens.
   - **Attribution is the valuable half, and is achievable**:
     - Linux: `fanotify` with `FAN_OPEN_PERM` can block *and* attribute; an
       `auditd` watch (`-w <path> -p wa -k veilvoice`) attributes without a
       daemon of our own. `blame.rs` already reads `ausearch` when it is there.
     - Windows: a SACL plus Security event 4663, which carries the process
       name. `blame.rs` already queries it with `wevtutil`; it needs elevation
       and the audit policy enabled, and reports exactly that when it fails.
   - So the remaining work is **configuration and privilege**, not detection
     logic: an opt-in installer that sets the audit rule, and a service that
     watches and alerts. Both are outward-facing and should not be silent.

2. **Distribution: an installer, a portable verifier, and wider platforms.**
   **Mostly built in v0.1.9.** What exists, and what does not:

   - **Built and tested:** `install/install.{sh,ps1,bat}`, and
     `crates/veilvoice-verify` -- a portable verifier that checks a release
     with no GnuPG installed. Both verified end to end on Windows against the
     real published v0.1.8. `docs/INSTALL.md`.
   - **Written, never built:** WiX, `.deb`, `.rpm`, Flatpak, Homebrew, Gentoo
     ebuild, in `packaging/`. They parse and nothing more. `docs/PACKAGING.md`
     has a per-format status table.
   - **Built:** OpenBSD and NetBSD, beside FreeBSD. macOS Intel and Apple
     Silicon were already separate; Windows 10 and 11 share one executable and
     will keep doing so unless a measurement says otherwise.
   - **Not done:** nobody other than the author has run any of it, and
     `install.sh` has never run on a real Linux or macOS machine. That is the
     single most useful thing a contributor could do, and both documents say so.

   *The original specification is kept below because the rules in it still
   govern anything added here.*

   The goal is that somebody who has never used a terminal can get a genuine,
   verified VeilVoice, and that somebody who has can check every step by hand.

   - **An install script per platform** -- `install.bat` / `install.ps1` for
     Windows, `install.sh` for Linux and macOS. Each one:
     1. downloads the binary, `SHA256SUMS`, `SHA256SUMS.asc` and the public key;
     2. checks the key's fingerprint against
        `8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A`, **hardcoded in the script**,
        and refuses on any mismatch;
     3. verifies the detached signature over `SHA256SUMS`;
     4. verifies the binary's SHA-256 against that list;
     5. offers -- as a **tick box, never silently, never by default** -- to
        install VB-CABLE (Windows, proprietary donationware), Audacity, and GPG
        itself where it is missing.

     Every download must be refused rather than continued on any failed check,
     and the script must say which check failed.

   - **A portable mini-verifier**, one small cross-platform binary that embeds
     the fingerprint and the public key and does the same verification without
     needing GPG installed. It **cannot** embed the expected hash of the file it
     is checking -- a file cannot contain its own digest -- so it takes the hash
     two ways: typed in by hand, or fetched from the published `SHA256SUMS`.
     A match against a published hash proves the download is intact; a match
     against a hash somebody else independently produced from their own build is
     what proves the build is reproducible, and the tool should explain that
     difference rather than blur it.

   - **A WiX installer for Windows** offering the same optional components, and
     the platform-appropriate equivalents elsewhere: a `.pkg` or a Homebrew cask
     on macOS, a `.deb`/`.rpm` and a Flatpak on Linux.

   - **Wider platform coverage** -- macOS Intel and Apple Silicon as separate
     builds, a single Windows executable (Windows 10 and 11 do not need separate
     binaries; only split them if a measurement says otherwise), OpenBSD and
     NetBSD alongside the existing FreeBSD build, and a **Gentoo ebuild** that
     builds from this repository the way every other Gentoo package does.

   - **A documented reproducible build environment** so a third party can
     produce a byte-identical binary and compare hashes -- which is the only
     thing that makes the verifier's stronger claim checkable.

   All of it belongs in `docs/`, and none of it should be described as finished
   until somebody other than the author has run it on a machine that did not
   build it.

3. **Presentation.** *Partly built in v0.1.9.*

   - ~~An animated soundbar banner for the README~~ **done**: a 24-frame APNG
     from `assets/generate.py`, served through `<picture>` so
     `prefers-reduced-motion` is honoured, with frame 0 byte-identical to the
     static banner so a viewer without APNG support shows that instead of
     nothing. The dropped bars are gone (see the commit for why they were the
     wrong idea, not merely ugly).
   - A pass over the website and wiki for typography, spacing and motion, so the
     animation already there reads as designed rather than as decoration.
   - A proofreading pass across the whole repository -- prose, comments and
     user-facing strings -- for punctuation, grammar and capitalisation.
     `tools/site-tests/characters.test.js` already fails the build on stray
     control, private-use, zero-width, bidi and mojibake characters across every
     tracked text file; that is the mechanical half and it is done. The
     editorial half is not, and cannot be mechanised.

4. **Audacity + VB-CABLE opt-in installer.** Subsumed by item 2 above; kept
   here because the tick-box-never-silent rule applies wherever it lands.
5. **The audit's own remaining list** -- `docs/AUDIT.md` section 5. Short
   version: an outside reviewer (absent, not a task); running the `fuzz/`
   targets to convergence on a machine with clang; **32-bit targets in CI**,
   which is now the source of *two* shipped defects rather than one (F-4 and
   F-11, both 32-bit-only, both found by reading); and a decoder crash in a
   format VeilVoice does not itself parse, which nothing in-process can catch
   under `panic = "abort"`.

6. **Move `gpg_secrets/` out of OneDrive.**
7. **Text-to-speech mode** — type text, an AI voice speaks it. The strongest
   anonymity, since the original voice is never captured. Weights and training
   corpus must both be GPL-3-compatible; keep it fully offline; ship weights
   outside the reproducible-build hash. Piper is the first candidate.
8. **Local transcription** (whisper.cpp / whisper-rs) so audio never leaves the
   machine.
9. **Non-cryptographic voice-changer mode** — masculine/feminine sliders,
   monitor toggle. **Must be visually and textually distinct** from
   de-identification so nobody mistakes a fun filter for protection.
10. **Window-kernel spectral synthesis** to lift the bin-grid pitch quantisation
   (see `spectral.rs`).
11. **Publish to crates.io**; per-crate `examples/`.

---

## 8. Hard-won lessons — do not rediscover these

- **`region`'s lock guard panics in its destructor.** Locking is page-granular;
  two small secrets sharing a page meant the first drop unlocked the second's
  memory, and on Windows `VirtualUnlock` fails legitimately. Secrets now own
  whole pages and unlock explicitly.
- **`Zeroize for Vec` truncates to length 0**, it does not merely zero bytes.
  Zeroize the *slice*.
- **lofty cannot remove ID3v2 from WAV.** Hence the chunk-level cleaner.
- **A step's own `env:` is not in scope for its own `if:`.** This silently
  disabled release signing entirely.
- **`echo` appends a newline** — fatal for a passphrase. Use `printf '%s'`.
- **macOS canonicalises `/tmp` → `/private/tmp`**, defeating `--remap-path-prefix`.
- **MSVC needs `/Brepro`; ld64 needs `-no_uuid`** for reproducibility.
- **`reg query` echoes the full hive name**, so querying `HKCU\...` matched
  nothing and the monitor silently reported an empty machine.
- **`--no-install-recommends` drops the cross libc**, so ARMv7 linked with no
  `Scrt1.o`.
- **zlib output differs between Python builds** — compare decoded pixels, not
  compressed bytes.
- **Test that detection actually detects.** The registry bug looked exactly like
  good news. Verify against a real consumer.
- **Struct-update syntax cannot be used on a type that implements `Drop`.**
  `Security` wipes its passphrases on drop, so `Self { path, ..Default::default() }`
  fails to compile with six separate "cannot move out of" errors. Assign the
  fields instead; the comment in `Security::load` says why.
- **A clap argument declared next to `#[command(subcommand)]` must precede the
  subcommand on the command line** — `veilvoice lock --path X status`, not
  `... status --path X` — unless it is marked `global = true`, which is what
  `--path` now does.
- **`rpassword` needs a real console.** Piping a passphrase in does not work; it
  blocks on `CONIN$`. Anything that prompts cannot be smoke-tested from a
  non-interactive shell, so test the layer beneath the prompt instead (which is
  what `atrest.rs` and `cli/lock.rs` do).
- **`hound::WavWriter::finalize` consumes the writer**, so an in-memory encode
  has to borrow the cursor — `WavWriter::new(&mut cursor, spec)` — and read it
  back through `cursor.into_inner()` after the writer is dropped.
- **The lock file is read before anyone has authenticated.** Treat it as hostile
  input: a file that will not parse is an error and keeps the app locked, never
  a missing lock. `LockStore::open` returns `Ok(None)` only for `NotFound`.
- **Render the site before believing it.** Three paragraphs were invisible on
  the published page — one of them the box saying the app lock is not
  tamper-proof — and every unit test passed throughout, because the stub
  modelled the observer firing and the bug was the observer *not* firing.
  `python -m http.server 8787 --directory website`, then look at it.
- **An IntersectionObserver does not fire when the viewport jumps.** An anchor
  link, a restored scroll position or find-in-page can carry an element from
  below the fold to above it between two frames; the ratio never leaves zero, so
  no callback runs. `reveal.js` therefore has a sweep alongside the observer.
  Any reveal effect needs one.
- **A test double must model the platform, not the happy path.** The reveal stub
  only knew how to fire the observer, so it could not express the bug. It now
  models a viewport with a position.
- **Placeholders must be un-parked recursively.** `String.replace` does not
  rescan its own replacement, so a link whose label is inline code emitted an
  anchor around a bare placeholder — an invisible character, so the link looked
  empty. Loop until the string stops changing.
- **Files served raw must be ASCII.** `website/js/*.js` and the licence texts are
  opened directly by readers, and a viewer that guesses CP1252 turns an em dash
  into mojibake in the middle of the sentence promising the code is honest.
  Enforced by `tools/site-tests/characters.test.js`; use `\uXXXX` where a
  character must survive.
- **A test that depends on what software a machine has run is not a test of this
  crate.** The Windows consent-store test failed CI twice for that reason —
  first requiring the store to be non-empty, then requiring a `NonPackaged`
  subkey. Assert the shape of the reply; assert the parser against a key every
  installation has.
- **Argon2 cost parameters come from the file, and `argon2` 0.5.3 validates them
  in the wrong order.** It computes `m_cost < p_cost * 8` before checking
  `p_cost`'s ceiling, so a large `p_cost` overflows. And `m_cost` is allocated
  before anything else, so `u32::MAX` asks for 4 TiB and aborts. Both are now
  bounded in `KdfParams::checked()` — never bypass that funnel.
- **Run the parser campaigns in DEBUG, not release.** The release profile sets
  `overflow-checks = false`, so an arithmetic overflow — one of the two bugs —
  is invisible there. CI runs them in debug on purpose.
- **A green fuzzer is not a read.** F-4 was a 32-bit-only overflow that no
  campaign on an x86-64 machine could ever reach; it was found by reading. Do
  both.
- **The engine keeps persistent state, so one bad sample is forever.** The
  accent neutraliser's long-term spectrum is an EMA: a single NaN in, and every
  output sample afterwards was NaN, silently. Input is now sanitised in
  `StftEngine::process`. A 32-bit-float WAV can legally contain NaN.
- **`x.clamp(-16.0, 16.0)` in the STFT broke a test that feeds a 0..2048 ramp.**
  The gate is for impossible values, not a limiter — keep the bound enormous
  (±1e6) so it never touches anything a real signal or a test probe produces.
- **Timing tests must use the minimum, not the median.** Noise is one-sided.
  A first pass using medians on Windows reported a 1.49× "leak" that was pure
  scheduler jitter; the minimum gave 0.996 and was stable across runs.
- **A hostile-input checker has to parse like a browser.** Scanning the raw
  string for `onerror=` calls `&lt;script&gt;` an attack — that is the renderer
  *working*. Five of the first six "findings" were the checker, not the code.
- **`website/js/markdown.js` contains literal NUL bytes** (it did, as parking
  sentinels). String-matching editors cannot target them; patch such files with
  a script. They are gone now, replaced by private-use characters.
