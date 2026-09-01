<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Changelog

The section matching a release tag is published at the top of that release's
notes on GitHub, so this file is the source of truth for what changed rather
than a summary written afterwards.

## Unreleased

Nothing yet. The next release goes here.

## v0.1.16

A verifier that finds the release you are standing in and no longer calls a
genuine one unverified, buttons that line up with the buttons beside them,
pictures that grow rather than cutting their words, no em dash left anywhere
in the repository, and the audit's own arithmetic checked by a machine
instead of by hand.

### The verifier finds the release you are standing in

- **F-107** Running `veilvoice-verify` with no arguments from inside an
  extracted release answered "no VeilVoice release was found to check". Every
  archive tool unpacks into a folder beside the archive, so the folder somebody
  opens holds the binaries and none of what they are checked against:
  `SHA256SUMS`, its signature and the archive are one level above. The search
  looked in the current directory, the program's own, Downloads and Desktop,
  and in the parent of none of them, while the comment above it said "where
  somebody who unpacked the archive and ran the verifier inside it will be".
  It now looks one level up from each, and one level only: a verifier that
  climbs a stranger's filesystem looking for something to check is worse than
  one that asks. Checked against the published v0.1.15 end to end, including
  341 of 341 extracted files and the machine's own GnuPG.

- **F-108** `veilvoice-verify auto /no/such/place` checked a different
  directory, printed INTACT and exited 0, without the path that was typed
  appearing anywhere in the output. The fallback through the current
  directory, Downloads and Desktop is right when nobody said where to look and
  wrong the moment somebody does. The exit status is what makes it more than a
  nuisance: `veilvoice-verify auto "$DIR" || exit 1` is the obvious way to
  script this, and a typo or an unset variable produced a green result about
  whatever happened to be lying around. A named directory is now checked
  before anything is searched, and a refusal names the path and says nothing
  was checked.

- **F-109** `veilvoice anonymise recording.wav` from a script, a scheduled job
  or anything with its input redirected failed with `No such device or address
  (os error 6)`. That is the operating system's word for "there is no console",
  and it names nothing that was wanted and none of the ways on. VeilVoice
  encrypts at rest by default, so it wants a passphrase and there is nowhere to
  ask. Both prompts now check for a terminal first and explain: `--encrypt-to`
  with a public key, which types nothing and is the one that works in a script;
  a terminal, if somebody is there; or `--encrypt false --yes`, described as
  what it is. Checking first rather than reporting the failure afterwards keeps
  the message the same on Windows.

### Buttons line up with the buttons beside them

The desktop application padded its passphrase labels with trailing spaces to
fake a column, which lines nothing up in a proportional font. The screens that
drifted worst were the ones drawn only after setup, because those carry the
labels that needed the most padding, and the buttons under them inherited it.
Each label now gets a real column and every field starts in the same place.

The unlock screen drew its mark and headings centred and its password row hard
left. Nesting the row in a centred layout does not fix that, because a row
allocates the whole width and there is nothing left to centre it within, so it
is measured on one frame and placed on the next.

On the website `.row`, which holds the download buttons on three pages, was a
flex container that never said how its children line up and so fell back to
`stretch`: invisible until one label wraps on a narrow screen and makes every
button beside it taller.

**A suite checks this for new features.** It reads the pages to find what
actually holds a button, rather than keeping a list that would go stale on the
next feature, and fails any flex container of buttons that has not decided.

### Pictures grow rather than cutting their words

A crate or file banner whose description ran past two lines used to end in an
ellipsis, which reads as a summary and is a sentence that stopped: one crate's
banner said what it notices and not what it does about it. Banners are now
sized from the lines they need, and banners that already fitted are unchanged
byte for byte.

The terminal captures had the same cap at 44 lines, so pictures of `--help`
ended three flags early. The canvas was already sized from the content, so
removing the cap simply makes them taller. No generated drawing ends in an
ellipsis now.

### Screenshots are rounded in the file, not in the page

The application draws a rounded window and a capture is a rectangle. The
corners are now rounded into the alpha channel rather than by CSS, because the
README is rendered by GitHub, which strips styles from images, and the release
archives carry the same files: a picture that is round in one of three places
is not round. Only corner alpha changes, with the arc antialiased from
coverage, so the picture stays the pixels the application drew.

The page then had to stop drawing them a second time. Three rules rounded these
images and one painted a background behind them, which showed through the
corners the file had just made transparent.

**No mouse pointer, established by reading the capture script rather than the
pixels.** Captures use `PrintWindow`, which asks the window to draw itself; a
pointer is drawn by the compositor over the screen and is never part of a
window's own rendering, so such a capture cannot contain one. The suite checks
the capture method, and fails if it is ever swapped for a screen copy.

### The twenty-first audit round: 2.19 billion inputs

All seven fuzzing targets at twenty minutes each, twice the length of any
previous run and the first to include `release_contents`. **2,127,796,269
inputs, no crash, no hang, no out-of-memory.** The one new artefact was a slow
unit declaring 569 MiB, ten passes and 127 lanes, legal on every axis against
ceilings of 4 GiB, sixteen and 0x00ffffff, which is the class F-91 bounded.

Doubling the time found nothing new. That is evidence the targets are converged
on the structure they can reach, and is not evidence the parsers are correct:
three of the seven have still never found anything, five have no committed
corpus, and none of it has run on Windows or macOS.

### The eighteenth audit round, run against the published v0.1.15

v0.1.15 was downloaded, extracted and checked with the verifier that ships
inside it. Not a fixture: the artefacts on the release page.

- **F-104** The GnuPG half called a correctly signed release **unverified**, in
  the strongest words the program has, for every reader with GnuPG installed.
  The release is signed by the signing subkey of the VeilVoice key, as most
  keys sign; GnuPG names the signing key first and the primary key last, and
  only the first was read. The key it had been measured against signs with
  itself, so the two fingerprints were the same string and the mistake was
  invisible in every test. Both are compared now, and the test carries the line
  GnuPG actually printed for v0.1.15. The same run proved the rest: all 341
  files in the extracted release matched the signed contents list, with nothing
  else in the folder.

### No em dash anywhere, in any encoding

Fifty in `veilvoice-cli` and four elsewhere that reach a user went first, from
`--help` output, warnings, errors and printed results, with the ten committed
CLI screenshots re-captured from the built binary so the gallery shows what the
program prints.

The rest followed: every `//!` and `///` doc comment in all twenty-seven
crates, the Markdown, the workflows, the site tests, the build files, the
generators and the hand-written website pages. Every one is a rewritten
sentence rather than a swapped character, because a dash carrying a "because"
or a "so" leaves a comma splice behind when it is simply deleted.

**The remainder this file stated last was measured with the wrong instrument.**
"349 remain" was accurate over every file `grep -r` reaches, and blind to the
roughly 2,800 `&mdash;` and `&#8212;` in the website, which no search for the
character had ever looked for. A number is only as honest as the search that
produced it.

Three places keep the character on purpose, because all three read it as input
rather than writing it: the roff translation table, the roadmap parser's
spelling of an empty estimate, and the Markdown renderer's entity decoder.

Eight comma splices were introduced by the sweep and caught before release,
seven by reading the rendered website and one by grepping the diff for a comma
followed by a new independent clause. Both passes were needed: the site suites
pass on a splice, and the mechanical check found what reading had missed.

### The nineteenth and twentieth audit rounds

- **F-105** The audit's verdict claimed one hundred and four defects and then
  broke them down into sixty-five. The headline had been maintained every
  round; the breakdown under it had not been touched in eleven, so every
  conclusion in the document's most quotable paragraph was a seventh-round
  conclusion written in the present tense. The breakdown is gone rather than
  corrected, because a corrected one drifts again for the same reason.
- **F-93 and F-94 were fixed in code and never written up.** Both were real,
  both were described in full in the commit that fixed them, and neither had an
  entry, so the finding numbers ran to 104 with a hole at 94 and nothing said
  so. Writing them up forced an honest qualification of this project's
  "no confidentiality failure" claim, which had never mentioned the two
  encrypted-volume defects; the claim stands as defined, and the audit is now
  explicit about which side of the line they fall on.
- **F-106** A test deleted another test's fixture. Two tests take a scratch
  GnuPG home and each removes it when it finishes, and the helper named that
  directory after the clock alone. Two threads reading the same tick get the
  same path, `create_dir_all` reports success because the directory is already
  there, and the faster test's cleanup destroyed the slower one's keyring
  mid-run. Impossible on Linux, where the nanoseconds always differ, and it
  happened on macOS, where the clock is coarser. Caught by CI on a commit that
  changed no Rust at all, which is the honest signature of a defect that was
  always there.

### The audit's arithmetic is measured now

`docs/MEASURED.md` records how many findings the audit writes up and the
highest number it hands out, read from the document's own headings. They agree
exactly when no number has been skipped, so F-94's absence would have shown as
103 against 104 rather than as nothing.

A new site suite checks the README's count and range and the verdict's count
and range against those measured numbers, never against each other: that is
F-71, where a guard compared one hand-typed claim to another and passed while
both were wrong. Its four failure modes were tested by reintroducing them.

### The seventeenth audit round, run on the screenshots

- **F-103** Nothing said when a committed screenshot had gone stale. The check
  compared each drawing against the text file beside it and compared that file
  against nothing: it is written by a separate `--capture` command that the
  verification run does not call, so a string could be rewritten and every
  check in the repository would pass while the website went on showing the old
  wording. Found the only way this kind of thing is: the interface text was
  rewritten, everything passed, and the help screenshot still contained a dash
  the program no longer prints. The check now runs the commands and compares
  what they print against what is committed. Every capture is a `--help`
  screen, so its output depends on the binary and not on the machine.

## v0.1.15

Encrypted volumes, one password if you want one, a window that locks itself,
and an interview from an OBS recording through to a video.

### A verifier anybody can use, checking everything

**One press, or one command, and it checks the lot.** `veilvoice-verify` and
the desktop application's verify tab now check the signature over the hash
list, then the archive against that list, then **every file you extracted out
of the archive**, and then ask the GnuPG on your own machine the same question
and show you what it said.

**The extracted folder is now checked, not described.** A release publishes
`CONTENTS.sha256`, listing every file inside every archive with its SHA-256. It
is staged before `SHA256SUMS` is computed, so the hash list covers it and the
signature covers it too, and the chain runs all the way down:

    SHA256SUMS.asc -> SHA256SUMS -> CONTENTS.sha256 -> each file on disk

That answers the question somebody actually has. "This zip is the published
one" is a step; "the program I am about to run is the published one" is the
thing worth knowing, and until now nothing on disk recorded which archive a
folder came out of, so the honest report was two separate answers and the
advice to unzip it again. Anything in that folder the release never published
is named as well, because a folder holding every correct file plus one extra
program passes every other check and is not the release.

Releases before v0.1.15 carry no such list and are checked as far as the
archive, which the tool says at the time rather than implying more.

**Your own GnuPG, run for you.** If `gpg` is on the machine, VeilVoice adds the
signing key to your keyring, tells you it did and how to remove it in one
command, runs `gpg --verify`, and reports the answer. The signature is then
checked by two independent implementations, and where they disagree the run
fails. GnuPG's machine-readable status channel is what is read, never its
prose, so a GnuPG in any language gives the same answer. A good signature by
some *other* key is a failure rather than a pass, which is the check the
instructions have always asked people to remember to make.

A GnuPG that cannot run on your machine is not counted against the download,
and is not drawn in red: that is a fact about the computer, and reporting it as
a refusal would tell somebody not to run a release that is perfectly sound.

The commands are still printed every time. Running GnuPG from inside the
program under suspicion makes the *implementation* independent; only you typing
them makes the *invocation* independent.

### Smaller things

- The release workflow's `tag` input was declared and ignored. A dry run
  started from a branch named every archive after that branch and would have
  tried to publish a release called `main`. It now checks out the tag it was
  given, names everything after it, and publishes nothing at all on a manual
  run, which is what "dry run" was always supposed to mean.

### The sixteenth audit round, run on the manifest generator

- **F-102** The generator normalised archive member paths with
  `lstrip("./")`, which strips a set of characters rather than a prefix.
  Measured: `.hidden/file` came out as `hidden/file`, so a release containing a
  dotfile would publish it under a name no file on disk has and every verifier
  would report it missing on a sound release; and `../escape` came out as
  `escape`, quietly rewriting a path that leaves the release into one that
  looks ordinary. The reader refuses such a path rather than sanitising it, and
  says why; the writer was doing the opposite. Both ends state the same rule
  now, and the writer fails the release job rather than publishing something
  every verifier would reject.

### The releases page, and what it was not telling you

**Every version is on it now.** The backlog began at v0.1.6, because
`CHANGELOG.md` keeps one combined section for v0.1.5 and earlier: six published
releases with no entry, no summary and no download links. They are listed, each
pointing at its own release page for its notes.

**The files are at the end of each release rather than the top**, so opening one
shows what changed in it. A short summary sits beside every version while it is
closed, so the list stays readable at a glance; the detail is inside.

### The fifteenth audit round, run on that page

- **F-101** The page listed five archives per release and the release workflow
  builds eleven. Two of the five names had never existed: it said
  `macos-aarch64` and `linux-aarch64` where every release has published
  `macos-arm64` and `linux-arm64`. So every entry carried two links that answer
  with a not-found, and six published platforms -- the Raspberry Pi build, the
  two static builds, FreeBSD, OpenBSD and NetBSD -- had no link at all.
  Measured against the assets GitHub actually holds for v0.1.14 rather than
  argued. The list is derived from the release workflow now, so a platform
  added there appears here and a label renamed there cannot leave a dead link;
  CI checks that every archive the workflow builds is linked.

### The fourteenth audit round, run on the verifier written an hour earlier

New code in the one program whose entire job is not to be fooled is exactly the
code an audit exists for. Three defects, all found by reading, and all three
the same mistake: a check that could not see, answering as though it had.

- **F-98** A folder the sweep could not open was reported as holding nothing
  extra. Measured: a tree deep enough to pass `PATH_MAX` stops the walk at
  about 1988 levels and a file below that read as absent rather than
  unreachable; a permission bit does it in one line. What could not be read is
  now named and withholds the pass, because unknown is not empty.
- **F-99** A symbolic link standing where a program should be, pointing at a
  copy of the genuine bytes, was reported as matching the signed list. The
  release published a file, not a link, and a link is a name somebody else may
  repoint after this has looked. The sweep for extra files already refused to
  walk through links, so the two halves of one module disagreed about what a
  link is.
- **F-100** A release signed by the project key *and* by somebody else's, in
  that order, would have been refused: only the first of GnuPG's signature
  reports was read. Safe direction, still a defect. A verifier people learn to
  work around protects nobody.

### The thirteenth audit round, run on what CI refused

- **F-96** A program that has just been started is not yet wearing its own
  name. The failsafe checks that a process id still belongs to the program it
  means to close, and its own tests start a `sleep` and act on it microseconds
  later. Measured, once in four thousand spawns: the kernel hands the parent
  back before it sets the new name, so the check reads the name of the process
  that did the starting. The shipped code already refuses on that answer, which
  is the direction that closes nothing when unsure; the tests now wait for the
  child to appear under its own name first.
- The real-time headroom test asserted an absolute number and so measured the
  machine: under emulation on the 32-bit job it failed while every native
  target passed. It compares the same audio run with and without accent
  tracking now, on the same machine in the same test, which is the claim it was
  always making.
- `veilvoice-gui`'s help text is read only on the platforms that have a console
  to print it to, and was declared on all of them, so the Windows build failed
  on a constant nothing reads.
- **F-97** Six committed drawings depended on which Python was installed.
  CPython 3.12 gave `sum` compensated summation over floats, so the same box
  widths added up a fraction differently, one box centred a tenth of a pixel
  further along, and files generated on one machine stopped matching the
  generator run on another. Everything generated here is committed and compared
  byte for byte so that "generated from the source" can be checked rather than
  asserted, and a check whose answer depends on the interpreter is not one. The
  sum is exactly rounded now, verified identical across four Python versions,
  and the job that checks it runs under two of them instead of one.
- The committed search index was seven bytes behind `ROADMAP.md`, which was
  edited after the generators had run. The check that exists for this caught
  it.

### The twelfth audit round, run before this release

- **F-95** A VeraCrypt volume chosen and answered for, then locked, still
  received the file. F-93 earlier in this cycle fixed the *panel*, which asked
  whether the folder existed when the question is whether anything is mounted
  on it. The file is written somewhere else, and that path asked neither: it
  checked only whether the hidden-volume question had been answered. So a
  locked vault took the recording onto the ordinary disk while its owner
  believed it had gone inside, which is the failure the feature exists to
  prevent, surviving the fix aimed at it. The mount table is now read at the
  moment of writing, and a job whose destination is not open is refused rather
  than quietly redirected.

### Encrypted volumes, and one password if you want one

**Cryptomator and VeraCrypt.** VeilVoice can write every veiled recording
straight into an encrypted folder you already have. It finds mounted vaults and
volumes, offers them, and takes a folder you point at by hand when it finds
none. `veilvoice volumes` reports the same thing from the command line.

It never opens, closes or unlocks anything, and never asks for a volume
password. Mounting your encrypted storage is your act, taken in the tool you
chose; a test refuses `Command::new` in the shipped half of the detection
module.

**The hidden-volume question** is the part that needed care rather than code. A
VeraCrypt container can hold a second volume inside the free space of the first,
and writing into the outer one can allocate over the hidden data. The two are
indistinguishable from outside by design, so nothing can detect it and only the
owner can answer. VeilVoice asks once, before the first write, and offers three
answers: no hidden volume, this *is* the hidden one, or this is the outer one.
The last is refused with a reason that says what would happen.

A destination nobody has answered for **blocks the job**. It does not quietly
fall back to writing beside the original, because a recording sitting outside a
vault while its owner believes it is inside one is the failure this exists to
prevent. A settings file edited into nonsense reads as unanswered rather than as
fine.

**What a vault is worth** is single-sourced and said in both front ends: it
protects the files inside it, not the temporary files, swap or hibernation
image, thumbnails or recently-opened lists the system writes about them.
Encrypt the disk as well, with BitLocker, FileVault, LUKS or LUKS2, `softraid`
or GELI. Defence in depth, not a second lock on the same door.

**The app lock can now seal every recording.** A third choice beside
*passphrase* and *public key*, offered where a lock exists, remembered between
launches. It reverses a decision this project has documented and defended, so
the cost is stated where it is chosen: one password then opens the application
*and* everything it has ever written, and forgetting it loses the recordings
rather than a session. Two separate secrets remain the default.

Recordings sealed this way do not depend on the lock file. Each carries its own
salt, so `veilvoice decrypt` opens it with the same password on any machine.
That is deliberate: a key derived from the lock's own salt would have meant
deleting the lock destroyed the archive, and deleting the lock is the documented
remedy for forgetting the password.

The passphrase is kept for the session only when that mode is already chosen. A
user who has not asked for it keeps the previous behaviour exactly, where it is
wiped the instant it has been checked.

### Getting the tree ready for a release audit

The work that a roadmap marker does not cover, and that a deploy needs.

**Manual pages.** `lintian` reported `no-manual-page` for all three binaries,
and it was right: `man veilvoice` produced nothing.
`tools/release/manpage.py` now derives each page from the binary's own
`--help` while a package is being built, so nothing is committed and no page
can drift from the command it describes. `help2man` does this job and was
tried first; it turns every em dash in VeilVoice's help into `???`, at every
locale, and a page that renders the program's own description as three question
marks looks finished and is not.

`veilvoice-gui` had no `--help` at all. It opened a window instead, and on a
machine with no display answered with a winit error naming `WAYLAND_DISPLAY`.
It now answers, on Unix, where a release build is guaranteed a console; on
Windows `windows_subsystem = "windows"` sends `println!` nowhere, so behaviour
there is deliberately unchanged rather than quietly made worse.

**The RPM builds.** A source RPM and two binary subpackages, and the thing a
build proves that a parse cannot is that `%files` and `%install` agree. They
do. The gaps are named rather than glossed: it ran on Ubuntu rather than any
RPM distribution, and it needed `--nodeps` and `--nocheck`. Four of the six
package definitions are still drafts and are still described as drafts.

**32-bit.** 716 tests pass on `i686` and `armv7` alike, up from 682, because
the new lock, vault and integrity code brought its own tests and had never run
anywhere but x86-64. Two of this project's shipped defects came from that gap.

