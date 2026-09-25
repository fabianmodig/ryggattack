// In-game draw-call anatomy: per-draw instance counts, drawArrays vs drawElements,
// split per render pass (framebuffer binding), for one representative frame set.
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
const require = createRequire(process.env.PLAYWRIGHT_NODE_MODULES ?? import.meta.url);
const { chromium } = require("playwright");
const dist = path.resolve(process.argv[2]);
const fire = process.argv.includes("--fire");
const types = { ".wasm": "application/wasm", ".js": "text/javascript", ".html": "text/html", ".png": "image/png" };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(req.url.split("?")[0]); if (p.endsWith("/")) p += "index.html";
  fs.readFile(path.join(dist, p), (e, d) => { if (e) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { "content-type": types[path.extname(p)] || "application/octet-stream" }); res.end(d); });
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
const page = await (await browser.newContext({ viewport: { width: 320, height: 180 } })).newPage();
await page.addInitScript(() => {
  const P = WebGL2RenderingContext.prototype;
  const g = (window.__a = { rec: false, frames: [], cur: null });
  const newFrame = () => ({ passes: [], pass: null });
  const ensurePass = (f) => { if (!f.pass) { f.pass = { fb: "?", draws: [] }; f.passes.push(f.pass); } return f.pass; };
  const wrap = (name, fn) => { const o = P[name]; P[name] = function (...a) { if (g.rec && g.cur) fn(a); return o.apply(this, a); }; };
  wrap("bindFramebuffer", (a) => { const f = g.cur; f.pass = { fb: a[1] ? "fbo" : "canvas", draws: [] }; f.passes.push(f.pass); });
  wrap("framebufferTexture2D", (a) => { const p = ensurePass(g.cur); p.att = (p.att || 0) + 1; if (a[1] === 0x8d00 || a[1] === 0x821a) p.depthOnly = true; });
  wrap("drawElementsInstanced", (a) => ensurePass(g.cur).draws.push(["E", a[1], a[4]]));
  wrap("drawArraysInstanced", (a) => ensurePass(g.cur).draws.push(["A", a[2], a[3]]));
  wrap("drawElements", (a) => ensurePass(g.cur).draws.push(["E", a[1], 1]));
  wrap("drawArrays", (a) => ensurePass(g.cur).draws.push(["A", a[2], 1]));
  const tick = () => { if (g.rec) { if (g.cur) g.frames.push(g.cur); g.cur = newFrame(); } requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
});
const hold = async (key, ms = 3500) => { await page.keyboard.down(key); await sleep(ms); await page.keyboard.up(key); await sleep(2500); };
await page.goto(`http://127.0.0.1:${server.address().port}/`);
await page.waitForFunction(() => !document.getElementById("status"), null, { timeout: 180000 });
await sleep(5000);
await page.locator("#ryggattack-canvas").focus();
await hold("Enter"); await hold("w"); await hold("Enter", 4000); await sleep(4000);
if (fire) { await page.keyboard.down("Space"); await sleep(6000); }
await page.evaluate(() => { window.__a.rec = true; });
await sleep(8000);
const frames = await page.evaluate(() => { const g = window.__a; g.rec = false; return g.frames.filter((f) => f.passes.some((p) => p.draws.length)); });
const f = frames.at(-1);
console.log(`frames captured: ${frames.length}; analysing the last one with draws`);
for (const [i, p] of f.passes.entries()) {
  if (!p.draws.length) continue;
  const n = p.draws.length;
  const inst = p.draws.reduce((s, d) => s + d[2], 0);
  const single = p.draws.filter((d) => d[2] === 1).length;
  const arrays = p.draws.filter((d) => d[0] === "A").length;
  const verts = p.draws.reduce((s, d) => s + d[1] * d[2], 0);
  const hist = {};
  for (const d of p.draws) { const b = d[2] === 1 ? "1" : d[2] <= 4 ? "2-4" : d[2] <= 16 ? "5-16" : d[2] <= 64 ? "17-64" : ">64"; hist[b] = (hist[b] || 0) + 1; }
  console.log(`pass ${i} target=${p.fb}${p.depthOnly ? " (depth-only/shadow?)" : ""}: draws=${n} instances=${inst} single-instance draws=${single} drawArrays=${arrays} verts=${verts} hist=${JSON.stringify(hist)}`);
}
await browser.close(); server.close();
