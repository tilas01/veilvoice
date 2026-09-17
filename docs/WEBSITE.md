<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# The website

<https://tilas01.github.io/veilvoice/>

A static site with no framework, no build step and no bundler. Every page is
HTML on disk, every script is a plain ES module the browser loads directly, and
nothing is minified. That is deliberate: a site about a program whose whole
claim is that you can check what it does should itself be something you can read
without tooling.

## Two editions, and why the second is not a stub

**The ordinary site** is `website/`. It uses JavaScript for the things
JavaScript is genuinely good at: search, theme switching, replaying recorded
terminal sessions, hashing a downloaded file in the browser.

**The no-JavaScript edition** is `website/nojs/`, and it is supported rather
than tolerated. A reader with scripts off gets a real index of the whole
project, not an apology. `website/nojs/search.html` carries the entire corpus
**in the page**, so the browser's own find-in-page searches it, and it is
generated from the same walk as the machine-readable index the scripted search
fetches. The two cannot disagree, because one generator writes both.

Everything on the ordinary site that needs a script also has a `<noscript>`
answer, and the rule is the one `reveal.js` states about itself: content must
never stay invisible.

## What writes what

Almost nothing on this site is typed by hand. The generators are listed in
dependency order in `tools/verify.py`, and that order matters: the search index
is built last, after the SEO pass has edited the pages it indexes.

| Tool | Writes |
|---|---|
| `tools/docs/generate.py` | The reference: a page per crate and per source file, for the site, the repository and the wiki |
| `tools/docs/sources.py` | The syntax-highlighted source pages under `website/reference/` |
| `tools/docs/guides.py` | The per-program guides, assembled from `docs/USER_GUIDE.md` |
| `tools/docs/wiki.py` | The wiki's landing page, sidebar and document pages |
| `tools/site/split.py` | The section pages, split out of `index.html` |
| `tools/site/roadmap.py` | `roadmap.html`, from `ROADMAP.md` |
| `tools/site/faq.py` | `faq.html`, from `docs/FAQ.md` |
| `tools/site/releases.py` | `releases.html`, from `CHANGELOG.md` |
| `tools/site/demo.py` | `website/js/demo-data.js`, from the recorded sessions |
| `tools/site/seo.py` | Canonical addresses, the sitemap and `robots.txt` |
| `tools/search-index/generate.py` | `search-index.json` and `nojs/search.html` |

Each writes a header saying it is generated. Editing the output is wasted work:
the matching `--check` run in CI regenerates into memory and compares.

`website/nojs/index.html` is the one page written by hand rather than generated,
because it is a summary of the project rather than a view of something else.

## The scripts

Twelve modules, 3178 lines, no dependencies and nothing from a CDN.

| Module | What it does |
|---|---|
| `demo-data.js` | Generated. The facts the demonstration is drawn from |
| `legal.js` | The welcome dialogue: licence, liability waiver, and the AI-assistance disclosure |
| `markdown.js` | A small Markdown renderer and syntax highlighter, written rather than pulled from a CDN |
| `prefetch.js` | Quietly fetches the few pages a reader is most likely to open next |
| `repo.js` | Live repository data: stars, description, latest release, rendered README |
| `reveal.js` | Reveal-on-scroll, under one rule that outranks the effect: content must never stay invisible |
| `search.js` | Scores a query against the committed index |
| `sessions.js` | Replays five recordings of the real programs at typing speed |
| `teleport.js` | Makes a fragment link land on the heading it names, clear of the sticky header |
| `theme.js` | Nine palettes, Tokyo Night default, kept in `localStorage` and never sent anywhere |
| `verify.js` | SHA-256 of a downloaded archive, computed in the browser |
| `walkthrough.js` | Every screen of the application as a photograph, and the command line as a list of jobs |

**Why `markdown.js` exists at all.** Pulling `marked` and `highlight.js` off a
CDN would have been three lines. It would also have meant two more parties able
to change what this site executes, on a site whose subject is not trusting
people by default.

**Nothing here phones home.** `theme.js` keeps a preference in `localStorage`,
which stays in the browser. `verify.js` hashes the file you give it locally: the
file never leaves the machine. `repo.js` is the one module that fetches anything
across the network, and what it fetches is GitHub's public API about this
repository.

## Styles and palettes

`website/css/themes.css` holds nine palettes and `main.css` the layout. Both are
documented as source pages in the reference, the same as the scripts. The
palettes are shared with the desktop application, so a reader who picks one on
the site sees the same one in the program.

## Running it locally

```bash
python tools/site/serve.py
```

It serves `website/` with correct MIME types on a local port. There is an nginx
configuration alongside it for anybody who would rather use that.

The site tests check the pages without a browser:

```bash
node tools/site-tests/run.js
```

Those cover characters, structure, rendering, hostile input, the scroll reveal,
that every page is reachable, and that the local site serves every address the
sitemap claims.

## Addresses

Every page states its own canonical address and appears in `sitemap.xml`;
`robots.txt` allows every crawler and names the sitemap. `tools/site/seo.py`
writes all three, and `--check` fails if a page has been added without one.