**The parser campaign, over all six targets at ten minutes each.**
703,074,471 inputs, no crash, no hang, no out-of-memory. A **seed corpus** is
now committed for the two targets that start cold, and what it buys was
measured: on `lock_file`, a cold run starts at 25 code paths and reaches 460
after 64,309 inputs, while a seeded run *starts* at 625.

- **F-92** `Manifest::open_sealed` and `Policy::open_sealed` used the
  four-gigabyte Argon2 ceiling meant for a container somebody was sent and
  chose to open. Neither is that. The manifest sits at a fixed path, and this
  cycle's own marker 75 made the desktop application read it at every unlock,
  so anybody able to write that directory could make every unlock allocate four
  gigabytes, which on a modest machine is an abort. Both now use the unattended
  ceiling that F-91 gave the app lock. Found by decoding a slow unit the
  campaign reported rather than filing it as Argon2 being slow on purpose, and
  the lesson is one already recorded twice: F-91 was written up as being about
  the app-lock file when it was about any file the program opens without being
  asked.

### The app lock, hardened as far as it honestly goes

Markers 74 to 79, and one round of audit on the result.

**The locked window says it is locked and nothing else.** It used to name the
lock file, its directory, how many attempts had failed, and that deleting the
file starts over. All true, all addressed to the wrong person: the reader of a
locked window is either its owner, who does not need any of it right now, or
somebody who picked the machine up, who should not be handed the location of
the file and the news that removing it works. What the lock is worth is now in
`docs/USER_GUIDE.md` and on the security tab of the *unlocked* application.

**Every record now carries an authentication tag keyed by the passphrase.** One
Argon2id run is split by HKDF into the verifier that goes on disk and a tag key
that never does. Somebody who swaps the stored password for one of their own,
or drops the Argon2id cost so a guess becomes cheap, cannot make the edit look
authentic, and the next successful unlock says so. The report is stored, so it
survives a restart, and clearing it asks for the passphrase, so the person who
caused it cannot dismiss it.

The failed-attempt counter is deliberately **outside** the tag. It is written
at the one moment the tag key does not exist, so covering it would mean
reporting every honest typo as tampering. The rate limit is exactly as
editable as it always was and the documentation says so.

**The lock is kept twice.** Two copies, in two directories, under names derived
from a per-installation index, with contents masked so a search for the magic
bytes finds nothing. Deleting one does not remove the lock: the other puts it
back and the loss is reported. The names and the mask are **obscurity**, they
are labelled as obscurity everywhere they appear, and they are not counted as
security anywhere.

**The second copy goes somewhere only an administrator can write**, on Linux
and macOS, when VeilVoice is already running with the privilege to put it
there. It never asks for that privilege and never elevates itself. On Windows
the equivalent needs an access-control list this project does not link the API
to set, so there the second copy is a second copy and says so.

**`veilvoice-guard` is in the window.** The integrity record is taken at the
first launch that finds none and checked at every one after, on a worker
thread. With an app lock set it is sealed under that passphrase and the check
runs at the unlock, which is the one moment the passphrase exists. Without one
it is written in the clear and the tab says so in those words, because a record
sealed under a key kept beside it would look like the sealed case and be worth
nothing.

**The nine palettes are in the header**, where the website keeps its own. They
have been in the application since marker 26; nobody found them on a page
inside Settings.

**A live session repaints at 16 ms and everything else at 50.** Twenty frames a
second is fine for a progress line and is not fine for a meter following a
voice. The About tab now shows the measured frame time, because this was
written on a machine with no display and a number from the person with the
problem is worth more than a change made blind.

### The eleventh audit round: seven defects in the code above

New security code written after an audit is precisely the code an audit exists
for, so markers 74 to 79 got their own round. Six defects, every one in code
written this cycle, every one found by reading the diff.

Two of them were **worse than the thing they hardened**, which is the pattern
worth naming.

- **F-85** Any read of the vault index that was not a clean sixteen bytes drew
  a new index and wrote it, so one refused read, one sharing violation or one
  exhausted descriptor table would have orphaned the lock under a name nothing
  could ever compute again. The plain file this replaced could not be orphaned,
  because its name was a constant. Only a genuinely absent index is created now.
- **F-86** The administrator-owned spare silently kept the previous password
  after a change made from an unelevated run, so deleting the copy anybody can
  delete reverted the lock to a password somebody may still know. `store` now
  reports whether the spare caught up, a change that did not reach it is not
  called finished, and two copies holding different passwords are reported.
- **F-87** A spare that could never be written was reported as a deleted one,
  at every launch, which is how an alarm stops being read.
- **F-88** Dismissing an interference report ran Argon2id three times. A
  control nobody will wait for is a control nobody uses.
- **F-89** A power cut mid-write left a short file, which does not parse, which
  reads as tampering. Both copies now go through a write-and-rename.
- **F-90** Setting an app lock never upgraded an existing plain integrity
  record to a sealed one, so somebody who did the thing that earns the sealed
  record kept the readable one.
- **F-91** A lock file could declare four gigabytes of Argon2 memory, which on
  a modest machine is an allocation failure rather than a wait, and this build
  aborts on one: the window would fail to start with no way in. The generous
  ceiling is right for a container somebody chose to open and wrong for the one
  file this program parses before anybody has authenticated. Found by
  re-running the coverage-guided campaign, which the changed format made worth
  doing again.

### The tenth audit round: security, functionality, and what it costs to run

Marker 73, run last on purpose, because an audit of code that is still moving
is an audit of code that will not exist.

Every claim a machine can test, tested: no `unsafe` in any of the 26 crates, no
HTTP client anywhere in the dependency graph, `veilvoice-priv` starting only
its two read-only probes, 682 tests passing on both 32-bit targets, all
fourteen generator checks, and the coverage-guided campaign over six targets
for five minutes each. **293 million inputs, nothing found.**

One number in that changed for a reason worth recording. Last round `lock_file`
managed 3,274 inputs in its five minutes; this round it managed 445,714, which
is 136 times more. That is F-82's fix: with no ceiling on the number of Argon2
passes, most of that target's time went into a handful of absurd derivations.
Fixing a denial of service made the campaign that found it two orders of
magnitude more productive.

### F-84 - the preview said "nowhere else" before it knew where

`--preview` exists so somebody can hear their own veiled voice before an
interview rather than during one, and it printed *"the veiled voice goes to
this machine's output and nowhere else"* **before naming the device**.

That is not always true. `--preview --output <a cable>` keeps the cable,
because an explicit choice is honoured, and a machine whose default output is a
virtual cable does the same without being asked, which is not a strange setup
for somebody who routes their audio through one. Either way, whatever is
listening on that cable hears the preview.

A false reassurance in the one place somebody is checking their setup is worse
than none, because checking is what they came there to do. The claim now comes
after the device, names it rather than the machine, and says so outright when
the device is a cable. The desktop application made the same claim in a notice
and now makes the same check.

### The optimisation pass: 43.6 per cent of the search index was drawings

Measured. The index was 4,779,645 bytes, of which 3,903,419 was excerpt text,
of which **1,700,062 was generated SVG markup and copies of assets**. Every
byte of that is downloaded by every reader who uses the search, and it bought
them nothing: all 536 SVGs here are produced by a generator, the words in a
drawing are the words of the document it was drawn from, and a search result
pointing at an SVG file is one nobody can use.

The argument is not new. It is written at the top of the index generator about
the crate documentation, in those words, and it was applied to the banners and
not to the diagrams. That is how 43.6 per cent accumulated without anybody
deciding on it: an exclusion list naming the files somebody thought of.

**The index is 2,532,102 bytes now, 47 per cent smaller**, and 749 KB rather
than 918 KB over the wire. The rule is a property of the file rather than a
list of paths. Search still returns 63 results for "voiceprint" and the first
one is a document.

### The roadmap, as something you watch

Marker 71. Everything that is finished, scrolling past in a little under half a
minute, then four seconds with a ring filling in the corner before it starts
again. The countdown is there because a loop with no warning restarts under the
reader while they are still on the last line.

**An animation rather than an encoded file, and that is a decision rather than
a shortcut.** This project ships no codec and does not bundle `ffmpeg`, and the
rule it already settled for video output applies here: render here, and always
produce something that needs nothing else installed. An encoded file would also
be a committed binary whose bytes depend on which build of which encoder made
it, so it could not be regenerated and compared the way every other picture in
this repository is. What is there plays in any browser with no plugin and no
download, weighs a few kilobytes, takes the reader's colour scheme, and is
generated from `ROADMAP.md`, so it cannot show a marker as finished that is
not. The `ffmpeg` command to turn it into a file is printed under it.

Somebody who has asked their system for less movement gets the list at the top
and no countdown, rather than a picture that never settles.

The picture checker needed to learn about clipping for this: text inside a
`clip-path` is meant to be outside the frame, because that is what a scrolling
list is, and measuring it against the canvas would have reported sixty rows as
overflowing and been wrong about every one. Everything outside the clip is
still measured.

### Every banner was cutting its own sentence in half

Found by measuring rather than by looking, which is the point of it.

The colour key and explanation added to the diagrams needed a canvas tall
enough for them, and the height was worked out in one function and drawn in
another. They disagreed by one row, so the last line of every note was clipped
by the bottom edge of the picture it was explaining. That is the third time
this repository has cut its own text off, after F-37's banner and the terminal
drawings' ellipsis, and the two earlier ones were both found by somebody
looking at a picture. Looking does not scale to three hundred of them.

So `tools/site-tests/images.test.js` measures every piece of text in every
generated drawing against the canvas it sits on. It found the clipped note it
was written for, and then it found something considerably worse.

**The banner subtitles ran off the right edge, on almost every banner in the
repository.** `veilvoice-failsafe` overran by 665 pixels: its subtitle stopped
after "while you are" and the rest was simply outside the picture. A banner is
the first thing on a crate's page, in its README and in the wiki, and it was
cutting its own sentence in half in all three. Nobody had noticed because
nobody had read one to the end.

That is F-37 again, in the same place, four rounds later. The lesson recorded
then was that a picture has to be looked at; the lesson now is that a
measurement is what scales.

Subtitles wrap to two lines and mark the cut if a third would be needed. The
narrow diagrams widen to fit their own key, because a key drawn off the edge of
its picture is the same fault, and to fit their note, because a crate with one
file drew a picture 141 pixels across and wrapped a paragraph into it one word
at a time.

And one number where there were three. The two generators laid text out at 0.60
and 0.567 of the font size and the checker measured at 0.62, so drawings came
out a few pixels wider than the suite would accept. All three are 0.62 now,
which is also the safer figure: the font is whichever of the stack the reader
has, and a layout that assumes the narrowest overflows on the others.

**4,956 pieces of text in 490 drawings, all inside their canvas.**

### Every workflow chart now carries its own colour key and explanation

Marker 69. There are 303 of these drawings, one per crate and one per file, and
until now each of them needed the page around it to mean anything: the colour
key was a line of Markdown beside the picture and the explanation was a
paragraph under it. That is fine on the page and useless everywhere else the
drawing goes, which is a README, the wiki, and an `<img>` where nothing around
it travels with it.

Both are inside the picture now, wrapped to the canvas. So is a fourth entry
the key never had: what the dashed line means, which a reader previously had to
guess.

**The arrows are coloured by where they come from**, so a reader can follow one
call out of a box without tracing every line back to its start. That needs one
arrowhead per colour rather than one in total, because an SVG marker does not
inherit the stroke of the path it is on and `context-stroke` is not in every
engine this site supports. A back edge stays one colour for all of them: what
matters there is telling a cycle from a step, and colouring those by origin
would bury it under five hues.

**And a long name wraps instead of being cut.** It used to become an ellipsis
past thirty characters. No name in the tree was long enough to trigger it,
which is exactly why it was worth fixing rather than leaving: a truncation that
has never fired is one waiting for the first long name, in a picture whose only
job is saying what the box is.

Getting that right took two goes, and the second failure is the more
instructive. The first replacement wrapped to two lines and then cut the second
one, so `DeidConfig::reseed_range_is_finer_than_a_frame` came out as
`reseed_range_is_finer_than_a_f` with no ellipsis to admit it: the same defect
with a longer fuse and less honesty. Names now break after their `::` and after
an underscore, which is where a reader breaks them anyway, and only a run with
no boundary in it at all is split by width.

The note being clipped by the bottom edge of its own picture was the same
shape of mistake in the third place: the height was worked out in one function
and drawn in another, and the two disagreed by one row of the key. They are one
function now.

### Twenty questions, answered, including the ones where the answer is no

Marker 68. `docs/FAQ.md` and the page it renders to. Answers to what actually
gets asked, and roughly half of them are limits rather than features: it does
not hide what you said, it cannot tell who is speaking, it cannot detect a
keylogger and nothing can, the app lock does not protect your recordings, the
decoy passphrase is not deniability, and it has been audited only by its
author.

The answers live in a Markdown file because that file is readable on GitHub, in
a checkout, and by somebody who cloned this and never opened the website, which
is the audience this project keeps writing for. The page is generated from it
and checked in CI, and the contents list at the top is derived from the
headings rather than kept beside them.

Building it found one defect in the tool the roadmap page also uses.
`split.page` rewrites every `#anchor` in a body to `index.html#anchor`, which
is right for a section lifted off the front page and wrong for a page written
for itself: all twenty entries in the new contents list pointed at the front
page, where none of them exists. Caught by `source.test.js`, the suite added
with the source pages, doing exactly what it was written for.

### A working model of the application, and of the command line, in the page

Marker 67. Everything else on the site describes the program. The screenshots
are photographs of it, which are honest and are still pictures: a reader could
not find out what happens when they change a setting, or what the command line
answers, without downloading and running something. For a tool whose argument
is "check this yourself", asking somebody to install it before they can look at
it is the wrong way round.

There is now an overlay, opened from four buttons under the front page
animation, or from a link: `#try`, `#try-cli`, `#try-both`, `#try-verify`. It
holds a model of the desktop application with all nine of its tabs and a panel
for each, a terminal that replays what every subcommand printed, both side by
side, and the release verifier walking its three checks in order. Point at
anything and a line underneath says what it does, which is a helper rather than
a tooltip because a tooltip is for somebody who already suspects there is
something to find out.

**It says what it is, at the top, where a reader meets it.** This is a drawing
of VeilVoice and not VeilVoice: the panels are written by hand, the device
names and levels are illustrations, and nothing in it touches any audio. A
demonstration that lets somebody believe they have used the software has misled
them, and that sentence is the price of having one.

**What is not invented is generated and checked.** `tools/site/demo.py` reads
the tab list out of the application's own source and the terminal output out of
the committed captures of what the real program printed, and `--check` fails
the build when either has moved. A model that drifts from the program is a
claim rather than an omission, which is the distinction this repository has had
to make four times under other names.

The buttons are drawn only when scripts are running, through the class
`theme.js` sets from a blocking head script. A button that does nothing is
worse than no button, and the scripts-off edition has the photographs.

### The front page animation now says what the engine does, not one word of it

The picture of a voice going in and an unidentifiable one coming out had the
mark glowing in the middle and a caption that summarised the whole engine as
the word **discarded**. That word is true. It is also one word for six things,
and the one it names is the third of them.

The middle of the picture now lists the six, lighting one at a time: framed,
phase measured, phase discarded, pitch and formants moved, modulation seed
rolled, words left alone. Under it the caption explains each one rather than
gesturing at the set, including the two separate reasons there is nothing to
invert, which the single word ran together: the phase is thrown away and never
written anywhere, **and** every speaker is mapped onto one register and one
vocal tract, so several different people arrive at the same place.

Same style throughout: no script, no image, one animation, and it follows the
reader's theme. Colour is the only thing that changes, because a step that
moved or grew would push the labels around and six labels jostling for six
seconds is a picture nobody reads. With motion reduced every step is drawn lit
rather than dark, since the list is the content and the sequence is the
decoration. On a narrow screen the mark turns on its side and the labels do
not, because sideways text is not a label.

### The roadmap, published as a page, with a picture of where it has got to

`ROADMAP.md` is the file that decides what is done. It is also two thousand
lines in a repository, which is the wrong shape for the question people
actually ask: is this finished, and if not, what is left.

`website/roadmap.html` is that question answered. One square per marker,
grouped by the sections the roadmap already has, coloured by state, with the
unfinished ones listed under it with their estimates and the blocked ones
listed separately with what each is waiting for.

**It is generated from `ROADMAP.md` and checked against it**, the same
arrangement the documentation, the artwork and the search index have. A
published roadmap that quietly disagrees with the roadmap would be worse than
not publishing one, and this repository has recorded that exact failure four
times under other names.

Two things the picture deliberately does not say. **Area is not progress**:
every square is the same size and a marker is not a day, so reading the
coloured fraction as "how far along this is" would be wrong, and the page says
so directly under the picture. And **blocked is its own colour rather than a
shade of unfinished**, because five markers are waiting on a decision or on
somebody else's rules, and drawing them as "not done yet" would promise work
that no amount of effort delivers.

Eight new markers, 66 to 73, are the work asked for after v0.1.14, in the order
it is expected to be done, ending with a full security and functionality audit
and an optimisation pass before the next deploy. That one is last deliberately:
an audit run before the code stops moving is an audit of code that no longer
exists, and its estimate is the widest on the page because the audit's estimate
is the number that has historically been wrong.

### The pictures: no black margin, and no sentence cut off mid-word

Two things, both visible in the committed images and neither visible in any
test until one was written for them.

**Every window capture had a black border.** Eleven columns down each side, one
row along the top and two along the bottom, measured. `gui.ps1` asks Windows
for the DWM frame rather than `GetWindowRect`, which is the right rectangle and
is still not exact. Eleven pixels of nothing is the difference between a
picture that sits in a page and one with a ragged margin somebody has to look
past, and on a rounded container it is what stops the rounding meeting the
content.

`tools/shots/crop.py` takes it off, and `tools/verify.py` checks it stays off.
Nothing is redrawn or resampled: rows and columns are removed, and only ones
that are entirely a single opaque near-black colour matching the corner. The
near-black part is load-bearing and was found by running the tool without it,
whereupon it cheerfully ate nine rows of the **title bar**, which is also
uniform and is not desktop.

**And the terminal drawings cut their lines off with an ellipsis.** Three of
them ended a sentence mid-word:

```
-o, --output <OUTPUT>   Output device name. Defaults to a virtual cable if one is fo…
```

A picture whose entire job is explaining a flag, showing the reader that there
is more and they cannot have it. Long lines now **wrap** rather than truncate,
so the picture gets taller instead of wider, which is the axis a page can
afford. The wrap keeps the help screen's second column a column: a continuation
indents to where its description started rather than to zero.

That last part took two attempts and both failures are worth recording. The
column pattern first matched a single token, so `-o, --output <OUTPUT>` was read
as the flag `-o,` followed by no column gap, and the line fell through to the
prose branch which collapsed the alignment entirely. And a word longer than the
line, a path or a URL, could still be broken past the width. Both are fixed and
the longest line in any drawing now measures exactly the cap.

### The coverage-guided campaign has been run, and it found two things

`fuzz/` has held six libFuzzer targets since the fifth round, and
`docs/AUDIT.md` has recorded, every round since, that they had never been run:
libFuzzer needs a clang toolchain and the machine this is developed on is
Windows. "We have a fuzzing setup" and "we have fuzzed this" are different
claims and only the first was true.

All six have now been run, five minutes each, on x86-64 Linux. Between them
they got through 250 million inputs and found **two defects, both shipped, both
in code three audit rounds had read.** The run counts and the limits of what
five minutes proves are in `fuzz/README.md`.

### F-82 - a header could ask for four billion Argon2 passes, and get them

The memory cost has had a ceiling since F-2, with a long note explaining that
it arrives from the file and that a header claiming `u32::MAX` asks for four
terabytes. The **time** cost had a test for zero and nothing else.

Nothing overflows and nothing allocates, so every check passed. The derivation
simply did not finish. The campaign produced a header declaring 4,521,984
passes: **measured at about 74 hours** in a release build, and that is not the
worst case, only the one it happened to reach. `u32::MAX` passes is roughly
eight years.

It matters in two places and the second is worse. A `.veil` file is something
somebody sent you, so opening it hangs the program. The app-lock file carries
the same numbers and is **read before anyone has authenticated**, so anything
able to write it could stop VeilVoice from starting at all, with no error and
nothing to see. The campaign found it through both doors, independently.

The ceiling is 16 passes, chosen by measurement: RFC 9106's two profiles use
one and three, libsodium's most expensive preset uses four, this crate's
default is three. At the memory ceiling that is 75 seconds. The most expensive
header this build will accept is now a wait somebody can sit through.

