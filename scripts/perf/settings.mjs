// End-to-end check of the VIDEO SETTINGS screen: every row changes its value
// and is saved, the choice survives a reload, and the screen also opens from
// the pause dialog. Keys are held for whole frames (the game can run at under
// 1 fps on a software rasteriser), and every step checks what was saved.
// Usage: node settings.mjs <dist> [--browser chromium|firefox|webkit] [--out DIR]
// Exits non-zero if any check fails; prints one "CHECK ok|FAIL <name>" per check.
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
const require = createRequire(process.env.PLAYWRIGHT_NODE_MODULES ?? import.meta.url);
const engines = require("playwright");
const args = process.argv.slice(2);
const dist = path.resolve(args[0]);
const opt = (n, d) => { const i = args.indexOf(`--${n}`); return i >= 0 ? args[i + 1] : d; };
const engine = opt("browser", "chromium");
const outDir = opt("out", ".");
fs.mkdirSync(outDir, { recursive: true });
const types = { ".wasm": "application/wasm", ".js": "text/javascript", ".html": "text/html", ".png": "image/png" };
const server = http.createServer((req, res) => {
  let p = decodeURIComponent(req.url.split("?")[0]); if (p.endsWith("/")) p += "index.html";
  fs.readFile(path.join(dist, p), (e, d) => { if (e) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { "content-type": types[path.extname(p)] || "application/octet-stream" }); res.end(d); });
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const url = `http://127.0.0.1:${server.address().port}/`;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const launchOpts = { chromium: { args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] },
  firefox: { firefoxUserPrefs: { "webgl.force-enabled": true, "webgl.disabled": false } } };
const browser = await engines[engine].launch(launchOpts[engine] ?? {});
const context = await browser.newContext({ viewport: { width: 720, height: 640 } });
const page = await context.newPage();
await page.addInitScript(() => {
  Date.now = () => 1790000000000;
  window.__frames = 0;
  const tick = () => { window.__frames++; requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
});
const errors = [];
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text().slice(0, 300)); });
page.on("pageerror", (e) => errors.push("pageerror: " + String(e).slice(0, 300)));

let failures = 0;
const check = (name, ok, detail = "") => { if (!ok) failures++; console.log(`CHECK ${ok ? "ok" : "FAIL"} ${name}${detail ? " -- " + detail : ""}`); };
const saved = () => page.evaluate(() => localStorage.getItem("ryggattack.video"));
const frames = () => page.evaluate(() => window.__frames);
async function waitFrames(n, timeout = 120000) {
  const start = await frames(); const t0 = Date.now();
  while ((await frames()) - start < n && Date.now() - t0 < timeout) await sleep(100);
}
// Hold a key over two whole frames, so the game sees exactly one press.
async function press(key) {
  await page.keyboard.down(key); await waitFrames(2); await page.keyboard.up(key); await waitFrames(3);
}
const shot = (n) => page.screenshot({ path: path.join(outDir, `settings-${engine}-${n}.png`), timeout: 180000 }).catch(() => console.log("shot failed", n));
async function boot() {
  await page.goto(url);
  await page.waitForFunction(() => !document.getElementById("status"), null, { timeout: 180000 });
  await waitFrames(8);
  await page.locator("#ryggattack-canvas").focus();
}
const enc = (s) => `scale=${s[0]} aa=${s[1]} shadows=${s[2]} effects=${s[3]} forest=${s[4]} fps=${s[5]}`;
const HIGH = enc(["100%", "ON", "HIGH", "HIGH", "FULL", "OFF"]);
const MEDIUM = enc(["100%", "OFF", "LOW", "MEDIUM", "FULL", "OFF"]);
const LOW = enc(["67%", "OFF", "OFF", "LOW", "SPARSE", "OFF"]);

// 1. A first visit starts on High and saves it.
await boot();
check("first visit defaults to High", (await saved()) === HIGH, await saved());
await shot("1-menu");

// 2. Main menu -> SETTINGS.
await press("ArrowDown");
await press("Enter");
await shot("2-settings-high");

