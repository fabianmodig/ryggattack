// Still frames for visual-regression checks: the lobby and the first moments
// of a round, at a fixed size, so that two builds can be diffed.
// Usage: node shots.mjs <dist> <label> [--width W --height H --out DIR]
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
const width = Number(opt("width", "640")), height = Number(opt("height", "360"));
const outDir = opt("out", ".");
const types = { ".wasm": "application/wasm", ".js": "text/javascript", ".html": "text/html", ".png": "image/png" };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(req.url.split("?")[0]); if (p.endsWith("/")) p += "index.html";
  fs.readFile(path.join(dist, p), (e, d) => { if (e) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { "content-type": types[path.extname(p)] || "application/octet-stream" }); res.end(d); });
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
const page = await (await browser.newContext({ viewport: { width, height } })).newPage();
// The map is seeded from the wall clock (web_time -> Date.now), so pin it:
// every build then grows the same track and forest.
await page.addInitScript(() => { Date.now = () => 1790000000000; });
const shot = (n) => page.screenshot({ path: path.join(outDir, `shot-${label}-${n}.png`), timeout: 180000 });
const hold = async (key, ms = 5000) => { await page.keyboard.down(key); await sleep(ms); await page.keyboard.up(key); await sleep(4000); };
await page.goto(`http://127.0.0.1:${server.address().port}/`);
await page.waitForFunction(() => !document.getElementById("status"), null, { timeout: 180000 });
await sleep(8000);
await page.locator("#ryggattack-canvas").focus();
await hold("Enter");   // START -> lobby
await shot("lobby");
await hold("w");       // WASD takes seat 1
await hold("Enter", 6000);
await sleep(6000);
await shot("playing");
await browser.close(); server.close();