The exact bytes are a regression test in the deterministic campaign, where they
run on every commit on every platform with no nightly toolchain.

### F-83 - the tamper record refused to write what it was happy to read

`Manifest::of` refused to record a path containing a line break.
`Manifest::parse` accepted one. So VeilVoice would not write a record it was
perfectly willing to read from somebody else, and `veilvoice guard check` reads
whichever file is at the path it is given.

The campaign produced a manifest whose recorded path contained a **carriage
return**. That is not a parsing problem. The product of this whole feature is a
report somebody reads to decide whether their files have been altered, and that
report is printed to a terminal, where a carriage return returns the cursor to
the start of the line and everything already printed is overwritten by what
follows. A crafted path makes the report say something other than what is
recorded. An escape character can colour, move the cursor or clear the screen.

Both ends refuse the same thing now, and they refuse the whole control range
rather than the two characters that were found, because listing the ones
somebody thought of is how the next one gets in.

### The live monitor: what is going in, and what is coming out

Live scramble has drawn input and output meters for some time, and they were
inside one panel. Switch to Group to set up an interview, or to Settings, and
the only picture of what the microphone was doing went off screen while the
audio carried on.

There is now a monitor that rides the window: **on by default, on every tab**,
showing the level going in and the level coming out, plus a sticky `CLIPPED`
warning because clipping is destructive and is over in a millisecond. Settings
moves it to a floating card in the corner or switches it off, and the live tab
keeps its full meters either way.

**And a way to hear yourself before anybody else does.** `preview to my
headphones` in the application, `--preview` on the command line: the same
engine, with the veiled voice going to this machine's own output instead of to
a virtual cable. It is the check the meters cannot make. A level says sound
arrived and sound left; a working meter and a bypassed engine draw the same
bar. Listening and hearing a voice that is not yours is what tells you the
engine is running, and that sentence is printed beside the meters rather than
left to be worked out.

While a preview runs, everything that said `live` in green says `preview` in
yellow, because somebody who has those two the wrong way round is either
speaking to a call in their own voice or speaking to nobody.

`--no-monitor` turns the terminal meters off for a log, and says once that it
is running rather than drawing a bar into a file nobody will read.

### The Debian package has been built, installed and run

`docs/AUDIT.md` has listed "none of the package definitions has been built" as
open since the fourth round. One of the six is now closed.

`dpkg-buildpackage -us -uc -b` produced `veilvoice_0.1.14-1_amd64.deb` and
`veilvoice-gui_0.1.14-1_amd64.deb`. Both installed with `dpkg -i`, the
installed `veilvoice --version` reported 0.1.14, `veilvoice info` and
`veilvoice-verify --help` ran, and both removed cleanly. The release build and
`cargo test --release --workspace` ran as part of it, because that is what
`debian/rules` does.

One machine, x86-64, Ubuntu 24.04, with a rustup toolchain rather than Debian's
own `cargo` and `rustc` packages, which is why the build needed `-d` to get
past `dpkg-checkbuilddeps`. `lintian` has not been run and nothing has been
uploaded anywhere. All of that is written beside the yes in
`docs/PACKAGING.md`, because "we built a .deb once" and "this is a Debian
package" are different claims.

Doing it found two defects.

### F-80 - the documented way to build the Debian package could not run

Two things, either of which stops it before any compilation begins.

**There was no `debian/changelog`.** `dpkg-buildpackage` takes the package's
version from that file and refuses to start without it. The recipe in
`docs/PACKAGING.md` copies `packaging/debian` into place and runs the build,
and nothing in either creates one.

**And `packaging/debian/rules` was tracked as mode 100644.**
`dpkg-buildpackage` runs it directly, so it has to be executable, and the mode
git records is the mode everybody who clones gets.

So the printed recipe failed on its first command for anybody who tried it. The
documentation said "not built", which is honest about the outcome and is not
the same as knowing the route was broken. Both are fixed and both are checked,
the mode through `git ls-files -s` rather than through the filesystem.

### F-81 - every package definition was five releases behind

Six files in `packaging/` name a version. All six said 0.1.9 while the
workspace was at 0.1.14.

What that meant, file by file: `brew install --build-from-source` would have
fetched and compiled the **v0.1.9** tarball; `flatpak-builder` would have
checked out the **v0.1.9** tag; the AppStream metadata told a software centre
that 0.1.9 is the newest release there is; and `rpmbuild` with no `--define`
would have stamped a package 0.1.9. Two of the commands printed in
`docs/PACKAGING.md` for a reader to copy carried the same number.

Nobody noticed because nothing was looking. It is the shape this repository
keeps finding: F-41 was generated output drifting from its generator, F-61 and
F-63 were comments that had stopped being true, F-71 was two hand-typed numbers
agreeing with each other. This is six files agreeing with a number that had
moved on without them.

All six are at the workspace version now, and
`tools/site-tests/packaging.test.js` compares nine version claims against
`[workspace.package]` in `Cargo.toml`. Verified by putting the old version back
in one file and watching the suite fail, rather than by assuming a new test
tests anything.

### CI now runs the tests where a pointer is 32 bits wide

`docs/AUDIT.md` has named this as the single highest-value change available to
CI since the fifth round, and for a specific reason: **two shipped defects came
out of its absence.** F-4 was an arithmetic overflow and F-11 was an erase loop
that never terminated and left the file it was destroying intact. Both were
reachable only on a 32-bit target, both were found by reading, and neither was
reachable by any campaign in a matrix where every entry is 64-bit.

The new job runs two targets. `i686-unknown-linux-gnu` runs on the runner's own
kernel with nothing emulated. `armv7-unknown-linux-gnueabihf` is compiled with
the cross linker and only its test binaries go through `qemu-arm-static`.
Measured before the job was written: **the same 682 tests across 47 suites pass
on both**, with 18 seconds of execution on i686 and 88 on armv7, so emulation
is not what this costs. Compilation is.

**It is not the whole workspace**, and the reason is packaging rather than
correctness: four crates link ALSA, GTK and X11, and building those for a
second architecture is a multiarch sysroot exercise. The arithmetic, the
parsers and the erase loop are in the crates the job does run. The crate list
is spelled out rather than written as an exclusion, so a crate added to the
workspace is absent until somebody decides it belongs, rather than joining
silently and failing for a linker reason that has nothing to do with 32-bit
arithmetic.

A passing run is not a campaign, and the audit entry says so: this shows the
existing tests hold where a pointer is narrow, not that anybody has gone
hunting there.

### F-79 - a security step that printed somebody else's error above its own "ok"

The installer fetches the signing key from the website, and from the repository
if the website does not answer. A failure of the first is not a failure at all,
which is why there are two addresses. But `curl -fsS` prints its own message
even in silent mode, so on a machine where the first address does not answer,
this is what a reader saw:

```
==> Checking the signing key's fingerprint
curl: (22) The requested URL returned error: 403
  ok   fingerprint matches 8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A
```

An error from another program, in the step that anchors every other check in
the script, immediately above the word `ok`. The check had passed. Knowing that
from those three lines requires knowing how the script is written.

The first attempt is now quiet and a line in the script's own voice says which
copy of the key it fell back to. The final failure stays loud and still names
both addresses. `install.ps1` never printed the raw error, because its fetch
swallows the exception, but it never said it had fallen back either, and now it
does.

**This one shipped**, in v0.1.14 and in every release with an installer before
it. No check was skipped and nothing unverified was installed: what failed was
the script's account of itself, in the one place a reader is being asked to
trust a chain of checks they cannot see.

### The installer has now been run on Linux

`docs/AUDIT.md` has listed "nobody has run `install.sh` on a real Linux or
macOS machine" as open since the fourth round. Half of that is now closed. On
x86-64 Linux, against the published v0.1.14 release: the latest tag was found,
the archive downloaded, the key's fingerprint compared against the constant in
the script, the signature over `SHA256SUMS` verified, the archive's hash
matched against the signed list, both binaries installed, and the installed
`veilvoice info` ran and reported 0.1.14. Its refusals were exercised on the
same machine, an unknown option and a version that is not published, and both
exited 1.

That run is what found F-79. macOS is still unrun, and its `sh` is not Linux's.

### Every box in a flowchart now opens the source, here, in your colours

Marker 27. The reference pages have drawn a call graph per crate and per file
since marker 18, and every box in every one of them was a link that left the
site: a blob on GitHub, in a new tab, in somebody else's colours.

There is now a page per file on this site carrying the file itself, coloured
with the same six classes the site already uses for code, so it follows
whichever of the nine themes you picked. Clicking a box opens that file at that
function, **with the whole function marked** rather than the one line its name
is on, and the mark reaches up over the function's documentation and its
attributes, because that is where this project puts the reason for anything.

The mark is `:target` in the stylesheet and nothing else. No script runs, so it
works with JavaScript off, it works from a bookmark, and it works on an engine
that has never heard of any of this. Line numbers are a pseudo-element, so
selecting the code and copying it does not take the numbers with it, and they
stay put while a long line scrolls under them.

Two things paid for by measurement rather than reasoning. A line is
`width: max-content` with a floor of the full column, because a block inside a
sideways scroller is only as wide as the *visible* box: the mark stopped where
the column did, in the middle of the code it was pointing at. And the block is
`white-space: normal` while each line inside it is `white-space: pre`, which
looks backwards and is not: the newlines between one line element and the next
are markup, and left as significant they drew the whole file double spaced.

Costs, stated rather than buried: 128 new pages and about 6.5 MB, all of it
under `website/reference/`, which the search index already excludes. A source
edit now writes a proportional diff into one more generated file.

The item table's line numbers land on the same pages. The GitHub link is still
there, on each file's page, named as what it is.

`tools/site-tests/source.test.js` is a new suite for this: every fragment that
names another page has to resolve, every source page has to draw exactly as
many lines as the file it claims to show, every box has to stay on this site,
and the mark has to remain a CSS rule.

### F-78 - four tests shared one global and could undo each other

The active theme in the desktop application is a process-global atomic, which
is right: it is read on every repaint. Four tests read and write it, and cargo
runs the tests of one crate on parallel threads in one process.

Nothing kept them apart, so one test could switch the theme and ask whether the
palette had changed while another put it back in between and made the answer
no. **Measured: one failure in forty runs of that module**, and it fired for
real in a full run of the workspace. The four now take a mutex in turn, and
sixty runs after that produced none.

Written up rather than quietly fixed because of what a flaky test costs: a
suite that fails one run in forty teaches whoever runs it that red means run it
again, which is the habit that lets a real failure through.

### F-77 - a count on the front page was measured on one machine only

The front page states a number of tests, and that number is generated by
running the suite rather than typed, which is what F-71 was about. It is still
a number about one computer: **the same commit measures 996 tests on Windows
and 988 on Linux**, because nine tests are compiled only on Windows.

So a reader on Linux who does the thing this project keeps inviting, which is
to check the claim instead of believing it, counts 988 and finds the page
saying 996. `docs/MEASURED.md` now records the host it was measured on and says
in its own header that the total differs by platform and by how much. Wording
the front page so it is true wherever it is read is a change to that page's
own voice, and it is listed as open rather than made here.

### F-76 - a program that had died was reported as still running

F-74 made Failsafe ask, before and after it closes something, whether a process
id still belongs to the program it means. On Unix it asked by name, and the
name is the wrong question.

**A process that has exited keeps its id and its name until its parent collects
its exit status.** Measured on Linux: after `kill -TERM`, `/proc/<pid>/comm`
still reads `sleep`, `ps -p <pid> -o comm=` still prints `sleep` and still
exits 0, and the only field that has changed is the run state, now `Z`.

So the check said "still there" about a program that was already dead, and
Failsafe waited out its whole retry loop, about two and three quarter seconds,
before telling somebody to go and close by hand a program that had closed
itself at the start of it. That is F-74's false report in the other direction:
one said it had closed something it had not, this said it had failed to close
something it had.

The run state is now read first, and a process that has ended counts as gone,
because it has. A `ps` with no state column falls back to the name check rather
than to a refusal, so a platform without that column loses the extra check and
not the feature.

**Found by running the suite on Linux**, where it failed two tests that pass on
Windows, whose process table drops a terminated process at once.

### F-74: Failsafe could close the wrong program, and said so either way

Two defects in one path, and the second is the worse of them.

**A process id is not a durable handle to a program.** Between the scan that
finds something holding a microphone and the line that closes it, that program
can exit and the operating system can hand its id to another. Closing by number
alone would terminate whatever inherited it. The window is small and it is not
theoretical: Failsafe exists to fire while somebody is plugging things in and
programs are starting and stopping, which is exactly when ids get recycled.

**And `taskkill` exits 0 whether or not it killed anything.** Measured: with a
filter that matches nothing it prints `INFO: No tasks running with the
specified criteria` and returns success, indistinguishable from a real
termination. The code checked the exit status, so Failsafe would have written
*"closed Discord (process 4812)"* into its log while Discord carried on sending
audio.

That is the worst sentence a safety catch can produce. It is not a failure to
act; it is a false report of having acted, and the whole feature exists for the
case where nobody is watching the window.

The name now travels with the kill: on Windows as a `taskkill` filter, so the
check and the act are one operation, and elsewhere as a check immediately
before, which narrows the window rather than closing it and says so. Where the
question cannot be answered, the answer is **no**: not closing something is
recoverable and closing the wrong thing is not. Afterwards the process is
looked for again, and a kill that did not kill now reports so.

### F-75: the application baseline was written world-readable

`veilvoice appctl` records what normally runs on this machine, with
`std::fs::write` and its default permissions.

That file decides what counts as ordinary, which makes it a security setting.
Another local account could add a line and have a program of their choosing
treated as unremarkable for ever, or read it to learn exactly what runs here
and when. It now goes through the same helper the key material uses, which sets
the permissions **as the file is created** rather than afterwards.

### Every file in the tree now explains itself twice

Marker 52. Every crate already carried a technical note and a plain-words one.
Now every **file** does: all 125 of them, including the build scripts, the
examples and the test files.

Eighty-five were written for this. None of them is a template. A formulaic
sentence about each file would have satisfied the check and taught nobody
anything, which is worse than the check failing, so each was written from what
that file actually does and says the thing worth knowing about it:

> **`spectral.rs`.** Sound carries two things: which frequencies are present,
> and how they line up in time. The second one, the timing, is a great deal of
> what makes a voice recognisably yours, and it is thrown away here and
> replaced. It is not scrambled or hidden; it is discarded, and there is
> nothing left to recover it from.

> **`shred.rs`.** This is meant to destroy a file, and the first thing it does
> is tell you how much that is worth. On the drives most computers now have,
> overwriting a file does not reliably remove it.

> **`plan.rs`.** VeilVoice does not work this out for itself. A program that
> guessed would sometimes put one person's words in another person's voice, and
> you would not find out by listening, because the result would sound perfectly
> fine.

The website, the wiki and the reference pages are regenerated from these, so
every file's page now carries both halves.

### A second passphrase, and the one that does not exist

```
veilvoice decoy
```

Marker 42, which this roadmap called the most dangerous thing on its list. It
set three conditions before anything shipped: what happens when it is typed by
mistake, what "securely erased" is really worth on flash storage, and what an
attacker who learns the trigger can do. Working through those decided the shape.

**The decoy is shipped.** A second passphrase opens VeilVoice with nothing in
it: a way to comply with somebody standing over you without handing over your
recordings.

A decoy too close to the real passphrase is **refused**, because somebody
watching a keyboard would learn both at once and somebody typing under pressure
would give away the wrong one. Length counts as difference, so `hunter2` and
`hunter2222` is not two passphrases.

Both are derived with the same Argon2id cost and compared in constant time, and
**both are always derived** even when the first matches. An early return would
make the real passphrase measurably faster and tell an observer with a stopwatch
which one had just been typed. A copy with no decoy set does the second
derivation anyway, so having one is not itself detectable.

**The destructive duress passphrase is not shipped, and will not be.** On flash
storage a write does not overwrite: the controller puts the new data in a fresh
physical page and leaves the old one holding the original until it is collected,
which may be never, and no program running as a user can reach it. VeilVoice
already refuses to overstate that about its own secure-erase feature.

So a destructive passphrase would be believed at exactly the moment being wrong
costs the most: somebody types it, assumes the recordings are gone, and behaves
accordingly while the ciphertext is still on the disk. **A control people rely
on and that does not work is worse than no control at all.** It also answers the
first condition: because nothing is destroyed, typing the decoy by mistake costs
a relaunch and nothing else.

**And it does not give you deniability.** VeilVoice is open source and this
feature is documented, so anybody who recognises the program knows it exists and
can ask for the other passphrase. A decoy buys you something to hand over; it
does not buy you an argument that there is nothing more. That sentence is the
first thing the feature prints, because somebody who mistakes it for deniability
is worse off than somebody who never had it.

A test reads the crate's own source and fails the build if anything in it grows
the ability to delete a file.

### `veilvoice accel`: your graphics hardware, and the one place it helps

```
veilvoice accel
```

Finds the graphics devices on the machine, says which can encode video, and
suggests one. NVIDIA through **NVENC**, AMD through **AMF**, Intel integrated
graphics through **Quick Sync**, Apple silicon through **VideoToolbox**, with
the driver version each system reports. A separate card is suggested over
integrated graphics, and the reason says *usually* rather than pretending it
was measured.

**The audio engine is not offered a graphics card, and the reason is a
measurement.** Veiling sixty seconds of audio takes about 0.58 seconds on one
core, roughly a hundred times faster than real time, and live mode finishes
each 1024-sample frame in about 0.05 ms out of the 21 ms it has. A graphics
card is fast at doing one thing to a very large batch; moving a frame that
small onto it and back costs more than the work. A "use the GPU" switch there
would make VeilVoice slower, so there is not one.

**Video encoding is the opposite shape of problem**, and that is what the
hardware is offered for. `Encoding::encoder` carries the choice through to
`ffmpeg`, and the software encoder stays the default: two people rendering the
same recording should get the same file unless one of them asked not to.
Asking for hardware also switches `-crf` to `-cq`, because the hardware
encoders do not have the first and ffmpeg refuses to start rather than
rendering more slowly.

**Finding a device is not proof it can be used**, and that is printed beside
every result: hardware encoding also needs a working driver and a copy of
ffmpeg built with that encoder, neither of which can be determined by reading
a device name.

Threads are reported too, and documented as being for **batches**. One
recording cannot be split across cores: the ratchet and the phase state run
forward in time, so two halves veiled in parallel would not produce the file
the whole of it would.

Measured on the machine this was written on: one NVIDIA GeForce RTX 4060,
driver 32.0.16.1062, NVENC, 28 threads.

## v0.1.14

The safety catch, an interface that reads like English, and a window that does
not stop when you open a file.

**Failsafe is the headline, and it is on by default.** It watches for the one
accident this whole project is powerless against once it has happened: you are
talking through VeilVoice with your voice veiled, you plug in a headset, and
your computer quietly switches the call to the real microphone. Your own voice
goes out and nothing on screen looks any different. Failsafe notices within
about a second and closes the program that took it. It says plainly that it
notices rather than prevents, because the gap is short and it is not zero.

**Seven file pickers used to freeze the entire window** until you chose a file.
That is the answer to "it lags when I select things", and it was an accurate
report.

**Nothing the application says has a dash in it any more**, headings are
sentences rather than shouting, and every tab scrolls so nothing can be shrunk
out of reach.


### The interface reads like English, and the screenshots are real

Headings were shouted in capitals: `SETTINGS`, `WHAT A PASS PROVES`, `APP
LOCK`. They are sentences now, capitalised the way a sentence is, and the tab
labels with them. **No dashes anywhere the application speaks**, either: every
one has been rewritten into the punctuation that carries the same pause, and a
test reads the whole crate and fails the build if one comes back.

**Every tab is inside one scroll area.** The lock tab had none at all, so on a
short window its controls could not be reached by any means: not scrolled to,
not tabbed to, not resized into view without making the window taller than the
screen. One scroller rather than one per tab, so a tab added later gets it
without anybody remembering, and so the wheel is never trapped in whichever of
two nested scrollers the pointer happens to be over.

The window opens at 1100 by 720 instead of 720 by 620, which is large enough to
read without resizing and still fits a 1366 by 768 laptop with its taskbar. The
floor is 720 by 520, and it is now about **width**: anything taller than the
window can be scrolled to, but the layout is monospace and column-based and
below roughly 720 across the columns overlap rather than reflow.

### `--tab`, and screenshots that cannot show the wrong thing

```
veilvoice-gui --tab verify
```

