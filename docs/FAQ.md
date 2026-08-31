<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Questions people ask

Every claim here is also made somewhere it can be checked: the whitepaper, the
audit, the user guide or the source. Where the answer is that VeilVoice does
not do something, that is the answer rather than a gap.

Rendered to a page at `website/faq.html` by `tools/site/faq.py`. This file is
the source; edit it here.

## What does VeilVoice actually do?

It destroys the biometric voiceprint of a speaker and keeps the words. Pitch,
formants, timbre, micro timing and the melody of an accent go; what was said
stays intelligible and transcribable.

That is the whole claim, and the second half is deliberate. A scrambler you
cannot understand protects nobody, because nobody uses it.

## Can the original voice be recovered from the output?

No, and there are two separate reasons rather than one.

The measured phase of every frame is **discarded and never written anywhere**.
It is not encrypted or hidden, so there is no key that brings it back and
nothing stored from which it could be reconstructed.

And every speaker is mapped onto **one** canonical register and vocal tract.
Many voices go in and one set of characteristics comes out, so several
different people arrive at the same place. Even in principle there is nothing
to invert, because the mapping is not one to one.

## Does it hide what I said?

No, and it is important that it does not. The words survive on purpose. If the
message itself is sensitive, that is a different problem with a different
answer, which is encryption.

## Does it send anything anywhere?

No. There is no networking code in the project, and the build fails if an HTTP
client appears anywhere in the dependency graph. That is a CI job rather than a
promise, and it is one of the claims you can check in about ten seconds.

The one exception is the desktop application's check-for-updates button, which
you press or do not press.

## Will other people still understand me?

Yes. Intelligibility is the design constraint the rest of the engine works
around. What changes is who you sound like, not what you said.

## Can I use it on a call?

Yes, through a virtual audio cable: VeilVoice takes your microphone and writes
the veiled voice to the cable, and the calling program listens to the cable
instead of the microphone. VB-CABLE on Windows, BlackHole on macOS, PipeWire on
Linux. VeilVoice detects them and never bundles them.

Press **preview to my headphones** first. It runs the same engine and sends the
result to your own output and nowhere else, so you can hear what you sound like
before an interview rather than during one.

## What operating systems does it run on?

Eleven platforms have signed builds, including Windows, macOS on both
architectures, Linux, FreeBSD and OpenBSD. The microphone and camera monitor
works on Windows and Linux; macOS exposes no public interface for it, and the
tab says so rather than showing an empty list as good news.

## Is it free, and what licence?

Free software, GPL-3.0-or-later. The source is the whole of it: no separate
paid version, no telemetry, nothing held back.

## Who wrote it, and why a pseudonym?

It is published under the name tilas01. The pseudonym is deliberate, and it
costs something concrete rather than nothing: kernel-level enforcement on
Windows and macOS needs a certificate issued to a verified legal identity, and
so does macOS notarisation. Those are unavailable here, and the roadmap says so
rather than describing them as future work.

## How do I know the download is the one that was published?

Put `veilvoice-verify` in the folder you downloaded to and run it, or drop the
archive on the desktop application's verify tab. That is the whole instruction,
and it does every check below in the right order.

The checks, and the order is the point:

1. The signing key's **fingerprint**, compared against the one published in the
   README, on the website and in every release's notes.
2. The **signature** over `SHA256SUMS`, verified with that key.
3. The **archive's hash**, compared against the now trusted list.
4. **Every file you extracted**, compared against `CONTENTS.sha256`, which the
   release publishes and `SHA256SUMS` covers. This is the one that tells you
   the program you are about to run is the published one, rather than only that
   the zip was.

Checking the hash first and the signature afterwards proves only that the file
matches a list that might itself have been replaced. `veilvoice-verify` does it
in the right order, needs no GnuPG installed, and has no flag that skips a
step, because a verification with a skip switch is decorative.

If you do have GnuPG it uses that too: it adds the key to your keyring, tells
you it did and how to remove it, runs `gpg --verify`, and fails if the two
implementations disagree. It also prints the commands so you can run them
yourself, which is the part no program can do for you.

Releases before v0.1.15 carry no `CONTENTS.sha256` and stop at check 3, which
the tool says at the time.

You can also build the repository yourself and compare what comes out against
the published hashes for your platform.

