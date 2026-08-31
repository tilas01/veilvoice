<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice — roadmap

**What is built, what is coming, and roughly when.** One marker is one feature:
written, tested, documented and merged. A marker is not ticked because the code
compiles — it is ticked when the thing works, has tests, has documentation, and
survives the checks in CI.

Estimates are in working days and they are estimates. Where a marker depends on
something outside this project — a platform's rules, a decision that has not
been taken — that is written down rather than absorbed into a number.

**Where we are now:** **v0.1.14 is released**, signed and published for
eleven platforms -- OpenBSD included since v0.1.11. Everything below the line
marked *shipped* is work in progress.

Since v0.1.14: every box in every flowchart on the reference pages opens the
file it names **on this site**, in the theme the reader chose, with the whole
function marked; a safety catch that reported a program it had closed as still
running; and a test that failed one run in forty and was passing the other
thirty-nine.

Since v0.1.13: **Failsafe**, on by default, which notices the moment another
program picks up a real microphone while you are being veiled; a baseline that
learns what normally runs here; a report of what privilege VeilVoice holds; a
randomised ratchet interval that had been written and never called; and an
interface that reads like English instead of shouting. Seven file dialogs that
froze the whole window while they were open are now threaded.

Since v0.1.12: the verifier can build this repository and compare what comes
out against the published hashes, group mode is visible in the desktop
application, projects and profiles can be saved, and the number of speakers is
capped at a limit that was **measured** rather than chosen. The reproducibility
check was verified by running it: two builds of this tree in two separate
target directories, all three binaries byte for byte identical.

---

## Legend

| | |
|---|---|
| **done** | Built, tested, documented, in `main` |
| **next** | Started or specified in detail, being worked on now |
| **planned** | Specified, not started |
| **blocked** | Cannot proceed until something outside the code changes |

---

## Shipped

| # | Marker | Status |
|---:|---|---|
| 1 | DSP engine — phase discard, many-to-one normalisation, CSPRNG modulation | **done** |
| 2 | Accent neutralisation, on by default | **done** |
| 3 | Cryptography — Argon2id, X25519+ML-KEM-768, XChaCha20-Poly1305 | **done** |
| 4 | Encryption at rest, by default, plaintext never touching disk | **done** |
| 5 | App lock — Argon2id verifier, persisted rate limit | **done** |
| 6 | Audio — device enumeration, live path, decode, WAV write | **done** |
| 7 | Metadata stripping — tags, EXIF/GPS, chunk-level RIFF cleaner | **done** |
| 8 | Microphone and camera monitor (Windows, Linux) | **done** |
| 9 | Tamper detection, unprivileged half (`veilvoice-guard`) | **done** |
| 10 | Secure erase, with an honest account of flash storage | **done** |
| 11 | CLI and desktop app | **done** |
| 12 | Website, wiki, no-JavaScript edition, legal gate | **done** |
| 13 | Search over the whole repository and website, with a static fallback | **done** |
| 14 | Portable release verifier needing no GnuPG (`veilvoice-verify`) | **done** |
| 15 | Install scripts for Windows, Linux and macOS | **done** |
| 16 | Reproducible signed releases on ten platforms | **done** |
| 17 | Four audit rounds — 47 defects found and fixed | **done** |

## In progress

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 18 | Documentation generator — a page, flowchart and banner for every crate and **every** `.rs` file, mirrored to the website and the GitHub wiki | **done** | — |
| 20 | Repository panel no longer shows a README's own markup as text | **done** | — |
| 21 | Write the missing module documentation for the 14 files that had almost none | **done** | — |
| 22 | Website split into a page per section, every published link still working | **done** | — |
| 23 | Motion and polish — smooth loading and scrolling, hover, CSS-first tooltips | **done** | — |
| 24 | Demonstration animation: a voice going in, the mark lighting up, an unidentifiable wave coming out | **done** | — |
| 25 | Cycling line of project facts, slow enough to read — CSS rather than an image, so it follows the reader's theme and needs no script | **done** | — |
| 26 | Every website theme in the app, plus user-defined palettes with contrast computed rather than assumed | **done** | — |
| 27 | Interactive workflow diagrams that open the relevant source, highlighted, in the site's palette | **done** | — |
| 28 | Randomised, user-configurable ratchet interval, with invalid input refused rather than clamped | **done** | — |
| 29 | One single binary — the same executable runs as the desktop app or as the command line, installed or portable | **blocked** | — |
| 30 | Installer with a window: Tokyo Night, animated, and **portable** described as the normal case rather than as something missing | **done** | — |
| 31 | Optional companion setup — VB-CABLE on Windows, PipeWire on Linux, BlackHole on macOS, and Audacity everywhere — detected if present, installed only if confirmed | **done** | — |
| 32 | The site's search presented as an **index**, and animated | **done** | — |

## Security and monitoring features

Each of these is a crate of its own, so that another project can depend on one
without taking all of them. Every one is **opt-in**, and every one states what
it cannot do as plainly as what it can.

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 33 | Screen-capture detection — which recorders are running, muted per program by an allowlist | **done** | — |
| 34 | Hide VeilVoice's own window from screen capture and recording | **blocked** | — |
| 35 | Keyboard and mouse activity monitoring, reported as the heuristic it is | **done** | — |
| 36 | `veilvoice-sentry` — ransomware canaries and mass-change rate detection | **done** | — |
| 37 | `veilvoice-appctl` — learn what runs, then allowlist it, with time-limited grants and a log | **done** | — |
| 38 | `veilvoice-policy` — settings sealed with the existing post-quantum cryptography, and shaped so they can only be tightened | **done** | — |
| 39 | Privileged mode: an opt-in service, and an elevated no-service mode, with the difference visible to the user | **done** | — |
| 40 | Alert on driver and kernel-module installation; cross-view checks | **done** | — |
| 65 | **Failsafe** — on by default: notice the moment another program picks up a *real* microphone while you are being veiled, warn, and close it | **done** | — |
| 41 | Notification overlay — rounded, translucent, contrast computed, or an alert, or off | **done** | — |
| 42 | Duress and decoy passwords | **done** | — |
| 43 | Transcription through your own API key, given **veiled audio only** | **blocked** | — |

## Conversations, subtitles and video

Asked for after v0.1.12. One recording, several speakers, each given a
different voice and each voiceprint destroyed just as thoroughly; names and
subtitles; and an optional video of the result.

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 46 | **Conversation mode** — tell the engine a recording holds more than one speaker, and give each a distinct voice while destroying every voiceprint | **done** | — |
| 47 | Up to ten speakers, each with a name, carried into the audio and into subtitles | **done** | — |
| 48 | A rolling seed **per speaker**, at a randomised interval inside a range the user sets, with no interval hardcoded and a fresh one at every launch | **done** | — |
| 49 | **Video output** — the waveform, a circle per speaker in their palette colour or their own picture inside a coloured ring, a title, and a black or image background with padding | **done** | — |
| 50 | A **preview** of the video and of the voices before anything is generated | **done** | — |
| 51 | An **asynchronous pipeline**, every speaker rendering at once rather than in sequence | **done** | — |
| 52 | Every crate and every `.rs` file explained: the technical workflow in a paragraph, then the same thing in plain words | **done** | — |
| 53 | The website on mobile, and on every engine — not only the one it was written in | **done** | — |
| 54 | **Seventh audit round** across the whole tree, then the production deploy | **done** | — |

