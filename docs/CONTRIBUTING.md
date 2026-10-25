<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Contributing to VeilVoice

## Before anything else

**A security problem does not go here.** Report it privately, the way
[`docs/SECURITY.md`](SECURITY.md) describes. The ordinary issue tracker is
right for everything else, including a bug that is merely embarrassing.

## Getting it building

VeilVoice is a Rust workspace of thirteen crates producing two programs. It
needs the toolchain `rust-toolchain.toml` pins, 1.96.0, which rustup fetches
for you on the first build, and on Linux the ALSA headers and `pkg-config`.
Running the desktop application on Linux also needs `libxkbcommon-x11-0`, which
is loaded at startup rather than linked against;
[INSTALL.md](INSTALL.md) has the whole from-source walkthrough, including the
package names on Fedora and Arch.

```bash
git clone https://github.com/tilas01/veilvoice
cd veilvoice

# Linux only: the audio headers, and how the build finds them.
sudo apt-get install -y libasound2-dev pkg-config

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

**An advisory accepted once is accepted in every tool that reads it.** The
argument for each exception is written in `.cargo/audit.toml`. `deny.toml` and
`osv-scanner.toml` repeat the identifier, because cargo-deny, osv-scanner and
the OpenSSF Scorecard each read only their own file, and
`tools/audit/dependencies.py` fails the build if the three lists differ.
Beyond our own checks, `.github/workflows/codeql.yml` analyses the Rust, the
Python, the website's JavaScript and the workflows themselves, and
`.github/workflows/scorecard.yml` grades the repository from the outside every
week.

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

**One branch, `main`, and there is never a second.** Work is committed and
pushed to `main`, CI runs on every push, and a release is cut from it.

| | A push to `main` | A release |
|---|---|---|
| CI | runs on every push | runs again, whole, on the commit being released |
| The website and the wiki | checked, not published | published, from the commit that was released |
| Tags | none | `v0.1.23`, made by the release workflow at the commit it built |

A release is the commit at the tip of `main` that the `promote` workflow
checked. Nothing is merged to cut one: the workflow starts the release build,
the build creates the tag at the commit it built, and the website and the wiki
publish from that commit when it finishes. An edit to the website or to the
documentation therefore reaches the site and the wiki with the next release;
running `pages` or `wiki` by hand publishes the newest release again, never the
tip of `main`.
The gates `promote` runs first are below.

The one exception to "never a second" is a `dependabot/*` branch. Dependabot
pushes one per pull request it opens, and it goes away when that pull request
is merged or closed, so it is a temporary branch belonging to a bot rather than
a place anybody works. Those pull requests target `main`, the default branch:
`.github/dependabot.yml` names no `target-branch`, and
`tools/audit/dependabot.py` fails the build if an entry names any other branch
or if `ci.yml` stops building `main` on a push.

A bump is applied to `main` by hand, in tilas01's name, and its pull request is
closed rather than merged: a merge would make the bot an author of the history,
which the rule under Commits does not allow.

An early build is a tag rather than a branch: the same source, cut from `main`
between releases, with the same workflow, the same signing and the same hash
lists, published as a prerelease (`v0.1.23-beta.1`) so the download page never
offers it by default. The website and the wiki are not published for one.
Roadmap item 165 is the rest of that work.

**A version is cut when a group of changes is finished and audited**, not on a
date. In practice that means: the roadmap items in the group are done, a
`tools/verify.py` run passes whole rather than `--quick`, the screenshots and
the recorded sessions have been retaken if the interface moved, `CHANGELOG.md`
has the notes, and an audit round has read what changed. `docs/AUDIT.md`
records that round; the release notes are derived from `CHANGELOG.md` and
nothing hand-copies them anywhere.

**That list is a workflow now rather than a habit.** Run `promote` from the
Actions tab, give it the version, and leave *Publish* off the first time: it
reports what stands between the tip of `main` and that release and changes
nothing. Run it again with *Publish* on and it starts the release build of the
exact commit it checked. Work is pushed to `main` while the checks run, so if
`main` has moved by the time they finish it stops and says so rather than
releasing commits nothing checked; run it again and it checks the new tip. The
release build creates the tag at the commit it built, and the website and the
wiki publish from that commit when it finishes. No tag is pushed by hand at any
point.

What it asks before anything is built:

- the version a person typed is the version `Cargo.toml` declares
- `CHANGELOG.md` has a `## v<version>` section and it is the newest one
- every roadmap item that release's section names is marked done
- `docs/AUDIT.md` has been written in since the previous release
- the tag does not exist
- `tools/verify.py` passes whole, from a fresh checkout rather than a
  working tree
- `ci.yml` is green on the exact commit being released

The first five are [`tools/release/readiness.py`](../tools/release/readiness.py),
which runs anywhere: `python tools/release/readiness.py v0.1.23` answers the
same question from a laptop, and is the quickest way to see what a release is
still waiting on.

**Every release tag in this repository points at a commit that is on no
branch**, and anything written here that walks from a tag has to know it. All
twenty-three of them do: the history was rewritten at some point and the tags
were left behind on the pre-rewrite objects. The trees are identical, so what
was published is exactly what the branch has, and the commits are a parallel
line, so `git describe`, `git log <tag>..HEAD` and `git merge-base` all give
answers about a history that is not this one. That is finding F-254, and F-255
is the release gate quietly passing for four days because of it.

Until the tags are moved, which is a change to published history and a decision
for the repository's owner, `release_point()` in
[`tools/release/readiness.py`](../tools/release/readiness.py) is how a previous
release is located: the tag when it genuinely is an ancestor, otherwise the
commit on this branch carrying **the tag's tree**, which is the same content
under a different parent. Do not reach for the tag directly in a new script.

**The fresh checkout is the part that is easy to skip and should not be.** A
commit was pushed referencing a test fixture that `.gitignore` had kept out
of the repository. `git add` skipped it without a word, `git status` was clean
because an ignored file is never a candidate to report, `tools/verify.py
--quick` passed, and the tree did not compile for anybody who cloned it. No
check run where the author is standing can see that, because the file is
there. The gate runs where a reader stands.

**One step after a release may still need a person, once.** The `wiki`
workflow publishes when a release finishes, and it has something to push to
only if the wiki repository exists. It tries to create that repository itself.
Where GitHub refuses, the publish is skipped with a warning rather than failed,
so the run stays green, and the GitHub wiki stays on whatever it last held
until the one-time step in the web interface is done. That step is written out
in [`WEBSITE.md`](WEBSITE.md) under *The wiki, in two places, from one source*.
It is named here so the warning is not where it is discovered.

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

Commits here have **one author**, and three rules follow from it. The first
two are enforced by `tools/audit/authorship.py`, which `tools/verify.py` runs
before a commit is made and CI runs on every commit a push carries.

**Author and committer are both tilas01.** Every commit says
`tilas01 <tilas01@users.noreply.github.com>` in both fields. The one exception
is GitHub itself as committer, on a commit it makes for him in the website. The
committer matters as much as the author: it is the field a log shows least and
the one GitHub checks a signature against, and four of the fifteen commits that
once named somebody else did so only there (F-235).

A cloud container sets its own identity for git, in a config file and in some
containers in four environment variables as well. The variables beat every
config file, so `git config user.name` changes nothing where they are set, and a
plain `git commit` there is wrong by default. Set all four in the shell, before
`tools/verify.py` and before the commit:

```bash
export GIT_AUTHOR_NAME=tilas01 GIT_AUTHOR_EMAIL=tilas01@users.noreply.github.com
export GIT_COMMITTER_NAME=tilas01 GIT_COMMITTER_EMAIL=tilas01@users.noreply.github.com
```

The guard asks git what the next commit would say, so this is caught before the
commit exists. A commit that went wrong and has not been pushed is repaired with
`git commit --amend --no-edit --reset-author` once the identity is right;
`--amend` alone keeps the old author.

**No assistant is named in the history.** No `Co-Authored-By:` or session
trailer, no "generated with" footer or session link, and no assistant, its
maker or a model named anywhere in a commit message, a tag, a release note or a
pull request body, including by the name of a file it keeps. That is a rule
about where credit lives rather than a denial that it is owed: AI assistance is
credited in the README and in the footer of every page of the website, which is
where a reader looking for it will look.

**Every commit is signed by a key GitHub verifies as tilas01's.** An unsigned
commit shows no badge. A commit signed by a key registered to another account
shows Unverified, which reads worse than none, and that is what a cloud
container's own signing produces in his name: twenty-five of his commits read
that way before it was noticed. The keys that count are `SIGNING_KEYS` in the
guard, and CI asks GitHub whether it shows each pushed commit Verified. Until
every place commits are made from can sign with one of those keys, a missing or
foreign signature is reported rather than failed on; `SIGNATURES_REQUIRED` is
the switch, and it turns on in the commit that makes that true.

Where a commit is made decides which key signs it. On tilas01's own machine it
is his own key, `F8ADBAB7A4FB333C`. A cloud session cannot hold that one, so it
signs with a key made for the purpose and registered to the same account, held
base64-encoded in the project's `VEILVOICE_SIGNING_KEY` environment variable.
`python tools/repo/signing.py` imports it and sets that clone up to sign with
it, and fails, saying why, when the variable is missing, when the key is not one
the guard accepts, or when the identity variables above would still win. A
container's own signing key is never used in his name.

**A pushed commit is not rewritten.** Several sessions can be working from
`main` at once, and a force push moves the ground under every one of them. The
rewrite that corrects the fifteen is `tools/repo/rewrite.py`, which keeps every
tree and every date and refuses to push anything the guard would refuse. It is
the one exception, not a precedent.

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