A deep link into a tab, and the thing that finally made `tools/shots/gui.ps1`
honest. Three earlier versions of that script drove the interface by clicking
and each failed differently: hard-coded coordinates that went stale when a tab
was inserted; a pixel scan that merged two labels once capitalising them closed
the gap; and, underneath both, the fact that synthetic mouse input needs the
window in the foreground and Windows refuses to give the foreground to a
background process. `SetForegroundWindow` reports that refusal by returning
false, which nothing was reading, so the click went nowhere and whichever tab
was already open got photographed under nine names.

It no longer clicks. The application is started once per tab, maximised, and
photographed with `PrintWindow`, which asks the window to draw itself and needs
neither focus nor visibility. A fingerprint of each capture is compared against
the others, so two tabs coming out identical is caught rather than published.

The README's pictures are retaken at the full resolution of the screen, and the
page now says plainly that **Windows 11 is confirmed and Windows 10 is
supported but not yet confirmed**, which are different sentences.

### Measured: the window does not flicker

Reported, investigated, and not reproduced. Twenty-four consecutive captures of
an idle window differ in **fourteen sample points**, all inside `x 36..88,
y 68..84`, which is exactly where the animated soundbar is drawn. Everything
else is pixel-identical. Twenty frames taken while the window was resized were
all drawn, none blank or stale.

An earlier attempt to blame the renderer was reverted: a `wgpu` swap was
written, and the instrument built to justify it turned out to measure nothing
three times over. What *was* found and fixed is in the entry below, and it is a
better match for the report: seven file dialogs that stopped the window dead.

### `veilvoice gui` finds the application in three places, not one

```
veilvoice gui      # or: veilvoice g
```

The command existed and looked in exactly one place: beside the binary. A
reader who had **installed** VeilVoice and typed `veilvoice gui` from
anywhere else was told the application was not there, which was untrue.

It now looks beside this program first (a portable folder holds all three
together, and somebody who unpacked a release means the one they unpacked),
then where an install puts it, then on `PATH`, and if it finds nothing it
lists every place it looked rather than saying "not found".

**It never starts anything by a bare name.** `PATH` on Windows searches the
current directory first, so `veilvoice gui` run inside a downloads folder
holding something called `veilvoice-gui.exe` would have started that. This is
the one command whose entire job is launching another program, which makes it
a poor place to be relaxed about which. A test reads the module's own code,
not its comments, and fails the build if a bare name appears.

`veilvoice g` is the same command, and `--quiet` opens the window without
printing anything.

`veilvoice install` already copied all three programs and added them to
`PATH`; that has not changed.

### Failsafe: on by default, because this accident is silent

```
veilvoice failsafe
```

The accident: you are talking through VeilVoice with your voice veiled, you
plug in a headset, and your computer quietly switches the call to the **real**
microphone. Your own voice goes out. The veiled window is still open in front
of you, meters still moving, looking exactly as it did a second earlier.

Nobody notices that, because there is nothing to notice. It is not
carelessness; it is a decision the operating system makes on your behalf. So
Failsafe is **on by default**, and by default it also **closes** the program
that took the microphone, a warning you have not read yet does not stop
your voice going out.

**It notices. It does not prevent, and that difference is printed every time:**

> Failsafe cannot stop your computer handing a microphone to another program.
> [...] What it does is notice, within about a second, and act, so there
> is a moment between another program taking a real microphone and Failsafe
> reacting to it. That moment is short and it is not zero, and anything that
> told you otherwise would be lying to you about how safe you are.

Closing a program is bounded rather than general. Never VeilVoice itself,
never a system process, and never by name, only the specific process the
watch feed named, with the protection checked **twice**: once when deciding
and again inside the only function that acts, because the cost of being wrong
is ending somebody's desktop session. Every close is written down.

Three distinctions the crate refuses to blur:

- **"Nothing is wrong" and "nothing is being protected" are different.** With
  live veiling stopped it says so, rather than showing an all-clear.
- **A platform that cannot see is never reported as clear.** An empty list from
  a system that cannot answer is not good news.
- **An unreadable setting reads back as ON.** A settings file this build cannot
  parse must never be the reason the safety catch is off.

A program using VeilVoice's own cable is the arrangement working, not the
accident, and is not reported, since otherwise the alarm fires constantly and
gets ignored.

### The window no longer freezes while you pick a file

Every file picker in the application was opened with the **blocking** API,
straight from the frame that handled the click:

```rust
if ui.button("choose file…").clicked() {
    if let Some(path) = rfd::FileDialog::new().pick_file() { … }
}
```

`pick_file` does not return until you have chosen or cancelled, and it was
being called from inside the render loop. So for as long as that dialog was
open **VeilVoice drew nothing at all**: no repaints, animations stopped, meters
frozen, and dragging the window left a trail of stale pixels. Somebody
browsing for a recording for thirty seconds had a frozen application for
thirty seconds.

There were seven of them, the input file, the recording and plan in
group mode, opening and saving a project, the public key, and all three slots
on the verify tab.

They now run on a thread of their own and the answer is collected without
waiting, so the window keeps painting the whole time. A test reads every source
file in the crate and fails the build if a blocking picker reappears anywhere
outside the one module that is allowed to know about it.

**macOS keeps the old behaviour, deliberately.** `NSOpenPanel` must be driven
from the main thread; opening one anywhere else does not work, and on some
versions it does not fail politely either. A frozen window is better than a
dialog that never appears.

Cancelling is reported as an answer rather than as silence, and a picker thread
that dies without answering is treated as a cancel, since otherwise the button
that opened it stays disabled with no way back.

### `veilvoice privilege`: what it is running with, and what that lets it see

Marker 39. Most of VeilVoice needs no special permissions, because changing a
voice is something any program can do with your own account. The parts that
*watch* see further as an administrator, and this says which of those you are
getting.

**It never raises its own privileges, installs a service, or asks for a
password.** It prints the command and you decide. A privacy tool that silently
acquires administrator rights is a privacy tool nobody can reason about. A test
names every subprocess the crate starts, so that stays true rather than staying
a comment.

**The opt-in service is deliberately not shipped.** A service outlives the
window it was started from, starts itself at boot, and runs whether or not
anybody is using the program, and somebody who tried VeilVoice once should
not find it still running next month. Leaving the window open is the honest
form of continuous monitoring, because then what it can see is exactly what it
says it can see.

**Kernel level is not reached and says so.** A driver on 64-bit Windows needs
an EV code-signing certificate issued to a verified legal entity and then
Microsoft's attestation signing; macOS needs an Apple Developer ID and an
entitlement granted case by case. Both are identity checks on a named legal
person, and this project is published under a pseudonym on purpose.

"I could not tell" is its own answer and never reported as "not elevated", because
understating what VeilVoice can see sounds like the cautious direction and is
not, because somebody would conclude a feature is unavailable and stop reading
its output.

Two details that came from measuring. The Windows probe keys on the well-known
SID `S-1-5-32-544` rather than the group's *name*, which is translated and
would report every non-English machine as unprivileged. And the "Group used for
deny only" attribute, which is what an administrator account looks like when
it is **not** elevated, sits on the same 236-character line as the SID; a
console wraps it so it looks like two rows, and reading it that way would call
every administrator account elevated. Verified on a machine in exactly that
state, which reports `your own account`.

### `veilvoice appctl`: learn what normally runs, then notice what does not

```
veilvoice appctl learn            # for a few days, while you work normally
veilvoice appctl learn --finish   # close the baseline
veilvoice appctl check            # what is running that it does not know
```

Marker 37. **It does not block anything and cannot.** It is a way of noticing,
not a lock on the door: a program it calls unknown is still running. Real
enforcement needs a kernel driver or a signed system policy and an application
identity to sign it with, and this project is published under a pseudonym on
purpose.

That note is printed by **every** subcommand, not once at setup, not
behind a flag, because a warning shown once is a warning forgotten by
the second week, and the one thing a reader must not come away believing is
that this stopped something.

**Learning has an end.** A baseline that is always learning has learned
nothing: whatever an attacker starts joins the picture the moment it starts.
Freezing an empty baseline is refused, because a baseline that learned nothing
calls everything unknown, which is the same as calling nothing unknown.

**Grants expire**, checked against the clock rather than against a sweep that
may not have run. An expired grant is left on record rather than removed,
"this was allowed until Tuesday" is worth more to a reader than a row that
quietly vanished. Permanent is spelled `--forever` rather than a distant date,
so choosing it is something somebody typed.

**Only the decisions worth reading are logged.** A line for every ordinary
program every time it is seen is a log nobody reads, and a log nobody reads is
not a control.

Measured on a real machine: 111 programs learned from 313 sightings, then a
`check` with a stray process running named `timeout.exe` and `smartscreen.exe`
and the second was started by Windows itself, which is exactly the case this
is for.

### `veilvoice-proc` gains a second caller

The process listing extracted for `veilvoice-input` is now shared by three
features rather than two. `veilvoice-appctl` has no dependencies at all: the
caller supplies the names, so the crate is arithmetic over a list and its tests
need no machine to run on.

### Notifications: a card, an alert, or nothing: with the contrast measured

Marker 41. Three ways for the application to tell you something, chosen in
Settings under *interface*:

- **a card in the corner**: rounded, translucent, fades on its own, will
  not take focus or interrupt what you are typing, which also means it can be
  missed;
- **a message that stops you**: cannot be missed, and cannot be missed
  quietly, which is what you want when the thing being reported is that
  something started recording;
- **nothing**: offered because a monitor that interrupts you every thirty
  seconds is one you switch off entirely, and then it is watching for nothing.

**The contrast is computed against the colour that is actually on screen.** A
translucent card is a colour laid *over* the panel behind it, so measuring the
card's own tint answers a question nobody asked. VeilVoice blends the two,
takes the WCAG ratio against the result, and picks the text colour by measuring
every candidate in the palette rather than assuming black or white, a
user palette can be anything, and an assumed extreme puts a colour on screen
that is in no theme.

If nothing reaches 4.5:1, the card is drawn **opaque** rather than shipped
illegible. Translucency is a nicety; reading a warning is not. The preferences
panel prints the measured ratio and says when translucency had to be given up,
because a quietly solid card otherwise looks like a design choice.

Alerts from the monitor now queue until they have been shown, so one that
arrives while you are on another tab is still waiting when you come back rather
than having scrolled past in a log you were not looking at. One at a time: a
stack of cards covering the window is how somebody dismisses six warnings
without reading any of them.

**One honest limit, printed beside the setting.** These appear inside
VeilVoice's own window and nowhere else. It does not put messages into your
desktop's notification area, because that needs a registered application
identity on two of the three platforms and this project is published under a
pseudonym on purpose.

### A ratchet interval that is not the same in every copy of the program

```
veilvoice anonymise recording.wav --reseed-range 250,1800
```

Markers 28 and 48. The modulation seed rolls forward on a ratchet; a fixed
interval is a fixed thing to observe. The interval is now drawn fresh before
every roll, from a range that is itself **drawn from the operating system's
random source at launch**, so it is a property of your run rather
than of the binary. `--reseed-range fixed`, or the checkbox in the application,
restores the old fixed interval by name.

**Anything that is not a usable range is refused with the reason, never
adjusted to fit.** Six distinct refusals, each naming which end was wrong and
what the bound is:

```
✗ --reseed-range: the range runs backwards: 1800 is not below 250, and the low end comes first
✗ --reseed-range: 0 is not a length of time; both ends must be above zero
✗ --reseed-range: 900000 ms is past the 600000 ms ceiling. A ratchet that slow is
  almost certainly a typo, and a long interval weakens forward secrecy without
  buying anything
```

Clamping would leave somebody running on a setting they did not choose and
cannot see. For a control whose whole purpose is that the interval should not
be predictable, that is the worst available failure.

What is displayed is the **effective** range, quantised to whole frames, not
what was asked for, the ratchet can only fire on a frame boundary, and
showing the request would describe a spread that does not exist. Asking for
`250,1800` reports `251-1803 ms`.

### F-73: the randomised ratchet was written, documented, and never called

**This one had shipped.** `reseed_range_ms` and `with_random_reseed_range` were
implemented and tested, and the field's documentation said "the front ends call
this at launch, which is what makes the shipped interval something other than a
number compiled in".

Nothing called it. The function appeared three times in the tree: its
definition, that sentence, and one test of itself. Every released copy of
VeilVoice rolled the modulation seed every two seconds exactly.

What it is worth is small and real. The ratchet is forward secrecy, not
irreversibility, the many-to-one mapping is what destroys the
voiceprint and does not depend on the ratchet at all, so a predictable period
never made a voice recoverable. What it gave an observer was a clean segment
boundary every two seconds in every recording VeilVoice has ever produced, in
every copy. Removing that is the entire reason the feature exists.

It is the fourth defect in two rounds where a sentence was true about the design
and false about the code, and the first where the sentence described work that
was *finished* and simply never wired up. A passing test covered the feature.
Reading the module would not have found it; looking for the function's callers
did.

A comment cannot be tested, so the fix tests the code the comment is about: a
test reads both front ends' source and fails the build if the call is missing.

Measured across three consecutive runs:

```
Seed rolls    16-69 ms, drawn fresh before every roll -- no period to observe
Seed rolls    773-1963 ms, drawn fresh before every roll -- no period to observe
Seed rolls    1088-1120 ms, drawn fresh before every roll -- no period to observe
```

### What can see your keyboard and mouse: and why a clean result proves nothing

```
veilvoice input
veilvoice input known
```

Marker 35. It names the programs running right now that are **able** to observe
keyboard and mouse, such as remote-support tools, macro recorders, password
managers, screen readers, and says what each one is and why it can
reach input at all. Nearly all of it is software somebody installed on purpose,
and the crate says that too.

**It does not claim to detect keyloggers, because nothing can.** The mechanisms
a logger uses are the mechanisms accessibility software uses, and software
written to hide is written to hide from a process list. So every result, found
or not, is printed with the sentence that matters:

> a result of nothing found does not mean nothing is watching. It means
> nothing this build recognises is running, which is a much smaller claim

Somebody who reads "nothing found" as "nothing there" has been made *less* safe
by running it. A test asserts that the empty-result summary says so, and
another reads every sentence the crate can print and fails the build if any of
them accuses a program of doing anything rather than being able to.

**It does not hook the keyboard to find out.** Detecting input monitoring by
monitoring input would make this the thing it warns about, and on Windows it
would need exactly the call `#![forbid(unsafe_code)]` rules out. A test reads
the crate's own source and fails if `SetWindowsHookEx`, `GetAsyncKeyState`,
`CGEventTap`, `/dev/input` or `evdev` appear in the code.

"I could not look" and "I looked and found nothing" are different answers and
carry different summaries, because reporting the first as the second is the one
mistake here that costs somebody something.

### `veilvoice-proc`: one process listing, not two

Screen-capture detection and input-monitor detection need the same answer:
which programs are running. It was private to `veilvoice-capture`. Depending on
that crate for it would have meant a keyboard feature pulling in a table of
screen recorders, which is exactly what `ROADMAP.md` says these crates must not
do; copying it would have left two parsers to drift apart, which is why
`veilvoice-check` was extracted out of the verifier in the first place.

So it is a crate of its own with no dependencies, and it carries the limits of
its own answer: it sees programs running as you, and it sees that they are
*open*, never what they are doing.

## v0.1.13

Build it yourself and check the release against what you built; group mode you
can see; projects, profiles and a voice limit that was measured rather than
chosen; and the eighth audit round.

**The headline is `veilvoice-verify reproduce`.** Until now the verifier
answered one question: *is this download the one that was published*.
It now answers the harder one: **is the published build the one this source
produces**. A signature says who made a file. Only a build says what it is made
of.

**The eighth audit round found seven defects (F-66 to F-72), none of them
shipped.** Two were found by continuous integration rather than by anybody's
judgement, and both had been watched to pass on the machine they failed on.
Three more were found by running a command and reading what it printed. Not one
would have been found by reading the code, which is the round's whole lesson
and the third time this project has had to learn it.


### Build it yourself, and check the release against what you built

```
veilvoice-verify deps
veilvoice-verify reproduce . --sums SHA256SUMS --sig SHA256SUMS.asc
```

Markers 55 to 59. `veilvoice-verify file` answers *is this download the one
that was published*. This answers the harder one: **is the published build the
one this source produces**. A signature says who made a file. Only a build says
what it is made of.

**The signature is verified before any hash from the list is read.** Not warned
about, and refused, with nothing built and nothing compared. The comparison
function takes the hash list as *text* rather than as a path, so there is
nowhere in it that could read an unverified file by accident; a test reads the
function's own body and fails the build if `std::fs` appears in it.

**A difference is a finding, not an accusation.** Both hashes and the differing
file names are printed rather than a verdict, and it exits 5, which is
deliberately not the status that means tampering. Most causes are dull: a
different compiler version, a path baked into a panic message, a timestamp.

**"Builds for every operating system" means "builds for the one it is on",** and
the help says so. `veilvoice-cli` cannot be compiled for Linux from Windows
because `alsa-sys` needs ALSA's headers, and a macOS build needs Apple's SDK,
which Apple's licence does not allow to be redistributed. Three machines give
you three platforms verified, which is how a reproducible-build claim is
normally checked.

**`deps` names what a build needs, who ships it, and why VeilVoice wants it**
namely the toolchain, a linker, `pkg-config`, and ALSA's headers, which only live
mode needs. Missing pieces are installed **only on an explicit yes**, with the
exact command line shown before the question, through the package manager the
system already has. It adds no network client: the claim that this project's
dependency graph contains no HTTP client is unchanged and still checkable with
`cargo tree`.

It will not run rustup for you. That installer downloads a compiler, writes to
your home directory and edits your shell profile, and all three are yours to
agree to.

**A build with nothing to compare against is not a pass.** It exits 3, and says
the hash list may be for another platform. A hash list naming nothing that was
built would otherwise report success by vacuum, which is the failure mode this
whole exercise is most exposed to.

### F-72: three tests passed here and failed on the same platform

Several tests read this project's own source with `include_str!` and find a
function's end by searching for `"\n}\n"`. They passed locally and failed on
GitHub's Windows runners, not on a different platform, on the *same*
one, minutes after being watched to pass. This machine has
`core.autocrlf=input`; GitHub's Windows runners default to `true`, so the file
arrives with CRLF and the pattern matches nothing.

There was no `.gitattributes`, so a checkout's line endings were whatever each
contributor's git happened to be set to.

**The tests are the small half.** Every artefact here is regenerated and
compared byte for byte by `tools/verify.py`, and every generator writes LF. A
contributor whose git converts text on checkout would find every `--check`
failing on files they had never touched, on their first run, with a diff that
shows nothing, and would reasonably conclude the repository was
broken.

`.gitattributes` now pins text to LF for everyone and names the binary formats
rather than trusting detection to guess right on a `.wav`. The source-reading
tests normalise as well: a test that depends on a git setting is one somebody
will trip over on a machine nobody here owns.

The failure mode is now a test of its own, a search for `"\n}\n"`
is asserted to succeed against LF and to fail against CRLF, so it is
on record as reachable rather than as a story about it.

### F-71: the guard against stale claims compared one copy to another

The front page said **354 tests** and "no unsafe code, in any of the **nine**
crates". The tree holds 890 tests across 19 crates, and the website runs 11
suites rather than ten.

A guard existed, and passed. It was written after an earlier round of this
exact drift, its comment says "this was the one place claims were
hand-typed with nothing watching them", and then it compared the
front page against `docs/AUDIT.md`. Both numbers were typed by the same hand at
the same time, so both drifted together and the check reported success.

The numbers now come from the tree. `tools/measured/generate.py` writes
`docs/MEASURED.md`, the test count **by running the tests**, the
crate count from `Cargo.toml`, the suite count from `run.js`'s own list,
and every claim on the page and in the audit is compared against it.

Three things that would have let it happen again:

- The test count is **measured, not counted**: a static count of `#[test]`
  gives 903 against a measured 890, because some tests sit behind features that
  are off by default.
- The suite count comes from the runner's list, not a directory listing. A
  suite that exists and is not in `SUITES` does not run.
- **Spelled-out numbers are refused.** "the nine crates" is how this drifted
  unnoticed; no check can compare a word.

The guard was checked by breaking it, then restoring it. A control nobody has
watched fail is a control nobody has tested.

### F-70: the reproducibility checker would have said no to everybody

The new `reproduce` command ran `cargo build --release` and nothing else: no
`--remap-path-prefix`, no `SOURCE_DATE_EPOCH`, no per-linker determinism flag,
no `--target`. The release sets all four, and `docs/REPRODUCIBLE_BUILDS.md` has
said since before this checker existed that reproducibility depends on the build
*environment* setting them.