## Does the app lock protect my recordings?

No. The app lock guards the application: it stops somebody who walks up to your
unlocked computer from opening VeilVoice and reading what is in it. A recording
is protected by its own encryption, which is on by default and is a separate
thing.

Somebody who can read your disk can read the lock file. The lock is worth
having and it is not a substitute for encrypting the recording, and the
application says so where you set it.

## What is the decoy passphrase? Is it deniability?

A second passphrase that opens VeilVoice with nothing in it, so you can comply
with somebody standing over you without handing over your recordings.

**It does not give you deniability.** VeilVoice is open source and this feature
is documented, so anybody who recognises the program knows the decoy exists and
can ask for the other passphrase. It buys you a way to hand something over. It
does not buy you an argument that there is nothing more, and that sentence is
the first thing the feature prints.

There is no destructive duress passphrase and there will not be one. On flash
storage a write does not overwrite: the controller puts new data in a fresh
page and leaves the old one until it is collected, which may be never. A
passphrase that claimed to destroy your recordings would be believed at exactly
the moment being wrong costs the most.

## Can it work out who is speaking in a recording?

No. Group mode gives each speaker a different voice, and it works from a plan
you write saying who speaks when. VeilVoice does not guess, because a program
that guessed would sometimes put one person's words in another person's voice
and you would not find out by listening: the result would sound perfectly fine.

Telling voices apart automatically is diarisation, it means shipping a trained
model, and this project ships none.

## Does it detect keyloggers?

No, and nothing can. The mechanisms a logger uses are the mechanisms
accessibility software, password managers and remote support tools use, and
software written to hide is written to hide from a process list.

What `veilvoice input` does is name the programs currently running that are
**able** to see your keyboard and mouse, and say what each is for. It prints,
with every result, that a clean answer proves nothing. Somebody who reads
"nothing found" as "nothing there" has been made less safe by running it.

## Does it hide its own window from screen recording?

No. Excluding a window from capture needs a foreign function call, and every
crate here carries `#![forbid(unsafe_code)]`, which is on the front page and is
one of the things you can check quickly. `veilvoice capture` says so rather
than implying otherwise.

Worth noting that it would not buy much: a window excluded from capture is
still visible to a camera pointed at the screen, and the thing VeilVoice
protects is a file rather than a picture of a window.

## What is Failsafe, and does it prevent anything?

It notices the moment another program picks up a **real** microphone while you
are being veiled, and by default it closes that program.

The accident it exists for: you are talking through VeilVoice, you plug in a
headset, the operating system offers the new microphone, the calling program
takes it, and from that moment your real voice is going out with the veiled
window still open in front of you and the meters still moving. Nobody notices,
because there is nothing to notice.

**It notices; it does not prevent, and the difference is printed every time.**
Stopping the operating system handing over a microphone needs exclusive capture
of every input device or a driver, and this project ships neither. Failsafe
sees it within about a second and acts. That moment is short and it is not
zero.

## Has it been audited?

Nine rounds, eighty-three defects found and fixed, all written up individually
in `docs/AUDIT.md` with what each one was and how it was found.

**By the author.** There has been no outside review, and that is recorded at
the top of the audit rather than left for you to wonder about. Three rounds of
wider tools and wider scope have each found real defects in code a previous
round called clean, which is a measurement of what author-only review is worth.

## What does it not protect against?

The honest list, which is also in the whitepaper:

- **Anyone who has the original recording.** VeilVoice changes a copy.
- **What you say.** Names, places and details identify you regardless of voice.
- **Everything around the audio.** Metadata is stripped from files VeilVoice
  writes; it cannot strip the metadata of a platform you upload to.
- **A compromised machine.** Software that is already running as you can read
  the microphone before VeilVoice does.
- **Being the only person it could be.** If three people were in the room,
  changing the voice does not change that.

## Do I need a GPU, or the internet?

Neither. It runs on the processor, offline, and the hardware report says what
it found and why the engine does not use it: the work per frame is small and
sending it to a graphics card and back costs more than doing it.

## What happens if I forget a passphrase?

The recording stays sealed. There is no recovery, no backdoor and no reset,
because any of those would be a way in for somebody else too. The app lock can
be removed by deleting its file, which does not affect encrypted recordings.