## Building it yourself, and proving the download matches

Asked for after the conversation work. Today `veilvoice-verify` answers one
question — *is this download the one that was published* — and answers it
without GnuPG, without a network client of its own, and without ever holding a
private key. The request is to make the same program answer the harder
question: **is the published build the one this source produces**, and to have
it set your machine up so you can find out.

The program is renamed `veilvoice-setup-tools`, because checking a signature is
then the smallest thing it does.

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 55 | **Build the whole repository from source**, from the tool itself: find or install a toolchain, pin it to `rust-toolchain.toml`, and run the same build the release does | **done** | — |
| 56 | **Reproducibility check** — build here, hash what came out, and compare it against the published `SHA256SUMS` entry for this platform, saying which files matched and which did not | **done** | — |
| 57 | **The hashes are trusted only after the signature is** — verify the detached signature over `SHA256SUMS` against the project key *before* any hash from it is compared, and refuse rather than warn if it does not verify | **done** | — |
| 58 | **Set the machine up per platform** — the build dependencies each operating system actually needs, detected, named with who ships them, and installed only on an explicit yes | **done** | — |
| 59 | **Custom install** — CLI, desktop app, or both, from a build you just made or from a download you just verified | **done** | — |
| 60 | **Four verbosity levels** — nothing, minimal, normal (the default) and everything — applied to every one of the above, with the exit status carrying the answer when the output carries nothing | **done** | — |

## Group mode, where you can see it

The engine has handled several speakers since marker 46. The desktop app has
never shown it. These are about making the thing visible and usable rather than
about the signal, which is already done.

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 61 | **Group mode in the desktop app**, shown as a mode rather than hidden in a flag: off by default, a toggle that does not persist, and a separate tick for "always start in group mode" | **done** | — |
| 62 | **A name and a colour per speaker in the app** — the colour chosen automatically to be as distinct as the number of speakers allows, overridable per speaker, and drawn from every palette the website offers | **done** | — |
| 63 | **Live levels while a recording is running**, in the app and in the terminal | **done** | — |
| 63b | **A wave per speaker while recording** — the same picture, but split by who is talking | **blocked** | — |
| 64 | **Speaker detection through software you already have** — detected exactly as the other companions are, never bundled, and the honest paths kept for a machine without it | **blocked** | — |

## Seeing it before you install it

Asked for after v0.1.14. Everything here is about the same problem from two
sides: somebody deciding whether to trust this, and somebody using it and not
being sure it is working. Neither is answered by more features.

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 66 | **The live monitor** — what is going in and what is coming out, on every tab, on by default, and a preview that lets you hear yourself veiled before anybody else does | **done** | — |
| 67 | **An interactive demonstration on the website** — the inside of the application and of the command line, laid out in the site's own colours, that a reader can click through before downloading anything | **done** | — |
| 68 | **A frequently asked questions page**, answering what gets asked rather than what is convenient to answer | **done** | — |
| 69 | **A drawn graphic for every workflow chart** — coloured arrows, an explanation inside the picture, and every word wrapped rather than running off the edge | **done** | — |
| 70 | **This roadmap, published as a page**, with a picture of what is done and what is not, generated from this file so the two cannot disagree | **done** | — |
| 71 | **A video of the roadmap**, scrolling what is finished, with a short pause and a countdown before it repeats | **done** | — |
| 72 | **The front page animation, in more depth** — the same picture, saying what the engine actually does to the signal rather than one word | **done** | — |
| 73 | **A full security and functionality audit, and an optimisation pass, before the next deploy** — the whole tree, both halves, and the last thing that happens | **done** | — |

## The lock, the guard, and a window that does not stutter

Asked for after the tenth audit round. Four of these are one subject seen from
different sides: the app lock is the weakest control this project ships, it is
described as such in several places, and the request is to make it as strong as
it can honestly be made rather than to keep apologising for it.

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 74 | **The lock screen tells an attacker nothing** — no explanation of what the lock is or is not worth while it is locked, the account of that moved to the documentation and to the unlocked application, and a small animation in its place | **done** | — |
| 75 | **`veilvoice-guard` inside the desktop application** — the integrity record taken at the first launch and checked at every one after, sealed under the app-lock passphrase where there is one | **done** | — |
| 76 | **The app lock, hardened as far as it honestly goes** — an authentication tag under the passphrase, two copies with the spare administrator-owned where the platform allows it, restoration when one goes, randomised names and masked contents, and a report that only the passphrase can clear | **done** | — |
| 77 | **What the lock is worth, written down properly** — one account, in the documentation, separating the parts that are real from the parts that are only obscurity | **done** | — |
| 78 | **Every website palette in the application, chosen from the interface** | **done** | — |
| 79 | **A window that does not stutter** — the interface measured rather than described, every task off the drawing thread, and the smallest amount of code that does it | **done** | — |

## Finally

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 44 | Fifth audit round — every vulnerability class across the tree, twelve findings written up individually (F-48 to F-59) | **done** | — |
| 45 | **v0.1.10 released** — ten platforms, signed, and verified by hand after publication | **done** | — |
| 80 | **Ready for the release audit** — the RPM built, `lintian` run and its findings fixed, manual pages generated from the binaries, 32-bit re-run over the new code, and the parser campaign run over all six targets with a seed corpus kept | **done** | — |

---

## Encrypted volumes: Cryptomator, VeraCrypt, and the disk underneath

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 81 | **Find the encrypted volumes this machine already has** — detect an installed Cryptomator or VeraCrypt, and the vaults and mounted volumes each is offering, without asking either to do anything | **done** | — |
| 82 | **Write veiled output into a chosen volume** — a destination that is a Cryptomator vault or a mounted VeraCrypt volume, remembered, and used for every export | **done** | — |
| 83 | **The hidden-volume question, asked properly** — asked before the first write, three answers, and a job that will not start until one is given | **done** | — |
| 84 | **The guided path, for when detection fails** — plain instructions, a folder chosen by hand, and the same confirmation a detected one gets | **done** | — |
| 85 | **What full-disk encryption is for, said once and said properly** — BitLocker, FileVault, LUKS and LUKS2, and the OpenBSD and FreeBSD equivalents, single-sourced and shown in both | **done** | — |
| 86 | **The app lock as a key, not only a verifier** — the app-lock passphrase seals everything VeilVoice veils, automatically, as an option that says what it costs | **done** | — |

---

## Asked for after the encrypted volumes

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 87 | **A video of a veiled recording** — a black frame and the audio, so a recording can be posted where only video is accepted | **planned** | 2–3 d |
| 88 | **Import from every format OBS writes** — bring in a recording made elsewhere, video or audio, and take the sound out of it | **planned** | 2 d |
| 89 | **Veil the other person afterwards** — the interviewee given their own voice in post, through the group plan that already exists | **planned** | 2–3 d |
| 90 | **GnuPG verification inside the window** — in the verify tab, beside the hash check, using the GnuPG somebody already has | **done** | — |
| 91 | **`veilvoice-verify` finds the release itself** — GnuPG arguments where wanted, and an `auto` that looks in Downloads, checks the archive, and checks what came out of it | **done** | — |
| 92 | **An autolock timeout** — off by default, and when on, from five minutes to forty eight hours, chosen from a list or typed, with the range itself adjustable | **done** | — |
| 93 | **Group mode explained where it is used** — how to build a plan, what each field does, and what happens without one | **planned** | 2 d |
| 94 | **Release notes people can actually read** — every release listed newest first, its notes opening in place, and every file one click away | **planned** | 2 d |
| 95 | **One version per release, in order, enforced** — the tag, the workspace and every package definition checked against each other before a release can go out | **planned** | 1 d |
| 96 | **v0.1.15 released** — the audit run over everything since v0.1.14, CI green, and the release published | **planned** | 1–2 d |

