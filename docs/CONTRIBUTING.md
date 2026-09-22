<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Contributing to VeilVoice

## Before anything else

**A security problem does not go here.** Report it privately, the way
[`docs/SECURITY.md`](SECURITY.md) describes. The ordinary issue tracker is
right for everything else, including a bug that is merely embarrassing.

## Getting it building

VeilVoice is a Rust workspace of thirteen crates producing two programs. It
needs a recent stable toolchain, and on Linux the audio and windowing headers.

```bash
git clone https://github.com/tilas01/veilvoice
cd veilvoice

# Linux only: the headers cpal and the window need.
sudo apt-get install -y libasound2-dev libgtk-3-dev libxdo-dev

cargo build --workspace
cargo test --workspace
```

The desktop application is `veilvoice-gui`, the command line is `veilvoice`, and
both are in `target/debug` after that build. There is no third binary: the
verifier lives inside both.

## The one thing that decides whether a change is finished

**Run `python tools/verify.py` before you push.** It regenerates everything
derived from the source, then checks that what is committed matches, and it is
the same set of checks CI runs. It takes a while and it is not optional: most of
what it catches is not a broken test but a document that has quietly stopped
being true.

```bash
python tools/verify.py
```

If it fails, the message says which check and why. Nothing in it is advisory.

**Then run `git status`, before you push and after the verify passes.** Most of
those checks work by regenerating the derived file and comparing it, and the
regenerated file is left where it was written, so a run that passes can still
leave something uncommitted. `Cargo.lock` is the one that catches people:
the site's search index reads it, so a dependency bump makes
`website/search-index.json` and `website/nojs/search.html` stale, `verify.py`
rewrites both in place, and the push then carries a lock file whose index is
still describing the old one. A green run is not the same as a clean tree.

## Why so much of this is generated

Anything stated in more than one place is derived from one source or checked
against it, rather than written twice. The version comes from `Cargo.toml`. The
line counts come from the tool that counts lines. The per-crate pages, the
website reference and the wiki are all one generator reading the doc comments in
the code. The release notes are `CHANGELOG.md` and everything else quotes it.

So the way to change what a document says is usually to change the code or the
one source file, and re-run the generator. Editing a generated file directly is
wasted work: the next `--check` will fail on it, and the header at the top of
each one says so.

## The four standing rules

Three of these are enforced by a build rather than by anybody remembering.

**Nothing in a realtime path allocates, locks or prints.** An audio callback
runs on the operating system's audio thread with a deadline of a few
milliseconds, and each of those three hands the thread to somebody else's
schedule. Every buffer is sized once before the stream starts, and only the
non-blocking forms (`try_lock`, `try_push`) are used. A test reads the callbacks
themselves and fails naming the line.

**A dependency says what it is for, on the line that declares it.** One
sentence, at the moment the decision is made. `tools/audit/dependencies.py`
fails the build on a bare name. If there is no sentence to write, that is the
answer, and it does not go in.

**Upgrades arrive continuously and never land on their own.** Dependabot
watches every manifest weekly, and `.github/workflows/ci.yml` is the gate:
nothing auto-merges, so every bump waits for a person looking at green CI.
Minor and patch versions arrive batched, because twenty a week is noise. A
major arrives alone, because it is a decision rather than an update: `cpal`
0.15 to 0.18 was thirty-seven compile errors across four files in the realtime
path. A handful of majors are held back in `.github/dependabot.yml`, each
scoped to `version-update:semver-major` so patches and security fixes still
come through, and each with its reason written where the dependency is
declared. `tools/audit/dependabot.py` fails the build if one of those holds
names a dependency the tree no longer uses, or if one is missing its
`update-types` and would therefore silence an advisory.

**Work that can be done once is done once, and a comment says what made it
constant.** A value computed per frame or per sample that does not change per
frame or per sample is a defect.

**The reading happens before the push.** Every change is read for the three
above before it goes. That is the one rule a test cannot check, which is why it
is written as a habit and the others are guards.

## House style

- **British spelling.**
- **No em dashes** anywhere a person reads: not in the interface, the
  documentation or the website. Use a comma, a full stop, or a pair of hyphens.
- **Markdown where it is rendered, plain text where it is not.** Markdown is
  formatting, so it belongs where something renders it and nowhere else. A
  commit message is read as plain text by `git log`, by every terminal and by
  GitHub's own commit view, so it is written as plain text: see *Commits*
  below for the shape. The same goes for anything a release carries that is
  written in a `.md` file and never rendered from one, such as a tag message
  or a note read in a terminal. Where markdown genuinely is rendered, the
  formatting stays: the release notes on the GitHub website are rendered
  markdown, so they keep their headings, their links and their lists, and so
  does everything in `docs/`, in `wiki/` and on the website.
- **Every behavioural change carries a regression test.** A fix without one is a
  fix that comes back.
- **`docs/AUDIT.md` gets the write-up when the change is a fix**, including what
  was wrong, how it was found and what now stops it returning.
- **Say why, not what.** The code already says what it does. A comment earns its
  place by recording the decision somebody would otherwise have to re-derive,
  and the reason a plausible alternative was not taken.

## Branches, and when a version is cut

**Two branches, and there is never a third.** `dev` is where development
happens; `main` is what has been released.

| | `dev` | `main` |
|---|---|---|
| What it holds | every change, as it is made | the released program |
| Who pushes to it | anybody working on VeilVoice | only a release merge |
| CI | runs on every push | runs on every push |
| The website and the wiki | not published from here | published from here |
| Tags | `v0.1.23-beta.1`, marked prerelease | `v0.1.23`, made by the release workflow at the commit it built |

