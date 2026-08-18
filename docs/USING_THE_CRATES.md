<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Using VeilVoice as a library

Every part of VeilVoice is an ordinary Rust crate. Nothing here needs the
desktop app, the command-line tool, or a running VeilVoice process — you can
take the de-identification engine on its own, or the container format on its
own, and use it in your own program.

**Every example on this page compiles.** They are not written into this
document by hand: each one is a real file under `crates/*/examples/`, built by
`cargo clippy --workspace --all-targets` on every commit, so an example that
stops compiling fails CI rather than sitting here misleading you. Run any of
them:

```bash
cargo run -p veilvoice-core --example veil_a_buffer
```

---

## The licence, first, because it decides whether you can

VeilVoice is **GPL-3.0-or-later**. That is not a formality and it is worth
being blunt about, because it is not the licence most Rust crates use:

- If you link any of these crates into a program you **distribute**, that
  program must also be released under the GPL-3.0-or-later, with source.
- If you only ever run it yourself — internally, on your own machines, not
  distributed — the GPL places no obligation on you at all.
- If you need it under different terms, there is no dual licence to fall back
  on.

Every *dependency* of these crates is permissive (MIT / Apache-2.0 / BSD / ISC /
Zlib), so nothing else complicates this. The obligation comes from VeilVoice
itself, deliberately.

## Adding a dependency

Not published on crates.io yet, so point Cargo at the repository and pin a tag:

```toml
[dependencies]
veilvoice-core = { git = "https://github.com/tilas01/veilvoice", tag = "v0.1.9" }
```

Pin a **tag**, not a branch. A branch moves under you, and this project's whole
argument is that you can check what you are running.

| Crate | Take it if you want |
|---|---|
| `veilvoice-core` | the de-identification engine, and nothing else |
| `veilvoice-crypto` | the `.veil` container, Argon2id, hybrid PQ key exchange, secure erase |
| `veilvoice-audio` | decoding, WAV writing, device enumeration, live capture |
| `veilvoice-meta` | stripping tags, EXIF and GPS |
| `veilvoice-guard` | file-integrity manifests and tamper reporting |
| `veilvoice-watch` | which applications hold the microphone or camera |
| `veilvoice-verify` | a binary, not a library — see [INSTALL.md](INSTALL.md) |

---

## 1. De-identify a buffer of samples

The smallest useful thing. `Deidentifier` owns all its state and is
allocation-free once built, so it is safe to call from inside an audio
callback.

```rust
use veilvoice_core::{DeidConfig, Deidentifier};

let config = DeidConfig { sample_rate: 48_000.0, ..DeidConfig::default() };
let mut deid = Deidentifier::new(config)?;

// Mono `f32` samples, nominally in [-1, 1].
let input: Vec<f32> = vec![0.0; 48_000];
let veiled = deid.process_vec(&input);

assert_eq!(veiled.len(), input.len());
```

`DeidConfig::default()` is the configuration the application ships with, and it
has accent neutralisation **on**. Two things about it are worth knowing rather
than discovering:

- **Every field is validated.** `Deidentifier::new` rejects a non-finite or
  out-of-range configuration rather than building an engine that produces
  `NaN` for the rest of the session. That was a real defect (F-10) and the
  validation is the fix, so do not bypass it by constructing state directly.
- **The engine keeps persistent state.** The accent neutraliser's long-term
  spectrum is an exponential moving average, so a single bad input sample used
  to poison every later output. Input is sanitised in the engine now, but the
  general point stands: one `Deidentifier` per stream, and do not reuse one
  across unrelated audio if you want the transform to settle honestly.

## 2. Read a file, veil it, write it back

```rust
use veilvoice_audio::io;
use veilvoice_core::{DeidConfig, Deidentifier};

let audio = io::load(std::path::Path::new("interview.mp3"))?;

let config = DeidConfig { sample_rate: audio.sample_rate as f32, ..DeidConfig::default() };
let mut deid = Deidentifier::new(config)?;
let veiled = veilvoice_audio::io::Audio {
    samples: deid.process_vec(&audio.samples),
    sample_rate: audio.sample_rate,
};

let wav: Vec<u8> = io::wav_bytes(&veiled)?;
std::fs::write("clean.wav", &wav)?;
```

`io::load` decodes anything `symphonia` handles and gives you mono `f32`. It
refuses a file that would decode to more than about twelve hours at 48 kHz
rather than exhausting memory, and it pre-flights the header, because a WAV
declaring a sample rate of zero used to kill the process outright (F-9).

Take the sample rate **from the file**. Feeding 48 kHz samples through an
engine configured for 44.1 kHz does not fail; it just shifts everything, which
is worse than failing.

## 3. Seal it, rather than writing a bare WAV

The words survive de-identification on purpose, so an unencrypted result is
still a recording of everything that was said. The container is the same one
the application uses.

```rust
use veilvoice_crypto::{container, kdf::KdfParams};

let sealed: Vec<u8> =
    container::seal_with_password(b"correct horse battery staple", &wav, KdfParams::default())?;
std::fs::write(container::veil_path(std::path::Path::new("clean.wav")), &sealed)?;

// ... and back again
let plain: Vec<u8> = container::open_with_password(b"correct horse battery staple", &sealed)?;
assert_eq!(plain, wav);
```

Full runnable version: `cargo run -p veilvoice-crypto --example seal_and_open`.

