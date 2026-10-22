// Real keyboard play and pause regression for the fixed-tick foundation.
// Usage: PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json node scripts/perf/foundation.mjs dist/web verification/browser
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const require = createRequire(process.env.PLAYWRIGHT_NODE_MODULES ?? import.meta.url);
const { chromium } = require('playwright');
const dist = path.resolve(process.argv[2] ?? 'dist/web');
const out = path.resolve(process.argv[3] ?? 'verification/browser');
fs.mkdirSync(out, { recursive: true });
const types = { '.wasm': 'application/wasm', '.js': 'text/javascript', '.mjs': 'text/javascript', '.html': 'text/html', '.png': 'image/png' };
const server = http.createServer((req, res) => {
  let file;
  try { file = decodeURIComponent(req.url.split('?')[0]); }
  catch { res.writeHead(400); res.end(); return; }
  if (file.includes('\0')) { res.writeHead(400); res.end(); return; }
  if (file === '/favicon.ico') { res.writeHead(204); res.end(); return; }
  if (file.endsWith('/')) file += 'index.html';
  const target = path.resolve(dist, `.${file}`);
  if (!target.startsWith(`${dist}${path.sep}`)) { res.writeHead(403); res.end(); return; }
  fs.readFile(target, (error, data) => {
    if (error) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': types[path.extname(file)] ?? 'application/octet-stream' });
    res.end(data);
  });
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
// Exercise the actual test server's input boundary before launching Chromium.
const statusFor = requestPath => new Promise((resolve, reject) => {
  http.get({ host: '127.0.0.1', port: server.address().port, path: requestPath }, response => {
    response.resume(); resolve(response.statusCode);
  }).on('error', reject);
});
for (const [requestPath, expected] of [
  ['/%2e%2e/%2e%2e/Cargo.toml', 403], ['/%E0%A4%A', 400], ['/%00', 400],
]) {
  assert.equal(await statusFor(requestPath), expected, `unsafe static-file path ${requestPath}`);
}
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const errors = [], consoleLog = [];
try {
  const page = await browser.newPage({ viewport: { width: 960, height: 540 } });
  page.on('pageerror', error => errors.push(String(error)));
  page.on('console', message => {
    consoleLog.push({ type: message.type(), text: message.text() });
    if (message.type() === 'error') errors.push(message.text());
  });
  await page.addInitScript(() => { Date.now = () => 1790000000000; });
  const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
  const hold = async (key, ms = 5000) => {
    await page.keyboard.down(key); await sleep(ms);
    await page.keyboard.up(key); await sleep(4000);
  };
  const shot = name => page.screenshot({ path: path.join(out, `${name}.png`), timeout: 180000 });
  await page.goto(`http://127.0.0.1:${server.address().port}/?quality=0`);
  await page.waitForFunction(() => !document.getElementById('status'), null, { timeout: 180000 });
  await sleep(8000);
  await page.locator('#ryggattack-canvas').focus();
  await hold('Enter');
  await hold('w');
  await hold('ArrowRight');
  await hold('Enter'); // 3 -> 0 bots
  await shot('lobby-solo');
  await hold('ArrowLeft');
  await hold('Enter', 6000);
  await shot('playing');
  await hold('d');
  await hold('Space', 5000);
  await shot('firing');
  await hold('Escape', 2000);
  const paused = await shot('paused');
  await sleep(5000);
  const stillPaused = await shot('paused-later');
  assert(paused.equals(stillPaused), 'pause must freeze the rendered world and effect timers');
  await hold('Escape', 2000);
  const resumed = await shot('resumed');
  assert(!resumed.equals(paused), 'resume must return to gameplay');
  // Return through the actual pause menu, then claim the second keyboard seat.
  // Settings and the first seat persist; bots remain zero.
  await hold('Escape', 2000);
  await hold('ArrowDown');
  await hold('Enter');
  await hold('Enter');
  await hold('ArrowUp');
  await shot('two-keyboard-lobby');
  await hold('Enter', 6000);
  await shot('two-keyboard-playing');
  await page.keyboard.down('Space');
  await hold('ControlRight', 5000);
  await page.keyboard.up('Space');
  await shot('two-keyboard-firing');
  assert.deepEqual(errors, [], 'browser errors');
  fs.writeFileSync(path.join(out, 'browser.json'), JSON.stringify({
    errors, console: consoleLog, pausePixelsIdentical: true, staticPathChecksPassed: true,
    localStorage: await page.evaluate(() => ({ ...localStorage })),
  }, null, 2));
  console.log('PASS: static-path rejection, keyboard lobby, solo play, steering/fire, pixel-identical pause/resume, two keyboard seats; zero browser errors');
} finally {
  fs.writeFileSync(path.join(out, 'console.json'), JSON.stringify({ errors, console: consoleLog }, null, 2));
  await browser.close();
  server.close();
}
