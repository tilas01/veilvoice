<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# VeilVoice — internal audit

**Auditor:** tilas01 (maintainer). **Date:** 2026-08-19. **Version:** 0.1.10
(unreleased), covering the whole tree.

## The standard this is held to

Earlier revisions of this document named "an independent audit" as the one
outstanding item, and treated everything else as done. That framing is dropped.
It let a very large amount of unexamined code sit behind a single caveat, and it
made the honest answer to "has this been checked?" depend on someone else doing
the work.

The standard now is stated positively, and it is a standard about **this code**
rather than about who looked at it:

> Every line is written to current Rust security practice, and the whole tree
> has been audited against the full range of Rust-specific vulnerability
> classes: integer overflow and truncating casts under **both** profiles;
> panics reachable from untrusted input; allocation sized by untrusted input;
> every parser that reads bytes somebody else produced; resource exhaustion and
> loops whose termination depends on a length field; TOCTOU, symlinks and file
> creation modes; secret zeroization, page locking, constant-time comparison and
> `Debug` leakage; cryptographic misuse; concurrency; dependency risk; and error
> handling that degrades quietly to a weaker posture. The website is held to the
> same standard as a security boundary in its own right.

Each of those classes has a section below saying what was examined and what was
found, including the ones that found nothing — "we looked and there was
nothing" is a result, and it is not the same as not looking.

**What has not changed:** no external firm or independent researcher has
reviewed this code, and the documentation still says so wherever it matters.
That is a fact about the world, not an outstanding task item, and it is
recorded as such rather than as a promise to be redeemed later. An outside
reviewer would still be worth having. The difference is that their absence is no
longer offered as the explanation for anything.

## The seventeenth round: what nothing was checking about the screenshots

Prompted by a sweep rather than by reading: taking the em dashes out of the
interface text meant editing strings that ten committed screenshots show.

**One defect found and fixed (F-103)**, and it is the one that had been sitting
underneath a "still open" entry for several rounds without being seen as a
defect in its own right.

### F-103 -- nothing said when a committed screenshot had gone stale

`tools/shots/terminal.py`.

`--check` compared each drawing against the text file beside it. It compared
that text file against nothing at all. The file is written by `--capture`,
which is a separate command that `tools/verify.py` does not run and nobody runs
by accident, so a string could be rewritten in `veilvoice-cli` and **every
check in this repository would pass** while `assets/screenshots/` went on
showing the old wording, on the website, in the README and in the gallery.

Not hypothetical, and found the only way this kind of thing is found. The
interface text was rewritten, the whole of `verify.py` passed, and
`cli-help.txt` still contained an em dash the program no longer prints.

The audit had recorded the re-capture as a manual step. That was accurate and
it was half the picture: the missing half is that nothing said when the manual
step was **due**. A manual step nobody is told to take is not a step in the
process, it is a hope.

`--check` now also runs the commands and compares what they print against what
is committed, and every capture here is a `--help` screen, so its output is a
function of the binary rather than of the machine and two people on the same
commit get the same bytes. That is what makes it checkable, and it is why the
list stays help screens: a capture of what is installed or what is plugged in
could not be checked this way. It skips where there is no build, which is the
CI job that runs the other checks in this file, and it does not skip in
`verify.py`, which runs after `cargo build` on the machine where the strings
were just edited.

Proved rather than assumed: a capture was edited by hand, the drawing
regenerated from it so the existing check would pass, and the new one reported
`cli-clean.txt is not what veilvoice clean --help prints` and failed the run.

## The sixteenth round: the manifest generator, an hour after writing it

The same rule as the fourteenth round, applied to the other half of marker 97.
The generator that writes `CONTENTS.sha256` is the writing end of the seam the
verifier reads, and it had just been moved out of the release workflow into a
script so that it could be tested at all.

**One defect found and fixed (F-102)**, by measuring what a line of path
handling actually does rather than what it reads as.

### F-102 -- `lstrip("./")` strips characters, not a prefix

`tools/release/contents.py`.

Tar members sometimes carry a `./` prefix, so the generator normalised them
with `name.replace("\\", "/").lstrip("./")`. `str.lstrip` takes a **set of
characters**, not a prefix, and removes every leading character that is in it.

Measured, not reasoned about:

```text
'./veilvoice/x'  ->  'veilvoice/x'      as intended
'.hidden/file'   ->  'hidden/file'      a file renamed
'../escape'      ->  'escape'           a path silently made acceptable
'./.config/y'    ->  'config/y'         both at once
```

The first of those is a correctness failure with an ugly shape: a release
containing a dotfile would publish it under a name no file on disk has, so
every verifier would report it **missing** on a release that is perfectly
sound, and the owner would be told their download had been tampered with.

The second is worse and is the one worth the finding. A member that climbs out
of the release directory was quietly rewritten into one that looks ordinary.
The reader's own note says, in as many words, that such a path must be refused
rather than sanitised, because a manifest containing one is not a manifest with
a bad line in it: it is a file that did not come from this project's release
job. The writer was doing the opposite, and the two ends of the seam had been
written a few hours apart by the same hand.

Both ends state the same rule now, and the writer refuses -- naming the archive
and the path, and failing the release job -- rather than publishing something
every verifier would reject. The round-trip test builds a release with a
dotfile in it, so the renaming half cannot come back.

Beside it, a wrong number: the generator reported the file count by subtracting
twice the number of archives from the line count, which is off by one per
archive. It said five files for six. Counted rather than derived now. Not a
finding, and worth writing down anyway in a document that keeps telling itself
numbers must be measured.

## The fifteenth round: the page that says where to get it

The releases page, read after the verifier rather than before it, because a
download nobody can reach is not made safer by a checker.

**One defect found and fixed (F-101)**, and it is the oldest shape in this
document: two hand-kept lists compared against each other and against nothing
else.

### F-101 -- the download page linked two files that had never existed

`tools/site/releases.py`.

The page listed five archives per release. The release workflow builds
**eleven**. Two of the five names were wrong: the page said `macos-aarch64` and
`linux-aarch64` where every release has ever published `macos-arm64` and
`linux-arm64`. So every release entry on the page carried two links that answer
with a not-found, and six published platforms -- the Raspberry Pi build, the
two static musl builds, FreeBSD, OpenBSD and NetBSD -- had no link at all.

Measured against a published release rather than argued: the names the page
constructs for v0.1.14 were compared against the assets GitHub actually holds
for that tag. Two linked and absent, six present and unlinked.

This is F-71's shape, which this document has already described once: two
hand-typed things kept beside each other by attention, each only ever compared
against the other. Correcting the five names would have restored the same
arrangement with the same future.

`release.yml` is the file that decides what is built and what each archive is
called, so the page reads it. A platform added to that matrix now appears on the
page; a label renamed there cannot leave a dead link here. The generator refuses
to write a page at all if it can read fewer than five archives out of the
workflow, because a page with no downloads on it looks like a release that
built nothing rather than like a broken generator, and `packaging.test.js`
checks in CI that every label the workflow builds is linked.

Two smaller things went with it. The page's backlog began at v0.1.6, because
`CHANGELOG.md` keeps one combined section for v0.1.5 and earlier and the parser
matched `## vX.Y.Z` only: six published releases with no entry, no summary and
no files. They are listed now, each pointing at its own release page for notes,
which is where those notes genuinely are. And the files moved to the end of each
release's notes rather than the top, so opening a release shows what changed in
it rather than a wall of links.

## The fourteenth round: reading the verifier written an hour earlier

Marker 97 rewrote what `veilvoice-verify` and the desktop verify tab do: a
signed list of every file inside every archive, checked file by file against
the extracted folder, and the reader's own GnuPG run over the same signature.
New code in the one program whose entire job is not to be fooled is exactly the
code an audit exists for, so it got a round of its own before anything else was
built on it.

**Three defects found and fixed (F-98, F-99 and F-100), all by reading the code
written an hour earlier**, and all three are the same mistake wearing different
clothes: a check that could not see, answering as though it had seen.

### F-98 -- a folder that could not be read was reported as holding nothing

`veilvoice-check/src/contents.rs`.

`extras` sweeps the extracted directory for files the release never published,
because a folder holding every correct file plus one extra program passes every
other check and is not the release. Its walk opened each directory and, on any
error, returned. The caller got an empty list, and an empty list is drawn as
**"there is nothing else in the folder"**.

Measured rather than argued: a directory tree deep enough that its absolute
path passes `PATH_MAX` stops `read_dir` at about 1988 levels on Linux, and a
file below that point was reported as absent rather than as unreachable. A
permission bit does the same thing in one line and is how it would actually
happen: a folder extracted by another account, or one whose mode came out of
the archive wrong.

The sweep now returns what it could not read alongside what it found, and both
front ends withhold the pass and name the folder. Unknown is not empty.

The walk also stopped recursing while this was fixed. The measurement above
says the recursion could not in fact have overflowed a stack, because
`PATH_MAX` bounds the depth long before the stack does -- but that bound comes
from the operating system rather than from this code, and a bound nobody here
chose is not one this code can rely on.

### F-99 -- a link where a file should be, hashing correctly, passed

`veilvoice-check/src/contents.rs`.

`check` asked whether the path exists and then hashed it. Both follow a
symbolic link, so a link standing where a program should be, pointing at a copy
of the genuine bytes, was reported as **matching the signed list**.

Two things are wrong with that. The release published a file, not a link. And a
link is a name somebody else may be able to repoint after this has looked,
which is precisely the substitution a hash check cannot notice.

It is caught by reading rather than by luck that the *other half of the same
module* already knew this: the sweep for extra files deliberately does not walk
through links. One function treated a link as a thing to refuse and the
function beside it treated the same link as the file it pointed at.

`check` asks `symlink_metadata` now, and a link, a directory or anything else
that is not an ordinary file is its own verdict and never a pass.

### F-100 -- a genuine release signed twice would have been refused

`veilvoice-gnupg/src/lib.rs`.

GnuPG reports every signature on a file: a run over a doubly signed file prints
several `VALIDSIG` lines. The reader took the first one and compared it against
the expected fingerprint, so a release signed by the project key **and** by
somebody else's, in that other order, was reported as signed by the wrong key
and refused.

That is the safe direction and it is still a defect. A verifier that refuses
genuine releases teaches people that verification is unreliable, and a check
people learn to work around protects nobody. The question is whether the
expected key signed this data, and it is now asked of every signature rather
than of whichever one GnuPG happened to print first.

**What the three have in common** is worth naming, because it is the fourth
time this project has recorded it. Each was a place where the code could not
answer and answered anyway: an unreadable directory read as empty, a link read
as the file it points at, one signature read as all of them. The rule the
failsafe already states in as many words -- "could not tell, so refuse" -- is
the rule all three were missing.

## The thirteenth round: the three ways the release would not compile

The twelfth round read the code. This one read the build machines, because
v0.1.15 was cut, merged, and then refused by continuous integration on four
jobs at once. Three of those were real and none of them was in the shipped
program: two were tests asserting something that is not true of every machine,
one was a constant that only one platform reads, and the fourth was a generated
file committed a few minutes before the file it is generated from changed.

**Two defects found and fixed (F-96 and F-97)**, with two test defects and one
process failure written down beside them, because a test that fails on somebody
else's machine costs exactly as much as a bug until it is understood. F-97 was
found by the fix for the fourth failure rather than by reading, which is the
round's own small lesson: the first explanation for a stale generated file was
true and was not the whole truth.

### F-96 -- a just-started program is not yet wearing its own name

`veilvoice-failsafe/src/act.rs`.

`still_named` answers "does this process id still belong to this program", and
the failsafe asks it immediately before closing anything. Its tests start a
`sleep` of their own and act on it in the next few microseconds, and one CI run
reported `sleep is no longer process 7030` about a `sleep` that was still
running when the job cleaned up after it.

Measured rather than guessed: four thousand spawns of `/bin/sleep`, each one
followed on the next line by a read of `/proc/<pid>/comm`. One of them came back
with the name of the *parent*. The kernel releases a `vfork`ed parent when the
child's address space goes, which happens inside `begin_new_exec` and before the
line that gives the new program its name, so there is a window in which the id
is live, belongs to the program that is starting, and answers with somebody
else's name.

The shipped code is right as it stands: it refuses during that window, and
refusing is the direction that closes nothing on a doubtful answer. A scan never
sees the window either, because a program holding a microphone has been running
for rather longer than a microsecond. The tests are the only callers that can
reach it, and they now wait for the child to appear under its own name before
they assert anything about it, which is the precondition they always assumed.

### Two tests that were measuring the machine

**The real-time headroom test.** `stays_comfortably_realtime_with_accent_on`
asserted an absolute real-time factor below 0.5. On the armv7 job, where the
test binaries run under emulation, it measured 0.557 and failed, while the same
commit passed on every native target. There is no single number that is generous
enough under an emulator and tight enough to catch anything on a real machine,
so the number was the wrong shape of assertion.

The claim worth defending never needed one. Accent tracking is an addition to
the spectral work rather than a multiple of it, and the regression the test
exists to catch, a pitch search that stopped being decimated, is an order of
magnitude. So the same audio is now run twice on the same machine in the same
test, once with the neutraliser bypassed, and the two are compared. Measured
under armv7 emulation: 0.352 seconds bypassed against 0.469 with accent
tracking, a ratio of 1.3 against a bound of 4.

**The dead constant.** `veilvoice-gui`'s `USAGE` is read only by the `--help`
path, which is Unix only and says why: a Windows release build declares the
windowed subsystem and has no console to print to. A constant nothing reads is
dead code, `-D warnings` is on, and the Windows job was the only one that could
see it. It is declared where it is read now. The test that checks the help text
against the tab names reads the file rather than the constant, so it still runs
on every platform.

### F-97 -- a committed drawing that depended on which Python was installed

`tools/docs/generate.py`.

The fourth job was written off as the process failure below, and the process
failure was real. Regenerating and committing did not fix it. The same job
failed again on six files nobody had touched, and this time the explanation was
not staleness.

`tools/docs/generate.py` lays out a call graph and writes the coordinates into
an SVG. It summed box widths with the built-in `sum`, and **CPython 3.12 gave
`sum` compensated summation over floats**. The same widths therefore add up to a
value a fraction different from the one 3.11 produces, everything downstream is
a centring calculation, and one box landed at `x=40.1` under one interpreter and
`x=40.2` under the other. Six generated files, three drawings and their three
pages, differed by a tenth of a pixel that no eye could see and that a byte
comparison could not miss.

Measured: the committed files match under 3.11 and differ under 3.12, on this
machine, with nothing else changed. That is the whole defect. This repository
commits its generated output and compares it byte for byte precisely so that
"generated from the source" is checkable rather than asserted, and a check that
passes or fails on which interpreter a contributor happens to have is not a
check.

The sum is `math.fsum` now, which is exactly rounded and therefore identical on
every version and platform. Verified across 3.10, 3.11, 3.12 and 3.13: the same
1108 files, byte for byte.

**And the check that would have caught it is now there.** The assets job runs
the generators under two Python versions rather than one. A single-interpreter
check cannot see this class of defect at all, which is why it took a red build
on somebody else's machine to find the first one.

### The process failure, which is the one worth remembering

The fourth job failed because `website/search-index.json` did not match its
generator. The difference was seven bytes in one file: `ROADMAP.md` was edited
*after* `tools/verify.py` had regenerated everything from it, and the commit
carried the older index. Every generated artefact in this repository has a
`--check` wired into CI precisely so that this is caught rather than shipped,
and it was caught. The rule it enforces is that `verify.py` is the last thing
run before a commit and not merely a thing run before a commit.

## The twelfth round: v0.1.15, and one defect that had already been fixed once

Everything built after the eleventh round, read before the release: the
encrypted-volume work, the app lock as a key, the autolock, the releases page,
the GnuPG commands and the ffmpeg pair.

**One defect found and fixed (F-95), and it is the same defect as F-93 in the
place that actually matters.**

### F-95 -- a vault locked after it was chosen still received the file

`veilvoice-gui/src/storage.rs` and `app.rs`.

F-93, found and fixed in this same cycle, was that the storage panel asked
whether the destination folder *existed* when the question is whether anything
is *mounted* on it: unmounting a volume leaves its mount point behind as an
ordinary empty directory. That was fixed, tested, and written up.

It was fixed in the panel. The panel draws a warning. The file is written
somewhere else, by `Destination::place`, and `place` never asked either
question: it checked `ready`, which is about whether the hidden-volume question
has been answered, and nothing else.

So the sequence that fails is the ordinary one. Somebody opens their VeraCrypt
volume, chooses it, answers the hidden-volume question. Later they lock the
volume. Nothing about the destination changes, because nothing about it has
changed: the answer is still given, so `ready` is still true. The next veiled
recording is written into the bare mount point, on the ordinary unencrypted
disk, while its owner believes it went into the vault.

That is precisely the failure the whole feature exists to prevent, and it
survived the fix aimed at it.

`place` now takes the mount list and consults it, and `start_job` reads that
list at the moment of writing rather than using what the panel last saw. A job
whose destination is no longer mounted is refused with a message naming the
remedy, rather than quietly falling back to writing beside the source, which
would leave the recording unencrypted somewhere else instead.

**The lesson is the one already recorded twice, now three times.** F-91 was
written up as being about the app-lock file when it was about any file the
program opens without being asked, and F-92 was the two other places. F-93 was
written up as being about `still_there` when it was about *every* place that
decides whether a vault is usable, and F-95 is the one that was missed. Fixing
the instance in front of you and moving on is this project's most reliable
source of second defects.

What would have caught it earlier: asking, for each fix, "where else is this
decision made", and in particular "where is the value actually used", rather
than "where was the symptom seen".

## The eleventh round: the code written after the tenth round

The tenth round was run last on purpose, on the grounds that an audit of code
that is still moving is an audit of code that will not exist. Then markers 74
to 79 were built, and three of them are security code: an authentication tag on
the app lock, a second copy of it in a vault, and the integrity record moved
into the window. New security code written after the audit is precisely the
code an audit exists for, so it got its own round.

**Eight defects found and fixed (F-85 to F-92), every one of them in code
written in this cycle or reachable only because of it. Six were found by
reading the diff; two came out of re-running the coverage-guided campaign,
which the changed lock format made worth doing again.**

Two of the six were worse than the thing they were added to improve, which is
the pattern worth naming: *hardening that fails open*. A lock that could be
orphaned by one refused read, and a spare copy that silently kept the previous
password, are both worse than the plain single file they replaced.

### The campaign, run again over all six targets

Ten minutes each rather than the tenth round's five, and all six rather than
only the one whose format changed. **703,074,471 inputs. No crash, no hang, no
out-of-memory on any target.**

| Target | Inputs in 10 minutes | Artefacts |
|---|---|---|
| `container_header` | 67,305 | one slow unit, which became F-92 |
| `lock_file` | 1,280 | two slow units, both the class F-91 accepts |
| `wav_chunks` | 75,412,092 | none |
| `wav_preflight` | 291,305,857 | none |
| `guard_manifest` | 11,926,580 | none |
| `hybrid_keys` | 324,361,357 | none |

The two crypto targets run four and five orders of magnitude fewer inputs than
the rest, and that is the campaign working rather than failing. Both parse a
header carrying Argon2id cost parameters, so an input that gets past the header
checks costs a real key derivation: between one and nineteen seconds, measured
under F-91. Everything else parses bytes and returns.