---

## The things that are not just work

Some of the markers above depend on something other than effort, and
pretending otherwise would make this roadmap a wish list. They are named rather
than numbered, because a number changes whenever a row above it does -- which is
exactly what happened when the USB work was dropped from this list.

**Transcription: the decision has now been taken, and it is a narrow one.**
Marker 43 was blocked because VeilVoice talks to no servers at all and CI fails
the build if a network client appears anywhere in the dependency graph — one of
the few claims a reader can check in ten seconds, and a large part of why this
project is worth trusting.

The decision is: **transcription may happen, and anything that leaves this
machine is the veiled audio, never the recording.** That is a smaller trade than
it first looks, because the veiled audio is the thing this project exists to
produce: the words are intact and the voiceprint is gone, so a provider given it
receives a transcribable recording of a voice that is not anybody's. Sending the
original would hand a biometric to a third party, and that is the thing being
refused rather than the transcription.

Three rules go with it, and they are what keep the front page true:

* **Off by default, and never a default.** No transcription happens unless it
  is switched on for that run.
* **The guarantee is kept in the dependency graph.** Nothing here adds an HTTP
  client to VeilVoice: a local model is reached by running the program the user
  already installed, and a provider is reached by shelling out to the system's
  own transfer tool — the same arrangement the release verifier has used for
  downloads since it existed. The CI job that fails on a network client stays
  exactly as it is.
* **The claim is reworded where it appears, not quietly kept.** "It talks to no
  servers. Ever." becomes true-with-a-named-exception the moment this ships, and
  the wording changes in the same commit as the code, not after it.

Not every provider accepts audio input, and that is not a small caveat: an API
that takes text and images does not take a WAV, whatever else it can do. Which
providers actually accept audio is a fact about somebody else's service that
this machine cannot check offline, so it is checked *from a machine that can*
before a line of it is written — the same rule that turned marker 64 from
planned into blocked, one paragraph down, and saved a feature that would have
shipped unable to work.

**Detecting who is speaking: decided in principle, and then measured, and the
measurement moved it.**

The decision stands: VeilVoice ships no model, and uses software you already
have or nothing. What changed is *which* software, and it changed because the
promise a few paragraphs up — "that gets checked before anything is built
rather than discovered by a user" — was kept.

`ollama` was the named candidate. It was checked on a machine that has it:

* **It is there and it is detectable.** Found at an absolute path, version
  0.32.5, with seven models installed. The companion-detection pattern works on
  it exactly as it does on Audacity.
* **None of it transcribes speech.** Every model on that machine is a text
  model. ollama's registry is language and vision models; speech-to-text is not
  what it hosts. Detecting ollama and offering "transcription" through it would
  have produced a feature that cannot work, on a machine where every check
  passed.
* **Running it is not free.** A single `ollama list` started a background
  server, opened a local UI port, started an update checker on an hourly timer,
  and made a network request to GitHub — all of it in the first two seconds,
  none of it asked for. For most programs that is unremarkable. For this one it
  means "VeilVoice can use ollama" would have to be read as "VeilVoice can
  start a background service that phones home on a timer", and that has to be
  said in those words or not offered.

So marker 64 is **blocked**, on a question rather than on effort: local
speech-to-text means a Whisper-family program — `whisper.cpp`, `faster-whisper`
— and speaker diarisation means a third thing again. Which of those to detect,
and whether starting any of them is acceptable given what was measured above,
is the maintainer's call. Marker 43 is blocked behind the same question for its
local half.

The two honest paths that exist today — one microphone per person, or a turn
list — remain, and remain the default. A machine with none of this installed
behaves exactly as it does now.

**Privileged mode and driver alerting cannot reach kernel level on Windows
or macOS.** Loading a
kernel driver on 64-bit Windows requires an EV code-signing certificate issued
to a verified legal entity and then Microsoft's attestation signing. macOS
requires an Apple Developer ID and an entitlement Apple grants case by case.
Both are identity checks, and this project is published under a pseudonym on
purpose. **The decision taken is to ship the administrator version** — which is
most of the protection and none of the pretence — and to say plainly that
kernel-level enforcement is unavailable on those two platforms and why. Linux
and OpenBSD have no such gate.

**"Builds for every operating system" will mean "builds for the one it is
running on".** Markers 55 to 58. A build needs that platform's headers and
linker: `veilvoice-cli` cannot be compiled for Linux from this machine today
because `alsa-sys` needs ALSA's headers, and a macOS build needs Apple's SDK,
which Apple's licence does not allow to be redistributed or run elsewhere.
Every other crate cross-checks cleanly with `--target`, and that is a *type
check*, not a binary anyone should install.

So the honest shape is: the tool builds VeilVoice for the machine it is on, and
compares that against the published build **for that platform**. Three machines
give you three platforms verified, which is exactly how a reproducible-build
claim is normally checked, and it is a real answer rather than a pretended one.
Where a cross-target check *is* possible the tool will offer it and will label
it as what it is.

**Reproducibility is a property of the release, not of the checker.** Marker 56
can only report what it finds. If a build here and the published build differ,
that is a finding to publish, not a bug in the tool to paper over -- and the
first version will print both hashes and the differing file names rather than a
verdict, because "not reproducible" has several causes and most of them are
boring.

**The one thing marker 55 does not do is install the compiler.** It reports
the Rust toolchain like any other dependency — found or missing, with the
version — and points at rustup, which is how the Rust project ships it. It
does not run that installer. rustup downloads a compiler, writes to the home
directory and edits the shell profile, and all three belong to the person whose
machine it is rather than to a program acting for them. Every other dependency
on Linux is offered through the system package manager under the rule below.

**A dependency probe can be wrong in the direction that matters, and one was.**
The Windows linker check looked for `link` on `PATH` and reported whatever it
found. On the first machine it ran on that was Git for Windows'
`usr/bin/link.exe` — GNU coreutils' hardlink utility, which shares a name with
Microsoft's linker and has nothing to do with building Rust. It said the linker
was present; the build would then have failed. There is no honest probe for it,
because cargo finds MSVC through the registry rather than `PATH`, so it now says
it cannot tell and lets the build be the judge. Recorded as F-68.

**Markers 48 and 63 are each half-built, and stay open until both halves are.**
Marker 48's per-speaker seeding is done: every speaker gets its own seed and
its own destination, so there is nothing shared between them. What is missing
is the *randomised interval inside a range the user sets*, which is marker 28
and is still open. Marker 63's levels are done and run in the terminal during
`veilvoice live`; the wave **per speaker** is not, because it needs per-speaker
capture, which live mode does not have. Rounding either of these up to done
would be the overstatement this project's second rule exists to prevent.

