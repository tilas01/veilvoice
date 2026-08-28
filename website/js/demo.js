// SPDX-License-Identifier: GPL-3.0-or-later
//
// An interactive model of VeilVoice, inside the page.
//
// # What this is for
//
// Everything else on this site describes the program. The screenshots are
// photographs of it, which are honest and are still pictures: a reader cannot
// find out what happens when they change a setting, or what the command line
// answers, without downloading and running something. For a tool whose whole
// argument is "check this yourself", asking somebody to install it before they
// can look at it is the wrong way round.
//
// So there is a model. Four of them, chosen from a strip at the top: the
// desktop application, the command line, both side by side, and the release
// verifier. They respond to clicks, they explain themselves as you go, and
// they run entirely in the page.
//
// # The label, which is not optional
//
// **This is a drawing, not the program.** The panels are written by hand, the
// device names and levels in them are illustrations, and nothing here veils
// any audio. That sentence is printed at the top of the overlay where a reader
// meets it, not buried in a comment, because a demonstration that lets
// somebody believe they have used the software is a demonstration that has
// misled them.
//
// What is *not* invented is checked: the tabs come from the application's own
// source and the terminal replays exactly what each command printed, both
// through `demo-data.js`, which `tools/site/demo.py` generates and CI verifies.
// A model that drifts from the program is a claim rather than an omission.
//
// # Why it is built here rather than shipped as a page
//
// One overlay over whichever page the reader is on, so the demonstration is a
// thing you open and close rather than a place you navigate to and have to
// find your way back from. It is created on first open and reused after that,
// so a reader who never opens it pays for nothing but this file.
//
// # No JavaScript
//
// The buttons that open it are drawn only when scripts are running, through
// the `js` class `theme.js` sets on `<html>` from a blocking head script. A
// button that does nothing is worse than no button, and the scripts-off
// edition has the photographs, which is the honest alternative.
//
// In plain words
//
// A pretend version of VeilVoice that you can click around in without
// installing anything: the app with its tabs, the command line with its real
// output, and the tool that checks a download.
//
// It is a drawing. It says so at the top. Nothing you do in it touches any
// audio, and the numbers in it are made up for the picture. What is real is
// the list of tabs and what each command prints, both taken from the source
// code so this cannot quietly fall out of step with the program.