`lock_file`'s 1,280 is the extreme, against 445,714 in the tenth round for the
same target, and the reason is measurable rather than arguable. **Thirty-six of
the 53 corpus entries the campaign has kept now parse successfully**, so most of
what it runs reaches the derivation instead of being turned away. Fewer runs,
deeper ones. The tenth round recorded the same relationship in reverse, when
F-82 bounded an unbounded time cost and made this target 136 times more
productive.

**Three slow units, and the difference between filing them and reading them is
this round's most useful lesson.** libFuzzer flags any input taking over a
second, and a deliberately slow key derivation taking over a second is the
whole point of a deliberately slow key derivation, so the easy reading is that
all three are noise. Two of them are: 518 MiB at sixteen passes, inside every
bound, about five seconds. The third declared 640 MiB on the *container*
target, and asking which callers reach that path without a person choosing the
file turned a non-finding into F-92 and two real fixes.

What is still open, and it is the same sentence as before with better numbers
behind it: ten minutes is not convergence either, no corpus is committed so
every cold run rediscovers the structure, and nobody has run any of this on
Windows or macOS.

### F-85 -- one refused read would have orphaned the lock for ever

`veilvoice-crypto/src/vault.rs`.

The vault derives both file names from sixteen random bytes in an index file.
`Vault::at` read that index and, on anything that was not a clean read of
exactly sixteen bytes, drew a new value and wrote it. The comment beside it
argued the case honestly enough: a damaged index has nothing to recover from,
so there is nothing to lose by replacing it.

The argument is right and the code was still wrong, because the arm caught
every *other* failure too. A read refused by permissions, a Windows sharing
violation, an exhausted file-descriptor table: any one of them, once, and a new
index would be written over a perfectly good one. The lock files would still be
on disk, under names that nothing could ever compute again, and the user's app
lock would simply be gone.

The fix is one match arm. Only `ErrorKind::NotFound` creates an index; every
other outcome refuses and reports. A refusal costs a confusing session and
loses nothing.

This is the class the whole round is about: a piece of hardening whose failure
mode is worse than the thing it hardened. The plain file it replaced could not
be orphaned, because its name was a constant.

### F-86 -- the spare copy kept the previous password

`veilvoice-crypto/src/vault.rs` and `lock.rs`.

The second copy is written under `/etc/veilvoice` when VeilVoice is already
running with enough privilege to put it there, which is the point: removing the
lock then needs `sudo`. An ordinary run cannot write it, and `store` swallowed
that failure deliberately, on the reasoning that the arrangement was working as
designed.

It was, and the consequence was not designed. After `veilvoice lock change`
from an ordinary run, the first copy holds the new password and the spare still
holds the old one. Delete the first copy -- which anybody can, it is in the
user's own directory -- and `load` restores from the spare. The lock reverts to
the previous password. Somebody who knew the old password has a way back in,
created by the feature that was supposed to make removal harder.

Three fixes, because one was not enough:

- `Vault::store` returns whether the spare was written instead of discarding it.
- `LockStore::change_password` returns `Error::AppLockSpareStale` when it was
  not, and both front ends print the one thing that finishes the change.
- `Vault::load` compares the two copies' salt and verifier. When they differ it
  returns `Found::Disagreed`, prefers the copy the running program writes, and
  the disagreement is raised as a tamper report.

The third is what closes it rather than merely reporting it: reverting now
requires the copies to agree, and they do not.

### F-87 -- a spare that could never be written was reported as a deleted one

`veilvoice-crypto/src/vault.rs`.

`load` reported `Found::Restored` whenever one copy was missing, and the caller
turns that into a standing tamper report that only the passphrase clears. On a
machine where `/etc/veilvoice` exists but is not writable by the user -- created
by a package, or by one earlier elevated run -- the spare is never written, so
every launch found it missing, rebuilt nothing, and raised the alarm again.

An alarm that fires every time is an alarm nobody reads, which is the failure
mode this project keeps writing tests against. `Restored` is now reported only
when the rebuild actually reached the disk. A copy that cannot be written was
never there; a copy that was there and went is evidence.

### F-88 -- dismissing a warning cost three key derivations

`veilvoice-crypto/src/lock.rs`.

`LockStore::acknowledge` called `unlock`, which runs Argon2id at 256 MiB, and
then `AppLock::acknowledge`, which calls `verify`, which runs it again -- and
the store's own `unlock` inside that path made three. The better part of a
minute on a slow machine to dismiss a message.

Functionality rather than cryptography, and it belongs in a security audit
anyway: a control nobody will wait for is a control nobody uses, and the
control here is the one that tells somebody their lock was interfered with.
`unlock` has already proved the passphrase by the time the flag is cleared, so
the second and third proofs are removed.

### F-89 -- a power cut read as tampering

`veilvoice-crypto/src/vault.rs` and `privatefile.rs`.

Both copies were written by truncating and rewriting. A process that dies
part-way through leaves a short file, a short file does not parse, and a copy
that does not parse is treated as one somebody interfered with. A power cut
during a save would have raised a tamper report, and the report cannot be
cleared without the passphrase.

`privatefile::replace_owner_only` writes to a temporary file beside the
destination and renames over it, which the operating system does as one step.
The temporary is created owner-only and the rename carries that permission, so
there is no moment at which the contents are readable by anybody else.

### F-92 -- the same defect as F-91, in the two places it was not looked for

`veilvoice-guard/src/manifest.rs` and `veilvoice-policy/src/policy.rs`, found
by decoding a slow unit the `container_header` target produced.

F-91 gave the app-lock file an unattended memory ceiling, on the argument that
nobody chooses to open a file the program reads by itself at launch. The
argument is general and it was applied to exactly one file.

Two others have the same shape. `Manifest::open_sealed` reads the integrity
record, and this cycle's own marker 75 made that automatic: it used to run only
when somebody typed `veilvoice guard check`, and now the desktop application
reads it at every unlock. `Policy::open_sealed` reads the sealed policy at a
fixed path beside the plain one. Both used the generous four-gigabyte ceiling
meant for a `.veil` somebody was sent and decided to open.

So anybody who can write the configuration directory can leave a sealed
manifest declaring four gigabytes of Argon2 memory, and every unlock from then
on allocates it. On a modest machine that is an allocation failure, and this
workspace aborts on one, so the window dies immediately after a correct
passphrase is entered. Both now pass `UNATTENDED_MAX_M_COST`, which is one
gigabyte and four times what either ever writes.

The general lesson is the one this project keeps relearning and has now
recorded three times: a fix applied to the instance that was found, rather than
to the class, leaves an exclusion list that names the files somebody happened
to think of. F-91 was written up as being about the app-lock file. It was
about *any* file the program opens without being asked, and the sentence that
would have found these two was already in `container.rs`, describing when to
use `open_with_password_within`.

**What made the difference was decoding the artefact rather than filing it.**
The campaign reported a slow unit on `container_header`, not a crash, and the
easy reading is that a slow Argon2 is Argon2 working. Reading the actual cost
out of it, and then asking which callers reach that path without a person
choosing the file, is what turned a non-finding into two.

### F-91 -- a lock file could ask for more memory than the machine has

`veilvoice-crypto/src/lock.rs`, found by the coverage-guided campaign after the
format changed.

The parser validated the Argon2id costs with `KdfParams::checked`, which permits
up to four gigabytes of memory. That ceiling is deliberate and right for a
container: somebody chose to open that file, it is slow, and they can decide to
stop waiting. `KdfParams::within` exists for the other case and its own
documentation names it exactly: "a caller running without a human present,
anything processing files it did not choose".

Nobody chooses to open an app-lock file. It is read at launch, before anything
has been authenticated, and on a modest machine four gigabytes is not a wait,
it is an allocation failure, and this build aborts on one. The window would
fail to start with no way in.

The campaign produced a header declaring 1,664 MiB, with sixteen passes and
thirty-seven lanes, and libFuzzer flagged the unit as slow. Nothing crashed,
which is why this had gone unnoticed through two previous campaigns: the
finding is a permitted value, not a bug in handling one.

Two things make it worth fixing now rather than accepting as before. The lock
file is the *only* file this program parses before anybody has authenticated,
so it is the only place where the attended argument does not apply at all. And
the recovery is harder than it was last round, by this cycle's own doing: the
vault derives its file names rather than using a fixed one, so "delete the lock
file and start again" now needs the index read first. Hardening raised the cost
of recovering from a hostile file, which is a reason to make the hostile file
refusable rather than a reason to leave it.

The parser now uses `within(UNATTENDED_MAX_M_COST)`, which is one gigabyte:
four times what this program has ever written into one of these files.

Measured on this machine, in a release build, rather than reasoned about:

| What | Cost | Time |
|---|---|---|
| The default this program writes | 256 MiB, t=3, p=4 | **1.07 s** |
| The worst a lock file may now declare | 1024 MiB, t=16, p=4 | **19.30 s** |
| The unit libFuzzer flagged after the fix | 262 MiB, t=16, p=37 | **4.78 s** |

So the fix moves the worst case from an allocation this build aborts on to a
wait, which is what `within` documents itself as buying and is the honest
description of it. Nineteen seconds to be told the password is wrong is a
hostile file doing real damage to somebody's day; it is not a lock-out, and the
owner can still get in and change it.

The campaign flags the second slow unit as slow too, and that one is accepted:
262 MiB at the maximum sixteen passes is under five seconds and Argon2 taking
seconds is the entire point of Argon2. A ceiling tight enough to catch it would
be tight enough to catch a legitimate lock somebody deliberately made expensive.
The number of passes is already bounded at sixteen by F-82.

### F-90 -- setting an app lock never upgraded the integrity record

`veilvoice-gui/src/integrity.rs`.

The record of VeilVoice's own files is sealed under the app-lock passphrase
when there is one and written in the clear when there is not. Somebody whose
first launch had no lock got the plain record, and setting a lock afterwards
never replaced it: the sealed path only ran when a sealed file already existed.
They had done exactly the thing that earns the sealed record and kept the
readable one, with the interface telling them which they had and nothing
telling them how to change it.

The upgrade now happens on the first unlock that finds a plain record, and only
after the check against it has come back clean. Sealing a record that no longer
matches the files would seal somebody else's version of them and stamp it
authoritative, which is the same failure as F-85 in a different place: the
recovery path doing the attacker's work.

## The tenth round: security, functionality, and what it costs to run

The round asked for before the next deploy, covering all three and run last on
purpose: an audit of code that is still moving is an audit of code that will
not exist.

**One defect found and fixed (F-84), and one measured inefficiency removed.**

### The mechanical checks, all of them

Every claim this project makes that a machine can test, tested:

| Claim | How it was checked | Result |
|---|---|---|
| No `unsafe`, in any crate | every `lib.rs` and `main.rs` for `#![forbid(unsafe_code)]`, then a sweep for the keyword | 26 of 26, none found |
| It talks to no servers | the whole dependency graph for an HTTP client | none |
| `veilvoice-priv` only reports | the guard test that names every subprocess the crate starts | two probes, both read-only |
| The parsers survive hostile input | the coverage-guided campaign, six targets, five minutes each | **293 million inputs, nothing found** |
| Every generated file matches its generator | `tools/verify.py` | all fourteen checks pass |
| Every picture keeps its words inside | `images.test.js` | 4,958 pieces of text in 492 drawings |
| The tests hold where a pointer is 32 bits | `i686` and `armv7` | 682 tests each, no failures |

The campaign is the one worth dwelling on, because the number changed for a
reason. Last round `lock_file` managed 3,274 inputs in five minutes; this round
it managed **445,714**, which is 136 times more. That is F-82's fix: with no
ceiling on the number of Argon2 passes, most of that target's five minutes went
into a handful of absurd derivations. Fixing a denial of service made the
campaign that found it two orders of magnitude more productive, which is worth
knowing the next time a bound looks like a formality.

### F-84 -- the preview said "nowhere else" before it knew where

`veilvoice-cli/src/main.rs` and `veilvoice-gui/src/app.rs`, both in code
written this cycle.

`--preview` exists so somebody can hear their own veiled voice before an
interview rather than during one, and it printed:

> Preview. The veiled voice goes to this machine's output and nowhere else.

It printed that **before naming the device**, and it is not always true.
`--preview --output <a cable>` keeps the cable, because an explicit choice is
honoured. And a machine whose *default* output is a virtual cable does the same
thing without being asked, which is not a strange configuration: somebody who
routes their audio through one may well have set it as the default. In either
case whatever is listening on that cable hears the preview.

**A false reassurance in the one place somebody is checking their setup is
worse than none**, because checking is exactly what they came there to do. The
claim now comes after the device, names it rather than the machine, and when
the device is a cable it says so outright and says what to do instead. The
desktop application made the same claim in a notice and now makes the same
check.

Not a confidentiality failure, and it is the same shape as one: a sentence that
tells somebody a thing is private when the code has not established that.

### The optimisation pass: 43.6 per cent of the search index was drawings

Measured rather than guessed. `website/search-index.json` was 4,779,645 bytes,
of which 3,903,419 was excerpt text, of which **1,700,062 was generated SVG
markup and copies of assets**. Every byte of it is downloaded by every reader
who uses the search.

It bought them nothing. All 536 SVGs in this repository are produced by a
generator and carry a marker saying so; not one is hand-written prose. The
words inside a drawing are the words of the document it was drawn from, which
is indexed at that document, and a search result pointing at an SVG file is a
result nobody can use.

The argument is not new. It is written at the top of
`tools/search-index/generate.py` about the crate documentation, in those words,
and it was applied to the banners and not to the diagrams. That is how 43.6 per
cent accumulated without anybody deciding on it: an exclusion list naming the
files somebody thought of.

The index is now **2,532,102 bytes, 47 per cent smaller**, and 749 KB rather
than 918 KB over the wire. The rule is a property of the file rather than a
list of paths: a `.svg` is a drawing, and drawings are not indexed. Search
still returns 63 results for "voiceprint" and the first is a document.

### What this round did not do

No outside review, which is the entry at the top of "still open" and remains
the largest gap. No fuzzing on Windows or macOS. No profiling of the audio
engine, which needs a machine with a microphone and is where an optimisation
pass would find real numbers; the pass here measured what a reader downloads,
because that is what this machine can measure honestly.

## This round

**Ten defects found and fixed (F-74 to F-83.)** Three are in the security
crates written since the eighth round and none of those has shipped: `main`
carries them and they are not in v0.1.14. One is on the published front page
and has been there for as long as the count has. One is in the test suite
itself, where it had been passing four runs in five. Three shipped in the
installer and the package definitions. **And two are in the cryptography and
the tamper record, both reachable from a file somebody sends you, both shipped,
and both found by running the coverage-guided campaign that this document had
listed as built and never run.**

This round covers what was added after it: Failsafe, the application baseline,
the privilege report, the hardware detection, the decoy passphrase, and the
desktop application's file dialogs and notifications.

**All four are the same failure in different clothes: trusting an answer that
was never given.** F-74 trusted a process id to still mean what it meant a
moment ago, and then trusted an exit code that is returned unconditionally.
F-75 trusted a file's default permissions to be appropriate for a security
setting. F-76 trusted a name in the process table to mean the program is
running. F-77 trusted one machine's measurement to be a fact about the tree.
F-78 trusted a test that passes to mean a test that holds. F-79 trusted another
program's error message to be readable in the middle of this one's output.
F-80 trusted a recipe nobody had run. F-81 trusted six files to keep up with a
number that had moved five times. F-82 trusted a number in a file to be a
number of passes somebody would wait for. F-83 trusted a path in a record to be
a path rather than a way of rewriting the report that prints it. None would
have been found by reading the code. F-74 was found by writing a test that killed a real process and watching
what it actually did; F-76, F-77 and F-78 by running that same suite on a
second operating system, where one test failed, one number came out different,
and one test failed only sometimes; F-79 by running the installer on that
machine; and F-80 and F-81 by building a package, which are three things this
document had listed as never done.

### F-83 -- the tamper record refused to write what it was happy to read

`veilvoice-guard/src/manifest.rs`. `Manifest::of` refused to record a path
containing a line break. `Manifest::parse` accepted one. So VeilVoice would not
write a record it was perfectly willing to read from somebody else, and
`veilvoice guard check` reads whichever file is at the path it is given. A
record is exactly the kind of thing that gets handed to you.

Found by the coverage-guided campaign, which produced a manifest whose recorded
path contained a **carriage return**.

What that costs is not theoretical, and it is not about parsing. The entire
product of this module is a report somebody reads to decide whether their files
have been altered, and that report is printed to a terminal. A carriage return
returns the cursor to the start of the line, so everything already printed is
overwritten by whatever follows it: a crafted path makes the report say
something other than what is recorded. An escape character does more again, and
can colour, move the cursor, or clear the screen. **A tamper report that can be
made to lie is the feature failing at the only thing it does.**

Both ends now refuse the same thing, and they refuse the whole C0 and C1
control range rather than the two characters that were found, because listing
the ones somebody thought of is how the next one gets in. Refusing rather than
stripping is deliberate: a path this format cannot represent faithfully is one
it must not claim to hold. Such a filename is legal on Unix and vanishingly
rare, and being told so is better than a record that quietly describes a
different file.

The asymmetry is the thing to hold rather than the character list, so the test
asserts it directly: what `of` will not write, `parse` will not read.

### F-82 -- a header could ask for four billion passes, and get them

`veilvoice-crypto/src/kdf.rs`. The Argon2 memory cost has had a documented
ceiling since F-2 and F-3, with a long note explaining that `m_cost` arrives
from the file, that Argon2 allocates it before doing anything else, and that a
header claiming `u32::MAX` asks for four terabytes. The time cost had a test
for zero and nothing else.

Nothing overflows and nothing allocates, so every check passed. The derivation
simply did not finish.

Found by the coverage-guided campaign, which produced a header declaring
`m_cost` 65535, `t_cost` 4,521,984 and `p_cost` 1280. Every existing test that
header would meet says it is fine: the arithmetic is in range, the memory is
under the ceiling, and `m_cost >= p_cost * 8` holds. **Measured in a release
build: about 74 hours.** That is not the worst case, only the one the fuzzer
happened to reach; `u32::MAX` passes at the same memory is roughly eight years.

**It matters in two places and the second is worse.** A `.veil` file is
something somebody sent you, and merely attempting to open it hangs the
program. The app-lock file carries the same three numbers and is read **before
anyone has authenticated**, so anything able to write it could stop VeilVoice
from starting, for ever, with no error and nothing to see. That is precisely
the argument the memory ceiling already makes, and nobody had made it about
time. The campaign found it through both doors independently, producing a
lock-file input declaring 1,279,870,294 passes.

`MAX_T_COST` is 16, enforced in `checked`, the single funnel every derivation
passes through, so it holds for the container, for the app lock and for
anything built against these crates. Chosen by measurement rather than by
feel: RFC 9106's two recommended profiles use one pass and three, libsodium's
most expensive preset uses four, and this crate's default is three, so 16 is
four times the highest of them. At the memory ceiling it is 75 seconds and at
the unattended ceiling 18. The most expensive header this build accepts is now
a wait somebody can sit through.

One ceiling, not two. The first attempt also put a tighter one inside
`KdfParams::within`, which is wrong: `within` is on the *attended* path as well,
since `open_with_password` is the same call with a larger number, so it would
have refused a container a person had deliberately chosen to open. Caught by
noticing that the regression test still passed with `MAX_T_COST` raised to
`u32::MAX`, which meant something else was doing the refusing.

