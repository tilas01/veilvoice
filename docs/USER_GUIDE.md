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

`--path` puts the lock in one named file instead, somewhere of your choosing.
Without it, the lock goes where §5.2 describes.

### 5.1 Read this part

**The app lock is not tamper-proof, and cannot be.** A program running on your
computer has nowhere to hide a secret from that computer. Anyone holding the
disk can attack the stored password hash offline, and given enough access can
still remove the lock entirely.

What it does buy is real, and it is worth being precise about which parts are
which, because two of the four things below are speed bumps and two are not.

**Real, and the reason the lock exists:**

- It stops **casual access** — the person who sits down at your unlocked
  session. That is a common threat, and a genuine one.
- Three attempts are free, then the wait doubles — 5 s, 10 s, 20 s, up to
  fifteen minutes — and the count is **written to disk**, so killing the app
  does not reset it. Somebody who edits the file directly still defeats this;
  see §5.3.
- Argon2id at 256 MiB makes each offline guess expensive. That helps a good
  passphrase and does not save a bad one.

**Real, and new:**

- Each stored lock carries an **authentication tag** computed with a key that
  exists only while your correct passphrase is in memory. If somebody swaps the
  stored password for one of their own, or weakens the Argon2id cost so a guess
  becomes cheap, the next time you unlock, VeilVoice tells you. The report is
  written down, so it survives a restart, and clearing it asks for your
  passphrase, so the person who caused it cannot dismiss it.
- The lock is kept in **two copies**, in two directories. Deleting one does not
  remove the lock: the other puts it back, and you are told it happened.
- On Linux and macOS, when VeilVoice is run with administrator rights, the
  second copy is written under `/etc/veilvoice` and is thereafter not writable
  by an ordinary user. Removing the lock then needs `sudo`. VeilVoice never
  asks for that privilege and never elevates itself; it uses what it already
  has. On Windows the equivalent needs an access-control list VeilVoice does
  not set, so there the second copy is a second copy and nothing more.

**Not real, and not counted as security anywhere:**

- The two files have **unguessable names** and their contents are **masked**,
  so a search of your disk for the string `VEILLOK1` finds nothing and a backup
  rule written against `applock.bin` misses. The names come from a value in an
  index file at an obvious path, because something has to be findable or
  VeilVoice could never open its own lock again. Anybody who reads that index,
  or reads the source, recomputes both names in a second. This is obscurity. It
  makes careless deletion and casual searching harder and it stops nobody who
  is paying attention.

If someone taking your disk is the threat, the answers are full-volume
encryption (LUKS, BitLocker, FileVault) and the at-rest encryption above. Not
this.

### 5.2 Where the lock is kept

In your platform's configuration directory: `%APPDATA%\veilvoice` on Windows,
`~/Library/Application Support/veilvoice` on macOS, `$XDG_CONFIG_HOME/veilvoice`
or `~/.config/veilvoice` on Linux.

Inside it you will find `applock.index`, which is sixteen random bytes, and two
files whose names are derived from it. The second copy is in the same directory
unless VeilVoice was run with administrator rights, in which case it is under
`/etc/veilvoice`.

`veilvoice lock status` prints the directory rather than the file names, since
the names carry no meaning for a reader. If you delete `applock.index`, both
copies become unreachable and the lock is gone. Treat that file as part of the
lock rather than as scratch.

### 5.3 What the tag does not cover

The failed-attempt counter and its timestamp sit **outside** the authentication
tag, and the reason is unavoidable rather than an oversight: they are written at
the one moment the tag key does not exist. A wrong passphrase has to be counted,
counting it means writing the file, and that write cannot be authenticated by a
key only a right passphrase produces. Putting them inside would mean either
reporting every honest typo as tampering or not counting failures at all.

So the rate limit is exactly as defeatable by a text editor as it always was.
The tag covers the parts an attacker actually wants to change: the stored
password, the Argon2id cost, and the tamper report itself.

Two other things it does not cover. Replacing the lock wholesale with one the
attacker created is not detected, because their record is authentic under their
own passphrase. The second copy is what stands in the way of that, not the tag.
And restoring an older copy of your own lock file, to wind the report back, is
not detected either.

### 5.4 Two passwords by default, and one if you choose it

The app lock and the recording passphrase are separate secrets by default. If
one password did both, opening the app would be the same act as unsealing
everything it had ever written. VeilVoice keeps the two derivations domain
separated, so typing the same passphrase in both places still does not produce
two copies of one value, though one guess would then open both.

**You can now choose to have one.** On the security tab, under how recordings
are sealed, there is a third option beside *passphrase* and *public key*: **app
lock**. With it on, every recording VeilVoice writes is sealed with your
app-lock password automatically, with nothing else to set up and nothing else
to remember. The choice is remembered between launches.

Read this before turning it on:

- **One password now opens the application and everything it has ever
  written.** Somebody who makes you unlock VeilVoice in front of them has
  opened the archive, not just the session. That is the entire cost, and it is
  the reason the two are separate by default.
- **Forgetting that password loses the recordings**, not just a session.
  Without this on, forgetting the app-lock password costs you the lock and the
  fix is deleting it. With it on, deleting the lock does not help: the
  recordings are encrypted, and there is no recovery.
- **The recordings do not depend on the lock file.** Each file carries its own
  salt and cost, so `veilvoice decrypt` opens it with the same password on any
  machine, with or without a lock. Removing the lock does not lock you out of
  anything.
- **It takes effect at the next unlock.** The password is taken as the lock
  opens, because that is the only moment it exists. Turning the option on
  mid-session tells you to lock and unlock again, rather than quietly writing
  the next recording unencrypted.

