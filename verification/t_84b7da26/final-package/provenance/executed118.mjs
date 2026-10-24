// Actual LOCAL artifact only. Never builds, deploys, injects state, or visits public rooms.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import {spawn, execFileSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {canonical, redact} from 'file:///home/hermes/src/ryggattack-online-explosions/scripts/live-deployment-helpers.mjs';
import {checkCopiedEntryArtifacts} from 'file:///home/hermes/src/ryggattack-online-explosions/scripts/online-hud-artifacts.mjs';
import {capturePrivateScreenshot, PrivacyViolation, publishSoloEvidence} from 'file:///home/hermes/src/ryggattack-online-explosions/scripts/solo-bots-helpers.mjs';
import {assertSharedExplosionEvidence} from 'file:///home/hermes/src/ryggattack-online-explosions/scripts/online-explosion-evidence.mjs';
import {watchOwnedProcess, stopOwnedProcess, stopOwnedBrowser, leaveAdmittedClient, finalizeCleanupReport, watchCleanupInterruption} from 'file:///home/hermes/src/ryggattack-online-explosions/scripts/online-explosion-cleanup.mjs';

const usage = `Usage: RYGGATTACK_EXPLOSION_ARTIFACT_READY=1 node scripts/online-explosion-smoke.mjs --local
Parent must finish/freeze native + full WASM dist/web builds first. No build here.
Two independent Chromium processes, one actual WASM human each; owned loopback server.
PLAYWRIGHT_NODE_MODULES: installed Playwright package.json; RYGGATTACK_NATIVE_BIN_DIR: native bins.
Writes a unique verification/online-explosion/<attempt>/; mechanical success still needs image review.
See docs/online-explosion.md for artifact conditions, bounds oracle and review checklist.`;
if (process.argv.slice(2).length === 1 && ['--help', '-h'].includes(process.argv[2])) { console.log(usage); process.exit(0); }
if (process.argv.slice(2).join(' ') !== '--local') { console.error(usage); process.exit(2); }
const repo = "/home/hermes/src/ryggattack-online-explosions";
const web = path.join(repo, 'dist/web');
const bin = path.resolve(process.env.RYGGATTACK_NATIVE_BIN_DIR ?? path.join(repo, 'target/debug'));
const out = path.join(repo, 'verification/rear-browser', new Date().toISOString().replace(/[:.]/g, '-') + '-' + crypto.randomBytes(6).toString('hex'));
fs.mkdirSync(out, {recursive: true});
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const started = performance.now(), now = () => Math.round(performance.now() - started);
const secrets = new Set(), clients = [], browsers = [];
const report = {status: 'running', scope: 'LOCAL two actual rendered WASM humans, protocol 3; legal HTML controls + keyboard only',
  budgets: {gameplay_wall_ms: 180000, screenshot_only_ms: 60000, max_matches: 3, captures_per_client: 3},
  started_at: new Date().toISOString(), checks: [], errors: [], screenshots: [], cleanup: {},
  visual_review: {status: 'required', claim: 'Wire transforms are NOT measured GPU mesh bounds or image pixel dimensions.'}};
