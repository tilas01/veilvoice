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
| Tags | `v0.1.23-beta.1`, marked prerelease | `v0.1.23` |

Work is committed and pushed to `dev`. `main` moves when a release is cut, by
merging `dev` into it and tagging that merge; the release workflow builds the
tag, and the site redeploys when it finishes.

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

## Commits

Write the message for somebody reading it in two years with no memory of the
conversation that produced it: what changed, and why that rather than the
obvious alternative. Long is fine. Vague is not.

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
