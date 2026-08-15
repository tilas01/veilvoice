// SPDX-License-Identifier: GPL-3.0-or-later
//
// Live repository data: stars, description, latest release, and the README
// rendered with syntax highlighting.
//
// # The one third-party request on this site, and why it is opt-in
//
// Everything else here is served from the same origin. This module talks to
// api.github.com, which learns your IP address and that you looked at this
// project. GitHub already knows both — it is serving the page you are reading —
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
    return typeof value === "number" ? value.toLocaleString() : "—";
  }

  function loadMeta() {
    return fetch(API, { headers: { Accept: "application/vnd.github+json" } })
      .then(function (r) {
        if (!r.ok) { throw new Error("GitHub returned " + r.status); }
        return r.json();
      })
      .then(function (data) {
        text("stars", number(data.stargazers_count));
        text("forks", number(data.forks_count));
        text("issues", number(data.open_issues_count));
        text("repo-desc", data.description || "");
        if (data.license && data.license.spdx_id) {
          text("repo-license", data.license.spdx_id);
        }
      });
  }

  function loadRelease() {
    return fetch(API + "/releases/latest", { headers: { Accept: "application/vnd.github+json" } })
      .then(function (r) { return r.ok ? r.json() : null; })
      .then(function (data) {
        if (!data) { return; }
        text("latest-tag", data.tag_name || "—");
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
      });
  }

  function start() {
    var status = document.getElementById("repo-status");
    if (status) { status.textContent = "loading from api.github.com …"; }

    Promise.allSettled([loadMeta(), loadRelease(), loadReadme()]).then(function (results) {
      var failed = results.filter(function (r) { return r.status === "rejected"; });
      if (!status) { return; }
      if (failed.length === results.length) {
        status.textContent =
          "could not reach api.github.com — the project page on GitHub has the same information.";
      } else {
        status.textContent = "";
      }
    });
  }

  document.addEventListener("DOMContentLoaded", function () {
    var button = document.getElementById("load-repo");
    if (button) {
      button.addEventListener("click", function () {
        button.disabled = true;
        button.textContent = "loading …";
        start();
      });
    }
  });
})();