**A reproducibility checker that always says no is worse than none.** Marker
56's first version ran `cargo build --release` and nothing else, so it would
have reported every user's build as differing from the published one — for
the dull reason this repository has documented since before the checker
existed: absolute paths are baked into panic messages and debug info, and
removing them is the build environment's job. Two builds of this tree in two
directories on this machine produced three differing binaries out of three,
measured. It now reproduces the release environment instead of approximating
it — the same `--remap-path-prefix` for source and `CARGO_HOME`, the same
`SOURCE_DATE_EPOCH` from the commit, the same per-linker flag, the same
explicit `--target` — and prints every one of them before building, because a
comparison whose settings are invisible cannot be checked by whoever reads the
result. A test compares the flags against `release.yml` itself, so changing one
and not the other fails the build. Recorded as F-70.

**Installing build dependencies means running somebody else's package manager.**
Marker 58. That is the same trade the companion setup already makes and it gets
the same rule, which predates this roadmap: detect what is there, say what each
thing is and who ships it, and install only on an explicit yes -- never
silently, never ticked by default. What it will not do is add a network client
to VeilVoice: it shells out to the tool the platform already has, exactly as the
verifier does for downloads today, so the guarantee that this project's own
dependency graph contains no HTTP client is unchanged.

**"Nothing" is a real verbosity level and needs the exit status to carry the
answer.** Marker 60. A tool that prints nothing and returns zero on failure is
worse than a noisy one. Every operation gets a distinct non-zero status, and
they are documented, before the quiet mode exists.

**"Every engine" is a claim only one engine has been asked about.** Marker 53.
The mobile half is done and was done by measurement: twelve pages at five
viewport widths, with and without scripts, and eight separate causes of
horizontal scrolling found and fixed -- a grid item's default minimum width, an
unshrinkable table of code names, a tooltip that pushed the front page sideways
while closed, two sections missing their gutters, and a note in the header that
only appears when scripts are off. None of them was visible from the source.

All of it was measured in Chromium, because that is the engine on this machine.
Firefox and WebKit have rendered none of it. The stylesheet has long carried
fallbacks written *for* those engines -- `-webkit-backdrop-filter` for Safari 17
and earlier, a solid colour before every `color-mix`, `:focus-visible` split
into its own rule -- and `tools/site-tests/css.test.js` checks that each is
still there. That is reading the specification carefully; it is not the same as
having looked. The marker stays open until something other than Chromium has
drawn the page.

**A seed cannot roll faster than a frame, and a frame is 5.3 ms.** The
request was for a rolling interval between 0.7 ms and 2.7 ms. The engine
analyses audio in frames of 1024 samples with 75 % overlap, so it produces one
set of modulation parameters every 256 samples -- 5.33 ms at 48 kHz. Rolling
the seed more often than that changes nothing, because there is nothing in
between to change. Making the frame short enough would mean a 128-point
transform, which is 375 Hz per bin: too coarse to find a formant, which is the
thing being moved. So the interval will be settable in milliseconds and
randomised inside the range asked for, it will be **quantised to whole frames**,
and the interface will report the interval that is actually in force rather
than the one that was typed. Rolling every single frame is what the whole
requested range comes to, and that is already the fastest this can honestly go.

**Video needs an encoder, and this project ships no codec.** A window of
waveform and circles is straightforward to draw; turning a few thousand frames
into a file somebody can play is not, and writing an H.264 encoder is not a
sensible thing for this project to do. The plan is to render the frames here
and produce the video through `ffmpeg` when it is present -- detected and
offered exactly as the other companions are, never bundled, never installed
without an explicit yes -- and to always write a self-contained animation that
needs nothing else installed, so the feature still produces something on a
machine without it. What will not happen is a silent dependency on a program
the user did not know they were running.

**Hiding VeilVoice's own window from a screen recorder needs `unsafe`.**
Marker 34. Excluding a window from capture is `SetWindowDisplayAffinity` on
Windows, and the equivalents on macOS and under Wayland; all of them are
foreign-function calls, and every crate in this workspace carries
`#![forbid(unsafe_code)]` — which is on the front page and is one of the things
a reader can check in ten seconds. The trade is the maintainer's: a documented
`unsafe` shim in one file, or a window that can be recorded. **Until it is
made, the honest state is written where a user will read it**: VeilVoice does
not hide itself, `veilvoice capture` says so, and marker 33 shipped without
pretending otherwise. Nothing about it is hard except the decision.

Worth noting that the same decision would not buy very much. A window
excluded from capture is still visible to a camera pointed at the screen, and
the thing VeilVoice protects — the recording — is a file, not a picture of a
window.

**Marker 39 ships the administrator version and reports the difference; it
does not acquire anything.** `veilvoice privilege` says what VeilVoice is
running with, what that level can and cannot see, and prints the command to run
it the other way. It never re-launches itself elevated, installs a service, or
asks for a password: those are changes to somebody's machine and they belong to
the person whose machine it is. A test names every subprocess the crate starts,
so "it only reports" stays true rather than staying a comment.

**The opt-in service is deliberately not shipped**, and the reason is written
where a reader will meet it: a service outlives the window it was started from,
starts itself at boot, and runs whether or not anybody is using the program —
somebody who tried VeilVoice once should not find it still running next month.
Leaving the window open is the honest form of continuous monitoring, because
then what it can see is exactly what it says it can see.

Two details found by measuring rather than reasoning. The Windows probe keys on
the well-known SID `S-1-5-32-544` rather than the group's **name**, which is
translated and would report every non-English machine as unprivileged. And the
"Group used for deny only" attribute — what an administrator account looks like
when it is *not* running elevated — is on the same 236-character line as the
SID, not the next one; a console wraps it so it looks like two rows, and
reading it that way would report every administrator account as elevated
whether or not it was. Verified on a machine in exactly that state.

**Who is talking is clear in everything VeilVoice writes, and the engine's
settings are not.** The page and the video carry each speaker's name and a
circle that lights on their turn, and the subtitles carry the names as typed.
What none of them carry is the destination voice's register, vocal tract or
frequencies. Those are shown in the application, where somebody choosing
between voices needs them, and a test renders a page and fails the build if any
of that vocabulary appears in it. It describes the *destination* rather than
the speaker, so it leaks nothing either way; it is simply noise to a viewer,
and it invites a reader to think the numbers say something about the people.

**In a live session, who is talking is a different question and an honest one
to refuse.** One microphone carries one signal, and telling two voices apart
inside it is diarisation, which is markers 43 and 64 and is blocked for the
reason recorded there. The path that does work is one microphone per person,
which is marker 63's other half.

**Marker 63 is split in two, because one half shipped and the other cannot
start.** The maintainer confirmed the live output is what was wanted and that
showing it is right, so the levels are marked done under their own number and
the diarisation half is marker 63b, which stays blocked for the reason below.
Carrying both under one number meant a finished feature reading as blocked.

The *levels* are done and have been for some time:
`veilvoice live` draws them in the terminal and the desktop application draws
them beside the devices, both with peak-hold. The **wave per speaker** is a
different thing entirely — it needs the live input separated by who is talking,
which is diarisation, which is markers 43 and 64 and is blocked for the reason
recorded there: real speaker separation means shipping a trained model, and
locked decision 5 says this project does not.

Leaving it marked *planned* would imply an estimate exists for work that cannot
start, which is the same overstatement in the other direction. What could be
built without diarisation — a wave per speaker in a *rendered* conversation,
where the plan already says who speaks when — exists, and is what the video
output and the HTML player draw.

