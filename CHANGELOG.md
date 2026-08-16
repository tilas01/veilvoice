<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Changelog

The section matching a release tag is published at the top of that release's
notes on GitHub, so this file is the source of truth for what changed rather
than a summary written afterwards.

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
