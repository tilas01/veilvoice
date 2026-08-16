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

  function loadRelease() {
    return fetch(API + "/releases/latest", { headers: { Accept: "application/vnd.github+json" } })
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (data) {
        if (!data) { return; }
        text("latest-tag", data.tag_name || "--");
        var list = document.getElementById("asset-list");
        if (!list || !data.assets) { return; }
        list.innerHTML = "";
        data.assets
          .slice()
          .sort(function (a, b) { return a.name.localeCompare(b.name); })
          .forEach(function (asset) {
            var li = document.createElement("li");
            var link = document.createElement("a");
            link.href = asset.browser_download_url;
            link.textContent = asset.name;
            link.rel = "noopener noreferrer";
            var size = document.createElement("span");
            size.style.color = "var(--muted)";
            size.textContent = asset.size
              ? "  (" + (asset.size / 1048576).toFixed(1) + " MB)"
              : "";
            li.appendChild(link);
            li.appendChild(size);
            list.appendChild(li);
          });
      });
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
        target.innerHTML = window.MD.render(markdown);
        // The README's own banner is already the page hero; showing it twice
        // just pushes the content down.
        var first = target.querySelector("img");
        if (first && /banner/.test(first.getAttribute("src") || "")) {
          first.remove();
        }
        // Repo-relative links must resolve against GitHub, not this site.
        target.querySelectorAll("a[href]").forEach(function (a) {
          var href = a.getAttribute("href");
          if (href && !/^https?:/i.test(href) && href.charAt(0) !== "#") {
            a.setAttribute(
              "href",
              "https://github.com/" + OWNER + "/" + REPO + "/blob/main/" +
                href.replace(/^\.?\//, "")
            );
            a.setAttribute("rel", "noopener noreferrer");
          }
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