So the answer was decided before the build started. Measured: two builds of this
tree in two directories produced three binaries with three different hashes.

The severity is in what it would have taught the one reader who took the trouble
to build from source, that the release does not match its source.
**A checker that always answers "not reproducible" is worse than no checker**,
because the next time it says so for a real reason, that reader has learned to
ignore it.

It now reproduces the release environment rather than approximating it, and
prints every setting before building:

```
  The settings a release is built with, reproduced here:
    target            x86_64-pc-windows-msvc
    RUSTFLAGS         --remap-path-prefix=<source>=/veilvoice --remap-path-prefix=<cargo home>=/cargo -C link-arg=/Brepro
    SOURCE_DATE_EPOCH 1787746339
```

The remapped path is the one the compiler is actually given, not
`canonicalize`, which on Windows returns a `\\?\` path that cargo never hands
to rustc, so a remap built from it matches nothing and does nothing silently.
`RUSTFLAGS` is set rather than appended to, because a value inherited from the
terminal is one the published build did not have. Outside a git checkout there
is no commit date and it says so rather than inventing one.

A third remap had to be added and the measurement is what found it: with the
source and `CARGO_HOME` remapped, two builds gave two identical binaries and
one that differed, `veilvoice-gui`, because `OUT_DIR` lives under the *target*
directory. The release never meets this, since it builds into `target/` inside
the tree it already remaps.

Measured on this machine, two builds in two separate target directories:

| | `veilvoice` | `veilvoice-gui` | `veilvoice-verify` |
|---|---|---|---|
| As first written | differs | differs | differs |
| Source and `CARGO_HOME` remapped | identical | **differs** | identical |
| Target directory remapped as well | identical | identical | identical |

A test compares the flags against `release.yml` itself, so changing one without
the other fails the build.

### F-69: the build succeeded, and then looked for it in the wrong place

Found by running `build` on this machine. After a release build that took
several minutes and worked, the tool hashed `root/target/release`, a path it
computed rather than asked for, and ended with:

```
  ok    the build finished

FAILED: the build left nothing to hash
  .\target\release is not there
```

`CARGO_TARGET_DIR`, `build.target-dir` in a `.cargo/config.toml`, and a target
directory shared between checkouts all move it. It now asks `cargo metadata`,
which is the only thing that knows, and the run ends with three hashes instead.

The JSON is read by hand rather than by taking a dependency for one field, and
the escapes are undone properly: the value is a Windows path, and text taken
between the first two quotes gives `C:\\Users\\...`, which looks almost right
and does not open.

**Three of this round's four defects are the same mistake.** F-67 answered from
a default configuration rather than the one in force, F-68 from a program that
shared a name with the right one, F-69 from a path that is usually correct.
None was a logic error, and none would have been found by reading the code.

### F-68: the linker check found Git's hardlink tool and called it a linker

Found by running `deps` on this machine. The Windows probe looked for `link` on
`PATH` and reported whatever came back, which was
`C:\Program Files\Git\usr\bin\link.exe`, GNU coreutils' hardlink utility.
It shares a name with Microsoft's linker and has nothing whatever to do with
building Rust.

So the dependency check said the linker was present, and a build on that machine
would have stopped with a linker error anyway. A probe that answers from the
wrong program is worse than no probe: it produces a confident wrong answer where
absence would have produced a useful one.

There is no honest probe for it. `link.exe` is only on `PATH` inside a Developer
Command Prompt, cargo finds MSVC through the registry instead, and any `link`
that *is* on `PATH` is more likely to be something else. It now says it cannot
tell, in those words, and lets the build be the judge.

### Four verbosity levels, and eight exit statuses that mean something

```
veilvoice-verify --quiet file veilvoice.tar.gz --sums SHA256SUMS --sig SHA256SUMS.asc
echo $?
```

`--quiet` says nothing at all. `--brief` says the answer and nothing else.
The default is unchanged, and `--verbose` adds every command, path and hash.

**The statuses came first, and that is the whole point.** A tool that prints
nothing and returns zero when a signature did not verify is worse than a noisy
one: it reports success by staying quiet. So there is no quiet mode until every
outcome has its own documented number, and `--help` prints the table:

```
EXIT STATUS
  0   everything asked for was done and every check passed
  1   the command line could not be understood; nothing was attempted
  2   a check ran and FAILED -- do not run what you downloaded
  3   a check could not be completed; nothing was proven either way
  4   the build was attempted and the compiler stopped
  5   the build here does not match the published build
  6   build dependencies are missing and were not installed
  7   the check passed; putting the files in place did not
```

**2 and 3 were the same number before, and they are different facts.** "I
checked and it was wrong" means somebody may have tampered with your download.
"I could not check" usually means a network hiccup. A script could not tell
them apart, and neither could a reader: a mistyped path used to print *"Nothing
about this download has been proven. Do not run it."* Missing files, unreadable
signatures and unhashable paths now say what actually happened, and mistyped
commands say `USAGE:` rather than accusing a release.

**5 is deliberately not 2.** A build here differing from the published build is
a finding to look into and publish. Most causes are boring, and calling it
tampering would be a claim this program cannot support.

Two tests read the program's own source and fail the build if any line prints
without asking the level first, one for standard output, one for standard
error. A quiet mode is only as good as the last line nobody remembered to gate,
and that omission is invisible: every other test still passes and the default
output is still correct.

### F-67: the group panel rendered with the default settings, not yours

The eighth audit round, over the voice limit, saved projects and profiles, and
the table of communication programs. Two defects, neither shipped.

Every question the group panel answered about voices, meaning how many speakers it
would allow, which mode it would let you switch to, and **the render itself**,
was computed from the engine's *default* configuration rather than from the
settings the rest of the window was set to.

So somebody who set the strength to its highest and turned accent
neutralisation on, then rendered a group conversation, got a render at the
default strength with the accent work off. It reported success. The controls
were on another tab and the panel had never been handed them.

The quieter half: how many voices stay clearly apart depends on the frame grid,
because a coarser grid snaps destination pitches onto wider steps. Under a
configuration where fewer than eight are separable, the panel still printed "8"
and still let eight people in, and two of them would have shared a voice,
discovered by listening to the finished recording.

The panel now carries the configuration, copied from the application before
anything is painted. The regression test moves the frame size to something that
genuinely lowers the count, and checks that the number shown *and* the number
enforced both follow.

### F-66: a saved project could come back different from how it went out

A value that trimmed away to nothing was written as a key with an empty value:
`Some("   ")` went out as `title  ` and came back as `Some("")`, which is neither what
was saved nor absent. A truncated project file carrying `plan  ` with nothing
after it yielded a plan path naming no file, which failed later with a message
about a file called nothing instead of being read as "no plan named".

Writer and reader are symmetric now, in both directions. The fix worth having
is the test: every shape a project can be in is saved, read, saved and read
again: empty and whitespace values, no members, the maximum members, no
outputs at all, and names containing the field separator and a line break. The
old test only ever exercised one tidy project.

### Talking through VeilVoice on Discord, Signal, Telegram, Matrix and the rest

```
veilvoice capture calls
```

It names this machine's virtual cable, says which of those programs are running,
and gives the exact menu for each one:

```
  your microphone  ->  veilvoice live  ->  a virtual audio cable
                                                   |
                                                   v
                                       the calling program, with
                                       the cable as its microphone
```

**Nothing has to know VeilVoice exists.** The program asks the operating system
for a microphone, the operating system hands it the cable, and the cable carries
a voice that is not yours. That is why the table is *not* a list of what is
supported, because anything that lets you pick a microphone works, including programs
nobody here has tested and ones that do not exist yet, and the command says so.

**Two things it does not do, said as plainly as the rest.**

* It changes **what you send and nothing else**. The other people on the call
  are not going through VeilVoice; their voices arrive as they always did, and
  if you record the call, their half is not veiled. Veiling a whole call means
  capturing what the program plays back, which is a different mechanism on every
  operating system and is not built.
* It **does not reach inside any of those programs**, not their traffic, not
  their audio, not their processes. That is deliberate rather than missing:
  intercepting an end-to-end encrypted call is the act this project exists to
  make useless, and a privacy tool that shipped a way to do it would be arguing
  against itself.

A running chat program is reported at the same low weight the screen-capture
monitor uses for a program that *can* share a screen: it is running, which is
not the same as being on a call, and treating the two alike is how a monitor
becomes noise nobody reads.


### Profiles and projects: `veilvoice-workspace`

Two things that sound alike and are not.

A **profile** is a named way of working, and three ship:

| | |
|---|---|
| **One person** | one speaker, everything this engine has turned on |
| **A group, a voice each** | capped at the measured number of separable voices |
| **A group, one voice for everybody** | nobody can be picked out by sound at all |

Every profile carries a paragraph saying what choosing it *means*, and a test
requires each of those to state a **limit** and not only a capability. A profile
called "highest security" whose name does the work of an explanation is the
thing this project refuses everywhere else, so it is refused here too.

Picking one is a starting point, not a lock: it sets the controls it names and
leaves everything else alone. A preset that overrode a choice made after it was
picked would be found out in the output.

A **project** is one piece of work: which recording, which plan, who is in it,
what they are called, what colour each is, which palette, what gets written.
Saved beside the recording so opening it next week puts everything back.

**It holds no audio and no passwords**, and the file says so in its own header
It is a thing you might send somebody so they can set up the same way, and if
it carried a passphrase, sending it would hand over the recordings too. It
*does* hold the speaker names you typed, and it says that as well.

Plain text, `VEILWORK1`, the same shape as a plan. An unknown keyword is
**refused**, not skipped: a project written by a newer build may describe a
setup this one cannot reproduce, and honouring half of it would render under
settings nobody chose. A profile or palette this build does not have is
**reported and left alone** rather than quietly swapped, the whole point of the
file is that it puts things back.

A gap in the speaker slots is refused too, because the slot *is* the voice: a
missing slot 1 would move everybody after it onto a different voice from the one
they were saved with, audible only as "somebody sounds wrong".

### Two things caught by tests rather than by reading

The member parser split on the first whitespace character, and the format
separates fields with **two** spaces, so `member  0  -  Alex` parsed as a
colour of `""` and a name of `"-  Alex"`. A round-trip test caught it in the
first run; reading the line would not have.

And the "no passwords in a project file" test tripped on the file's **own
denial**: the header says "no audio and no passwords", which contains
"password". `docs/AUDIT.md` records exactly this trap from a scope note, where a
search for "prevents" matched "nothing here prevents it". The test reads the
data now and not the prose about the data.

### The screenshot script stopped remembering where the toggle is

Adding the profile section above the group-mode toggle moved it, and the
capture's measured click landed elsewhere, so group mode stayed off and the
picture was of an empty panel with nothing to say it was wrong. The same
failure as the tab coordinates, and the same answer: the capture now turns
group mode on through the application's own "always start in group mode"
preference, written before the window opens and **put back exactly as it was**
afterwards. A screenshot script that leaves a preference changed is one that
edits somebody's configuration to take a picture.


### Eight voices, not ten: measured, and the group is capped at it

The engine holds ten destination voices and all ten are *different*. Only
**eight** are far enough apart that somebody following a conversation can tell
which is which.

That is a measurement, not a judgement. `separation` expresses both axes,
rendered pitch and vocal-tract scale, as ratios, because hearing is ratio-based
on both, and `clear_voices` walks the table asking when a new voice first comes
within three semitones of one already handed out:

```
 8 voices: closest pair 1.2500     <- 25 % apart, comfortable
 9 voices: closest pair 1.1842     <- 18 %, under the floor
```

The ninth is slots 4 and 8: **exactly the same rendered pitch**, vocal tracts
18 % apart. Group mode now stops at eight and says why.

**The first version of the metric was wrong, and measuring it is what showed
that.** It took the *smaller* of the two separations, reasoning that two voices
are only as separable as their closest resemblance. Run, it reported that three
voices were already indistinguishable, which is plainly false: slots 0 and 4
share a pitch and have vocal tracts 45 % apart, so one sounds like a much larger
person. A listener separates two voices by whichever cue is *strongest*, so the
measure is the larger of the two.

### One voice for everybody, which is the more private option

```
veilvoice conversation render plan.txt talk.wav --one-voice
```

Every speaker gets the *same* voice, and they are told apart by their names in
the subtitles and by which circle lights up in the picture. Two consequences,
one of each kind, and both are said where the option is offered:

* **It is more private.** In distinct mode the output carries one bit of
  structure the input had, as in *this is speaker three*, so anybody holding two
  recordings of the same group can line them up by voice slot. There is nothing
  to line up when everybody sounds the same.
* **It cannot be followed by ear.** That is the price, and it is why this is
  not the default.

It has **no speaker limit** from voices, because one voice cannot collide with
itself, so it is also the answer when eight is not enough, and the refusal at
nine says so rather than being a dead end.

Verified by measuring the audio, not by reading the log: rendering the same
two-speaker recording gives **93.8 Hz and 234.1 Hz** in distinct mode and
**93.8 Hz and 93.8 Hz** with `--one-voice`.

The mode is deliberately **not stored in the plan file**. A plan says who is in
the recording and when they speak; how it is rendered is decided by whoever is
rendering, and a mode hidden in a shared file would quietly change what somebody
else's render sounds like.

### The demonstration no longer says "left" and "right"

Below 640 px the three panels stack, so "the bars on the left" named the wrong
thing on every phone, and it never meant anything to a reader using a screen
reader at any width. The caption names the labels instead, which is true in
every layout and to every reader. A site test refuses the directions coming
back.

### `inset` needed its longhands, and one place needed them badly

`inset` arrived in Safari 14.1; an older engine drops the declaration entirely.
For the legal gate that is not cosmetic: it is shown with
`body { overflow: hidden }`, so an overlay that does not cover the page leaves
a reader unable to scroll with nothing visible stopping them. Both uses now
carry `top`/`right`/`bottom`/`left` first, and a test refuses a bare `inset`.


### Every crate now says what it is for in plain words

All eighteen, plus `fuzz` in its README. The technical explanation was already
there; this is the same thing said to somebody who does not write software, at
the end of each crate's own `//!` block, so it is reviewed in the same diff as
the code it describes.

> **`veilvoice-core`, in plain words.** This is the part that actually changes
> the voice. A recording goes in and a recording comes out. The words are the
> same and you can still understand every one of them; the voice is not yours
> any more, and there is no setting, no key and no clever program that turns it
> back.

**Required, not encouraged.** `tools/docs/generate.py` refuses to write a page
for a crate that has not got one, with the crate named, the same rule
`sources.py` already applies to the website's own files, under the same
heading. "We should document that" does not survive a busy week; a build that
stops does.

### F-65: two crates were invisible rather than uncovered

`veilvoice-check` and `veilvoice-update` were added to the workspace this cycle
and to neither of the documentation generator's crate lists. So they had no
page, no banner, no diagram, **and no entry under "not yet covered" either.**

That last part is what makes it a defect. `ALL_CRATES` exists precisely so the
tool can say what it is *not* covering rather than quietly covering less than
the tree contains. A crate in neither list is invisible rather than uncovered,
which is the one outcome those lists were written to prevent, and it was
reached by the ordinary act of adding a crate.

The lists stay written out, a generator that discovers its own inputs cannot
tell you it is missing one, but they are now checked against the workspace
manifest in both directions, and a mismatch stops the run with the names in it.
Both crates are documented: 751 files for 19 crates, up from 721 for 17.


### Seventh audit round: four defects, and one encoder proved sound

Four found and fixed, **F-61 to F-64**, all in code written this cycle. `main`
has not been released since v0.1.12, so "none had shipped" and "all were
written this cycle" are the same sentence, and the round says so rather than
claiming credit for it.

**Three of the four are the same shape: a comment that had stopped being true.**

* **F-61**: the verify tab's dropped file was read at the *end* of `update`,
  after the panel that shows it had been painted, so a drop and the highlight
  under a hovering file were a frame late. The comment above the call said
  "before anything is drawn". A wrong thing that agrees with itself survives a
  reading, and this one had survived several.
* **F-62**: nothing woke the window while a file hovered over it. An idle egui
  window repaints only when asked, and the repaint condition listed every busy
  state and no hovering state, so the drop target lit nothing up and the file
  did not appear until the mouse moved for some other reason. The one moment
  where the user is waiting for the window and the window has decided nothing
  is happening.
* **F-63**: the stylesheet comment beside the responsive-table rule said
  "nothing changes on a desktop". Measured: the tables render 820 px wide in an
  860 px column, so the row rules stop forty pixels short. `width: 100%` does
  not restore it, because the shrink happens on the anonymous table box inside
  the block. The trade is still right; the comment now says what it costs.
* **F-64**: a malformed `SHA256SUMS` line carrying a digest and no name could
  answer a lookup for `""`, which is what `Path::file_name` gives for a
  directory or `..` once it has been through `unwrap_or_default`. Not reachable
  from either front end, so a hole rather than a live defect. Both halves
  closed.

**What was checked and found sound.** The GIF encoder's dictionary-reset path
had never been seen by an independent decoder, because the banner's own frames
may never reach 4096 codes. A 512×512 field built to force it reset the
dictionary **66 times** and was decoded by Windows' GDI+ with **zero
mismatched pixels in 262,144**.

The front page's defect count is checked against `docs/AUDIT.md` by a site
test, which is what noticed the page still said sixty.


### Transcription and speaker detection: checked before built, and the check moved them

Both were decided in principle earlier in this cycle and neither is built. The
roadmap promised the provider question would be "checked before anything is
built rather than discovered by a user". It was, on a machine that has the
software, and the answer changed the plan.

`ollama` was the named candidate for the local half. Measured:

* **It is there and it is detectable.** Absolute path, version 0.32.5, seven
  models. The companion-detection pattern works on it exactly as it does on
  Audacity.
* **None of it transcribes speech.** Every model on that machine is a text
  model, and ollama's registry hosts language and vision models rather than
  speech recognition. Detecting ollama and offering transcription through it
  would have produced a feature that cannot work, on a machine where every
  check passed.
* **Running it is not free.** One `ollama list` started a background server,
  opened a local UI port, started an hourly update checker and made a network
  request to GitHub, all in the first two seconds, none of it asked for. For
  most programs that is unremarkable. For this one, "VeilVoice can use ollama"
  would have to be read as "VeilVoice can start a background service that
  phones home on a timer", and that has to be said in those words or not
  offered.

So markers 43 and 64 are **blocked** on a question rather than on effort. Local
speech-to-text means a Whisper-family program; diarisation means a third thing
again; and whether starting any of them is acceptable, given what was measured,
is the maintainer's call. The two honest paths, one microphone per person, or
a turn list, remain, and remain the default.

Nothing shipped for this. That is the point: the alternative was shipping a
feature that could not work, and finding out from somebody who trusted it.


### The desktop meters were arguing with themselves

The terminal's meters were fixed a few commits ago. The desktop application's
had the same fault and one worse: the bar was filled **linearly** while the
number printed beside it was in **decibels**. At ordinary speech the number
said -12 dB and the bar showed a quarter. Two meters disagreeing about the same
reading is worse than one bad meter, because it makes the reader doubt the
number as well as the bar.

The scale moved to `veilvoice-audio`, beside the thing that produces the peaks,
so both front ends now draw the same reading the same way. The desktop bars
gained the rest of it too: a **peak-hold hairline** that decays after a second
and a half, a clip colour on the same threshold, and a muted bar below -40 dBFS
so a quiet room does not read as a working microphone.

A test asserts the bar and the number agree, that the fill is exactly the
affine map of the decibel figure, because the failure this replaces was
precisely the two of them drifting apart while both looked plausible alone.


### The website's own source is documented, twice over

**In all three places**, as everything else here is: the repository, the
website, and the GitHub wiki. One generator writes all three from one header
comment, so they cannot disagree, which is the entire reason any of it is
generated.

The wiki is a single flat namespace shared with the crate pages, so these are
named `Source-*` and both generators sweep for orphans over exactly the paths
they own: `generate.py` skips `wiki/Source-` and `website/reference/source/`,
and `sources.py` sweeps them. A page about a file that has been deleted fails
the build in either direction.


Ten files the site invites you to open and read, eight scripts and two
stylesheets, had no page, no picture and no index, while every crate and every
`.rs` file had all three. `tools/docs/sources.py` gives them the same
treatment: a banner, a workchart of what calls what, a list of what is in
there, and an index.

Every page says what the file does **technically**, and then says the same
thing **in plain words**. Both are read out of the file's own header comment,
so they sit beside the code they describe and are reviewed in the same diff.