**Marker 29 moves to blocked, because it is a decision rather than a task.**
One executable that opens a window when double-clicked and takes subcommands
when given them needs `AttachConsole` and `FreeConsole` on Windows: a PE
declares exactly one subsystem, so a console binary that opened a window would
flash a console every time, and a windowed one would send its output nowhere
when run from a terminal. Switching at run time is FFI, and every crate here
carries `#![forbid(unsafe_code)]`. Relaxing that for one convenience is the
maintainer's call and not something to slip in, so it waits for one.

**Markers 81 to 85 are two encryption tools and one honest sentence about what
stacking them buys.**

The request is that VeilVoice notice an installed Cryptomator or VeraCrypt,
offer to put every exported file inside one, support VeraCrypt's hidden
volumes, guide the user by hand when it cannot manage the integration itself,
and say plainly that the disk underneath should be encrypted too. Each of those
is a separate marker because each can be finished, shipped and judged on its
own, and because the first one being impossible on some platform must not stop
the last one being written.

*Detection is reading, never driving.* Marker 81 finds what is installed and
what is currently mounted, and does nothing else. It does not launch either
program, does not ask either to mount or unlock anything, and never handles a
volume passphrase. VeilVoice already refuses to acquire privilege (marker 39)
and this is the same rule in a new place: mounting somebody's encrypted volume
is their act, taken in the tool they chose, not something a voice de-identifier
does on their behalf. A mounted Cryptomator vault and a mounted VeraCrypt
volume are both just directories by the time VeilVoice sees them, which is
exactly why this can be honest and small.

*Writing into one is a destination, not a mode.* Marker 82 is a remembered
output directory with a label saying what kind of volume it is. The encryption
is entirely the other tool's, and calling it "VeilVoice encryption" would be
the overclaim this project refuses. What VeilVoice adds is that the export
lands there by default rather than in a Downloads folder somebody meant to
clear out.

*The hidden-volume question is the one that must not be guessed.* VeraCrypt's
hidden volumes exist so that a person under compulsion can hand over one
passphrase and reveal an outer volume. Writing to the outer volume of a
container that has a hidden one can destroy the hidden data, because the outer
filesystem does not know the hidden one is there. VeilVoice cannot tell the two
apart by looking, and no amount of cleverness will change that: it is the
design of the feature that they are indistinguishable. So marker 83 asks, once,
before the first write, and stores the answer with the destination. It never
infers, never defaults to "probably fine", and refuses to write until it has an
answer. A tool that quietly guessed wrong here would destroy exactly the data
its user was most careful about.

*Marker 84 exists because detection will fail.* Portable installs, custom
paths, a distribution that packages either tool somewhere unexpected, a
platform neither supports. The answer is not a silent fallback to writing
somewhere unencrypted: it is instructions, a directory the user picks by hand,
and a confirmation step that will not continue until they have said which
volume they mean and what kind it is. The failure mode to avoid is a user who
believes their exports are in a vault and finds them beside it.

*Marker 85 is a sentence, and it is the most important one in the group.*
Cryptomator and VeraCrypt protect files at rest inside a container. They do not
protect the temporary files an operating system writes, the swap or hibernation
image the kernel writes, the thumbnails a file manager writes, or the recently-
opened list a desktop keeps. A veiled recording that lives inside a vault can
still have left traces outside it, and full-volume encryption is what covers
those: BitLocker on Windows, FileVault on macOS, LUKS or LUKS2 on Linux,
`softraid -C` on OpenBSD, GELI on FreeBSD.

So the honest framing, and the one the documentation and the application will
both use, is that this is **defence in depth and not a second lock on the same
door**: the volume protects the file, the disk protects everything the system
wrote about the file without being asked. Describing the pair as "dual layer
encryption" without that sentence would leave somebody thinking a vault alone
is enough, and it is not.

*What none of it changes*: VeilVoice's own `.veil` containers are already
encrypted with its own cryptography, and nothing here replaces or weakens that.
A veiled recording written into a Cryptomator vault is encrypted twice, by two
independent tools, and the useful property of that is not extra strength but
independence: a defect in one is not a defect in both.

**Marker 90 puts the GnuPG commands where the question is already being asked.**
The verify tab is where somebody is working out whether a download is genuine,
and the honest answer to that question includes "and here is how to ask
something other than me". The commands are copyable and they are not run: this
project checks signatures with a key compiled into itself, which is a
convenience with an obvious circularity, and a window that shelled out to `gpg`
and reported what it said would not have escaped it.

The body of the recipe moved into `veilvoice-check`, which the portable verifier
and the window already share for the checking itself, so the two cannot drift
into printing different commands. A test holds that.

**Marker 91 reports the archive and the extracted folder separately, and that
separation is the whole of the thinking.**

`auto` already found the release and checked the archive. What was asked for on
top is that it check what came out of the archive, and that GnuPG be available
for anybody who wants it.

The second is easy. The first has a limit that must not be papered over:
`SHA256SUMS` is signed and it covers **archives**. Nothing signs the contents of
a directory somebody unzipped last week, and nothing on disk records which
archive a folder was extracted from. So verifying `veilvoice-0.1.14-linux-x86_64.zip`
proves that archive is the signed one, and proves nothing whatever about the
folder beside it, which may predate the download or have been edited since.

Rolling both into one green result would tell somebody their installed copy is
verified when it is not, which is the most expensive kind of wrong this project
can be. The two are reported separately, the limit is stated in the output in
those words, and the one thing that resolves it is given: extract the archive
that was just checked, now, and use that.

What can honestly be said about the extracted folder is whether the programs are
there and whether the system will run them, and that is a real thing to get
wrong: an unpacking tool that drops the execute bit leaves somebody with files
that look right and will not start. A failed archive stops before any of this,
held by a test, because "the archive is bad, and here are the programs beside
it" reads as reassurance and there is none to give.

*GnuPG commands are printed, never run.* A verifier that shells out to `gpg` and
reports what it said has not escaped the circularity it exists to escape: the
thing running `gpg` is the binary under suspicion. `veilvoice-verify gnupg` is
its own subcommand rather than a footnote under `auto`, because somebody who
wants the independent answer should not have to be told the answer by this
binary first.

**Markers 81 to 85 are built, and two things were learned in the building.**

The first is that the hidden-volume question is worth more than the feature
around it. Everything else here is a remembered output directory; that question
is the only part where getting it wrong destroys data that cannot be recovered.
So it is not a checkbox: `Hidden` starts `Unanswered`, an unanswered
destination refuses to place a file, the outer volume of a declared pair is
refused outright, and a settings file edited into nonsense reads as unanswered
rather than as "fine". A job with an unanswered destination is **blocked**
rather than redirected back beside the source, because the silent fallback puts
a recording outside a vault while its owner believes it is inside one.

The second was found by writing it. The panel called `still_there`, a stat
syscall, once per frame, for an answer that changes when somebody unlocks a
volume. That is precisely what marker 79 taught the draw path to refuse, and it
went straight into a new module where that guard test does not look. The answer
is cached at refresh now and a second guard test lives beside the panel.

Detection stayed read-only throughout, as marker 39 requires, and a test refuses
`Command::new` in the shipped half of the module. Verified on this machine end
to end: a real binary on `PATH` is found, a mount table produces the two
volumes and none of the ordinary mounts, and a VeraCrypt volume is refused until
answered.

**Marker 86 reverses a decision this project has documented and defended, and
it is written down that way rather than quietly.**

The request is that the app lock stop being only a password check and become a
key, so that every file VeilVoice veils is encrypted automatically without
anybody choosing a second passphrase.