The exact bytes are now a regression test in
`crates/veilvoice-crypto/tests/parser_fuzz.rs`, where they run on every commit
on every platform with no nightly toolchain. It is written with a deadline on a
thread rather than a stopwatch around the call, because the defect is a *hang*:
a test that times the call after it returns cannot fail, it can only never
finish, and a test that hangs says less than one that fails in five seconds and
names the reason.

### F-81 -- every package definition was five releases behind

`packaging/`. Six files name a version. All six said 0.1.9 while the workspace
was at 0.1.14.

What that meant, per file, rather than as a general complaint: `brew install
--build-from-source` would have fetched and compiled the **v0.1.9** tarball;
`flatpak-builder` would have checked out the **v0.1.9** tag; the AppStream
metadata would have told a software centre that 0.1.9 is the newest release
there is; and `rpmbuild` with no `--define` would have built a package stamped
0.1.9 from whatever source it was pointed at. Two of the commands printed in
`docs/PACKAGING.md` for a reader to copy carried the same number.

Nobody had noticed because nobody was looking. This is the shape the repository
keeps finding: F-41 was generated output drifting from its generator, F-61 and
F-63 were comments that had stopped being true, F-71 was two hand-typed numbers
agreeing with each other. Here it is six files agreeing with a number that had
moved on without them, in the one directory this document had already recorded
as never having been built or run.

All six are at the workspace version, and
`tools/site-tests/packaging.test.js` compares nine version claims against
`[workspace.package]` in `Cargo.toml` and fails the build when any of them
disagrees. Verified by putting the old version back in one file and watching it
fail, rather than by assuming a new test tests anything.

The AppStream file lists the newest release only. Listing every release means a
date beside each one, and the dates between 0.1.9 and 0.1.14 are not recorded
anywhere this file could be generated from. An invented date is the kind of
unchecked claim this project refuses everywhere else.

### F-80 -- the documented way to build the Debian package could not run

`packaging/debian/`, and the recipe in `docs/PACKAGING.md`. Two things, either
of which stops it before any compilation begins.

**There was no `debian/changelog`.** `dpkg-buildpackage` reads the package's
version out of that file and refuses to start without it: `error: cannot open
file debian/changelog`. The recipe copies `packaging/debian` into place and
runs the build, and nothing in either creates one.

**And `packaging/debian/rules` was tracked as mode 100644.** `dpkg-buildpackage`
runs `debian/rules` directly, so it has to be executable, and the mode git
records is the mode everybody who clones gets.

So the printed recipe failed on its first command for anybody who tried it, and
`docs/PACKAGING.md` said "not built" without saying "and it cannot be". The
first is honest about the outcome; it is not the same as knowing the route is
broken.

Both are fixed and both are now checked, the mode through `git ls-files -s`
rather than through the filesystem, because that is what other people clone.

**With those two in place it builds, and the row in `docs/PACKAGING.md` moves
from no to yes.** `veilvoice_0.1.14-1_amd64.deb` and
`veilvoice-gui_0.1.14-1_amd64.deb`, both installed with `dpkg -i`, the
installed `veilvoice --version` reporting 0.1.14, `veilvoice info` and
`veilvoice-verify --help` running, and both removing cleanly. The release build
and `cargo test --release --workspace` ran as part of it, because that is what
`debian/rules` does.

One machine, x86-64, Ubuntu 24.04, with a rustup toolchain rather than Debian's
own `cargo` and `rustc` packages, which is why `-d` was needed to get past
`dpkg-checkbuilddeps`. `lintian` has not been run and nothing has been uploaded
anywhere. That is written into `docs/PACKAGING.md` beside the yes.

### F-79 -- a security step that printed somebody else's error above its own "ok"

`install/install.sh`, and the same place in `install/install.ps1`. The signing
key is fetched from the website, and from the repository if the website does
not answer. A failure of the first is not a failure at all, which is the whole
reason there are two addresses.

But `fetch` is `curl -fsSL`, and `-S` prints curl's own message on stderr even
in silent mode. So on a machine where the first address does not answer, what a
reader saw was:

```
==> Checking the signing key's fingerprint
curl: (22) The requested URL returned error: 403
  ok   fingerprint matches 8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A
```

An error from another program, inside the step that anchors every other check
in the script, immediately above the word `ok`, with nothing to say those two
lines are about different things. The check had in fact passed. Somebody
reading that output has to know how the script is written to know that.

Measured on a machine whose network refuses `tilas01.github.io` and allows
`raw.githubusercontent.com`, which is an ordinary situation rather than a
contrived one: a filtering proxy, a restricted network, or GitHub Pages being
down for a few minutes.

The first attempt's own output is now suppressed and replaced with a line in
the script's voice saying which copy it is falling back to. The final failure
stays loud and still names both addresses. The PowerShell installer never
printed the raw error, because its fetch swallows the exception, but it also
never said it had fallen back; it says so now.

**This one shipped.** It is in v0.1.14 and in every release with an installer
before it. It is not a verification failure, and that distinction is worth
keeping: no check was skipped and nothing unverified was installed. What
failed was the script's account of itself, in the one place where a reader is
being asked to trust a chain of checks they cannot see.

### F-78 -- four tests shared one global and could undo each other

`veilvoice-gui/src/theme.rs`. The active theme is a process-global
`AtomicUsize`, which is right: it is read on every repaint and an atomic load
is what that path can afford. Four tests in the module read and write it, and
`cargo test` runs the tests of one crate on parallel threads in one process.

Nothing kept them apart. So `switching_theme_changes_the_palette_and_the_visuals`
could set the theme to `paper`, and another test could put the index back to
zero before the assertion that the palette had changed, and the palette had
not. A third stores `usize::MAX` deliberately, to prove an impossible index
saturates rather than panics, and any test reading the palette while that value
was in place saw the last theme in the table instead of the first.

**Measured: one failure in forty runs of that module alone**, and it fired for
real in a full-workspace run, where more threads are competing. The tests that
touch the global now take a mutex in turn, and sixty runs after that produced
no failures.

The reason this is written up rather than quietly fixed is what a flaky test
does to everything else. A suite that fails one run in forty teaches the person
running it that a red result means run it again, which is precisely the habit
that lets a real failure through. This project's argument is that its claims
are checkable; a check that answers differently on different runs is not one.

A sweep for the same shape across the tree found no second instance: this is
the only test module in any crate that writes process-wide state.

### F-77 -- a count the front page states was measured on one machine only

The front page says a number of tests, `docs/AUDIT.md` says the same number,
and both are checked against `docs/MEASURED.md`, which is generated by actually
running the suite. That arrangement was built to end F-71, where two hand-typed
copies of a claim agreed with each other and both were wrong, and it does end
it. What it does not do is make the number a fact about the tree.

**Measured: the same commit reports 996 tests on Windows and 988 on Linux.**
Nine tests are compiled only on Windows and are simply not there elsewhere. So
a reader on Linux who does the thing this project keeps inviting them to do,
which is check the claim rather than believe it, runs `cargo test --workspace`,
counts 988, and finds the front page saying 996.

The number is not wrong so much as unqualified, and an unqualified number reads
as a property of the code. This is the same class as the findings above it: a
claim that is true about the thing that produced it and is presented as true
about something larger.

`docs/MEASURED.md` now carries the host triple the count was taken on, and says
in its own header that the total differs by platform and by how much. The
number itself stays measured rather than typed, and whoever regenerates it
leaves their platform beside it.

**What this does not fix.** The count on the published page is still one
platform's, and which platform depends on who last regenerated it. Wording it
so that it is true wherever it is read is a change to the front page's own
voice and is the maintainer's to make; it is listed under "still open" below
rather than done quietly here.

### F-76 -- a program that had died was reported as still running

`veilvoice-failsafe/src/act.rs`. F-74's fix asks the operating system, twice,
whether a process id still belongs to the program it is about to close, and
then asks again afterwards to find out whether the close worked. On Unix that
question was asked by name, and the name is the wrong question.

**A process that has exited keeps its id and its name until its parent collects
its exit status.** Measured on Linux, on a child this project's own test suite
started: after `kill -TERM`, `/proc/<pid>/comm` still reads `sleep`,
`ps -p <pid> -o comm=` still prints `sleep` and still exits 0, and the only
field that has changed is the run state, which is now `Z`.

So the check answered "yes, still there" about a program that was already dead.
`close` then waited out its whole retry loop, about two and three quarter
seconds, and told the reader to go and close by hand a program that had closed
itself at the start of it. **That is F-74's false report in the other
direction.** F-74 said it had closed something it had not; this said it had
failed to close something it had. Both are a safety catch describing an event
that did not happen, and the second one sends somebody looking for a window
that is not there while the real one, if there is one, goes unmentioned.

The state is now read first, from `/proc/<pid>/stat` where it exists and from
`ps -o state=` where it does not, and a process that has ended counts as gone,
because it has. A `ps` with no `state` column falls back to the name check
rather than to a refusal, so a platform without it loses the extra check and
not the feature.

**This was found by running the suite on Linux**, where it failed two tests
that pass on Windows. That is the eighth round's F-72 happening again in the
same place: a test whose result depends on the platform it is run on is only
evidence about that platform. The Windows process table drops a terminated
process at once and has no equivalent state to be caught by, so no amount of
running it there would have shown this.

### F-74 -- Failsafe could close the wrong program, and say so either way

`veilvoice-failsafe/src/act.rs`. Two defects in one path, and the second is the
worse of them.

**A process id is not a durable handle to a program.** Between the scan that
finds a program holding a microphone and the line that closes it, that program
can exit and the operating system can hand its id to something else. Closing by
number alone would terminate whatever inherited it. The window is small and it
is not theoretical: this feature exists to fire while somebody is plugging
things in and programs are starting and stopping, which is exactly when ids are
being recycled.

**And `taskkill` exits 0 whether or not it killed anything.** Measured: given a
filter that matches nothing it prints `INFO: No tasks running with the
specified criteria` and returns success, indistinguishable from a real
termination. The code checked `status.success()`, so Failsafe would have
recorded *"closed Discord (process 4812)"* in its log while Discord carried on
sending audio. **That is the worst sentence a safety catch can produce**: it is
not a failure to act, it is a false report of having acted, and the whole
feature exists for the case where nobody is watching the window.

The name now travels with the kill. On Windows `taskkill` is given
`/FI "IMAGENAME eq ..."` as well as the id, so the check and the act are one
operation rather than two with a gap; measured, a mismatched name leaves the
process running. Elsewhere the name is checked immediately before, which
narrows the window rather than closing it, and that is said rather than
implied. Where the check cannot be answered at all, the answer is *no*: not
closing something is recoverable and closing the wrong thing is not.

Afterwards the process is looked for again, with a short backoff, and a kill
that did not kill reports so. The test that found this closes a real process it
started; the version of it that passed a made-up name now correctly fails to
close anything, which is how the fix was confirmed.

### F-75 -- the application baseline was written world-readable

`veilvoice-cli/src/appctl.rs`. `veilvoice appctl` records what normally runs on
this machine and writes it with `std::fs::write`, which takes the default
permissions.

That file decides what counts as ordinary, which makes it a security setting
rather than a convenience. Another local account could add a line and have a
program of their choosing treated as unremarkable for ever, or simply read it
to learn exactly what runs on this machine and when.

This project already has one place that gets file permissions right, and the
part that matters is that it sets them **as the file is created** rather than
afterwards: a file that exists for even a moment with the wrong permissions is
a file somebody else's program may have read in that moment. The baseline now
goes through it.

## The eighth round

**Eight defects found and fixed (F-66 to F-73.)** Seven had not shipped.
**F-73 had**: the randomised ratchet interval was implemented, documented as
being used, and called by nothing, so every released copy rolled on the same
fixed two-second period. It is the first defect in three rounds that was in a
shipped release rather than in work in progress, and it was found by looking
for a function's callers rather than by reading it.

**Two of the seven were found by continuous integration rather than by
anybody's judgement**, and both had been watched to pass on the machine they
failed on. That is the round's real lesson: the checks that matter most are the
ones that run somewhere the author is not.

This round covers what was added after it: the measured voice limit and the
one-voice-for-everybody mode, saved projects and application profiles, the
table of communication programs, and the build-and-reproduce half of the
verifier.

**Both are the same shape as most of the last round, and it is worth naming:**
a piece of code answering from a *default* rather than from the state actually
in force. F-66 wrote a value that had trimmed away to nothing and read it back
as a different thing than was saved. F-67 computed a group render -- and the
speaker limit shown beside it -- from `DeidConfig::default()` while the person
had set the strength and the accent work somewhere else in the window.

F-67 is the serious one, and it is the kind this project's second rule exists
for. Somebody who set the strength to its highest, turned accent neutralisation
on, and then rendered a group conversation got a render at the **default**
strength with the accent work **off**. Nothing in the window said so; the
controls were on a different tab, and the group panel had never been given
them. That is software quietly doing less than it says, which is the failure
this whole tree is written against.

### F-73 -- the randomised ratchet was written, documented, and never called

`veilvoice-core/src/chain.rs`, and both front ends. `DeidConfig::reseed_range_ms`
replaces the fixed roll interval with one drawn fresh before every roll, so the
ratchet has no period to observe. `with_random_reseed_range` draws that range
from the OS CSPRNG. Both were implemented, both were tested, and the field's own
documentation said:

> Not part of `DeidConfig::default`, which stays deterministic so the test suite
> does. **The front ends call `DeidConfig::with_random_reseed_range` at launch**,
> which is what makes the shipped interval something other than a number
> compiled in.

Nothing called it. `with_random_reseed_range` appeared three times in the whole
tree: its definition, that sentence, and one test of itself. So every shipped
copy of VeilVoice rolled the modulation seed every two seconds exactly -- a
number compiled into the binary, which is precisely what the sentence said was
not happening.

**What it is worth is small and real.** The ratchet is forward secrecy, not
irreversibility: the many-to-one mapping is what destroys the voiceprint, and it
does not depend on the ratchet at all. A predictable roll period does not make a
voice recoverable. What it gives an observer is a clean segment boundary every
two seconds in every recording VeilVoice has ever produced, in every copy, which
is a property of the *program* rather than of the session -- and removing that
is the entire reason the feature was written.

**What makes it worth an entry is the shape.** This is the fourth defect in two
rounds where a sentence was true about the design and false about the code, and
the first where the sentence described work that had been *finished* and simply
never wired up. Nothing was broken; a call was missing. Reading the module would
not find it, reading the tests would not find it -- the feature had a passing
test -- and it took looking for the callers of a function to notice there were
none.

A comment cannot be tested, so the fix tests the code the comment is about: a
test reads both front ends' source and fails the build if the call is not there.

Marker 28 was completed alongside it, since the engine half already existed:
the interval is now user-configurable, and **anything that is not a usable range
is refused with the reason rather than clamped**. Six distinct refusals -- not
two numbers, not a number, not positive, backwards, too short, too long -- each
naming which end was wrong and what the bound is. Clamping would leave somebody
running on a setting they did not choose and cannot see, which for a control
whose whole purpose is unpredictability is the worst available failure.

### F-72 -- three tests passed on this machine and failed on the same platform

`veilvoice-verify`, `veilvoice-guard`, `veilvoice-gui`, `veilvoice-watch`. Several
tests read this project's own source with `include_str!` and search it -- for a
forbidden call inside one function's body, for an ungated `println!`, for a
probe that looks up a program by name. They locate a function's end by finding
`"\n}\n"`.

They passed locally and failed on GitHub's Windows runners. Not on a different
platform: **on the same one**, minutes after being watched to pass. This machine
has `core.autocrlf=input`, so its checkout uses LF; GitHub's Windows runners
default to `core.autocrlf=true`, so theirs arrives with CRLF, the pattern
matches nothing, and `.expect("its end")` panics.

There was no `.gitattributes`, so the line endings of a checkout were whatever
each contributor's git happened to be set to.

**The tests are the small half.** The serious half is the generators. Every
artefact here is regenerated and compared **byte for byte** by
`tools/verify.py`, and every generator writes LF unconditionally. A contributor
whose git converts text on checkout would find every `--check` failing on files
they had never touched, with a diff that shows nothing, on their first run --
and the natural conclusion is that the repository is broken rather than that
their git is configured differently. Nothing in the tree said otherwise.

So `.gitattributes` pins text to LF for everyone, names the binary formats
explicitly rather than trusting detection to guess right on a `.wav`, and says
in the file why. The source-reading tests normalise as well, because a test
that depends on a git setting is a test somebody will trip over on a machine
nobody here owns.

Three guards, and the second is the one worth having:

* Every `include_str!` in a test file must be followed by the normalisation, or
  the suite fails and names the line.
* The failure mode is a test of its own: a search for `"\n}\n"` is asserted to
  succeed against LF and to **fail** against CRLF, so it is on record as
  reachable rather than as a story about it.
* `.gitattributes` is read from a test and must pin `eol=lf`, because nothing
  else in the build would notice it being deleted.

Written the first time, the first guard matched its own detection line and
reported itself as the defect. Its needle is assembled at run time now.

### F-71 -- the guard against stale claims compared one copy to another

`tools/site-tests/css.test.js`. The front page said **354 tests** and "no
unsafe code, in any of the **nine** crates". The tree held 890 tests across 19
crates. `docs/AUDIT.md` said "354 tests across 9 crates, plus doctests and 10
site-test suites"; the runner runs 11.

The part that makes this a defect rather than a stale line is that **a guard
existed and passed**. It was written after an earlier round of exactly this
drift, and its own comment says so:

> It said "336 tests" and "47 defects across four audit rounds" while the tree
> had 354 and 59. [...] Everything else in this repository that makes a claim
> is generated and checked; this was the one place claims were hand-typed with
> nothing watching them.

And then it chained one hand-typed number to another: it compared the front
page against `docs/AUDIT.md` and failed only if the two disagreed. Both were
typed by the same hand at the same time, so both drifted together and the check
reported success for four rounds. **A check that compares one copy of a claim
against another copy agrees with itself.** That is the same defect as F-61 and
F-63 -- a statement that was true when written, with nothing tied to the thing
it describes -- arriving for the third time in two rounds, this time inside the
control written to prevent it.

The numbers now come from the tree. `tools/measured/generate.py` writes
`docs/MEASURED.md`: the test count **by running the tests**, the crate count
from `Cargo.toml`, and the suite count from `run.js`'s own list. Every claim in
the page and in this document is compared against that file, and the generator
runs inside `tools/verify.py` before the suites that read it.

Three details, each of which is a way this would have failed again:

* **The test count is measured, not counted.** A static count of `#[test]`
  gives 903 against a measured 890: some tests sit behind features that are not
  on by default. Counting the attribute would have produced a new wrong number
  with a new reason to trust it.
* **The suite count comes from the runner's list, not from a directory
  listing.** A suite file that exists and is not in `SUITES` does not run, and
  counting it would state a number nobody gets.
* **Spelled-out numbers are refused.** "the nine crates" is exactly how this
  drifted with nothing noticing, because no check can compare a word. The suite
  now fails if the crate count is written as a word.

The guard was checked by breaking it: the page's test count was set back to 354
and the suite failed with *"the front page's test count says 354, the tree
measures 890"*, then passed again when restored. A control nobody has watched
fail is a control nobody has tested.

### F-70 -- the reproducibility checker would have said no to everybody

