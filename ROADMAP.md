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
| 63 | **Live levels and a wave per speaker**, in the app and in the terminal, while a recording is running | **blocked** | — |
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
| 74 | **The lock screen tells an attacker nothing** — no explanation of what the lock is or is not worth while it is locked, the account of that moved to the documentation and to the unlocked application, and a small animation in its place | **next** | 1 d |
| 75 | **`veilvoice-guard` inside the desktop application** — the integrity record taken at install and checked at every launch, sealed with the existing cryptography rather than left in the clear | **planned** | 2–3 d |
| 76 | **The app lock, hardened as far as it honestly goes** — the file stored where only an administrator can write it, its contents and its name randomised, a tamper check on every read, restoration from the sealed copy when it fails, and an alert that does not go away until the tamper passphrase is given | **planned** | 4–5 d |
| 77 | **What the lock is worth, written down properly** — one account, in the documentation, covering what the hardening buys and what it does not | **planned** | 1 d |
| 78 | **Every website palette in the application, chosen from the interface** | **planned** | 1 d |
| 79 | **A window that does not stutter** — the interface measured rather than described, every task off the drawing thread, and the smallest amount of code that does it | **planned** | 3–4 d |

## Finally

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 44 | Fifth audit round — every vulnerability class across the tree, twelve findings written up individually (F-48 to F-59) | **done** | — |
| 45 | **v0.1.10 released** — ten platforms, signed, and verified by hand after publication | **done** | — |

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

**Marker 63 is half shipped and half blocked, and it moves to blocked rather
than sitting as planned.** The *levels* are done and have been for some time:
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

**Markers 74 to 79, and the two places they meet a rule this project already
has.**

*Storing the lock where only an administrator can write it* is worth having and
it runs into marker 39's decision, which is that VeilVoice **never acquires
privilege**: it does not re-launch itself elevated, install a service, or ask
for a password, because those are changes to somebody's machine and they belong
to the person whose machine it is. The shape that keeps both: when VeilVoice is
*already* running with administrator rights it writes the lock somewhere only
an administrator can, and when it is not it says so, prints the one command
that would move it there, and carries on with the file it can write. What it
will not do is prompt for elevation on its own.

*Hiding the file's name and contents* is obscurity, and obscurity is not a
security property. Randomising where the lock lives and what it looks like
raises the cost for somebody poking around and stops nothing that a determined
local attacker with a debugger will do. It is worth doing for the first case
and it must not be described as protection against the second. Marker 77 is
that sentence, written where a user reads it, and marker 74 is the decision
that the *lock screen* is not the place for it: telling somebody standing at a
locked window what the lock cannot do is helping the one person who should not
be told.

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
