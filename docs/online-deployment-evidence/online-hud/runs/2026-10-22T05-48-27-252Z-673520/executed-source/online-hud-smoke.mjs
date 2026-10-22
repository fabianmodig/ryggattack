// Local actual-artifact acceptance. Never builds, deploys, or contacts a public room.
// Parent must finish native + WASM builds first; only one Bevy renderer is opened.
// Scope: same client/socket/private room, real target-seven input/UI preflight,
// natural Lobby return, then a separately scoped original target-one core match.
// Neither case can borrow another case's states, winner, DOM proof or deadline.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {spawn} from 'node:child_process';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {canonical, redact} from './live-deployment-helpers.mjs';
import {checkCopiedEntryArtifacts} from './online-hud-artifacts.mjs';
import {MATCH_CASES, createMatchCase, caseForSample, caseAtPageTime, assertFreshReset, isLobbyClear, assertOutcomeTimeline, observeOutcomeTransition} from './online-hud-cases.mjs';
import {capturePrivateScreenshot, PrivacyViolation, publishSoloEvidence} from './solo-bots-helpers.mjs';

const usage = `Usage: node scripts/online-hud-smoke.mjs [--local]
Verify parent-built dist/web against an owned loopback native backend.
No build, public endpoint, custom origin, state injection, or extra game client.
PLAYWRIGHT_NODE_MODULES may name the installed Playwright package.json.
RYGGATTACK_NATIVE_BIN_DIR may name the parent-built native binary directory.
Outputs: verification/online-hud/<timestamp>/ (failed runs retained).
Scope: target-seven real input/UI preflight, then target-one natural core,
same WASM client and private room. Each match completes naturally within 180s.
Screenshot timeout: 60 seconds per capture only; never extends either match cap.
A separate, labeled presentation-only fixture runs AFTER the game page closes.`;
const argv = process.argv.slice(2);
if (argv.length === 1 && ['--help', '-h'].includes(argv[0])) {
  console.log(usage); process.exit(0);
}
if (!(argv.length === 0 || (argv.length === 1 && argv[0] === '--local'))) {
  console.error(usage); process.exit(2);
}
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const web = path.join(repo, 'dist/web');
const bin = path.resolve(process.env.RYGGATTACK_NATIVE_BIN_DIR ?? path.join(repo, 'target/debug'));
const out = path.join(repo, 'verification/online-hud', new Date().toISOString().replace(/[:.]/g, '-') + '-' + crypto.randomBytes(3).toString('hex'));
fs.mkdirSync(out, {recursive: true});
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const started = performance.now(), elapsed = () => Math.round(performance.now() - started);
const secrets = new Set();
const report = {
  status: 'running', started_at: new Date().toISOString(), scope: 'LOCAL ONLY: same one real WASM human + one bot, same owned room/socket; target-seven real input/UI preflight naturally completes, then original target-one natural core with early held Space; no forced result or restart',
  budgets: {preflight_completion_ms: 180000, natural_completion_ms: 180000, screenshot_only_ms: 60000},
  hashes: {}, checks: [], errors: [], screenshots: [], protocol: [], lobbies: [],
  preflight: createMatchCase(MATCH_CASES[0], null).stats,
  natural: createMatchCase(MATCH_CASES[1], null).stats,
  fixture: {label: 'ISOLATED SYNTHETIC PRESENTATION ONLY — NOT NATURAL GAMEPLAY OR BUFF PROOF', checks: []}, cleanup: {}
};
let browser, browserServer, page, fixture, backend, backendExit, server, origin, address;
let identity, lobby, activeCase, leaveSent = false, privacyFailed = false, interrupted = false;
const cases = new Map(), inputs = [], commands = [], capturedSamples = [], pendingSamples = [];
const interrupt = () => { interrupted = true; };
process.on('SIGINT', interrupt); process.on('SIGTERM', interrupt);
const fault = (source, message) => report.errors.push({source, message: String(message), at_ms: elapsed()});
async function until(predicate, label, timeout = 90000, cleanup = false) {
  const end = performance.now() + timeout;
  while (!predicate()) {
    assert(cleanup || !interrupted, 'interrupted');
    assert(performance.now() < end, label);
    await sleep(25);
  }
}
function filesBelow(directory) {
  return fs.readdirSync(directory, {withFileTypes: true}).sort((a, b) => a.name.localeCompare(b.name)).flatMap(entry => {
    const name = path.join(directory, entry.name);
    assert(!entry.isSymbolicLink(), 'source/artifact symlinks are not accepted');
    return entry.isDirectory() ? filesBelow(name) : entry.isFile() ? [name] : [];
  });
}
function hashInputs() {
  // Superset of consumed project Rust, all web entry modules, exact generated
  // JS/WASM and helper bytes. No copies or fabricated dist artifacts are made.
  const source = [...filesBelow(path.join(repo, 'src')).filter(f => f.endsWith('.rs')),
    ...filesBelow(path.join(repo, 'web')),
    ...filesBelow(path.join(repo, 'scripts')).filter(f => /\.(?:mjs|js|sh)$/.test(f)),
    path.join(repo, 'Cargo.toml'), path.join(repo, 'Cargo.lock')];
  const artifacts = filesBelow(web);
  for (const required of ['index.html', 'rooms.mjs', 'refresh-rate.mjs', 'online-hud.mjs', 'online-hud-bridge.mjs', 'ryggattack.js', 'ryggattack_bg.wasm']) {
    assert(artifacts.includes(path.join(web, required)), `missing parent-built artifact: ${required}`);
  }
  const result = {};
  for (const file of [...new Set([...source, ...artifacts, path.join(bin, 'ryggattack-server')])].sort()) {
    const bytes = fs.readFileSync(file);
    result[path.relative(repo, file)] = {sha256: sha(bytes), bytes: bytes.length};
  }
  // Compare only the actual app entry graph, not unrelated retired Pages HTML.
  // Generated JS/WASM remain separately required, hashed and MIME-checked.
  checkCopiedEntryArtifacts(path.join(repo, 'web'), web);
  assert(fs.readFileSync(path.join(web, 'ryggattack_bg.wasm')).subarray(0, 4).equals(Buffer.from([0, 97, 115, 109])), 'actual WASM magic');
  return result;
}
function projection(frame) {
  return Object.fromEntries(['tick', 'simulation_tick', 'names', 'wins', 'winner', 'round_finished', 'transition'].map(key => [key, frame[key]]).concat([
    ['carts', frame.carts.map(cart => Object.fromEntries(['id', 'active', 'alive', 'speed_seconds', 'all_hits_seconds'].map(key => [key, cart[key]])))]]));
}
function same(actual, expected, label) { assert.deepEqual(canonical(actual), canonical(expected), label); }
function model(payload) {
  const f = payload.lobby?.phase === 'Running' && Number.isInteger(payload.seat) ? payload.frame : null;
  const players = (f?.carts ?? []).filter(c => c.active).sort((a, b) => a.id - b.id).map(c => ({
    seat: c.id, name: f.names[c.id]?.trim() || `Bot ${c.id + 1}`, score: f.wins[c.id], out: !c.alive,
    buffs: [['Speed', c.speed_seconds], ['All-hits', c.all_hits_seconds]].filter(([, seconds]) => seconds > 0)
  }));
  const survivor = players.filter(p => !p.out);
  const match = players.find(p => p.seat === f?.winner);
  const win = match || (f?.round_finished && survivor.length === 1 ? survivor[0] : null);
  const outcome = win ? `${win.name} wins the ${match ? 'match' : 'round'}` : f?.round_finished && players.length && !survivor.length ? 'Round draw' : '';
  return {players, names: players.map(p => p.name + (p.seat === payload.seat ? ' (you)' : '')),
    states: players.map(p => [`${p.score} wins`, p.out ? 'OUT' : 'IN', ...p.buffs.map(([label, seconds]) => `${label} ${(Math.ceil(seconds * 10) / 10).toFixed(1)}s`)].join(' · ')), outcome};
}
function validateSample(sample) {
  const p = sample.payload;
  const match = caseForSample(cases, sample), stats = match?.stats;
  if (match) {
    stats.dom_transition = match.domTransition;
    observeOutcomeTransition(match.domTransition, sample, identity.room, match.plan.id);
  }
  if (!p || !sample.mounted) return false;
  assert.equal(typeof sample.raw, 'string', 'HUD is an actual serialized publication');
  assert.deepEqual(Object.keys(p).sort(), ['frame', 'lobby', 'menu', 'notice', 'rtt_ms', 'seat', 'status'], 'HUD public whitelist');
  if (p.lobby) {
    assert(identity, 'HUD admission requires observed real Welcome');
    assert.deepEqual(Object.keys(p.lobby).sort(), ['id', 'phase'], 'minimal lobby publication');
    assert.equal(p.lobby.id, identity.room);
    const local = sample.room?.lobby;
    assert(local, 'coherent room and HUD lobby');
    same(p.lobby, {id: local.id, phase: local.phase}, 'coherent lobby publication');
    assert((match?.lobbies ?? report.lobbies).some(observed => observed.id === local.id && JSON.stringify(canonical(observed.phase)) === JSON.stringify(canonical(local.phase)) && observed.humans.some(h => h.id === identity.participant && h.seat === p.seat)), 'HUD lobby and participant seat came from an observed authoritative Lobby in this case');
    assert.equal(sample.room.participant, identity.participant);
    assert(local.humans.some(h => h.id === identity.participant && h.seat === p.seat), 'coherent participant seat');
  } else {
    assert.equal(p.seat, null); assert.equal(p.frame, null);
  }
  if (p.frame) {
    assert.equal(p.lobby.phase, 'Running'); assert.equal(p.seat, identity.seat);
    const authoritative = match?.states.get(p.frame.tick);
    if (!authoritative) return false; // CDP event dispatch may trail the binding.
    same(p.frame, projection(authoritative), 'published HUD equals actual authoritative State projection');
    // Compare with the most recent wire State at OBSERVATION, never at delayed drain.
    const lag = Math.max(0, sample.latest_wire_tick - p.frame.tick);
    assert(lag <= 64, 'published HUD may not lag latest observed authoritative State by >1 second');
    stats.max_published_tick_lag = Math.max(stats.max_published_tick_lag, lag);
    stats.matched_frames++;
  }
  // Running can precede the first State by one transport turn. The required
  // nonempty roster gate below prevents such a publication counting as proof.
  const expected = model(p);
  const matches = sample.visible && JSON.stringify(sample.names) === JSON.stringify(expected.names) && JSON.stringify(sample.states) === JSON.stringify(expected.states) && sample.outcome === expected.outcome;
  if (!stats) return false;
  stats.samples++;
  if (!matches) return false; // The production bridge intentionally polls at 100 ms.
  stats.dom_matches++;
  if (p.frame && expected.players.length) {
    assert.equal(expected.players.length, 2, 'one real human + one bot');
    const safe = {case_id: sample.case_id, at_ms: sample.at_ms, page_ms: sample.page_ms, phase: p.lobby.phase, tick: p.frame.tick, names: sample.names, states: sample.states, outcome: sample.outcome};
    if (!stats.first_dom) {
      assertFreshReset(p.frame);
      assert(sample.names.includes('HudLocal (you)'), 'actual authoritative human name and own-seat label');
      const bot = expected.players.find(player => player.seat !== identity.seat);
      assert.equal(p.frame.names[bot.seat] ?? null, null, 'local bot has no human name');
      assert(sample.names.includes(`Bot ${bot.seat + 1}`), 'natural active bot fallback rendered');
      stats.first_dom = safe;
      stats.fresh_reset_dom = safe;
    }
    if (expected.players.some(player => player.out)) stats.out.push(safe);
    if (expected.players.some(player => player.buffs.length)) stats.buffs.push(safe);
    if (expected.outcome) stats.outcomes.push(safe);
  }
  return true;
}
async function drainSamples() {
  while (pendingSamples.length) {
    const sample = pendingSamples.shift();
    capturedSamples.push(sample);
    validateSample(sample);
  }
}
async function expand() {
  if (!await page.locator('#online-entry').evaluate(e => e.open)) await page.locator('#online-entry summary').click();
}
async function screenshot(target, name, scope) {
  if (privacyFailed) throw new PrivacyViolation();
  const before = elapsed(), phaseStart = lobby?.phase ?? null, caseId = activeCase?.plan.id ?? null;
  let publishedStart = null, publishedEnd = null;
  try {
    publishedStart = await target.evaluate(() => { try { return JSON.parse(globalThis.ryggattackHud ?? 'null')?.lobby?.phase ?? null; } catch { return null; } });
    await capturePrivateScreenshot(target, secrets, () => target.screenshot({path: path.join(out, name + '.png'), timeout: 60000}));
    publishedEnd = await target.evaluate(() => { try { return JSON.parse(globalThis.ryggattackHud ?? 'null')?.lobby?.phase ?? null; } catch { return null; } });
  } catch (error) {
    if (error instanceof PrivacyViolation) privacyFailed = true;
    throw error;
  }
  report.screenshots.push({file: name + '.png', scope, case_id: caseId, started_ms: before, duration_ms: elapsed() - before,
    phase_at_start: phaseStart, phase_at_end: lobby?.phase ?? null, published_phase_at_start: publishedStart, published_phase_at_end: publishedEnd,
    caveat: 'Capture can span phase changes; does not prove an earlier winner. Fast DOM observations supply result proof.', sha256: sha(fs.readFileSync(path.join(out, name + '.png')))});
}
async function startHttp() {
  server = http.createServer((req, res) => {
    let file;
    try {
      let pathname = decodeURIComponent(req.url.split('?')[0]);
      if (pathname.includes('\0')) throw Error('NUL');
      if (pathname.endsWith('/')) pathname += 'index.html';
      file = path.resolve(web, '.' + pathname);
    } catch { res.writeHead(400).end(); return; }
    if (!file.startsWith(web + path.sep)) { res.writeHead(403).end(); return; }
    // Resolve real paths too, so an artifact symlink cannot escape the sandbox.
    fs.realpath(file, (error, real) => {
      if (error) { res.writeHead(404).end(); return; }
      if (!real.startsWith(fs.realpathSync(web) + path.sep)) { res.writeHead(403).end(); return; }
      fs.readFile(real, (error, bytes) => {
        if (error) { res.writeHead(404).end(); return; }
        res.writeHead(200, {'content-type': ({'.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm'})[path.extname(file)] ?? 'application/octet-stream'}).end(bytes);
      });
    });
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  origin = `http://127.0.0.1:${server.address().port}`;
  for (const [rawPath, expected] of [['/%2e%2e/Cargo.toml', 403], ['/%ZZ', 400], ['/%00', 400]]) {
    const status = await new Promise((resolve, reject) => {
      const req = http.get({host: '127.0.0.1', port: server.address().port, path: rawPath}, res => { res.resume(); resolve(res.statusCode); });
      req.setTimeout(5000, () => req.destroy(Error('sandbox test timeout'))); req.on('error', reject);
    });
    assert.equal(status, expected); report.checks.push({raw_path: rawPath, status});
  }
  for (const file of filesBelow(web)) {
    const relative = path.relative(web, file).split(path.sep).join('/');
    const response = await fetch(`${origin}/${relative}`, {redirect: 'error', signal: AbortSignal.timeout(30000)});
    assert.equal(response.status, 200);
    const bytes = Buffer.from(await response.arrayBuffer());
    assert.equal(sha(bytes), report.hashes[path.relative(repo, file)].sha256, 'served exact artifact bytes');
    if (/\.(?:mjs|js)$/.test(file)) assert.match(response.headers.get('content-type'), /^(text|application)\/javascript\b/);
    if (file.endsWith('.wasm')) assert.match(response.headers.get('content-type'), /^application\/wasm\b/);
  }
}
async function startBackend() {
  backend = spawn(path.join(bin, 'ryggattack-server'), ['127.0.0.1:0'], {stdio: ['ignore', 'pipe', 'pipe'], env: {...process.env, RYGGATTACK_ALLOWED_ORIGINS: origin}});
  backendExit = new Promise(resolve => { backend.once('exit', (code, signal) => resolve({code, signal})); backend.once('error', error => resolve({error: error.message})); });
  report.server_events = [];
  backend.stderr.on('data', bytes => report.server_events.push({at_ms: elapsed(), text: String(bytes)}));
  address = await new Promise((resolve, reject) => {
    let text = '';
    const timer = setTimeout(() => reject(Error('native backend startup timeout')), 10000);
    const finish = (error, value) => { clearTimeout(timer); error ? reject(error) : resolve(value); };
    backend.on('error', error => finish(error));
    backend.once('exit', code => finish(Error(`native backend exited ${code}`)));
    backend.stdout.on('data', bytes => {
      text += String(bytes); const match = /ws:\/\/127\.0\.0\.1:(\d+)/.exec(text);
      if (match) finish(null, `ws://127.0.0.1:${match[1]}`);
    });
  });
  report.origin = origin; report.backend = address;
}
function observeSocket(ws) {
  // Browser URL serialization adds '/' to a bare ws://host:port. Compare URL
  // identities rather than rejecting that harmless normalization. Event handlers
  // record failures instead of throwing past the evidence/cleanup finally block.
  if (new URL(ws.url()).href !== new URL(address).href) fault('scope', 'unexpected WebSocket endpoint (owned loopback backend only)');
  report.protocol.push({kind: 'socket', url: ws.url(), at_ms: elapsed()});
  ws.on('socketerror', error => fault('socket', error));
  ws.on('framereceived', ({payload}) => {
    try {
      const message = JSON.parse(String(payload));
      redact(message, secrets); // Discover first. NEVER retain the raw Welcome.
      if (message.Welcome) {
        assert(!identity, 'one admission only');
        identity = Object.fromEntries(['version', 'room', 'participant', 'seat', 'code'].map(key => [key, message.Welcome[key]]));
        report.identity = identity;
      }
      if (message.Lobby) {
        lobby = message.Lobby;
        assert.equal(lobby.humans.length, 1, 'only one human in owned room');
        const observed = {...lobby, at_ms: elapsed(), case_id: activeCase?.plan.id ?? null};
        report.lobbies.push(observed);
        if (activeCase) {
          activeCase.lobbies.push(observed);
          if (lobby.phase === 'Running' && activeCase.stats.completion_started_ms === undefined) {
            activeCase.stats.completion_started_ms = elapsed();
          }
        }
      }
      if (message.State) {
        assert(activeCase && lobby.phase === 'Running', 'State belongs to an active Running case');
        const latest = message.State;
        activeCase.latest = latest; activeCase.states.set(latest.tick, latest);
        if (!activeCase.stats.first_state) {
          assertFreshReset(latest);
          activeCase.stats.first_state = {frame: projection(latest), at_ms: elapsed()};
        }
        if (latest.winner !== null && latest.winner !== undefined && !activeCase.winner) {
          activeCase.winner = latest; activeCase.stats.winner = {frame: projection(latest), at_ms: elapsed()};
        }
      }
      if (message.Error || message.Notice) fault('protocol', message.Error ?? message.Notice);
    } catch (error) { fault('received-frame', error.message); }
  });
  ws.on('framesent', ({payload}) => {
    try {
      const message = JSON.parse(String(payload)); redact(message, secrets);
      const kind = typeof message === 'string' ? message : Object.keys(message)[0];
      assert(['Hello', 'Configure', 'Ready', 'Input', 'Leave'].includes(kind), 'no accidental room/game commands');
      if (kind === 'Input') {
        const input = message.Input; inputs.push({...input, at_ms: elapsed(), case_id: activeCase?.plan.id ?? null});
        assert(activeCase, 'Input belongs to an active case'); activeCase.stats.input.count++;
        assert.equal(input.horizontal, 0, 'neutral steering only'); assert.equal(input.vertical, 0, 'neutral steering only');
        if (input.fire) activeCase.stats.input.neutral_fire++;
      } else {
        commands.push(kind); report.protocol.push({kind, at_ms: elapsed(), case_id: activeCase?.plan.id ?? null});
        if (kind === 'Hello') { assert.equal(message.Hello.name, 'HudLocal'); assert.equal(message.Hello.room ?? null, null); }
        if (kind === 'Configure') {
          assert(activeCase && !activeCase.stats.configure_sent, 'one intentional Configure per case');
          assert.equal(message.Configure.bots, 1); assert.equal(message.Configure.wins_target, activeCase.plan.target);
          activeCase.stats.configure_sent = true;
        }
        if (kind === 'Ready') {
          assert(activeCase && activeCase.stats.configure_sent && !activeCase.stats.ready_sent, 'one intentional Ready per configured case');
          assert.equal(message.Ready, true); activeCase.stats.ready_sent = true;
        }
        if (kind === 'Leave') leaveSent = true;
      }
    } catch (error) { fault('sent-frame', error.message); }
  });
}
async function installObserver() {
  // Read-only publication/DOM observation. It does not wrap WebSocket, invoke the
  // bridge, assign any application global, or send any game command.
  await page.exposeBinding('__onlineHudEvidence', (_source, sample) => {
    const match = caseAtPageTime(cases, sample.page_ms);
    sample.at_ms = elapsed(); sample.case_id = match?.plan.id ?? null;
    sample.latest_wire_tick = match?.latest?.tick ?? sample.payload?.frame?.tick ?? 0;
    pendingSamples.push(sample);
  });
  await page.evaluate(() => {
    let previous = '', last = 0;
    setInterval(() => {
      const root = document.querySelector('.rygg-online-hud');
      let payload, room;
      try { payload = JSON.parse(globalThis.ryggattackHud ?? 'null'); room = JSON.parse(globalThis.ryggattackRoom ?? 'null'); } catch { return; }
      const sample = {page_ms: performance.now(), raw: globalThis.ryggattackHud, payload, room,
        mounted: globalThis.ryggattackHudMounted === true, visible: !!root && !root.hidden && root.getClientRects().length > 0,
        names: [...document.querySelectorAll('.rygg-online-hud .oh-name')].map(e => e.textContent),
        states: [...document.querySelectorAll('.rygg-online-hud .oh-state')].map(e => e.textContent),
        outcome: root?.querySelector('.oh-outcome:not([hidden])')?.textContent ?? '',
        player_count: root?.querySelectorAll('.oh-player').length ?? null,
        outcome_hidden: root?.querySelector('.oh-outcome')?.hidden === true,
        buffs_absent: !!root && !root.textContent.includes('All-hits ') && !root.textContent.includes('Speed '),
        menu: root?.querySelector('.oh-menu:not([hidden])')?.textContent ?? '',
        diagnostics_open: root?.querySelector('details.oh-diagnostics')?.open ?? false};
      // Always sample at 50ms, not only on a winner socket event. This captures
      // persistence during the server's result interval independently.
      const key = JSON.stringify([sample.names, sample.states, sample.outcome, sample.menu, payload?.lobby?.phase]);
      if (key !== previous || performance.now() - last >= 50) {
        previous = key; last = performance.now(); globalThis.__onlineHudEvidence(sample).catch(() => {});
      }
    }, 25);
  });
}
async function neutralWindow(label, action) {
  const tick = activeCase.latest.tick, controlCount = commands.length;
  await action();
  // Only packets AFTER the real key-down count; pre-focus/menu packets cannot
  // accidentally satisfy the suppression gate while the action is in flight.
  const from = inputs.length;
  await until(() => inputs.length >= from + 3 || lobby.phase !== 'Running', `${label}: real neutral input samples`, 5000);
  const samples = inputs.slice(from);
  assert.equal(lobby.phase, 'Running', `${label}: game ended before input window; not a pass`);
  assert(samples.length >= 3 && samples.every(i => !i.fire && i.horizontal === 0 && i.vertical === 0), `${label}: neutral input`);
  assert(activeCase.latest.tick > tick, `${label}: server keeps advancing`);
  assert.equal(commands.length, controlCount, `${label}: no accidental commands`);
  activeCase.stats.checks.push({input_window: label, count: samples.length, from_tick: tick, to_tick: activeCase.latest.tick});
  await drainSamples();
}
async function runNatural() {
  const {chromium} = createRequire(process.env.PLAYWRIGHT_NODE_MODULES ?? '/home/hermes/.npm/_npx/e41f203b7505f1fb/node_modules/playwright/package.json')('playwright');
  // Keep the exact owned Chromium child handle for bounded graceful cleanup;
  // launchServer + connect still launches ONE browser and ONE game renderer.
  browserServer = await chromium.launchServer({host: '127.0.0.1', args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--disable-backgrounding-occluded-windows', '--disable-renderer-backgrounding']});
  browser = await chromium.connect(browserServer.wsEndpoint());
  report.browser_version = browser.version();
  // Match the established rendered-network harness viewport on SwiftShader.
  // Desktop/narrow geometry is checked separately without a second game renderer.
  const viewport = {width: 640, height: 480};
  report.viewport = viewport;
  const context = await browser.newContext({viewport, ignoreHTTPSErrors: false});
  // Enforce local-only even for accidental runtime fetches or external resources.
  await context.route('**/*', route => {
    const url = new URL(route.request().url());
    if (url.origin === origin || ['data:', 'blob:'].includes(url.protocol)) return route.continue();
    fault('scope', 'blocked nonlocal browser request'); return route.abort();
  });
  page = await context.newPage(); page.setDefaultTimeout(30000); page.setDefaultNavigationTimeout(45000);
  page.on('websocket', observeSocket);
  page.on('pageerror', error => fault('page', error.message));
  page.on('crash', () => fault('page', 'renderer crash'));
  page.on('console', message => { if (message.type() === 'error') fault('console', message.text()); });
  page.on('requestfailed', request => fault('request', `${request.url()} ${request.failure()?.errorText}`));
  const session = await context.newCDPSession(page);
  await session.send('Network.enable');
  session.on('Network.webSocketWillSendHandshakeRequest', event => {
    const observed = Object.entries(event.request.headers).find(([key]) => key.toLowerCase() === 'origin')?.[1];
    report.protocol.push({kind: 'handshake', origin: observed});
    if (observed !== origin) fault('origin', 'unexpected WebSocket Origin');
  });
  await page.goto(`${origin}/?quality=0`, {waitUntil: 'domcontentloaded'});
  await expand(); await page.locator('#backend').fill(address); await page.locator('#player-name').fill('HudLocal');
  await page.locator('#create-room').click();
  await until(() => identity && lobby, 'real HTML form admission');
  await page.waitForFunction(() => JSON.parse(globalThis.ryggattackRoom ?? '{}').lobby);
  await installObserver();
  for (const plan of MATCH_CASES) {
    // The previous case must complete and clear the real presentation before this
    // scope exists. Tick counters can repeat across matches, so never share maps.
    if (activeCase) assert(activeCase.stats.returned_lobby && activeCase.stats.completed, 'previous case naturally completed');
    await drainSamples();
    const pageBoundary = await page.evaluate(() => performance.now());
    activeCase = createMatchCase(plan, elapsed()); cases.set(plan.id, activeCase);
    activeCase.observed_from_page_ms = pageBoundary;
    // Include this case's actual initial Lobby (not another case's Running or
    // return history), so publications during ordinary settings edits are valid.
    activeCase.lobbies.push({...lobby, at_ms: elapsed(), case_id: plan.id});
    activeCase.stats.observed_from_page_ms = pageBoundary;
    report[plan.id] = activeCase.stats;
    await expand(); await page.locator('#bots-room').fill('1');
    await page.locator('#target-room').selectOption(String(plan.target)); await page.locator('#apply-room').click();
    await until(() => activeCase.stats.configure_sent && lobby.settings.bots === 1 && lobby.settings.wins_target === plan.target, `${plan.id}: ordinary one-bot Configure`);
    activeCase.stats.configured_lobby = lobby;
    // Close the real controls during Countdown; do not spend target-one Running on
    // UI prechecks. Focus the actual canvas and press Space immediately on start.
    await page.locator('#ready-room').click();
    await page.locator('#online-entry summary').click(); await page.locator('canvas').focus();
    await until(() => activeCase.lobbies.some(l => l.phase?.Countdown) && lobby.phase === 'Running' && activeCase.latest, `${plan.id}: Ready -> Countdown -> Running`);
    if (plan.inputChecks) await runInputPreflight();
    await page.keyboard.down('Space');
    await until(() => activeCase.stats.input.neutral_fire > 0, `${plan.id}: actual neutral held-Space socket input`, 5000);
    await screenshot(page, plan.id === 'natural' ? 'actual-playing' : 'preflight-playing', `${plan.id}: actual WASM game; phase labels bracket capture; fast DOM observations, not this image, prove the result`);
    await completeMatch();
  }
  assert.equal(report.protocol.filter(p => p.kind === 'socket').length, 1);
  assert.equal(report.protocol.filter(p => p.kind === 'handshake').length, 1);
  assert.deepEqual(commands, ['Hello', 'Configure', 'Ready', 'Configure', 'Ready'], 'only intentional create/two configure/two ready before Leave');
}
async function runInputPreflight() {
  assert.equal(activeCase.plan.id, 'preflight');
  await until(() => pendingSamples.some(s => s.payload?.frame && s.names.length === 2), 'actual WASM HUD publication and roster', 10000);
  await drainSamples();
  assert(activeCase.stats.first_dom, 'actual active-seat names, bot fallback, scores and IN/OUT DOM match State');
  const diagnostics = page.locator('details.oh-diagnostics');
  await diagnostics.locator('summary').click();
  const controlCount = commands.length;
  await sleep(300); await drainSamples();
  assert(await diagnostics.evaluate(e => e.open), 'diagnostics opens by ordinary click and survives updates');
  assert.equal(commands.length, controlCount, 'diagnostics emits no room commands');
  await expand();
  await neutralWindow('focused form preserves editing and neutralizes Space', async () => {
    await page.locator('#player-name').fill('Hud typing'); await page.keyboard.down('Space');
  });
  await page.keyboard.up('Space');
  assert.equal(await page.locator('#player-name').evaluate(e => document.activeElement === e), true, 'form retains focus');
  await page.locator('#online-entry summary').click(); await page.locator('canvas').focus();
  await neutralWindow('Escape menu suppresses held Space without pausing server', async () => {
    await page.keyboard.press('Escape', {delay: 100}); await page.keyboard.down('Space');
  });
  await page.keyboard.up('Space');
  await page.waitForFunction(() => JSON.parse(globalThis.ryggattackHud ?? '{}').menu === true && !document.querySelector('.rygg-online-hud .oh-menu')?.hidden);
  await drainSamples();
  assert(capturedSamples.some(s => s.case_id === activeCase.plan.id && s.payload?.menu === true && s.menu.includes('server keeps running')), 'actual menu DOM evidence');
  assert(await diagnostics.evaluate(e => e.open), 'diagnostics remains open through menu/focus updates');
  await page.keyboard.press('Escape', {delay: 100});
  await page.waitForFunction(() => JSON.parse(globalThis.ryggattackHud ?? '{}').menu === false);
  await page.locator('canvas').focus();
}
async function completeMatch() {
  const {plan, stats} = activeCase;
  const deadline = stats.completion_started_ms + plan.completionMs;
  // Each cap starts at its own actual authoritative Running event, includes UI
  // work/capture, and is never reset or extended by a screenshot or another case.
  while (!(activeCase.winner && lobby.phase === 'Lobby')) {
    assert(!interrupted, 'interrupted'); assert(elapsed() < deadline, `${plan.id}: natural match completion exceeded 180 seconds`);
    await drainSamples(); await sleep(25);
  }
  stats.completion_duration_ms = elapsed() - stats.completion_started_ms;
  assert(stats.completion_duration_ms <= 180000, 'unchanged natural completion deadline');
  await page.keyboard.up('Space'); await drainSamples();
  const winner = activeCase.winner;
  assert(stats.first_state && stats.fresh_reset_dom, `${plan.id}: fresh authoritative + DOM reset before scoring, no inherited wins/winner/buffs`);
  assert(winner.round_finished && winner.wins[winner.winner] >= plan.target, 'natural authoritative scoring + match winner');
  assert(stats.out.length > 0, 'natural OUT actually rendered, not just a socket elimination');
  const returned = activeCase.lobbies.find(l => l.at_ms > stats.winner.at_ms && l.phase === 'Lobby');
  assert(returned, 'actual authoritative Lobby follows this case winner');
  assert(stats.outcomes.some(s => /wins the (match|round)$/.test(s.outcome)), 'natural named round/match result DOM');
  assert.equal(lobby.id, identity.room); assert.equal(lobby.code, identity.code);
  same(lobby.settings, stats.configured_lobby.settings, 'same actual lobby settings for this case');
  assert(lobby.humans.length === 1 && lobby.humans[0].id === identity.participant && lobby.humans[0].connected && !lobby.humans[0].ready);
  // Acceptance-oracle correction, not a retroactive original-harness pass.
  // Fast read-only samples retain both the real pre-wire proof and Running
  // in-transit tail. Receipt timestamps conservatively bound observation time:
  // no cross-clock subtraction and no assumption of instantaneous client pump.
  // A slow 30s locator wait cannot satisfy the 1000ms wire-to-clear bound.
  const caseSamples = () => capturedSamples.filter(s => s.case_id === plan.id);
  while (true) {
    await drainSamples();
    if (caseSamples().some(s => s.page_ms === activeCase.domTransition.clear_page_ms &&
      s.at_ms >= returned.at_ms && isLobbyClear(s, identity.room))) break;
    assert(!interrupted, 'interrupted');
    assert(elapsed() - returned.at_ms <= 1000, `${plan.id}: actual DOM + serialized HUD clear not captured within 1000ms of wire Lobby`);
    await sleep(25);
  }
  await drainSamples();
  const named = stats.outcomes.filter(s => s.outcome.endsWith('wins the match'));
  const winnerName = winner.names[winner.winner]?.trim() || `Bot ${winner.winner + 1}`;
  const timeline = assertOutcomeTimeline({caseId: plan.id, roomId: identity.room,
    expectedOutcome: `${winnerName} wins the match`, returned, named, samples: caseSamples()});
  stats.outcome_timeline = timeline;
  stats.outcome_persistence_ms = timeline.before_persistence_ms;
  stats.returned_lobby = returned; stats.checks.push('actual Lobby return clears roster/outcome/buff DOM and serialized frame within 1000ms of wire receipt and <=100ms of first observed post-Running raw Lobby (page clock); no post-clear outcome reappearance within case');
  stats.completed = true;
  await screenshot(page, plan.id === 'natural' ? 'actual-returned-lobby' : 'preflight-returned-lobby', `${plan.id}: actual WASM client after natural completion; not winner proof`);
}
async function leaveAndCloseGame() {
  if (!page || page.isClosed()) return;
  if (identity) {
    await page.keyboard.up('Space'); await expand(); await page.locator('#leave-room').click();
    await until(() => leaveSent, 'actual Leave packet', 10000, true);
    await page.waitForFunction(() => {
      const room = JSON.parse(globalThis.ryggattackRoom ?? '{}'), p = JSON.parse(globalThis.ryggattackHud ?? '{}');
      const root = document.querySelector('.rygg-online-hud');
      return room.status?.includes('Left room') && !room.lobby && !p.lobby && p.seat === null && p.frame === null && root?.querySelectorAll('.oh-player').length === 0 && root.querySelector('.oh-outcome').hidden;
    }, null, {timeout: 10000});
    report.cleanup.leave_packet = true; report.cleanup.left_room_and_hud_clear_readback = true;
    await drainSamples();
    report.leave_readback = await page.evaluate(() => ({room: JSON.parse(globalThis.ryggattackRoom), hud: JSON.parse(globalThis.ryggattackHud)}));
  }
  await page.close(); report.cleanup.game_page_closed_before_fixture = true;
}
async function runPresentationFixture() {
  // No WASM bootstrap, room adapter, protocol, or live publication overrides.
  // Use the exact served index's real DOM/CSS with script elements removed in an
  // isolated document, then import ONLY the exact served presentation module.
  fixture = await browser.newPage({viewport: {width: 1024, height: 768}});
  fixture.on('websocket', () => fault('fixture', 'presentation fixture must not open sockets'));
  fixture.on('pageerror', e => fault('fixture', e.message));
  const html = fs.readFileSync(path.join(web, 'index.html'), 'utf8').replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '');
  await fixture.route(`${origin}/__presentation_fixture__`, route => route.fulfill({contentType: 'text/html', body: html}));
  await fixture.goto(`${origin}/__presentation_fixture__`);
  await fixture.evaluate(async base => {
    const {mountOnlineHud} = await import(base + '/online-hud.mjs');
    document.getElementById('status').remove();
    const label = document.createElement('p'); label.textContent = 'ISOLATED SYNTHETIC PRESENTATION FIXTURE — NOT GAMEPLAY';
    label.style.cssText = 'position:absolute;bottom:0;left:0;color:white;max-width:45%;font-size:11px'; document.body.append(label);
    globalThis.__fixtureHud = mountOnlineHud(document.getElementById('game'));
    document.getElementById('online-entry').open = false;
    document.getElementById('room-controls').hidden = false;
    document.querySelector('#online-entry form').addEventListener('submit', e => e.preventDefault());
    for (const id of ['ready-room', 'apply-room', 'leave-room']) document.getElementById(id).addEventListener('click', () => { document.getElementById(id).dataset.clicked = 'true'; });
  }, origin);
  const escaped = '<img src=x onerror="globalThis.fixtureEscaped=true"> & a very long driver name';
  const failure = 'Connection lost: ' + 'retryable-failure-with-long-diagnostic-text/'.repeat(15);
  const fixturePayload = {lobby: {id: 'presentation-only', phase: 'Running'}, seat: 0, status: failure, notice: failure, menu: true, rtt_ms: 23,
    frame: {tick: 20, simulation_tick: 19, names: [escaped, null, 'Other', 'Fourth'], wins: [2, 1, 0, 0], winner: 0, round_finished: true, transition: 200,
      carts: [0, 1, 2, 3].map(id => ({id, active: true, alive: id === 0, speed_seconds: id === 0 ? 2.34 : 0, all_hits_seconds: id === 0 ? 4.56 : 0}))}};
  for (const width of [1024, 375]) {
    await fixture.setViewportSize({width, height: 900});
    await fixture.evaluate(p => globalThis.__fixtureHud.update(p), fixturePayload);
    const observed = await fixture.evaluate(() => {
      const root = document.querySelector('.rygg-online-hud');
      const bounds = [...root.querySelectorAll('.oh-player,.oh-notice,.oh-connection,.oh-outcome')].map(e => {const r = e.getBoundingClientRect(); return {left: r.left, right: r.right, top: r.top, bottom: r.bottom, scroll: e.scrollWidth, width: e.clientWidth};});
      return {names: [...root.querySelectorAll('.oh-name')].map(e => e.textContent), states: [...root.querySelectorAll('.oh-state')].map(e => e.textContent), outcome: root.querySelector('.oh-outcome').textContent,
        notice: root.querySelector('.oh-notice').textContent, connection: root.querySelector('.oh-connection').textContent, bounds, injected: !!root.querySelector('img') || !!globalThis.fixtureEscaped,
        columns: getComputedStyle(root.querySelector('.oh-players')).gridTemplateColumns.split(' ').length,
        hud_z: Number(getComputedStyle(root).zIndex), room_z: Number(getComputedStyle(document.getElementById('online-entry')).zIndex),
        globals_untouched: !('ryggattackHud' in globalThis) && !('ryggattackCommand' in globalThis)};
    });
    assert.deepEqual(observed.names, [escaped + ' (you)', 'Bot 2', 'Other', 'Fourth']);
    assert(observed.states[0].includes('2 wins · IN · Speed 2.4s · All-hits 4.6s'), 'coexisting independent authoritative timer values');
    assert(observed.states.slice(1).every(s => s.includes('OUT')), 'OUT presentation');
    assert.equal(observed.outcome, `${escaped} wins the match`);
    assert.equal(observed.notice, failure); assert.equal(observed.connection, failure);
    assert(!observed.injected && observed.globals_untouched, 'escaped names, no live globals or protocol');
    assert.equal(observed.columns, width === 375 ? 2 : 4);
    assert(Number.isFinite(observed.hud_z) && Number.isFinite(observed.room_z) && observed.hud_z < observed.room_z, 'actual computed HUD paint order remains below real room controls');
    assert(observed.bounds.every(r => r.left >= 0 && r.right <= width && r.top >= 0 && r.bottom <= 900 && r.scroll <= r.width + 1), 'all HUD text/boxes fit viewport; long errors visible, not clipped');
    await fixture.locator('details.oh-diagnostics summary').click();
    await fixture.evaluate(p => globalThis.__fixtureHud.update({...p, frame: {...p.frame, tick: 21}}), fixturePayload);
    assert(await fixture.locator('details.oh-diagnostics').evaluate(e => e.open), 'diagnostics open state survives updates');
    await fixture.locator('details.oh-diagnostics summary').click();
    await fixture.locator('#online-entry summary').click();
    await fixture.locator('#player-name').fill('Fixture focus');
    await fixture.keyboard.type(' ');
    assert(await fixture.locator('#player-name').evaluate(e => document.activeElement === e && e.value === 'Fixture focus '), 'real form editing retains focus through HUD layering');
    for (const id of ['apply-room', 'ready-room', 'leave-room']) {
      // Reset the fixture-only sentinel so each viewport proves a fresh click.
      await fixture.locator('#' + id).evaluate(e => { delete e.dataset.clicked; });
      await fixture.locator('#' + id).click();
      assert.equal(await fixture.locator('#' + id).getAttribute('data-clicked'), 'true', 'actual room controls remain clickable through HUD layering');
      assert(await fixture.locator('#' + id).evaluate(e => document.activeElement === e), 'ordinary control click receives focus');
    }
    await fixture.locator('#online-entry summary').click();
    report.fixture.checks.push({width, names_escaped: true, timers_coexist: true, out: true, long_failure_fully_visible: true, diagnostics_stable: true, computed_z: {hud: observed.hud_z, room: observed.room_z}, real_index_room_controls_clickable: true, form_focus: true});
    await screenshot(fixture, `fixture-${width}`, report.fixture.label);
  }
  const draw = structuredClone(fixturePayload); draw.frame.tick = 22; draw.frame.winner = null; draw.frame.carts.forEach(c => {c.alive = false;});
  await fixture.evaluate(p => globalThis.__fixtureHud.update(p), draw);
  assert.equal(await fixture.locator('.oh-outcome').textContent(), 'Round draw');
  const round = structuredClone(draw); round.frame.tick = 23; round.frame.carts[1].alive = true;
  await fixture.evaluate(p => globalThis.__fixtureHud.update(p), round);
  assert.equal(await fixture.locator('.oh-outcome').textContent(), 'Bot 2 wins the round');
  report.fixture.checks.push('synthetic draw and named bot round result');
  await fixture.evaluate(() => globalThis.__fixtureHud.destroy()); await fixture.close();
}
async function shutdown() {
  if (fixture && !fixture.isClosed()) try { await fixture.close(); } catch { fault('cleanup', 'fixture close failed'); }
  if (page && !page.isClosed()) {
    try { await leaveAndCloseGame(); } catch (error) { fault('cleanup', error.message); try { await page.close(); } catch {} }
  }
  if (browser) {
    try {
      const disconnected = await Promise.race([browser.close().then(() => true), sleep(5000).then(() => false)]);
      if (!disconnected || browser.isConnected()) fault('cleanup', 'browser connection did not close gracefully');
    } catch { fault('cleanup', 'browser close failed'); }
  }
  if (browserServer) {
    const child = browserServer.process();
    const exited = child.exitCode !== null || child.signalCode !== null
      ? Promise.resolve(true) : new Promise(resolve => child.once('exit', () => resolve(true)));
    try {
      const graceful = await Promise.race([browserServer.close().then(() => true), sleep(5000).then(() => false)]);
      if (!graceful) {
        child.kill('SIGKILL'); report.cleanup.browser_forced_kill = true;
        fault('cleanup', 'owned browser required forced shutdown');
      }
    } catch {
      child.kill('SIGKILL'); fault('cleanup', 'owned browser graceful shutdown failed');
    }
    report.cleanup.browser_closed = await Promise.race([exited, sleep(2000).then(() => false)]);
    if (!report.cleanup.browser_closed) fault('cleanup', 'owned browser process did not exit');
  }
  if (backend) {
    if (backend.exitCode === null && backend.signalCode === null) backend.kill('SIGTERM');
    let result = await Promise.race([backendExit, sleep(5000).then(() => null)]);
    if (!result) {
      backend.kill('SIGKILL'); report.cleanup.backend_forced_kill = true;
      result = await Promise.race([backendExit, sleep(2000).then(() => null)]);
      fault('cleanup', 'backend required forced shutdown');
    }
    if (!result) fault('cleanup', 'owned backend failed bounded shutdown');
    report.cleanup.backend_exit = result;
  }
  if (server?.listening) {
    let didClose = false;
    const closed = new Promise(resolve => server.close(() => { didClose = true; resolve(); }));
    server.closeIdleConnections?.();
    if (!await Promise.race([closed.then(() => true), sleep(3000).then(() => false)])) {
      server.closeAllConnections?.(); await Promise.race([closed, sleep(1000)]);
    }
    report.cleanup.http_closed = didClose && !server.listening;
    if (!report.cleanup.http_closed) fault('cleanup', 'owned HTTP server failed bounded shutdown');
  }
}
try {
  assert.notEqual(process.env.NODE_TLS_REJECT_UNAUTHORIZED, '0', 'no TLS verification bypass');
  report.hashes = hashInputs();
  await startHttp(); await startBackend(); await runNatural();
  await leaveAndCloseGame(); await runPresentationFixture();
  assert.equal(report.errors.length, 0, 'unexpected browser/protocol/cleanup errors');
  same(hashInputs(), report.hashes, 'all consumed source/artifact/helper bytes unchanged during verification');
  report.status = 'passed';
} catch (error) {
  report.status = 'failed'; report.failure = String(error.stack ?? error);
  // No console/DOM dumps; raw errors are scrubbed only at final publication.
  if (page && !page.isClosed()) try { await screenshot(page, 'actual-failure', 'failure-time actual app; not proof of earlier phases'); } catch (e) { fault('screenshot', e.message); }
} finally {
  try { await shutdown(); } catch (error) { fault('shutdown', error.message); }
  if (report.errors.length) report.status = 'failed';
  report.finished_at = new Date().toISOString(); report.total_ms = elapsed();
  for (const plan of MATCH_CASES) {
    report[plan.id].buff_scope = report[plan.id].buffs.length ? 'Case-local natural DOM timer observations; separate from synthetic fixture assertions.' : 'No natural buff observed in this case; synthetic timer fixture is NOT natural buff proof.';
  }
  // Discover private values first, redact then publish; helper scans EVERY final
  // artifact (including manifest/JSON/image bytes) and purges on privacy failure.
  const safe = redact(report, secrets);
  try {
    fs.writeFileSync(path.join(out, 'dom-observations.json'), JSON.stringify(redact([...capturedSamples, ...pendingSamples], secrets), null, 2) + '\n');
    fs.writeFileSync(path.join(out, 'input-observations.json'), JSON.stringify(redact(inputs, secrets), null, 2) + '\n');
    fs.writeFileSync(path.join(out, 'authoritative-state-projections.json'), JSON.stringify(redact([...cases.values()].flatMap(match => [...match.states.values()].map(frame => ({case_id: match.plan.id, frame: projection(frame)}))), secrets), null, 2) + '\n');
    if (!publishSoloEvidence(out, safe, secrets, {privacyFailed})) process.exitCode = 1;
  } catch { process.exitCode = 1; }
  if (safe.status !== 'passed') process.exitCode = 1;
  console.log(JSON.stringify({status: process.exitCode ? 'failed' : safe.status, evidence_directory: out, failure: safe.failure ? redact(String(safe.failure).split('\n')[0], secrets) : undefined}));
  process.removeListener('SIGINT', interrupt); process.removeListener('SIGTERM', interrupt);
}