**The plain half cannot be generated, so the tool refuses to invent it.** A
sentence assembled from a filename is padding and a reader can tell, so
`sources.py` fails with the list of files that have no `In plain words`
section rather than writing a page without one. All ten have one now, written
for somebody who does not write software:

> **`website/js/verify.js`, in plain words.** This is the box on the verify
> page where you drop a file you have downloaded, and it tells you whether it
> is the one that was published. Your file never leaves your computer.

The workchart is a *syntactic* reading, an edge means the callee's name
appears, called, inside the caller's body, and the page says so, exactly as
the Rust pages do. A stylesheet has no functions, so its chart is its own
section comments, in order, which is the structure a stylesheet actually has.

Both generators now sweep for orphans in the directories they own, so a page
about a file that has been deleted fails the build rather than sitting in the
tree describing something that is gone.

**Found by looking at the page**, not by any test: the first version passed its
body to the page shell as a joined string, and the shell does `out.extend`, so
the string was extended one character at a time and the page rendered its own
markup as spaced-out text. Every check passed.


### Drag a download onto the window and be told what it is

New **verify** tab in the desktop application. Drop the download, the
`SHA256SUMS` and the `SHA256SUMS.asc` anywhere on the window, whichever tab is
open, and it says whether the file is the one this key published.

All three slots are visible from the start rather than discovered one refusal
at a time. Dropping one file and getting a verdict would be a lie, and an
interface that says "and now the other two" *after* the drop teaches people
that verification is fiddly rather than that it needs three things.

The order the check runs in is the whole of its value, and it is asserted
rather than assumed: **the signature is verified over the bytes of the list
before any number in that list is read.** A checker that compared the hash
first would, for the moment between the two, be trusting an unsigned document
and anyone who can hand you a file can hand you a `SHA256SUMS` to go with it.

### One implementation of the checking, not two

The arithmetic moved out of `veilvoice-verify` into a new crate,
`veilvoice-check`. The alternative was linking a GUI toolkit into the portable
verifier, which is the one program here whose *smallness* is a feature: it is
what somebody downloads before they trust anything else in this project.

The verifier is unchanged in what it does and what it prints. It is a caller
now instead of an owner, and the one place a silent accept could come from is
the one place there is only one of.

**The verifier's own test suite caught a regression during the move.** A
`?` where the original had a `continue` meant one malformed line near the top
of a `SHA256SUMS` made every hash below it invisible, and the answer would
have been "not listed", which reads as *wrong release* rather than as *this
file is unreadable*. Found within a minute of the code moving.

### The capture script no longer remembers where the tabs are

It **finds** them, by scanning the strip of pixels the labels sit in and
grouping the lit columns into runs. It used to remember, and those coordinates
went stale the first time a tab was inserted: every click still landed on *a*
tab, so every capture was different, the duplicate check saw nothing wrong, and
three tabs were quietly photographed under the wrong names. Two guards survive
from that, an identical consecutive pair stops the run, and after each click
the pixel above the label has to be the raised background a selected tab sits
on.

A third redaction: the lock tab prints where the app lock file lives, which is
under the account name. `assets/screenshots/README.md` names all three.


### Group mode can now actually render

The group tab could be configured and could do nothing. It has the rest of it
now: a recording, a plan, a title, a palette, and a **render** button.

The **plan** is where the turns come from, and the panel says so where it is
asked rather than after the fact: this panel knows *who* is in the recording,
and only a plan knows *when* each of them speaks. Audio no turn claims is
silenced rather than passed through, so a render with no plan would produce a
silent file, not a veiled one.

The names the panel holds win over the names in the plan file, because they are what
was just typed, and the turns are the plan's and are untouched.
`Conversation::rename_speakers` is new for that, and it **refuses a count that
does not match** rather than reconciling one: a plan naming three renamed from
a list of two would put somebody's audio in another person's voice, and since
both voices are unfamiliar, nobody would ever hear it. Nothing is changed
unless every name passes the same validation `add_speaker` applies, so a
refusal leaves the plan exactly as it was rather than half-renamed.

The **page palette** is the same nine, by the same identifiers, so a page
rendered here and one rendered by `veilvoice conversation render --theme` are
the same picture. It is per-run and resets to Tokyo Night, the same shape as
the mode toggle above it.

The render runs on a thread and the window never waits for it. A worker that
dies without answering is reported rather than leaving a spinner turning
forever.

### The gallery's declared image sizes cannot go stale

`width` and `height` on an `<img>` are what stop the page reflowing as each
picture loads. Those numbers went stale within an hour of being written, the
capture window was made taller so the group tab would fit, and a *wrong*
declared size is worse than none: the page reserves the wrong box and then
jumps anyway, under a reader who is mid-sentence. `tools/shots/terminal.py
--check` now reads each capture's real dimensions out of its PNG header and
fails if the page disagrees.


### The live meters now show a level a person can read

`veilvoice live` had meters. They were **linear**, which is arithmetically fine
and useless as a meter, because loudness is not. Ordinary speech recorded at a
sensible level peaks around -12 dBFS -- 0.25 linear -- so it filled a quarter
of the bar and read as near-silence. The only way to fill that bar was to be
clipping.

```
                dBFS                            the linear meter it replaces
silence         ····················  -60.0     ····················
room tone       ██··················  -54.0     ····················
quiet speech    ███████████▉········  -24.4     █···················
normal speech   ████████████████····  -12.0     █████···············
loud speech     ██████████████████··   -6.0     ██████████··········
shouting        ███████████████████▍   -2.0     ████████████████····
clipping        ████████████████████    0.0     ████████████████████
```

Now: -60 dBFS to 0, the number printed beside the bar, eighth-block characters
so twenty columns give a hundred and sixty steps rather than twenty, a
**peak-hold marker** that decays after a second and a half, and a **sticky
CLIP** warning -- clipping is destructive and over in a millisecond, and a
warning that has gone before the person looks up was never given. Below -40 the
bar is drawn muted, so a quiet room does not read as a working microphone.

It says what it is: a **sample peak** meter, not a loudness meter, and not a
true-peak one. It cannot see an inter-sample peak, a waveform that passes
above full scale between two samples and clips in a converter without any one
sample exceeding 1.0, and it says nothing about those rather than implying it
caught them.

Two things the tests settled rather than assumed. A wrong reading now pins the
meter **high** rather than low: both are wrong, and a meter stuck at the top is
noticed in a second while one stuck at the bottom looks exactly like an
unplugged microphone. And the clip threshold's own test had the numbers
backwards at first, a *linear* 0.99 is -0.087 dBFS, already inside a tenth of
a decibel of full scale. Decibels near the top of the scale are far finer than
they look in linear terms, which is most of the reason a linear meter is a bad
meter.


### Video and page renders follow the same nine palettes everything else does

```
veilvoice conversation render plan.txt talk.wav --page --theme gruvbox
veilvoice conversation preview plan.txt --theme nord
```

`veilvoice-video` knew one colour scheme. It now carries all nine the website
declares and the desktop application offers, with the same identifiers, and a
test reads `website/css/themes.css` and fails if any hex ever disagrees, the
same arrangement the app has had since the themes existed. A second test fails
if the stylesheet gains a theme this crate has never heard of, because a picker
offering nine and a renderer knowing eight is a picker with one entry that
silently draws the wrong colours.

The default is Tokyo Night. An unknown name is **refused**, and the refusal
lists every one it could have been: a picture quietly drawn in a different
scheme than the one asked for is worse than an error. The page follows the
theme unless a `--background` was asked for separately, in which case both
requests are honoured.

**The ten speaker colours do not change with the palette, and that is a
decision rather than an omission.** A palette here has six chromatic tokens;
ten mutually separable colours cannot be got out of six without inventing four,
and four invented colours are four whose separation nobody has measured. This
set *was* measured, the closest pair anywhere in it scores 63, and that pair
is only reached by a recording with nine or ten people. What the palette
decides is everything around them, so a Gruvbox render is a Gruvbox picture
with those ten circles in it. The ink drawn on each circle is computed rather
than assumed, and a test checks every one clears 4.5:1.

While the contrast arithmetic was there it was pointed at the palettes
themselves: body text on the page clears 4.5:1 in all nine, light ones
included.


### Pictures of the thing, in the README and on the website

Every tab of the desktop application, and ten of the command line's screens.
They are in the README, on the front page under **what it looks like**, and
mirrored under `website/assets/screenshots/` by the tool that owns them rather
than by hand.

**Two kinds, and they are different on purpose.**

`gui-*.png` are photographs of the running application, taken by
`tools/shots/gui.ps1`. It starts the release build, fixes the window to one
size and position, clicks each tab, and captures the window's real frame bounds
rather than the extended bounds, which include the invisible resize border and
the drop shadow, and put a strip of whatever is behind the window down both
sides of every picture. It also **fails rather than writing a wrong picture**:
each capture is compared with the one before it, and an identical pair means a
click coordinate has gone stale, which would otherwise produce a directory of
identical pictures with different names.

`cli-*.svg` are drawings, generated from the command output committed beside
them in `cli-*.txt`. `python tools/shots/terminal.py --check` regenerates every
one and compares, and it runs in `tools/verify.py`, so a picture of a command
line that disagrees with the command line fails the build. The `.txt` is the
file to read in a diff; an SVG diff is unreadable, a diff of what the program
printed is the review.

### Two of the pictures are redacted, and it says which

A screenshot of a working application is a screenshot of somebody's machine.
The live tab lists this machine's audio devices, meaning product names describing the
maintainer's hardware, and the install tab prints two paths containing the
**account name**, which is not the pseudonym this project is published under.
Both are painted over by the capture script, in the colours the interface draws
them in, and `assets/screenshots/README.md` names exactly what was covered and
what it says instead. Nothing else is altered.

### Two things the repository's own checks caught

The first capture ran `veilvoice` with `text=True`, which decodes with the
locale encoding, CP1252 on this machine, and the help screens are full of em
dashes. Every one was written as three wrong characters. The stray-character
suite, which has been in this repository since long before any of this, is what
noticed, three checks after the capture ran.

The redaction then failed with "a generic error occurred in GDI+", which is
what GDI+ says instead of "the file is locked": `new Bitmap(path)` holds the
file open for as long as the object lives, so saving back to the same path
cannot work. It draws onto a copy now.


### A check for updates that only happens when you press it

New crate, `veilvoice-update`, and a button on the desktop app's **about** tab.

**Nothing is automatic.** No timer, no check at startup, nothing in the
background. An update checker that runs by itself is a beacon: it tells a
server that this machine has VeilVoice on it, roughly how often it is used, and
from which address. That is what is being refused. A button somebody presses,
once, when they want to know, is a different act, and it is the only one on
offer. A test asserts that a freshly built panel has asked nothing and is
asking nothing, so a check at startup cannot be added without failing the build.

**There is still no HTTP client in the dependency graph.** The crate has no
dependencies at all. It runs the transfer tool your operating system already
ships, as `veilvoice-verify` has fetched releases since it existed, found at
an absolute path and never by bare name, because resolving a program by name on
Windows searches the current directory first. That is finding F-13, and it does
not get to happen twice.

**It downloads nothing and installs nothing.** It reports a version number and
leaves every decision to you. An update checker that could install its own
answer is one that can be made to install somebody else's. Every report carries
the caveat in the words the user sees: a version number on a page is not a
signature, and a download should still be checked with `veilvoice-verify`.

Being ahead of the newest release is reported as **ahead**, not as "up to
date": somebody running an unreleased build should know that is what they are
running. A version string this build cannot compare, a pre-release suffix, or
something that is not three numbers, is reported as unreadable rather than
ordered. Getting pre-release precedence subtly wrong is how a checker tells
people to downgrade.

**The wording changed in the same commit as the code.** The front page said "no
telemetry, no update check". No telemetry is unchanged and nothing here sends
anything about you. "No update check" has become "no *automatic* update check",
in the README, on the front page, on the security page, in the app's own
version screen, where "network access: none, by construction" is now "none,
except the update check you press", and in `veilvoice --help`, which can still
say it talks to no servers because the button is not in the command line.

### Found by running it, not by reading it

The first version scanned the release page for `/releases/tag/` and took the
first match. Against the real page it returned nothing: GitHub's release page
contains an **empty** `/releases/tag/` before any real one, so the first match
was an empty string and the check reported "no version number" against a page
that plainly had one.

Fixed, and improved past the bug: curl is now asked for the **redirect target**
rather than the page. `/releases/latest` redirects to `/releases/tag/vX.Y.Z`,
so the answer is one line. Measured on the real page: **54 bytes instead of
205,538**, with nothing of the reply's markup reaching the scanner at all.


### An installed copy no longer offers to install itself

The **install** tab is now only shown to a portable copy. A program that offers
to install itself when it already is installed is telling you something untrue
about what you are running, and there is no reading of that tab which is
correct in that case.

Under the tab's own header is the explanation of why it is there and how to
make it not be, because "why is this asking to install itself" is a question
asked while looking at the tab. The control itself lives in
**settings → interface**, deliberately: a tab that could hide itself and
nothing else could bring back would be a one-way door.

The check is read once, when the panel is built, and not per frame. It touches
the filesystem, and the tab row is drawn every frame, a `stat` in the paint
path is the exact shape of the defect that made this window freeze every couple
of seconds.

`veilvoice install` from the command line is unchanged either way.


### Group mode, where you can see it

The engine has handled several speakers since conversation mode shipped. The
desktop app has never shown it. There is now a **group** tab, and it is a
picture rather than a flag: a circle per person in their colour, their name
under it, and the destination voice each of them becomes.

**Off by default, and the toggle does not persist.** Group mode changes what a
recording is *treated as*, and a mode that survives a restart is a mode
somebody eventually forgets is on, which here means a recording of one person
rendered against a plan describing several, silencing everything the plan does
not claim. So the toggle is per-run, and a **separate, explicit tick**,
"always start in group mode", is the only thing written to disk. Two controls
where one would look like enough, deliberately: they answer two different
questions.

**A colour per speaker, assigned by slot.** A speaker's colour is a function of
their slot, exactly as their destination voice is, and for the same reason:
anything chosen by measuring the input would make an output property a function
of the input speaker, which is the linkage this project exists to destroy. Slot
0 and slot 1 are the furthest-apart pair in the table because two people is the
common case, and a test in the app asserts that stays true.

**Any colour, from any palette.** Clicking a swatch opens a picker: the ten a
slot can be given, and then every colour of all nine palettes the website
offers, each group named. An override is a person's choice about their own
recording, made after the fact, and carries none of the linkage problem above.

**Colour is never the only signal.** The name is drawn beside every circle, in
the list, and in the subtitles. A panel that separated speakers by colour alone
would be one about eight per cent of men could not use.

Outputs default to **audio, subtitles and page, all three**. A default that
produces less than was asked for is one that gets discovered after the
recording has been deleted.

Ten is the limit and it is stated rather than wrapped around: there are ten
destination voices, and an eleventh speaker would have to share one. Two is the
floor, because one speaker is not a group.

### Two things that were looked at rather than reasoned about

The speaker strip was built at 96 pixels a card, which turned "high register,
wide tract (234 Hz, 900 Hz)" into five wrapped lines, a row of circles reading
as a wall of text. And with the colour picker open the panel is taller than the
window, so without a scroller the picker was not reachable at all. Both were
obvious in a capture of the running application and invisible in the source.

**And the freeze that was reported against v0.1.12 is still gone.** Measured on
the running release build with the group tab open: 220 round trips to the
window's message loop over 25 seconds: median **0.19 ms**, 95th percentile
**15.6 ms**, worst **27.1 ms**, and never once flagged as not responding.

### Two decisions taken, and written down

The roadmap's two open questions were both answered, and neither quietly.

**Transcription may happen, and what leaves this machine is the veiled audio,
never the recording.** That is a narrower trade than it looks: the veiled audio
is the thing this project exists to produce, the words intact, the voiceprint
gone, so a provider given it receives a transcribable recording of a voice
that is nobody's. Sending the original would hand a biometric to a third party,
and that is what is being refused. Off by default; nothing added to the
dependency graph, because a local model is reached by running the program the
user already installed and a provider by the system's own transfer tool, as the
release verifier has always done for downloads; and the "talks to no servers"
wording changes in the same commit as the code, not after it.

**Detecting who is speaking ships no model.** If `ollama` is on the machine,
VeilVoice can offer to use it, named with who makes it, exactly as VB-CABLE and
Audacity are. If it is not, VeilVoice does not install one and does not pretend
to guess. One microphone per person and a turn list remain, and remain the
default.


### `veilvoice conversation` can draw what it made

`veilvoice-video` has existed and been fully tested since the conversation
work; nothing could reach it. Two commands now can.

```
veilvoice conversation render plan.txt talk.wav --page
veilvoice conversation preview plan.txt --audio talk.wav --at 4.5 --ffmpeg
```

`--page` writes a fourth file beside the audio and the two subtitle tracks: a
self-contained HTML player with the waveform, a circle per speaker that lights
when they speak, and the captions. It references the audio and the WebVTT track
**by name**, not by embedding them, so the four files move together and the
page does not double the size of a recording already on disk beside it.

The waveform drawn is the waveform of the **veiled** audio. Drawing the input's
would put a picture of the original signal next to a file whose whole point is
that the original is gone.

`preview` answers "what will I get" in a second rather than in the length of
the recording: the layout, the speaker circles, and which destination voice
each speaker becomes. It needs no recording at all, and without one the waveform
is drawn flat, which is honest about there being nothing measured yet.

`--width`, `--height`, `--padding`, `--background` and `--black` shape the
picture, and are **refused rather than clamped**: a 200-pixel render of nine
speakers is a question, and quietly drawing an illegible one answers a
different one. The flags are read whether or not `--page` was given, so
`--width 40` fails the same way with and without it.

`--ffmpeg` prints the command that would turn frames into a video file, and
says whether ffmpeg is on this machine. **It never runs it.** This project
ships no codec and starts no program you did not ask for.


### The front page now shows what the product actually does

A new section, **what happens to your recording**, between the banner and
the list of features. A file goes in, the voiceprint is destroyed, a file comes
out with the same words in a voice that is not yours. One eight-second CSS
cycle: the input card lights, a packet travels the connector, the engine pulses,
a second packet travels out, the output card lights.

It is text and CSS, for the same three reasons the banner is: it follows the
reader's palette, every claim in it can be selected and read aloud, and
`prefers-reduced-motion` reaches it, after which what is left is three labelled
cards side by side, which is the whole point of the picture. The motion is the
ordering, not the meaning.

Deliberately not another waveform. The section below it is two waveforms and
the mark between them and is about what the *signal* does; this one is about
what *you* do, so it is files and labels.

And it states its own limit in the caption rather than in a footnote: the
voiceprint goes, **what you said stays**, because the output is meant to be
listened to and transcribed. If the words themselves identify you, a name, a
place, a story only you could tell. VeilVoice has not touched that and does
not claim to. Segmental accent cues survive for the same reason.


### The banner: a GIF in the README, and CSS on the site

**The README animates again.** `assets/generate.py` gained a GIF encoder, its
own LZW, no dependency, no quantiser, so the animated banner is back in the
first thing every reader sees, in the one animated format every client draws.

The note beside the APNG had rejected GIF because "GIF is limited to 256
colours, so the palette would have to be quantised". That was right about
quantising and wrong about this picture, and one measurement settled it: the
whole banner uses **63** distinct colours, the waveform frames **255** between
them, and the two together **261**. 261 is over the limit for one palette and
well under it for the two GIF actually allows, a frame may carry its own
colour table. Nothing is quantised, nothing is approximated, and the bytes are
identical on every machine.

The one real loss is the frame delay: GIF counts in hundredths of a second and
1/60 s is not a hundredth of anything, so the GIF is 50 frames at 2/100 rather
than the APNG's 60 at 1/60. Same one-second loop, at the fastest rate the
format can honestly express. The alternative, rounding to 1/100, would give a
banner that runs at 60 % speed in some viewers and full speed in others.

The encoder was checked against a decoder that is not ours: Windows' own GDI+
reads the file, reports 50 frames, and reproduces the generator's pixels
exactly for frames 0, 1, 25 and 49, including the local colour tables and the
frame-to-frame composition. That matters because GIF's LZW has one detail every
implementation gets wrong: the decoder builds its dictionary one entry behind
the encoder, so the encoder must widen its codes one entry early. The wrong
rule produces a file this project can read back perfectly and no browser can.

