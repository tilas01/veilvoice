<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Working in this repository

Read by every session that works here, which is why it is committed rather than
ignored. [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) is the human copy of the
same rules and says why each one exists. Where the two disagree, CONTRIBUTING is
right and this file is stale: fix this file in the same commit.

## Commits have one author, and it is tilas01

Before anything else, in the shell you will run `tools/verify.py` and commit from:

```bash
export GIT_AUTHOR_NAME=tilas01 GIT_AUTHOR_EMAIL=tilas01@users.noreply.github.com
export GIT_COMMITTER_NAME=tilas01 GIT_COMMITTER_EMAIL=tilas01@users.noreply.github.com
python tools/repo/signing.py
```

- **Author and committer are both `tilas01 <tilas01@users.noreply.github.com>`.**
  A cloud container presets its own identity, and some set it in those four
  variables, which beat every config file. `git config user.name` alone changes
  nothing there.
- **Every commit is signed with a key GitHub verifies as tilas01's.**
  `tools/repo/signing.py` imports the key the project's `VEILVOICE_SIGNING_KEY`
  variable holds and sets this clone up to sign with it. Never sign with the
  container's own key: GitHub knows it as another account's, so a commit it
  signs in tilas01's name reads Unverified. Until that variable exists, the
  guard reports an unsigned commit rather than refusing it.
- **No assistant is named in the history.** No `Co-Authored-By:`, no session
  trailer or link, no "generated with" footer, and no assistant, its maker or a
  model named in a commit message, a tag, a release note or a pull request body,
  including by the name of this file. Credit for assistance lives in the
  README's Credits section and the website footer, and nowhere else.
- `tools/audit/authorship.py` checks all of this before a commit is made and
  again on every push. When it fails, fix the identity and
  `git commit --amend --no-edit --reset-author`. Never push past it.
- **A pushed commit is never rewritten.** A force push is tilas01's decision,
  every time.

## Commit messages

Plain text, not markdown. A title line, a blank line, then prose, and where the
change is a list of changes, one line per change beginning with a hyphen and a
space. No headings, no bold. British spelling, and no em dashes anywhere a
person reads, commit messages included.

## Branches

Only `dev` and `main` exist, apart from a Dependabot pull-request branch. Work
is committed and pushed to `dev`. `main` is moved from `dev`, never committed to
on its own. A dependency bump is applied to `dev` by hand rather than merged
from the bot's branch, so the bot never becomes an author.

## Before every push

1. `python tools/verify.py --quick`. If Rust changed, also `cargo fmt --all`,
   `cargo clippy --workspace --all-targets` with no warnings, and
   `cargo test --workspace`.
2. Read `git status`. Verify stages what its generators write, so a commit made
   before it runs is missing them.
3. Commit, `git fetch origin`, `git rebase origin/dev`, verify again, amend,
   push. A conflict in a generated file is resolved by taking either side and
   running verify again; generated output is never edited by hand.

## The rest of the house style

- Nothing is stale: everything that describes a change moves in the same
  commit. `CHANGELOG.md` and `docs/AUDIT.md` are the exception, because they
  record the past.
- Every behavioural change carries a regression test, and every fix a
  `docs/AUDIT.md` write-up under the next finding number. Numbers cannot be
  reserved: read the highest from `origin/dev` at push time, and again after
  every rebase.
- Nothing in an audio callback allocates, locks or prints.
- Every crate keeps `#![forbid(unsafe_code)]`.
