<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice — roadmap

**What is built, what is coming, and roughly when.** One marker is one feature:
written, tested, documented and merged. A marker is not ticked because the code
compiles — it is ticked when the thing works, has tests, has documentation, and
survives the checks in CI.

Estimates are in working days and they are estimates. Where a marker depends on
something outside this project — a platform's rules, a decision that has not
been taken — that is written down rather than absorbed into a number.

**Where we are now:** **v0.1.10 is released**, signed and published for ten
platforms, and verified by hand after publication -- fingerprint checked, good
signature, hashes matched, and the shipped verifier checked its own release
with no GnuPG involved. Everything below the line marked *shipped* is work in
progress.

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
| 22 | Website split into a page per section, every published link still working | **planned** | 2 d |
| 23 | Motion and polish — smooth loading and scrolling, hover, CSS-first tooltips | **done** | — |
| 24 | Demonstration animation: a voice going in, the mark lighting up, an unidentifiable wave coming out | **done** | — |
| 25 | Cycling line of project facts, slow enough to read — CSS rather than an image, so it follows the reader's theme and needs no script | **done** | — |
| 26 | Every website theme in the app, plus user-defined palettes with contrast computed rather than assumed | **done** | — |
| 27 | Interactive workflow diagrams that open the relevant source, highlighted, in the site's palette | **planned** | 3–4 d |
| 28 | Randomised, user-configurable ratchet interval, with invalid input refused rather than clamped | **planned** | 1–2 d |
| 29 | One single binary — the same executable runs as the desktop app or as the command line, installed or portable | **planned** | 2 d |
| 30 | Windows installer: Tokyo Night, animated, and a **portable** option that installs nothing | **planned** | 4–5 d |
| 31 | Optional companion setup — VB-CABLE on Windows, PipeWire on Linux, BlackHole on macOS, and Audacity everywhere — detected if present, installed only if confirmed | **planned** | 3–4 d |
| 32 | The site's search presented as an **index**, and animated | **done** | — |

## Security and monitoring features

Each of these is a crate of its own, so that another project can depend on one
without taking all of them. Every one is **opt-in**, and every one states what
it cannot do as plainly as what it can.

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 33 | Screen-capture detection — application name, PID, when, how often | **planned** | 3 d |
| 34 | Hide VeilVoice's own window from screen capture and recording | **planned** | 1–2 d |
| 35 | Keyboard and mouse activity monitoring, reported as the heuristic it is | **planned** | 2–3 d |
| 36 | `veilvoice-sentry` — ransomware canaries and mass-change rate detection | **planned** | 3–4 d |
| 37 | `veilvoice-appctl` — learn what runs, then allowlist it, with time-limited grants and a log | **planned** | 5–7 d |
| 38 | `veilvoice-policy` — settings sealed with the existing post-quantum cryptography | **planned** | 2–3 d |
| 39 | Privileged mode: an opt-in service, and an elevated no-service mode, with the difference visible to the user | **planned** | 5–7 d |
| 40 | Alert on driver and kernel-module installation; cross-view checks | **planned** | 3–4 d |
| 41 | Notification overlay — rounded, translucent, contrast computed, or an alert, or off | **planned** | 2 d |
| 42 | Duress and decoy passwords | **planned** | 7–10 d |
| 43 | Cloud transcription through your own API key | **blocked** | — |

## Finally

| # | Marker | Status | Estimate |
|---:|---|---|---|
| 44 | Fifth audit round — every vulnerability class across the tree, twelve findings written up individually (F-48 to F-59) | **done** | — |
| 45 | **v0.1.10 released** — ten platforms, signed, and verified by hand after publication | **done** | — |

---

## The things that are not just work

Three of the markers above depend on something other than effort, and
pretending otherwise would make this roadmap a wish list. They are named rather
than numbered, because a number changes whenever a row above it does -- which is
exactly what happened when the USB work was dropped from this list.

**Cloud transcription is blocked on a decision, not on code.**
VeilVoice currently talks to no servers at all, and CI fails the build if a
network client appears anywhere in the dependency graph. That is one of the few
claims a reader can verify in ten seconds, and it is a large part of why this
project is worth trusting. Transcribing through a provider means an HTTP client
exists somewhere. It can be done honestly — one crate, off by default, the
guarantee kept everywhere else and the claim reworded everywhere it appears —
but it is a real trade and it is not made silently. Note also that not every
named provider accepts audio input; that will be checked before anything is
built rather than discovered by a user.

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

**Duress and decoy passwords are the most dangerous thing on this list.** A duress password
destroys data on purpose, and a decoy system exists to be believed. Neither is
shipped until the failure modes are handled: what happens when it is typed by
mistake, what "securely erased" is actually worth on flash storage (the answer
is *less than people think*, and this project already documents that), and what
an attacker who learns the trigger can make it do. It is off by default, it
takes a deliberate setup, and it will carry the plainest warnings in the
project.

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