Only offered when an app lock is set, because there is nothing to seal with
otherwise.

### 5.5 If you forget the app-lock password

Delete `applock.index` and the two files beside it, in the directory §5.2 names.
That is not a backdoor: it is the same thing anyone with access to your files
could do, which is exactly why the lock is described as protecting against
casual access rather than as a security boundary.

If the second copy was written under `/etc/veilvoice`, removing it needs `sudo`.

The unlock screen does not show any of this. It says the app is locked and asks
for the passphrase, and nothing else. The person reading a locked window is
either its owner, who does not need the file's location at that moment, or
somebody who picked the machine up, who should not be handed it at all.

Forgetting a **recording** passphrase is different. There is no recovery, by
design.

---

## 5.5 Locking the window when you walk away

Off unless you turn it on, under **Settings, Locking**.

When it is on, choose how long from the list, which runs from five minutes to
two days, or type your own: `90m`, `2h`, `1d`. Typing a value outside the list
widens the list to hold it, so the range is yours rather than ours. There is a
button to put it back.

**Starting a long job does not count as using the window.** That is deliberate,
and it is the case the feature exists for: somebody who starts a render and
leaves the room has left the room, and the recording being produced is the thing
worth locking away.

The countdown runs on the window's own clock, not the system one, so changing
the machine's time neither brings the lock forward nor pushes it back.

## 5.6 VeilVoice checking its own files

The first time VeilVoice runs it writes down what its own program file looks
like: the size and a SHA-256. Every launch after that it checks the file against
that record and shows the answer on the security tab. Nothing has to be turned
on and there is no command to remember, which was the whole problem with
`veilvoice guard init` being a command.

**With an app lock set**, the record is sealed under your app-lock passphrase,
and the check runs at the moment you unlock, because that is the one moment the
passphrase exists. Somebody who changes the program file then has to change the
record too, and to do that they need your passphrase.

**With no app lock**, there is no passphrase to seal it with, so the record is
written in the clear and the security tab says so in those words. It still
catches a file that changed by accident, a half-finished update, or a careless
overwrite. It does not catch somebody who thought to rewrite the record as
well, and a record sealed under a key kept beside it would be a decoration
rather than a protection.

Either way this **detects**; it does not prevent. And it cannot tell an update
you installed from a file somebody swapped, because on disk those look
identical. If you have just updated, a report of a change is the update.

The record is taken at first launch rather than at install, because installing a
package runs as an administrator and the record belongs to the user account that
will run the program. A record written into the administrator's own
configuration directory would describe nothing anybody checks.

`veilvoice guard init`, `check` and `status` still work, read the same record,
and can watch more files than the window does. See §4.

## 5.7 Saving into a Cryptomator vault or a VeraCrypt volume

VeilVoice can write every veiled recording straight into an encrypted folder you
already have, instead of leaving it beside the original. The security tab has a
section called **Where recordings go**.

It looks for a mounted Cryptomator vault or VeraCrypt volume and offers what it
finds. If it finds nothing, point at the folder by hand and say which of the two
it belongs to; a folder you chose yourself is treated exactly like one that was
found, including the question below.

**VeilVoice never opens or closes these for you, and never asks for their
password.** Unlock the volume in its own program first. Mounting your encrypted
storage is your act, taken in the tool you chose, and a voice de-identifier is
not the program to be doing it on your behalf.

### The hidden-volume question

If you choose a **VeraCrypt** volume, VeilVoice asks one question before it will
write anything, and will not start a job until you answer:

> Does this container have a hidden volume inside it?

It asks because it cannot tell, and neither can anything else. A VeraCrypt
container can hold a second volume inside the free space of the first, so that
somebody forced to hand over a password can open the outer one truthfully while
the inner one stays unprovable. That only works because the two are
indistinguishable from outside.

The danger is specific. **Writing into the outer volume of a container that has
a hidden one can destroy the hidden data**, because the outer filesystem does
not know the inner one is there and will allocate over it. VeraCrypt has a
protection mode for this and it needs the hidden volume's password, which
VeilVoice does not have and will not ask for.

So there are three answers and they do different things:

| Answer | What happens |
|---|---|
| No hidden volume | VeilVoice writes there |
| This is the hidden one | VeilVoice writes there; you are already inside the hidden volume, which is safe |
| This is the outer one | VeilVoice refuses, and says why |

Cryptomator is not asked, because it has no such concept and a question with no
meaning only teaches people to click through questions.

A destination you have not answered for **blocks the job**. It does not quietly
fall back to writing beside the original, because a recording sitting outside a
vault while you believe it is inside one is exactly what this is here to
prevent. If the volume is locked when you come to use it, VeilVoice says so
rather than writing into the empty mount point.

### 5.8 Encrypt the disk as well

An encrypted volume protects the files inside it. It does not protect the
temporary files, swap or hibernation image, thumbnails or recently-opened lists
your system writes about them, and any of those can outlive the recording.

Encrypt the whole disk too:

| System | Use |
|---|---|
| Windows | BitLocker |
| macOS | FileVault |
| Linux | LUKS or LUKS2 |
| OpenBSD | `softraid -C` |
| FreeBSD | GELI |

This is defence in depth, not a second lock on the same door. The volume
protects the file; the disk protects everything the system wrote about the file
without being asked. A veiled recording inside a Cryptomator vault on an
encrypted disk is encrypted by two independent tools, and what that buys is not
extra strength so much as independence: a defect in one is not a defect in both.

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