`veilvoice-verify/src/builder.rs`. The build ran `cargo build --release
--workspace --locked` and nothing else. No `--remap-path-prefix`, no
`SOURCE_DATE_EPOCH`, no per-linker determinism flag, and no `--target`.

Every one of those is set by `.github/workflows/release.yml` when the published
binaries are built, and `docs/REPRODUCIBLE_BUILDS.md` has said since before this
checker existed that reproducibility depends on the build *environment* setting
them. So the comparison was decided before it ran: a user's build would differ
from the published one every time, and the tool would tell them so.

**Measured rather than reasoned about.** Two builds of this tree, in two target
directories on this machine, with the checker as first written: three binaries,
three different hashes. The cause is the dull one the module's own documentation
already listed -- the absolute path of the source tree is baked into panic
messages and debug info, so a build in one directory and a build in another
cannot be the same bytes.

The severity is not in the code. It is in what the tool would have taught the
one reader who took the trouble to build from source: that the release does not
match its source. **A checker that always answers "not reproducible" is worse
than no checker at all**, because the next time it says so for a real reason,
that reader has already learned to ignore it. This is the second rule of the
project -- never overstate what the software does -- failing in the other
direction, and it is worth recording that the other direction exists.

It now reproduces the release environment rather than approximating it: the
same remapping for the source tree and for `CARGO_HOME`, the same
`SOURCE_DATE_EPOCH` taken from the commit being built, `/Brepro` on MSVC and
`-no_uuid` on ld64, and the same explicit `--target` -- which also moves the
output down one level, so F-69's directory question had to be answered again.

Three details worth keeping:

* **The remapped path is the one the compiler is given.** Not
  `std::fs::canonicalize`, which on Windows returns an extended-length path
  beginning `\\?\`. Cargo never hands rustc that form, so a remap built from it
  matches nothing and does nothing, silently, with every check still passing.
  The release workflow records the same failure on macOS, where `/tmp`
  canonicalises to `/private/tmp`; this is the same trap through the other
  platform's door.
* **`RUSTFLAGS` is set, not appended to.** A value inherited from the
  terminal is a value the published build did not have.
* **Outside a git checkout there is no commit date, and it says so** rather
  than substituting one. An invented timestamp would make the build differ from
  the published one for a brand new reason.

**A third remap had to be added, and finding it needed the measurement.**
With the source tree and `CARGO_HOME` remapped, two builds gave two identical
binaries and one that differed: `veilvoice-gui`. The cause is `OUT_DIR`, which
lives under the *target* directory and reaches a binary through a build script.
The release workflow never meets this, because it builds into `target/` inside
the source tree it is already remapping, so `OUT_DIR` is covered for free -- and
a checker that compares two builds in two separate target directories does not
get that for free. Without it the tool would have reported a difference caused
entirely by where it chose to put its own build output: not a false negative it
inherited, but one it manufactured.

Measured at each stage, on this machine, two builds in two target directories:

| | `veilvoice` | `veilvoice-gui` | `veilvoice-verify` |
|---|---|---|---|
| As first written | differs | differs | differs |
| Source and `CARGO_HOME` remapped | identical | **differs** | identical |
| Target directory remapped as well | identical | identical | identical |

Every setting is printed before the build. A test compares the flags against
`release.yml` itself, so changing one without the other fails the build.

### F-69 -- the build succeeded, and then looked for it in the wrong place

`veilvoice-verify/src/builder.rs`. After running the release build, the tool
hashed what came out of `root/target/release` -- a path it computed rather than
asked for. `CARGO_TARGET_DIR` in the environment, `build.target-dir` in a
`.cargo/config.toml`, and a target directory shared between checkouts all move
it, and none of those are exotic. The machine this was written on has the first
one set.

**The shape of the failure is the interesting part.** The build ran. It took
several minutes and it succeeded. Then the run ended with

```
  ok    the build finished

FAILED: the build left nothing to hash
  .\target\release is not there
```

and exit status 3. Correct work, thrown away, reported with a message pointing
at a directory that was never going to exist -- and reported as *incomplete*,
which is at least honest, but only by accident: nothing in the code knew it had
guessed.

It now asks `cargo metadata`, which is the only thing that knows. The
JSON is read by hand rather than by taking a dependency for one field, and the
escapes are undone properly, because the field is a Windows path and a value
taken between the first two quotes yields `C:\\Users\\...` -- a path that looks
almost right and does not open. Malformed input, an unterminated string and a
non-string value all give `None` rather than something plausible.

The test is written against the environment rather than against a fixture: the
suite itself runs with `CARGO_TARGET_DIR` set, and `OUT_DIR` is inside the real
target directory whatever it is, so a function that went back to guessing would
disagree with reality inside its own test run.

**Three of this round's four are the same mistake**, which is worth stating
plainly. F-67 answered from a default configuration instead of the one in
force. F-68 answered from a program that shared a name with the right one. F-69
answered from a path that is usually correct. None was a logic error and none
would have been found by reading the code -- each was found by running it and
looking at what it said. That is the same lesson as F-37 and the fifth round's
install-script defects, arriving for the third time.

### F-68 -- the linker check found Git's hardlink tool and called it a linker

`veilvoice-verify/src/deps.rs`. The Windows probe for a C linker looked for
`link` on `PATH` and reported whatever came back. On the machine it was first
run on that was `C:\Program Files\Git\usr\bin\link.exe` -- GNU coreutils'
hardlink utility, which shares a name with Microsoft's linker and has nothing
whatever to do with building Rust.

So the dependency report said `A C compiler and linker  found` and named a path
that looked entirely convincing, on a machine where a build would then have
stopped with a linker error. **A probe that answers from the wrong program is
worse than no probe**: absence would have produced a useful answer, and this
produced a confident wrong one.

It is in this document rather than in a commit message because of *how it was
found*. It was found by running the command and reading the path it printed --
the same route as F-37 and as the three install-script defects in the fifth
round, and not a route any test was going to take. Nothing was wrong with the
code as written; `which("link")` did exactly what it says. The mistake was
believing a name.

There is no honest probe here, and that is the fix. `link.exe` is only on
`PATH` inside a Developer Command Prompt, cargo locates MSVC through the
registry instead, and any `link` that *is* on `PATH` is more likely to be
something else. It now returns "could not tell", in those words, with the
reason -- and [`Presence::Unknown`] was already distinguished from
[`Presence::Missing`] precisely so that an unanswerable probe does not become
an offer to install something that is already there.

### F-67 -- the group panel rendered with the default settings, not yours

`veilvoice-gui/src/group.rs`. Every question the panel answered about voices --
how many speakers it would allow, which mode it would let you switch to, what a
profile could be applied to, and the render itself -- was computed from
`DeidConfig::default()`. The application's own settings, which is what the rest
of the window acts on, were never handed to it.

It is wrong twice over.

The render is the serious half: `render_now` built its `Settings` from the
defaults and overrode only the sample rate, so the intensity, the accent
configuration and the reseed interval that had been chosen were all discarded.
A group render was weaker than the one that was asked for and reported success.

The limit is the quieter half. `voices::clear_voices` depends on the frame
grid, because a coarser grid snaps destination pitches onto wider steps and
collapses registers onto each other. Under a configuration where fewer than
eight voices stay clearly apart, the panel still printed "8" and still let
eight people be added -- and two of them would have shared a voice, discovered
only by listening to the finished recording.

The panel now carries the configuration, copied from the application before
anything is painted, and uses it for all four. The regression test moves the
frame size to something that genuinely lowers the count and checks that the
number the panel *prints* and the number it *enforces* both follow -- a test
that would pass against the old code if it only checked the default.

### F-66 -- a saved project could come back different from how it went out

`veilvoice-workspace/src/lib.rs`. A value that trimmed away to nothing was
written as a key with an empty value: `Some("   ")` went out as `title  ` and
came back as `Some("")`. Neither what was saved nor absent.

The reachable half is the reader's. A hand-edited or truncated project file
carrying `plan  ` with nothing after it produced `Some(PathBuf::from(""))`,
which is a plan path that names no file and fails later with a message about a
file called nothing, rather than being read as "no plan named" at the point
where that is still a sentence somebody can act on.

Small, and it is here because of *how it was found*: the round-trip test only
ever exercised one tidy project. The fix is the property rather than the case
-- every shape a project can be in is now saved, read, saved and read again,
including empty and whitespace values, no members, the maximum members, no
outputs at all, and names containing the field separator and a line break.

Writer and reader are now symmetric in both directions: a value that trims to
nothing is **absent**, and an empty value read back is `None`.

## The seventh round

**Five defects found and fixed (F-61 to F-65.)** **None had shipped**: every
one was in code written during this cycle, and `main` has not been released
since v0.1.12.

This round covers what has been added since then: a manual update check, a
shared release-checking library and the desktop verify tab that made it
necessary, group mode in the window, a GIF encoder, video palettes, two
generators for pictures of the application and the command line, and the
website's own source documented for the first time.

**The most useful finding is the least dramatic.** F-61 was a comment that
described the code correctly *if* you read only the comment: it said a dropped
file was read "before anything is drawn", and the call sat at the bottom of
`update`, after the panel that shows the result. A wrong thing that agrees with
itself survives a reading, and this one had survived several. It was found by
looking for the repaint that F-62 turned out to be missing.

**Three of the four are the same shape**: a claim that was true when written,
or true of a simpler version, and never re-measured. F-63 is a stylesheet
comment claiming a change was invisible on a desktop when it moves a table by
forty pixels; F-61 is a comment describing a call that had moved. The
project's rule about never overstating what the software does turns out to
apply to comments about layout exactly as it does to claims about cryptography.

**What was checked and found sound.** The GIF encoder's dictionary-reset path
had never been exercised by an independent decoder -- the banner's own frames
may not reach 4096 codes. A 512x512 field of pseudo-random indices was built to
force it, reset the dictionary 66 times, and was decoded by Windows' GDI+ with
**zero mismatched pixels in 262,144**. The colour table refuses a 257th colour
rather than truncating, the sub-block framing is exact at 255 and 256 bytes,
and none of the encoder's paths falls over on empty input.

### F-61 -- a dropped file was read a frame after the panel that shows it

`veilvoice-gui/src/app.rs`. `Verify::take_dropped` was called at the end of
`update`, after `CentralPanel` had already been painted, so a file dropped on
the window and the highlight under a hovering one were both one frame stale.
The comment above the call said "read once a frame, before anything is drawn".

Moved to the top of `update`, before any panel. The comment is now true.

### F-62 -- nothing woke the window while a file was hovering over it

`veilvoice-gui/src/app.rs`, `verify.rs`. An idle egui window repaints only when
something asks it to, and the repaint condition listed every *busy* state and
no hovering state. Dragging a file over an otherwise idle window therefore lit
nothing up, and the dropped file did not appear until the mouse moved for some
other reason -- the one moment in this application where the user is waiting
for the window to react and the window has decided nothing is happening.

`Verify::wants_repaint` is busy **or** hovering, and the frame loop asks it.

### F-63 -- a stylesheet comment claimed a change nothing could see

`website/css/main.css`. Making tables their own sideways scrollers stopped the
reference pages scrolling on a phone, and the comment beside it said "nothing
changes on a desktop". Measured: the security page's tables render at 820 px
inside an 860 px column, so the row rules stop forty pixels short of the text
above them. `width: 100%` does not restore it -- tried and measured -- because
the shrink happens on the anonymous table box inside the block, not on the
block.

The trade is still worth making. The comment now says what it costs.

### F-65 -- two crates were invisible to the documentation generator

`tools/docs/generate.py`. `veilvoice-check` and `veilvoice-update` were added to
the workspace this cycle and to neither `CRATES` nor `ALL_CRATES`, so they had
no page, no banner, no diagram -- and no entry under "not yet covered" either.

That last part is what makes it a defect rather than a gap. `ALL_CRATES` exists
precisely so this tool can say what it is *not* covering rather than quietly
covering less than the tree contains; the audit's own section 4.5 describes a
silent partial pass as the failure mode to avoid. A crate in neither list is
**invisible rather than uncovered**, which is the one outcome those lists were
written to prevent, and it was reached by the ordinary act of adding a crate.

The lists are still written out -- a generator that discovers its own inputs
cannot tell you it is missing one -- but they are now checked against the
workspace manifest, in both directions, and a mismatch stops the run with the
names in it. Both crates are documented.

### F-64 -- a nameless line in a `SHA256SUMS` could answer a lookup

`veilvoice-check/src/lib.rs`. A malformed line carrying a digest and no name --
`aaaa   ` -- was parsed as a name of `""`, and `check_file` derived its `wanted`
name from `Path::file_name`, which is `None` for a directory, a root or `..`
and was turned into `""` by `unwrap_or_default`. A path with no final component
could therefore be answered with a digest belonging to nothing, and it would
look exactly like a successful lookup.

Neither half is reachable from either front end today -- both pass a real file
-- so this is a hole rather than a live defect. Both halves are closed: an
empty name never matches, and a path that names no file is refused with a
reason instead of being turned into an empty string.

## The sixth round

**Twelve defects found and fixed (F-48 to F-59.)** **Two had shipped** -- both
live on the published site, one of them the appearance of the default theme --
and ten were caught in code written during this round. Section 2.5 keeps those
groups apart rather than counting them together.

This round covers what has been added since v0.1.9: a documentation generator
that writes 366 files including HTML this site publishes, a parser for
user-written colour palettes, a headless-browser driver, a cycling strip of
claims on the front page, and a licence change touching 352 files.

**Two findings are the reason to keep building the check before trusting the
thing.** F-49 -- the default theme's secondary text failing WCAG contrast on
every surface this project has, including the licence line inside its own
banner -- was found by writing a contrast rule for *other people's* palettes
and then pointing it at this project's own. F-50 -- the generator silently
overwriting the file recording that the fuzz targets have never been run to
convergence -- was found by reading the diff of a successful run.

*(The previous round: eleven defects, F-37 to F-47, three of which had
shipped.)*

The round covers what v0.1.9 adds: a search index over the whole repository and
website, a portable release verifier that needs no GnuPG, install scripts,
packaging definitions for six formats, and an animated banner.

**One finding is the reason to keep looking at the page rather than at the
tests.** F-37 had the website rendering the text inside its own banner
illegibly, on every viewport, for as long as the banner has existed. Every unit
test passed throughout. It was found by rendering the page and reading it.

*(The previous round: twenty-eight defects, F-9 to F-36, thirteen in the Rust
and fifteen in the website.)*

The uncomfortable pattern repeats, and is worth naming rather than burying: the
previous round said the audit scope was *finished*. It was finished against the
list that round had drawn up. Drawing up a wider list found, among other things,
a four-kilobyte file that killed the process, a configuration value that made
every output sample silently `NaN`, an erase operation that would destroy a file
other than the one named, and a Markdown document that froze the reader's tab
for eight seconds. None of those needed a new technique to find. They needed
somebody to look at that particular thing.

Three of the new findings are the previous round's own recorded "open items"
(a scheme check on `repo.js`, a KDF cost ceiling, a coverage-guided fuzzing
setup). Those are now done or built. The rest were not on anybody's list.

---


## 1. Mechanical checks

| Check | Result |
|---|---|
| `unsafe` code | **None.** All 9 crates carry `#![forbid(unsafe_code)]`, enforced at compile time. |
| Generated documentation | 366 files -- a page, banner and flowchart for **every one of the 63 `.rs` files** and every crate -- verified against the source by `python tools/docs/generate.py --check` in CI. |
| `cargo clippy --workspace --all-targets` | **0 warnings**, both with and without the `live` feature. |
| `cargo fmt --all --check` | Clean. |
| `cargo audit` | **1 vulnerability, accepted on a narrow and enforced ground** -- see A-6. Two `unmaintained` advisories accepted with written reasoning in `.cargo/audit.toml`. |
| Test suite | 1125 tests across 27 crates, plus doctests and 14 site-test suites in `tools/site-tests`. These three numbers are measured into `docs/MEASURED.md` and checked against this line, because the previous guard compared them against the front page -- one hand-typed number against another -- and both drifted together (F-71). The test count is measured on one machine and is not the same on every platform: see F-77. |
| Coverage-guided fuzzing | 6 libFuzzer targets in `fuzz/`, one per parser that reads untrusted bytes. Built and type-checked; **not run to convergence** -- see section 5.2. |
| Networking crates in the graph | **None.** CI fails the build if `reqwest`/`hyper`/`curl`/`ureq`/`tungstenite`/`isahc`/`surf` appears. |
| `TODO`/`FIXME`/`HACK` markers | None. |
| Secrets in the repository | None. `gpg_secrets/` is gitignored; `*.asc` ignored by default with only the public key allowed back explicitly. |
| Dependency licences | All permissive (MIT / Apache-2.0 / BSD / ISC / BSL / CC0 / Zlib / Unicode-3.0). No copyleft conflict with GPL-3.0-or-later. Re-checked across the 155 packages rPGP adds. |

---

## 2. Findings

### 2.1 Found and fixed in earlier rounds (F-1 to F-8)

Kept in full rather than summarised. A finding with the details filed off is
not a finding, and several of these are the reason a later one was looked for
at all.

**F-1 — Duplicate seeding path that panicked (`veilvoice-core`).**
`Modulator::from_os_rng` read the OS CSPRNG and `.expect()`-ed on failure. It
was public API, never called, and duplicated `Deidentifier::new`, which does the
same thing and returns a `Result`. Two paths for one job where the unused one
aborts the process is a footgun in a security crate. **Removed.**

**F-2 — Argon2 parallelism overflow, reachable from any hostile file
(`veilvoice-crypto`). Denial of service.**
`argon2` 0.5.3 validates in the wrong order: `Params::new` evaluates
`m_cost < p_cost * 8` *before* it checks `p_cost > MAX_P_COST`. A `p_cost` above
`u32::MAX / 8` therefore overflows the multiplication. `p_cost` is read verbatim
from a `.veil` header — and from the app-lock file, which is parsed **before
anyone has authenticated**. With overflow checks on (every debug build, and any
project consuming these crates as libraries, which the README explicitly invites)
that is a panic on attacker-controlled input.

VeilVoice's own release profile sets `overflow-checks = false`, where the
multiplication wraps and the ceiling test then rejects it — so shipped binaries
were not affected. "Our release profile happens to make the panic unreachable"
is not a property to rely on and is not true for anyone building against these
crates. **Fixed** by validating in `KdfParams::checked()`, the single funnel
every derivation passes through, in arithmetic that cannot overflow. Found by
`tests/parser_fuzz.rs` within seconds of the campaign first running.

**F-3 — Unbounded Argon2 memory cost (`veilvoice-crypto`). Denial of service,
release builds included.**
`m_cost` is also read verbatim, and Argon2 allocates that many KiB before doing
anything else. A header claiming `u32::MAX` asks for **4 TiB**; the allocation
fails, and a failed allocation in Rust aborts the process. Merely *attempting to
open* a hostile container killed the program, in debug and release alike. For the
app lock it is worse — anything able to write that file could stop VeilVoice
from starting at all.

**Fixed** with a documented ceiling of 4 GiB (`KdfParams::MAX_M_COST`), chosen to
sit above RFC 9106's largest recommended profile (2 GiB) and this crate's own
default (256 MiB) while refusing absurd values. Found by the same campaign,
immediately after F-2 was fixed — which is the argument for running a fuzzer to
exhaustion rather than until the first green run.

