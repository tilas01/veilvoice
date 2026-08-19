<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Changelog

The section matching a release tag is published at the top of that release's
notes on GitHub, so this file is the source of truth for what changed rather
than a summary written afterwards.

## v0.1.11

**If you have v0.1.10 on Windows, replace it.** The desktop application flashed
a console window and could fail with no message at all.

### The Windows desktop application

Three separate defects were behind one report — "it flashes a command prompt,
loads in an unusable state, and crashes".

**The flashing console was never the application's own window.** `veilvoice-gui`
has no console. Every *subprocess* it starts has one, and on Windows that means
a window appears and vanishes as the child runs: once at startup, when the
application asks the system whether animation is wanted, and again on every
poll while the monitor tab is open. Every subprocess in the project now starts
with `CREATE_NO_WINDOW`, and a test in each crate fails if one is added without
it.

**A failure used to produce nothing at all.** No console, and the release build
aborts rather than unwinds, so a crash left no message, no dialog and no log —
nothing to report but "it crashed". VeilVoice now writes a short report beside
your preferences and tells you about it next time it starts. **It is written on
your machine and sent nowhere**; there is no network code in this program to
send it with.

If the window never appears at all, the most likely cause is that the computer
could not provide an OpenGL context — common in a virtual machine, over a
remote desktop session, or with hybrid graphics. The report says so, and points
at `veilvoice`, the command-line tool, which does the same work and needs no
graphics.

### Icons

Every executable now carries its icon, on every platform.

- **Windows:** embedded in the binary, at all six sizes. It was previously
  shipped as a loose `.ico` beside the program — a file Windows never reads —
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
