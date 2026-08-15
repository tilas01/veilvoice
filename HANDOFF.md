<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Working handoff note. Safe to delete once the project is rolling. -->

# VeilVoice — resume-here handoff

Working notes and roadmap. Everything the project needs lives inside this
folder; open it as its own workspace.

## Build / test

Building inside a cloud-synced folder (OneDrive, Dropbox) makes the sync client
fight the compiler for file locks, so redirect the target directory first:

```powershell
$env:CARGO_TARGET_DIR = "$env:LOCALAPPDATA\veilvoice\target"
cargo test --workspace   # 151 tests + doctests
cargo clippy --workspace --all-targets   # clean, zero warnings
cargo audit              # policy in .cargo/audit.toml
cargo build --release
```

Useful when changing the DSP — prints where the output partials actually land,
which is the fastest way to see the two synthesis modes:

```powershell
cargo run -p veilvoice-core --example spectrum_report
```

## Locked decisions (confirmed by the user)

1. **Goal = irreversible speaker de-identification with preserved
   intelligibility.** Not white-noise spectral fill. "Impossible to recover the
   voice" and "must stay understandable/transcribable" are mutually exclusive:
   the words are kept on purpose; the *voiceprint* (pitch, formants, timbre,
   accent, micro-timing) is what gets destroyed.
2. **GUI = egui/eframe**, black minimal, Tokyo Night palette, JetBrains Mono.
3. **Ship the PUBLIC verify key only** — the private signing key never goes in
   the repo or the binary. Anyone can `cargo build` with **no secrets**; release
   signing secrets are added to CI later by the maintainer.
4. Identity = pseudonym **tilas01**, **no e-mail anywhere**, GPL-3.0-or-later.
5. Left as plain files for now (no `git init` yet — do it when ready).
6. **Accents must not survive either.** Implemented in `accent.rs`; scope and the
   one honest limit are in "Accent neutralisation" below.
7. **A text-to-speech mode is wanted** for total anonymity — see milestone 9.

## Forensic-irreversibility requirement (design contract)

The user requires that the original voice be **forensically unrecoverable** from
any output. How the design delivers this, and its honest limit:

- **Phase is discarded every frame.** `spectral.rs` keeps only the magnitude
  spectrum and resynthesises a fresh phase, so the original excitation phase —
  the exact waveform and speaker micro-timing — is never stored and cannot be
  inverted. This is the core one-way step.
- **Non-stationary CSPRNG modulation.** The formant ratio (and, on unvoiced
  frames, the pitch ratio) changes every frame from a ChaCha20 stream whose
  32-byte seed comes from the OS CSPRNG, lives only in RAM, and is zeroized on
  drop. There is no single fixed transform to undo, and the target sequence is
  unknowable without the seed. On voiced frames the fundamental is instead
  pinned to a constant canonical register, which destroys strictly more pitch
  information than randomising it would.
- **Envelope/excitation split + independent warping** moves the biometric
  formant structure somewhere it never was while phonemes stay legible.
- **Many-to-one normalisation.** Pitch register, vocal-tract scale and long-term
  spectral tilt are each collapsed onto one canonical value, so a whole
  population of speakers maps to the same output. This is information
  destruction, not displacement, and it is independent of the phase discard.
- **Honest caveat to state in the whitepaper:** because intelligibility is
  preserved, the *linguistic content* (the words) is by definition still
  present — that is the point. What is made unrecoverable is *speaker identity /
  the original voice*, not the message. The whitepaper must prove the identity
  claim and not overclaim message secrecy (for message secrecy, encrypt the
  file with `veilvoice-crypto`).
- TODO for the crypto milestone: option to **derive the modulation seed from a
  session key** so a user can reproduce a scramble only if they hold the key,
  and to encrypt any at-rest recording with ChaCha20-Poly1305 (PQ-hybrid
  wrapped) so nothing hits disk in the clear.

## Accent neutralisation (`accent.rs` + `pitch.rs`) — done

Accent splits into two kinds of cue, and they get opposite answers:

- **Suprasegmental (removed).** Intonation contour, pitch register, vocal-tract
  scale, long-term spectral tilt. Every speaker is mapped onto one canonical
  target, so these are gone. Each correction is *many-to-one*, which destroys
  information and therefore composes with the phase discard rather than merely
  displacing identity.
- **Segmental (cannot be removed, and this must not be overclaimed).** Which
  phonemes were actually produced — rhoticity, vowel mergers, /θ/→/t/. At that
  level the accent *is* the words; changing it means changing what was said. No
  filter can do it. The text-to-speech mode (milestone 9) is the real answer,
  because it never carries the original signal at all. **The whitepaper must
  state this limit plainly.**

