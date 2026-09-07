// SPDX-License-Identifier: GPL-3.0-or-later
//
// The command line, on the page, typed out.
//
// # What this plays, and why it is not a drawing
//
// Five recordings of the real programs: the bytes `veilvoice` wrote on a real
// terminal, with the passphrases typed at the real prompts. `tools/shots/
// sessions.py` records them into `assets/screenshots/session-*.txt` and
// `tools/site/demo.py` turns those into `window.VEILVOICE_DEMO.sessions`, which
// CI regenerates and compares. Nothing here is written by hand, so nothing here
// can quietly stop matching the program.
//
// The one invented thing is the pacing. A session that arrives all at once is a
// paste rather than a session, so the typing is played at about the speed
// somebody types and the output at about the speed a terminal fills. That is
// the whole of the fiction and it is stated here rather than implied.
//
// # What used to be here instead
//
// A hand-built model of the desktop application, drawn in CSS, opened from a
// button as an overlay over whichever page the reader was on. It responded to
// clicks and it was a drawing: the device names, the levels and the panels were
// all written by hand, and it veiled no audio. It was labelled as a drawing,
// and a label is a weaker thing than not needing one.
//
// It was also redundant. The real window is photographed on every build, nine
// captures, one per screen, and those are on this page too. Offering a reader a
// drawing of an interface *and* photographs of the same interface asks them to
// work out which one to believe, on a site whose argument is that they should
// not have to take anybody's word for anything.
//
// So the drawing is gone and this is what replaced it: the recordings, on the
// page rather than behind a button, above the photographs of the window.
//
// # In plain words
//
// Plays back real terminal sessions at typing speed, so you can see what the
// program actually prints without installing it.

(function () {
  "use strict";

  // Milliseconds. Slow enough to read a command as it appears, quick enough
  // that a forty-line help screen does not outlast the reader's patience.
  var PER_CHARACTER = 24;
  var AFTER_PROMPT = 200;
  var PER_LINE = 30;
  var PER_BLANK = 80;

  function el(name, className, text) {
    var node = document.createElement(name);
    if (className) { node.className = className; }
    if (text !== undefined) { node.textContent = text; }
    return node;
  }

  function still() {
    return !!(window.matchMedia
      && window.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }

  document.addEventListener("DOMContentLoaded", function () {
    var root = document.getElementById("cli-demo");
    if (!root) { return; }

    var data = window.VEILVOICE_DEMO || {};
    var sessions = data.sessions || [];
    if (!sessions.length) { return; }

    var bar = el("div", "term-bar");
    ["#f7768e", "#e0af68", "#9ece6a"].forEach(function (colour) {
      var dot = el("i", "term-dot");
      dot.style.background = colour;
      bar.appendChild(dot);
    });
    var name = el("span", "term-name", "veilvoice");
    bar.appendChild(name);

    var picks = el("div", "term-picks");
    picks.setAttribute("role", "tablist");
    picks.setAttribute("aria-label", "Recorded sessions");
    var note = el("p", "term-note", "");
    var out = el("pre", "term-out");
    // A live region, so somebody using a screen reader is told the output
    // arrived rather than being left with a box that silently fills.
    out.setAttribute("aria-live", "polite");
    out.setAttribute("tabindex", "0");

    var replay = el("button", "term-btn term-btn-do", "play it again");
    replay.type = "button";
    var skip = el("button", "term-btn term-btn-do", "show all of it");
    skip.type = "button";

    root.appendChild(bar);
    root.appendChild(picks);
    root.appendChild(note);
    root.appendChild(out);

    var timer = null;
    var current = sessions[0];
    var started = false;

    function stop() {
      if (timer) { window.clearTimeout(timer); timer = null; }
    }

    function whole(entry) {
      return entry.steps.map(function (step) {
        return "$ " + step.typed + "\n" + step.output;
      }).join("\n\n");
    }

    function mark(entry) {
      Array.prototype.forEach.call(picks.children, function (button) {
        if (!button.dataset.name) { return; }
        var on = button.dataset.name === entry.name;
        button.className = on ? "term-btn term-btn-on" : "term-btn";
        button.setAttribute("aria-selected", on ? "true" : "false");
        button.setAttribute("tabindex", on ? "0" : "-1");
      });
    }

    function play(entry, animate) {
      stop();
      current = entry;
      name.textContent = entry.programme;
      note.textContent = entry.note;
      mark(entry);

      if (!animate || still()) {
        out.textContent = whole(entry);
        return;
      }

      out.textContent = "";
      var queue = [];
      entry.steps.forEach(function (step, index) {
        if (index) { queue.push(["line", ""]); }
        queue.push(["prompt", ""]);
        step.typed.split("").forEach(function (character) {
          queue.push(["type", character]);
        });
        queue.push(["line", ""]);
        step.output.split("\n").forEach(function (line) {
          queue.push(["line", line]);
        });
      });

      var at = 0;
      (function tick() {
        if (at >= queue.length) { timer = null; return; }
        var kind = queue[at][0];
        var text = queue[at][1];
        at += 1;
        var wait;
        if (kind === "prompt") { out.textContent += "$ "; wait = AFTER_PROMPT; }
        else if (kind === "type") { out.textContent += text; wait = PER_CHARACTER; }
        else { out.textContent += "\n" + text; wait = text ? PER_LINE : PER_BLANK; }
        out.scrollTop = out.scrollHeight;
        timer = window.setTimeout(tick, wait);
      })();
    }

    sessions.forEach(function (entry, index) {
      var button = el("button", "term-btn", entry.title);
      button.type = "button";
      button.setAttribute("role", "tab");
      button.dataset.name = entry.name;
      button.title = entry.note;
      button.addEventListener("click", function () {
        started = true;
        play(entry, true);
      });
      if (index === 0) { button.setAttribute("tabindex", "0"); }
      picks.appendChild(button);
    });

    replay.addEventListener("click", function () { play(current, true); });
    skip.addEventListener("click", function () {
      stop();
      out.textContent = whole(current);
    });
    picks.appendChild(replay);
    picks.appendChild(skip);

    // Arrow keys move within the strip, as a tablist is meant to: one tab stop
    // for the whole row rather than seven for somebody who wants none of them.
    picks.addEventListener("keydown", function (event) {
      var order = ["ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp"];
      var step = order.indexOf(event.key);
      if (step < 0) { return; }
      event.preventDefault();
      var at = sessions.indexOf(current);
      at = (at + (step < 2 ? 1 : -1) + sessions.length) % sessions.length;
      started = true;
      play(sessions[at], true);
      var button = picks.querySelector('[data-name="' + sessions[at].name + '"]');
      if (button) { button.focus(); }
    });

    // The first one is shown straight away, and starts typing when it is on
    // screen. Animating a terminal nobody has scrolled to spends the reader's
    // battery on something they never saw, and finishing before they arrive
    // leaves them looking at the end of a session they did not watch.
    play(current, false);
    if (!window.IntersectionObserver || still()) { return; }
    var watcher = new window.IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting && !started) {
          started = true;
          play(current, true);
        }
      });
    }, { threshold: 0.25 });
    watcher.observe(root);
  });
})();