**The website's banner is now drawn in CSS**, and has no soundbar. It is live
text with a veil drifting across the wordmark, the same letters, unresolvable,
which is the one illustration on that page of what the tool does. Three things
follow from it being text rather than pixels: it follows the reader's palette
instead of being baked in one of nine; its claims can be selected, searched and
read aloud; and `prefers-reduced-motion` reaches it, which no rule in a
stylesheet can do to a PNG. It is also legible on a phone, which the drawn
banner never was, and finding F-37 was this project's own claims rendered
illegibly inside an image at phone width.

The waveform is gone from it deliberately: the motif appears twice more on the
same page, in the demonstration and in the mark, and a third one at the top was
the loudest thing there competing with the two that carry meaning.

With scripts off, a `<noscript>` serves the README's GIF instead. Nothing in
the CSS banner needs a script; a reader who has chosen the edition of this site
that runs nothing is simply better served by one picture than by a page of
rules.

**Stated rather than left to be discovered:** `assets/banner-animated.png` now
has no consumer. It is still generated and still checked, because it is the
better animation: 60 fps, full alpha, half the bytes, and because deleting a
working generator is the maintainer's call rather than a tidy-up.


### The site stops scrolling sideways on a phone

Six separate faults, each found by measuring a rendered page rather than by
reading the stylesheet, and none of them visible in the source:

* A **grid item defaults to `min-width: auto`**, so the reference pages' content
  column refused to be narrower than its widest table, at 658 px, and took every
  heading and paragraph on the page sideways with it, at any viewport.
* A **table of item names** cannot be made narrow: the names are code. It is now
  its own sideways scroller. Deliberately at every width, not only on a phone:
  the reference pages keep a 200 px contents column until 720 px, so a tablet at
  768 px gives a table *less* room than a phone does.
* `overflow-wrap: break-word` does **not** shrink an element's intrinsic width,
  only `anywhere` does. With `break-word` alone the items table still scrolled
  inside a 630 px desktop column.
* A **tooltip** is `position: absolute`, anchored to the left of the word it
  annotates, and a `visibility: hidden` box still takes part in layout. A closed
  tooltip two thirds along a card pushed the **front page** 82 px sideways with
  nobody hovering anything. Below 900 px it is now pinned to the bottom of the
  viewport: full width, out from under the finger that opened it.
* The **hero** is the one section not inside `.wrap`, so it had no gutter: on a
  390 px screen the tagline's first and last characters touched both edges.
* `.search-page` set a `padding` **shorthand** on an element that already had
  `.wrap`'s `padding: 0 20px`. A shorthand replaces rather than adds, so the
  index's 200 rows ran edge to edge.

With scripts off there were two more, in a combination nothing had ever
rendered, the site had been checked with scripts off, and separately on a
phone, never both. The switch's "(locked: scripts are not running)" note is
257 px of `nowrap` beside the colour picker, which pushed every page 90 px
sideways; it now takes its own line, in full. And the static index's excerpts
are lines of source, one of them 1270 px of unbroken regular expression.

Twelve pages at five viewport widths, with and without scripts, now report no
horizontal scrolling at all.

**What this does not claim.** Every measurement here was taken in Chromium.
Firefox and WebKit have not rendered any of it, so the marker for "every
engine" stays open rather than being ticked on one engine's word.

### Two tools for looking, rather than reasoning

`tools/render/probe.py` is new: it drives the same headless browser
`shot.py` does and prints *numbers*, which pages scroll sideways, which element
is to blame, what any expression evaluates to on any page at any width. Every
fault above came out of it.

`shot.py` had a defect of its own worth writing down: it reuses one browser
profile directory between runs, so Edge cached `main.css` and photographed a
stylesheet one edit out of date. A fix measured as applied by one tool appeared
*not* applied by the other, and the two disagreed for a full round of debugging.
Both now disable the cache.


### The flowcharts are drawn at a size a person can read

Every generated flowchart laid one rank out on one line, so the canvas was as
wide as the busiest rank in the file. `veilvoice-core/chain.rs` reached
**4490 px**, and the drawing went into the page as `width="100%"` with no size
of its own, inside a column measured at 630 px. The browser scaled it to
**0.147**, a 13 px label rendered under two pixels tall. That number is a
measurement of the published page over the DevTools protocol, not an estimate;
the same measurement on a 390 px viewport reported a `scrollWidth` of 561, so
the wide drawings were part of what pushed the reference pages sideways on a
phone as well.

A rank now **wraps** into as many lines as it needs, the canvas is only as wide
as the widest line actually is, and every drawing carries its own `width` and
`height` with `max-width: 100%`. So a diagram renders at its own size on a
desktop and scales *down* on a narrow screen, never up, and never to a fifth
of legible. The widest canvas in the tree is now 649 px.

The repository and the wiki showed the same graph as a Mermaid fence, which
left the layout to GitHub. They now show **the same drawing the website shows**,
written to `assets/diagrams/`, with the Mermaid source kept underneath in a
`<details>` for anyone who wants GitHub to render it natively. One layout,
checked in one place.

`tools/site-tests/diagrams.test.js` is the permanent guard: a canvas over
900 px, a drawing with no intrinsic size, or a `.diagram` that does not scroll
on its own, fails the build.


### The installer has a window, and the companions are asked about properly

`veilvoice install` did the work already; it simply had no interface but a
terminal. The desktop application now has an **install** tab that calls the
same code, not a second implementation of it. The logic moved out of the
`veilvoice-cli` binary into a new library crate, `veilvoice-setup`, because a
binary crate has no consumers and two programs editing `PATH` independently is
how a machine gets broken.

The tab states what you already have before it offers to change anything:
portable is the normal case and is described as one. Beside the button is the
exact list of what an install touches, a copy into your own program
directory, a `PATH` entry appended to the value it first read, an Apps &
features entry on Windows, and nothing else. No administrator rights, no
service, no system directory.

### Companion software: detected, described, and never assumed

New in both front ends: VB-CABLE, BlackHole, PipeWire and Audacity are looked
for, and each is shown with who wrote it and under what licence **before** you
are asked anything.

```
veilvoice companions                      # report only
veilvoice companions --install audacity   # the explicit yes
```

Three rules, enforced in the library so no front end can be more permissive
than another:

- **Nothing is ticked, because there is nothing to tick.** Each is one button
  for one named program.
- **VeilVoice never runs somebody else's installer.** VB-CABLE is proprietary
  donationware and a driver, so the offer is to open VB-Audio's page and
  nothing more. Downloading and executing an unverified third-party binary
  would be a strange thing for a program whose subject is verifying what you
  run.
- **Privilege is reported, never requested.** A package manager that needs
  root has its command printed, not run. A window cannot honestly collect a
  `sudo` password.

Detection has three answers rather than two: found, not found where it usually
installs, or could not tell and here is why. The middle one is a statement
about where VeilVoice looked, not a claim about your machine, and the wording
says so.

### `veilvoice-sentry`: an early warning that says what it cannot do

The first of the security crates. Two signals, and the honest account of each:

**Canaries.** A file VeilVoice writes and nothing reads. If it ever changes,
something walked that folder and wrote to everything in it. Very few false
positives, and one large hole: it only fires if whatever is running *reaches*
that folder, so a quiet canary is not evidence that nothing happened, and the
wording never lets it read as one.

**Churn.** Record what a directory holds, look again later, and report how much
changed and how fast. No blind spot, and much weaker evidence: a backup
restore, a photo import, an archive extraction and a compiler all produce
exactly this shape, because they are all mass rewrites. The output is numbers
and a level against thresholds **you** set, and the highest level is called
"high concern" and phrased as a question -- "was that you?" -- because that is
a question the person at the keyboard can answer in a second and no amount of
file counting ever can.

```
veilvoice sentry plant ~/Documents     # a canary, and a note on its limits
veilvoice sentry baseline ~/Documents  # what is there now
veilvoice sentry check                 # both, against thresholds you set
```

`check` exits non-zero only when a **canary** tripped, which is a fact. Churn
never fails the command at any level: a check that fails every time somebody
copies a folder is a check somebody deletes from their scheduled task.

Entropy is reported for a rewritten canary and is described as what it is. Near
8.0 bits per byte means incompressible, which is true of encrypted data and
equally true of every JPEG and `.zip` on the machine, so the line says both.
It is used only for a file this crate planted and therefore knows was prose.

Nothing here names the program responsible, and nothing here stops anything.
Stopping a process mid-run needs an interposition point in the kernel, and on
Windows that needs a code-signing identity issued to a verified legal entity --
which conflicts with publishing under a pseudonym. That remains a decision
rather than an omission.

### `veilvoice-policy`: settings that can only be tightened

The second security crate, and the design turns on one problem.

To *apply* a policy at every launch, VeilVoice has to be able to read it at
every launch. If reading it needs a passphrase, you type one every time; if it
does not, anybody who can write the file can rewrite the policy. The usual
answers are a privileged daemon holding the key or a key hidden in the binary,
and neither is honest here -- this project needs no privileges, and a key
inside a binary anybody can download is not a key.

So the constraint went into the shape of the data instead. **Every requirement
a policy can express makes VeilVoice stricter.** There is no requirement that
turns encryption off, none that lowers the de-identification floor, none that
disables the app lock, and no room in the format to write one. Somebody who
edits the plain file without the passphrase can therefore do exactly one thing:
make this machine's VeilVoice *more* restrictive than its owner asked for. That
is a nuisance and not a privacy failure, which is why the plain file is read and
applied without a passphrase and reported honestly as "seal not checked".

```
veilvoice policy seal --encrypt-recordings --minimum-intensity 80 \
    --note "Set by whoever set it. Ask before changing."
veilvoice policy status    # what is fixed, and why each one is fixed
veilvoice policy verify    # needs the passphrase; proves the seal matches
```

The desktop application draws every fixed control disabled with the reason
underneath, because a disabled control with no explanation is a bug report. The
*enforcement* is not the drawing code: the values a job actually uses come from
a constrained posture, so a policy holds even if a control is drawn wrongly --
the same rule the at-rest dialogue has always followed.

Two things a policy will not do. It will not set your app lock, because that
needs a passphrase only you have; a required lock is announced beside the
control that satisfies it and the application stays usable. And `veilvoice
policy remove` does not ask for the passphrase, because it could not usefully:
anybody who can run that command can delete the same two files with a file
manager, and pretending otherwise would teach you something false about what
this program does.

### `veilvoice-drivers`: what is loaded in the kernel, and what changed

The third security crate. Record the loaded drivers, look again later, and say
what appeared, disappeared or changed. Loading a driver is the step almost
everything that wants to watch a microphone from underneath has to take.

| Platform | Source | Needs privilege |
|---|---|---|
| Linux | `/proc/modules`, cross-checked against `/sys/module` | no |
| Windows | `driverquery.exe /FO CSV /NH` | no |
| macOS | `kmutil showloaded`, falling back to `kextstat` | no |

**The limit is stated before the feature.** This reads a list the operating
system hands out, so anything able to lie to that list is not in it. A quiet
report is not evidence that nothing is hiding, and the wording never lets it
read as one.

**The cross-view check, honestly.** On Linux the kernel publishes the same fact
twice, and modules present in one list and absent from the other are reported.
That catches something which unlinked itself from `/proc/modules` and forgot
`/sys/module` -- a mistake real rootkits have made. It catches carelessness and
nothing else: both lists come from the same kernel, so anything with the
privilege to edit one can edit both. No other platform here has a second list,
so the check is empty there and `support()` says that means "nothing was
checked", not "the check passed".

**A new driver is not a finding.** Printers, graphics updates, VPN clients and
virtual audio cables -- VeilVoice recommends one -- all load drivers. A change
is a fact about a list, and a test fails the build if any of the wording ever
turns into an accusation.

Load addresses are deliberately not recorded on either Linux or macOS: they are
zeroed for an unprivileged reader on a machine with kernel-pointer restriction,
and change at every boot on one without. Recording them would report every
module as altered after a restart, which is a report nobody opens twice.

### `veilvoice-capture`: you can record VeilVoice, and it will stop nagging

Asked for directly: notice when something like OBS is recording, let the
notification be switched off for a program you meant to run, and **do not get
in the way of recording VeilVoice itself**.

**The last one first, because it is the question people actually have.** You
can record this application with OBS or anything else. Screen capture of the
VeilVoice window is not blocked, not degraded and not treated as an attack. If
you are making a video about it, or streaming while you use it, it appears on
the recording like any other window.

That is a choice and also a limit. Excluding a window from capture means
`SetWindowDisplayAffinity` on Windows and its equivalents elsewhere, which is
FFI, and every crate in this workspace carries `#![forbid(unsafe_code)]` --
which is a front-page claim. So the exclusion is **not built**, ROADMAP marker
34 is now marked blocked rather than planned, and anybody who needs a window
that cannot be recorded should know they do not have one here.

```
veilvoice capture status         what is running, what is allowed
veilvoice capture list           every program this build knows
veilvoice capture allow obs      stop notifying about that one
veilvoice capture check          exits non-zero if something unallowed is running
```

**Allowed means muted, not hidden.** An allowed program still appears in
`status`; only a notification filters on it. Something that vanished from the
interface entirely would be a setting for lying to yourself. Allowing a key
this build does not know is refused, with the known keys printed -- a
misspelled entry would silently allow nothing while you believed a
notification was off.

Two more limits, both in `SCOPE` with tests holding the wording:

- It only knows the programs in its table, so an **empty report is not evidence
  that nothing is recording**. Something written to record a screen quietly
  would not be called `obs64.exe`.
- **Running is not recording.** Zoom being open is not somebody watching your
  screen, and the table separates a program whose job is recording from one
  that merely can share. Telling the difference needs the compositor to say who
  holds a capture session, which is FFI again.

Linux reads `/proc/<pid>/comm` and spawns nothing; Windows and macOS ask
`tasklist` and `ps`. Two entries in the table carry both their full name and
the fifteen-character form the Linux kernel truncates `comm` to -- listing one
would have matched on one platform and silently not on the other, which is the
shape of bug this project keeps finding in itself.

### Also

- `veilvoice-setup` is usable on its own: no dependencies, no `unsafe`, and
  the Windows registry reached through `reg.exe` as everywhere else.
- The setup tab's progress strip claims no percentage. `reg.exe` and a package
  manager report no progress, and a bar that fills to 90% and waits is a lie
  with a shape.

## v0.1.12

Fetching and checking a release in one command, installing without an
installer, and diagrams that open the code.

### Verify a release without downloading it yourself

```
veilvoice-verify release v0.1.12
veilvoice-verify release v0.1.12 veilvoice-v0.1.12-linux-x86_64.tar.gz
```

Fetches the hash list and its signature, checks the signature **first**, then
checks the file against the list. The order is deliberate: checking a hash
first would prove only that a download matches a list which might itself have
been replaced.

**This program still contains no HTTP client.** VeilVoice has no networking
crate anywhere in its dependency graph, a property you can check yourself with
`cargo tree`, and one CI fails the build over. The download is done by the tool
your operating system already ships: `curl.exe` on Windows, curl or wget
elsewhere. Only one host is ever contacted and it is compiled in; there is no
way to point this at another, no update check, and nothing is fetched unless
you asked for it on the command line.

### Install, or don't

```
veilvoice install          # copy, add to PATH, register for removal
veilvoice install --status # what is installed, and which copy is running
veilvoice uninstall --yes  # undo exactly those three things
veilvoice gui              # open the desktop application
```

**Portable remains the default.** VeilVoice runs from wherever you unpack it
and nothing has to be installed. This exists so that typing `veilvoice` in a
terminal works.

Everything is **per-user**: no administrator, no system directory, no service.
The PATH entry is appended to the value that is already there and removed
without touching anything else, an uninstaller that rewrites PATH from a
template destroys whatever else you had, at the moment you are least likely to
check.

Installing tells you, once: this program never checks for updates and cannot
tell you when one exists, because it has no network code. Watch the releases
page, and verify what you download.

### Diagrams that take you to the code

Every box in every flowchart is now a link to the exact line it stands for, in
both the Markdown on GitHub and the SVG on the website. Nodes carry their line
number, and are coloured by role, a way in, a public function also used
internally, or a private helper, with a legend saying which is which.

Each page also opens with **what the file contains**: how many functions, types
and constants; the types it owns; and the ways in, with what calling each one
reaches. All of it read out of the source, so none of it can disagree with the
code.

The diagrams on the website are painted with the site's own colour tokens, so
they follow whichever of the nine themes you chose, or one you wrote yourself,
instead of being Tokyo Night in the middle of somebody's Gruvbox.

### Fixed

- Rustdoc's `[`name`]` links rendered with their brackets showing, on 63
  generated pages.
- The generated pages' navigation still said "search" after the site renamed it
  to "index".
- The website was missing a section the Markdown had, while both claimed
  parity.

## v0.1.11

**If you have v0.1.10 on Windows, replace it.** The desktop application flashed
a console window and could fail with no message at all.

### The Windows desktop application

Three separate defects were behind one report: "it flashes a command prompt,
loads in an unusable state, and crashes".

**The flashing console was never the application's own window.** `veilvoice-gui`
has no console. Every *subprocess* it starts has one, and on Windows that means
a window appears and vanishes as the child runs: once at startup, when the
application asks the system whether animation is wanted, and again on every
poll while the monitor tab is open. Every subprocess in the project now starts
with `CREATE_NO_WINDOW`, and a test in each crate fails if one is added without
it.

**A failure used to produce nothing at all.** No console, and the release build
aborts rather than unwinds, so a crash left no message, no dialog and no log,
nothing to report but "it crashed". VeilVoice now writes a short report beside
your preferences and tells you about it next time it starts. **It is written on
your machine and sent nowhere**; there is no network code in this program to
send it with.

If the window never appears at all, the most likely cause is that the computer
could not provide an OpenGL context, which is common in a virtual machine, over a
remote desktop session, or with hybrid graphics. The report says so, and points
at `veilvoice`, the command-line tool, which does the same work and needs no
graphics.

### Icons

Every executable now carries its icon, on every platform.

- **Windows:** embedded in the binary, at all six sizes. It was previously
  shipped as a loose `.ico` beside the program, a file Windows never reads,
  so Explorer, the taskbar and any pinned shortcut showed the generic
  executable glyph. A release check reads the built binary and fails if the
  icon is missing.
- **macOS:** an `icon.icns` with six sizes.
- **Linux, FreeBSD, NetBSD, OpenBSD:** a `.desktop` launcher entry and hicolor
  theme icons at six sizes, which is how those desktops find an application's
  icon. There were none before.

All of it comes from `assets/generate.py`, from the same pixels as everything
else, and `--check` verifies it.

### Licence

**VeilVoice is GPL-3.0-or-later.** v0.1.10 was briefly published under a
different licence; that is reverted, and this and every future release are
GPL-3.0-or-later, as every release before v0.1.10 was.

## v0.1.10

Documentation you cannot outrun.

Every crate and **every one of the 63 `.rs` files** in this repository now has a
page, a flowchart and a banner, generated from the doc comments in the source
and mirrored to the website and the GitHub wiki. A fifth audit round found
twelve defects ([`docs/AUDIT.md`](docs/AUDIT.md)); two of them had shipped.

### A page for every file

`tools/docs/generate.py` writes 366 files from the `//!` and `///` comments
already in the tree: a README for each crate, a page for each `.rs` file, a
generated SVG banner and a Mermaid flowchart for each, a table of contents on
every page, and the same content rendered again for `website/reference/` and
for the GitHub wiki.

Four decisions are built into it. Flowcharts are Mermaid, so a diagram stays
text -- diffable, greppable, reviewable in a pull request. Banners are generated
SVG rather than 63 more committed binaries. Per-file prose is *extracted from
the source*, so it cannot disagree with the code; if a page reads thinly the
fix is to write the doc comment, which improves rustdoc at the same time. And
the website and wiki come from one generator, not two hand-maintained copies.

The site loads nothing from a third party, so it cannot run Mermaid. The same
graph is laid out by a small engine in the generator and emitted as inline SVG
that needs no script at all -- the same nodes and edges, a different picture,
and the page says so rather than glossing it.

`python tools/docs/generate.py --check` runs in CI and fails if the tree and its
documentation have parted company. It also refuses to overwrite a file it did
not write, after doing exactly that to `fuzz/README.md` once.

### Your own colour schemes

Drop a `.palette` file beside your preferences and it appears in the theme
picker with the nine built-in schemes. All twelve tokens are required, every
colour must be a full `#rrggbb`, and every problem is reported rather than the
first -- nothing is quietly filled in from the default theme.

**Contrast is computed, not trusted.** A palette whose text fails the WCAG
ratio against its own background is refused, with the measured ratio in the
message so you know how far off it is. `docs/example.palette` is a worked
example that passes.