How it works, and the two rules that keep it safe:

1. **Corrections are always long-term, never per-frame.** Per-frame spectral
   shape is what distinguishes /i/ from /u/; normalising it frame-by-frame would
   erase the vowels along with the accent. Vocal-tract and tilt estimates use
   multi-second time constants, so they track the *speaker* and let the phonemes
   move freely underneath. A test asserts vowel contrast survives.
2. **Tilt shaping is a straight line in log-frequency and nothing more.** A
   bin-by-bin match to a canonical spectrum removes more speaker colour, but it
   also flattens per-frame vowel differences — measured at a 10× collapse in
   vowel separation before this was cut back to a pure tilt rotation. A smooth
   monotone ramp is structurally incapable of that.

`pitch.rs` is a decimated YIN tracker. It works in the time domain because the
FFT cannot resolve f0 at usable frame sizes (47 Hz bins cannot tell 100 Hz from
140 Hz); decimating to 8 kHz keeps it at roughly 8 Mflop/s.

### Voiced resynthesis was rewritten (this was a real defect)

The original channel-vocoder synthesis gave every bin its own centre frequency,
so each harmonic became a cluster of grid-frequency sinusoids with unrelated
phases — a 210 Hz voice came out beating between 187.5 and 234.4 Hz. That is the
metallic-vocoder sound, and it also made the canonical register *unreachable*,
because output pitch was quantised regardless of the ratio applied.

Voiced frames with accent on now replace the excitation with an ideal harmonic
comb at the canonical fundamental, snapped to the nearest whole bin — the
textbook source-filter model. Because every comb line sits exactly on a bin
centre, the existing per-bin phase advance is exactly right for it and the
partials overlap-add coherently. `spectrum_report` shows the difference: an
exact harmonic series at 140.6 Hz whatever pitch went in. Unvoiced frames keep
the channel-vocoder path, which is correct for fricatives and noise.

Consequences worth knowing:

- Grid snapping costs pitch resolution. Irrelevant at the default
  `prosody_flatten = 1.0` (one constant register, snapped once), but any
  *partial* flattening steps across the grid instead of gliding. **Future work:**
  window-kernel (Hann-mainlobe) spectral synthesis would give arbitrary f0
  resolution and lift the restriction.
- The canonical register lands on the nearest grid bin, so its exact value
  depends on `sample_rate / frame_size` — 140.6 Hz at the 48 kHz / 1024 default.
- The CSPRNG pitch modulation is deliberately **not** applied to the comb.
  Pitch normalised to a constant already carries nothing, so randomising it adds
  no de-identification strength; the CSPRNG still drives the formant ratio and
  the per-bin phase offsets, so the transform stays non-stationary.

## Status

- ✅ `veilvoice-core` — de-identification DSP engine, **done + 40 tests pass,
  clippy clean**. Streaming phase-vocoder STFT (realfft), magnitude-only
  resynthesis (the irreversible step), ChaCha20 per-frame pitch/formant
  modulation, accent neutralisation, harmonic-comb voiced resynthesis, chorus /
  light reverb / soft-clip, live ms-latency + realtime-factor stats. Group delay
  = one frame (1024 samples ≈ 21.3 ms @ 48 kHz). Measured realtime factor with
  accent tracking on is **0.009** (≈1 % of one core) in the low-optimisation
  test profile, so pitch tracking costs essentially nothing.
- ✅ `veilvoice-crypto` — **implemented, 51 tests.** Argon2id (RFC 9106
  profile), X25519 + ML-KEM-768 hybrid KEM with an HKDF transcript-binding
  combiner, XChaCha20-Poly1305, the `.veil` container with an authenticated
  header, and page-locked zeroizing `Secret`.
- ✅ `veilvoice-audio` — **implemented, 20 tests.** symphonia decode to mono
  f32, hound WAV write, device enumeration with virtual-cable detection, and a
  live capture→de-identify→playback path over a lock-free ring buffer.
- ✅ `veilvoice-meta` — **implemented, 24 tests.** lofty for tag containers,
  a chunk-level RIFF cleaner for WAV, img-parts for image EXIF/GPS.
- ✅ `veilvoice-cli` — **implemented, 7 tests.** `anonymise`, `live`, `devices`,
  `clean`, `encrypt`, `decrypt`, `keygen`, `info`.
- ✅ `veilvoice-gui` — **implemented, 8 tests.** egui/eframe, Tokyo Night,
  monospace, three modes, live meters.
