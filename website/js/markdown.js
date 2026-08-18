// SPDX-License-Identifier: CC-BY-NC-SA-4.0
//
// A small Markdown renderer and syntax highlighter.
//
// # Why not a library
//
// Pulling `marked` and `highlight.js` off a CDN would be three lines. It would
// also mean every visitor to a privacy tool's website makes requests to a third
// party that learns their IP address and what they were reading, and it would
// mean trusting code nobody here has read. This is a few hundred lines that
// does what this page needs and nothing else.
//
// # Escaping
//
// Everything is HTML-escaped *first*, and only the tags this renderer itself
// emits are ever introduced. Raw HTML in the source Markdown is shown as text
// rather than injected, so the README cannot inject script into this page even
// if it were altered upstream.

window.MD = (function () {
  "use strict";

  // Placeholders are single characters from the Unicode private-use area. See
  // `parker` below for why, and why the source is stripped of them first.
  var PARK_BASE = 0xe000;
  var PARK_LIMIT = 0xf8ff;
  var PARK_RE = new RegExp("[\uE000-\uF8FF]", "g");

  /**
   * Characters that are stripped outright rather than escaped, because they
   * have no legitimate place in rendered prose and are invisible when they go
   * wrong:
   *
   * - **C0 and C1 control characters**, apart from tab, newline and carriage
   *   return. A NUL reaching the output is not exploitable -- the HTML parser
   *   turns it into U+FFFD -- but it is a stray invisible character in a
   *   document, which is its own kind of wrong.
   * - **Bidirectional overrides and isolates** (U+202A-U+202E, U+2066-U+2069).
   *   These reorder how text is *displayed* without changing what it says, so
   *   a README could be made to read differently on the page than in the
   *   repository. That is the Trojan Source class of trick, and a site whose
   *   whole argument is "the rendered README is the real README" has a
   *   specific reason to care.
   *
   * Ordinary right-to-left text is untouched: it does not need these controls
   * to display correctly, only to be *overridden*.
   */
  var STRIP_RE = new RegExp(
    "[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F" +
      "\u202A-\u202E\u2066-\u2069]",
    "g"
  );

  function escapeHtml(text) {
    return String(text)
      .replace(PARK_RE, "")
      .replace(STRIP_RE, "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  /**
   * Set finished markup aside so later passes cannot chew on it.
   *
   * The placeholder has to satisfy three things at once, and the obvious
   * choice -- `" " + index + " "` -- satisfies none of them:
   *
   *   1. **Later passes must not match it.** The number highlighter
   *      (`\b\d+\b`) matched the index inside a space-delimited placeholder and
   *      wrapped it in a span. The un-parking pass then no longer recognised
   *      it, so the parked content was *discarded*: every string literal in
   *      every code block rendered as a stray highlighted digit. A private-use
   *      character is not a word character, not a digit and not a quote, so no
   *      pass in this file matches it.
   *   2. **The source must not be able to forge one.** `escapeHtml` strips the
   *      private-use range before anything else runs, so a placeholder in the
   *      working string can only be one this code put there.
   *   3. **Adjacent placeholders must both survive.** The old form ate its own
   *      delimiters -- the space closing one placeholder was the space opening
   *      the next, so alternate items silently failed to come back. A single
   *      character has no delimiters to share.
   */
  function parker() {
    var parked = [];
    return {
      park: function (markup) {
        if (parked.length > PARK_LIMIT - PARK_BASE) { return markup; }
        parked.push(markup);
        return String.fromCharCode(PARK_BASE + parked.length - 1);
      },
      /**
       * Put every parked fragment back, including the ones nested inside
       * others.
       *
       * Parked markup can itself contain a placeholder: `[`code`](url)` parks
       * the inline code first, and then parks an anchor whose label *is* that
       * placeholder. `String.replace` does not rescan its own replacement
       * text, so a single pass emitted `<a href="url">` wrapped around a bare
       * private-use character -- which browsers draw as nothing. Every link in
       * the README whose label was inline code rendered as an empty link, so
       * "see [`docs/AUDIT.md`](docs/AUDIT.md)." came out as "see .".
       *
       * Looping until the string stops changing fixes it. The bound is a
       * guard, not a limit: nesting here is two deep at most, and comparing
       * the result is a more honest termination condition than trusting that.
       */
      unpark: function (html) {
        for (var depth = 0; depth < 8; depth++) {
          var next = html.replace(PARK_RE, function (ch) {
            var index = ch.charCodeAt(0) - PARK_BASE;
            return index < parked.length ? parked[index] : "";
          });
          if (next === html) { return next; }
          html = next;
        }
        return html;
      }
    };
  }

  // --- syntax highlighting -------------------------------------------------

  /**
   * Keyword patterns by language.
   *
   * A null-prototype object, and looked up through `hasOwnProperty` below,
   * because the key comes from the source document: the fence info string is
   * matched with `\w*`, and `constructor` and `__proto__` are both `\w*`. On a
   * plain object literal, ```` ```constructor ```` made `KEYWORDS[lang]`
   * resolve to `Object` through the prototype chain, and ```` ```__proto__ ````
   * resolved to `Object.prototype`.
   *
   * Neither did any harm as the code stood -- `String.replace` stringifies a
   * non-regex search value, so it looked for the literal text
   * `function Object() { [native code] }` and found nothing. But "an attacker
   * can steer this lookup onto `Object.prototype` and the result happens to be
   * inert" is a description of a bug that has not gone off yet, not of a safe
   * lookup. A document must not be able to reach the prototype chain at all.
   */
  var KEYWORDS = Object.assign(Object.create(null), {
    rust: /\b(fn|let|mut|pub|use|mod|struct|enum|impl|trait|for|in|if|else|match|return|const|static|crate|self|super|where|as|dyn|move|async|await|ref|type|unsafe)\b/g,
    bash: /\b(if|then|else|fi|for|while|do|done|case|esac|function|return|export|local|source|cd|echo|cat|sudo|git|cargo|gpg|sha256sum|python|python3|tar|unzip|curl|wget)\b/g,
    toml: /^\s*\[[^\]]+\]/gm,
    json: /"(?:[^"\\]|\\.)*"(?=\s*:)/g
  });

  /** Own-property lookup, so a document cannot reach `Object.prototype`. */
  function keywordsFor(lang) {
    return Object.prototype.hasOwnProperty.call(KEYWORDS, lang) ? KEYWORDS[lang] : null;
  }

  function highlight(code, language) {
    var html = escapeHtml(code);
    var lang = (language || "").toLowerCase();

    // Strings and comments first, then protect them from later passes by
    // parking the markup behind placeholders. Highlighting a keyword that
    // happens to sit inside a string is the classic way these break.
    var store = parker();
    var park = store.park;

    html = html.replace(/(&quot;(?:[^&]|&(?!quot;))*&quot;|'[^']*')/g, function (m) {
      return park('<span class="tok-str">' + m + "</span>");
    });

    if (lang === "rust" || lang === "toml" || lang === "js" || lang === "javascript") {
      html = html.replace(/(\/\/[^\n]*|#[^\n]*)/g, function (m) {
        return park('<span class="tok-com">' + m + "</span>");
      });
    } else if (lang === "bash" || lang === "sh" || lang === "shell" || lang === "yaml" || lang === "") {
      html = html.replace(/(#[^\n]*)/g, function (m) {
        return park('<span class="tok-com">' + m + "</span>");
      });
    }

    var keywords = keywordsFor(lang);
    if (keywords) {
      html = html.replace(keywords, '<span class="tok-kw">$&</span>');
    }
    if (lang === "rust" || lang === "js" || lang === "javascript") {
      html = html.replace(/\b([a-z_][a-z0-9_]*)\s*\(/gi, '<span class="tok-fn">$1</span>(');
    }
    html = html.replace(/\b(\d+(?:\.\d+)?)\b/g, '<span class="tok-num">$1</span>');

    return store.unpark(html);
  }

  // --- URLs ----------------------------------------------------------------

  /**
   * Whether a link or image target may be emitted at all.
   *
   * The rule is: if the target names a scheme, it must be http or https;
   * if it names no scheme, it is a relative path and is fine. That is an
   * allowlist over schemes -- `javascript:`, `data:`, `vbscript:` and anything
   * invented after this was written are all refused by default, because they
   * are not on the list rather than because someone remembered to name them.
   *
   * Two things are excluded that look relative and are not:
   *
   * - `//host/path`, which is protocol-relative and goes off-site. Treating it
   *   as internal is exactly how a link ends up without
   *   `rel="noopener noreferrer"` and leaks a referrer.
   * - a leading backslash, which several browsers normalise to `/` -- so
   *   `\\host` becomes protocol-relative by the back door.
   *
   * The previous version tested `^(?:https?:|[./#])`, which refused any path
   * not starting with `.`, `/` or `#`. That is most ordinary Markdown links:
   * `[whitepaper](docs/WHITEPAPER.md)` silently rendered as plain text. Safe,
   * but wrong, and quietly wrong -- the worst combination for a page whose
   * whole argument is "go and read the source".
   */
  function safeUrl(url) {
    if (/^[\\/]{2}/.test(url) || /^\\/.test(url)) { return false; }
    var scheme = /^([a-zA-Z][a-zA-Z0-9+.-]*):/.exec(url);
    return scheme ? /^https?$/i.test(scheme[1]) : true;
  }

  function isExternal(url) {
    return /^https?:/i.test(url);
  }

  /**
   * Link and image targets, written so the engine cannot backtrack over them.
   *
   * The previous pair were `\(([^)\s]+)[^)]*\)`. Those two runs **overlap**: a
   * character that is neither `)` nor whitespace can be taken by either one, so
   * for a `(` that never closes, the engine tries every way of splitting the
   * text between them. That is quadratic, and it was measured rather than
   * guessed at -- `![a](` followed by a run of ordinary characters:
   *
   * | characters | time to render |
   * |---|---|
   * | 16 000 | 0.13 s |
   * | 32 000 | 0.49 s |
   * | 64 000 | 1.96 s |
   * | 128 000 | 7.97 s |
   *
   * Four times the work for twice the input, exactly. A 400 KB document on one
   * line is a minute and a half of a frozen tab, on the main thread, from text
   * this page **fetched over the network** -- which is the whole reason the
   * renderer is treated as a security boundary.
   *
   * The fix is to make the two runs disjoint rather than to bound the input:
   * the optional title must *begin* with whitespace, so no character can belong
   * to both parts and there is only one way to split any given input. The
   * grammar accepted is unchanged.
   */
  /**
   * The runs are also **bounded**, which fixes a second, separate quadratic.
   *
   * Removing the ambiguity above stops the engine trying every split of one
   * attempt. It does not stop it making a great many attempts: for
   * `[[[[...]([]([...`, every `[` is a candidate start, and at each one an
   * unbounded `[^\]]+` scans forward until it finds a `]`. That is n starts
   * times an n-character scan, and it was measured too:
   *
   * | `[` repeated | then `](` repeated | time |
   * |---|---|---|
   * | 10 000 | 10 000 | 0.23 s |
   * | 20 000 | 20 000 | 0.90 s |
   * | 40 000 | 40 000 | 3.61 s |
   * | 80 000 | 80 000 | 14.59 s |
   *
   * A repetition bound turns each scan into a constant, so the whole pass
   * becomes linear in the document. The limits are far past anything real -- a
   * link label of five hundred characters is a paragraph, and the longest URL
   * in this project's Markdown is sixty-odd characters. Past them the text is
   * rendered as the plain text it is, which is a visible, harmless outcome; a
   * tab frozen for fifteen seconds is neither.
   */
  var MAX_LABEL = 512;
  var MAX_TARGET = 2048;
  var MAX_TITLE = 512;

  var TARGET_IMAGE = new RegExp(
    "!\\[([^\\]]{0," + MAX_LABEL + "})\\]" +
      "\\(([^)\\s]{1," + MAX_TARGET + "})(?:\\s[^)]{0," + MAX_TITLE + "})?\\)",
    "g"
  );
  var TARGET_LINK = new RegExp(
    "\\[([^\\]]{1," + MAX_LABEL + "})\\]" +
      "\\(([^)\\s]{1," + MAX_TARGET + "})(?:\\s[^)]{0," + MAX_TITLE + "})?\\)",
    "g"
  );

  /**
   * Inline code, bounded for the same reason: an unclosed backtick followed by
   * a long run makes every later backtick a fresh start position.
   */
  var INLINE_CODE = new RegExp("`([^`]{1,4096})`", "g");

  // --- inline --------------------------------------------------------------

  function inline(text) {
    var out = escapeHtml(text);
    var store = parker();
    var park = store.park;

    // Inline code first: its contents must not be interpreted as emphasis.
    out = out.replace(INLINE_CODE, function (_, code) {
      return park("<code>" + code + "</code>");
    });

    // Images and links share one scheme allowlist. They did not always: the
    // image branch used to interpolate whatever was in the parentheses, so
    // `![x](javascript:...)` produced `src="javascript:..."`. No current
    // browser executes a `javascript:` image source, which is why it went
    // unnoticed, but "no browser we tested still honours this" is not a
    // security argument. One rule, applied in both places.
    out = out.replace(TARGET_IMAGE, function (_, alt, src) {
      if (!safeUrl(src)) { return alt; }
      return park('<img src="' + encodeURI(src) + '" alt="' + alt + '">');
    });

    out = out.replace(TARGET_LINK, function (_, label, href) {
      if (!safeUrl(href)) { return label; }
      return park(
        '<a href="' + encodeURI(href) + '"' +
        (isExternal(href) ? ' rel="noopener noreferrer"' : "") + ">" + label + "</a>"
      );
    });

    out = out.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    out = out.replace(/(^|[\s(])\*([^*\n]+)\*/g, "$1<em>$2</em>");
    out = out.replace(/(^|[\s(])_([^_\n]+)_/g, "$1<em>$2</em>");

    return store.unpark(out);
  }

  // --- block ---------------------------------------------------------------

  /**
   * How deeply blockquotes may nest before the rest is rendered as plain text.
   *
   * A blockquote strips one `>` and calls `render` again, so nesting depth is
   * recursion depth and the document chooses it. A line of five thousand `>`
   * characters overflowed the JavaScript stack and threw a `RangeError` -- and
   * because `repo.js` reports any rejection from the README fetch as "could not
   * reach api.github.com", the user was given a confidently wrong explanation
   * for a page that had loaded fine and then broken while rendering.
   *
   * Sixteen is far past any real document; the deepest quote in this project's
   * own Markdown is two.
   */
  var MAX_QUOTE_DEPTH = 16;

  function render(source, depth) {
    depth = depth || 0;
    var lines = String(source).replace(/\r\n?/g, "\n").split("\n");
    var html = [];
    var i = 0;

    function tableRow(line) {
      return line.trim().indexOf("|") === 0 || /\s\|\s/.test(line);
    }

    while (i < lines.length) {
      var line = lines[i];

      // fenced code
      var fence = line.match(/^```(\w*)/);
      if (fence) {
        var lang = fence[1];
        var body = [];
        i++;
        while (i < lines.length && !/^```/.test(lines[i])) { body.push(lines[i]); i++; }
        i++;
        html.push("<pre><code>" + highlight(body.join("\n"), lang) + "</code></pre>");
        continue;
      }

      // headings -- h1 is skipped because the page supplies its own title
      var heading = line.match(/^(#{1,6})\s+(.*)$/);
      if (heading) {
        var level = heading[1].length;
        html.push("<h" + level + ">" + inline(heading[2]) + "</h" + level + ">");
        i++;
        continue;
      }

      if (/^\s*([-*_])\s*\1\s*\1[\s-*_]*$/.test(line)) { html.push("<hr>"); i++; continue; }

      // blockquote
      if (/^>\s?/.test(line)) {
        var quote = [];
        while (i < lines.length && /^>\s?/.test(lines[i])) {
          quote.push(lines[i].replace(/^>\s?/, ""));
          i++;
        }
        if (depth >= MAX_QUOTE_DEPTH) {
          // Past the limit the remaining `>` are shown as the text they are,
          // rather than recursed into. Escaped, so this is still only ever the
          // renderer's own markup reaching the page.
          html.push("<p>" + inline(quote.join(" ")) + "</p>");
        } else {
          html.push(
            "<blockquote>" + render(quote.join("\n"), depth + 1) + "</blockquote>"
          );
        }
        continue;
      }

      // table: a header row followed by a --- separator
      if (tableRow(line) && i + 1 < lines.length && /^[\s|:-]+$/.test(lines[i + 1]) &&
          lines[i + 1].indexOf("-") !== -1) {
        var cells = function (row) {
          return row.replace(/^\s*\|/, "").replace(/\|\s*$/, "").split("|")
                    .map(function (c) { return inline(c.trim()); });
        };
        var table = ["<table><thead><tr>"];
        cells(line).forEach(function (c) { table.push("<th>" + c + "</th>"); });
        table.push("</tr></thead><tbody>");
        i += 2;
        while (i < lines.length && tableRow(lines[i]) && lines[i].trim() !== "") {
          table.push("<tr>");
          cells(lines[i]).forEach(function (c) { table.push("<td>" + c + "</td>"); });
          table.push("</tr>");
          i++;
        }
        table.push("</tbody></table>");
        html.push(table.join(""));
        continue;
      }

      // lists
      if (/^\s*(?:[-*+]|\d+\.)\s+/.test(line)) {
        var ordered = /^\s*\d+\./.test(line);
        var items = [];
        while (i < lines.length && /^\s*(?:[-*+]|\d+\.)\s+/.test(lines[i])) {
          items.push(inline(lines[i].replace(/^\s*(?:[-*+]|\d+\.)\s+/, "")));
          i++;
          // continuation lines belong to the item above
          while (i < lines.length && /^\s{2,}\S/.test(lines[i]) &&
                 !/^\s*(?:[-*+]|\d+\.)\s+/.test(lines[i])) {
            items[items.length - 1] += " " + inline(lines[i].trim());
            i++;
          }
        }
        var tag = ordered ? "ol" : "ul";
        html.push("<" + tag + ">" + items.map(function (t) {
          return "<li>" + t + "</li>";
        }).join("") + "</" + tag + ">");
        continue;
      }

      // HTML comments and raw block tags in the source are dropped rather than
      // passed through, so nothing upstream can inject markup into this page.
      if (/^\s*<!--/.test(line)) {
        while (i < lines.length && lines[i].indexOf("-->") === -1) { i++; }
        i++;
        continue;
      }
      if (/^\s*<\/?(p|div|img|br|hr|center|table|h[1-6])\b/i.test(line)) { i++; continue; }

      if (line.trim() === "") { i++; continue; }

      // paragraph
      var para = [];
      while (i < lines.length && lines[i].trim() !== "" &&
             !/^(#{1,6}\s|```|>|\s*(?:[-*+]|\d+\.)\s)/.test(lines[i])) {
        para.push(lines[i]);
        i++;
      }
      if (para.length) { html.push("<p>" + inline(para.join(" ")) + "</p>"); }
    }

    return html.join("\n");
  }

  // `safeUrl` is exported so that `repo.js` can apply the *same* rule to the
  // URLs it takes from the GitHub API. Two independently written scheme checks
  // on one page is two things to keep in step, and the one that gets forgotten
  // is the one that matters.
  return { render: render, highlight: highlight, escape: escapeHtml, safeUrl: safeUrl };
})();