**Residual, stated rather than fixed:** a container may still declare a
legitimate-but-expensive cost, so an attacker can make opening their file *slow*.
That is inherent to shipping the cost with the file, which is what lets old files
open after defaults rise. Slow is not crashing, the user chose to open that file,
and they can stop waiting.

**F-4 — 32-bit overflow in the RIFF chunk walker (`veilvoice-meta`).**
`clean_wav_bytes` computed `declared + 8` where `declared` is a `u32` widened to
`usize`. On a 64-bit host that cannot overflow — which is why no amount of
fuzzing on this machine would ever have found it. **VeilVoice ships an ARMv7
build**, where `u32::MAX + 8` overflows `usize` and panics under overflow checks.
**Fixed** with `saturating_add`, which is also the correct semantics since the
value is clamped to the real length on the next line. Found by reading, and
recorded here as the counter-example to "the fuzzer is green, therefore the
parser is fine".

**F-5 — One NaN sample permanently destroyed the engine (`veilvoice-core`).
Silent, total, and reachable from an input file.**
The accent neutraliser's long-term spectrum is an exponential moving average, so
anything folded into it never washes out. A single non-finite input sample
reached it, and from then on **every output sample was NaN for the rest of the
session** — with nothing reported to the user. Measured: after one NaN, 0 of
48,000 subsequent samples were finite.

A 32-bit-float WAV can legally contain NaN, and `symphonia` decodes it
faithfully rather than sanitising. The realistic route is the ordinary one:
somebody sends you a recording and you veil it before passing it on. The failure
is silent, so the first sign is a recording that turned out to be silence.

**Fixed** at the single gate every sample passes through (`StftEngine::process`):
non-finite samples become zero, and magnitudes are bounded at ±1e6 — six orders
of magnitude above real audio, low enough that squaring and summing cannot reach
the float ceiling and produce a NaN by the other door.

Not a confidentiality failure: the output was garbage, not the original voice, so
it failed in the safe direction. It was still a total loss of function with no
error, and `tests/hostile_audio.rs` now covers it along with infinities,
full-scale DC, square waves, impulse trains and digital silence.

**F-6 — The site's Markdown renderer emitted unfiltered image URLs
(`website/js/markdown.js`).**
Links went through a scheme allowlist; images did not, so
`![x](javascript:...)` produced `<img src="javascript:...">`. `js/repo.js`
assigns the rendered README straight to `innerHTML`, so this was on the path from
a fetched file into the live page. No current browser executes a `javascript:`
image source, which is why it survived unnoticed — but "no browser we tested
still honours this" is not a security argument. **Fixed**: one `safeUrl` helper,
applied in both places.

**F-7 — The renderer silently deleted every string literal on the page
(`website/js/markdown.js`).**
Finished markup is parked while later passes run, and the placeholder was a
NUL-delimited decimal index. The number highlighter (`\b\d+\b`) matched the index
*inside the placeholder* and wrapped it in a span, after which the un-parking
pass no longer recognised it and **the parked content was discarded**. Every
string literal in every code block rendered as a stray highlighted digit:
`let s = "hello";` was published as `let s = 0 ;`. Adjacent placeholders shared a
delimiter as well, so alternate items were dropped.

Not a security bug. Arguably worse in context: the site's whole argument is "go
and read the source", and it was displaying source that was not the source.
**Fixed** with single-character private-use placeholders, which no pass matches
and which `escapeHtml` strips from the input so they cannot be forged.

**F-8 — The URL allowlist rejected ordinary relative links (same file).**
The scheme test was `^(?:https?:|[./#])`, so any target not beginning with `.`,
`/` or `#` was refused — which is most Markdown links. Every
`[whitepaper](docs/WHITEPAPER.md)` in the README rendered as plain text on the
site, and `js/repo.js`'s rewriting of repo-relative links could never fire.
Safe, wrong, and quietly wrong. **Fixed**: a scheme is required to be http(s);
no scheme means a relative path and is fine; protocol-relative `//host` and
leading backslashes are refused explicitly, since both look relative and behave
external.

### 2.2 Found and fixed in the third round -- the Rust

**F-9 -- A four-kilobyte WAV killed the process (`veilvoice-audio`). Denial of
service, reachable from a file somebody sends you.**

A WAV whose `fmt ` chunk declares a **sample rate of zero** makes `symphonia`
panic inside `Probe::format`, at `TimeBase::new` -- *before* VeilVoice is handed
anything it could inspect. VeilVoice's release profile sets `panic = "abort"`,
so that is not an error a caller can handle: it is the process ending.
`veilvoice anonymise` on a file somebody sent you was the whole exploit.

Confirmed by construction, and the boundary is narrow: of the malformed headers
tried -- zero channels, 65 535 channels, zero bits per sample, 65 535 bits per
sample, a mismatched format tag -- **every other one is already refused cleanly
by `symphonia` itself**. Only the zero rate crashes.

**Fixed** with a pre-flight check on the file's own bytes before the decoder is
given the stream (`io::preflight`). It reads the head from the same handle that
is then rewound and passed on, so the bytes checked are the bytes decoded rather
than the result of a second open. It is deliberately narrow -- duplicating what
the decoder already validates would be a second parser to keep in step, which is
its own bug source.

**Residual, stated rather than engineered around:** this cannot protect against
a panic in a decoder for a format VeilVoice does not itself parse. Under
`panic = "abort"` no wrapper can, short of decoding in a separate process. The
mitigations are this check, keeping `symphonia` current, and its being a
widely-used pure-Rust decoder rather than a C library.

**F-10 -- A configuration value made every output sample `NaN`, silently
(`veilvoice-core`). This is F-5 arriving through a second door.**

F-5 sanitised the *samples*. Nothing sanitised the *configuration they were
processed under*. `DeidConfig::checked()` tested `self.sample_rate < 8_000.0`,
and `NaN` compares false against every bound, so `NaN` passed validation. The
engine then built happily and produced `NaN` for every output sample, for the
whole session, with nothing reported -- the exact failure mode F-5 was written
up as, reached without a single bad sample ever being read.

Three defects in one place, all confirmed by running them:

- **`NaN`**: builds, and every output sample is `NaN`. Silent, total.
- **`INFINITY`**: panics with an arithmetic overflow at `pitch.rs:74` in every
  build with overflow checks on -- which is every debug build and any project
  consuming these crates as libraries, which the README explicitly invites.
- **A merely enormous value**: `sample_rate` sizes the delay lines in
  `effects.rs` (`Reverb`'s comb is `0.0297 x sample_rate` samples, and the three
  chorus voices are similar). A WAV's `fmt ` chunk carries a **`u32`** sample
  rate and `symphonia` passes it straight through, so a four-kilobyte file
  declaring `u32::MAX` asks for roughly **two gigabytes** of buffers before a
  single sample is processed. A failed allocation aborts. This one *is*
  file-reachable, and was confirmed end to end through `veilvoice anonymise`.

`frame_size` had no upper bound at all, and it sizes every internal buffer and
the FFT plan.

**Fixed** by validating the whole configuration rather than parts of it: every
float is checked for **finiteness first** and then for range; `sample_rate` is
bounded to 768 kHz (above every real converter -- professional hardware tops out
at 384 kHz); `frame_size` to 65 536; and the remaining floats are clamped to
values the DSP can act on, since each has a meaningful nearest legal value.
`tests/hostile_audio.rs` covers all of it, including that every configuration
which *does* build produces finite audio.

**F-11 -- Secure erase never terminated on a 32-bit build
(`veilvoice-crypto`). The file was left intact.**

`shred_file` sized its write buffer with `CHUNK.min(length.max(1) as usize)`.
On a 32-bit target -- and VeilVoice ships an ARMv7 build -- a file of exactly
4 GiB truncates to `0`, so the buffer was empty, `take` was always zero,
`remaining` never decreased, and the loop **ran for ever**. `veilvoice shred`
hung, and the file it was asked to destroy was still there.

The same 32-bit-only shape as F-4, and equally invisible to any campaign on an
x86-64 host. **Fixed** by computing the length in `u64` and only then narrowing
(`length.clamp(1, CHUNK as u64) as usize`), with an explicit refusal if `take`
is ever zero, so a loop that could reach that state fails instead of spinning.
Found by reading.

**F-12 -- Secure erase destroyed the wrong file (`veilvoice-crypto`).
Data loss, and a report that said it had succeeded.**

`shred_file` called `fs::metadata` (which follows symlinks), checked
`is_file()`, then opened the path (also following). Erasing a symbolic link
therefore filled **its target** with random data and unlinked only the link --
and reported `removed: true` about a file that was still there while an
unrelated one had been destroyed. Point it at a link named `recording.wav` and
whatever it referenced was gone.

The check was also a TOCTOU: the path could be replaced with a symlink between
the `metadata` call and the `open`, so the object checked was not necessarily
the object written.

**Fixed** on both counts: `symlink_metadata` first, refusing a link outright
with a new `Error::ShredSymlink` that explains why; then open, and ask the *open
handle* what it is, so no second lookup can disagree with the first. A caveat
about hard links was added to the report as well, since those share the data
without being links to the name. Regression tests confirm the target survives.

**F-13 -- A planted executable in the working directory was run
(`veilvoice-guard`, `veilvoice-watch`). Local code execution.**

`Command::new("reg")`, `Command::new("wevtutil")` and `Command::new("ausearch")`
do not name the system tool -- they name a *search*. Rust's `Command` resolves a
bare program name through the platform's search order, and **on Windows that
order includes the current working directory** ahead of most of `PATH`. Running
`veilvoice watch` or `veilvoice guard check` from a directory that happens to
contain a file called `reg.exe` executed it, as the user, with no prompt. A
downloads folder is enough.

That this is in the *monitor* and the *tamper detector* is the sharp part: they
are the two features somebody runs precisely because they suspect something is
already wrong on the machine.

**Fixed** by resolving every system tool to an absolute path -- `%SystemRoot%`'s
`System32` (and `Sysnative`, for a 32-bit process on 64-bit Windows), and a
fixed list of standard directories for `ausearch` -- and returning "I cannot
tell you" when the tool is not where it should be. That is the right answer for
these modules anyway: their whole design is to report honestly rather than
guess, and running *something else called `wevtutil`* is the worst possible
guess. Tested against a decoy.

**F-14 -- The app-lock verifier was world-readable on every save
(`veilvoice-crypto`).**

`LockStore::save` used `fs::write`, which creates a file with the process umask
-- ordinarily `0644` -- and only then chmod'd it to `0600`. The stored Argon2id
password verifier was therefore readable by every other local user for the
window between the two calls. That window reopened on **every save**, and a save
happens after every failed unlock attempt: an attacker who can cause failed
attempts can cause the window to reopen at will.

`LockStore::create` additionally tested `path.exists()` and then wrote, which
loses twice -- another process can win the race, and a symbolic link planted at
the lock path would have been followed, so the write would land on whatever it
pointed at.

**Fixed** with a new `veilvoice_crypto::privatefile` module that applies the
permission **at creation** via `OpenOptions::mode`, and offers an exclusive form
built on `create_new` so the refusal is one atomic answer from the kernel rather
than a check followed by a hope. Tests assert the mode both at creation and
after a save, and that a planted symlink is refused.

**F-15 -- The private key and the decrypted plaintext were written
world-readable (`veilvoice-cli`).**

The same create-then-chmod pattern on `veilvoice keygen`'s secret-key file
(mitigated by the key itself being encrypted, but the window had no reason to
exist), and no permission handling at all on the two outputs that most need it:
`veilvoice decrypt`'s plaintext, and `anonymise --encrypt false`'s recording --
which the tool's own warning describes, correctly, as still containing
everything that was said.

**Fixed** by routing all three through `privatefile`. The plaintext warning now
also states what that permission is and is not worth, and a test fails the build
if that sentence is ever softened -- a file permission must not start reading as
a substitute for the encryption the user just declined.

**F-16 -- Windows attribution gave a confidently wrong explanation
(`veilvoice-guard`).**

The Security-log query was built with `path.replace('\'', "''")`. Doubling a
quote is the SQL and XQuery rule; **XPath 1.0, which is what `wevtutil`
speaks, has no escape for a quote inside a string literal at all**. A path
containing an apostrophe therefore produced a syntactically broken query, which
failed, and the failure was reported to the user as *"object-access auditing is
off, or this needs to run elevated"*.

Not a code-execution route -- the query is an argument, not a shell string --
but for a module whose entire purpose is being honest about what it cannot see,
telling somebody worried about surveillance the wrong reason is the specific
failure it was written to avoid. **Fixed** by declining the query for such a
path and saying exactly that, since the character genuinely cannot be expressed.

**F-17 -- Decoding was unbounded (`veilvoice-audio`).**

`io::load` grew its sample buffer with no ceiling. Compressed formats expand: a
mono MP3 at 32 kbit/s decodes to 48 000 `f32` per second, a forty-eight-fold
expansion, so a hundred-megabyte download becomes some five gigabytes of
samples. A failed allocation aborts. **Fixed** with a documented ceiling of
about twelve hours at 48 kHz, refused with a message that names the limit rather
than truncating the recording silently.

**F-18 -- The post-quantum shared secret was not zeroized
(`veilvoice-crypto`).**

`x25519_dalek::SharedSecret` zeroizes itself on drop and the combined input
keying material was wiped explicitly, but the ML-KEM half is a plain
`Array<u8, U32>` and was simply dropped. In a crate whose stated design is that
key material is page-locked and wiped, the post-quantum shared secret was the
one piece left in freed memory. **Fixed** in both `encapsulate` and
`decapsulate`.

**F-19 -- The metadata cleaner could emit a corrupt file (`veilvoice-meta`).**

`clean_wav_bytes` wrote its RIFF size as `(body.len() + 4) as u32`. For a body
at or above 4 GiB that truncates, producing a size field that does not describe
the file -- a WAV handed back to the user as clean that will not open. For a
*metadata cleaner* that is the worst shape of bug: the user believes the file is
safe. Only reachable past what RIFF can express in the first place, so **fixed**
by refusing with `Error::Malformed` rather than by widening anything.

**F-20 -- The app-lock file's KDF costs were validated too late
(`veilvoice-crypto`).** Hardening.

`AppLock::parse` accepted whatever costs the file declared and left them to be
rejected at the first verification. They are attacker-controlled -- this file is
read before anyone has authenticated -- and `KdfParams::checked` is the single
funnel that bounds them (F-2, F-3). **Fixed** by validating at parse, so the
failure is reported as "this lock file is broken", which is true and actionable,
rather than as a password that never works.

**F-21 -- A hand-written manifest reported every file as new
(`veilvoice-guard`).**

`Manifest::of` normalises paths to forward slashes; `Manifest::parse` did not.
A manifest written by hand or by an older build with backslashes therefore keyed
its entries differently from the ones `check`'s `extra` argument is keyed by, so
**every recorded file was also reported as newly added**. A tamper report full
of false positives is one nobody reads, which defeats the only thing the module
does. **Fixed** by normalising on the way in as well as on the way out.

### 2.3 Found and fixed in the third round -- the website

`js/repo.js` fetches README.md over the network and assigns the rendered result
to `innerHTML`. That path is a security boundary and is audited as one.

**F-22 -- The Markdown renderer could freeze the reader's tab. Quadratic
backtracking, measured.**

The link and image patterns were `\(([^)\s]+)[^)]*\)`. Those two runs
**overlap** -- a character that is neither `)` nor whitespace can be taken by
either -- so for a `(` that never closes the engine tries every way of splitting
the text between them:

| `![a](` followed by | time to render |
|---|---|
| 16 000 characters | 0.13 s |
| 32 000 | 0.49 s |
| 64 000 | 1.96 s |
| 128 000 | 7.97 s |

Four times the work for twice the input, exactly. Rendering happens on the main
thread, on text the page **fetched over the network**, so a 400 KB document on
one line is a minute and a half of a frozen tab.

**Fixed** by making the two runs disjoint -- the optional title must begin with
whitespace, so there is only one way to split any input -- rather than by
bounding the document. The grammar accepted is unchanged. 128 000 characters now
renders in under a millisecond.

**F-23 -- A second, independent quadratic in the same file.**

Removing the ambiguity stops the engine trying every split of one attempt. It
does not stop it making a great many attempts: for `[[[[...](](...`, every `[`
is a candidate start and an unbounded `[^\]]+` scans forward from each one
looking for a `]`.

| `[` repeated, then `](` repeated | time |
|---|---|
| 10 000 | 0.23 s |
| 20 000 | 0.90 s |
| 40 000 | 3.61 s |
| 80 000 | 14.59 s |

**Fixed** with repetition bounds -- 512 characters for a link label, 2 048 for a
target, 4 096 for inline code -- which turn each scan into a constant and the
whole pass linear. The limits are far past anything real, and a test asserts
that ordinary Markdown still renders as links, because F-8 was exactly the
mistake of being safe and quietly wrong.

**F-24 -- A deeply nested blockquote crashed the render, and the reader was
told the network had failed.**

A blockquote strips one `>` and calls `render` again, so the **document** chose
the recursion depth. Five thousand `>` characters overflowed the JavaScript
stack and threw a `RangeError`. Because `repo.js` reports any rejection from the
README fetch as *"could not reach api.github.com"*, the reader was given a
confident, wrong explanation for a page that had loaded fine and then broken
while rendering. **Fixed** with a nesting limit of sixteen, past which the
remaining markers are shown as the text they are.

**F-25 -- A code fence could reach `Object.prototype`.**

The fence info string is matched with `\w*`, and both `constructor` and
`__proto__` are `\w*`. On a plain object literal, a fence language of
`constructor` made `KEYWORDS[lang]` resolve through the prototype chain to
`Object`, and `__proto__` to `Object.prototype`.

Neither did any harm as the code stood -- `String.replace` stringifies a
non-regex search value, so it looked for the literal text
`function Object() { [native code] }` and found nothing. That is a description
of a bug that has not gone off, not of a safe lookup. **Fixed** with a
null-prototype object and an own-property check.

**F-26 -- Download links were trusted by omission (`js/repo.js`).**
*The previous round recorded this as open work; it is now closed.*

`link.href = asset.browser_download_url` was assigned with no check, on the
reasoning that GitHub's own API for this repository always returns a github.com
URL. That reasoning is true and it is not a control. **Fixed** by requiring
`https:` and reusing the renderer's own `safeUrl` -- deliberately shared rather
than re-implemented, because two scheme checks on one page is two things to keep
in step and the forgotten one is the one that matters. A refused asset is still
*named*, so the reader learns the file exists; it is simply not clickable.

**F-27 -- A malformed API response was reported as a network failure.**

`assets.sort` called `a.name.localeCompare(b.name)`, which throws if `name` is
not a string; one bad entry rejected the whole promise and the panel told the
reader it could not reach GitHub. The asset list and the README size were also
unbounded. **Fixed**: every field is type-checked, the list is capped at a
hundred entries, and a README above a megabyte is declined with a message
saying so rather than rendered.

**F-28 -- Repo-relative links resolved somewhere other than where they
pointed.**

The rewriting pasted strings together, leaving `..` segments for the browser to
normalise afterwards, so a link written as `../../../elsewhere` produced a URL
that resolved to a different part of github.com than the link appeared to name.
The host was never in doubt, so this is misdirection rather than escape -- but a
page asking people to click through and read the source should send them where
the link says. **Fixed** by resolving through `URL` against a fixed base and
refusing anything that climbs out of it.

**F-29 -- The sticky header had no blur on every iPhone running iOS 17 or
earlier (`css/main.css`).**

