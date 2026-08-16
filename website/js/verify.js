// SPDX-License-Identifier: GPL-3.0-or-later
//
// In-browser SHA-256 verification for downloaded release archives.
//
// # The file never leaves your machine
//
// Hashing happens locally through WebCrypto (`crypto.subtle.digest`), which is
// built into the browser. The file is read with FileReader, hashed in memory,
// and discarded. There is no upload, no fetch, no XHR, and no server that
// could receive it -- you can confirm that by reading this file, which is the
// whole of the implementation.
//
// # Why it streams
//
// A release archive can be tens of megabytes and WebCrypto has no incremental
// digest API, so the whole file must be in memory at once for `digest()`.
// Reading it in chunks first lets the progress bar move and keeps the tab
// responsive, rather than freezing until the browser finishes.

(function () {
  "use strict";

  var CHUNK = 4 * 1024 * 1024;

  function hex(buffer) {
    var bytes = new Uint8Array(buffer);
    var out = "";
    for (var i = 0; i < bytes.length; i++) {
      out += bytes[i].toString(16).padStart(2, "0");
    }
    return out;
  }

  function readFile(file, onProgress) {
    return new Promise(function (resolve, reject) {
      var chunks = [];
      var offset = 0;
      var reader = new FileReader();

      reader.onerror = function () { reject(new Error("could not read the file")); };
      reader.onload = function (event) {
        chunks.push(new Uint8Array(event.target.result));
        offset += event.target.result.byteLength;
        onProgress(Math.min(1, offset / (file.size || 1)));
        if (offset < file.size) { next(); } else { resolve(join(chunks, offset)); }
      };

      function next() {
        reader.readAsArrayBuffer(file.slice(offset, offset + CHUNK));
      }
      if (file.size === 0) { resolve(new Uint8Array(0)); } else { next(); }
    });
  }

  function join(chunks, total) {
    var all = new Uint8Array(total);
    var at = 0;
    chunks.forEach(function (c) { all.set(c, at); at += c.length; });
    return all;
  }

  /** Normalise whatever the user pasted down to a bare hex digest.
   *  Accepts a raw hash, or a whole `SHA256SUMS` line like
   *  "<hash>  veilvoice-v0.1.1-linux-x86_64.tar.gz". */
  function expectedFrom(text) {
    var match = String(text).toLowerCase().match(/\b[0-9a-f]{64}\b/);
    return match ? match[0] : "";
  }

  document.addEventListener("DOMContentLoaded", function () {
    var drop = document.getElementById("drop");
    var picker = document.getElementById("file");
    var expected = document.getElementById("expected");
    var output = document.getElementById("digest");
    var verdict = document.getElementById("verdict");
    var bar = document.getElementById("progress");
    if (!drop || !picker) { return; }

    function compare() {
      var want = expectedFrom(expected.value);
      var got = (output.dataset.hash || "").toLowerCase();
      verdict.className = "verdict";
      if (!got || !want) { verdict.textContent = ""; verdict.style.display = "none"; return; }
      verdict.style.display = "block";
      if (want === got) {
        verdict.classList.add("match");
        verdict.textContent = "MATCH -- this file is byte-for-byte what the hash describes.";
      } else {
        verdict.classList.add("fail");
        verdict.textContent =
          "NO MATCH -- do not run this file. It is not the release it claims to be, " +
          "or the download was corrupted.";
      }
    }

    function handle(file) {
      if (!file) { return; }
      verdict.style.display = "none";
      output.dataset.hash = "";
      output.textContent = "reading " + file.name + " ...";
      bar.style.display = "block";
      bar.value = 0;

      readFile(file, function (fraction) { bar.value = fraction * 0.8; })
        .then(function (bytes) {
          output.textContent = "hashing ...";
          bar.value = 0.9;
          return crypto.subtle.digest("SHA-256", bytes);
        })
        .then(function (digest) {
          var value = hex(digest);
          output.dataset.hash = value;
          output.textContent = value + "   (" + file.name + ")";
          bar.value = 1;
          setTimeout(function () { bar.style.display = "none"; }, 400);
          compare();
        })
        .catch(function (error) {
          bar.style.display = "none";
          output.textContent = "error: " + error.message;
        });
    }

    drop.addEventListener("click", function () { picker.click(); });
    picker.addEventListener("change", function () { handle(picker.files[0]); });
    expected.addEventListener("input", compare);

    ["dragenter", "dragover"].forEach(function (name) {
      drop.addEventListener(name, function (e) {
        e.preventDefault();
        drop.classList.add("hot");
      });
    });
    ["dragleave", "drop"].forEach(function (name) {
      drop.addEventListener(name, function (e) {
        e.preventDefault();
        drop.classList.remove("hot");
      });
    });
    drop.addEventListener("drop", function (e) {
      if (e.dataTransfer && e.dataTransfer.files) { handle(e.dataTransfer.files[0]); }
    });
  });
})();