Work is committed and pushed to `dev`. `main` moves when a release is cut, by
merging `dev` into it and tagging that merge; the release workflow builds the
commit, and the site redeploys when it finishes. The `promote` workflow does
all of that, and the gates it runs first are below.

The one exception to "never a third" is a `dependabot/*` branch. Dependabot
pushes one per pull request it opens, and it goes away when that pull request
is merged or closed, so it is a temporary branch belonging to a bot rather than
a place anybody works. Those pull requests target `dev`, like every other
change: `.github/dependabot.yml` says `target-branch: dev` on each ecosystem
and `tools/audit/dependabot.py` fails the build if one stops saying it, because
a bump merged straight into `main` is a change `dev` does not have and the next
release merge either loses it or conflicts with it.

An early build is a tag rather than a branch: the same source, the same
workflow, the same signing and the same hash lists, published as a prerelease
so the download page never offers it by default. Roadmap item 165 is the rest
of that work.

**A version is cut when a group of changes is finished and audited**, not on a
date. In practice that means: the roadmap items in the group are done, a
`tools/verify.py` run passes whole rather than `--quick`, the screenshots and
the recorded sessions have been retaken if the interface moved, `CHANGELOG.md`
has the notes, and an audit round has read what changed. `docs/AUDIT.md`
records that round; the release notes are derived from `CHANGELOG.md` and
nothing hand-copies them anywhere.

**That list is a workflow now rather than a habit.** Run `promote` from the
Actions tab, give it the version, and leave *Move main* off the first time: it
reports what stands between `dev` and that release and changes nothing. Run it
again with *Move main* on and it merges `dev` into `main`, pushes it, and
starts the release build, which creates the tag at the commit it built and
redeploys the website when it finishes. No tag is pushed by hand at any point.

**It cannot run until it is on `main`, which is once.** GitHub registers a
`workflow_dispatch` workflow only from the repository's default branch, so a
workflow that exists on `dev` alone is not listed and not dispatchable: the API
answers 404. `promote.yml` arrives on `main` with the first release merge, so
that merge is the one this workflow cannot perform. Cut it the old way, by
merging `dev` into `main` and dispatching `release` with the version, or push
`promote.yml` to `main` on its own first. From the release after it, the
workflow does all of it.

What it asks before anything moves:

- the version a person typed is the version `Cargo.toml` declares
- `CHANGELOG.md` has a `## v<version>` section and it is the newest one
- every roadmap item that release's section names is marked done
- `docs/AUDIT.md` has been written in since the previous tag
- the tag does not exist, and nothing has landed on `main` that `dev` lacks
- `tools/verify.py` passes whole, from a fresh checkout rather than a
  working tree
- `ci.yml` is green on the exact commit being promoted

The first six are [`tools/release/readiness.py`](../tools/release/readiness.py),
which runs anywhere: `python tools/release/readiness.py v0.1.23` answers the
same question from a laptop, and is the quickest way to see what a release is
still waiting on.

**The fresh checkout is the part that is easy to skip and should not be.** A
commit reached `dev` referencing a test fixture that `.gitignore` had kept out
of the repository. `git add` skipped it without a word, `git status` was clean
because an ignored file is never a candidate to report, `tools/verify.py
--quick` passed, and the tree did not compile for anybody who cloned it. No
check run where the author is standing can see that, because the file is
there. The gate runs where a reader stands.

**One gate on that list may still need a person, once.** The `wiki` workflow
publishes from `main`, so a release merge is when it runs, and it has something
to push to only if the wiki repository exists. It tries to create that
repository itself, and where GitHub refuses, the one-time step in the web
interface is written out in [`WEBSITE.md`](WEBSITE.md) under *The wiki, in two
places, from one source*. It is named here so a release merge is not where it
is discovered.

## Commits

Write the message for somebody reading it in two years with no memory of the
conversation that produced it: what changed, and why that rather than the
obvious alternative. Long is fine. Vague is not.

**The message is plain text, not markdown.** A title line on its own, a blank
line, then the prose, and where the change is a list of changes, one line per
change beginning with a hyphen and a space. No `#` headings, no `**bold**`, no
backticks around a name for emphasis, no markdown links. Nothing renders a
commit message, so markup in one is punctuation a reader has to read past:

```
Roadmap 169: one loading indicator, used everywhere

Every worker roadmap item 167 created showed progress its own way, which
is five spinners with five behaviours and no answer for a reader who has
asked their system for less movement.

- One indicator, drawn once, with the sentence beside it saying what is
  happening
- Reduced motion turns it into a static progress statement rather than a
  spinning thing
- The Browser, the verifier and the companion probes all use it
```

Backticks around a path or a command are fine where the name would otherwise
be ambiguous, because a reader understands them and they are one character.
The rule is against writing a document in the message, not against punctuation.

Commits here have **one author**. Do not add `Co-Authored-By:` trailers, and do
not put an assistant or model name in a commit message, a tag, a release note or
a pull request body. That is a rule about where credit lives rather than a
denial that it is owed: AI assistance is credited in the README and in the
footer of every page of the website, which is where a reader looking for it will
look.

## Pull requests

Say what the change does and why, and what you did to convince yourself it
works. If `tools/verify.py` passes, say so. If it does not and you think it is
wrong, say that too, and why: a guard that fires on something legitimate is
itself a finding, and the repair is usually to make the guard read the program
more precisely rather than to loosen it.

Small and complete beats large and nearly. A change that needs a paragraph of
caveats usually wants to be two changes.

## Licence

GPL-3.0-or-later. By contributing you agree your work is licensed under it.
Copyright stays with tilas01, who is the sole author for licensing purposes.