`backdrop-filter` was used unprefixed. Safari did not support the unprefixed
property until version 18. **Fixed** with `-webkit-backdrop-filter` alongside.

**F-30 -- The legal gate was an invisible modal on pre-2023 engines.**

`color-mix()` arrived in Chrome 111, Safari 16.2 and Firefox 113. An older
engine discards the whole declaration -- and three of the four uses had no
preceding fallback, so the element was left with **no background at all**. The
worst was `.legal-overlay`: a fixed overlay shown together with
`body.legal-locked { overflow: hidden }`. Without its background the reader got
a page that had silently stopped scrolling, with nothing visible to explain why.
**Fixed**: every `color-mix` now has a plain colour before it.

**F-31 -- No focus ring at all on Safari before 15.4.**

`:focus-visible` was the only focus rule. An unsupported pseudo-class makes the
*entire selector list* invalid, so older Safari dropped the rule and the page
became unnavigable by keyboard. **Fixed** with a plain `:focus` rule as the
fallback and `:focus:not(:focus-visible)` to suppress it for pointer clicks --
kept in separate rules, so one invalid selector cannot take the other down.

**F-32 -- Native controls rendered light on a near-black page.**

No `color-scheme` was declared, so the theme `<select>`'s dropdown list, the
verifier's `<progress>` bar and the file picker were drawn by the platform in
its light styling. It is the one part of the page CSS cannot reach. **Fixed**
per theme, beside the palette it has to agree with, with a test asserting the
declared scheme matches the background's luminance.

**F-33 -- The legal gate could not be dismissed on an iPhone.**

`.legal-box` used `max-height: 88vh`. On iOS Safari `vh` is measured against the
viewport with the browser chrome *collapsed*, which is taller than what is
visible while the URL bar is expanded -- so the bottom of the box, which is
where the **continue** button is, could sit below the visible area. The page
behind is deliberately scroll-locked, so there was no way to reach it. The gate
could not be dismissed, on a phone, on first load, which is when everyone meets
it. **Fixed** with `dvh` and a `vh` fallback. Found by a check written for this
round, not by inspection.

**F-34 -- The sticky header took a fifth of a phone screen, and its links were
too small to hit.**

Nine navigation links at 375 px wrapped onto four rows; because the header is
`position: sticky` that cost **165 px of an 812 px screen at every scroll
position** (measured, not estimated). The links were also 22 px tall, below the
24 px minimum of WCAG 2.5.8 -- and mis-tapping in a nine-item row means landing
on the wrong section rather than on nothing.

**Fixed** with a two-row header on narrow viewports whose navigation is a single
horizontally-scrolling row: **79 px**, down from 165. Anchor targets also gained
`scroll-margin-top`, so following a nav link no longer lands on a heading hidden
underneath the header.

**F-35 -- The in-browser verifier failed with an unusable message on an
insecure origin (`js/verify.js`).**

`crypto.subtle` is only defined in a secure context. Over plain `http://` to a
LAN address -- exactly how somebody serving this folder tests it from a phone --
it is `undefined`, and the code walked into "Cannot read properties of undefined
(reading 'digest')". True, and useless. The published site is HTTPS and
unaffected; this is about not lying to the person who self-hosts. **Fixed** with
a feature check and a sentence explaining what to do.

**F-36 -- The verifier used twice the memory it needed (`js/verify.js`).**

Chunks were collected into an array and then copied into a second buffer, so
peak memory was twice the file size before WebCrypto was handed anything -- and
a release archive is tens of megabytes. On a phone that is the difference
between checking a download and having the tab killed. **Fixed** by allocating
the destination once and writing each chunk into it, and by refusing with a
clear message, naming `sha256sum` and `certutil`, if the buffer cannot be
allocated at all.

### 2.4 Found and fixed in this round -- the fourth (F-37 to F-46)

This round audited what v0.1.9 adds: a search index over the whole repository,
a portable release verifier, install scripts, packaging definitions, and an
animated banner. Ten defects, and they divide into two groups that are worth
keeping apart rather than counting together.

**Three were already shipped** (F-37, F-41, F-42) -- they were live on the
published site or in the published repository when this round began.

**Seven were in code written during this round** (F-38 to F-40, F-43 to F-46),
caught before any of it was released. Recorded anyway, at the same length,
because a defect found on the way to shipping is evidence about how the work is
being done, and quietly omitting them would make this round look cleaner than
it was. Two of them are the same mistake this document has already recorded
twice.

**F-37 -- The website destroyed the text in its own banner. Shipped, on every
viewport, and only visible by looking.**

Every image on the site carried `image-rendering: pixelated`. That property is
for pixel art being *enlarged*, where it keeps edges crisp. Every image on this
site is *shrunk*: `banner.png` is 1280 px wide and is never drawn wider than
860, and on a 375 px phone it is drawn at about 343 -- a 3.7x reduction.
Nearest-neighbour sampling does not blend the rows it discards, it keeps roughly
one in four and deletes the rest.

The banner's own text is drawn in one- and two-pixel strokes, so more than half
of it disappeared. Measured by rendering the page and reading it:

| Intended | As published, at 375 px |
|---|---|
| `THE VOICEPRINT IS DESTROYED. THE WORDS STAY READABLE.` | `TIE VOICEPRIN IS DESTOYED. THE WORDS STAY EDGRE.` |
| `BY TILAS01 ON GITHUB` | `BY ~I,FS01 CN GITHUB` |
| `SECURE AUDITED RUST CODE` | sliced horizontally, half the strokes gone |

The first thing a phone reader saw was this project's own headline claims,
illegible -- on a site whose entire argument is that you can go and read things
for yourself. Not a security defect. It is the same *family* as F-7, where the
site displayed source that was not the source: a page that undermines its own
argument by being quietly wrong about what it shows.

**Fixed** by removing the property from all four places it appeared. Confirmed
by a side-by-side render at 343 px before and after.

**Found by rendering the page**, which is the only way it could have been
found: every unit test passed throughout, and the geometry checks that exist
measure layout, not legibility. This is the third revision of this document to
have to record that looking at the page found something no test did.

**F-38 -- The search index covered about an eighth of what it claimed.**

Sections were **truncated** to 240 characters rather than split, so for any file
longer than a paragraph most of the text was simply absent from the index --
while the search page said it searched every file. A search box that silently
does not look at most of the corpus answers "no results" with exactly the same
confidence as one that does, which makes it worse than having none.

Found by a test asking whether searching for `onerror` -- a string this
repository certainly contains, in its own hostile-markup fixtures -- returned
anything. It did not.

**Fixed** by splitting rather than truncating: every character of every indexed
file now lands in exactly one chunk, and the 240-character bound applies to how
much a single *result displays*, not to how much is searched. Coverage is also
now stated precisely rather than rounded up -- prose, the website, the build
files and the licences are complete; Rust is indexed by item name and doc
comment, not by function body, and both the generator and the page say so.

**F-39 -- The search index indexed itself, and could never converge.**

The generator's output is tracked and lives under `website/`, so it was walked
like any other file. Each run's input therefore contained the previous run's
output: the file grew on every regeneration and `--check` could never agree
with a freshly built one.

The failure mode is what makes this worth writing up. It does not look like a
bug, it looks like flaky CI -- a check that fails, passes after a regenerate,
and fails again next time. **Fixed** by excluding the generated paths, and
asserted by a test so it cannot come back as a mystery.

**F-40 -- The no-JavaScript search page hid its own content from find-in-page.**

The static index is the whole reason `website/nojs/` remains a supported
edition: the entire corpus is in the page so that the reader's own find-in-page
searches it. Every entry was inside a **collapsed** `<details>`.

Text inside a closed `<details>` is only searchable on engines that auto-expand
it -- Chromium since 102, later elsewhere. On an older Safari or Firefox the
page would have answered every search with nothing, while looking perfectly
fine. That is the exact shape of F-30, F-31 and F-33, and worse than any of
them, because the page would have been *confidently* empty rather than visibly
broken.

**Fixed** by expanding every entry, with a contents list to keep a long page
navigable. Height is free; correctness on browsers a great many people run is
not.

**F-41 -- The website's artwork had drifted from its generator, and nothing
checked it.**

`website/assets/` holds its own copies of the icons and banner, and they were
kept in step by hand. `assets/generate.py --check` only ever looked at
`assets/`. So the copy the site actually serves could differ from the script
that is supposed to produce it, with nothing to notice -- and it did, the moment
the banner changed.

For a project whose stated position is that the artwork is generated rather
than a committed blob, "generated, and also a second copy somebody maintains by
hand" is a materially weaker claim than the one being made. **Fixed**: the
generator writes both copies and `--check` verifies both.

**F-42 -- A documentation link pointed at a file that does not exist.**

`docs/INSTALL.md` shipped a link to `REPRODUCIBLE-BUILDS.md`. The file is called
`REPRODUCIBLE_BUILDS.md`. One character, written by somebody who had just read
the real filename.

A dead link is not cosmetic here. The argument this project makes is "go and
read it yourself", and a link that goes nowhere is that argument failing
quietly. **Fixed**, and made mechanical: a tenth site-test suite resolves every
local link in every tracked `.md` and `.html` file.

The first version of that checker reported two faults and both were the checker:
`docs/AUDIT.md` quotes a README link to explain how it once rendered wrongly,
and quotes `src="a&quot;onerror=x"` to explain why a naive scanner calls that an
attack -- so the scanner called them broken links. Documentation describing a
bug correctly, flagged as a bug. **That is section 4.4 of this document
happening again**, where five of six "findings" were the checker rather than the
code. Quoted code is now stripped before extraction, and the suite is verified
to still detect a planted broken link.

**F-43 -- A byte-order mark was written into a shipped script.**

`Set-Content -Encoding utf8` on Windows PowerShell 5.1 writes a BOM, and one
landed at the head of `install/install.ps1`. Caught by
`tools/site-tests/characters.test.js`, which is exactly what that suite exists
for -- an invisible character causing an invisible fault. Recorded not because
it was hard to fix but because the guard earned its place again, and because
the same command will do the same thing to the next person who uses it.

**F-44 -- The Windows installer could not use the GnuPG most Windows machines
already have.**

Git for Windows bundles an MSYS build of GnuPG, which a great many people have
without knowing it. Given a Windows path it treats the whole thing as a
*relative POSIX* path and resolves it against the working directory, producing
`/c/current/dir/C:\Users\...` and failing with "directory does not exist".

Found by running the installer rather than reading it. **Fixed** by translating
paths to `/c/Users/...` form when the resolved GnuPG is an MSYS build, which
turns "install Gpg4win first" into "verified" for a large share of Windows
users.

**F-45 -- Then its agent would not start, and the error blamed the key.**

With the paths fixed, the import failed with `gpg: error running
'/usr/bin/gpg-agent': exit status 2`. GnuPG's agent puts a Unix-domain socket
inside the home directory it is given, and such a path cannot exceed about 108
bytes. A GUID-named temporary directory produced a 90-character home directory,
and the agent could not start.

**Measured rather than guessed: a 37-character home directory worked, 90 did
not.** The user-visible symptom was "the downloaded public key could not be
imported" -- true, and completely misleading, which is the failure mode this
project keeps writing up (F-16 told somebody worried about surveillance the
wrong reason). **Fixed** by keeping the temporary directory short, and by
refusing with an explanation in the remaining case rather than leaving a puzzle.

**F-46 -- Progress output on stderr terminated the Windows installer.**

Under `$ErrorActionPreference = "Stop"`, Windows PowerShell 5.1 turns anything
a native program writes to stderr into a terminating error, whatever its exit
code. GnuPG reports progress on stderr as a matter of course, so the script
died on a successful verification. **Fixed** by relaxing the preference around
native calls and judging them on their exit code, which is the thing that
actually says whether they worked.

**F-47 -- The page checks applied to a list of pages, not to every page.
Found after v0.1.9 was tagged.**

`tools/site-tests/html.test.js` opens with a comment saying it checks "the
signing-key fingerprint on every page". It checked a hardcoded list of three
files. `search.html` was added to the site in this release and was therefore
checked for **nothing**: not balanced tags, not duplicate ids, not dangling
anchors, not third-party assets, not inline event handlers, and not the
fingerprint -- which it did not have.

The fingerprint is the one thing on these pages that lets a reader tell a real
release from a forged one, and the page that shipped without it is the one
about finding things in this project.

**This is section 4.5 of this document happening to the tests themselves.** The
lesson recorded there -- *a finished scope is only as wide as the list it was
drawn from* -- was written about audit scope, and the same failure was sitting
in the test that enforces it. Enumerating from memory is the defect; enumerating
from the directory is the fix.

**Fixed** by discovering every `.html` file under `website/` rather than listing
them, which immediately found the missing fingerprint and now covers any page
added in future the moment it exists. Five pages checked, up from three.

Found while verifying the published release, by asking whether the live search
page carried the fingerprint -- a check made because the release documentation claims CI
enforces it. It said so, and it did not.

### 2.5 Found and fixed in the fifth round (F-48 to F-59)

This round covers what has been added since v0.1.9: a documentation generator
that writes 366 files including HTML this site publishes, a parser for
user-written colour palettes, a headless-browser driver, a cycling strip of
claims on the front page, and a licence change touching 352 files.

**Twelve defects. Two had shipped** (F-48 and F-49, both live on the published
site); ten were caught in code written during the round. That distinction is
kept because a round counting just-written-and-immediately-fixed defects
alongside ones a reader could have hit is flattering itself.

**F-48 -- the repository panel rendered a README's own markup as text. Shipped.**

The front page fetches this project's README and renders it. The README opened
with a centred banner -- `<p align="center"><picture>...</picture></p>`, which
is GitHub's own idiom -- and the panel displayed *the source of those tags*, as
a paragraph of escaped tag soup, immediately above the word VeilVoice.

Neither half was broken. `markdown.js` escapes raw HTML on purpose: that is the
property which makes its output safe to hand to `innerHTML`, and it has been
through two rounds of hostile-input auditing. A README is entitled to contain
presentational markup. The two correct behaviours met and produced garbage at
the top of the page, live, for as long as the panel and that README coexisted.

**Fixed** by stripping block-level HTML from the fetched Markdown *before*
rendering -- the renderer keeps escaping everything, unchanged -- and by
switching the README to a plain Markdown image. Fenced code is left alone, so a
document showing markup as an example still shows it. Regression test added,
and confirmed to fail without the fix.

The lesson is F-37's again in a different medium: this was not findable from
the tests, and was obvious the moment somebody looked at the page.

**F-49 -- the default theme's secondary text failed WCAG contrast. Shipped.**

Found by writing a contrast check for *other people's* palettes (see F-51) and
then pointing it at this project's own themes:

| Theme | `muted` on `bg` | Required |
|---|---:|---:|
| tokyo-night | 2.76:1 | 3.0:1 |
| solarized | 2.79:1 | 3.0:1 |

Below the floor for text of any size. `--muted` is not decoration here: it
carries the hero tagline, the figure captions, the scope notes stating what the
app lock is *not*, and the licence line inside the banner image. Tokyo Night is
the default, so this was the shipped appearance of the website, the desktop
app, the command line and the artwork.

**Fixed** with each palette's own upstream colour rather than an invented one --
Tokyo Night's `dark5` (`#737aa2`, 4.10:1) and Solarized's `base00` (`#657b83`,
3.37:1) -- so both themes stay recognisably themselves. Propagated through
`themes.css`, the app's theme table, the CLI's escape codes, the no-JavaScript
edition and `assets/generate.py`, and the artwork regenerated and looked at.

Lowering the threshold until the existing colours passed would have been
fitting the rule to the defect. That is the move this document keeps catching
in other people's reasoning and it would have been no better here.

**F-50 -- the documentation generator overwrote a hand-written file.**

Adding `fuzz/` to the documented crates made the generator write
`fuzz/README.md` -- over a hand-written one explaining how to run the targets
and recording that they have **not been run to convergence**, a sentence
section 5.2 of this document cites by name.

Nothing failed. The generator reported success, `--check` compared its output
against the file it had just written and agreed, and the honesty note was gone.
That is finding F-41 running in reverse: generated output silently replacing
the thing it was meant to describe.

**Fixed** by refusing to overwrite any file that does not carry the generator's
own marker, naming every one, and writing nothing at all until every
destination has been checked -- so a refusal leaves the tree as it was rather
than half regenerated. Generated banners and wiki pages now carry the marker
too, which they should have anyway.

**F-51 -- the theme drift test only ran in one direction.**

The app's themes are asserted against `website/css/themes.css` so the two
front-ends cannot drift. The test walked the app's `THEMES` and checked each
against the stylesheet -- which catches a colour changed on either side, and
**misses a theme added to the website**, because nothing walked the stylesheet
looking for entries the app had never heard of.

Same shape as the hardcoded page list in `html.test.js` that let `search.html`
ship unchecked: a check enumerating from a list is only ever as wide as the
list. **Fixed** by enumerating from the stylesheet as well. Verified by adding
a fake theme to the CSS and watching the test fail.

**F-52 -- generated pages rendered links without a scheme check.**

`inline_html` escapes everything before inserting markup, so `<script>` in a
doc comment comes out as text. What it did not do is look at what a link points
at:

```
[click](javascript:alert(1))  ->  <a href="javascript:alert(1">click</a>
```

`website/js/markdown.js` has had `safeUrl` for this since the third round, and
`website/js/repo.js` states in a comment that the rule is "the renderer's own
`safeUrl`, deliberately shared rather than copied". The 366 generated reference
pages were the one part of this site rendering links outside that rule.

Only the maintainer writes these doc comments today, so this was not reachable
by an outsider. **That is a fact about who has commit access, not a property of
the code**, and this document's standard is explicitly about the code. **Fixed**
by copying the rule exactly -- not improving on it, because two subtly different
link rules on one site is worse than one strict rule in two places.

**F-53 -- a palette file that is not a file hung the application at startup.**

`veilvoice-gui` reads user-written `.palette` files during startup. The size
bound was applied by calling `metadata()` on the path and then
`read_to_string()`. A FIFO reports a length of zero, sails past the size check,
and blocks the read for ever.

This runs **before the window is created**, so the symptom is an application
that launches and never appears: no error, nothing on screen, and nothing
pointing at the palettes directory. The same two lines were also a TOCTOU --
two operations on a path, with the bound applied to what the file *was* rather
than to what gets read.

**Fixed** by bounding the read instead: open the handle, ask the *handle*
whether it is a regular file, and take at most `MAX_BYTES + 1` bytes so a file
that grows between the check and the read cannot get past it. Same shape as the
release verifier streaming an archive through SHA-256 in fixed chunks.

**F-54 -- duplicate anchor ids in generated HTML.**

`veilvoice-meta/src/lib.rs` has a doc-comment heading called "Items", and so
does the generated section listing a file's items. Both slugged to `items`, so
the browser jumped to whichever came first and the table of contents silently
sent the reader to the wrong place. Caught by `html.test.js`, which is exactly
the class of hand-written-HTML mistake it exists for. **Fixed** by allocating
every anchor on a page from one place, in document order, using GitHub's own
suffix rule so the Markdown and HTML renderings agree about where a link goes.

**F-55 -- the Mermaid theme directive was emitted with its markers halved.**

The init directive `%%{init: ...}%%` is built with `%`-formatting, which turns
`%%` into `%`. Every one of 366 diagrams shipped with `%{init: ...}%`, which
Mermaid does not recognise -- so the Tokyo Night theme was silently ignored and
every diagram on GitHub rendered in default colours.