let server, backend, backendLifecycle, origin, address, privacyFailed = false, gameplayStarted;
const interruption = watchCleanupInterruption(process);
const safeText = value => String(value).replace(/\b(?:wss?|https?):\/\/[^\s"'<>]+/g, '[endpoint omitted]');
const fault = (source, error) => report.errors.push({source, message: safeText(error?.message ?? error), at_ms: now()});
function gameBudget() {
  assert(!interruption.interrupted, 'interrupted');
  assert(gameplayStarted === undefined || now() - gameplayStarted < report.budgets.gameplay_wall_ms, '180s visual gameplay budget exhausted (captures do not extend it)');
}
async function until(predicate, label, ms = 90000, gameplay = false, cleanup = false) {
  const end = now() + ms;
  while (!predicate()) {
    assert(cleanup || !interruption.interrupted, 'interrupted');
    assert(cleanup || report.errors.length === 0, 'observed browser/protocol error; see errors');
    if (gameplay) gameBudget();
    assert(now() < end, label); await sleep(25);
  }
  if (gameplay) gameBudget();
}
function filesBelow(dir) {
  return fs.readdirSync(dir, {withFileTypes: true}).sort((a, b) => a.name.localeCompare(b.name)).flatMap(entry => {
    assert(!entry.isSymbolicLink(), 'artifact/source symlinks forbidden');
    const file = path.join(dir, entry.name);
    return entry.isDirectory() ? filesBelow(file) : entry.isFile() ? [file] : [];
  });
}
function hashInputs() {
  const rust = filesBelow(path.join(repo, 'src')).filter(f => f.endsWith('.rs'));
  const source = [...rust, ...filesBelow(path.join(repo, 'web')), ...filesBelow(path.join(repo, 'scripts')).filter(f => /\.(?:mjs|js|sh)$/.test(f)),
    path.join(repo, 'Cargo.toml'), path.join(repo, 'Cargo.lock'), path.join(repo, 'docs/online-explosion.md')];
  const artifacts = filesBelow(web), native = path.join(bin, 'ryggattack-server');
  for (const file of ['index.html', 'rooms.mjs', 'ryggattack.js', 'ryggattack_bg.wasm']) assert(artifacts.includes(path.join(web, file)), `missing parent-built ${file}`);
  checkCopiedEntryArtifacts(path.join(repo, 'web'), web);
  assert(fs.readFileSync(path.join(web, 'ryggattack_bg.wasm')).subarray(0, 4).equals(Buffer.from([0, 97, 115, 109])), 'actual WASM magic');
  // Necessary stale-build guard, not a cryptographic proof of compiler provenance.
  const newest = Math.max(...[...rust, path.join(repo, 'Cargo.toml'), path.join(repo, 'Cargo.lock')].map(f => fs.statSync(f).mtimeMs));
  for (const file of [native, path.join(web, 'ryggattack_bg.wasm')]) assert(fs.statSync(file).mtimeMs >= newest, 'native/WASM older than Rust source; rebuild after freeze');
  assert(fs.lstatSync(native).isFile() && !fs.lstatSync(native).isSymbolicLink(), 'regular native artifact required');
  return Object.fromEntries([...new Set([...source, ...artifacts, native])].sort().map(file => {
    const bytes = fs.readFileSync(file); return [path.relative(repo, file), {sha256: sha(bytes), bytes: bytes.length}];
  }));
}
const pick = (object, keys) => Object.fromEntries(keys.map(key => [key, object[key]]));
function publicFrame(frame) {
  return {...pick(frame, ['tick', 'simulation_tick', 'ack', 'names', 'wins', 'winner', 'round_finished', 'transition', 'until_drop']),
    carts: frame.carts.map(c => pick(c, ['id', 'active', 'alive', 'position', 'forward', 'shot_cooldown', 'speed_seconds', 'all_hits_seconds'])),
    bodies: frame.bodies.map(b => pick(b, ['id', 'kind', 'position', 'rotation', 'scale']))};
}
function publicLobby(lobby) {
  return {...pick(lobby, ['id', 'host', 'phase']), settings: pick(lobby.settings, ['arena', 'wins_target', 'bots']),
    humans: lobby.humans.map(h => pick(h, ['id', 'name', 'seat', 'ready', 'connected']))};
}
async function startLocal() {
  server = http.createServer((req, res) => {
    let file;
    try {
      let pathname = decodeURIComponent(req.url.split('?')[0]);
      if (pathname.includes('\0')) throw Error('NUL');
      if (pathname.endsWith('/')) pathname += 'index.html';
      file = path.resolve(web, '.' + pathname);
    } catch { res.writeHead(400).end(); return; }
    if (!file.startsWith(web + path.sep)) { res.writeHead(403).end(); return; }
    fs.realpath(file, (error, real) => {
      if (error) { res.writeHead(404).end(); return; }
      if (!real.startsWith(fs.realpathSync(web) + path.sep)) { res.writeHead(403).end(); return; }
      fs.readFile(real, (error, bytes) => {
        if (error) { res.writeHead(404).end(); return; }
        res.writeHead(200, {'content-type': ({'.html': 'text/html', '.mjs': 'text/javascript', '.js': 'text/javascript', '.wasm': 'application/wasm'})[path.extname(real)] ?? 'application/octet-stream',
          'content-security-policy': `connect-src 'self' ${address ?? "'none'"}; object-src 'none'; base-uri 'self'`}).end(bytes);
      });
    });
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  origin = `http://127.0.0.1:${server.address().port}`;
  for (const [rawPath, expected] of [['/%2e%2e/Cargo.toml', 403], ['/%ZZ', 400], ['/%00', 400]]) {
    const status = await new Promise((resolve, reject) => {
      const req = http.get({host: '127.0.0.1', port: server.address().port, path: rawPath}, res => { res.resume(); resolve(res.statusCode); });
      req.setTimeout(5000, () => req.destroy(Error('sandbox check timeout'))); req.on('error', reject);
    });
    assert.equal(status, expected); report.checks.push({raw_path: rawPath, status});
  }
  for (const file of filesBelow(web)) {
    const response = await fetch(`${origin}/${path.relative(web, file).split(path.sep).join('/')}`, {redirect: 'error', signal: AbortSignal.timeout(30000)});
    assert.equal(response.status, 200);
    assert.equal(sha(Buffer.from(await response.arrayBuffer())), report.hashes[path.relative(repo, file)].sha256, 'served artifact hash');
    if (/\.(?:mjs|js)$/.test(file)) assert.match(response.headers.get('content-type'), /^(text|application)\/javascript\b/);
    if (file.endsWith('.wasm')) assert.match(response.headers.get('content-type'), /^application\/wasm\b/);
  }
  backend = spawn(path.join(bin, 'ryggattack-server'), ['127.0.0.1:0'], {stdio: ['ignore', 'pipe', 'pipe'], env: {...process.env, RYGGATTACK_ALLOWED_ORIGINS: origin}});
  backendLifecycle = watchOwnedProcess(backend);
  report.server_events = [];
  backend.stderr.on('data', bytes => report.server_events.push({at_ms: now(), text: safeText(String(bytes))}));
  address = await new Promise((resolve, reject) => {
    let text = ''; const timer = setTimeout(() => reject(Error('backend startup timeout')), 10000);
    const done = (error, value) => { clearTimeout(timer); error ? reject(error) : resolve(value); };
    backend.once('error', error => done(error)); backend.once('exit', () => done(Error('backend exited')));
    backend.stdout.on('data', bytes => { text += String(bytes); const match = /ws:\/\/127\.0\.0\.1:(\d+)/.exec(text); if (match) done(null, `ws://127.0.0.1:${match[1]}`); });
  });
}
function observe(c, ws) {
  if (new URL(ws.url()).href !== new URL(address).href) fault('scope', 'unexpected socket endpoint');
  c.sockets++; ws.on('socketerror', error => fault(c.name, error));
  ws.on('close', () => { c.socketClosed = true; });
  ws.on('framereceived', ({payload}) => {
    try {
      const message = JSON.parse(String(payload)); redact(message, secrets); // Discover, NEVER save raw Welcome.
      if (message.Welcome) {
        assert(!c.identity, 'one admission per client'); const w = message.Welcome;
        assert.equal(w.version, 3); c.code = w.code;
        c.identity = pick(w, ['version', 'room', 'participant', 'seat']);
      }
      if (message.Lobby) {
        const lobby = publicLobby(message.Lobby);
        if (lobby.phase === 'Running' && c.lobby?.phase !== 'Running') {
          c.match++; assert(c.match <= report.budgets.max_matches, 'max three natural match starts');
          gameplayStarted ??= now();
        }
        c.lobby = lobby; c.lobbies.push({at_ms: now(), match: c.match, ...lobby});
      }
      if (message.State) {
        assert(c.lobby?.phase === 'Running', 'State only during Running');
        const frame = publicFrame(message.State), prev = c.frames.at(-1);
        assert(!prev || frame.tick > prev.frame.tick, 'advancing wire State');
        if (prev?.match === c.match) assert(frame.simulation_tick > prev.frame.simulation_tick, 'advancing simulation State within this match');
        assert.equal(frame.carts.filter(cart => cart.active).length, 2, 'two active humans, zero bots');
        for (const human of c.lobby.humans) assert.equal(frame.names[human.seat], human.name, 'wire cart name/seat mapping');
        c.frames.push({at_ms: now(), match: c.match, frame}); c.latest = frame;
      }
      if (message.Error || message.Notice) fault(c.name, 'unexpected server rejection');
    } catch (error) { fault(`${c.name}:received`, error); }
  });
  ws.on('framesent', ({payload}) => {
    try {
      const message = JSON.parse(String(payload)); redact(message, secrets);
      const kind = typeof message === 'string' ? message : Object.keys(message)[0];
      assert(['Hello', 'Configure', 'Ready', 'Input', 'Leave'].includes(kind), 'legal UI protocol commands only');
      if (kind === 'Hello') { assert.equal(message.Hello.version, 3); assert.equal(message.Hello.name, c.name); }
      if (kind === 'Configure') assert.deepEqual(message.Configure, {arena: 'Forest', wins_target: 7, bots: 0});
      if (kind === 'Ready') assert.equal(message.Ready, true);
      if (kind === 'Input') {
        const input = pick(message.Input, ['sequence', 'horizontal', 'vertical', 'fire']);
        assert([-1,0,1].includes(input.horizontal) && [-1,0,1].includes(input.vertical) && !input.fire, 'legal directional keys and no fire');
        c.inputs.push({at_ms: now(), match: c.match, phase: c.lobby?.phase ?? null, ...input});
      } else c.commands.push({kind, at_ms: now(), match: c.match});
      if (kind === 'Leave') c.leaveSent = true;
    } catch (error) { fault(`${c.name}:sent`, error); }
  });
}
async function expand(c, open = true) {
  if (await c.page.locator('#online-entry').evaluate(e => e.open) !== open) await c.page.locator('#online-entry summary').click();
}
async function openClient(chromium, name, code) {
  const c = {name, sockets: 0, match: 0, lobbies: [], frames: [], inputs: [], commands: [], cleanup: {}}; clients.push(c);
  const owned = await chromium.launchServer({host: '127.0.0.1', args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--disable-backgrounding-occluded-windows', '--disable-renderer-backgrounding']});
  browsers.push({owned, client: c, lifecycle: watchOwnedProcess(owned.process())});
  const browser = await chromium.connect(owned.wsEndpoint(), {timeout: 30000}); browsers.at(-1).browser = browser;
  c.browser_version = browser.version(); c.pid = owned.process().pid;
  const context = await browser.newContext({viewport: {width: 640, height: 480}});
  await context.route('**/*', route => {
    const url = new URL(route.request().url());
    if (url.origin === origin || ['data:', 'blob:'].includes(url.protocol)) return route.continue();
    fault('scope', 'blocked nonlocal request'); return route.abort();
  });
  const page = await context.newPage(); c.page = page;
  page.setDefaultTimeout(30000); page.setDefaultNavigationTimeout(45000);
  page.on('websocket', ws => observe(c, ws));
  page.on('pageerror', error => fault(`${name}:page`, error)); page.on('crash', () => fault(name, 'renderer crashed'));
  page.on('console', message => { if (message.type() === 'error') fault(`${name}:console`, message.text()); });
  page.on('requestfailed', () => fault(`${name}:request`, 'resource request failed'));
  const cdp = await context.newCDPSession(page); await cdp.send('Network.enable');
  cdp.on('Network.webSocketWillSendHandshakeRequest', event => {
    const observed = Object.entries(event.request.headers).find(([key]) => key.toLowerCase() === 'origin')?.[1];
    c.origin_verified = observed === origin; if (!c.origin_verified) fault(name, 'socket Origin mismatch');
  });
  await page.goto(`${origin}/?quality=0`, {waitUntil: 'domcontentloaded'});
  await expand(c); await page.locator('#backend').fill(address); await page.locator('#player-name').fill(name);
  if (code) await page.locator('#room-code').fill(code);
  await page.locator(code ? '#join-room' : '#create-room').click();
  await until(() => c.identity && c.lobby, `${name}: admission`);
  return c;
}
function sharedProof() {
  const [a, b] = clients;
  const matches = [];
  for (let match = 1; match <= Math.max(a.match, b.match); match++) {
    const proof = assertSharedExplosionEvidence(a.frames.filter(f => f.match === match).map(f => f.frame), b.frames.filter(f => f.match === match).map(f => f.frame));
    matches.push({match, ...proof});
  }
  return matches;
}
async function ensureRunning() {
  gameBudget();
  if (!clients.every(c => c.lobby?.phase === 'Running' && c.latest?.winner === null)) {
    await until(() => clients.every(c => c.lobby?.phase === 'Lobby'), 'natural match return before Ready', 90000, true);
    for (const c of clients) {
      await c.page.keyboard.up('Space'); await expand(c);
      await c.page.locator('#ready-room').click();
    }
    await until(() => clients.every(c => c.lobby?.phase === 'Running' && c.latest?.winner === null && c.frames.at(-1)?.match === c.match), 'both actual Running states', 90000, true);
  }
  for (const c of clients) { await expand(c, false); await c.page.locator('canvas').focus(); await c.page.keyboard.down('Space'); }
  const starts = clients.map(c => ({inputs: c.inputs.length, tick: c.latest.tick}));
  await until(() => clients.every((c, i) => c.latest.tick > starts[i].tick && c.inputs.slice(starts[i].inputs).filter(input => input.phase === 'Running' && input.fire).length >= 3), 'both held Space sampled over whole advancing frames', 20000, true);
}
async function presentation(c) {
  // Explicit public whitelist; never dump room storage, raw globals or URL fields.
  return c.page.evaluate(() => {
    const hud = JSON.parse(globalThis.ryggattackHud ?? 'null');
    return {phase: hud?.lobby?.phase ?? null, seat: hud?.seat ?? null, tick: hud?.frame?.tick ?? null,
      names: [...document.querySelectorAll('.rygg-online-hud .oh-name')].map(e => e.textContent),
      canvas: [...document.querySelectorAll('canvas')].map(e => ({width: e.width, height: e.height, visible: e.getClientRects().length > 0}))};
  });
}
async function capture(c, index) {
  await ensureRunning(); // Reestablish real Running before EACH sequential capture.
  const begin = c.frames.length;
  await until(() => c.frames.length > begin && c.latest.bodies.some(b => b.kind === 4 && b.scale[0] >= .5), 'fresh visible-size explosion wire opportunity', 30000, true);
  const before = {at_ms: now(), match: c.match, tick: c.latest.tick, body_ids: c.latest.bodies.filter(b => b.kind === 4).map(b => b.id), presentation: await presentation(c)};
  assert.equal(before.presentation.phase, 'Running', 'capture begins in actual published Running');
  assert.equal(before.presentation.seat, c.identity.seat);
  for (const other of clients) assert(before.presentation.names.includes(other.name + (other === c ? ' (you)' : '')), 'distinct real names rendered on each client');
  assert(before.presentation.canvas.some(canvas => canvas.visible && canvas.width > 0 && canvas.height > 0), 'actual rendered canvas dimensions');
  const file = `${c.name}-${index}.png`;
  try { await capturePrivateScreenshot(c.page, secrets, () => c.page.screenshot({path: path.join(out, file), timeout: 60000})); }
  catch (error) { if (error instanceof PrivacyViolation) privacyFailed = true; throw error; }
  const after = {at_ms: now(), match: c.match, tick: c.latest.tick, presentation: await presentation(c)};
  report.screenshots.push({file, sha256: sha(fs.readFileSync(path.join(out, file))), before, after,
    caveat: 'Capture interval, not exact render-to-wire synchronization. Review actual pixels for visible blasts, cart/rail reference and sequence changes.'});
  gameBudget();
}
async function run() {
  report.scope='LOCAL two actual rendered WASM humans; legal UI and directional keyboard only; no fire or state injection';
  report.replay={launcher_sha256:"c29e391b19d24afcfdc15a1158b607740cf238e4c69b6944c9420bc1c5680442",base_harness_sha256:"d46cda804fa29d77ce4c82a95345798d468dcfe40f702d640b7aa097256211e5",policy:'A neutral; B cycles cardinal intent every 6 seconds. Contact sought in full shared wire; no event injected.'};
  const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES)('playwright');
  const a=await openClient(chromium,'RearA'),b=await openClient(chromium,'RearB',a.code);
  assert.notEqual(a.pid,b.pid);assert.equal(a.identity.room,b.identity.room);
  await until(()=>clients.every(c=>c.lobby.humans.length===2),'two humans');
  await expand(a);await a.page.locator('#arena-room').selectOption('Forest');
  await a.page.locator('#target-room').selectOption('7');await a.page.locator('#bots-room').fill('0');await a.page.locator('#apply-room').click();
  await until(()=>clients.every(c=>c.lobby.settings.bots===0&&c.lobby.settings.wins_target===7),'configured');
  for(const c of clients){await c.page.locator('#ready-room').click();await expand(c,false);await c.page.locator('canvas').focus();}
  await until(()=>clients.every(c=>c.latest&&c.lobby.phase==='Running'),'running');
  const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0),sub=(a,b)=>a.map((x,i)=>x-b[i]);
  function contacts(){
    const frames=a.frames,other=new Map(b.frames.map(x=>[x.frame.tick,x.frame])),events=[];
    for(let i=1;i<frames.length;i++){
      const p=frames[i-1].frame,n=frames[i].frame,dt=n.simulation_tick-p.simulation_tick;
      if(dt!==4||n.tick-p.tick!==4||p.round_finished||n.round_finished||!other.has(p.tick)||!other.has(n.tick))continue;
      assert.deepEqual(p,other.get(p.tick));assert.deepEqual(n,other.get(n.tick));
      const carts=p.carts.filter(c=>c.active&&c.alive);if(carts.length!==2)continue;
      const d=carts[0].forward;if(dot(d,carts[1].forward)<.9999)continue;
      const gap=dot(sub(carts[1].position,carts[0].position),d),front=gap>0?1:0,rear=1-front;
      const offset=sub(carts[front].position,carts[rear].position);
      if(Math.hypot(...offset.map((x,i)=>x-Math.abs(gap)*d[i]))>.001||Math.abs(gap)>1.5)continue;
      const ms=[];
      for(const x of carts){const y=n.carts.find(c=>c.id===x.id);if(!y?.alive||dot(x.forward,y.forward)<.9999||((x.speed_seconds>0)!==(y.speed_seconds>0)))break;
        const v=sub(y.position,x.position),travel=dot(v,d),ordinary=4.2*(x.speed_seconds>0?1.25:1)*dt/64;
        ms.push({seat:x.id,travel,ordinary,extra:travel-ordinary,off_axis:Math.hypot(...v.map((x,i)=>x-travel*d[i])),forward_dot:dot(x.forward,y.forward)});
      }
      if(ms.length===2&&ms.every(m=>m.off_axis<.001)&&Math.abs(ms[front].extra-.4)<.002&&Math.abs(ms[rear].extra)<.002)events.push({from_tick:p.tick,to_tick:n.tick,initial_gap:Math.abs(gap),front_seat:carts[front].id,rear_seat:carts[rear].id,measurements:ms});
    }return events;
  }
  let held=null,index=0,next=now();const directions=['ArrowUp','ArrowRight','ArrowDown','ArrowLeft'];
  while(now()-gameplayStarted<170000){
    assert.equal(report.errors.length,0,'protocol/browser errors');
    const events=contacts();if(events.length){report.rear_contacts=events;break;}
    if(now()>=next){if(held)await b.page.keyboard.up(held);held=directions[index++%4];await b.page.keyboard.down(held);next=now()+6000;}
    await sleep(100);
  }
  if(held)await b.page.keyboard.up(held);
  report.rear_contacts=contacts();assert(report.rear_contacts.length>0,'no qualifying rear contact within bounded legal-input replay');
  report.checks.push({rear_contacts:report.rear_contacts.length,scope:'Both actual browser streams equal at before/after ticks; straight live aligned pair, ordinary travel plus0.4 only leader, unchanged heading for both. CPU paired-entry fixture remains causal oracle; no pixel displacement claim.'});
  for(const c of clients){await capturePrivateScreenshot(c.page,secrets,()=>c.page.screenshot({path:path.join(out,c.name+'.png'),timeout:60000}));report.screenshots.push({file:c.name+'.png',sha256:sha(fs.readFileSync(path.join(out,c.name+'.png'))),claim:'Actual post-event gameplay; not exact contact-tick pixels.'});}
}

async function leave(c) {
  return leaveAdmittedClient(c, {clients, expand, until, presentation});
}
async function shutdown() {
  for (const c of clients) {
    try { await leave(c); } catch (error) { fault(`${c.name}:leave`, error); }
    if (c.page && !c.page.isClosed()) try {
      c.cleanup.page_closed = await Promise.race([c.page.close().then(() => c.page.isClosed()), sleep(5000).then(() => false)]);
      if (!c.cleanup.page_closed) fault(`${c.name}:page-close`, 'page close timed out');
    } catch (error) { fault(`${c.name}:page-close`, error); }
  }
  for (const entry of browsers) {
    const result = await stopOwnedBrowser(entry);
    entry.client.cleanup.browser_exit = result;
    entry.client.cleanup.browser_process_exited = result.valid;
    if (!result.valid) fault('cleanup', 'owned Chromium did not complete a clean requested shutdown');
  }
  if (backend) {
    const result = await stopOwnedProcess(backendLifecycle, async () => {
      backend.kill('SIGTERM'); await backendLifecycle.exit;
    });
    report.cleanup.backend_exit = result;
    if (!result.valid) fault('cleanup', 'owned backend did not complete a clean requested shutdown');
  }
  if (server?.listening) {
    let didClose = false;
    const closed = new Promise(resolve => server.close(() => { didClose = true; resolve(true); })); server.closeIdleConnections?.();
    if (!await Promise.race([closed, sleep(3000).then(() => false)])) { server.closeAllConnections?.(); await Promise.race([closed, sleep(1000)]); }
    report.cleanup.http_closed = didClose && !server.listening; if (!report.cleanup.http_closed) fault('cleanup', 'HTTP server did not fully close');
  }
}
try {
  assert.equal(process.env.RYGGATTACK_EXPLOSION_ARTIFACT_READY, '1', 'parent build/freeze acknowledgement required; see --help');
  assert.notEqual(process.env.NODE_TLS_REJECT_UNAUTHORIZED, '0', 'TLS bypass forbidden');
  report.source_revision = execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo, encoding: 'utf8'}).trim();
  report.source_worktree = execFileSync('git', ['status', '--porcelain'], {cwd: repo, encoding: 'utf8'}).trim();
  report.hashes = hashInputs();
  // Preserve exact reviewed source/helper bytes, including a dirty worker patch.
  for (const [relative, expected] of Object.entries(report.hashes)) {
    if (!/^(?:src\/|web\/|scripts\/|Cargo\.(?:toml|lock)$|docs\/online-explosion\.md$)/.test(relative)) continue;
    const bytes = fs.readFileSync(path.join(repo, relative));
    assert.equal(sha(bytes), expected.sha256, 'source snapshot matches frozen hash');
    const target = path.join(out, 'source-inputs', relative);
    fs.mkdirSync(path.dirname(target), {recursive: true}); fs.writeFileSync(target, bytes);
  }
  await startLocal(); await run();
  report.status = 'captured-review-required';
} catch (error) { report.status = 'failed'; report.failure = safeText(error.stack ?? error); }
finally {
  try { await shutdown(); } catch (error) { fault('shutdown', error); }
  if (report.hashes) try { assert.deepEqual(canonical(hashInputs()), canonical(report.hashes), 'source/artifacts changed during run'); report.cleanup.inputs_unchanged = true; } catch (error) { fault('freeze', error); }
  const cleanupExitCode = finalizeCleanupReport(report, {clients, interrupted: interruption.interrupted});
  process.exitCode ||= cleanupExitCode;
  report.finished_at = new Date().toISOString(); report.total_ms = now();
  // Whitelists only: no raw Welcome, browser endpoint, storage or DOM dump.
  report.clients = clients.map(c => ({name: c.name, identity: c.identity, browser_version: c.browser_version, pid: c.pid, sockets: c.sockets, origin_verified: c.origin_verified, cleanup: c.cleanup}));
  try {
    for (const [file, value] of [['inputs.json', clients.map(c => ({name: c.name, inputs: c.inputs, commands: c.commands}))],
      ['states.json', clients.map(c => ({name: c.name, frames: c.frames}))], ['lobbies.json', clients.map(c => ({name: c.name, lobbies: c.lobbies}))]]) {
      fs.writeFileSync(path.join(out, file), JSON.stringify(redact(value, secrets)) + '\n');
    }
    if (!publishSoloEvidence(out, redact(report, secrets), secrets, {privacyFailed})) process.exitCode = 1;
  } catch { process.exitCode = 1; }
  if (report.status !== 'captured-review-required') process.exitCode = 1;
  console.log(JSON.stringify({status: process.exitCode ? 'failed' : report.status, evidence_directory: out}));
  interruption.dispose();
}