Pointing that same check at this project's own themes found that the default
theme's secondary text was below the accessibility floor -- on the site, in the
app, in the terminal and inside the banner image. Fixed, using each palette's
own upstream colour.

### Smaller things

- The front page carries a slowly cycling line of things that are true about
  this project, several of which are limits rather than boasts. CSS rather than
  an image, so it follows your theme, needs no script, and can be selected and
  read aloud.
- The banner's waveform is summed from three harmonics rather than one
  sinusoid, so it reads as audio rather than as a test tone.
- The repository panel no longer shows a README's own markup as text.
- The site's search is presented as the *index* it is. The URL is unchanged.
- `tools/render/shot.py` drives headless Edge over the DevTools protocol, with
  no dependency, so pages can be rendered and looked at before being believed.
- `ROADMAP.md` is the public answer to "what is coming", with 45 markers and
  what each depends on.

## v0.1.9

Getting a genuine copy, and finding your way around one.

This release adds a search index over the whole repository and website, a
portable verifier that checks a release **without GnuPG installed**, install
scripts that refuse rather than continue, package definitions for six formats,
and OpenBSD and NetBSD builds. A fourth audit round found ten defects
([`docs/AUDIT.md`](docs/AUDIT.md)); three of them had shipped.

### Verify a download without installing anything

`veilvoice-verify` ships in every archive. It carries the signing key and the
fingerprint `8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A` compiled in, needs no
GnuPG, uses no network, and writes nothing.

```
veilvoice-verify key
veilvoice-verify file veilvoice-v0.1.9-linux-x86_64.tar.gz \
    --sums SHA256SUMS --sig SHA256SUMS.asc
```

It cannot embed the expected hash of the file it checks -- a file cannot contain
its own digest -- so the hash comes from outside, and **where it came from
decides what a match proves**. A hash from the published `SHA256SUMS` proves the
download is *intact*, and rests entirely on trusting whoever signed it. A hash
from somebody else's build of the same tag proves the release is *reproducible*,
and rests on nobody in particular. The tool says which one it just established
and refuses to run both at once and report a single answer. `--explain` sets out
the difference at length.

### Install scripts that refuse rather than continue

`install/install.sh`, `install/install.ps1` and `install/install.bat`. Each
checks the key's fingerprint against a value **hardcoded in the script**,
verifies the signature over `SHA256SUMS`, verifies the archive against that
list, and only then installs. In that order: checking the hash first proves only
that a download matches a list that might itself have been replaced.

There is no flag to skip verification, because an installer with one is an
installer whose verification is decorative. Without GnuPG the scripts stop
rather than falling back to "the hash matched". Every refusal names the check
that failed and installs nothing.

Optional extras are asked once, default to **no**, and are not installed at all
under `--yes` -- which means "do not ask me", not "assume yes". VB-CABLE is
proprietary donationware, so the Windows script only opens VB-Audio's page: it
will not accept somebody else's licence on your behalf.

Documented in [`docs/INSTALL.md`](docs/INSTALL.md), which puts the **by-hand**
route first, because "run this script and trust it" is a strange thing to ask on
behalf of a tool whose argument is that you should not have to trust anybody.

### Search the whole project

A new [search page](https://tilas01.github.io/veilvoice/search.html) indexes
every tracked file -- the Rust and its doc comments, the documentation, the
website, the tests, the build and the licences -- with sorting and filtering.
Results link to the exact line on GitHub or the exact section on the site.

It works **without JavaScript**. `website/nojs/search.html` is a complete static
index of every file and section, generated from the same walk of the repository,
so the two cannot disagree. The whole corpus is in the page and your browser's
own find-in-page searches it. Everything is expanded rather than folded away,
because text inside a collapsed `<details>` is not searchable on every browser,
and an index that answers confidently with nothing is worse than no index.

Coverage is stated precisely rather than rounded up: documentation, the website,
the build files and the licences are complete; Rust is indexed by item name and
doc comment, **not** by function body.

### Packaging and platforms

WiX (Windows MSI), `.deb`, `.rpm`, Flatpak, a Homebrew formula and a Gentoo live
ebuild, in `packaging/`. Every one builds from the tagged source with `--locked`
rather than repackaging a binary. The Flatpak requests **no network permission**,
which is the checkable form of the offline claim. Nothing installs a service, a
scheduled task or anything that runs at startup.

**None of the package definitions has been built or installed yet** -- they
parse, and that is the whole of what is claimed.
[`docs/PACKAGING.md`](docs/PACKAGING.md) carries a per-format status table.

OpenBSD and NetBSD builds join FreeBSD. All three run in emulated VMs, are
allowed to fail without blocking a release, and are marked `not-verified` for
reproducibility because they are built once rather than twice.

### Fixes

- **The website rendered the text in its own banner illegibly (F-37).** Every
  image carried `image-rendering: pixelated`, which is for pixel art being
  *enlarged*; every image here is *shrunk*. Nearest-neighbour sampling deletes
  the rows it discards rather than blending them, so on a phone the hero read
  `TIE VOICEPRIN IS DESTOYED. THE WORDS STAY EDGRE.` and
  `BY ~I,FS01 CN GITHUB` instead of the claim, the licence and the authorship.
  It had been that way for as long as the banner existed, on every viewport,
  with every test passing. Found by rendering the page and reading it.
- **The website's artwork had drifted from its generator (F-41).**
  `website/assets/` held hand-maintained copies that `--check` never looked at.
  The generator now writes both and checks both -- "generated from source" and
  "generated, plus a copy somebody maintains by hand" are different claims.
- **A documentation link pointed at a file that does not exist (F-42).** Now
  mechanical: a site-test suite resolves every local link in every tracked
  document.
- Seven further defects were found in code written during this round and fixed
  before release. They are written up at the same length in
  [`docs/AUDIT.md`](docs/AUDIT.md) rather than omitted, because a round that
  quietly drops them looks cleaner than it was.

### The banner

No bar is missing from it any more. The waveform used to drop one bar in five to
suggest "the signal coming apart", which read as a broken image -- and was the
wrong idea anyway: VeilVoice does not remove signal, it destroys the structure
that identifies a speaker while keeping every word. The left half is now a
coherent travelling wave and the right half is the same bars with the phase
relationship between them gone.

It is animated, as a 24-frame APNG generated by `assets/generate.py` like every
other asset here, and served through `<picture>` so that a reader who has asked
their system for less motion gets the still one. A browser without APNG support
shows the first frame, which is byte-for-byte the static banner.

### A dependency worth naming

The portable verifier uses rPGP so that checking a release needs no GnuPG. It is
by far the largest dependency in this project, and it brings in the `rsa` crate,
which carries [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071)
with no fixed version available.

That advisory concerns RSA **private key** operations. The verifier only ever
checks a signature against a public key compiled into it; there is no private
key in this repository and no secret for a timing side channel to leak. Because
that is an argument about *usage* rather than about the crate, CI now fails the
build if a secret-key or decryption API appears in the verifier -- so the
argument cannot quietly stop being true. Reasoning in `.cargo/audit.toml` and
`docs/AUDIT.md` (A-6).

The alternative was hand-writing OpenPGP parsing and RSA verification, where a
subtle mistake is a silent accept in the one tool whose job is not to silently
accept.

### Still not done, and said plainly

- **Nobody but the author has run the install scripts or the verifier.** They
  are tested end to end on Windows against the real published v0.1.8 release.
  `install.sh` has never run on a real Linux or macOS machine.
- **No package definition has been built.**
- The `fuzz/` targets still have not been run to convergence, and 32-bit targets
  are still not in CI. Both unchanged from v0.1.8.

336 tests, ten website suites, no `unsafe`, no networking crates.

## v0.1.8

A security release. A third audit round, run against the *classes* of defect
rather than against a list of things that seemed worth checking, found and fixed
**twenty-eight** of them -- thirteen in the Rust, fifteen in the website.

Nothing here is a confidentiality failure: no finding let anyone recover a
voiceprint, read a sealed recording or bypass a password. Several are worse than
that sounds anyway, because they are failures of the thing being relied on. Full
write-ups, one per finding, are in [`docs/AUDIT.md`](docs/AUDIT.md).

### Security fixes -- the engine and the tools

- **A four-kilobyte WAV killed the process (F-9).** A file declaring a sample
  rate of zero made the decoder panic inside its own probe, before VeilVoice saw
  anything it could check -- and the release profile sets `panic = "abort"`, so
  that was the program ending rather than an error. `veilvoice anonymise` on a
  file somebody sent you was the whole of it. Now pre-flighted and refused with
  an explanation.
- **A configuration value made every output sample silent (F-10).** `NaN`
  passed validation, because `NaN` compares false against every bound. The
  engine built happily and produced `NaN` for the rest of the session with
  nothing reported. The same shape as the v0.1.6 finding about a single bad
  *sample*, reached through the configuration instead. An absurd sample rate --
  which a WAV header can carry, as a `u32` -- also asked for about two gigabytes
  of delay lines from a four-kilobyte file.
- **Secure erase destroyed the wrong file (F-12).** It followed symbolic links,
  so erasing a link filled its *target* with random data, unlinked only the
  link, and reported success. It now refuses a link and says why.
- **Secure erase never finished on 32-bit builds (F-11).** A 4 GiB file
  truncated a length to zero and the overwrite loop ran for ever, leaving the
  file intact. Reachable only on the ARMv7 build; found by reading.
- **A planted executable in the working directory was run (F-13).** On Windows
  the program-search order includes the current directory, so
  `Command::new("reg")` in the monitor and `wevtutil` in the tamper detector
  could execute a file that happened to be sitting beside you. Both now resolve
  to absolute paths under the system directory.
- **Secrets were created world-readable and tightened afterwards (F-14, F-15).**
  The app-lock verifier -- rewritten after *every failed unlock attempt* -- plus
  `keygen`'s private key, `decrypt`'s plaintext output and an unencrypted
  recording. All now created owner-only in the first place, through a new
  `veilvoice_crypto::privatefile` module.
- **The post-quantum shared secret was not zeroized (F-18).**
- Plus: an unbounded decode that a compressed file could turn into an
  out-of-memory abort (F-17); a corrupt WAV the metadata cleaner could hand back
  as clean (F-19); app-lock cost parameters validated too late (F-20); a
  manifest that reported every recorded file as new (F-21); and a Windows
  attribution query whose escaping was wrong, so it told the user the wrong
  reason it could not see (F-16).

### Security fixes -- the website

- **The Markdown renderer could freeze the reader's tab.** Two independent
  quadratics, measured rather than guessed: 128 000 characters on one line took
  **eight seconds**, and a second shape took **fourteen**. That is on the main
  thread, on text the page fetches over the network. Both are now linear
  (F-22, F-23).
- **A deeply nested blockquote crashed the render** and the reader was told the
  network had failed (F-24).
- **Download links from the GitHub API were assigned with no scheme check**
  (F-26) -- the item the previous audit listed as open. A refused asset is still
  named, just not clickable.
- **The legal gate was an invisible modal** on any engine older than Chrome 111,
  Safari 16.2 or Firefox 113: `color-mix()` with no fallback left it with no
  background while it still locked scrolling (F-30). And it **could not be
  dismissed at all on an iPhone**, because `88vh` put the continue button below
  the visible area while the page behind was scroll-locked (F-33).
- **No focus ring at all on Safari before 15.4** (F-31), **no header blur on
  iOS 17 and earlier** (F-29), and **native controls drawn light on a dark
  page** (F-32).
- **The mobile header took a fifth of the screen** -- 165 px of an iPhone's
  812 px, at every scroll position -- and its links were below the minimum
  touch-target size. Now 79 px with a single scrolling row (F-34).
- A code fence could reach `Object.prototype` (F-25); a malformed API response
  was reported as a network failure (F-27); repo-relative links resolved
  somewhere other than where they pointed (F-28); the in-browser verifier gave
  an unusable error on an insecure origin (F-35) and used twice the memory it
  needed (F-36).

### Added

- **`fuzz/`** -- six coverage-guided libFuzzer targets, one per parser that
  reads bytes somebody else produced, with overflow checks deliberately left
  **on** in an otherwise optimised profile, because two of this project's shipped
  defects were arithmetic overflows that a release-profile fuzzer cannot see.
  Built and type-checked against the real APIs; **not yet run to convergence**,
  and `fuzz/README.md` says so plainly rather than implying otherwise.
- **A KDF cost ceiling for unattended callers** --
  `container::open_with_password_within` and `KdfParams::UNATTENDED_MAX_M_COST`.
  A hostile container can declare a legal-but-expensive cost; a service
  processing files it did not choose can now decline instead of spending the
  memory. The other item the previous audit listed as open.

### Testing

- 269 tests to **285**, and five website suites to **seven**.
- **`tools/site-tests/markdown.complexity.test.js`** asserts the renderer stays
  *linear*, by measuring the same shape at two sizes and comparing the ratio.
  Each measurement runs in a child process with a timeout, because a regular
  expression cannot be interrupted once it starts backtracking -- an in-process
  version would hang instead of failing, and a hung CI job gets retried while a
  failing one gets read.
- **`tools/site-tests/repo.test.js`** drives the repository panel against
  scripted hostile API responses through the real DOM path.
- **`tools/site-tests/css.test.js`** checks the cross-engine invariants that
  have no build step to enforce them: a `-webkit-` prefix beside every
  `backdrop-filter`, a plain colour before every `color-mix()`, a `:focus`
  fallback beside every `:focus-visible`, a `color-scheme` on every theme
  matching its own background, and a `dvh` upgrade after every `vh`.

## v0.1.7

### Added

- **Tamper detection** (`veilvoice-guard`, `veilvoice guard`). Records a
  SHA-256 manifest of VeilVoice's own files and reports what was modified,
  removed or added since. `--sealed` encrypts the record under a passphrase, so
  rewriting it to match a tampered file needs the passphrase as well as write
  access -- keep that passphrase somewhere other than beside the record.
  - It is **detection, not prevention**, and never says otherwise. Nothing that
    runs as an ordinary program can stop another program with the same rights.
  - Attribution is best-effort and usually unavailable: naming the responsible
    program needs the Linux audit subsystem or Windows object-access auditing,
    both normally switched off. It reports that it does not know, and prints
    what an administrator would have to enable, rather than guessing.
  - `veilvoice guard check` exits non-zero when anything changed, so a script
    can act on it.

### Security

- **Typed passphrases are moved into page-locked memory immediately.** The GUI
  used to keep the confirmed session passphrase as a plain `String` for the
  whole session, and the CLI passed copies around after the prompt. Both now
  convert to a zeroizing `Secret` the moment the passphrase is confirmed and
  wipe the buffer it arrived in, narrowing the exposure from "until the app
  closes" to "while the user is typing". Audit A-5 is updated to separate the
  part that was fixed from the part that remains accepted: the typing window
  itself cannot be removed, and none of this helps against an attacker who can
  read the process's memory.

## v0.1.6

### Breaking

- **`veilvoice anonymise` now writes an encrypted `.veil` container.**
  `-o clean.wav` produces `clean.wav.veil`. Open it with
  `veilvoice decrypt clean.wav.veil -o clean.wav`, or pass `--encrypt false`
  for the old behaviour, which prints what you are giving up and waits for you
  to type `UNENCRYPTED`.

### Added

- **At-rest encryption, on by default.** Every recording VeilVoice writes is
  sealed as it is written. The WAV is encoded in memory and encrypted there, so
  a recording that is going to be encrypted never touches the disk in the
  clear -- a plaintext file that is written and then deleted cannot be reliably
  taken back on flash storage.
- **Seal to a public key** instead of a passphrase: `--encrypt-to key.pub`,
  X25519 + ML-KEM-768 hybrid. Also offered in the desktop app.
- **An application lock.** A separate, rate-limited password gates the desktop
  app. Argon2id verifier, constant-time comparison, domain-separated from the
  recording passphrase. Three attempts are free, then the wait doubles from 5 s
  to a fifteen-minute cap, and the count is written to disk so restarting the
  app does not reset it.
  - `veilvoice lock set | status | change | remove [--path]`
  - A `lock` tab in the desktop app, a full-window unlock screen, and a `lock`
    button in the header that clears the session passphrase with it.
  - It is **not tamper-proof** and never says it is. Anyone who can write to
    your files can delete it. It stops casual access; if the disk is the
    threat, encrypt the volume.
- **`docs/USER_GUIDE.md`**, and a desktop-app section in the wiki.
- **A walkthrough on the website**, below the download, explaining what to do
  with the thing you just downloaded -- with scroll-reveal animations that
  degrade to plain visible content without JavaScript.
- **`.claude/launch.json`** and a documented one-liner for serving the site
  locally while working on it.

### Security fixes

Found by finishing the audit scope that `docs/AUDIT.md` had listed as
outstanding. Seven defects, four reachable from a file somebody sends you. None
was a confidentiality failure.

- **Argon2 parallelism overflow (F-2).** `argon2` 0.5.3 evaluates
  `m_cost < p_cost * 8` before checking `p_cost`'s ceiling, so a `p_cost` above
  `u32::MAX / 8` overflowed. That value is read verbatim from a `.veil` header
  and from the app-lock file, which is parsed before anyone has authenticated.
- **Unbounded Argon2 memory cost (F-3).** `m_cost` was also read verbatim and
  allocated up front, so a header claiming `u32::MAX` asked for four terabytes;
  the allocation fails and a failed allocation aborts the process. Merely
  *trying to open* a hostile container killed the program, release builds
  included. Now capped at 4 GiB, above RFC 9106's largest recommended profile.
- **32-bit overflow in the RIFF chunk walker (F-4).** Unreachable on 64-bit,
  live on the ARMv7 build. Found by reading, not by fuzzing.
- **One NaN sample destroyed the engine (F-5).** The accent neutraliser's
  long-term spectrum is an exponential moving average, so a single non-finite
  input sample poisoned it permanently: every subsequent output sample was NaN
  for the rest of the session, silently. A 32-bit-float WAV can legally contain
  one. Input is now sanitised at the single gate every sample passes through.
- **The site's Markdown renderer emitted unfiltered image URLs (F-6),** silently
  deleted every string literal in every code block (F-7), and rejected ordinary
  relative links (F-8).

### Website fixes

- **Empty links.** A link whose label was inline code rendered as an empty
  anchor, so "see [`docs/AUDIT.md`](docs/AUDIT.md)." was published as "see .".
- **Invisible paragraphs.** Three parts of the walkthrough never appeared,
  including the box stating the app lock is not tamper-proof. A viewport jump
  carried them past the observer without the intersection ratio ever changing.
- **Stray characters, repo-wide.** Control characters, private-use characters,
  zero-width characters, replacement characters, bidi overrides, byte-order
  marks and CP1252 mojibake are now checked mechanically across every tracked
  text file.
- **Files served raw are ASCII-only** -- `website/js/*.js` and the licence and
  waiver texts. GitHub Pages sends `charset=utf-8`, but an editor or terminal
  that guesses the encoding turned a prose em dash into mojibake in the middle
  of the sentence making the promise.
- **The repository panel animates while it loads**: a spinner on the button, a
  pulse on the figures, counted-up numbers and a staggered arrival. It remains
  **opt-in** -- it is the one third-party request on the site, and whether to
  make it stays the reader's decision.

### Testing

- **Parser campaigns** against the container header, the app-lock file and the
  RIFF chunk walker: deterministic seeded mutation, a million rounds per target
  in CI, run in debug because the release profile disables the overflow checks
  that caught two of the bugs above.
- **Timing measurement** of both password paths. No prefix correlation: 0.996
  for wrong-at-the-first-byte against wrong-at-the-last, 1.004 for the app lock.
- **Hostile-audio tests** covering NaN, infinities, full-scale DC, square waves,
  impulse trains and digital silence.
- **Website test suite** (`tools/site-tests/`, no dependencies): stray
  characters, page structure, rendering correctness, 39 hostile documents plus
  200,000 generated ones, and the scroll-reveal fallbacks.
- 186 tests at v0.1.5, **247 now**, plus the site suites. Clippy clean, no
  `unsafe`.

### Documentation

- `docs/AUDIT.md` rewritten: every finding written up individually, the
  completed scope, measured timing figures, and a verdict that says plainly the
  previous revision called this code clean while it contained at least four of
  these.
- `docs/WHITEPAPER.md`: a new section on the app lock and exactly what it is
  worth, at-rest encryption by default, and the honest note that better tools
  found what careful reading had not.

## v0.1.5 and earlier

See the release notes for each tag.