**F-56 -- derived call graphs contained calls that do not happen.**

The per-file flowchart draws the calls between functions in a file, found by
asking whether the callee's name appears followed by `(`. That drew an edge
from `SpectralState::transform` to `SpectralState::new` because the body
constructs a `Complex::new(..)`.

An edge that is not a call is worse than a missing one: these diagrams are
offered as *derived from the source*, so a reader has no reason to doubt one.
**Fixed** by inspecting the qualifier before the name -- nothing, `self.`,
`Self::` or the owning type is a call; anything else is somebody else's
function that happens to share a name, which in Rust is most of `new`, `len`,
`from` and `default`.

**F-57 -- `pub(crate)` was reported as `pub`.**

The item tables and the crate-level "public items" list treated any visibility
modifier as public, so crate-internal helpers appeared in the published API
surface of nine crates. A reader deciding what they may depend on was being
shown items that are not there.

**F-58 -- adding harmonics to the banner cost a fifth of its height.**

Three sinusoids whose amplitudes sum to 1.0 only reach 1.0 if they all peak
together, and the phase offsets exist precisely so they do not. The waveform's
peak fell from 62 pixels to 49. The animation still worked, still looped, and
simply looked smaller; no test could have noticed. **Fixed** by measuring the
normalisation constant from the same expressions the function uses, at import,
so editing an amplitude re-measures rather than leaving a stale number behind.

**F-59 -- a component whose resting state is invisible, under a blanket
reduced-motion rule.**

The cycling strip of project facts animates from `opacity: 0`. The global rule
at the top of `main.css` collapses every animation to 0.01ms for a reader who
has asked for less motion -- which would have ended every message at its final
keyframe, opacity 0, leaving the strip permanently blank with nothing to
indicate anything was missing.

Caught before shipping by rendering the page with the preference emulated
rather than by reasoning about it. **Fixed** explicitly: the cycle stops and the
first fact stays. A blanket rule that neutralises motion has to be checked
against components whose resting state is invisible, and that is now written
down here because it will not be the last one.

---

### 2.6 Found and fixed in the sixth round (F-60)

A short round, covering the code added after v0.1.11: the release fetcher, the
installer, and the crash log.

**One defect, and it was in the function whose own comment describes it.**

**F-60 -- the installer could replace a user's entire `PATH` with one entry.**

`veilvoice install` appends its directory to `HKCU\Environment\PATH`. It reads
the existing value first and appends, precisely so that it never writes a
`PATH` it did not read -- the module's documentation says so at the top, in
those words.

`read_user_path` returned `Ok(String::new())` on **two different outcomes**:
the value genuinely not existing, and `reg query` failing for any other reason.
`add_to_path` treats an empty result as "there is no `PATH` yet" and writes a
fresh value containing only VeilVoice's directory.

So on a machine where that query failed -- a transient error, an unexpected
locale, a policy restriction -- installing VeilVoice would have replaced the
user's entire user `PATH` with a single entry. There is no undo, the damage is
silent until something stops working, and `uninstall` would have made it worse
by removing that entry and leaving the value empty.

**Nothing was wrong with the reasoning; the code did not implement it.** The
comment describing the danger and the function creating it were forty lines
apart. That is the shape this document keeps recording: the check that is
believed rather than enforced.

**Fixed** by refusing to conflate "absent" with "unreadable".
`read_user_path` now returns `Value` or `Absent`, and everything else is an
error that refuses the write and says why. Only a value that was definitely
read, or definitely absent, permits a change. Verified by round-tripping a real
install: six entries, seven, six, and the one that left was the one that
arrived.

Found by re-reading code written hours earlier, against the classes rather than
against a list -- which is the only technique in this document that has worked
every time.

### 2.6 The new code, audited against the classes

**Untrusted input.** Two new parsers. `palettes.rs` reads files a user writes
by hand: every token validated, every colour required to be a full `#rrggbb`,
every problem reported rather than the first, and nothing filled in from the
default theme -- a palette that is *mostly* yours with a few colours from
somewhere else and no indication which is worse than one that refuses. The id
is constrained to what survives a round trip through the preferences file, and
may not collide with a built-in theme's.

**Allocation sized by untrusted input.** The palette loader bounds the number
of files (40), the bytes read from each (16 KiB, bounded at the read rather
than by a stat -- F-53), and holds no file in memory beyond that. The
documentation generator reads only tracked source files.

**Panics reachable from input.** `palettes.rs` has no `unwrap` on user data;
the one `unwrap_or` is unreachable by construction and falls back to a colour
rather than panicking, deliberately, because it sits on a path that leads to a
paint loop.

**Rendering untrusted text into a page.** Covered by F-52. Everything reaches
the generated pages through an escape-first path, and links now go through the
same scheme rule as the rest of the site.

**Resource exhaustion.** The generated diagrams are bounded at 22 nodes; past
that the item table is the better answer and the page says the diagram was
bounded rather than silently showing a subset. No regular expression in the
generator is built from file content.

**Concurrency.** The theme table is a `OnceLock` read through one relaxed
atomic load. No lock was added to the paint path; custom palette strings are
leaked once at startup, bounded, and the reasoning is written where it is done.

**Error handling that degrades quietly.** The class this round was most alert
to, and where F-50 and F-53 both landed. The generator now refuses rather than
overwrites; the palette loader refuses rather than reads; the fact strip fails
visible rather than blank.

**Dependency risk.** No dependency was added. `tools/render/shot.py` implements
a WebSocket client in sixty lines of standard library rather than taking a
package, on the same reasoning as the rest of `tools/`: a repository whose
argument is that its supply chain can be read should not add to it in order to
take a screenshot.

**Cryptography.** Untouched this round. No primitive, parameter or construction
changed.

**Nothing further found in these classes.** That is a result and is recorded as
one.

### 2.7 The v0.1.9 code, audited against the classes

The standard at the top of this document is a walk of every vulnerability class
across the whole tree. Applied to what this round adds:

**Untrusted input rendered into a page.** `search.js` reads an index built from
*every tracked file*, which in this repository includes the hostile-markup
fixtures -- `<script>`, `onerror=` and the rest are in `search-index.json`
today, as ordinary text. A single `innerHTML` would turn this project's own
test corpus into its payload. Every value reaches the page through
`textContent` or `createTextNode`, and match highlighting builds `<mark>`
elements rather than strings. Asserted by tests that ask the **tree** whether an
element is a script, not whether a string contains one -- the section 4.4
distinction, applied deliberately this time.

**Resource exhaustion in the renderer.** F-22 and F-23 were quadratic blow-ups
on text fetched over the network, and the same class applies here. The query is
bounded to 128 characters and 8 terms, scoring is a linear pass with `indexOf`,
results are capped at 200 scored and 60 rendered, and **no regular expression is
ever built from user input**, so there is no pattern for a query to blow up.
Measured: a 5,000-character query renders promptly.

**Allocation sized by untrusted input.** The verifier streams files through
SHA-256 in 64 KiB chunks rather than reading them whole; a release archive is
tens of megabytes. This is F-36 in the other direction, avoided on purpose.

**Dependency and environment trust.** The install scripts resolve GnuPG to
absolute, enumerated paths rather than by bare name. Resolving `gpg` through
Windows' search order would include the current working directory, so running
the installer from a folder containing a file called `gpg.exe` would run that
instead -- **F-13, in the one place where it matters most**, since this is the
program that decides whether the download is genuine.

**Order of verification.** Every script and the verifier check the signature
over `SHA256SUMS` *before* comparing any file against it, and refuse if it
fails. Checking the hash first would prove only that a download matches a list
that might itself have been replaced. There is no flag anywhere to skip
verification: an installer with one is an installer whose verification is
decorative.

**Error handling that degrades to a weaker posture.** The one class this round
was most alert to, because an installer is full of opportunities for it.
Without GnuPG the scripts stop rather than falling back to "the hash matched" --
a hash checked against an unverified list is not a security check. An unsigned
release is refused outright. `--sums` without `--sig` is refused. The verifier
refuses to run a signed-list check and a typed-hash check at once and report a
single answer, because they prove different things.

**Nothing found in these classes** beyond the defects listed above. That is a
result and is recorded as one.

### 2.8 Accepted, with reasoning

**A-1 — Remaining `expect()` sites (6).** Each was reviewed:

| Site | Assessment |
|---|---|
| `stft.rs` ×2 — FFT `expect` | Infallible given correct buffer sizes, which the engine owns and asserts on construction. A failure here is a programming error, not a runtime condition. |
| `hybrid.rs` — `OsRng::fill_bytes` | `rand_core::RngCore::fill_bytes` has no error return. Panicking on CSPRNG failure is the only option and is what every RustCrypto consumer does. Continuing with weak randomness would be far worse. |
| `hybrid.rs` — `NonZeroU32::new(1).unwrap()` | Compile-time constant, cannot fail. |
| `wav.rs` ×2 — `try_into().expect("4 bytes")` | Slices of statically known length 4, guarded by an explicit `pos + 8 <= end` bounds check immediately above. |

**A-2 — `paste` and `ttf-parser` unmaintained advisories.** Neither is a
vulnerability. `paste` is a compile-time proc macro that emits no runtime code.
`ttf-parser` does parse untrusted input, but the only fonts loaded are egui's
own embedded face and, optionally, a JetBrains Mono the user already installed
system-wide — no attacker-supplied font reaches it. Reviewed each release.

**A-3 — FreeBSD builds are not reproducibility-verified.** Built once in a VM
rather than twice in separate directories. Reported as `not-verified` in the
release notes rather than claimed.

**A-4 — `veilvoice-watch` on Linux sees only your own processes.** `/proc/<pid>/fd`
is readable by the owner and root. This is a kernel permission boundary, and
`support()` states it rather than letting an empty list imply an empty machine.

---

**A-6 -- `rsa` carries RUSTSEC-2023-0071, and it is accepted on a narrow
ground.**

The portable verifier uses rPGP so that checking a release needs no GnuPG. That
brings in the `rsa` crate, which carries the Marvin attack advisory -- key
recovery through a timing side channel, medium severity, **no fixed version
available**.

This is a *vulnerability*, not an unmaintained notice, and `.cargo/audit.toml`
previously stated that vulnerabilities always fail the build. That rule is being
weakened, so it is written out here and in that file rather than folded into the
list of unmaintained advisories where nobody would re-read it.

The ground: the advisory concerns operations performed with an RSA **private**
key -- that is what "key recovery" means, and a timing oracle needs a secret to
leak. `veilvoice-verify` performs signature *verification* and nothing else. It
holds a public key compiled into the binary; there is no private key anywhere in
this repository or in any released artefact, and no code path in the workspace
signs or decrypts with RSA. VeilVoice's own cryptography is X25519 + ML-KEM-768,
XChaCha20-Poly1305 and Argon2id.

That is an argument about how the crate is **used**, not about the crate, so it
is enforced rather than believed: a CI job fails the build if a secret-key or
decryption API appears in the verifier. If the argument stops being true, the
build stops with it, rather than this paragraph quietly ageing.

**The dependency itself was weighed rather than waved through.** rPGP is by far
the largest dependency in this project: 155 packages, every licence permissive,
no networking crate. The alternative was hand-writing OpenPGP packet parsing and
RSA PKCS#1 v1.5 verification, where a subtle mistake is a **silent accept** in
the one tool whose entire job is not to silently accept. A widely used
implementation that many people read is the better risk, and the size of it is
the price.

## 3. Cryptography

### 3.1 Primitives

| Purpose | Choice | Assessment |
|---|---|---|
| Password → key | Argon2id, RFC 9106 profile (256 MiB, t=3, p=4) | Current best practice. Memory-hard. Cost parameters travel with the file, so old files open after defaults rise. |
| Public-key | X25519 + ML-KEM-768 **hybrid** | Correct construction. Breaking it requires breaking *both*. |
| Payload | XChaCha20-Poly1305 | 192-bit nonce is randomly generated; collision risk negligible, and it removes the counter-management failure mode that sinks RFC 8439 ChaCha20 deployments. |
| KEM combiner | HKDF-SHA256 over both secrets **plus the full transcript** | Correct. Binding both ciphertexts and the recipient key prevents an attacker who substitutes one half from steering the derived key. |
| Header integrity | Authenticated as AEAD associated data | Prevents KDF-cost downgrade. Verified by test. |
| Modulation stream | ChaCha20, OS-seeded, ratcheted every 2 s | Forward-secure: ChaCha20 is not invertible, so a compromised current state cannot recover earlier segments. |

### 3.2 The app lock

Reviewed as it was written, and the review is short because the design refuses
to be clever:

| Property | Assessment |
|---|---|
| Verifier, not a key | Correct choice. It encrypts nothing because there is nothing local it could usefully encrypt, and a key that protected nothing would only invite the belief that it did. |
| Domain separation | `Argon2id("veilvoice/app-lock/v1\0" ‖ password, salt)`. Asserted by test to differ from a container key over the same passphrase and salt. |
| Comparison | Constant time, through `Secret`'s `subtle::ConstantTimeEq`. |
| Rate limit | Three free attempts, then doubling from 5 s to a 15-minute cap. Persisted after every attempt, so a process restart does not reset it. The shift is guarded against overflow and asserted at `u32::MAX`. |
| Clock handling | A clock that moves backwards yields no credit against the wait rather than a negative elapsed time. |
| Lock file | Unauthenticated **on purpose**. Any key that could authenticate it would have to sit beside it in the same file; a MAC would look like tamper-proofing without being any. Parse errors are errors, never "no lock". |
| Failure modes | A wrong password on `remove` reopens the store so the recorded failure is not lost with it. |

**The honest limit, recorded here as clearly as in the UI:** this defends
against casual access, not against an attacker with the disk. The file can be
deleted, the counter edited, the clock moved, and the hash attacked offline.
`lock::SCOPE` states all four, is shown on the unlock screen itself, and a test
fails the build if it is ever softened into a boast.

### 3.3 Encryption at rest, by default

The `anonymise` path now encodes the WAV in memory and seals it there, so a
recording that is going to be encrypted never lands on disk in the clear. That
closes a real hole rather than a theoretical one: on flash storage, a plaintext
file that is written and then deleted is not recoverable *by the user* and may
well still be recoverable by someone else.

Both front-ends refuse to start a job with encryption on and no passphrase or
recipient key set, rather than quietly falling back to plaintext. The GUI's
`Plan::Missing` exists solely to make that fallback impossible to write by
accident, and is tested.

Opting out is preserved and gated behind a warning that has to be answered.
Tests assert both that the default is on and that the warning still contains the
uncomfortable sentences.

### 3.4 Post-quantum posture

**Sound.** ML-KEM-768 (FIPS 203) is NIST-standardised and targets category 3.
The hybrid is the right call: ML-KEM is young, lattice schemes have had
implementation breaks, and a pure-PQ deployment would be a single point of
failure. The harvest-now-decrypt-later threat is real for recordings, which is
precisely why this is not deferred.

**Honest gap:** the *signature* on releases is RSA-4096, which is **not**
post-quantum. This is deliberate and low-risk — a signature only needs to resist
forgery until the release is superseded, and no PQ signature scheme has the
verifier tooling to make it practical today. It is recorded here so nobody
mistakes "post-quantum encryption" for "post-quantum everything".

### 3.5 Key handling

Page-locked out of swap, zeroized on drop, constant-time comparison, `Debug`
redacted. Each `Secret` owns whole pages exclusively — a bug found and fixed
earlier, where two secrets sharing a page meant dropping one unlocked the
other's memory.

`Secret::is_locked()` reports whether locking actually succeeded rather than
assuming. Locking does not survive hibernation, which is stated in the docs.

**A-5 — Typed passphrases exist as ordinary bytes while being typed.**
*Narrowed since first recorded; the residue is accepted.*

An egui text field owns a `String` and `rpassword` returns one, so a passphrase
is ordinary heap memory for as long as something is receiving keystrokes. That
window cannot be removed without a custom text widget, which would be far more
code and far less reviewed than the gap it closes.

What **was** fixed is the part that had no excuse: the GUI used to keep the
confirmed session passphrase as a plain `String` for the *entire session*, and
the CLI passed `Vec<u8>` and `String` copies around after the prompt. Both now
move the passphrase into a page-locked, zeroizing `Secret` the moment it is
confirmed, and wipe the buffer it came from. The window went from "until the app
closes" to "while the user is typing".

The remainder is accepted and stated rather than papered over: for those
moments the bytes are swappable, and none of this helps against an attacker who
can read the process's memory — who has already won, as `WHITEPAPER.md` §7
says.

---

## 4. The work that was outstanding, and what it found

The previous revision listed five items. Four are now done; the fifth cannot be
done from inside the project.

### 4.1 Adversarial read of `spectral.rs` and `accent.rs` — **done**

Read end to end against the irreversibility claim.

**The central claim holds.** `transform` reads `c.norm()` and nothing else from
the incoming spectrum, and every bin of `spec` is overwritten before it returns —
zeroed then rewritten on the comb path, assigned outright on the channel-vocoder
path. There is no branch on which measured phase survives, and none on which a
bin retains a previous frame's value.

Checked in detail, and correct:

- **`box_smooth`'s edge arithmetic.** The running sum seeds with `radius`
  copies of `src[0]` and slides with clamped indices; expanding the window by
  hand at `i = 0` and `i = 1` gives exactly the sums the code produces.
- **The comb's energy normalisation.** `amp` is set so the sum of squares over
  the comb lines equals the original frame's, so replacing the excitation is
  level-neutral rather than approximately so.
- **Every accent correction is bounded and long-term.** Prosody is clamped to
  `[0.5, 2.0]`, VTLN to `[0.72, 1.40]`, and the tilt curve to ±12 dB; the time
  constants are 3 s (VTLN) and 2 s (LTAS), which is the mechanism behind the
  "never derived from the current frame" claim that keeps vowels intact.
- **`log_centroid` refuses to produce a value it cannot stand behind**,
  returning `None` on an empty band or a non-finite result rather than a number.

One defect found: **F-5**, above. Two observations kept rather than fixed:

- `resample_linear` silently substitutes a ratio of 1.0 for a non-finite one,
  which would pass the envelope through unwarped — a *weaker* transform arriving
  quietly. With F-5 fixed and the accent ratios clamped, no input can reach it,
  so it is now defence in depth rather than a live path. Left as it is, and
  written down here so it is a known belt rather than a forgotten one.
- With accent neutralisation switched off, voiced frames take the
  channel-vocoder path and pitch is *randomised* rather than *normalised* —
  strictly weaker, as `WHITEPAPER.md` §3.2 already says. Consistent, not a
  defect.

### 4.2 Parser campaigns — **done**, and they found F-2, F-3 and F-4

`crates/veilvoice-crypto/tests/parser_fuzz.rs` and
`crates/veilvoice-meta/tests/wav_fuzz.rs`. Deterministic seeded PRNG with
structure-aware mutation — bit flips, truncation, splices, and 32-bit fields
replaced with `u32::MAX`, `i32::MAX`, 0 and 1 — plus every length around each
boundary walked exhaustively.

Properties asserted, not just "it did not crash": a parsed header must
re-serialise to exactly the bytes it came from, reported offsets must lie inside
the buffer, and a cleaned WAV must itself be a valid WAV whose size field matches
what was written.

Run at **1,000,000 rounds per target in debug** (overflow checks *on* — the
release profile disables them, and an overflow was one of the bugs) and
2,000,000 in release. Clean after the fixes. CI runs 1,000,000 per target.