(function () {
  "use strict";

  var data = window.VEILVOICE_DEMO || { version: "", tabs: [], commands: [] };
  var overlay = null;
  var lastFocus = null;
  var mode = "app";
  var tab = data.tabs.length ? data.tabs[0].key : "file";

  function el(name, className, text) {
    var node = document.createElement(name);
    if (className) { node.className = className; }
    if (text !== undefined) { node.textContent = text; }
    return node;
  }

  /**
   * The one line every panel needs and none of them should have to remember.
   *
   * A helper under the controls rather than a tooltip on each: a tooltip is
   * for a reader who already suspects there is something to find out, and the
   * whole point here is somebody who does not know what any of it does.
   */
  function helper(node, text) {
    var strip = node.querySelector(".demo-help");
    if (strip) { strip.textContent = text; }
  }

  function explainable(node, root, text) {
    var show = function () { helper(root, text); };
    node.addEventListener("mouseenter", show);
    node.addEventListener("focus", show);
    return node;
  }

  // --- the desktop application ---------------------------------------------

  /**
   * One panel per tab.
   *
   * Written by hand, and that is the part of this file that can go stale. The
   * *tabs* cannot: they are generated from `app.rs`. If a tab exists and has
   * no panel here, the reader is told so rather than shown an empty box, which
   * is the failure mode that would otherwise be invisible.
   */
  var PANELS = {
    file: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("p", "demo-h", "Input"));
      box.appendChild(explainable(
        el("button", "demo-btn", "choose file\u2026"), root,
        "Opens a file picker. In the real application this runs on its own " +
        "thread, so the window keeps painting while you browse."));
      var drop = el("div", "demo-drop", "or drop a recording here");
      box.appendChild(explainable(drop, root,
        "Dropping a file works anywhere in the window. The panel lights up " +
        "while the file is over it."));
      box.appendChild(el("p", "demo-h", "Output"));
      var enc = el("label", "demo-check");
      var encBox = el("input");
      encBox.type = "checkbox";
      encBox.checked = true;
      enc.appendChild(encBox);
      enc.appendChild(document.createTextNode(" encrypt the result"));
      box.appendChild(explainable(enc, root,
        "On by default. Turning it off makes the application print what you " +
        "are giving up and wait for you to type UNENCRYPTED in full."));
      var note = el("p", "demo-note",
        "veiled.wav.veil \u2014 sealed with a passphrase, so the plaintext never " +
        "touches the disk.");
      encBox.addEventListener("change", function () {
        note.textContent = encBox.checked
          ? "veiled.wav.veil \u2014 sealed with a passphrase, so the plaintext never touches the disk."
          : "veiled.wav \u2014 in the clear. The real application asks you to confirm this by typing a word.";
      });
      box.appendChild(note);
      return box;
    },

    live: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("p", "demo-h", "Devices"));
      box.appendChild(explainable(el("div", "demo-field", "input   Microphone"), root,
        "The microphone VeilVoice listens to. The real application lists what " +
        "this machine actually offers."));
      box.appendChild(explainable(el("div", "demo-field", "output  Virtual cable"), root,
        "Where the veiled voice goes. A virtual cable is what lets another " +
        "program hear it; without one you are warned rather than sent to the " +
        "speakers."));
      box.appendChild(el("p", "demo-h", "Levels"));
      var meters = el("div", "demo-meters");
      var bars = [];
      ["in", "out"].forEach(function (label) {
        var row = el("div", "demo-meter");
        row.appendChild(el("span", "demo-meter-label", label));
        var track = el("div", "demo-meter-track");
        var fill = el("i");
        track.appendChild(fill);
        row.appendChild(track);
        meters.appendChild(row);
        bars.push(fill);
      });
      box.appendChild(explainable(meters, root,
        "What is arriving and what is leaving. These say sound is moving. " +
        "They cannot say the voice has been changed: a working meter and a " +
        "bypassed engine draw the same bar."));
      var running = false;
      var timer = null;
      var start = el("button", "demo-btn demo-btn-strong", "start");
      var preview = el("button", "demo-btn", "preview to my headphones");
      var state = el("p", "demo-note", "stopped");
      function stop() {
        running = false;
        if (timer) { window.clearInterval(timer); timer = null; }
        bars[0].style.width = "0%";
        bars[1].style.width = "0%";
        start.textContent = "start";
        state.textContent = "stopped";
      }
      function run(isPreview) {
        running = true;
        start.textContent = "stop";
        state.textContent = isPreview
          ? "preview \u2014 going to this machine's output and nowhere else"
          : "live \u2014 going to the virtual cable";
        state.className = isPreview ? "demo-note demo-warn" : "demo-note demo-ok";
        var step = 0;
        timer = window.setInterval(function () {
          step += 1;
          var a = 28 + Math.abs(Math.sin(step / 3)) * 55;
          bars[0].style.width = a.toFixed(0) + "%";
          bars[1].style.width = (a * 0.92).toFixed(0) + "%";
        }, 140);
      }
      start.addEventListener("click", function () {
        if (running) { stop(); } else { run(false); }
      });
      preview.addEventListener("click", function () {
        if (running) { stop(); }
        run(true);
      });
      explainable(start, root,
        "Starts the engine. The veiled voice goes to the output device above.");
      explainable(preview, root,
        "The same engine, sent to your own headphones instead of the cable, so " +
        "you can hear what you sound like before anybody else does.");
      var row = el("div", "demo-row");
      row.appendChild(start);
      row.appendChild(preview);
      box.appendChild(row);
      box.appendChild(state);
      return box;
    },

    group: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("p", "demo-h", "Who is in the recording"));
      ["Ada", "Bram", "Cleo"].forEach(function (name, index) {
        var row = el("div", "demo-speaker");
        var dot = el("i", "demo-speaker-dot");
        dot.style.background = ["var(--accent)", "var(--ok)", "var(--warn)"][index];
        row.appendChild(dot);
        row.appendChild(el("span", null, name));
        row.appendChild(el("span", "demo-note", " a voice of their own"));
        box.appendChild(explainable(row, root,
          "Each speaker gets a different destination voice and a colour that " +
          "carries into the subtitles and the video. The colours are chosen " +
          "to be as distinct as the number of speakers allows."));
      });
      box.appendChild(el("p", "demo-note",
        "Group mode is off unless you turn it on, and it does not survive a " +
        "restart unless you tick that separately: a recording of one person " +
        "rendered against a plan for three would be silently wrong."));
      return box;
    },

    monitor: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("p", "demo-h", "Holding the microphone"));
      box.appendChild(explainable(el("div", "demo-field", "veilvoice-gui   since 14:02"), root,
        "VeilVoice itself, listed rather than hidden: a monitor that excuses " +
        "its own program is one you cannot check."));
      box.appendChild(explainable(el("div", "demo-field demo-warn", "zoom.exe        since 14:07"), root,
        "Another program on a real microphone while you are being veiled. " +
        "This is what the safety catch acts on."));
      box.appendChild(el("p", "demo-note",
        "On a platform that cannot see this, the tab says so. An empty list " +
        "from a blind monitor is a false reassurance and is never shown as " +
        "good news."));
      return box;
    },

    lock: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("p", "demo-h", "Passphrase"));
      var input = el("input", "demo-input");
      input.type = "password";
      input.value = "\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022";
      input.readOnly = true;
      box.appendChild(explainable(input, root,
        "Argon2id, with the cost stored beside the verifier so an old lock " +
        "still opens. The rate limit is written down, so closing the " +
        "application does not clear it."));
      box.appendChild(el("p", "demo-note",
        "The lock guards the application, not the recordings. A recording is " +
        "protected by its own encryption, and the app lock is not a substitute " +
        "for it."));
      return box;
    },

    verify: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("p", "demo-h", "Check a download"));
      var steps = el("ol", "demo-steps-list");
      [
        ["the key's fingerprint", "compared against the one built into the tool"],
        ["the signature over SHA256SUMS", "checked with that key, before any hash is trusted"],
        ["the archive's hash", "compared against the now-trusted list"]
      ].forEach(function (pair) {
        var item = el("li");
        item.appendChild(el("strong", null, pair[0]));
        item.appendChild(document.createTextNode(" \u2014 " + pair[1]));
        steps.appendChild(item);
      });
      box.appendChild(steps);
      box.appendChild(el("p", "demo-note",
        "In that order, and it refuses rather than continuing. There is no " +
        "flag that skips a step, because a verification with a skip switch is " +
        "decorative."));
      return box;
    },

    settings: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("p", "demo-h", "The live monitor"));
      var choice = el("div", "demo-row");
      ["a strip along the bottom", "a card in the corner", "only on the live tab"]
        .forEach(function (label, index) {
          var button = el("button", "demo-btn" + (index === 0 ? " demo-btn-on" : ""), label);
          button.addEventListener("click", function () {
            Array.prototype.forEach.call(choice.children, function (other) {
              other.className = "demo-btn";
            });
            button.className = "demo-btn demo-btn-on";
            helper(root, "Where the monitor sits while live scramble is " +
              "running. It is on by default, on every tab.");
          });
          choice.appendChild(button);
        });
      box.appendChild(choice);
      box.appendChild(el("p", "demo-h", "Colour scheme"));
      box.appendChild(explainable(el("div", "demo-field", "Tokyo Night, and eight more"), root,
        "Every theme the website offers, plus palettes you define yourself, " +
        "with the contrast measured rather than assumed."));
      return box;
    },

    install: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("p", "demo-h", "This copy"));
      box.appendChild(explainable(el("div", "demo-field", "portable \u2014 running from where you unzipped it"), root,
        "Portable is the normal case rather than something missing. The " +
        "installed and portable copies are the same executable."));
      box.appendChild(el("p", "demo-note",
        "Installing copies it somewhere the system can find it and adds it to " +
        "PATH. Nothing else: no service, no scheduled task, nothing that runs " +
        "at startup."));
      return box;
    },

    about: function (root) {
      var box = el("div", "demo-panel");
      box.appendChild(el("div", "demo-field", "VeilVoice " + data.version));
      box.appendChild(el("div", "demo-field", "Network access   none, by construction"));
      box.appendChild(el("div", "demo-field", "Licence          GPL-3.0-or-later"));
      box.appendChild(el("p", "demo-note",
        "VeilVoice destroys the voiceprint, not the words. It does not hide " +
        "what was said."));
      return box;
    }
  };

  function appModel(root) {
    var frame = el("div", "demo-app");

    var bar = el("div", "demo-titlebar");
    bar.appendChild(el("span", "demo-title", "VeilVoice v" + data.version));
    bar.appendChild(el("span", "demo-offline", "offline"));
    frame.appendChild(bar);

    var strip = el("div", "demo-tabs");
    var body = el("div", "demo-body");

    // The panels write into the card's helper line, not one of the model's
    // own. There was one of each for a while, and the effect was two identical
    // sentences under the window with only the upper one ever changing, which
    // reads as a rendering fault rather than as a feature.
    function draw() {
      body.textContent = "";
      var make = PANELS[tab];
      if (make) {
        body.appendChild(make(root));
      } else {
        var missing = el("p", "demo-note",
          "This tab exists in the application and has no panel drawn for it " +
          "here yet. The screenshots show it.");
        body.appendChild(missing);
      }
      Array.prototype.forEach.call(strip.children, function (button) {
        button.className = "demo-tab" + (button.dataset.key === tab ? " demo-tab-on" : "");
        button.setAttribute("aria-selected", button.dataset.key === tab ? "true" : "false");
      });
    }

    data.tabs.forEach(function (entry) {
      var button = el("button", "demo-tab", entry.label);
      button.dataset.key = entry.key;
      button.setAttribute("role", "tab");
      button.addEventListener("click", function () {
        tab = entry.key;
        draw();
      });
      strip.appendChild(button);
    });
    strip.setAttribute("role", "tablist");

    frame.appendChild(strip);
    frame.appendChild(body);
    draw();
    return frame;
  }

  // --- the command line ----------------------------------------------------

  function cliModel() {
    var frame = el("div", "demo-term");
    var bar = el("div", "demo-term-bar");
    ["#f7768e", "#e0af68", "#9ece6a"].forEach(function (colour) {
      var dot = el("i", "demo-term-dot");
      dot.style.background = colour;
      bar.appendChild(dot);
    });
    bar.appendChild(el("span", "demo-term-name", "veilvoice"));
    frame.appendChild(bar);

    var out = el("pre", "demo-term-out");
    var picks = el("div", "demo-term-picks");

    function show(entry) {
      out.textContent = "$ " + entry.typed + "\n\n" + entry.output;
      out.scrollTop = 0;
      Array.prototype.forEach.call(picks.children, function (button) {
        button.className = "demo-btn" + (button.dataset.name === entry.name ? " demo-btn-on" : "");
      });
    }

    data.commands.forEach(function (entry) {
      var button = el("button", "demo-btn", entry.typed.replace("veilvoice ", ""));
      button.dataset.name = entry.name;
      button.title = entry.note;
      button.addEventListener("click", function () { show(entry); });
      picks.appendChild(button);
    });

    frame.appendChild(picks);
    frame.appendChild(out);
    if (data.commands.length) { show(data.commands[0]); }
    return frame;
  }

  // --- the release verifier ------------------------------------------------

  function verifyModel() {
    var frame = el("div", "demo-term");
    var bar = el("div", "demo-term-bar");
    bar.appendChild(el("span", "demo-term-name", "veilvoice-verify"));
    frame.appendChild(bar);

    var out = el("pre", "demo-term-out");
    var step = 0;
    var lines = [
      "$ veilvoice-verify auto",
      "",
      "==> Checking the signing key's fingerprint",
      "  ok   fingerprint matches 8101FB3BB28D02FB239E0CDF9CC1C7E7A9B5833A",
      "==> Verifying the signature over SHA256SUMS",
      "  ok   signature is good",
      "==> Verifying the archive against SHA256SUMS",
      "  ok   sha256 matches",
      "",
      "Every check passed, in that order: the key, then the signature over the",
      "list, then the file against the list. A hash checked against a list",
      "nobody verified is not a check."
    ];
    var run = el("button", "demo-btn demo-btn-strong", "run it");
    var timer = null;
    function play() {
      if (timer) { window.clearInterval(timer); }
      step = 0;
      out.textContent = "";
      timer = window.setInterval(function () {
        if (step >= lines.length) {
          window.clearInterval(timer);
          timer = null;
          return;
        }
        out.textContent += (step ? "\n" : "") + lines[step];
        step += 1;
      }, 260);
    }
    run.addEventListener("click", play);
    var picks = el("div", "demo-term-picks");
    picks.appendChild(run);
    frame.appendChild(picks);
    frame.appendChild(out);
    out.textContent = lines.join("\n");
    return frame;
  }

  // --- the overlay ---------------------------------------------------------

  var MODES = [
    ["app", "The application"],
    ["cli", "The command line"],
    ["both", "Both"],
    ["verify", "The release verifier"]
  ];

  function stage(root) {
    var area = root.querySelector(".demo-stage");
    area.textContent = "";
    if (mode === "app") { area.appendChild(appModel(root)); }
    else if (mode === "cli") { area.appendChild(cliModel()); }
    else if (mode === "verify") { area.appendChild(verifyModel()); }
    else {
      area.className = "demo-stage demo-stage-split";
      area.appendChild(appModel(root));
      area.appendChild(cliModel());
      return;
    }
    area.className = "demo-stage";
  }

  function build() {
    var back = el("div", "demo-overlay");
    back.setAttribute("role", "dialog");
    back.setAttribute("aria-modal", "true");
    back.setAttribute("aria-label", "An interactive model of VeilVoice");

    var card = el("div", "demo-card");

    var head = el("div", "demo-card-head");
    var picker = el("div", "demo-modes");
    MODES.forEach(function (pair) {
      var button = el("button", "demo-mode", pair[1]);
      button.dataset.mode = pair[0];
      button.addEventListener("click", function () {
        mode = pair[0];
        Array.prototype.forEach.call(picker.children, function (other) {
          other.className = "demo-mode" + (other.dataset.mode === mode ? " demo-mode-on" : "");
        });
        stage(card);
      });
      picker.appendChild(button);
    });
    head.appendChild(picker);

    var close = el("button", "demo-close", "close");
    close.setAttribute("aria-label", "Close the demonstration");
    close.addEventListener("click", hide);
    head.appendChild(close);
    card.appendChild(head);

    card.appendChild(el("p", "demo-warn demo-disclaimer",
      "This is a drawing of VeilVoice, not VeilVoice. Nothing here touches any " +
      "audio and the levels and device names are illustrations. What is real: " +
      "the tabs come from the application's source and the terminal replays " +
      "exactly what each command printed."));

    card.appendChild(el("div", "demo-stage"));
    card.appendChild(el("p", "demo-help",
      "Point at anything in here and this line says what it does."));
    back.appendChild(card);

    back.addEventListener("click", function (event) {
      // The backdrop closes it; the card does not, or every click inside the
      // model would shut the thing the reader is using.
      if (event.target === back) { hide(); }
    });
    return back;
  }

  function onKey(event) {
    if (event.key === "Escape") { hide(); }
  }

  function show(which) {
    mode = which || "app";
    if (!overlay) {
      overlay = build();
      document.body.appendChild(overlay);
    }
    Array.prototype.forEach.call(overlay.querySelectorAll(".demo-mode"), function (button) {
      button.className = "demo-mode" + (button.dataset.mode === mode ? " demo-mode-on" : "");
    });
    stage(overlay.querySelector(".demo-card"));
    lastFocus = document.activeElement;
    overlay.classList.add("demo-open");
    document.documentElement.classList.add("demo-locked");
    document.addEventListener("keydown", onKey);
    var first = overlay.querySelector(".demo-close");
    if (first) { first.focus(); }
  }

  function hide() {
    if (!overlay) { return; }
    overlay.classList.remove("demo-open");
    document.documentElement.classList.remove("demo-locked");
    document.removeEventListener("keydown", onKey);
    if (lastFocus && lastFocus.focus) { lastFocus.focus(); }
  }

  // A fragment that opens it, so the demonstration has an address.
  //
  // `#try` is the application, `#try-cli` the command line, `#try-both` and
  // `#try-verify` the other two. Worth having for the same reason the section
  // pages are worth having: "look at this" is a thing people send each other,
  // and a link that lands on a page with a button on it somewhere is not the
  // same as a link that lands on the thing.
  var FRAGMENTS = {
    "#try": "app",
    "#try-cli": "cli",
    "#try-both": "both",
    "#try-verify": "verify"
  };

  function fromFragment() {
    var wanted = FRAGMENTS[window.location.hash];
    if (wanted) { show(wanted); }
  }

  document.addEventListener("DOMContentLoaded", function () {
    var buttons = document.querySelectorAll("[data-demo]");
    Array.prototype.forEach.call(buttons, function (button) {
      button.addEventListener("click", function (event) {
        event.preventDefault();
        show(button.getAttribute("data-demo"));
      });
    });
    fromFragment();
  });

  window.addEventListener("hashchange", fromFragment);
})();