**Built, and the construction is simpler than the one this paragraph first
proposed.** The plan was a third HKDF label producing a file key from the lock's
own derivation. That would have worked and it had a defect worth catching before
it shipped: a key tied to the lock file's salt means deleting the lock destroys
every recording sealed under it, and deleting the lock is the documented remedy
for forgetting the passphrase. So the recordings are sealed under the
*passphrase* instead, through the ordinary container path, with a fresh salt per
file. Nothing about opening them later depends on the lock existing, and
`veilvoice decrypt` opens them on any machine.

What has to be stated, because the current code states the opposite in as many
words, is the property that is being given up. `lock.rs` says today that the
lock *deliberately* does not derive a key that encrypts recordings, and
`USER_GUIDE.md` says to use two different passwords, both for one reason: if a
single passphrase did both, then opening the application would be the same act
as unsealing everything it had ever written. Somebody who is compelled to
unlock VeilVoice in front of another person currently reveals the session; with
marker 86 they reveal the archive as well.

So this is a genuine trade, not a free improvement, and it goes in with three
things attached or it does not go in:

- **The container passphrase stays.** App-lock sealing is a third choice beside
  *passphrase* and *public key*, not a replacement for either. A user who chose
  a separate recording passphrase keeps it.
- **It is opt-in, and the interface says what it costs**, in the same place and
  the same plain words the app lock already uses to say what it is worth. It is
  offered only where a lock exists, because there is nothing to seal with
  otherwise.
- **Losing the app-lock passphrase then loses the files.** Without it,
  forgetting the passphrase costs a session and the fix is deleting the lock.
  With it, deleting the lock does not help, and there is no recovery. Said in
  the interface and in `USER_GUIDE.md` section 5.4 before it can be switched
  on, rather than discovered afterwards.

One thing fell out of building it that the plan had not anticipated. The
passphrase has to be kept for the session to seal with, and keeping it is a
real cost, so it is kept *only when the mode is already chosen*: a user who has
not asked for this keeps the previous behaviour exactly, where the passphrase
is wiped the instant it has been checked. A test holds that, because the lazy
version of this change is one line and holds the passphrase for everybody.

**Marker 80 is the work a roadmap marker does not describe, and it found a
defect.** Every marker above being finished is not the same as a tree being
ready to release, and the difference is the list `docs/AUDIT.md` keeps under
"Still open": things nobody had run rather than things nobody had written.

Four of them were runnable and were run. The RPM builds, which turns a spec
file from a draft into a package and proves the one thing a parse cannot, that
`%files` and `%install` agree. `lintian` reports no errors over the Debian
packages, and the three warnings it did have are fixed rather than recorded.
The 32-bit targets were re-run over the new lock, vault and integrity code,
which had never run anywhere but x86-64. The parser campaign ran over all six
targets rather than one, at twice the length, and now starts from a committed
seed corpus for the two that would otherwise spend their budget rediscovering
a magic string.

The defect came out of the last of those, and not from a crash. The campaign
reported a slow unit, which is the least interesting thing it can report, since
a deliberately slow key derivation taking a second is what a deliberately slow
key derivation is for. Reading the cost out of it and asking which callers
reach that path without a person choosing the file produced F-92: two more
places using the ceiling meant for a file somebody was sent, one of which this
cycle had itself made automatic.

**Marker 75 seals the record with the only secret the program ever has, and
that decides when it can run.** Sealing needs a passphrase, and a window that
has just opened has none. The app-lock passphrase is the one secret this
program is ever given, so that is what the record is sealed with, and the
unlock is therefore when the check runs. With no app lock set there is nothing
to seal with, so the record is written in the clear and the tab says so in
those words. Sealing it under a key kept beside it would have looked like the
sealed case and been worth nothing, which is the failure this project keeps
refusing.

*Taken at first launch rather than at install*, and that is a correction rather
than a shortcut. Installing a package runs as an administrator; the record
belongs to the user account that will run the program, and lives in that
account's configuration directory. A postinstall hook would have written a
record into the administrator's directory describing a check nobody performs.
There is no per-user moment during a package install, so the first launch is
that moment.

**Marker 78 was already built and was not findable, which is a different
fault.** Every one of the website's nine palettes has been in the application
since marker 26, with the swatches and the contrast measured rather than
assumed, and the picker was on a page inside Settings. Somebody looks there
only if they already believe there is something to find. The website puts its
picker in the header of every page; so does this now, and Settings keeps the
fuller panel. Nothing was added to the engine and one control was moved into
sight.

**Marker 79 is done as far as a machine with no screen can take it.** The draw
path was read for the calls that wait, and there are none:
everything that can block already runs on its own thread and reports back
through a channel, which a test now holds rather than a comment. The repaint
cadence was one number, 50 ms, for a live session and a download alike, and
twenty frames a second is fine for a progress line and is not fine for a meter
following a voice: at that rate it steps rather than sweeps, and a window whose
only moving part is stepping reads as one that is struggling. A live session
asks for 16 ms now and everything else keeps 50.

*Compilation was asked about and was already at the ceiling.* The release
profile builds at `opt-level = 3` with fat link-time optimisation, one codegen
unit, `panic = "abort"` and the symbols stripped, which is every setting that
makes a difference. The one thing deliberately not set is `target-cpu=native`,
and `.cargo/config.toml` says why: it would produce a binary that runs on the
machine that built it and crashes on an older one, and it would break the
reproducible builds this project publishes. So there was nothing to change here
and the honest report is that there was nothing to change, not a commit that
moved a number.

*The draw path was read for per-frame filesystem calls and has none.* An
`exists()` or an `is_file()` in a frame is a stat syscall sixty times a second
for an answer that changed when a file was dropped, and it is the shape of
stutter that only appears on a network share or a drive that has spun down.
There were none to remove. The guard test now refuses them along with the calls
it already refused, so this stays true rather than being true today.

What is *not* claimed is that this fixes anything anybody has reported. The
machine this was written on has no display and cannot measure a frame. So the
About tab shows the frame time, smoothed, because a number from the person with
the problem is worth more than a change made blind, and 16 ms and 120 ms are
different problems with different causes. Flicker in particular has causes this
machine cannot reach: a compositor, a driver, a scaling factor. The window
clear colour was checked against the panel fill, which is the one flicker cause
that lives in this code, and they already match.

**Markers 74 to 79, and the two places they meet a rule this project already
has.**

*Storing the lock where only an administrator can write it* is worth having and
it runs into marker 39's decision, which is that VeilVoice **never acquires
privilege**: it does not re-launch itself elevated, install a service, or ask
for a password, because those are changes to somebody's machine and they belong
to the person whose machine it is. The shape that keeps both, and the shape
that shipped: the *second* copy goes to an administrator-owned directory when
VeilVoice is already running with enough privilege to make one, and stays in
the user's own directory when it is not. Putting the *first* copy there would
have broken the rate limit, because an unelevated run has to be able to write
down a failed attempt. The test for privilege is the attempt itself, which
avoids a platform call in each case and asks the only question that matters.
On Windows the equivalent needs an access-control list this project does not
link the API to set, so there the second copy is a second copy and is described
as one.

