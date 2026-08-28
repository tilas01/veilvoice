<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice — user guide

For people using VeilVoice, rather than reading its source. If you want the
argument for *why* any of this works, that is
[`WHITEPAPER.md`](WHITEPAPER.md); if you want to know what has and has not been
checked, that is [`AUDIT.md`](AUDIT.md).

There is a web version of this material at
[tilas01.github.io/veilvoice/wiki.html](https://tilas01.github.io/veilvoice/wiki.html).

---

## 1. What VeilVoice is for, in one paragraph

It destroys the **biometric voiceprint** of a speaker — pitch, formants, timbre,
micro-timing and the melody of an accent — so that neither software nor a human
listener can re-identify them, **while the words stay clean and transcribable**.
The words surviving is the point, not a compromise: a scrambler you cannot
understand is useless. It follows that de-identification alone does not keep the
*message* secret, which is why VeilVoice also encrypts what it writes.

---

## 2. Installing

Download an archive from
[Releases](https://github.com/tilas01/veilvoice/releases), or build it — a fresh
clone needs no secrets:

```bash
git clone https://github.com/tilas01/veilvoice && cd veilvoice
cargo build --release
```

**Verify the download before running it.** Instructions are in
[`REPRODUCIBLE_BUILDS.md`](REPRODUCIBLE_BUILDS.md) and on the site; there is also
an in-browser hash verifier that uploads nothing.

Nothing installs a service, writes to a registry, or phones home. Delete the
folder and it is gone.

---

## 3. The desktop app

`veilvoice-gui`. Five tabs.

### anonymise file

Choose a recording, press **anonymise**.

| Control | Effect |
|---|---|
| **intensity** | How far pitch and formants move from the original, 0.0–1.0. Default 1.0 — full normalisation. |
| **neutralise accent and intonation** | On by default. Collapses every speaker onto one canonical register and vocal tract. Turning it off is weaker de-identification. |
| **seed roll (s)** | How often the modulation stream ratchets forward. Default 2 s; 0 keeps one stream for the session. Inaudible by construction. |
| **strip metadata from the result** | On by default. |
| **encrypt the result at rest** | On by default. See below. |

### At-rest encryption

The result is **sealed as it is written**, so a file you name `clean.wav` lands
as `clean.wav.veil`. Two ways to seal it:

- **passphrase** — Argon2id at 256 MiB. Set once and held for the session;
  **change** clears it, and locking the app clears it too.
- **public key** — X25519 + ML-KEM-768 hybrid, to a `.pub` file from
  `veilvoice keygen`. Nothing to type and nothing to forget; only the matching
  private key opens it.

The **anonymise** button stays disabled until there is something to encrypt
with. A tool that quietly wrote plaintext because a field was still empty would
make the default worthless.

Unticking the box opens a dialogue that must be answered first. The result is
still a recording of every word that was said, and on flash storage deleting it
afterwards is not a reliable fix — so the question is asked once, plainly.

### live scramble

Pick an input and an output device and press **start**. A virtual audio cable is
preselected as the output when one is installed, because routing there is what
lets other applications hear the veiled voice; if none is found you are warned
rather than silently sent to the speakers. Levels, processing time per block,
engine latency and a glitch counter are shown live.

**Hear yourself first.** Beside **start** there is **preview to my headphones**.
It runs the same engine and sends the result to this machine's own output rather
than to the cable, so you hear the veiled voice and nobody else does. Use it
before an interview begins rather than during one. Use headphones while you do:
speakers plus a microphone is a feedback loop.

While a preview is running the interface says **preview** in yellow rather than
**live** in green, everywhere it says anything, because somebody who has those
two the wrong way round is either speaking to a call in their own voice or
speaking to nobody.

**The monitor follows you.** While a session is running, a strip along the
bottom of the window shows the level going in and the level coming out, on every
tab. It is on by default, because the moment you want it is the moment you are
setting up an interview on another tab and are not sure the microphone is still
working. Settings, under *the live monitor*, moves it to a floating card in the
corner or switches it off; the live tab keeps its full meters either way.

**What the meters can and cannot tell you.** They say sound is arriving and
sound is leaving, which is the thing that usually goes wrong: a muted
microphone, the wrong device, a cable nothing is listening to. They cannot tell
you the voice has been changed. A working meter and a bypassed engine draw the
same bar. The check for that is listening to the preview and hearing a voice
that is not yours.

### recording an interview

Group mode is about a recording that already exists, so the steps for an
interview are:

1. **Set up and check first.** Choose the microphone, press **preview to my
   headphones**, and listen. This is where you find out that the wrong device
   was selected, or that you are too close to the microphone and clipping.
2. **Start live scramble** into the virtual cable, and point whatever is
   recording or calling at that cable rather than at the microphone.
3. **Watch the strip.** It stays on screen while you work on other tabs. `in`
   moving and `out` flat means the engine has stopped or the cable has gone;
   `CLIPPED` means the input is too loud and is being cut off, which cannot be
   undone afterwards.
4. **Afterwards**, if the recording has several people in it and you want each
   one given a different voice, that is the **Group** tab and it works on the
   file.

### monitor

Which applications are holding your microphone and camera, with a log of starts
and stops, and an indicator in the header on every tab. On a platform that
cannot see this — macOS exposes no public interface — the tab says so. An empty
list from a blind monitor is a false reassurance and is never shown as good
news.

### lock

Set, change or remove the app lock, and lock immediately. See §5.

### about

Crate versions, licence, the typeface in use, and a plain statement of what
VeilVoice protects and what it does not.

---

## 4. The command line

`veilvoice`. Everything the app does, over SSH, in a container, or on a machine
with no GUI toolkit at all.

```bash
veilvoice anonymise interview.mp3 -o clean.wav    # writes clean.wav.veil
veilvoice anonymise interview.mp3 --encrypt-to friend.pub
veilvoice anonymise interview.mp3 --encrypt false # warns, then asks
veilvoice decrypt clean.wav.veil -o clean.wav

veilvoice live --output "CABLE Input (VB-Audio Virtual Cable)"
veilvoice devices

veilvoice clean photo.jpg                         # EXIF, GPS, tags
veilvoice encrypt notes.wav
veilvoice keygen
veilvoice lock set
veilvoice watch                                   # who is using the mic/camera
veilvoice shred secret.wav                        # irreversible
veilvoice info
```

Every command takes `--help`.

### Flags worth knowing

| Flag | Effect |
|---|---|
| `--intensity 0.0–1.0` | How far pitch and formants move. Default 1.0. |
| `--keep-accent` | Leaves intonation, accent and vocal tract intact. Weaker; use only if you know why. |
| `--reseed-secs N` | Seed roll interval. 0 keeps one stream for the session. |
| `--preview` | On `live`: sends the veiled voice to this machine's own output rather than to a virtual cable, so you hear it and nothing else does. Use headphones. |
| `--no-monitor` | On `live`: does not draw the level meters. For a terminal that is being logged or read by something other than a person. |
| `--clean-metadata false` | Keeps tags on the written file. On by default. |
| `--encrypt false` | Writes the recording in the clear. On by default; prints what you are giving up and waits for you to type `UNENCRYPTED`. |
| `--encrypt-to key.pub` | Seals to a recipient's hybrid public key instead of a passphrase. |
| `--yes` | Skips that confirmation, for scripts that already mean it. |

### Where did my WAV go?

`anonymise` seals its output, so `-o clean.wav` produces `clean.wav.veil`. Open
it with:

```bash
veilvoice decrypt clean.wav.veil -o clean.wav
```

If you genuinely want a bare WAV, `--encrypt false` still does that. It will tell
you what that costs first.

---

## 5. The app lock

VeilVoice can sit behind a password of its own, so someone who picks up your
unlocked computer cannot open it, see which files you have processed, or start a
live scramble.

```bash
veilvoice lock set        # choose a password
veilvoice lock status     # is one set, and is it rate limited right now?
veilvoice lock change
veilvoice lock remove
```

The desktop app has the same controls under its **lock** tab, plus a **lock**
button in the header that locks immediately and clears the session passphrase.

`--path` puts the lock file somewhere other than this platform's config
directory.

### Read this part

**The app lock is not tamper-proof, and cannot be.** A program running on your
computer has nowhere to hide a secret from that computer. Anyone who can write
to your files can delete the lock file; anyone holding the disk can edit the
attempt counter, move the clock to defeat the wait, or attack the stored
password hash offline.

What it does buy is real:

- It stops **casual access** — the person who sits down at your unlocked
  session. That is a common threat, and a genuine one.
- Three attempts are free, then the wait doubles — 5 s, 10 s, 20 s, up to
  fifteen minutes — and the count is **written to disk**, so killing the app
  does not reset it.
- Argon2id at 256 MiB makes each offline guess expensive. That helps a good
  passphrase and does not save a bad one.

If someone taking your disk is the threat, the answers are full-volume
encryption (LUKS, BitLocker, FileVault) and the at-rest encryption above. Not
this.

### Use two different passwords

The app lock and the recording passphrase are deliberately separate secrets. If
one password did both, opening the app would be the same act as unsealing
everything it had ever written. VeilVoice keeps the two derivations domain
separated, so typing the same passphrase in both places still does not produce
two copies of one value — but one guess would then open both, so do not.

### If you forget the app-lock password

Delete the lock file. That is not a backdoor: it is the same thing any attacker
with access to your files could do, which is exactly why the lock is described
as protecting against casual access rather than as a security boundary. The
unlock screen shows the file's path.

Forgetting a **recording** passphrase is different. There is no recovery, by
design.

---

## 6. Things VeilVoice will not do

Read this twice. Misunderstanding it is the only way this software gets someone
hurt.

- **It does not hide what you said.** The words are preserved on purpose and can
  be transcribed. Encrypt the file — which is now the default — if the content
  must stay secret.
- **It does not fully remove a strong accent.** Its melody and colour go; which
  phonemes you actually produced cannot be changed by any filter.
- **It does not sanitise the background** — room acoustics, other voices, a
  passing siren.
- **It does not hide your speaking rate** or rhythm. A weak biometric, but a
  real one.
- **It does not help against an attacker already running code on your machine.**
- **It does not clean the filename**, the filesystem timestamps, or the channel
  you send the file over.
- **The app lock is not tamper-proof**, as above.
- **Secure erase is not reliable on flash storage.** `shred` overwrites in
  place, which works on a spinning disk; wear levelling on an SSD, SD card or
  USB stick leaves the original blocks where no software can reach them. Full
  volume encryption is the answer that works.

---

## 7. Getting help, and checking for yourself

Nothing here asks for trust. The properties above are asserted by the test
suite:

```bash
cargo test --workspace
cargo run -p veilvoice-core --example spectrum_report
```

The code has been **audited by tilas01**, who wrote it. That is a maintainer
audit and is worth what a maintainer audit is worth: it catches what the author
can see. **No external firm or independent researcher has reviewed this code.**
Until one has, the strongest verification available to you is the source — which
is written to be read.
