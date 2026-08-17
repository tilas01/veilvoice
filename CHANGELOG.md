<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Changelog

The section matching a release tag is published at the top of that release's
notes on GitHub, so this file is the source of truth for what changed rather
than a summary written afterwards.

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