- ✅ Artwork, docs and CI — generated pixel-art icon/banner, whitepaper,
  reproducible-build guide, cross-platform CI and a release workflow that
  double-builds and diffs every binary.

## Remaining plan

1. ~~`veilvoice-crypto`~~ — **done.** OpenPGP was deliberately dropped from the
   crate: release signing belongs in CI (see `release.yml`), not in-process, and
   a stub feature flag would have been misleading.
2. ~~`veilvoice-audio`~~ — **done.** VB-CABLE is detected by name, not bundled.
3. ~~`veilvoice-gui`~~ — **done** for the three core modes. A live spectrograph
   and the mini studio are still open (see milestone 10).
4. **Terminal/TUI mode** (ratatui + crossterm) — the CLI covers scripting and
   headless use with live meters already; a full TUI is still open.
5. **Tray + hidden-background** operation with an **opt-in, configurable global
   hotkey** to restore (nothing bound by default; suggested default the user can
   accept or remap).
6. `veilvoice-meta` — strip/spoof audio tags + image EXIF/GPS (realistic vs.
   anything-valid mode); oxipng for lossless image optimisation (credit it).
7. WiX installer + portable zip; bundle VB-CABLE silent installer with
   detect-and-skip and vb-cable.com / donationware attribution.
8. Whitepaper, pixel-art icon + banner, reproducible-build CI (maxed, bit-for-
   bit verifier, hashes + OpenPGP sig in one zip, gated on secrets), SEO site +
   LIBRE policy explainer.
9. **Text-to-speech mode — secondary, after the repo is release-ready.** Type
   text, an AI voice speaks it live into the same output device. This is the
   *strongest possible* anonymity: the original voice is never captured, so
   there is nothing to recover and no segmental accent to leak — it is the one
   thing that closes the gap `accent.rs` documents. Notes for whoever builds it:
   - Ship **a few good open-source voices, feminine and masculine.** Check
     licences carefully: the model weights *and* the training corpus must both
     be redistributable under something GPL-3.0-compatible, and each voice needs
     credit. Piper (MIT, ONNX, CPU-real-time) is the obvious first candidate.
   - Keep it **fully offline** — no network calls, ever. That is a hard
     requirement of this project, not a preference, and it must be verified in
     the audit.
   - Weights are large and are **not** reproducible-build inputs; ship them as a
     separately-hashed download or an optional installer component so the
     bit-for-bit verification of the binary still holds.
   - Feed the synthesised audio through the same output path (VB-CABLE routing,
     meters, latency read-out) so it works in any app that takes a microphone.
   - Consider a push-to-talk / queued-line UX so typing latency is not audible
     mid-sentence.
10. **Second wave — after the first audit and first public release.** All
    requested; deliberately queued behind shipping and auditing v0.1.0.
    - **Library-first documentation.** The crates already work as dependencies
      and README now shows it, but this deserves proper rustdoc landing pages,
      a `examples/` directory per crate, and publishing to crates.io.
    - **Private transcription pipeline.** README documents the manual route
      (anonymise, then upload). Build it in: bundle or detect `whisper.cpp` /
      `whisper-rs` for fully local speech-to-text, so the audio never leaves the
      machine at all. Offer "transcribe locally" as the default and "anonymise
      then send to a cloud service" as the explicitly-labelled fallback. Keep
      the honest caveat that recognition accuracy drops on synthetic speech, and
      measure how much.
    - **Non-cryptographic voice-changer mode.** A separate, clearly-labelled
      mode for entertainment and comfort rather than anonymity: masculine and
      feminine presets on continuous sliders, pitch/formant/distortion controls,
      and a monitor toggle to hear yourself live. **It must be visually and
      textually distinct from the de-identification mode** — a user must never
      believe a fun voice filter is protecting their identity. Label it as not
      irreversible.
    - **In-app recording studio.** Record, trim, fade, normalise, export.
      Note: Audacity is **GPL-2.0-or-later**, which is *not* compatible with
      this project's GPL-3.0-or-later in the direction of copying code in — do
      not lift its source. Build on permissive Rust crates instead
      (`symphonia` and `hound` are already dependencies; add `rubato` for
      resampling), or relicense deliberately after checking.
    - **UI polish pass.** Re-examine layout for minimal, intuitive, easy on the
      eyes once the extra modes land; regenerate icons at every size.
11. **Final gate: full recursive security / best-practice / vulnerability audit**
    of the whole repo, run after everything above including the TTS mode.
    Cross-platform priority: **Windows 10/11 first**, then macOS (Apple Silicon
    + older Intel), then Linux; BSD optional.
