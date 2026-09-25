// Combined in-game measurement: navigate with long key holds (the game runs at
// ~1 fps under SwiftShader, so short taps get lost), screenshot to confirm the
// state, then sample rAF intervals, WebGL call counts, and CDP metrics for an
// idle and a firing window at each CPU throttle rate.
// Usage: node ingame.mjs <dist> <label> [--width W --height H --throttle 1,4]
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
const require = createRequire(process.env.PLAYWRIGHT_NODE_MODULES ?? import.meta.url);
const { chromium } = require("playwright");
const args = process.argv.slice(2);
const dist = path.resolve(args[0]);
const label = args[1];
const opt = (n, d) => { const i = args.indexOf(`--${n}`); return i >= 0 ? args[i + 1] : d; };
const width = Number(opt("width", "320")), height = Number(opt("height", "180"));
const throttles = opt("throttle", "1").split(",").map(Number);
const secs = Number(opt("seconds", "15"));
const outDir = path.dirname(new URL(import.meta.url).pathname);
const types = { ".wasm": "application/wasm", ".js": "text/javascript", ".html": "text/html", ".png": "image/png" };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(req.url.split("?")[0]); if (p.endsWith("/")) p += "index.html";
  fs.readFile(path.join(dist, p), (e, d) => { if (e) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { "content-type": types[path.extname(p)] || "application/octet-stream" }); res.end(d); });
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
const dpr = Number(opt("dpr", "1"));
const ablate = opt("ablate", "");
var out_meta;
const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: dpr });
out_meta = { dpr, ablate };
const page = await context.newPage();
// --video "<saved settings>" pre-seeds localStorage, e.g. the Low preset:
//   "scale=67% aa=OFF shadows=OFF effects=LOW forest=SPARSE fps=OFF"
const video = opt("video", "");
out_meta.video = video || "default (High)";
await page.addInitScript((video) => { if (video) localStorage.setItem("ryggattack.video", video); }, video);
await page.addInitScript((ablate) => {
  if (!ablate) return;
  const P = WebGL2RenderingContext.prototype;
  const fbInfo = new WeakMap(); let cur = null;
  const obf = P.bindFramebuffer; P.bindFramebuffer = function (t, fb) { if (t !== 0x8ca8) cur = fb; return obf.call(this, t, fb); };
  const oft = P.framebufferTexture2D; P.framebufferTexture2D = function (t, att, tt, tex, l) {
    if (cur && t !== 0x8ca8) { const i = fbInfo.get(cur) || { color: false, depth: false }; if (att >= 0x8ce0 && att <= 0x8cef) i.color = i.color || !!tex; else i.depth = true; fbInfo.set(cur, i); }
    return oft.call(this, t, att, tt, tex, l); };
  const skip = (name, a) => {
    if (ablate.includes("shadow") && cur) { const i = fbInfo.get(cur); if (i && i.depth && !i.color) return true; }
    if (ablate.includes("corners") && name.startsWith("drawArrays") && (name === "drawArraysInstanced" ? a[2] : a[2]) > 3) return true;
    return false; };
  for (const name of ["drawElements", "drawArrays", "drawElementsInstanced", "drawArraysInstanced"]) {
    const o = P[name]; P[name] = function (...a) { if (skip(name, a)) return; return o.apply(this, a); }; }
}, ablate);
const cdp = await context.newCDPSession(page);
await cdp.send("Performance.enable");
await page.addInitScript(() => {
  const P = WebGL2RenderingContext.prototype;
  const c = (window.__gl = { t: [], calls: 0, draws: 0, tris: 0, up: 0, upBytes: 0, perFrameDraws: [], cur: 0 });
  for (const name of Object.getOwnPropertyNames(P)) {
    const d = Object.getOwnPropertyDescriptor(P, name);
    if (!d || typeof d.value !== "function" || name === "constructor") continue;
    const f = d.value;
    const isDraw = name.startsWith("draw");
    const inst = name.endsWith("Instanced");
    const isUp = name === "bufferSubData" || name === "bufferData";
    P[name] = function (...a) {
      c.calls++;
      if (isDraw) { c.draws++; c.cur++; if (a[0] === 4) c.tris += (a[1] / 3) * (inst ? (name === "drawElementsInstanced" ? a[4] : a[3]) : 1); }
      if (isUp) { c.up++; const s = a[1]; c.upBytes += typeof s === "number" ? s : (s?.byteLength || 0); }
      return f.apply(this, a);
    };
  }
  const tick = (t) => { c.t.push(t); c.perFrameDraws.push(c.cur); c.cur = 0; requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
});
const shot = async (n) => { try { await page.screenshot({ path: path.join(outDir, `ig-${label}-${n}.png`), timeout: 90000 }); } catch { console.log("shot failed", n); } };
const hold = async (key, ms = 3500) => { await page.keyboard.down(key); await sleep(ms); await page.keyboard.up(key); await sleep(2500); };

await page.goto(`http://127.0.0.1:${server.address().port}/`);
await page.waitForFunction(() => !document.getElementById("status"), null, { timeout: 180000 });
await sleep(5000);
await page.locator("#ryggattack-canvas").focus();

async function metrics() { const { metrics } = await cdp.send("Performance.getMetrics"); return Object.fromEntries(metrics.map((m) => [m.name, m.value])); }
async function sample(s) {
  await page.evaluate(() => { const c = window.__gl; c.t = []; c.calls = 0; c.draws = 0; c.tris = 0; c.up = 0; c.upBytes = 0; c.perFrameDraws = []; });
  const b = await metrics();
  await sleep(s * 1000);
  const a = await metrics();
  return page.evaluate(([b, a]) => {
    const c = window.__gl; const t = c.t;
    const d = t.slice(1).map((x, i) => x - t[i]).sort((x, y) => x - y);
    const n = Math.max(1, t.length - 1); const wall = t.length > 1 ? t.at(-1) - t[0] : 0;
    const q = (p) => (d.length ? +d[Math.min(d.length - 1, Math.floor(d.length * p))].toFixed(0) : null);
    const f = Math.max(1, t.length);
    return { frames: t.length, fps: wall ? +(n / (wall / 1000)).toFixed(2) : 0, median_ms: q(0.5), p95_ms: q(0.95), max_ms: d.length ? +d.at(-1).toFixed(0) : null,
      draws_per_frame: Math.round(c.draws / f), gl_calls_per_frame: Math.round(c.calls / f), tris_per_frame: Math.round(c.tris / f),
      uploads_per_frame: +(c.up / f).toFixed(1), upload_kb_per_frame: +(c.upBytes / f / 1024).toFixed(0),
      renderer_task_ms_per_frame: +(((a.TaskDuration - b.TaskDuration) * 1000) / f).toFixed(1),
      renderer_script_ms_per_frame: +(((a.ScriptDuration - b.ScriptDuration) * 1000) / f).toFixed(1),
      js_heap_mb: +(a.JSHeapUsedSize / 1e6).toFixed(1) };
  }, [b, a]);
}
const out = { label, width, height, ...out_meta, results: {} };
out.canvas = await page.evaluate(() => { const c = document.getElementById("ryggattack-canvas"); return [c.width, c.height]; });
out.results.menu = await sample(10);
console.log("partial menu " + JSON.stringify(out.results.menu));
await hold("Enter");            // START -> lobby
await hold("w");                // WASD takes seat 1
await shot("lobby");
await hold("Enter", 4000);      // START ROUND (focused by default)
await sleep(4000);
await shot("playing");
for (const rate of throttles) {
  await cdp.send("Emulation.setCPUThrottlingRate", { rate });
  await sleep(2000);
  out.results[`idle_x${rate}`] = await sample(secs);
  console.log(`partial idle_x${rate} ` + JSON.stringify(out.results[`idle_x${rate}`]));
  await page.keyboard.down("Space");
  await sleep(5000);
  out.results[`fight_x${rate}`] = await sample(secs);
  console.log(`partial fight_x${rate} ` + JSON.stringify(out.results[`fight_x${rate}`]));
  if (rate === throttles[0]) await shot("fight");
  await page.keyboard.up("Space");
  await sleep(4000);
}
console.log("RESULTS " + JSON.stringify(out));
await browser.close(); server.close();
