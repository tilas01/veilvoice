<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice — what it destroys, what it keeps, and why

**Version 0.1.0.** This document is the honest version of the pitch. It states
what VeilVoice guarantees, how, and — at least as importantly — what it does
not. A privacy tool that overstates its reach is worse than none, because
someone will rely on the part that was never true.

---

## 1. The goal, and the contradiction in the obvious version of it

The intuitive ask is "make it impossible to isolate my voice — fill the whole
spectrogram with noise." That cannot be built, because it conflicts with the
other half of the requirement. Audio that stays **understandable and
transcribable** must carry the phonemes; noise that covers the phonemes covers
the words. The two goals are mutually exclusive, and no amount of engineering
reconciles them.

So VeilVoice targets the achievable and genuinely useful goal:

> **Irreversible destruction of the speaker's identity, with intelligibility
> preserved on purpose.**

The *voiceprint* — fundamental pitch, formant structure, timbre, micro-timing,
and the melody of an accent — is destroyed. The *words* survive. If the message
also needs to be secret, that is a different problem with a different solution:
encrypt it.

---

## 2. Threat model

**Assumed adversary.** Holds the output audio. Has unlimited compute, the full
VeilVoice source, and knowledge of every parameter *except* the per-session
random seed. May hold reference recordings of the target speaker and use
state-of-the-art speaker-recognition models. May store the file indefinitely,
including until a cryptographically relevant quantum computer exists.

**In scope.** Recovering the original waveform. Recovering the speaker's
biometric voiceprint. Matching the output against a reference recording of the
same speaker.

**Explicitly out of scope.**

- **The words.** Preserved deliberately. See "If the message must be secret
  too" below.
- **Background content.** Room acoustics, a doorbell, a colleague's voice, a
  regional siren — VeilVoice processes the whole signal but does not attempt
  scene sanitisation. Check what else is in your recording.
- **An attacker already on your machine.** If they can read process memory or
  tap the microphone before VeilVoice does, nothing here helps.
- **Metadata outside the file.** Filenames, filesystem timestamps and the
  channel you send it over. `veilvoice-meta` cleans metadata *inside* the file
  only.

---

## 3. Why the transform is one-way

Three independent mechanisms, each individually lossy. Reversal requires
defeating all three.

### 3.1 Phase is discarded every frame

For each STFT frame VeilVoice keeps only the magnitude spectrum and throws away
the measured phase. Phase encodes the precise waveform and the speaker's
micro-timing — the excitation pattern that makes one glottis distinguishable
from another.

This is not obfuscation, it is deletion. The information is never written down,
never stored, and never derivable from what remains: an infinite family of
waveforms shares any given magnitude spectrogram. A fresh, synthetic phase is
generated in its place.

### 3.2 Every speaker is collapsed onto one canonical identity

Three of the strongest biometric features are *normalised*, not randomised:

| Feature | What happens |
|---|---|
| Pitch register | Mapped to one constant canonical fundamental |
| Vocal-tract length | Warped so the long-term formant centroid hits one canonical value |
| Long-term spectral tilt | Rotated onto one canonical slope |

Each mapping is **many-to-one**. A whole population of speakers lands on the
same output value, so the original cannot be inferred from the result — there is
nothing to invert, only a value that many inputs share. This is strictly
stronger than randomising those features, which would merely displace them.

Crucially, every correction is derived from a **multi-second average**, never
from the current frame. Per-frame spectral shape is what distinguishes /i/ from
/u/; normalising it frame-by-frame would erase the vowels along with the accent.
The engine's test suite asserts that vowel contrast survives.

### 3.3 The residual transform is non-stationary and CSPRNG-driven

The formant ratio changes every frame, drawn from a ChaCha20 stream whose
32-byte seed comes from the OS CSPRNG, lives only in page-locked RAM, and is
zeroized on drop. There is no single fixed transform to undo, and the sequence
is unknowable without the seed, which is never written anywhere.

The per-bin synthesis phase offsets come from the same stream.

---

## 4. Accent: what is removed, and the limit

Accent is carried by two different kinds of cue, and they get different answers.