**This is not `cargo fuzz` and is not claimed to be.** It is not
coverage-guided, so it explores by construction rather than by feedback. It was
chosen because `cargo fuzz` needs nightly and libFuzzer, and a check that only
one person can run is a check that stops being run. A coverage-guided campaign
remains worth doing — see §5.

### 4.3 Timing analysis — **done**, no leak found

`crates/veilvoice-crypto/tests/timing.rs`, `#[ignore]`d by default because a
timing test on a shared runner measures the neighbours. Measured on an idle
Windows desktop, release build, 2,000 samples per case, reporting the
**minimum** — timing noise is one-sided, so the fastest sample is the closest
estimate of the work actually done.

| Comparison | Ratio |
|---|---|
| Container open: wrong at the first byte vs wrong at the last | **0.996** |
| Container open: wrong vs right password | **0.996** |
| App lock: wrong vs right password | **1.004** |
| App lock: one-byte vs full-length password | **1.004** |

Within half a percent. **No prefix correlation**, which is the property that
matters: an early-exit comparison turns the clock into a character-by-character
oracle and would show as a large factor, not a fraction of a percent.

The rate-limited path is a deliberate exception. It returns before touching the
KDF and is therefore **at least 23,600× cheaper** than a real attempt. That is
the point of a rate limit — refusing to spend the CPU — and it leaks only the
state the unlock screen displays on its face anyway.

*(A first pass reported a 1.49 ratio for wrong-vs-right. That was the harness,
not the code: it used medians on a noisy machine and, for the app lock, timed
two derivations against one. Recorded because a timing result that is not
reproducible is not a result.)*

### 4.4 Website JavaScript review — **done**, and it found F-6, F-7 and F-8

`js/repo.js` fetches README.md over the network and assigns the rendered output
to `innerHTML`. Everything on that path rests on one claim in `js/markdown.js`:
escape first, emit only your own tags.

`tools/site-tests/` now tests it: 39 hand-written hostile documents, each aimed
at a specific escape route, plus a randomised campaign run to **200,000
generated documents**. The check is an **allowlist** — output may contain only
tags and attributes the renderer is supposed to produce — because a blocklist of
"no `<script>`" passes anything that finds another door.

The check parses attributes the way a browser does. An early version did not,
and reported six findings of which five were false: `&lt;script&gt;` in the
output is the renderer working, and `src="a&quot;onerror=x"` is one attribute
whose value contains a quote, because entity references are decoded *after* the
value is delimited. A naive scan calls both of those attacks and buries the real
one in the noise.

After the fixes: clean at 200,000 rounds. The suite also covers rendering
correctness (F-7 was a *correctness* bug with no security dimension, and would
never have been caught by a security-only test), page structure, and the
scroll-reveal effect's fallbacks.

Also reviewed by reading:

- `repo.js` assigns `link.href = asset.browser_download_url` from the GitHub API
  without a scheme check. The value comes from GitHub's own release API for this
  repository and is always a `github.com` URL, so it is not currently reachable;
  it is the kind of assignment worth a guard anyway, and is listed in §5.
- `verify.js`, `theme.js`, `legal.js` and `reveal.js` write through
  `textContent`, `classList` and typed DOM properties throughout. No `innerHTML`
  outside the two places noted, no `eval`, no `new Function`, no
  `document.write`.

### 4.5 The wider sweep, and what it cost to skip it

The previous revision closed this section by saying the remaining gap was an
outside reviewer. That was true and it was also a way of not saying the other
thing: the scope that round had set itself was narrower than the code.

This round set the scope from the *vulnerability classes* rather than from a
list of things that seemed worth checking, and walked each class across the
whole tree. Twenty-eight defects. None of them needed a technique the previous
round lacked. Every one of them needed somebody to look at that particular
thing:

- The class **"allocation sized by untrusted input"** had been applied to the
  Argon2 cost parameters (F-3) and to nothing else. Applying it to the DSP
  configuration found F-10; to the decoder, F-17.
- The class **"panics reachable from untrusted input"** had been applied to
  VeilVoice's own parsers and not to the boundary with `symphonia`, where F-9
  was waiting -- and where, under `panic = "abort"`, a panic is not an error
  but the end of the process.
- The class **"TOCTOU and path handling"** had not been applied at all. It
  found F-12 (an erase that destroyed a file other than the one named), F-14
  and F-15 (secrets created world-readable), and the check-then-write races
  behind all three.
- The class **"dependency and environment trust"** had not been applied to
  *how a subprocess is located*, which is F-13: a planted `reg.exe` in the
  working directory, executed by the two features somebody runs when they
  already suspect their machine.
- On the website, **"resource exhaustion"** had never been applied to the
  renderer at all, because the hostile-input suite asked only whether the
  output was safe. It was. It just took eight seconds to produce (F-22, F-23).

The lesson is recorded plainly because it will apply again: *a finished audit
scope is only as wide as the list it was drawn from.* The defence is to
enumerate the classes rather than the worries, which is what the standard at
the top of this document now says.

## 5. Still open

1. **An outside reviewer.** Not a task item -- see the standard at the top of
   this document -- but still worth having, and still absent. Everything here
   is the author checking the author's work. What that is worth is now a
   measured quantity three times over: three rounds of wider tools and wider
   scope have each found real defects in code a previous round had called
   clean. This round it was F-37, live on the published site the whole time.

2. **The coverage-guided campaign has now been run twice, at five and then ten
   minutes per target, and that is still not convergence.** All six targets, on
   x86-64 Linux. The second run is 703 million inputs and is written up above;
   it found F-92.
   It found two defects in its first run: F-82, an unbounded Argon2 time cost
   reachable from a `.veil` file and from the app-lock file that is read before
   anyone authenticates; and F-83, a tamper record whose path could rewrite the
   report that prints it.

   That is the strongest evidence in this document for what an unrun campaign
   is worth. Both defects had shipped. Both were in code that three audit
   rounds had read. Neither was reachable by the deterministic campaign, which
   generates inputs by construction rather than by coverage feedback, and both
   are now regression tests inside it.

   **A seed corpus is now committed for the two targets that need one.** The
   gap it closes was measured rather than assumed, on `lock_file`, two minutes
   each way: a cold run starts at 25 code paths and reaches 460 after 64,309
   inputs, while a seeded run *starts* at 625. The seeds give a campaign more
   coverage in its first second than two minutes of cold running achieves,
   because both crypto targets hide their interesting code behind a magic
   string, a version byte and three cost fields.

   Only `container_header` and `lock_file` have seeds, and the other four are
   left deliberately: they manage between twelve and three hundred million
   inputs in ten minutes and find their own structure in seconds.
   `guard_manifest`'s minimised corpus alone is 3.9 MB, which is a great many
   committed bytes to save a target no time at all.

   What is still open: ten minutes is not convergence either, three of the six
   targets have never found anything, and nobody has run any of it on Windows
   or macOS. `fuzz/README.md` carries the run counts and the seed measurement
   and says the same thing in its own words.

3. **32-bit targets are now exercised in CI, and this entry says what that
   does and does not cover.** It had been open since the fifth round, named
   here as the single highest-value change available to CI, because *two*
   shipped defects came out of its absence: F-4 (an overflow) and F-11 (a
   non-terminating erase loop), both reachable only where a pointer is 32 bits
   wide, both found by reading, and neither reachable by any campaign on an
   x86-64 host.

   The `narrow` job runs `i686-unknown-linux-gnu` on the runner's own kernel
   and `armv7-unknown-linux-gnueabihf` under `qemu-user-static`. Re-measured
   after the app lock gained its version 2 record, the vault and the integrity
   module: **716 tests pass on both targets, with no failures on either.** The
   count moved from 682 because the new code brought its own tests with it, and
   running them here was the point: a keyed tag, a masked file and two
   filesystem paths are exactly the kind of code where a narrow pointer shows
   up, and none of it had been run anywhere but on x86-64 until this pass.

   **It is not the whole workspace, but it is two crates wider than it was.**
   `veilvoice-audio` and `veilvoice-video` have joined the `i686` list: they
   link ALSA and nothing else, and the multiarch sysroot that had been written
   off as an exercise turned out to be `dpkg --add-architecture i386` and one
   package. That is 81 more tests on a 32-bit target, in the crates that walk
   RIFF chunks, which is where F-4 was. `veilvoice-cli` and `veilvoice-gui` are
   still out and GTK is why: its 32-bit development packages do not install
   cleanly here, and that one really is a sysroot exercise rather than a 32-bit
   correctness one. The arithmetic, the parsers and the erase loop are
   in the crates the job does run, which is why those are the ones it runs.
   Neither target has been exercised on Windows or macOS, and no 32-bit
   *release* build is published for any platform.

   And a passing run is not the same as a campaign: this proves the existing
   tests hold where a pointer is narrow, not that a narrow pointer has been
   hunted for. F-4 and F-11 were found by reading, and reading is still what
   would find the next one.

4. **A hostile file in a format VeilVoice does not itself parse.** F-9's
   pre-flight covers the one confirmed decoder crash. Under `panic = "abort"`
   nothing in-process can cover the next one, whatever format it is in;
   decoding in a separate process is the only complete answer and is not built.

5. **The privileged half of tamper detection**, unchanged from the previous
   round and for the same reasons -- see [`ROADMAP.md`](../ROADMAP.md). Detection
   logic is done; privilege and setup are not, and the obvious version of it is
   worse than nothing.

6. **`install.sh` has now been run on Linux; macOS is still unrun, and nobody
   outside this project has run any of it.** On x86-64 Linux it was run end to
   end against the published v0.1.14 release: latest tag found, archive
   downloaded, key fingerprint compared, signature over `SHA256SUMS` verified,
   archive hash matched against the signed list, both binaries installed, and
   the installed `veilvoice info` ran. Its refusals were exercised on the same
   machine and both exited 1. That run found F-79.

   What is still open is macOS, whose `sh` is not Linux's, and the larger
   point: every one of these runs is the project checking its own work.
   `docs/INSTALL.md` says so in its own words.

7. **Two of the six package definitions have been built; four have not.** The
   Debian one builds, installs, runs and removes, on one x86-64 Ubuntu machine,
   and doing that found F-80 and F-81. The RPM now builds too, on the same
   machine: a source RPM from a `git archive` tarball, then two binary
   subpackages whose contents were checked against the spec. WiX, Flatpak,
   Homebrew and the Gentoo ebuild still only parse, and that is the whole of
   what is claimed for them. `docs/PACKAGING.md` carries a per-format table
   saying which is which.

   The RPM build is weaker than the Debian one and the gap is named rather than
   glossed: it ran on Ubuntu rather than on any RPM distribution, it needed
   `--nodeps` because `rpm` cannot read `dpkg`'s database (every build
   dependency was confirmed present by hand first), it needed `--nocheck` so
   the spec's own `%check` has never run, and `rpmlint` has not been run.
   What it did prove is the thing a parse cannot: `%files` and `%install`
   agree, in both subpackages, which is the classic spec defect.

   **`lintian` has now been run** over both Debian packages: no errors, five
   warnings. Two concern uploading into Debian's own archive and do not apply.
   The other three were `no-manual-page`, one per binary, and that one was
   real: `man veilvoice` produced nothing. It is now fixed, in the packaging
   rather than by hand, and the fix is described in item 10.

   Still open: nothing has been uploaded anywhere, `rpmlint` has not been run,
   the Debian build used a rustup toolchain rather than Debian's own `cargo`
   and `rustc` packages, and four formats remain drafts.

8. **`rsa` carries an unfixable advisory** (A-6). Accepted on the ground that
   the verifier performs no private-key operation, and enforced by a CI job
   rather than left as a claim. If rPGP ever offers a backend that avoids the
   `rsa` crate for verification, that is worth taking.

9. **The stated test count belongs to whichever platform last measured it.**
   F-77. `docs/MEASURED.md` now records the host it was taken on, so the number
   no longer pretends to be platform independent. What is still open is the
   wording on the front page, which states one platform's total with nothing
   beside it. Saying it in a way that stays true for a reader on any of the
   three is a change to the page's own voice rather than to a generator, so it
   waits for the maintainer.

10. **Manual pages are generated from each binary, and the third one needed a
    `--help` to generate from.** `lintian` reported `no-manual-page` for all
    three binaries. A page written by hand would be a second description of the
    interface kept in step with the first by nothing but attention, which is
    the arrangement that produced F-71, so `tools/release/manpage.py` derives
    each page from the binary's own `--help` at package build time. Nothing is
    committed, so nothing can go stale.

    `help2man` does this job and was tried first. It mangles the output: every
    em dash in VeilVoice's help came back as `???`, at `C`, at `C.utf8`, and
    with `LC_ALL` set either way. A page that renders the program's own
    description as three question marks looks finished and is not, so the forty
    lines that get the encoding right were worth writing.

    Two things fell out of doing it. `veilvoice-gui` had no `--help` at all: it
    opened a window instead, and on a machine with no display it answered a
    reasonable question with a winit error naming `WAYLAND_DISPLAY`. It now
    answers, on Unix, where a release build is guaranteed a console; on Windows
    `windows_subsystem = "windows"` means `println!` writes to nothing, so
    behaviour there is deliberately unchanged rather than silently made worse.
    And the first version of that help text named three tabs that do not exist
    (`watch` and `security` for what are really `monitor` and `lock`, and no
    `install`), which would have shipped inside the package. A test now
    compares the help text against `Tab::ALL`.

    Confirmed rather than assumed: `lintian` over the rebuilt packages reports
    no errors and two warnings, both about uploading into Debian's own archive,
    and all three `no-manual-page` warnings are gone. `dpkg -c` shows each page
    in the right package, and each was installed and rendered with `groff`.
    That check took one detour worth recording, because it looked like a
    packaging bug and was not: this build machine is a *minimized* Ubuntu
    image, which carries `path-exclude=/usr/share/man/*` and discards manual
    pages as it installs them. The pages were in the packages all along, and
    believing the first `ls` would have produced a confident and wrong entry
    here.

    Still open: read on Linux, with `groff`. Nobody has read them on macOS or
    through a different `man` implementation, and `mandoc -Tlint` has not been
    run.

11. **The interface text is clean; the doc comments are not.** The fifty em
    dashes in `veilvoice-cli` are gone, and so are the four elsewhere in the
    workspace that reach a user: two error messages in `veilvoice-crypto`, one
    in `veilvoice-guard` and one test assertion. Every sentence was rewritten
    rather than having its punctuation swapped, and the ten committed CLI
    drawings were re-captured from the built binary, so what the gallery shows
    is what the program prints.

    **349 remain, all of them in `//!` and `///` doc comments.** The rule
    covers those too, and they are not user-facing text: they are read on the
    generated documentation pages and by anybody in the source. This is a
    stated remainder rather than a claim of completion, and the number is
    measured rather than estimated.

    The entry used to say the re-capture was a manual pass needing a build, a
    machine and somebody deciding the new output is right. That was true, and
    it was also the whole problem: see F-103.

## 6. Verdict

**One hundred and three defects found and fixed across seventeen audit rounds (F-1 to F-103):**
eight in the first two, twenty-eight in the third, eleven in the fourth,
twelve in the fifth, one in the sixth, five in the seventh.

**None of the seventh round's five had shipped**, which is the first round that
can say so, and it is worth being careful about why: `main` has not been
released since v0.1.12, so "had not shipped" and "was written this cycle" are
the same sentence. It is not evidence that the code is getting better. The
pattern that *is* worth noting is that three of the four were **comments that
had stopped being true** -- about where a call sits, about what a stylesheet
costs -- rather than logic that was wrong. A wrong thing that agrees with
itself survives every reading.

Of this round's twelve, **two had shipped** and ten were caught in code written
during the round. Keeping those apart matters: a round that counts
just-written-and-immediately-fixed defects alongside ones that were live on the
published site is flattering itself.

Neither of this round's shipped defects was a confidentiality failure either,
and both are worth naming for what they were. One rendered this project's own
README as tag soup at the top of its front page. The other made the secondary
text of the default theme too low-contrast to meet the accessibility floor --
on the website, in the desktop application, in the terminal, and inside the
banner image, where the affected line was the licence.

**No finding in any round has been a confidentiality failure.** Nothing has let
an attacker recover a voiceprint, read a sealed recording, bypass a password, or
weaken the cryptography. That distinction is real and it is worth drawing. It is
not a reason to be pleased, because the failures that *did* occur are the ones a
person relying on this would care about:

- **Two erased the wrong thing or nothing at all.** F-12 filled an unrelated
  file with random data and reported success; F-11 hung for ever and left the
  target intact. Both in the feature whose entire promise is destroying a file.
- **Three killed the process from a file somebody sends you** (F-3, F-9) or
  from the app-lock file read before anyone has authenticated (F-2).
- **Two produced silence and said nothing.** F-5 from a sample, F-10 from a
  configuration value -- the same failure through two doors, the second found
  only because the first had been written up properly.
- **One ran a program the user did not choose** (F-13), in the two features
  somebody runs precisely because they suspect their machine.
- **Three left secrets readable by other local accounts** (F-14, F-15), or
  unwiped in freed memory (F-18).
- **On the website, two froze the reader's tab** for seconds at a time on text
  fetched over the network (F-22, F-23), and three made the page unusable on
  engines a great many people are still running -- including a legal gate that
  was an invisible modal on pre-2023 browsers (F-30) and one that could not be
  dismissed at all on an iPhone (F-33).
- **And one made the site illegible about itself** (F-37): the banner carrying
  this project's own claims, its licence and its authorship was rendered with
  more than half its pixel rows deleted, on every viewport, for as long as the
  banner has existed. Every test passed the whole time.

The cryptography itself continues to stand up. The primitives are standard and
well reviewed, and the composition -- the hybrid combiner, the authenticated
container header, the domain-separated app-lock verifier -- is done properly
rather than approximately. Not one defect in three rounds has been in the
cryptographic construction.

This round adds a boundary the previous three did not have: **the point where a
user decides whether to trust a download.** The install scripts and the verifier
are that boundary, and they are written to refuse rather than continue, to name
the check that failed, and to have no flag that skips verification. Three of
this round's defects (F-44, F-45, F-46) were in that code and were found by
running it rather than reading it -- which is the same lesson as F-37, in a
different medium.

Every defect has instead been at a **boundary**, and the list of boundaries has
grown each round: parameters read from a file and handed to a library without a
bound; samples folded into persistent state without a check; *configuration*
folded into the same state without a check; a decoder handed a file before
anyone looked at it; a path used without asking what it actually points at; a
subprocess named without saying where it lives; a file created before its
permissions were set; text read from the network and rendered without a bound on
the work. That is where the next one will be too.

The app lock adds a control whose value is real but bounded, and the bound is
stated everywhere it appears rather than only here. A lock a user over-trusts
makes them less safe, not more; that is the failure mode this design was written
to avoid, and the one to keep watching in any future change to it.

The project's main security asset is not any single control but the fact that
**every claim it makes is checkable**: no `unsafe`, no network, reproducible
builds, generated artwork, and documentation that states limits rather than
hiding them. This document is part of that, and it is the third revision in a
row to have to record that the previous one was more confident than the code
deserved. The honest response is to say so here rather than to quietly improve
the score -- and to keep the standard at the top of this document pointed at
*classes of defect* rather than at a list of things that felt worth checking,
because the list is what was wrong both times.
