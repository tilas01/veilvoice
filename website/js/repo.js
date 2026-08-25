// SPDX-License-Identifier: GPL-3.0-or-later
//
// Live repository data: stars, description, latest release, and the README
// rendered with syntax highlighting.
//
// # The one third-party request on this site, and why it is opt-in
//
// Everything else here is served from the same origin. This module talks to
// api.github.com, which learns your IP address and that you looked at this
// project. GitHub already knows both -- it is serving the page you are reading --
// so the marginal cost is nil for most visitors. But someone reading over Tor
// or a mirror is in a different position, so the fetch is announced in the page
// and can be skipped: the panel degrades to static text and a plain link, and
// nothing else on the site depends on it.
//
// No token, no cookies, no credentials. Unauthenticated GitHub API requests are
// rate-limited by IP to 60/hour, which a documentation page will never approach.
//
// In plain words
//
// This is the panel showing the project's stars, its latest release and its
// README, read from GitHub as you look at it.
//
// It is the only thing on this site that talks to anybody else's server, and
// the page says so before it does. GitHub learns your address and that you
// looked at this project -- which it already knows, because it is serving you
// this page. If you would rather it did not happen at all, the panel turns
// into a plain link and nothing else on the site notices.

(function () {
  "use strict";

  var OWNER = "tilas01";
  var REPO = "veilvoice";
  var API = "https://api.github.com/repos/" + OWNER + "/" + REPO;
  var RAW = "https://raw.githubusercontent.com/" + OWNER + "/" + REPO + "/main/README.md";

  function text(id, value) {
    var node = document.getElementById(id);
    if (node) { node.textContent = value; }
  }

  function number(value) {
    return typeof value === "number" ? value.toLocaleString() : "--";
  }

  /** Whether the reader has asked the system for less movement. */
  function still() {
    return window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  /**
   * Give a piece of the panel its entrance.
   *
   * `--i` staggers the four stat tiles so they arrive in sequence rather than
   * snapping in together, which is the difference between "the data loaded"
   * and "the data arrived".
   */
  function arrive(node, index) {
    if (!node) { return; }
    node.style.setProperty("--i", index || 0);
    // Restarting the animation needs the class gone for a frame, or a second
    // load after a failed one would not replay it.
    node.classList.remove("repo-in");
    void node.offsetWidth;
    node.classList.add("repo-in");
  }

  /**
   * Count a figure up to its value instead of snapping to it.
   *
   * Eased, and capped at a fixed duration whatever the number, so a repository
   * with four stars and one with forty thousand both take the same short
   * moment. Driven by requestAnimationFrame and finished by writing the exact
   * value, so the number on screen at the end is the real one rather than
   * wherever the easing happened to land.
   */
  function countTo(id, value) {
    var node = document.getElementById(id);
    if (!node) { return; }
    if (typeof value !== "number" || !isFinite(value)) {
      node.textContent = "--";
      return;
    }
    if (still() || value === 0) {
      node.textContent = value.toLocaleString();
      return;
    }

    var DURATION = 650;
    var started = null;
    function frame(now) {
      if (started === null) { started = now; }
      var t = Math.min(1, (now - started) / DURATION);
      // Ease out cubic: quick at first, settling rather than stopping dead.
      var eased = 1 - Math.pow(1 - t, 3);
      node.textContent = Math.round(value * eased).toLocaleString();
      if (t < 1) { window.requestAnimationFrame(frame); }
      else { node.textContent = value.toLocaleString(); }
    }
    window.requestAnimationFrame(frame);
  }

  function loadMeta() {
    return fetch(API, { headers: { Accept: "application/vnd.github+json" } })
      .then(function (r) {
        if (!r.ok) { throw new Error("GitHub returned " + r.status); }
        return r.json();
      })
      .then(function (data) {
        countTo("stars", data.stargazers_count);
        countTo("forks", data.forks_count);
        countTo("issues", data.open_issues_count);
        text("repo-desc", data.description || "");
        if (data.license && data.license.spdx_id) {
          text("repo-license", data.license.spdx_id);
        }
        var tiles = document.querySelectorAll(".stats .stat");
        for (var i = 0; i < tiles.length; i++) { arrive(tiles[i], i); }
        arrive(document.getElementById("repo-desc"), tiles.length);
      });
  }

  /**
   * The most release assets that will be turned into DOM nodes.
   *
   * The count comes from the API response rather than from this page, and a
   * release currently has nine binaries plus their checksums. A hundred is far
   * past that and stops a malformed or hostile response from being turned into
   * an unbounded number of elements.
   */
  var MAX_ASSETS = 100;

  /**
   * Whether a download URL may be put in an `href` at all.
   *
   * The rule is the renderer's own `safeUrl`, deliberately shared rather than
   * re-implemented, plus a requirement that the scheme actually be present and
   * `https:` -- a release asset is always an absolute GitHub URL, so anything
   * else is wrong whatever else it might be.
   *
   * This value comes from the GitHub API for this repository, so it is not a
   * live route today; the audit recorded it as "trusted by omission", which is
   * a different thing from trusted. A response is a response: an API that
   * returned `javascript:...` here would have had it assigned straight into a
   * clickable link on a page whose entire subject is not trusting remote code.
   * The fallback when a URL is refused is to show the asset name as plain text,
   * so a reader still learns the file exists and can find it on GitHub.
   */
  function safeAssetUrl(url) {
    if (typeof url !== "string" || url === "") { return false; }
    if (window.MD && typeof window.MD.safeUrl === "function" && !window.MD.safeUrl(url)) {
      return false;
    }
    return /^https:\/\//i.test(url);
  }

  function loadRelease() {
    return fetch(API + "/releases/latest", { headers: { Accept: "application/vnd.github+json" } })
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (data) {
        if (!data) { return; }
        text("latest-tag", typeof data.tag_name === "string" ? data.tag_name : "--");
        var list = document.getElementById("asset-list");
        if (!list || !Array.isArray(data.assets)) { return; }
        list.innerHTML = "";
        data.assets
          .slice(0, MAX_ASSETS)
          // `String(...)` before comparing: `localeCompare` on a value that is
          // not a string throws, and one bad entry would reject the whole
          // promise and be reported to the reader as a network failure.
          .sort(function (a, b) {
            return String(a && a.name).localeCompare(String(b && b.name));
          })
          .forEach(function (asset) {
            if (!asset || typeof asset.name !== "string") { return; }
            var li = document.createElement("li");
            var label;
            if (safeAssetUrl(asset.browser_download_url)) {
              label = document.createElement("a");
              label.href = asset.browser_download_url;
              label.rel = "noopener noreferrer";
            } else {
              // Named, but not made clickable.
              label = document.createElement("span");
              label.title = "this download URL was not an https link and is not linked";
            }
            label.textContent = asset.name;
            var size = document.createElement("span");
            size.style.color = "var(--muted)";
            size.textContent = typeof asset.size === "number" && isFinite(asset.size)
              ? "  (" + (asset.size / 1048576).toFixed(1) + " MB)"
              : "";
            li.appendChild(label);
            li.appendChild(size);
            list.appendChild(li);
          });
      });
  }

  /**
   * The largest README this page will render, in characters.
   *
   * This project's own is about thirteen kilobytes. A megabyte is seventy times
   * that and still renders in well under a frame; past it the document is not
   * a README, and rendering happens on the main thread, so the honest answer is
   * to say so and link to GitHub rather than to spend an unbounded amount of
   * the reader's time on it.
   *
   * The renderer itself is linear in its input now -- see the two quadratics
   * fixed in `markdown.js` -- so this is defence in depth against the next one
   * rather than the mitigation for those.
   */
  var MAX_README_CHARS = 1024 * 1024;

  /**
   * Remove block-level raw HTML from a Markdown document.
   *
   * `markdown.js` escapes raw HTML rather than emitting it, which is the
   * property that makes it safe to hand its output to `innerHTML`. The
   * consequence is that a README opening with a centred banner --
   * `<p align="center"><picture>...</picture></p>`, which is how GitHub wants
   * one written -- rendered as a paragraph of escaped tag soup above the
   * project's own name. That was live on the site.
   *
   * Neither half of that is a bug on its own. Together they are, and the fix
   * belongs here rather than in the renderer: block-level markup in somebody
   * else's README is presentation, and this panel is showing the prose. The
   * renderer keeps escaping everything, exactly as before.
   *
   * The rule is CommonMark's, simplified to the two block kinds that actually
   * occur: a comment runs to `-->`, and any other HTML block runs to the next
   * blank line. Fenced code is left alone -- a fence full of markup is an
   * example being shown deliberately, which is the distinction
   * `links.test.js` and the hostile-input suite both had to learn.
   *
   * One pass, no backtracking. Every regular expression here is anchored and
   * runs against a single line, because this text arrives over the network and
   * two quadratics in this file's neighbourhood already froze a reader's tab
   * for eight seconds (F-22, F-23).
   */
  var HTML_BLOCK_START = /^<(?:!--|\/?[a-zA-Z][a-zA-Z0-9-]*)/;
  var FENCE = /^(?:```|~~~)/;

  function stripHtmlBlocks(markdown) {
    var lines = markdown.split("\n");
    var out = [];
    var i = 0;
    var inFence = false;
    var fence = "";
    while (i < lines.length) {
      var line = lines[i];
      var trimmed = line.replace(/^[ \t]+/, "");
      if (inFence) {
        if (trimmed.indexOf(fence) === 0) { inFence = false; }
        out.push(line);
        i++;
        continue;
      }
      if (FENCE.test(trimmed)) {
        inFence = true;
        fence = trimmed.slice(0, 3);
        out.push(line);
        i++;
        continue;
      }
      if (HTML_BLOCK_START.test(trimmed)) {
        if (trimmed.indexOf("<!--") === 0) {
          while (i < lines.length && lines[i].indexOf("-->") === -1) { i++; }
          i++;                       // the line carrying the terminator
        } else {
          while (i < lines.length && lines[i].trim() !== "") { i++; }
        }
        continue;
      }
      out.push(line);
      i++;
    }
    return out.join("\n");
  }


  function loadReadme() {
    return fetch(RAW)
      .then(function (r) {
        if (!r.ok) { throw new Error("README unavailable (" + r.status + ")"); }
        return r.text();
      })
      .then(function (markdown) {
        var target = document.getElementById("readme");
        if (!target) { return; }
        if (typeof markdown !== "string") { return; }
        if (markdown.length > MAX_README_CHARS) {
          target.textContent =
            "The README is unusually large (" +
            Math.round(markdown.length / 1024) +
            " KB) and has not been rendered here. Read it on GitHub.";
          return;
        }
        target.innerHTML = window.MD.render(stripHtmlBlocks(markdown));
        // The README's own banner is already the page hero; showing it twice
        // just pushes the content down.
        var first = target.querySelector("img");
        if (first && /banner/.test(first.getAttribute("src") || "")) {
          first.remove();
        }
        // Repo-relative links must resolve against GitHub, not this site.
        //
        // Built through `URL` against a fixed base rather than by pasting
        // strings together. String concatenation left `..` segments in the
        // result for the browser to resolve afterwards, so a link written as
        // `[x](../../../elsewhere)` produced a URL that normalised to a
        // different part of github.com than the one it appeared to point at.
        // The host was never in doubt, so this was misdirection rather than
        // escape -- but a page asking people to click through to source they
        // are being told to read should send them where the link says.
        //
        // `Array.prototype.slice.call` rather than `NodeList.forEach`: the
        // latter is missing on older WebKit, and this file has to run there.
        var base = "https://github.com/" + OWNER + "/" + REPO + "/blob/main/";
        var anchors = Array.prototype.slice.call(target.querySelectorAll("a[href]"));
        anchors.forEach(function (a) {
          var href = a.getAttribute("href");
          if (!href || /^https?:/i.test(href) || href.charAt(0) === "#") { return; }
          var resolved;
          try {
            resolved = new URL(href.replace(/^\.?\//, ""), base);
          } catch (e) {
            return; // Not resolvable: leave it exactly as the renderer emitted it.
          }
          // Refuse anything that climbed out of the repository, or that a
          // scheme in the target steered off github.com altogether.
          if (resolved.origin !== "https://github.com" ||
              resolved.href.indexOf(base) !== 0) {
            return;
          }
          a.setAttribute("href", resolved.href);
          a.setAttribute("rel", "noopener noreferrer");
        });
        arrive(target, 0);
      });
  }

  /** Put the button back in a state the reader can act on. */
  function settleButton(button, failedEverything) {
    if (!button) { return; }
    button.textContent = failedEverything ? "try again" : "reload live data";
    button.disabled = false;
  }

  function start(button) {
    var status = document.getElementById("repo-status");
    var panel = document.getElementById("repo");
    if (status) { status.textContent = "loading from api.github.com ..."; }
    // Drives the pulse on the figures, so the panel looks busy rather than
    // looking like four em dashes are the answer.
    if (panel) { panel.classList.add("repo-loading"); }

    Promise.allSettled([loadMeta(), loadRelease(), loadReadme()]).then(function (results) {
      if (panel) { panel.classList.remove("repo-loading"); }
      var failed = results.filter(function (r) { return r.status === "rejected"; });
      var allFailed = failed.length === results.length;
      settleButton(button, allFailed);
      if (!status) { return; }
      if (allFailed) {
        status.textContent =
          "could not reach api.github.com -- the project page on GitHub has the same information.";
      } else {
        status.textContent = "";
      }
    });
  }

  document.addEventListener("DOMContentLoaded", function () {
    var button = document.getElementById("load-repo");
    if (!button) { return; }
    // The fetch stays opt-in: this panel is the one third-party request on the
    // site, and whether to make it is the reader's call, not ours.
    button.addEventListener("click", function () {
      button.disabled = true;
      // A spinner on the control that was pressed, rather than a message
      // somewhere else on the page.
      button.textContent = "";
      var spinner = document.createElement("span");
      spinner.className = "spin";
      button.appendChild(spinner);
      button.appendChild(document.createTextNode("loading"));
      start(button);
    });
  });
})();