*The authentication tag is the part of marker 76 that is not obscurity, and it
is smaller than it sounds.* One Argon2id run is split by HKDF into the verifier
that goes on disk and a tag key that never does, so an edit made by somebody
without the passphrase cannot be made to look authentic. That catches the
swapped password and the weakened cost. It does not catch deletion, which is
what the second copy answers, and it does not catch a lock replaced wholesale
with the attacker's own, which nothing here answers. The failed-attempt counter
had to be left outside the tag entirely: it is written at the one moment the
tag key does not exist, so covering it would have meant reporting every honest
typo as tampering. The rate limit is therefore exactly as editable as it was
before, and the documentation says so in as many words.

*Hiding the file's name and contents* is obscurity, and obscurity is not a
security property. Randomising where the lock lives and what it looks like
raises the cost for somebody poking around and stops nothing that a determined
local attacker with a debugger will do. The names come from an index file at a
fixed, obvious path, because something has to be findable or the program could
never open its own lock again, so anybody who reads that index recomputes both
names at once. It is worth doing for the first case and it must not be
described as protection against the second. Marker 77 is that sentence, written
where a user reads it, and marker 74 is the decision that the *lock screen* is
not the place for it: telling somebody standing at a locked window what the
lock cannot do is helping the one person who should not be told. The
interference report follows the same rule and is drawn on the security tab, not
on the lock screen, because telling whoever is holding the machine that their
edit was noticed tells them to try a different one.

**Marker 71 is an animation rather than an encoded file, and that is a
decision.** A video was asked for and a video is the right shape for it:
something to watch rather than a picture with a long dead pause in it. What it
is not is a reason to put an H.264 file in this repository. This project ships
no codec and does not bundle `ffmpeg`, and the rule already settled for video
output applies here too: render here, and always produce something that needs
nothing else installed. An encoded file would also be a committed binary whose
bytes depend on which build of which encoder made it, so it could not be
regenerated and compared the way every other picture here is. The result plays
in any browser with no plugin and no download, weighs a few kilobytes, takes
the reader's colour scheme, and is generated from this file so it cannot show a
marker as finished that is not. The command to turn it into a file is printed
under it for anybody who wants one.

**Markers 67 to 73 are one request in seven parts, and the order matters.**
The demonstration and the questions page come first because they are what
somebody meets before they have installed anything, and the roadmap page and
its video come next because "what is finished" is a question this project keeps
being asked and keeps answering in a file only a developer reads. The pictures
and the animation follow, because they are polish on something that already
works rather than a claim about what it does.

**Marker 73 is deliberately last, and it is last for a reason rather than by
accident.** An audit run before the code stops moving is an audit of code that
no longer exists. Nine rounds have each found real defects in code a previous
round called clean, and the two most recent found them in code that three
rounds had read: the fuzzing campaign turned up an unbounded Argon2 time cost
and a tamper report that could be made to lie. So the audit goes at the end,
after the last feature and before the deploy, and the estimate is the widest on
this page because the audit's estimate is the number that has historically been
wrong.

**Marker 66 is shipped, and the honest part is what it does not tell you.** The
monitor shows the level going in and the level coming out, on every tab, so the
question "is it still hearing me" is answered without navigating anywhere. What
a level cannot answer is whether the voice has been changed: a working meter and
a bypassed engine draw exactly the same bar. That sentence is printed beside the
meters in both front ends rather than left to be worked out, and the answer to
the question it raises is the preview, which sends the veiled voice to your own
headphones and to nothing else so you can hear that it is not your voice.

**Marker 27 opens the source here rather than sending the reader away, and
what it does not do is guess.** Every box in every flowchart was a link to a
blob on GitHub: correct for a README, which is read on GitHub, and wrong for a
reference page where somebody has chosen a theme and is halfway through a call
graph. There is now a page per file on this site carrying the file, coloured
with the classes the site already uses for code, and a box opens it at the
function it names with the whole function marked.

Three things worth recording. The mark is `:target` in the stylesheet, so it
needs no script and survives a bookmark; a version of this that ran JavaScript
would have been shorter to write and would have marked nothing for a reader
with scripts off, on a site whose no-JavaScript edition is a feature. The mark
covers the function's documentation and attributes as well as its body, because
that is where this project puts its reasons and a reader who clicked a box
labelled `still_named` wants them. And the syntax colouring reads the comments
and literals out of the *same scanner the call graph counts braces with*, so a
keyword inside a comment and a brace inside a comment are one fact read twice
rather than two guesses that can disagree.

It costs 128 pages and about 6.5 MB under `website/reference/`, which the
search index already excludes. That is stated rather than absorbed, because it
is a third of the site again.

**The test count this project states is a number about one machine, and now
says so.** F-77. The same commit measures 996 tests on Windows and 988 on
Linux: nine tests are compiled only on Windows. The number has been generated
rather than typed since F-71, which is what stopped two hand-typed copies
agreeing with each other, and it was still being presented as a fact about the
tree. `docs/MEASURED.md` now records the host it was taken on. What is left is
the wording on the front page, which states one platform's total with nothing
beside it, and that is a change to the page's own voice rather than to a
generator.

**Marker 53 is done for the engine it can be tested on, and says which.** The
mobile half was measured: twelve pages at five widths, with and without
scripts, eight separate causes of horizontal scrolling found and fixed. The
deployed site was checked again at 375 across and the page itself does not
scroll sideways at all; the wide things inside it, tables and code blocks,
scroll within their own boxes, which is the intended design rather than an
accident.

That was Chromium, which is what is available here. **Firefox and WebKit are
still unverified by anybody sitting in front of them.** The stylesheet carries
fallbacks written for both and the suite refuses several constructs that break
on older engines, which is reasoning rather than evidence. This page will not
say those two work until somebody has looked.

**Marker 54's audit rounds are done and its deploy has been happening all
along.** Ten rounds, eighty-four defects, and releases published from tags
with reproducibility checked per platform. There was never a single production
deploy to save up for.

**Marker 65, Failsafe, is the one feature here that is on by default**, and
the reason is the shape of the accident it guards against. You are talking
through VeilVoice, veiled. You plug in a headset. The operating system offers
the new microphone, the calling program takes it, and from that moment your
**real voice** is going out — with the veiled window still open in front of
you, meters still moving, looking exactly as it did a second earlier. Nobody
notices that, because there is nothing to notice. It is not carelessness; it
is a decision the operating system makes on somebody's behalf.

So the default is on, and the default also **closes** the offending program,
because a warning nobody has read yet does not stop a voice going out.

**It notices; it does not prevent, and the difference is printed every time.**
Stopping the operating system handing over a microphone needs exclusive-mode
capture of every input device or a driver, and this project ships neither.
What Failsafe does is see it within about a second and act. That moment is
short and it is not zero, and `CANNOT_PREVENT` says so wherever the feature
appears.

Closing somebody's program is bounded rather than general: never VeilVoice
itself, never a system process, never by name — only the specific process
the watch feed named, and the check is made twice, once when deciding and
again in the only function that acts. Every close is written down, because a
program that vanishes with nothing to explain it is indistinguishable from a
crash.

**Marker 37 is a baseline, and it is named honestly everywhere but its own
title.** `veilvoice-appctl` learns what normally runs here, then tells you when
something runs that was not in that picture. **It does not block anything and
cannot.** Real enforcement needs a kernel driver or a signed system policy and
an application identity to sign it with, and this project is published under a
pseudonym on purpose. Shipping something called "app control" that quietly only
watches would be the exact failure rule 2 exists to prevent, so the scope note
is printed by **every** subcommand — not once at setup, not behind a flag — and
the *unknown* verdict says in so many words that the program is still running.

