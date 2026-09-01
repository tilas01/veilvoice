<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

<!-- The animated banner, as a GIF, for the reason the still one was
     here instead: a README is read in a hundred clients that handle
     animation differently. The version this replaces needed a
     `<picture>` element to offer an APNG with a still fallback, and
     GitHub's renderer escapes `<picture>` -- which put a paragraph of
     raw markup above the project's name in the website's repository
     panel. One plain Markdown image needs no element to escape, and
     GIF is the one animated format every client draws. A client that
     will not animate it shows the first frame, which is exactly
     `assets/banner.png`.

     Nothing here is a committed blob: `assets/generate.py` draws the
     frames, writes the GIF with its own LZW encoder, and CI fails the
     build if the file and the generator disagree. The website does not
     serve this picture at all any more -- its banner is drawn in CSS,
     so it follows the reader's palette and its claims are text. -->
![VeilVoice: irreversible voice de-identification](assets/banner.gif)

# VeilVoice

**Irreversible voice de-identification, fully offline.**

### → [tilas01.github.io/veilvoice](https://tilas01.github.io/veilvoice/)

Website, wiki, and an in-browser hash verifier that never uploads your file.
There is a [JavaScript-free edition](https://tilas01.github.io/veilvoice/nojs/)
for readers who would rather not run scripts.

VeilVoice destroys the *biometric voiceprint* of a speaker, meaning pitch,
formants, timbre, micro-timing and the melody of an accent, so that neither software nor
a human listener can re-identify the speaker or reconstruct the original voice,
**while the words themselves stay clean and transcribable**.

There is no telemetry, no account, and no network code in the dependency
graph, and CI fails the build if an HTTP client appears in it.

**One thing reaches the network, and only when you press it.** The desktop app
has a *check for updates* button. It runs then and at no other time: no timer,
no check at startup, nothing in the background. It sends nothing about you or
your machine, it reads a public page anybody can open, and it downloads and
installs nothing: it reports a version number and every decision after that is
yours. There is still no HTTP client in the dependency graph: like the release
verifier, it borrows the transfer tool your operating system already ships.

Anonymising, scrambling, encrypting and every other thing VeilVoice does still
talk to no servers at all.

---

## What it does

1. **Anonymise a recording.** Wav, mp3, flac, ogg, m4a and friends in; a clean
   WAV out, with metadata stripped.
2. **Scramble your microphone live** and route the result to a virtual audio
   cable, so any application, whether a call, a stream or a recorder, receives the
   veiled voice instead of yours.
3. **Encrypt recordings at rest, by default.** Every file VeilVoice writes is
   sealed with post-quantum-hybrid cryptography unless you explicitly turn that
   off, and turning it off makes you read why first.
4. **Lock the app** behind a separate password, so someone who picks up your
   unlocked computer cannot open it. See the honest limits below.
5. **Detect tampering** with VeilVoice's own files, and say plainly when it
   cannot tell you which program did it.
6. **Strip identifying metadata** from audio and images (EXIF, GPS, tags).
7. **Watch what is listening.** See every application currently holding your
   microphone or camera, with alerts the moment one starts.
8. **Securely erase** a recording, with an honest account of what that is worth
   on flash storage.
9. **Work as a Rust library** in your own project. See below.

> ### Honest scope
>
> "Fill the whole spectrogram with white noise" and "stay understandable and
> transcribable" are mutually exclusive, because noise that covers the voice covers the
> words. VeilVoice therefore targets the achievable goal: **irreversible speaker
> de-identification with intelligibility preserved on purpose.** If the *message*
> also needs to be secret, encrypt it. That is a separate problem with a
> separate answer.
>
> The same honesty applies to **accent**. VeilVoice maps every speaker onto one
> canonical pitch register, vocal-tract scale and spectral tilt, so an accent's
> melody and colour do not survive. What no signal-level transform can change is
> *which phonemes you actually produced*, and at that level the accent and the words
> are the same thing.
>
> And to the **app lock**. It is an Argon2id password verifier with a rate
> limit, and it protects against *casual access*, meaning the person who picks up your
> unlocked laptop. It is not tamper-proof and it is not disk encryption: anyone
> who can write to your files can delete the lock, and anyone holding the drive
> can attack the stored hash offline. VeilVoice says this on the unlock screen
> itself rather than in a footnote. If the disk is the threat, encrypt the
> volume.
>
> The full argument, and everything an attacker can still learn, is in
> [`docs/WHITEPAPER.md`](docs/WHITEPAPER.md).

---

## What it looks like

Every picture below is of this build. The window captures are taken by
`tools/shots/gui.ps1`, which drives the release build and photographs each tab;
the terminal drawings are generated from the command output committed beside
them, and CI fails if a drawing and its output disagree. See
[`assets/screenshots/README.md`](assets/screenshots/README.md) for why those two
are different kinds of thing.

### The desktop application

| | |
|---|---|
| **Anonymise a file.** One recording, veiled, encrypted at rest by default. | **Live scramble.** A microphone in, a voice that is not yours out. |
| ![anonymise a file](assets/screenshots/gui-file.png) | ![live scramble](assets/screenshots/gui-live.png) |
| **Group mode.** Several people, a name and a colour each. | **Monitor.** Who is using the microphone and camera. |
| ![group mode](assets/screenshots/gui-group.png) | ![monitor](assets/screenshots/gui-monitor.png) |
| **Lock.** The app lock, and what it is and is not worth. | **Verify.** Drop a download on the window and be told what it is. |
| ![the app lock](assets/screenshots/gui-lock.png) | ![verify a download](assets/screenshots/gui-verify.png) |
| **Settings.** Nine palettes, motion, Failsafe, and which tabs are shown. | **Install.** Offered only to a portable copy. |
| ![settings](assets/screenshots/gui-settings.png) | ![install](assets/screenshots/gui-install.png) |
| **About.** Versions, scope, and the update check you press. | |
| ![about](assets/screenshots/gui-about.png) | |

Every one of these is taken by `tools/shots/gui.ps1`, which starts the
application once per tab with `--tab <name>`, maximises the window and
photographs it with `PrintWindow`. There is no clicking and there are no
coordinates, so a picture cannot quietly end up showing the wrong tab. They are
captured at the full resolution of the screen they were taken on.

**Confirmed on Windows 11.** That is the one this application has actually been
run and photographed on, and these pictures come from it.

**Windows 10 is supported and not yet confirmed**, which is a different
sentence and is meant to be. Nothing in the desktop application needs anything
newer than Windows 10: the oldest interfaces it uses are `DwmGetWindowAttribute`
(Windows Vista), `SetProcessDpiAwareness` and `PrintWindow` with
`PW_RENDERFULLCONTENT` (Windows 8.1), and the two are only used by the
screenshot tool in any case. The application itself asks for nothing beyond
`whoami`, `tasklist`, `taskkill` and `reg`, all of which predate Windows 10 by
years. So it should run, and saying "it does" is not something this page will
claim until somebody has sat in front of one.

macOS and Linux build and their tests pass in CI, which is weaker still: a
green test run is not a person using the window.

### The command line

Everything the window does, and some things it does not.

![veilvoice --help](assets/screenshots/cli-help.svg)

![veilvoice conversation --help](assets/screenshots/cli-conversation.svg)

![veilvoice anonymise --help](assets/screenshots/cli-anonymise.svg)

<details>
<summary>The rest of the commands</summary>

![veilvoice live --help](assets/screenshots/cli-live.svg)

![veilvoice conversation render --help](assets/screenshots/cli-render.svg)

![veilvoice conversation preview --help](assets/screenshots/cli-preview.svg)

![veilvoice companions --help](assets/screenshots/cli-companions.svg)

![veilvoice capture --help](assets/screenshots/cli-capture.svg)

![veilvoice guard --help](assets/screenshots/cli-guard.svg)

![veilvoice clean --help](assets/screenshots/cli-clean.svg)

</details>

## Install

Grab a build from [Releases](https://github.com/tilas01/veilvoice/releases),
or verify one first with the
[in-browser verifier](https://tilas01.github.io/veilvoice/#verify), or build it
yourself, since a fresh clone needs **no secrets**:

```bash
git clone https://github.com/tilas01/veilvoice && cd veilvoice
cargo build --release
```

Release binaries are built twice in different directories and verified
byte-identical. Install with a script that refuses rather than continues
([`docs/INSTALL.md`](docs/INSTALL.md)), check a download without GnuPG
installed (`veilvoice-verify`), or package it yourself
([`docs/PACKAGING.md`](docs/PACKAGING.md)).
See [`docs/REPRODUCIBLE_BUILDS.md`](docs/REPRODUCIBLE_BUILDS.md)
to check a download against the source yourself.

---

## Use it

### Desktop app

```bash
veilvoice-gui
```

Three modes, being anonymise a file, scramble live, and an about panel that states
the scope. Tokyo Night, monospace, dark.

### Command line

```bash
veilvoice anonymise recording.mp3 -o clean.wav   # writes clean.wav.veil, sealed
veilvoice anonymise recording.mp3 --encrypt-to friend.pub
veilvoice anonymise recording.mp3 --encrypt false   # warns first
veilvoice live --output "CABLE Input (VB-Audio Virtual Cable)"
veilvoice devices
veilvoice clean photo.jpg
veilvoice encrypt secret.wav
veilvoice decrypt clean.wav.veil -o clean.wav
veilvoice keygen
veilvoice lock set                     # password-gate the desktop app
veilvoice lock status
veilvoice guard init --sealed          # record what the files should be
veilvoice guard check                  # and see whether they still are
veilvoice watch                        # who is using the mic and camera
veilvoice shred secret.wav             # irreversible
```

Every command takes `--help`.

### Encrypted by default

`anonymise` seals its result into a `.veil` container rather than writing a bare
WAV, because de-identification and confidentiality are different problems and
only the first one is solved by the engine: **the words survive on purpose**, so
an unencrypted result is still a recording of everything that was said.

The WAV is encoded in memory and sealed there, so a recording that is going to be
encrypted never touches the disk in the clear, not even for a moment, because a
plaintext file that is written and then deleted is exactly what
[`veilvoice shred`](crates/veilvoice-crypto/src/shred.rs) explains cannot be
reliably taken back on flash storage.

Passing `--encrypt false` still works. It prints what you are giving up and, on
a terminal, waits for you to type `UNENCRYPTED`.

### Who is listening?

De-identifying your voice on a call achieves little if a second program is
recording the raw microphone at the same time. `veilvoice watch` names what is
holding your microphone and camera, and alerts the moment something starts:

```
● veilvoice is now using your microphone
```

Windows reads the same records that drive the OS privacy indicator; Linux reads
open handles under `/proc`. **macOS exposes no public interface for this**, so
nothing is reported there rather than something guessed: the tool tells you it
cannot see, because an empty list from a blind monitor is a false reassurance.

---

## Use it as a library

**Worked examples, with the licence implications spelled out, are in
[`docs/USING_THE_CRATES.md`](docs/USING_THE_CRATES.md).** Every example there is
a real file under `crates/*/examples/`, compiled on every commit, so none of it
can quietly stop being true:

```bash
cargo run -p veilvoice-core   --example veil_a_buffer
cargo run -p veilvoice-crypto --example seal_and_open
```

Every crate is a normal Rust library. Point Cargo at the repository:

```toml
[dependencies]
veilvoice-core  = { git = "https://github.com/tilas01/veilvoice" }
veilvoice-audio = { git = "https://github.com/tilas01/veilvoice" }
```

| Crate | What it gives you |
|---|---|
| `veilvoice-core` | The de-identification engine. No I/O, no threads, allocation-free `process()`. |
| `veilvoice-audio` | Device enumeration, file decode/encode, live capture→process→playback. |
| `veilvoice-crypto` | Argon2id, X25519+ML-KEM-768 hybrid, XChaCha20-Poly1305, page-locked secrets, the app-lock verifier. |
| `veilvoice-meta` | Metadata stripping for audio and images. |
| `veilvoice-watch` | Microphone and camera use, by application. Zero dependencies. |
| `veilvoice-guard` | Integrity manifest and tamper detection for VeilVoice's own files. |
| `veilvoice-setup` | Per-user install and its exact reversal, and detection of the optional companion software. |
| `veilvoice-sentry` | Ransomware canaries and directory churn measurement. Detects; stops nothing. |
| `veilvoice-policy` | Settings that can only be tightened, sealed with the same post-quantum container. |
| `veilvoice-drivers` | What is loaded in the kernel, compared against last time, with a cross-view check. |
| `veilvoice-capture` | Which screen recorders are running, and an allowlist for the ones you meant. |
| `veilvoice-conversation` | Several speakers in one recording: a voice each, names, and subtitles. |

The engine itself is small enough to drop into an audio callback:

```rust
use veilvoice_core::{DeidConfig, Deidentifier};

let mut deid = Deidentifier::new(DeidConfig::default())?;
let mut out = vec![0.0; block.len()];
deid.process(&block, &mut out);   // no allocation, callback safe
```

### Example: speech-to-text without handing over your voice

Cloud transcription is genuinely useful and genuinely invasive: the provider
receives a biometric identifier that is as durable as a fingerprint, and it
usually keeps it. But transcription only needs the *words*, which is exactly
the half VeilVoice preserves.

So run the audio through VeilVoice first. The service gets speech it can
transcribe and a voiceprint that belongs to nobody:

```rust
use veilvoice_audio::{deidentify, io};
use veilvoice_core::DeidConfig;

// Your real voice never leaves this function.
let original = io::load(std::path::Path::new("dictation.wav"))?;
let veiled = deidentify(&original, DeidConfig::default())?;
io::save_wav(std::path::Path::new("safe-to-upload.wav"), &veiled)?;
```

Or from the shell:

```bash
veilvoice anonymise dictation.wav -o safe-to-upload.wav
```

**Two caveats, stated plainly.** Accuracy drops somewhat, because the output is
synthetic-sounding, and recognisers are trained on natural speech. And *the
words still go to the provider*: this protects your identity, not the content of
what you said. If the content is sensitive too, do not upload it at all:
transcribe locally.

Local transcription is the stronger answer and is a planned integration (see
[`ROADMAP.md`](ROADMAP.md)); until then, `whisper.cpp` reads the WAV VeilVoice
writes with no extra work.

---

## Design pillars

- **Offline by construction.** Zero servers, enforced in CI.
- **No `unsafe` anywhere.** Every crate carries `#![forbid(unsafe_code)]`,
  including the page-locking path.
- **Irreversible.** Each frame's measured phase is discarded and resynthesised,
  permanently destroying the speaker's waveform and micro-timing.
- **Normalising, not just scrambling.** Pitch register, vocal-tract length and
  spectral tilt are each collapsed onto one canonical target, so a whole
  population of speakers maps to the same output. That destroys information
  rather than moving it.
- **Cryptographically modulated, with a rolling seed.** The residual transform
  is driven every frame by a ChaCha20 CSPRNG whose seed never leaves the
  process, and that seed is ratcheted forward every couple of seconds, so each
  stretch of audio is sealed off behind a one-way step rather than sharing one
  stream with the whole recording. Configurable, and inaudible by construction.
- **Post-quantum ready, and on by default.** At-rest encryption is X25519 +
  ML-KEM-768 hybrid, because a recording stored today may be attacked decades
  from now, and it is what `anonymise` does unless you say otherwise.
- **Amnesic.** Secrets are page-locked out of swap, zeroized on drop, compared
  in constant time, and opaque to `Debug`.
- **Reproducible & verifiable.** Pinned toolchain, committed lockfile,
  path-remapped builds, and a double-build check in CI.
- **Libre.** GPL-3.0-or-later.

---

## Layout

| Crate | Purpose |
|-------|---------|
| `veilvoice-core`   | De-identification DSP engine and accent neutralisation, the security-critical heart. |
| `veilvoice-crypto` | Argon2id, X25519+ML-KEM-768 hybrid, XChaCha20-Poly1305, amnesic secrets. |
| `veilvoice-audio`  | Capture/playback (cpal), virtual-cable routing, file import/export. |
| `veilvoice-meta`   | Metadata strip/spoof for audio and image EXIF/GPS. |
| `veilvoice-watch`  | Which applications are using the microphone and camera, and alerts on change. |
| `veilvoice-capture` | Screen recorders that are running, muted per program. Does **not** hide VeilVoice's window. |
| `veilvoice-conversation` | Who spoke when, one destination voice each, WebVTT and SubRip subtitles. |
| `veilvoice-drivers` | Loaded kernel drivers and modules, recorded and compared. Detects carelessness, not rootkits. |
| `veilvoice-policy` | Settings fixed so the interface cannot turn them off. Every one of them tightens. |
| `veilvoice-sentry` | Canaries and churn measurement over a directory, an early warning and never a preventer. |
| `veilvoice-setup`  | Per-user install, PATH, removal, and companion detection, shared by both front ends. |
| `veilvoice-cli`    | The `veilvoice` command-line tool. |
| `veilvoice-gui`    | The desktop app (egui, Tokyo Night). |

Artwork is **generated, not committed as opaque blobs**:
`python assets/generate.py` reproduces every icon and the banner from source.

---

## Status

**v0.1.16: early but real.** The engine, cryptography, audio path, metadata
cleaning, at-rest encryption, app lock, tamper detection, encrypted-volume
destinations, CLI and GUI are implemented and tested (1,126 tests across 27
crates plus doctests, and 14 website suites, clippy clean, no `unsafe`), with
randomised campaigns against every parser that reads untrusted input and
against the website's Markdown renderer. Release binaries are built for nine
targets and verified bit-for-bit reproducible on eight of them; the FreeBSD
build is made once in a VM and is reported as `not-verified` rather than
claimed.

**Audited by tilas01**, who wrote and reviewed it. Be clear about what that is
worth: a maintainer audit catches what the author can see, and **no external
firm or independent researcher has reviewed this code**. Read the source before
relying on it for anything that matters. It is written to be read.

Eighteen numbered audit rounds have found and fixed **104 defects** (F-1 to
F-104). Among them: a four-kilobyte file that killed the process, a
configuration value that made every output sample silent, a secure erase that
destroyed a file other than the one named, a locked encrypted volume that went
on accepting recordings onto the ordinary disk, and two ways to freeze a
reader's browser tab. **None in any round was a confidentiality failure** in
the strict sense that nothing let an attacker recover a voiceprint, read a
sealed recording, bypass a password or weaken the cryptography. The two
encrypted-volume defects came closest, and `docs/AUDIT.md` is exact about which
side of that line they fall on rather than leaving the claim to do the work. Every one is written up individually, including
the ones earlier rounds had declared clean, in
[`docs/AUDIT.md`](docs/AUDIT.md).

Using it: [`docs/USER_GUIDE.md`](docs/USER_GUIDE.md), or
[the wiki](https://tilas01.github.io/veilvoice/wiki.html).
Roadmap and open work: [`ROADMAP.md`](ROADMAP.md).

## Licence

GPL-3.0-or-later. See [`LICENSE`](LICENSE).

Virtual audio routing on Windows is usually provided by
[VB-CABLE](https://vb-audio.com/Cable/), which is proprietary donationware and
is **not** bundled here: install it separately if you want it.
