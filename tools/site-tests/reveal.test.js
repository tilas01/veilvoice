// SPDX-License-Identifier: GPL-3.0-or-later
//
// Behaviour tests for the scroll-reveal effect.
//
// The invariant is narrow and absolute: every path this module can take must
// end with the content visible. A reveal that hides text it then fails to show
// is worse than no effect at all, and the two ways that happens -- no
// IntersectionObserver, and a reader who asked for reduced motion -- are both
// silent in a browser that works.

function run() {
// Exercises reveal.js in a stub DOM: the three paths it can take must all end
// with the content visible, because the one unacceptable outcome is text that
// exists but is never shown.
const fs = require('fs');
const vm = require('vm');
const src = fs.readFileSync(require('path').resolve(__dirname,'..','..','website','js','reveal.js'), 'utf8');

function node() { const c = new Set(); return { classList: { add: v => c.add(v), has: v => c.has(v) }, _c: c }; }

function run({ reducedMotion, hasIO, count = 4 }) {
  const nodes = Array.from({ length: count }, node);
  let ready;
  const observed = [];
  const unobserved = [];
  let cb = null;
  const sandbox = {
    document: {
      addEventListener: (ev, fn) => { if (ev === 'DOMContentLoaded') ready = fn; },
      querySelectorAll: () => nodes,
    },
    window: {
      matchMedia: () => ({ matches: reducedMotion }),
    },
  };
  if (hasIO) {
    sandbox.window.IntersectionObserver = function (fn, opts) {
      cb = fn; this.options = opts;
      this.observe = n => observed.push(n);
      this.unobserve = n => unobserved.push(n);
    };
    sandbox.IntersectionObserver = sandbox.window.IntersectionObserver;
  }
  vm.createContext(sandbox);
  vm.runInContext(src, sandbox);
  ready();
  return { nodes, observed, unobserved, fire: (self) => cb(nodes.map(t => ({ isIntersecting: true, target: t })), self) };
}

let fails = 0;
const check = (name, ok) => { console.log((ok ? 'ok   ' : 'FAIL ') + name); if (!ok) fails++; };

// 1. Reduced motion: shown immediately, observer never built.
{
  const r = run({ reducedMotion: true, hasIO: true });
  check('reduced motion reveals everything at once', r.nodes.every(n => n._c.has('in')));
  check('reduced motion observes nothing', r.observed.length === 0);
}
// 2. No IntersectionObserver: shown immediately.
{
  const r = run({ reducedMotion: false, hasIO: false });
  check('missing IntersectionObserver still reveals everything', r.nodes.every(n => n._c.has('in')));
}
// 3. Normal path: hidden until intersecting, then shown and unobserved.
{
  const r = run({ reducedMotion: false, hasIO: true });
  check('normal path starts with nothing revealed', r.nodes.every(n => !n._c.has('in')));
  check('normal path observes every node', r.observed.length === 4);
  const self = { unobserve: n => r.unobserved.push(n) };
  r.fire(self);
  check('intersecting nodes are revealed', r.nodes.every(n => n._c.has('in')));
  check('revealed nodes are unobserved (no leftover work)', r.unobserved.length === 4);
}
// 4. No .reveal elements at all: must not throw or build an observer.
{
  const nodes = [];
  let threw = false;
  try {
    const sandbox = { document: { addEventListener: (e, f) => e === 'DOMContentLoaded' && (sandbox._r = f), querySelectorAll: () => nodes }, window: {} };
    vm.createContext(sandbox); vm.runInContext(src, sandbox); sandbox._r();
  } catch (e) { threw = true; }
  check('a page with no reveals is a no-op', !threw);
}
return fails;

}
module.exports = { run, name: "scroll reveal" };
if (require.main === module) { process.exit(run() ? 1 : 0); }