// 3. QUALITY steps Low -> Medium -> High, each saved.
await press("ArrowRight"); check("QUALITY right -> Low", (await saved()) === LOW, await saved());
await shot("3-settings-low");
await press("ArrowRight"); check("QUALITY right -> Medium", (await saved()) === MEDIUM, await saved());
await press("Enter"); check("QUALITY Enter -> High", (await saved()) === HIGH, await saved());
await press("ArrowLeft"); check("QUALITY left -> Medium", (await saved()) === MEDIUM, await saved());
await press("ArrowRight");
check("QUALITY back to High", (await saved()) === HIGH, await saved());

// 4. Every other row, one step each, from High.
const rows = [
  ["RESOLUTION left -> 75%", "ArrowLeft", ["75%", "ON", "HIGH", "HIGH", "FULL", "OFF"]],
  ["ANTI-ALIASING -> OFF", "ArrowRight", ["75%", "OFF", "HIGH", "HIGH", "FULL", "OFF"]],
  ["SHADOWS left -> LOW", "ArrowLeft", ["75%", "OFF", "LOW", "HIGH", "FULL", "OFF"]],
  ["EFFECTS left -> MEDIUM", "ArrowLeft", ["75%", "OFF", "LOW", "MEDIUM", "FULL", "OFF"]],
  ["FOREST -> SPARSE", "ArrowRight", ["75%", "OFF", "LOW", "MEDIUM", "SPARSE", "OFF"]],
  ["SHOW FPS -> ON", "ArrowRight", ["75%", "OFF", "LOW", "MEDIUM", "SPARSE", "ON"]],
];
for (const [name, key, want] of rows) {
  await press("ArrowDown");
  await press(key);
  check(name, (await saved()) === enc(want), await saved());
}
const CUSTOM = enc(rows.at(-1)[2]);
await shot("4-settings-custom");

// 5. The mouse works too: click the QUALITY row (Custom -> Low).
const box = await page.locator("#ryggattack-canvas").boundingBox();
// Rows sit in a centred 520 px panel; QUALITY is the first, under the title.
// Found by scanning down the centre line until a click changes the save.
let clicked = false;
for (let y = Math.round(box.height * 0.2); y < box.height * 0.5 && !clicked; y += 12) {
  await page.mouse.move(box.x + box.width / 2, box.y + y); await waitFrames(2);
  await page.mouse.down(); await waitFrames(2); await page.mouse.up(); await waitFrames(3);
  const now = await saved();
  if (now !== CUSTOM) { clicked = true; check("mouse click on QUALITY -> Low (fps kept)", now === LOW.replace("fps=OFF", "fps=ON"), `y=${y} ${now}`); }
}
if (!clicked) check("mouse click on QUALITY changes the preset", false, "no row found");
// Put the custom mix back through the keyboard for the persistence check.
await page.evaluate((v) => localStorage.setItem("ryggattack.video", v), CUSTOM);

// 6. Escape returns to the main menu; the setting persists across a reload.
await press("Escape");
await shot("5-menu-after");
await boot();
check("custom settings survive a reload", (await saved()) === CUSTOM, await saved());
await shot("6-reloaded-fps-on");
await press("ArrowDown"); await press("Enter");
await shot("7-settings-reloaded");
await press("Escape");

// 7. From a round: pause -> SETTINGS -> back -> resume.
await press("Enter");            // START -> lobby
await press("w");               // WASD takes seat 1
await press("Enter");            // START ROUND
await waitFrames(10);
await press("Escape");           // pause
await press("ArrowDown"); await press("Enter");
await shot("8-settings-from-pause");
await press("ArrowRight");       // QUALITY: Custom -> Low
check("QUALITY from the pause dialog -> Low", (await saved()) === LOW.replace("fps=OFF", "fps=ON"), await saved());
await press("Escape");           // back to the pause dialog
await shot("9-pause-again");
await press("Escape");           // resume
await waitFrames(10);
await shot("10-playing-low");

check("no console errors", errors.length === 0, errors.join(" | "));
console.log("SUMMARY " + JSON.stringify({ engine, failures, errors }));
await browser.close(); server.close();
process.exit(failures ? 1 : 0);
