<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Updating, and what to expect when you do

VeilVoice does not update itself, does not check for updates on its own, and
will not start doing either. This page is how to find out that something newer
exists, how to move to it, and what you will see afterwards.

---

## Nothing checks unless you press the button

There is no timer, no check at startup and nothing in the background. The only
thing in VeilVoice that reaches the network at all is the **Check for updates**
button on the About tab of the window, and it runs because somebody pressed it
in that run of the program.

The command line does not have it. `veilvoice --help` says so in its opening
paragraph: the command line talks to no servers, ever.

The reason is worth stating, because "check for updates automatically" reads as
a courtesy rather than as a disclosure. An update checker that runs by itself
tells a server that this machine has VeilVoice on it, roughly how often it is
used, and from which address. For most software that is uninteresting. For
software somebody installed because they are worried about being identified, it
is the one message you would least want sent, and it would be sent every day
without being asked for.

So it is a button. There is no setting that turns it into a schedule.

## What the check actually does

It fetches one public web page over TLS, the release page anybody can open in a
browser, and reads the version out of it. The request carries no identifier, no
configuration and no counter.

There is still no HTTP client anywhere in VeilVoice. The check runs the transfer
tool your operating system already ships, found at an absolute path rather than
by name, and reads its output. Resolving a program by name on Windows searches
the current directory before most of `PATH`, so a file called `curl.exe` left
beside VeilVoice would be run instead of the system one. That was a real
finding here, and it does not get to happen twice.

## What the answer is worth

Four answers come back:

| The window says | What it means |
|---|---|
| Up to date | The newest published release is the one you are running. |
| A newer version exists | Something newer has been published. The version is named. |
| Ahead of what is published | This build is newer than anything on the release page, meaning it was built from source. |
| A version it cannot read | A version string came back that this build cannot compare against its own. Reported rather than guessed at, because pretending to order two strings that do not parse is how a checker tells somebody they are out of date when they are not. |

Whatever it says, it is a version number read off a public page. **It is not a
signature, and nothing in the check has verified anything.** The window prints
that sentence beside the answer rather than leaving it to this document.

If the check fails, it will say why: your system ships no transfer tool it knows
how to drive, the tool ran and failed, or the reply held no version it could
find. In all three cases the answer is the same, which is to open the releases
page yourself and look.

## Two streams, and which one you are on

| Stream | Cut from | What it is |
|---|---|---|
| stable | the released branch | What the download page offers. |
| early | the development branch, under a prerelease tag | Newer, and checkable in exactly the same way. |

Which one a build belongs to is read from its own version string: a prerelease
version *is* a prerelease. Nothing is compiled in to say so, which is what keeps
a binary a pure function of its source. A check made by an early build reads the
page for early builds, so it does not tell you that you are behind when you are
ahead.

## Doing the update

There is no command that does this for you yet. Updating is four deliberate
steps, and the third one is the one that matters:

1. **Get the new release.** From the
   [releases page](https://github.com/tilas01/veilvoice/releases), or build it
   from source.
2. **Check it before you put it in place**, exactly as you checked your first
   copy: the signature over the hash list, against the fingerprint published in
   [`INSTALL.md`](INSTALL.md) and [`README.md`](../README.md).
   [`GUIDE_VERIFY.md`](GUIDE_VERIFY.md) is the whole account of it.
3. **Check it with the copy you already trust**, not with the copy you just
   downloaded. Your existing `veilvoice verify` has the signing key compiled
   into it. The new archive's own copy of anything cannot vouch for itself,
   and neither can a new binary you have not checked yet.
4. **Replace the files.** VeilVoice runs out of a folder and keeps its
   configuration, keys and vaults elsewhere, so replacing the program does not
   touch your data. If you installed it with `veilvoice install`, run that
   again from the new copy.

## What you will see afterwards

**VeilVoice will report that its own program file changed.** That is expected,
and it is the update.

The record described in [`GUARD.md`](GUARD.md) is a size and a hash of the
program file, and an update you installed looks exactly like a file somebody
swapped, because on disk those are the same event. Nothing can tell them apart
after the fact, which is precisely why step 3 above is where the real check
happens: you decide the new file is the published one *before* you put it in
place, and the change report afterwards is then something you already know the
answer to.

Once you have updated, record the new state so that the next report means
something again:

```bash
veilvoice guard init --sealed
```

If you have an app lock, the window does this for you when you next unlock and
confirm, and the record is sealed under your passphrase again.

Your settings, keys, vaults and app lock are not touched by an update. They
live in the configuration directory, not beside the program.

## What is deliberately not here yet

An updater that downloads a release, verifies it with the same code that
verifies any other download, and puts it in place is planned, and it is planned
*on top of* the verifier rather than beside it: an update path with its own
private idea of what a valid release looks like is two answers to one question,
and the second one is always the weaker. Until it exists, the four steps above
are the update, and they are the same four steps an updater would take.

An update checker that could install its own answer is an update checker that
can be made to install somebody else's, so if one arrives it will still be
something you press.

---

## Where to go next

- [`GUARD.md`](GUARD.md): the change report, and what to do about one.
- [`GUIDE_VERIFY.md`](GUIDE_VERIFY.md): checking a download, at length.
- [`INSTALL.md`](INSTALL.md): the fingerprint, and installing per system.
- [`CHANGELOG.md`](../CHANGELOG.md): what actually changed, per release.