Three decisions worth recording. **Learning has an end**: a baseline that is
always learning has learned nothing, because whatever an attacker starts joins
the picture the moment it starts. **Grants expire**, and an expired grant is
left on record rather than swept, because "this was allowed until Tuesday" is
worth more to a reader than a row that vanished; permanent is spelled
`forever` rather than a distant date, so choosing it is something somebody
typed. And **only the decisions worth reading are logged** — a line for every
ordinary program every time it is seen is a log nobody reads, and a log nobody
reads is not a control.

Measured on this machine: 111 programs learned from 313 sightings, baseline
closed, and a `check` while a stray process ran named `timeout.exe` and
`smartscreen.exe` — the second one started by Windows itself, which is exactly
the case this is for.

**Marker 41's contrast is computed against the colour actually on screen.**
A translucent card is a colour laid *over* the panel behind it, so measuring
the card's own tint answers a question nobody asked. The blend is computed, the
WCAG ratio is taken against that, the text colour is chosen by measuring every
candidate in the palette rather than assuming black or white — and if nothing
reaches 4.5:1, the card is drawn **opaque** instead of shipped illegible.
Translucency is a nicety; reading a warning is not. The preferences panel shows
the measured ratio and says when it had to give translucency up, rather than
letting a quietly solid card look like a design choice.

The third mode is *off*, and it is offered for a reason: a monitor that
interrupts somebody every thirty seconds is one they switch off at the
operating system, and then it is watching for nothing at all. Better a reader
who chose silence knowingly. What none of the three do is leave VeilVoice's own
window — a system notification needs a registered application identity on two
of the three platforms, and this project is published under a pseudonym on
purpose. That limit is printed beside the setting.

**Markers 28 and 48 were finished by finding out the engine already did it
and nothing asked.** The randomised ratchet range was written, documented and
tested inside `veilvoice-core`, and the doc comment on it said *"the front ends
call this at launch"*. Neither front end did. It was reached by nothing but its
own test for two releases, so every shipped copy rolled the modulation seed on
the same fixed two-second period — a number compiled into the binary,
which is exactly what that sentence said was not the case. Recorded as F-73.

Both front ends now draw a range from the operating system's random source at
launch, and a test reads their source and fails the build if either stops. The
interval is user-configurable through `--reseed-range 250,1800` and a checkbox
in the application, and **anything that is not a usable range is refused with
the reason** — never adjusted to fit, which is marker 28's wording and
the reason the parser returns six distinct refusals rather than a clamp. What
is displayed is the *effective* range, quantised to whole frames, because the
ratchet can only fire on a frame boundary and showing the request would
describe a spread that does not exist.

Measured: three consecutive runs reported 16-69 ms, 773-1963 ms and
1088-1120 ms.

**Marker 35 detects what *can* watch, and says so; it does not claim to detect
keyloggers.** Nothing can. The mechanisms a logger uses are the mechanisms
accessibility software, password managers and remote-support tools use, and
software written to hide is written to hide from a process list too. So
`veilvoice input` names the programs running that are **able** to see keyboard
and mouse, says what each is for, and prints -- with every result, found or not
-- that a clean answer proves nothing. Somebody who reads "nothing found" as
"nothing there" has been made less safe by running it, and that sentence is the
most important thing the crate outputs.

It also does not hook the keyboard to find out. Detecting input monitoring by
monitoring input would make it the thing it warns about, and on Windows it
would need exactly the call `#![forbid(unsafe_code)]` rules out. A test reads
the crate's own source and fails the build if any of those mechanisms appear
in it.

The process listing that both this and screen-capture detection need was
extracted into `veilvoice-proc` rather than copied or borrowed. Depending on
`veilvoice-capture` for it would have meant a keyboard feature pulling in a
table of screen recorders, which is what the note at the top of this section
says these crates must not do.

**Marker 42 shipped half of what it asked for, and the other half is refused.**
This was described here as the most dangerous thing on the list, with three
conditions before anything shipped: what happens when it is typed by mistake,
what "securely erased" is really worth on flash, and what an attacker who
learns the trigger can make it do. Working through those is what decided the
shape.

**The decoy is shipped.** A second passphrase opens VeilVoice with nothing in
it: a way to comply with somebody standing over you without handing over your
recordings. A decoy too close to the real passphrase is refused, because
somebody watching a keyboard would learn both at once and somebody typing
under pressure would give away the wrong one. Both passphrases are derived
with the same Argon2id cost and compared in constant time, and **both are
always derived** even when the first matches: an early return would make the
real one measurably faster and tell an observer with a stopwatch which had
been typed. A copy with no decoy set does the second derivation anyway, so
that having one is not itself detectable.

**The destructive duress passphrase is not shipped, and will not be.** On
flash storage a write does not overwrite: the controller puts the new data in
a fresh physical page and leaves the old one holding the original until it is
collected, which may be never, and no program running as a user can reach it.
This project already refuses to overstate that about its own secure-erase
feature. A destructive passphrase would be believed at exactly the moment
being wrong costs the most: somebody types it, assumes the recordings are
gone, and acts accordingly while the ciphertext is still on the disk. **A
control people rely on and that does not work is worse than no control at
all.** That also answers the first condition: because nothing is destroyed,
typing the decoy by mistake costs a relaunch and nothing else.

**And the decoy does not provide deniability, which is said wherever it
appears.** VeilVoice is open source and this feature is documented, so an
adversary who recognises the program knows it exists and can ask for the other
passphrase. It buys a way to hand something over; it does not buy an argument
that there is nothing more. Anybody who takes it for deniability is worse off
than somebody who never had it, which is why that sentence is the first thing
the feature prints.

---

## Notes on a few of these

**The installer and the portable build are the same binary.** One executable
that opens a window when it is double-clicked and takes subcommands when it is
given them, and which does not care whether it was installed or unzipped. An
installer that produces a *different* program from the portable download is two
things to test, two things to sign, and two things to get subtly out of step.

**The companion setup asks, every time, and never assumes.** VB-CABLE is
proprietary donationware, Audacity is somebody else's software, and PipeWire is
part of the user's operating system. So each is: detect whether it is already
there, say what it is and who makes it, and install only on an explicit yes.
Never silently, never ticked by default. That rule predates this roadmap -- it
is the same one the existing install scripts follow -- and it does not relax
because the interface got prettier.

**"Documented in the wiki" is a build step, not a promise.** Every page of the
reference is generated from the source by `tools/docs/generate.py`, and CI
fails if the tree and the documentation disagree. Installer documentation goes
through the same route, so it cannot drift from what the installer does.

---

## What was dropped, and when

A roadmap that quietly loses items is a roadmap nobody can trust, so removals
are recorded here rather than edited out of history.

- **USB device allowlisting, and BadUSB keystroke-timing defence.** Removed
  2026-08-19 at the maintainer's request, before any code was written. Nothing
  in the tree depended on them.

---

## How to read the estimates

They assume one person, working days, and no interruptions — so the calendar
will be longer than the sum. They also assume the work is done the way the rest
of this project is: tests that can express the bug, documentation generated
from the source rather than written beside it, and a claim in a document only
after the thing it describes actually works.

The number that has historically been wrong is the audit. Four rounds have each
found real defects in code a previous round called clean, and each round was
larger than the one before. The audit's estimate is what the work is expected
to take. It is not a promise that it will find nothing.
