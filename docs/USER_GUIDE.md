<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice: user guide

For people using VeilVoice, rather than reading its source. If you want the
argument for *why* any of this works, that is
[`WHITEPAPER.md`](WHITEPAPER.md); if you want to know what has and has not been
checked, that is [`AUDIT.md`](AUDIT.md).

There is a web version of this material at
[tilas01.github.io/veilvoice/wiki.html](https://tilas01.github.io/veilvoice/wiki.html).

---

## 1. What VeilVoice is for, in one paragraph

It destroys the **biometric voiceprint** of a speaker, meaning pitch, formants,
timbre, micro-timing and the melody of an accent, so that neither software nor a human
listener can re-identify them, **while the words stay clean and transcribable**.
The words surviving is the point, not a compromise: a scrambler you cannot
understand is useless. It follows that de-identification alone does not keep the
*message* secret, which is why VeilVoice also encrypts what it writes.

---

## 2. Installing

Download an archive from
[Releases](https://github.com/tilas01/veilvoice/releases), or build it, since a fresh
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

## 2.5 The two programs, and which one you want

A release contains two executables. They overlap on purpose, and which one to
reach for depends only on what you have in front of you.

| Program | What it is for |
|---|---|
| `veilvoice` | The command line. Everything the application does, over SSH, in a container, in a script, or on a machine with no graphics toolkit at all. Checking a download is `veilvoice verify`, which was a third binary until 0.1.18. |
| `veilvoice-gui` | The window. The same engine with somewhere to click, plus the things that only make sense with a screen: live level meters, the app lock, the microphone monitor, and a Verify tab that runs the same check as `veilvoice verify`. |

### Which parts are built in, and which are not

Everything VeilVoice does is in these binaries. There is no runtime to install,
no service, no plugin directory, and nothing is downloaded on first run.

That includes the parts people expect to be separate:

- **The signature check.** The signing key is compiled into the programs, and
  the OpenPGP verification is Rust code in this repository. `veilvoice verify`
  needs no GnuPG to do its job.
- **The audio decoders**, the resampler, the encryption, the key exchange, the
  hashing. All of it is in the binary.
- **The at-rest encryption**, including the post-quantum half.

Three things are genuinely outside, and each is optional:

- **GnuPG**, for a second opinion on a release signature. Worth having, and
  explained under §7.
- **A virtual audio cable**, if you want live mode to feed a call. On Linux
  this is usually PipeWire, which is already there.
- **`ffmpeg`**, only if you ask for a video file. Without it, the command
  prints exactly what it would have run and exits successfully, because
  nothing failed.

`veilvoice companions` lists all of them, says whether this machine has each,
and prints the one command that would install it. It never runs somebody
else's installer.

### What runs where

Eleven platforms get a signed archive with every release, and the table says what
is in each one. It is not the same everywhere, and where it is not, that is a
limit of what the platform offers rather than something waiting to be written.

| Platform | Command line | Desktop app | Live microphone |
|---|---|---|---|
| Windows 10 and 11, x86-64 | yes | yes | yes, with a virtual cable |
| macOS on Intel | yes | yes | yes, with a virtual cable |
| macOS on Apple Silicon | yes | yes | yes, with a virtual cable |
| Linux, x86-64 and arm64 | yes | yes | yes, through PipeWire |
| Linux, statically linked (musl) | yes | yes | yes |
| Raspberry Pi and other armv7 | yes | yes | yes |
| WSL on Windows | yes | yes, with WSLg | through the Windows side |
| FreeBSD, OpenBSD, NetBSD | yes | not shipped | no |

**Any Linux distribution.** The `.deb` and `.rpm` are conveniences, not
requirements: the plain archive is a folder of binaries that needs no package
manager, and the statically linked build needs no system libraries at all,
which is the one to reach for on a distribution nothing else fits.

**One library the desktop app needs, and why it is worth a paragraph.** The
window toolkit opens `libxkbcommon-x11` by name when it starts, rather than
linking against it. Nothing that works out dependencies by reading a binary can
see that, so a minimal or server install can be missing it and the application
will exit at once instead of drawing a window. If that happens, the crash report
VeilVoice writes names the library and the package that carries it; the short
version is `libxkbcommon-x11-0` on Debian and Ubuntu and `libxkbcommon-x11`
elsewhere. The command line needs none of it.

**The first time you open it.** After the two settings questions, VeilVoice
shows one card per tab saying what that tab is for, which takes about twenty
seconds and can be skipped at any point. Two of the nine are worth the card on
their own: Monitor is not a level meter, it watches for another program picking
up a real microphone while you are being veiled; and Lock is a passphrase on
the application rather than on a recording.

The last card says whether this copy is **portable** or **installed**, in those
words. Portable means it runs from wherever you put it and installs nothing:
move the folder and VeilVoice moves with it, delete the folder and it is gone.
Installed means it is on this machine for good, on your menu or path, with its
settings in your account. Both are fine, and the Install tab is where the
decision is made rather than in the tour.

After an upgrade the tour comes back only for tabs that did not exist last
time, and a release that adds no tab shows nothing. What is stored is the list
of tabs you have been shown, which is what "which of these is new to you" is
actually asking.

**When something goes wrong.** VeilVoice writes a report of a crash to a file
beside its settings, and on the next launch it offers it to you above whatever
tab you land on: what happened, where the file is, and a button to read the
whole of it before you decide anything.

Nothing is sent. Nothing here *can* send it, and that is not a policy but a
property of the build: this project contains no network client and the build
fails if one enters the dependency graph. The ordinary shape of this feature is
a reporter that uploads, and that is the wrong shape for a program people use
to protect themselves, because a report from a privacy tool is a report about
somebody who was being careful.

So the panel offers two things instead: copy the report, and open the issue
tracker. What happens next is your decision and your clipboard. If you would
rather it went away, "dismiss and delete it" removes the file.

The report holds the version, your operating system and processor, and the
error with its source location. It holds no file names, no settings, no
passphrase and nothing about any audio. That list is in the panel too, because
"would you like to send this" is only a real question if you can see what
"this" is.

**The BSDs get the command line only, and the reason is specific.** The audio
library VeilVoice uses has no backend for them, so live capture cannot work
there and the desktop application is built around a window that would have
nothing to listen to. Everything that operates on a file, meaning
de-identification, encryption, metadata cleaning and verification, is pure Rust
and runs exactly as it does anywhere else.

**WSL is Linux**, so the command line runs unchanged. The window needs WSLg,
which recent Windows has by default. A microphone belongs to Windows rather
than to the distribution, so live mode is the Windows build's job.

**Nothing is emulated and nothing is a wrapper.** Every archive is a native
build for that processor, compiled from the same source with the same pinned
compiler, and built twice in separate directories and compared byte for byte
before it ships.

### How anything reaches the network, given that nothing here is a network client

VeilVoice bundles no HTTP client, and this is checked rather than claimed:
nothing in the workspace links one. Two features nonetheless involve the
network, and the way they do it is the point.

**Check for updates**, in the desktop application only, asks the operating
system's own transfer tool to fetch one small file, and reads a version number
out of what it printed. It is a button, it is never automatic, and the command
line has no such feature at all.

The tool is found by **absolute path**, never through `PATH`. On Windows that
is `%SystemRoot%\System32\curl.exe`, which has shipped with Windows since
2018. Elsewhere it is `curl` at `/usr/bin`, `/bin` or `/usr/local/bin`, and
`wget` at the same three places if there is no `curl`.

That distinction is not fussiness. Windows searches the current directory
before `PATH`, so a file called `curl.exe` sitting beside VeilVoice would
otherwise be the program that ran, and a privacy tool reaching for the network
is the last place to accept a stranger's binary. If none of those paths holds a
tool, the button says so and nothing is run.

**Installing a companion** does not fetch anything either. It runs the package
manager already on the machine, which is the thing your system already trusts
to install software, and for anything needing root it prints the command
instead of running it.

The consequence worth stating: there is no code path in VeilVoice that opens a
socket. A firewall rule that blocks it entirely costs you the update button and
nothing else.

---

## 3. The desktop app

`veilvoice-gui`. One tab for each thing it does, in the strip across the
top. Every one of them has a section below, and `veilvoice-gui --tab <name>`
opens the window on one directly.

### anonymise file

Choose a recording, press **anonymise**.

| Control | Effect |
|---|---|
| **intensity** | How far pitch and formants move from the original, 0.0–1.0. Default 1.0, full normalisation. |
| **neutralise accent and intonation** | On by default. Collapses every speaker onto one canonical register and vocal tract. Turning it off is weaker de-identification. |
| **seed roll (s)** | How often the modulation stream ratchets forward. Default 2 s; 0 keeps one stream for the session. Inaudible by construction. |
| **strip metadata from the result** | On by default. |
| **encrypt the result at rest** | On by default. See below. |

### At-rest encryption

The result is **sealed as it is written**, so a file you name `clean.wav` lands
as `clean.wav.veil`. Two ways to seal it:

- **passphrase**: Argon2id at 256 MiB. Set once and held for the session;
  **change** clears it, and locking the app clears it too.
- **public key**: X25519 + ML-KEM-768 hybrid, to a `.pub` file from
  `veilvoice keygen`. Nothing to type and nothing to forget; only the matching
  private key opens it.

The **anonymise** button stays disabled until there is something to encrypt
with. A tool that quietly wrote plaintext because a field was still empty would
make the default worthless.

Unticking the box opens a dialogue that must be answered first. The result is
still a recording of every word that was said, and on flash storage deleting it
afterwards is not a reliable fix, so the question is asked once, plainly.

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

### group

Several people in one recording, each given a **different** destination voice,
so a listener can still follow the conversation by ear. Every voiceprint is
destroyed as thoroughly as one speaker's would be; what is kept is that the
speakers are distinguishable, not who they are.

It works on a recording that already exists, not on a live microphone.

| Control | What it is |
|---|---|
| **How you are working** | One person, a group with a voice each, or a group with one voice for everybody. The last is the honest choice when there are more people than there are voices far enough apart to tell apart. |
| **open / save project** | A project holds where your files are, who is in the recording and what you called them. No audio and no passwords, so it is safe to keep beside the recording. It does hold the names you typed. |
| **group mode** | On for this run. Closing the window turns it off again, so a recording of one person is never rendered against a plan describing several. |
| **always start in group mode** | Remembered, for people who are always working this way. |
| **the people** | A name and a colour each. **Names are not veiled by anything**: you type them and they go into the subtitles as typed. |
| **the recording, and the plan** | The plan says when each person speaks. Without one there is nothing to render against, and audio no turn claims is silenced rather than passed through, so a missing plan gives a silent file rather than an unveiled one. `veilvoice conversation inspect` describes a plan you already have. |
| **what a render writes** | Audio, subtitles and a player page. All three by default. |

VeilVoice does not guess who is speaking. Turns come from a plan file or from
one microphone per person, and that is a deliberate limit: guessing wrongly
would put one person's words under another person's name.

### verify

Check that a download is the one that was published, without leaving the
window. This is the same check `veilvoice verify` does, and §7 walks through
it in full.

Drop the archive on the window and the hash list and signature beside it are
picked up automatically. One press then checks the signature over the hash
list, the archive against that list, every file you extracted out of it, and
all of it again through your own GnuPG if you have one.

The commands are also printed for you to run yourself. That is not decoration:
a program telling you that a download is genuine came out of that download.
Running the commands yourself is the part no program can do for you.

**What a pass proves** is written on the tab, and it is worth reading. A good
signature and a matching hash prove the file is the one the holder of that key
published. They do not prove it is safe, that the source compiles to it, or
that the key belongs to anybody in particular.

### settings

Where every choice the window remembers is made, and where they are kept.

| Page | What is on it |
|---|---|
| **Interface** | The colour scheme, which is every palette the website has. Whether the mark in the header animates, and whether the window icon does. Whether the **install** tab is shown at all. |
| **Locking** | The app lock and the idle timer that turns it on. See §5 and §5.5. |
| **At rest** | Whether a result is sealed with the app-lock password as well, and where a vault lives if you keep one. See §5.7. |
| **Notifications** | How the window tells you a job has finished. |

The file itself is plain text, one `key = value` a line, at
`%APPDATA%\veilvoice\settings.conf` on Windows,
`~/Library/Application Support/veilvoice/settings.conf` on macOS and
`${XDG_CONFIG_HOME:-~/.config}/veilvoice/settings.conf` on Linux. Nothing in it
is secret and none of it is a password.

### install

Only there when you are running a portable copy, and it removes itself once
VeilVoice is installed: a program offering to install itself when it already is
tells you something untrue about what you are running. There is a tick under
settings to hide it on a portable copy too.

The install it offers is deliberately small. It copies the VeilVoice programs
beside this one into your own program directory and adds that directory to your
PATH, so that typing `veilvoice` in a terminal works. No administrator rights
are asked for, no service is created, and nothing is written outside your own
account.

**Companion software** is listed on the same tab, and none of it is part of
VeilVoice or required by it. Each entry names one program, says who makes it
and under what licence, says whether it was found, and gives the one command
that would install it. VeilVoice never runs somebody else's installer, and
anything needing root prints the command for you to run in a terminal where you
can see what you are approving.

### monitor

Which applications are holding your microphone and camera, with a log of starts
and stops, and an indicator in the header on every tab. On a platform that
cannot see this, because macOS exposes no public interface, the tab says so. An empty
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

- It stops **casual access**, meaning the person who sits down at your unlocked
  session. That is a common threat, and a genuine one.
- Three attempts are free, then the wait doubles: 5 s, 10 s, 20 s, up to
  fifteen minutes, and the count is **written to disk**, so killing the app
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

## 5.9 An interview, start to finish

The commonest thing people ask VeilVoice to do, in the order it happens. This is
also how to veil the person you were interviewing rather than only yourself.

### Step 1: get the sound out of what you recorded

If you recorded in OBS, or anything like it, you have a `.mkv` or `.mp4`
holding a video track and an audio track. VeilVoice reads audio:

```bash
veilvoice import interview.mkv          # writes interview.wav
```

That needs `ffmpeg`. VeilVoice does not ship one and will not install one; when
it is missing you get the exact command printed, to run yourself or after
installing it. `--dry-run` prints it without running anything.

Already have a `.wav`, `.mp3`, `.flac`, `.ogg`, `.m4a`, `.aac` or `.opus`? Skip
this step; those go straight into `anonymise`.

### Step 2: write a plan, so each person gets their own voice

Running an interview through `anonymise` gives **both people the same voice**.
That is private and useless: nobody can tell a question from its answer. A plan
gives each speaker their own destination voice.

**VeilVoice will not work out who is talking.** That is speaker diarisation, it
needs a trained model, this project ships none and asks no server, and a wrong
guess would either merge two people or invent a third with nothing in the output
showing it. So you tell it. A plan is a text file:

```text
VEILCONV1
title    Interview with Sam
speaker  0  Me
speaker  1  Sam
turn  0.000   4.200  0  So, how did it go?
turn  4.100  19.050  1
turn 19.000  22.400  0  And after that?
```

Line by line:

| Line | What it is |
|---|---|
| `VEILCONV1` | The first line of every plan, so the file says what it is |
| `title` | What the recording is called, shown in the player |
| `speaker  <n>  <name>` | One per person. The number is how turns refer to them |
| `turn  <from>  <to>  <speaker>  [words]` | One per stretch of speech, in seconds |

The words on a turn are optional. With them the subtitles carry what was said;
without them they carry the speaker's name, which is still enough to follow a
conversation whose voices have all been replaced.

Overlapping turns are fine. People talk over each other, and VeilVoice mixes
them rather than picking a winner.

**Anything no turn claims is silenced, not passed through.** A gap in a plan
must never put a real voice into the result, and how much was silenced is
printed so you can tell a deliberate pause from a plan that missed a minute.

Check a plan before spending time on a render:

```bash
veilvoice conversation inspect interview.plan
```

It prints who is in it, which voice each gets, and any overlaps.

### Step 3: render it

```bash
veilvoice conversation render interview.plan interview.wav -o veiled.wav
```

Every speaker comes out with their own voice and every voiceprint is destroyed,
including the interviewee's. Subtitles are written beside the audio in both
formats, and a self-contained player page comes with it that needs nothing
installed.

**None of it is encrypted, unlike `anonymise`.** Seal the audio afterwards with
`veilvoice encrypt` if it matters. Everything a render writes is created
readable only by your account, which is a file permission and nothing more: it
does not survive a copy, a backup, or anyone who has the disk.

And read the subtitles before sending them anywhere. They carry the names you
typed and the words you typed, in plain text, and nothing veils a name.

### Step 4: a video, if you need one

Somewhere that will not accept an audio file:

```bash
veilvoice video veiled.wav              # writes veiled.mp4
```

A black picture for the length of the recording. The picture is not the point
and does not pretend to be. Needs `ffmpeg`, same as step 1.

### Recording each person on their own microphone instead

If your recording already has one channel per person, the split is exact and
there is no plan to write. That is the better arrangement whenever you can
manage it: no times to type, and no chance of typing them wrong.

## 5.10 Policies: settings somebody else decided

For a newsroom, a clinic, a legal team: one person writes down what VeilVoice
must do on every machine, and the machines hold to it.

### The one idea worth understanding first

**A policy can only make VeilVoice stricter.** There is no requirement that
turns encryption off, none that lowers the de-identification floor, none that
disables the app lock, and there is nowhere in the file to write one.

That is what makes the whole thing work without a privileged service or a key
hidden in the program. A policy has to be readable at every launch to be
applied at every launch; if reading it needed a password, you would type one
every time. So the file is plain, and the protection is in its *shape*:
somebody who edits it without the passphrase can do exactly one thing, which
is make that machine stricter than its owner asked for. A nuisance, not a
privacy failure.

The sealed copy beside it is what proves the policy in force is the one that
was written.

### Writing one

The file is `policy` in VeilVoice's own folder, and it is plain text:

```
VEILPOLICY1
note  Newsroom standard, agreed 2026-03
require  encrypt-recordings
require  clean-metadata
require  app-lock
require  minimum-intensity  60
```

Two details the file is strict about, because it refuses rather than guesses:

- **`VEILPOLICY1` on the first line.** A file without it is rejected.
- **Two spaces** between a keyword and its value, not one and not an `=`. The
  same between `minimum-intensity` and its number.

`veilvoice policy status` prints the policy in force in exactly this form, so
the quickest way to get the syntax right is to write one requirement, run it,
and copy what it prints back.

The five requirements, and each one only tightens:

| `require` | What it insists on |
| --- | --- |
| `encrypt-recordings` | Every veiled recording is encrypted before it is written. No plaintext output. |
| `clean-metadata` | Metadata is stripped from what VeilVoice writes. |
| `neutralise-accent` | Accent neutralisation is on, not optional. |
| `app-lock` | An app lock must be set. VeilVoice will not run without one. |
| `minimum-intensity  N` | The de-identification floor, as a whole number from 0 to 100. A user may go higher, never lower. |

### Sealing it, so it cannot be quietly rewritten

```bash
veilvoice policy status          # what is in force, and whether it is sealed
veilvoice policy seal            # asks for a passphrase, writes the sealed copy
veilvoice policy verify          # does the plain file still match the seal?
```

`verify` exits non-zero if the two disagree, which is what a scheduled check
should look at. Removing a policy is `veilvoice policy remove`, and it asks.

### What a sealed policy is not

It is **not enforcement**, and the distinction matters:

- Anything that can write VeilVoice's own executable can replace VeilVoice, and
  no file it reads can stop that.
- Anything running as the user can delete the policy outright.

What a seal buys is that a policy cannot be *quietly rewritten into something
weaker*. Deletion is a different question, and it has a different answer: put
the policy files into a tamper manifest with `veilvoice guard`, and their
removal shows up there.

### The honest deployment shape

1. Write the policy on one machine and seal it.
2. Copy both files, the plain one and the sealed one, to each machine.
3. Add them to that machine's tamper manifest.
4. Have something run `veilvoice policy verify` on a schedule and report
   non-zero.

None of that needs a server, and none of it needs VeilVoice to run as anything
but the person using it.

---

## 5.11 A whole session in the desktop app, start to finish

The path most people actually want, in the window rather than the terminal.

**1. Check what you downloaded.** Verify tab. Drop the archive on it, or point
it at the folder. It checks the signature over the hash list first, then the
archive against that list, then every file you extracted. Green all the way
down before anything else. Section 6.5 says what each step proves.

**2. Answer the setup.** On a first run VeilVoice asks four things: how it
should look, a password for the application, a password for your recordings,
and whether the window locks itself when you walk away. Every one can be
skipped and changed later, and the second one is worth reading rather than
clicking past: it is also what encrypts VeilVoice's own files.

**3. Pick where output goes.** Settings, or the Anonymise tab. If you keep a
Cryptomator vault or a VeraCrypt volume, point VeilVoice at it now and answer
the hidden-volume question, because it will not write anything until you have
(section 5.7).

**4. Veil the recording.** Anonymise tab: choose the file, choose an intensity,
render. With a recording password set, what lands on disk is encrypted.

**5. Listen to it.** Not optional. Play the result and satisfy yourself the
voice is not recognisable to somebody who knows the speaker. No measurement
substitutes for this.

**6. Check it back.** Verify tab again if you are sending it somewhere, and
`veilvoice guard` if you want VeilVoice to notice its own files changing.

For several speakers at once, that is the Group tab and section 5.9, which
walks through an interview from the raw file to a finished render.

---

## 5.12 If VeilVoice keeps closing on Windows

A brand-new application that few people have run yet has, in antivirus terms, a
low reputation, and a low-reputation program that reads a microphone and writes
encrypted files is the shape some scanners are built to be wary of. Occasionally
one closes VeilVoice by mistake.

VeilVoice notices this itself. If a run ends without a clean shutdown and it did
not crash on its own, and an antivirus product is installed, the next launch
shows a plain notice: which product was found, that a new app is sometimes
stopped as a precaution, that you would normally have seen an alert from that
product too, and that adding an exclusion is worth doing only if you actually
keep seeing the problem. Nothing is hidden from your antivirus and nothing on
your system is changed; it is one paragraph of context so a vanished window is
not a mystery.

Because VeilVoice is offline, reproducible and signed, you can establish that it
is genuine before excluding anything: the Verify tab, or `veilvoice verify`,
walk through it. An exclusion is your decision to make, and only worth making
once you have checked.

## 6. Things VeilVoice will not do

Read this twice. Misunderstanding it is the only way this software gets someone
hurt.

- **It does not hide what you said.** The words are preserved on purpose and can
  be transcribed. Encrypt the file, which is now the default, if the content
  must stay secret.
- **It does not fully remove a strong accent.** Its melody and colour go; which
  phonemes you actually produced cannot be changed by any filter.
- **It does not sanitise the background.** Room acoustics, other voices, a
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

## 6.5 The verifier, `veilvoice verify`

One job: deciding whether the download you just made is the one that was
published.

Until 0.1.18 this was a third binary, `veilvoice verify`, shipped beside the
other two so it was usable *before* you trusted them. That argument was always
thinner than it looked -- the separate program came out of the same archive as
everything else, so trusting it was the same act of trust -- and it cost every
user a second file to find and a third name to learn. It is part of `veilvoice`
now. The check is identical: the same code, in `veilvoice-check`, that the
desktop application's Verify tab has always called.

If you would rather not use a terminal at all, the Verify tab does the whole of
this with the same code underneath.

### Just run it

```bash
veilvoice verify
```

With no arguments it looks for a release around it: the folder you are in, the
folder above, the one the program itself is in, and Downloads and Desktop. If
it finds an archive with a `SHA256SUMS` and a signature beside it, it checks
everything without being told anything.

That is the intended way to use it. Unpack the archive, run it inside the
folder, read the verdict.

### What one press actually checks

Four things, in the order that makes each one worth checking:

1. **The signature over the hash list.** Before the hashes, always. Whoever
   could replace your download could replace `SHA256SUMS` beside it, and the
   two would agree perfectly. The signature is what makes the list worth
   comparing against, and the fingerprint is what makes the signature worth
   checking.
2. **The archive against that list.**
3. **Every file you extracted out of it**, against a signed contents list
   published with the release. An archive can be genuine and a file inside your
   extracted copy still be wrong.
4. **All of it again through your own GnuPG**, if you have one. See below.

### Pointing it at something specific

```bash
veilvoice verify auto ~/Downloads          # look here, not wherever I am
veilvoice verify file veilvoice.tar.gz     # this file, against SHA256SUMS
```

A folder you name that does not exist is an error, not an invitation to go and
check somewhere else. That distinction cost a finding: the fallback through
Downloads and Desktop is right when nobody said where to look, and wrong the
moment somebody does.

### The second opinion, and why you want one

VeilVoice checks the signature itself, in Rust, with the signing key compiled
in. That check needs nothing installed and works on every platform.

It also came out of the download it is checking. **A tampered release ships a
tampered verifier.** That is not a bug to be fixed; no program can vouch for
itself.

So the commands are printed for you to run yourself, and the desktop
application will run your GnuPG if you ask it to:

```bash
veilvoice verify                # what to check, and where the files come from
veilvoice verify --script       # a shell script that uses gpg and nothing of ours
```

The script is about sixty lines. Read it before running it: the entire reason
to use it rather than `veilvoice verify` is that it is not this project's code.

### The strongest check, which no program here can do for you

Rebuild the release from source and compare:

```bash
veilvoice verify --build-script > reproduce-veilvoice.sh
sh reproduce-veilvoice.sh v0.1.18
```

A hash proves the file is the one whose hash was signed, and says nothing about
what is inside it, because the same person signed both. Rebuilding moves the
question from "do I trust the publisher" to "do I trust the source", and the
source is here to read.

### What a pass proves, and what it does not

A good signature and a matching hash prove the file is the one the holder of
that key published. They do **not** prove it is safe, that the source compiles
to it, or that the key belongs to anybody in particular. Compare the
fingerprint against the website and this README, from somewhere other than the
download you are checking:

```
8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A
```


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
Until one has, the strongest verification available to you is the source, which
is written to be read.
