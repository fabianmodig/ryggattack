import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const html = readFileSync(new URL("../web/index.html", import.meta.url), "utf8");
const moduleUrl = new URL("../web/refresh-rate.mjs", import.meta.url);
const calibrateRefreshRate = existsSync(moduleUrl)
  ? (await import(moduleUrl)).calibrateRefreshRate : undefined;

function environment() {
  let next = 0;
  const rafs = new Map();
  const timers = new Map();
  const listeners = new Map();
  const document = {
    hidden: false,
    addEventListener: (name, fn) => listeners.set(name, fn),
    removeEventListener: (name) => listeners.delete(name),
    getElementById: () => ({ focus() {}, remove() {}, classList: { add() {} } }),
  };
  const env = {
    document,
    requestAnimationFrame: (fn) => { rafs.set(++next, fn); return next; },
    cancelAnimationFrame: (id) => rafs.delete(id),
    setTimeout: (fn) => { timers.set(++next, fn); return next; },
    clearTimeout: (id) => timers.delete(id),
  };
  return { env, rafs, timers, listeners,
    frame(timestamp) {
      const pending = [...rafs.values()]; rafs.clear();
      for (const fn of pending) fn(timestamp);
    },
    timeout() { for (const fn of [...timers.values()]) fn(); },
    hide() { document.hidden = true; listeners.get("visibilitychange")?.(); },
  };
}

async function startPage(hz) {
  const clock = environment();
  const window = { addEventListener() {} };
  const starts = [];
  const source = html.match(/<script type="module">([\s\S]*?)<\/script>/)[1]
    .replace(/^\s*import .*;$/gm, "");
  const startup = vm.runInNewContext(`(async () => {${source}})()`, {
    ...clock.env, window,
    calibrateRefreshRate: () => calibrateRefreshRate(clock.env),
    init: async () => starts.push(window.ryggattackRefreshRate),
  });
  assert.equal(starts.length, 0, "game must not run during refresh calibration");
  for (let frame = 0; frame <= 30; frame++) clock.frame(frame * 1000 / hz);
  await startup;
  assert.equal(starts.length, 1);
  assert.ok(Math.abs(starts[0] - Math.min(hz, 60)) < 0.1,
    `init received ${starts[0]} Hz instead of ${Math.min(hz, 60)}`);
  assert.equal(clock.rafs.size, 0);
  assert.equal(clock.timers.size, 0);
}

test("perf servers serve the calibration module with a JavaScript MIME type", () => {
  for (const name of ["anatomy", "ingame", "shots"]) {
    const source = readFileSync(new URL(`./perf/${name}.mjs`, import.meta.url), "utf8");
    assert.match(source, /"\.mjs":\s*"(?:text|application)\/javascript"/,
      `${name} must serve imported .mjs files as JavaScript`);
  }
});

test("page calibrates unloaded rAF before starting wasm at 30/40/144 Hz", async () => {
  for (const hz of [30, 40, 144]) await startPage(hz);
});

test("page still starts wasm if calibration unexpectedly rejects", async () => {
  const clock = environment();
  const window = { addEventListener() {} };
  const starts = [];
  const source = html.match(/<script type="module">([\s\S]*?)<\/script>/)[1]
    .replace(/^\s*import .*;$/gm, "");
  await vm.runInNewContext(`(async () => {${source}})()`, {
    ...clock.env, window,
    calibrateRefreshRate: async () => { throw new Error("calibration unavailable"); },
    init: async () => starts.push(window.ryggattackRefreshRate),
  });
  assert.deepEqual(starts, [null]);
});

test("hidden pages discard calibration without scheduling rAF", async () => {
  const clock = environment();
  clock.env.document.hidden = true;
  const result = calibrateRefreshRate(clock.env);
  assert.equal(clock.rafs.size, 0, "hidden cadence is not display cadence");
  assert.equal(await result, null);
});

test("missing or failing rAF resolves null instead of blocking startup", async () => {
  for (const requestAnimationFrame of [undefined, () => { throw new Error("denied"); }]) {
    const clock = environment();
    clock.env.requestAnimationFrame = requestAnimationFrame;
    assert.equal(await calibrateRefreshRate(clock.env), null);
    assert.equal(clock.timers.size, 0);
  }
});

test("stalled or incomplete calibration has a bounded timeout and cleanup", async () => {
  const clock = environment();
  const result = calibrateRefreshRate(clock.env);
  assert.equal(clock.timers.size, 1, "calibration must have a startup deadline");
  clock.frame(0);
  clock.frame(33.3);
  clock.timeout();
  assert.equal(await result, null);
  assert.equal(clock.rafs.size, 0);
  assert.equal(clock.timers.size, 0);
  assert.equal(clock.listeners.size, 0);
});

test("visibility changes invalidate partially measured cadence", async () => {
  const clock = environment();
  const result = calibrateRefreshRate(clock.env);
  assert.equal(clock.listeners.size, 1);
  clock.frame(0);
  clock.frame(33.3);
  clock.hide();
  assert.equal(await result, null);
  assert.equal(clock.rafs.size, 0);
  assert.equal(clock.timers.size, 0);
});

test("implausible or invalid timestamps never become tiny or invalid targets", async () => {
  for (const dt of [0, -1, 0.1, 1000, NaN, Infinity]) {
    const clock = environment();
    const result = calibrateRefreshRate(clock.env);
    for (let i = 0; i <= 24; i++) clock.frame(i * dt);
    assert.equal(await result, null, `invalid cadence ${dt}`);
  }
});

test("one scheduling hitch does not lower an otherwise stable 40 Hz target", async () => {
  const clock = environment();
  const result = calibrateRefreshRate(clock.env);
  let timestamp = 0;
  for (let i = 0; i <= 24; i++) {
    timestamp += i === 10 ? 300 : 25;
    clock.frame(timestamp);
  }
  assert.ok(Math.abs(await result - 40) < 0.1);
});

test("unstable throttled cadence is not accepted as a low-refresh display", async () => {
  const clock = environment();
  const result = calibrateRefreshRate(clock.env);
  let timestamp = 0;
  for (let i = 0; i <= 24; i++) {
    timestamp += i % 2 ? 25 : 50;
    clock.frame(timestamp);
  }
  assert.equal(await result, null);
});