**Suprasegmental cues — removed.** Intonation contour, pitch range, voice
quality, and the vocal-tract scale behind a speaker's vowel space. These are
properties of the signal, and the normalisation described above collapses all
of them.

**Segmental cues — cannot be removed.** *Which phonemes the speaker actually
produced*: rhoticity, vowel mergers, dental-fricative substitution, aspiration
patterns. At this level the accent **is** the words. Changing it means deciding
that a different phoneme was said, which no filter can do — it requires
recognising the speech and re-synthesising it.

**Therefore: a strong regional accent may still be audible in the output, even
though its melody and colour are gone.** VeilVoice does not claim otherwise. The
planned text-to-speech mode closes this gap completely, because it never carries
the original signal at all.

---

## 5. Synthesis, and an honest note on how it sounds

Voiced frames are resynthesised as an ideal harmonic comb at the canonical
fundamental, quantised to the nearest FFT bin, passed through the (warped,
tilt-corrected) formant envelope. This is the textbook source-filter model.

Two consequences worth stating plainly:

- **The output has a synthetic, even quality.** Pitch is constant by design.
  This is the sound of the identity being gone, not a defect.
- **Pitch resolution is limited by the bin grid** (46.875 Hz at the 48 kHz /
  1024 default), so the canonical register lands on the nearest bin. Irrelevant
  when flattening fully, which is the default; partial flattening steps rather
  than glides. Lifting this needs window-kernel synthesis, which is future work.

Unvoiced frames keep a channel-vocoder phase, which is the correct model for
fricatives and noise.

---

## 6. What an attacker can still learn

Stated so nobody is surprised:

- **The words.** By design.
- **Speaking rate and rhythm.** VeilVoice does not time-warp. Rate is a weak
  biometric but a real one, and it survives.
- **Language, dialect vocabulary, and idiolect.** Word choice is content.
- **Whether two outputs came from the same *session*.** Within one session the
  seed is fixed. Different sessions are unlinkable; a single long recording is
  internally consistent.
- **Coarse voice-activity structure** — when you spoke and when you did not.

---

## 7. If the message must be secret too

Use `veilvoice-crypto`. De-identification and confidentiality are separate
problems and are solved separately:

- **Argon2id** (RFC 9106 profile) for password-derived keys.
- **X25519 + ML-KEM-768 hybrid** for public-key encryption. Hybrid because
  ML-KEM is young and X25519 falls to a quantum adversary; breaking the
  construction requires breaking both. This matters for
  *harvest-now-decrypt-later*: a recording stored today may be attacked
  decades from now.
- **XChaCha20-Poly1305** for the payload, with random 192-bit nonces, and the
  full container header authenticated as associated data — so an attacker
  cannot downgrade the stored KDF cost to make cracking cheap.
- **Page-locked, zeroizing secrets**, so keys do not reach the swap file.
  Locking does not survive hibernation and does not stop an attacker who can
  already read process memory; `Secret::is_locked` reports whether it actually
  succeeded rather than assuming.

---

## 8. Verifying these claims yourself

Nothing here asks for trust:

```bash
cargo test --workspace          # the properties above are asserted in tests
cargo run -p veilvoice-core --example spectrum_report
```

`spectrum_report` prints where the output partials land, demonstrating the
synthesis behaviour directly. The engine's tests assert speaker convergence, vowel-contrast
survival, gain neutrality and real-time performance. The crypto tests assert
header-downgrade detection, tamper detection on each half of the hybrid, and
that a wiped secret is actually wiped.

**VeilVoice contains no `unsafe` code.** Every crate carries
`#![forbid(unsafe_code)]`, including the page-locking path.

---

## 9. Status of this document

This is a design and rationale document, not a peer-reviewed security proof.
The de-identification argument rests on information destruction that is easy to
verify by reading `spectral.rs` and `accent.rs`; the cryptography uses standard,
well-reviewed primitives from the RustCrypto and dalek ecosystems rather than
anything invented here.

The code has been **audited by tilas01**, who wrote and reviewed it. That is a
maintainer audit and is worth exactly what a maintainer audit is worth: it
catches what the author can see. **No external firm or independent researcher
has reviewed this code**, and until one has, the strongest verification
available to you is the source itself — which is why it is written to be read.
