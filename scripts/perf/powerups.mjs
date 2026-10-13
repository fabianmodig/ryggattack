// Real keyboard-driven rail pickup smoke test; no game-state injection.
// Usage: PLAYWRIGHT_NODE_MODULES=/path/to/node_modules node scripts/perf/powerups.mjs dist/web verification [forest|volcano]
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
const require = createRequire(process.env.PLAYWRIGHT_NODE_MODULES ?? import.meta.url);
const { chromium } = require("playwright");
const dist = path.resolve(process.argv[2] ?? "dist/web");
const out = path.resolve(process.argv[3] ?? "verification");
const arena = process.argv[4] ?? "forest";
assert(["forest", "volcano"].includes(arena));
fs.mkdirSync(out, { recursive: true });
const types = { ".wasm": "application/wasm", ".js": "text/javascript", ".mjs": "text/javascript", ".html": "text/html" };
const server = http.createServer((req, res) => {
  const pathname = new URL(req.url, "http://localhost").pathname;
  const filename = path.resolve(dist, `.${pathname.endsWith("/") ? `${pathname}index.html` : pathname}`);
  if (!filename.startsWith(`${dist}${path.sep}`)) { res.writeHead(403).end(); return; }
  fs.readFile(filename, (error, data) => {
    if (error) { res.writeHead(404).end(); return; }
    res.writeHead(200, { "content-type": types[path.extname(filename)] ?? "application/octet-stream" }).end(data);
  });
});
await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const errors = [];
const messages = [];
page.on("pageerror", error => errors.push(error.message));
page.on("console", message => {
  messages.push({ type: message.type(), text: message.text() });
  if (message.type() === "error") errors.push(message.text());
});
const sleep = ms => page.waitForTimeout(ms);
const shot = async name => {
  const filename = path.join(out, `powerups-${arena}-${name}.png`);
  await page.screenshot({ path: filename, timeout: 180000 });
  console.log(`SCREENSHOT ${filename}`);
};
const hold = async (key, ms = 1500) => {
  await page.keyboard.down(key); await sleep(ms);
  await page.keyboard.up(key); await sleep(1500);
};
try {
  // Pin the map and powerup runtime seeds for reproducible screenshots,
  // never the monotonic gameplay clock or game state.
  await page.addInitScript(() => { Date.now = () => 1790000000000; });
  await page.goto(`http://127.0.0.1:${server.address().port}/?quality=0`);
  await page.waitForFunction(() => !document.getElementById("status"), null, { timeout: 180000 });
  await sleep(8000);
  await page.locator("#ryggattack-canvas").focus();
  await shot("menu");
  await hold("Enter");
  await hold("w");
  await hold("ArrowRight"); // BOTS, default three -> zero
  await hold("Enter");
  if (arena === "volcano") {
    await hold("ArrowRight"); await hold("ArrowRight");
    await hold("Enter");
    await hold("ArrowLeft"); await hold("ArrowLeft");
  }
  await shot("lobby");
  await hold("ArrowLeft");
  await hold("Enter");
  await hold("F3");
  await shot("start");
  // Let drops accumulate and traverse multiple junction routes. Slow-frame
  // software rendering advances clamped game time, so wall time is generous.
  for (let index = 0; index < 20; index++) {
    const key = ["d", "s", "a", "w"][Math.floor(index / 3) % 4];
    await hold(key, 10000);
    if (index === 7) {
      await hold("Escape"); await shot("paused");
      await sleep(12000); await shot("paused-later");
      await hold("Escape");
    }
    if (index === 14) await hold("Space", 1500);
    await shot(`play-${String(index).padStart(2, "0")}`);
  }
  fs.writeFileSync(path.join(out, `powerups-${arena}-browser.json`), JSON.stringify({ arena, errors, messages, localStorage: await page.evaluate(() => ({ ...localStorage })) }, null, 2));
  assert.deepEqual(errors, [], "browser JavaScript/WASM errors");
  console.log(`PASS: ${arena} loaded, keyboard lobby/solo match, steering, pause/resume and firing; no browser errors. Screenshots require visual review for pickups and HUD.`);
} finally {
  await browser.close();
  await new Promise(resolve => server.close(resolve));
}