Argon2id with the RFC 9106 profile, XChaCha20-Poly1305 payload, and a header
authenticated as associated data so nobody can downgrade the KDF cost without
the open failing. The cost parameters travel *in* the file, which is what lets
an old container still open after the defaults rise — and is why they are
bounded on parse rather than trusted (F-2, F-3, F-20).

For a recipient you cannot share a password with, `seal_to_public_key` uses
X25519 **and** ML-KEM-768 together: an attacker must break both, and a
recording captured today is not readable by a quantum adversary later.

## 4. Handle a passphrase without leaving it in memory

If you prompt for a passphrase yourself, put it somewhere that gets wiped.

```rust
use veilvoice_crypto::Secret;

let typed = String::from("correct horse battery staple");
let mut buffer = typed.into_bytes();
let secret = Secret::new(&mut buffer);   // `buffer` is zeroed by `new`

// `secret` is page-locked where the OS allows it, zeroized on drop, and its
// Debug impl prints nothing. Ask whether locking actually worked:
if !secret.is_locked() {
    eprintln!("this passphrase may be written to swap");
}

// Reading it is deliberately called `expose`, so the moment is visible at the
// call site rather than looking like any other getter.
let key_material: &[u8] = secret.expose();
```

`is_locked()` reports rather than assumes, because page locking genuinely fails
on some systems and a library that pretends otherwise is worse than one that
does not try. Locking does not survive hibernation, and that is stated rather
than glossed.

What this does **not** do is wipe the original `String`'s buffer: that needs
`unsafe`, and every crate here carries `#![forbid(unsafe_code)]`. The residue
is audit item **A-5**, recorded rather than papered over. Shrinking the window
from "until the program exits" to "while the user was typing" is the part that
was worth doing.

## 5. Strip metadata

```rust
use veilvoice_meta::{clean_audio_file, clean_image_file};

clean_audio_file(std::path::Path::new("clean.wav"))?;   // tags, including ID3 in WAV
clean_image_file(std::path::Path::new("photo.jpg"))?;   // EXIF, GPS
```

A de-identified voice is worth little if the file still records who made it,
where and on what. WAV needs a chunk-level cleaner because `lofty` cannot
remove ID3v2 from RIFF at all — see `wav.rs`.

## 6. Check whether files have been tampered with

```rust
use veilvoice_guard::Manifest;

let manifest = Manifest::of(&["veilvoice", "veilvoice-gui"])?;
std::fs::write("veilvoice.manifest", manifest.to_string())?;

// later
let recorded = Manifest::parse(&std::fs::read_to_string("veilvoice.manifest")?)?;
let report = recorded.check::<&str>(&[]);
// `Report` distinguishes modified, removed and added, so a caller can treat a
// new file differently from a changed one.
eprintln!("{report:?}");
```

This **detects**, it does not prevent, and it says so everywhere it appears.
Anything that can modify the files can modify the manifest beside them; the
value is in noticing, not in stopping.

## 7. See what is holding the microphone

```rust
use veilvoice_watch as watch;

match watch::support() {
    watch::Support::Yes => {
        for user in watch::current()? {
            println!("{} is using the {:?}", user.process, user.device);
        }
    }
    // Reported honestly rather than as an empty list: an empty list from a
    // blind monitor is a false reassurance.
    other => println!("cannot see on this platform: {other:?}"),
}
```

On Linux this sees only your own processes, because `/proc/<pid>/fd` is
readable by the owner and root. That is a kernel boundary, not a bug, and
`support()` says so rather than letting an empty list imply an empty machine.

---

## Things that will bite you

Collected from the audit rather than from theory. Each of these was a real
defect in this codebase, so they are the mistakes most available to a caller.

**Build with overflow checks on while you develop.** VeilVoice's release
profile sets `overflow-checks = false`, which is why one shipped arithmetic
overflow was invisible in release and obvious in debug. If you consume these
crates as libraries, your profile is yours: leave the checks on.

**`panic = "abort"` is a choice you inherit if you make it.** VeilVoice sets it
for its own binaries. A decoder panic in a format VeilVoice does not itself
parse cannot be caught under it — no wrapper can, short of decoding in a
separate process. If your program must survive a hostile input file, do not set
`panic = "abort"`, or decode out of process.

**Do not construct configurations field by field and skip validation.**
`DeidConfig::checked()` is the single funnel, and `NaN` compares false against
every bound — which is exactly how an unvalidated `NaN` sample rate produced a
whole session of silent `NaN` output.

**One `Deidentifier` per stream.** It is stateful by design; the accent
neutraliser's memory is what makes the transform coherent over time.

**The two passwords are different secrets.** The app-lock verifier and the
container passphrase are domain-separated in the KDF. Never derive one from the
other, and never let unlocking an application unseal recordings.

---

## What these crates will never do

- **Reach the network.** There is no networking code, and CI fails the build if
  an HTTP client enters the dependency graph. If you add one, that is yours.
- **Hide what was said.** De-identification is not encryption. The words are
  preserved deliberately and can be transcribed.
- **Remove a strong regional accent entirely.** Melody and colour go; which
  phonemes you produced cannot be changed at the signal level.
- **Guarantee an erase on flash storage.** `shred_file` overwrites and unlinks,
  and the report says what that is and is not worth.

The full argument is in [WHITEPAPER.md](WHITEPAPER.md), and every limit above is
stated there too, at greater length. If you build something on these crates,
please do not describe it as doing more than they do — several tests in this
repository exist purely to fail the build if that wording softens here.
