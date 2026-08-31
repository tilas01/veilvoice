<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Installing VeilVoice

There are three ways to get VeilVoice, and they differ in who is doing the
checking rather than in what you end up with.

| | You run | The checking is done by |
|---|---|---|
| [By hand](#1-by-hand) | four commands | **you**, and you can see each one |
| [With the install script](#2-with-the-install-script) | one command | the script, which refuses if anything fails |
| [With the portable verifier](#2b-with-the-portable-verifier) | one command, no GnuPG needed | a binary carrying the key |
| [From source](#3-from-source) | `cargo build` | the compiler, plus whatever you read |

The by-hand route is first on purpose. The install script does exactly what it
describes and nothing else, but "run this script and trust it" is a strange
thing to ask on behalf of a tool whose entire argument is that you should not
have to trust anybody. If you only ever read one section here, read that one.

**Status of this document.** The install scripts and the portable verifier have
been written and tested end to end on Windows against the real published v0.1.8
release -- the verifier checks that release's actual OpenPGP signature with no
GnuPG installed -- and the by-hand chain has been checked on the same release.
They have **not** yet been run by anyone other than the author, nor on a machine
that did not build them. Until that has happened they should be treated as working but
unproven — see [What is not finished](#what-is-not-finished).

---

## The fingerprint

Everything below rests on one value:

```
8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A
```

That is the OpenPGP key VeilVoice releases are signed with. It is published in
[`README.md`](../README.md), on
[the website](https://tilas01.github.io/veilvoice/), in the wiki, and in every
release's notes, and it is **hardcoded in the install scripts** rather than
fetched — a fingerprint you download alongside the thing it is meant to
authenticate is not a check, it is a formality.

The key's user ID is exactly `tilas01`, with no e-mail address attached.

If the fingerprint you see anywhere disagrees with the one above, stop.

---

## 1. By hand

Four commands, on any platform with GnuPG and a SHA-256 tool. Replace
`v0.1.9` and the archive name with the release and build you want; the
[releases page](https://github.com/tilas01/veilvoice/releases) lists them.

```bash
# 1. Get the key, and check its fingerprint against the value above.
curl -fsSLO https://tilas01.github.io/veilvoice/assets/veilvoice-signing-key.asc
gpg --import veilvoice-signing-key.asc
gpg --fingerprint tilas01
```

Compare what that prints against the fingerprint above, **character by
character**. This is the only step that anchors any of the others, and it is
the one step nothing can do for you.

```bash
# 2. Get the release, the hash list, and the signature over it.
V=v0.1.9
B=https://github.com/tilas01/veilvoice/releases/download/$V
curl -fsSLO $B/veilvoice-$V-linux-x86_64.tar.gz
curl -fsSLO $B/SHA256SUMS
curl -fsSLO $B/SHA256SUMS.asc

# 3. Verify the signature over the hash list.
gpg --verify SHA256SUMS.asc SHA256SUMS

# 4. Verify the download against the now-trusted hash list.
sha256sum -c SHA256SUMS --ignore-missing
```

On macOS use `shasum -a 256 -c SHA256SUMS --ignore-missing`. On Windows, in
PowerShell:

```powershell
Get-FileHash -Algorithm SHA256 .\veilvoice-v0.1.9-windows-x86_64.zip
# then compare that hash against the matching line in SHA256SUMS
```

### Why that order, and not the other one

Step 3 before step 4, always.

Checking the hash first proves that your download matches a list. It does not
prove anything about the list. Whoever could replace the download could replace
`SHA256SUMS` beside it, and the two would agree perfectly. The signature is the
only thing that makes the hash list worth comparing against, and the
fingerprint is the only thing that makes the signature worth checking.

A "good signature" warning about the key not being certified is expected and is
not a problem:

```
gpg: Good signature from "tilas01" [unknown]
gpg: WARNING: This key is not certified with a trusted signature!
```

That says GnuPG has no web-of-trust path to the key — which is true, and is why
you compared the fingerprint yourself in step 1. What matters is
`Good signature`. `BAD signature` means stop.

---

## 2. With the install script

The scripts live in [`install/`](../install/). Each one downloads the release,
performs exactly the checks above in exactly that order, and **refuses, naming
the check that failed**, rather than continuing past anything it could not
verify. None of them has a flag to skip verification, because an installer with
one is an installer whose verification is decorative.

### Linux and macOS

```bash
curl -fsSLO https://raw.githubusercontent.com/tilas01/veilvoice/main/install/install.sh
less install.sh          # it is 400 lines and it is meant to be read
sh install.sh
```

| Option | Effect |
|---|---|
| `--yes` | no prompts, and **no** optional components |
| `--version v0.1.9` | a specific release rather than the latest |
| `--prefix ~/.local` | where to install (default `~/.local`) |
| `--with-audacity` | install Audacity too |
| `--with-gpg` | install GnuPG if it is missing |

### Windows

```powershell
irm https://raw.githubusercontent.com/tilas01/veilvoice/main/install/install.ps1 -OutFile install.ps1
notepad install.ps1      # read it first
powershell -ExecutionPolicy Bypass -File install.ps1
```

`install.bat` is a wrapper for people who would rather double-click; it passes
its arguments straight through to `install.ps1` and contains no logic of its
own, deliberately — two implementations of a verification routine means one of
them is the stale one, and the stale one is the one that will be running when
it matters.

| Option | Effect |
|---|---|
| `-Yes` | no prompts, and **no** optional components |
| `-Version v0.1.9` | a specific release |
| `-Prefix "D:\Tools\VeilVoice"` | where to install |
| `-WithVBCable` | open the VB-CABLE download page |
| `-WithAudacity` | install Audacity through winget |

### The optional extras, and why they are questions

VeilVoice needs none of them. Each is offered **once**, as a question that
**defaults to no**, and `--yes` / `-Yes` installs none of them at all: `--yes`
means "do not ask me", and answering an unasked question by installing software
on somebody's machine is precisely the behaviour that makes install scripts
untrustworthy.

- **VB-CABLE** (Windows) is what lets live mode feed a veiled microphone into a
  call. It is **proprietary donationware** by VB-Audio, not free software. The
  script only *opens their download page* — it will not silently fetch and run
  a third-party installer, which would be a strange thing to do inside a script
  whose whole subject is verifying what you run.
- **Audacity** is a free audio editor, useful for recording and trimming before
  veiling. It is not bundled because it is GPL-2.0-or-later, which cannot be
  combined with this project's GPL-3.0-or-later.
- **GnuPG**, where missing, because without it the signature cannot be checked
  at all. If you decline it, the script stops rather than falling back to
  "the hash matched" — a hash checked against an unverified list is not a
  security check.

### What a refusal looks like

```
REFUSED: the signing key's fingerprint does not match
  expected  8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A
  found     1234567890ABCDEF1234567890ABCDEF12345678

  This is the check that anchors every other one, so nothing further
  was attempted.

Nothing has been installed.
```

Every refusal says which check failed and installs nothing. There is no
partial state to clean up: the download lands in a temporary directory that is
removed on exit, and nothing is copied anywhere until every check has passed.

---

## 2b. With the portable verifier

`veilvoice-verify` ships in every release archive. It is one binary that does
the same checks as GnuPG, with **nothing else installed** -- the signing key and
its fingerprint are compiled into it. It downloads nothing.

### The short way

Put it in the folder you downloaded to and run it. That is the whole
instruction.

```bash
veilvoice-verify
```

It finds the release near it and checks all of it, in this order, each step
only if the one before it passed:

1. the signature over `SHA256SUMS`;
2. every archive, against `SHA256SUMS`;
3. `CONTENTS.sha256`, against `SHA256SUMS`;
4. **every file you extracted**, against `CONTENTS.sha256`, naming anything in
   that folder the release never published;
5. all of it again through the GnuPG on your machine, if you have one.

Step 4 is the one worth having and it is new in v0.1.15. A hash over the
archive tells you the *zip* is genuine. This tells you the *program you are
about to run* is, which is the question anybody actually has. Nothing on disk
records which archive a folder was extracted from, so a release now publishes
`CONTENTS.sha256` listing every file inside every archive with its SHA-256,
staged before `SHA256SUMS` is computed so the signature covers it too:

```text
SHA256SUMS.asc -> SHA256SUMS -> CONTENTS.sha256 -> each file on disk
```

Releases before v0.1.15 carry no contents list and are checked as far as step
2, which the tool says at the time rather than implying more.

Step 5 adds the VeilVoice public key to your keyring, tells you it did and how
to remove it in one command, and runs `gpg --verify`. The signature is then
checked by two independent implementations and the run fails if they disagree.
A GnuPG that cannot run on your machine is **not** counted against the
download: that is a fact about the computer and says nothing about the file.

The commands in section 2 above are still printed every time, because running
GnuPG from inside the program you are checking makes the *implementation*
independent and only you typing them makes the *invocation* independent.

### The long way, one file at a time

```bash
veilvoice-verify key
    # prints the fingerprint it carries. Compare it against the one above.

veilvoice-verify file veilvoice-v0.1.9-linux-x86_64.tar.gz     --sums SHA256SUMS --sig SHA256SUMS.asc
```

### The one thing it cannot carry

It cannot embed the expected hash of the file it is checking: a file cannot
contain its own digest, because writing the digest in changes the file. So the
hash comes from outside, and **where it came from decides what a match proves**.
The tool keeps these apart and refuses to run both at once and report one
answer.

| Hash from | Proves | Rests on trusting |
|---|---|---|
| the published `SHA256SUMS` | the download is **intact** | whoever signed the release |
| somebody else's own build of the same tag | the release is **reproducible** | nobody in particular |

The first is what most people want. The second is what makes the first worth
anything: it closes the gap that a signed binary could contain something the
source does not. It needs a build by somebody who is not the author, which is
why this project cannot perform it for you.

```bash
# the stronger check, once somebody else has published a hash from their build
veilvoice-verify file veilvoice-v0.1.9-linux-x86_64.tar.gz --sha256 <their hash>

veilvoice-verify --explain     # the difference, at length
```

VeilVoice's own releases are built twice, in separate directories, and compared
before they ship. That is the publisher checking their own work -- worth
something, and not the same as somebody else checking it.

### Why it depends on a large library

`veilvoice-verify` uses [`pgp`](https://crates.io/crates/pgp) (rPGP), a pure-Rust
OpenPGP implementation, and it is by far the largest dependency in this project.
That cost was weighed rather than ignored. The alternative was hand-writing
OpenPGP packet parsing and RSA PKCS#1 v1.5 verification, and a subtle mistake
there would be a **silent accept** in the one tool whose entire job is not to
silently accept. A widely used implementation that many people read is the
better risk.

One consequence is recorded honestly rather than buried: that library brings in
the `rsa` crate, which carries
[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071) (the Marvin
attack) with no fixed version available. The advisory concerns RSA **private
key** operations. This tool only ever verifies a signature against a public key
compiled into it -- there is no private key anywhere in this repository, and no
secret for a timing side channel to leak. That reasoning is written out in
`.cargo/audit.toml`, and because it is an argument about *usage* rather than
about the crate, CI fails the build if a secret-key or decryption API ever
appears in the verifier.

---

## 3. From source

A fresh clone needs no secrets and no configuration.

```bash
git clone https://github.com/tilas01/veilvoice
cd veilvoice
cargo build --release --workspace
```

The binaries land in `target/release/`. `cargo run -p veilvoice-cli -- info`
reports what the build supports.

If you want a binary you can compare against the published one, see
[REPRODUCIBLE_BUILDS.md](REPRODUCIBLE_BUILDS.md).

---

## 4. After you have it: portable, or installed

Once VeilVoice is unpacked it runs. Nothing has to be installed, nothing is
written outside the folder, and deleting the folder removes it. **Portable is
the normal case**, not a lesser one.

Installing exists for one reason: so that typing `veilvoice` in a terminal
works. It is per-user, needs no administrator, and is reversed exactly.

```
veilvoice install          # copy, add to PATH, register for removal
veilvoice install --status # what is installed, and which copy is running
veilvoice uninstall --yes  # undo exactly those three things
```

The desktop application has the same thing on its **install** tab, with the
list of what will change printed beside the button. Both call the same code --
`veilvoice-setup` -- rather than each having its own idea of how to edit
`PATH`. A `PATH` edit is the one operation here that can damage a machine, so
there is one implementation of it and one set of tests over it.

"Installed" and "you are running the installed copy" are reported separately,
because they are different facts and confusing them is how somebody edits a
portable folder and wonders why the installed one did not change.

### Companion software

Four programs make VeilVoice easier to live with. **None of them is part of
VeilVoice and none of them is required.**

| | What it is | Who makes it | Licence |
|---|---|---|---|
| VB-CABLE | a virtual audio cable for Windows | VB-Audio Software | proprietary donationware |
| BlackHole | the same, for macOS | Existential Audio | MIT |
| PipeWire | the audio server most Linux distributions already run | the PipeWire project | MIT |
| Audacity | a free audio editor and recorder | the Audacity team | GPL-2.0-or-later |

A virtual cable is what lets live mode feed a veiled microphone into a call.
Without one live mode still runs; you simply have nowhere useful to send it.
Audacity is a convenience for recording and trimming, and is **recommended,
never embedded** -- GPL-2.0-or-later cannot be combined with this project's
GPL-3.0-or-later.

```
veilvoice companions                      # report only: what is here, and what is not
veilvoice companions --install audacity   # the explicit yes, one named program
```

The same list is on the desktop application's install tab, one row each.

Three rules, and they are enforced in the shared library rather than in each
front end, so neither can be more permissive than the other:

- **Nothing is ticked, because there is nothing to tick.** There is no
  "install recommended extras" control, because that is the control through
  which unwanted software has historically arrived.
- **VeilVoice never runs somebody else's installer.** VB-CABLE is proprietary
  and is a driver, so what is offered is to open VB-Audio's page -- their
  licence for you to accept, their installer for you to run. Fetching and
  executing an unverified third-party binary would be a strange thing for a
  program whose whole subject is verifying what you run.
- **Privilege is reported, never requested.** A package manager that needs
  root has its command printed for you to run in a terminal. Neither front end
  will ask you for a `sudo` password.

Detection has three answers and not two: found (with the path it was found
at), not found where it usually installs, or could not tell and here is why.
The middle answer is a statement about where VeilVoice looked, not a claim
about your machine -- if you keep Audacity somewhere unusual it will say "not
found", and that is exactly what the words mean.

---

## Where things get installed

| Platform | Default location |
|---|---|
| Linux, macOS | `~/.local/bin` |
| Windows | `%LOCALAPPDATA%\Programs\VeilVoice` |

Both are per-user and need no administrator or `sudo`. Nothing is written
outside them, no service is installed, no registry key is created beyond the
user `PATH` entry on Windows, and nothing runs at startup.

To uninstall, run `veilvoice uninstall --yes`, or use the desktop
application's install tab, or simply delete that directory -- the first two
also remove the `PATH` entry and the Apps & features registration, which
deleting the directory does not. VeilVoice keeps its own configuration under
the usual per-user location for the platform; `veilvoice lock status` names
the lock file's path if you have set one.

---

## Known rough edges

**Git for Windows' bundled GnuPG has a path-length limit.** It is an MSYS
build, and its agent puts a Unix-domain socket inside the home directory it is
given, which cannot exceed about 108 bytes. `install.ps1` keeps its temporary
directory short for that reason, and says so plainly if your `TEMP` is long
enough that even a short name overflows. Gpg4win is a native build and has no
such limit.

**`~/.local/bin` may not be on your `PATH`.** The script says so if it is not,
and prints the line to add.

**macOS Gatekeeper.** The binaries are not notarised — notarisation requires an
Apple Developer account, which requires a legal identity, which this project
does not have. macOS will refuse to run them until you allow it explicitly in
*System Settings → Privacy & Security*. This is stated rather than worked
around: a project that publishes under a pseudonym cannot also be notarised,
and pretending otherwise would be worse than the inconvenience.

---

## What is not finished

Recorded here rather than left for you to discover:

- **Nobody but the author has run these scripts.** They are tested end to end
  on Windows against the real v0.1.8 release, and the by-hand chain is verified
  on the same release. That is not the same as having been run on a machine
  that did not build them, and until it has been, treat "it works" as a claim
  with one source.
- **`install.sh` has now been run on Linux, and not on macOS.** On
  x86-64 Linux it was run end to end against the published v0.1.14 release: it
  found the latest tag, downloaded the archive, checked the key's fingerprint,
  verified the signature over `SHA256SUMS`, matched the archive's hash against
  the signed list, installed both binaries, and the installed `veilvoice info`
  ran. Its refusals were exercised on the same machine: an unknown option, and
  a version that is not published, both refusing with a status of 1.

  Running it found one defect, F-79, in what it says rather than in what it
  checks. macOS is still unrun, and its `sh` is not Linux's.
- **The packaged installers (WiX, `.deb`, `.rpm`, Flatpak, Homebrew), the
  OpenBSD and NetBSD builds and the Gentoo ebuild are not built yet.** They are
  specified in [`ROADMAP.md`](../ROADMAP.md). macOS Intel and Apple Silicon are already
  separate builds, and a single Windows executable already covers 10 and 11.
- **The portable verifier exists and is tested**, including against the real
  published v0.1.8 signature, but like the scripts it has only been run by its
  author.

If you run these on a machine that did not build them, saying so in an issue is
genuinely the most useful thing you could contribute.
