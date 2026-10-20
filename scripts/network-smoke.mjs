// Real WASM clients + the shared native adapter; no mock game/server.
// Build native bins and dist/web first. Requires Playwright Chromium.
// PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json node scripts/network-smoke.mjs [output-dir]
import assert from 'node:assert/strict';
import http from 'node:http';
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawn} from 'node:child_process';
import {createRequire} from 'node:module';
const require = createRequire(process.env.PLAYWRIGHT_NODE_MODULES ?? import.meta.url);
const {chromium} = require('playwright');
const root = path.resolve('dist/web');
const out = path.resolve(process.argv[2] ?? 'dist/network-evidence');
fs.mkdirSync(out, {recursive:true});
const sleep = ms => new Promise(r=>setTimeout(r,ms));
const listen = s => new Promise(r=>s.listen(0,'127.0.0.1',r));
// Frame floats are Rust f32. serde_json::Value in the native probe prints
// their exact f64 promotion; direct Frame serialization uses shorter f32 text.
const canonical = x => Array.isArray(x) ? x.map(canonical) : x && typeof x === 'object' ? Object.fromEntries(Object.keys(x).sort().map(k=>[k,canonical(x[k])])) : typeof x==='number' && !Number.isInteger(x) ? Math.fround(x) : x;
const digest = x => crypto.createHash('sha256').update(JSON.stringify(canonical(x))).digest('hex');
assert.equal(digest({x:0.62}),digest({x:0.6200000047683716}));
assert.notEqual(digest({x:0.62}),digest({x:0.63}));
const evidence = {artifact:Object.fromEntries(['index.html','rooms.mjs','ryggattack.js','ryggattack_bg.wasm'].map(f=>[f,crypto.createHash('sha256').update(fs.readFileSync(path.join(root,f))).digest('hex')])),clients:[],errors:[],checks:[],delay_ms_each_direction:75};
const clients = [];
function client(name) {
  const c={name,seat:null,frames:0,hashes:new Map(),inputs:[],rtts:[],sent:new Map(),shots:0,explosions:0,eliminated:0,rounds:0,moved:false,turns:false,initial:null,last:null}; clients.push(c); return c;
}
function frame(c,f) {
  const observed={tick:f.tick,at_ms:performance.now()};c.first_observed??=observed;c.last_observed=observed;
  c.frames++; c.hashes.set(f.tick,digest(f));
  if (!c.initial) c.initial=f;
  c.last=f;
  if (c.seat !== null) {
    const a=c.initial.carts[c.seat], b=f.carts[c.seat];
    if(a && b) { c.moved ||= JSON.stringify(a.position)!==JSON.stringify(b.position); c.turns ||= JSON.stringify(a.forward)!==JSON.stringify(b.forward); }
    const seq=f.ack[c.seat]; if(c.sent.has(seq)) { c.rtts.push(performance.now()-c.sent.get(seq)); c.sent.delete(seq); }
  }
  c.shots += f.bodies.filter(b=>b.kind===0).length;
  c.explosions += f.bodies.filter(b=>b.kind===4).length;
  if(f.round_finished) c.rounds++;
  if(f.carts.some(cart=>cart.active&&!cart.alive))c.eliminated++;
}
const staticServer=http.createServer((req,res)=>{
  let file;
  try { let p=decodeURIComponent(req.url.split('?')[0]); if(p.includes('\0')) throw Error(); if(p.endsWith('/'))p+='index.html'; file=path.resolve(root,'.'+p); }
  catch {res.writeHead(400).end();return;}
  if(!file.startsWith(root+path.sep)){res.writeHead(403).end();return;}
  fs.readFile(file,(e,d)=>{if(e){res.writeHead(404).end();return;}res.writeHead(200,{'content-type':({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html'})[path.extname(file)]??'application/octet-stream'}).end(d);});
});
let browser, server, probe, proxy, offline;
let serverLog='';
const sockets=new Set();
try {
  await listen(staticServer);
  // Reject traversal on the raw HTTP path (browsers normalize dot segments).
  for(const [p,status] of [['/%2e%2e/Cargo.toml',403],['/%ZZ',400],['/%00',400]]) {
    const code=await new Promise((resolve,reject)=>http.get({host:'127.0.0.1',port:staticServer.address().port,path:p},r=>{r.resume();resolve(r.statusCode);}).on('error',reject)); assert.equal(code,status);
  }
  const bin=path.resolve('target/debug');
  server=spawn(path.join(bin,'ryggattack-server'),['127.0.0.1:0'],{stdio:['ignore','pipe','pipe'],env:{...process.env,RYGGATTACK_ALLOWED_ORIGINS:`http://127.0.0.1:${staticServer.address().port}`}});
  evidence.server_log_events=[];
  server.stderr.on('data',b=>{serverLog+=b;evidence.server_log_events.push({at_ms:performance.now(),text:String(b)});});
  const port=await new Promise((resolve,reject)=>{let text='';server.stdout.on('data',b=>{text+=b;const m=text.match(/ws:\/\/127\.0\.0\.1:(\d+)/);if(m)resolve(Number(m[1]));});server.on('exit',code=>reject(Error(`server exit ${code}: ${serverLog}`)));setTimeout(()=>reject(Error('server startup timeout')),10000).unref();});
  proxy=net.createServer(down=>{
    const up=net.connect(port,'127.0.0.1');
    for(const s of [down,up]){sockets.add(s);s.setNoDelay(true);s.on('error',()=>{down.destroy();up.destroy();});s.on('close',()=>sockets.delete(s));}
    // Delay chunks concurrently: pausing for every timer would accidentally
    // impose a packet-size-dependent bandwidth cap, not just link latency.
    for(const [a,b] of [[down,up],[up,down]]) {
      let queued=0,blocked=false;
      const resume=()=>{if(!blocked && queued<1024*1024 && !a.destroyed)a.resume();};
      a.on('data',data=>{
        queued+=data.length;
        if(queued>=1024*1024)a.pause();
        setTimeout(()=>{
          queued-=data.length;
          if(b.destroyed)return;
          if(!b.write(data) && !blocked){blocked=true;a.pause();b.once('drain',()=>{blocked=false;resume();});}
          resume();
        },75);
      });
      a.on('end',()=>setTimeout(()=>b.end(),75));
    }
  });
  await listen(proxy);
  const address=`ws://127.0.0.1:${proxy.address().port}`;
  browser=await chromium.launch({args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding']});
  async function pageFor(name,code=null) {
    const c=client(name);
    const page=await (await browser.newContext({viewport:{width:640,height:480}})).newPage();
    c.events=[];
    await page.addInitScript(()=>{
      globalThis.socketTrace=[];
      const Original=globalThis.WebSocket;
      globalThis.WebSocket=class extends Original {
        constructor(...args){super(...args);socketTrace.push(['construct',performance.now(),String(args[0])]);for(const event of ['open','error','close'])this.addEventListener(event,()=>socketTrace.push([event,performance.now()]));}
      };
    });
    page.on('pageerror',e=>evidence.errors.push(`${name}: ${e.message}`));
    page.on('console',msg=>{ if(msg.type()==='error' || msg.type()==='warn') { console.log(`${name} console ${msg.type()}: ${msg.text()}`); if(msg.type()==='error') evidence.errors.push(`${name} console: ${msg.text()}`); } });
    page.on('requestfailed',req=>evidence.errors.push(`${name} request: ${req.url()} ${req.failure()?.errorText}`));
    page.on('websocket',ws=>{
      c.events.push(['socket',performance.now()]);
      ws.on('close',()=>c.events.push(['close',performance.now()]));
      ws.on('socketerror',e=>c.events.push(['error',performance.now(),String(e)]));
      ws.on('framesent',({payload})=>{try{const m=JSON.parse(payload);if(m.Hello)c.events.push(['hello',performance.now()]);if(m.Input){const i={...m.Input,at:performance.now()}; c.inputs.push(i);c.sent.set(i.sequence,i.at);}}catch{}});
      ws.on('framereceived',({payload})=>{try{const m=JSON.parse(payload);if(m.Welcome){c.seat=m.Welcome.seat;c.identity=m.Welcome;}if(m.Lobby){c.lobby=m.Lobby;(c.lobbies??=[]).push(m.Lobby);}if(m.State)frame(c,m.State);if(m.Error)evidence.errors.push(`${name}: ${m.Error}`);}catch(e){evidence.errors.push(`${name} parse: ${e.message}`);}});
    });
    await page.bringToFront();
    await page.goto(`http://127.0.0.1:${staticServer.address().port}/?quality=0`);
    await page.locator('#online-entry summary').click();
    await page.getByRole('textbox',{name:'Backend address',exact:true}).fill(address);
    await page.getByRole('textbox',{name:'Player name',exact:true}).fill(name);
    if(code)await page.getByRole('textbox',{name:'Room code',exact:true}).fill(code);
    await page.locator(code?'#join-room':'#create-room').click();
    try {
      await page.waitForFunction(()=>globalThis.ryggattackOnline && !globalThis.ryggattackOnline.startsWith('99|'),null,{timeout:90000});
    } catch(e) {
      evidence.errors.push(`${name} DOM: ${await page.locator('body').innerText()}`);
      c.events.push(['trace',await page.evaluate(()=>({trace:globalThis.socketTrace,focused:document.hasFocus(),hidden:document.hidden,state:globalThis.ryggattackOnline}))]);
      await page.screenshot({path:path.join(out,`${name}-failed.png`),timeout:180000});
      throw e;
    }
    console.log(`${name} running seat ${c.seat}`);
    c.page=page; return c;
  }
  const a=await pageFor('BrowserA');
  const until=async(predicate,label,ms=90000)=>{const end=performance.now()+ms;while(!predicate()){assert(performance.now()<end,label);await sleep(50);}};
  await until(()=>a.lobby,'creator lobby');
  const roomCode=a.lobby.code, roomID=a.lobby.id;
  assert.equal(roomID,3,'documented first-room map/RNG fixture');
  evidence.fixture={simulation_seed:3,lifecycle_win_target:1,native_lifetime_seconds:600,completion_input:'A Left+Fire; B Fire; native alternating horizontal+Fire; real server bot AI'};
  if(!await a.page.locator('#online-entry').evaluate(e=>e.open))await a.page.locator('#online-entry summary').click();
  await a.page.locator('#room-controls').waitFor({state:'visible',timeout:90000});
  assert(await a.page.locator('#room-controls').isVisible(),'creator room controls expanded');
  const share=await a.page.locator('#share-room').inputValue();
  assert.equal(new URL(share).searchParams.get('room'),roomCode);assert(!new URL(share).searchParams.has('name'));
  const b=await pageFor('BrowserB',roomCode);
  await until(()=>b.lobby?.humans.length===2,'joined human replaces lobby bot');
  assert.equal(b.lobby.settings.bots,2);assert.equal(b.lobby.id,roomID);
  assert.notEqual(a.identity.participant,b.identity.participant);
  // A second room uses a transport-only device, not a third software renderer.
  const isolated=new WebSocket(address);
  let isolatedLobby;
  isolated.addEventListener('open',()=>isolated.send(JSON.stringify({Hello:{version:2,name:'Isolated'}})));
  isolated.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Lobby)isolatedLobby=m.Lobby;});
  await until(()=>isolatedLobby,'isolated room');assert.notEqual(isolatedLobby.id,roomID);assert.equal(isolatedLobby.humans.length,1);
  evidence.room={id:roomID,code:roomCode,share,isolated_id:isolatedLobby.id};
  await a.page.locator('#bots-room').fill('4');
  await a.page.locator('#apply-room').click();
  await a.page.waitForFunction(()=>document.getElementById('room-notice').textContent==='Invalid settings',null,{timeout:60000});
  await a.page.locator('#arena-room').selectOption('Volcano');
  await a.page.locator('#target-room').selectOption('3');
  await a.page.locator('#bots-room').fill('1');
  await a.page.locator('#apply-room').click();
  await until(()=>a.lobby.settings.arena==='Volcano'&&a.lobby.settings.bots===1,'host settings applied');
  const native=client('native'); let text='',stderr='';
  probe=spawn(path.join(bin,'ryggattack-net-probe'),[address,'600',roomCode],{stdio:['ignore','pipe','pipe']});
  probe.stdout.on('data',chunk=>{text+=chunk;let i;while((i=text.indexOf('\n'))>=0){const line=text.slice(0,i);text=text.slice(i+1);try{const m=JSON.parse(line);if(m.lobby){native.lobby=m.lobby;native.participant=m.participant;}if(m.frame){native.seat=m.seat;frame(native,m.frame);if(m.rtt_ms)native.rtts.push(m.rtt_ms);}}catch(e){evidence.errors.push(`native: ${e.message}`);}}});
  probe.stderr.on('data',b=>stderr+=b);
  let probeStopped=false;
  const nativeDone=new Promise(r=>probe.on('exit',(code,signal)=>{evidence.native_exit={code,signal,at_ms:performance.now()};if(!probeStopped)evidence.errors.push('native probe exited before the lifecycle gate completed');r(code);}));
  await until(()=>a.lobby.humans.length===3&&native.lobby,'native joins same room');
  if(!await b.page.locator('#online-entry').evaluate(e=>e.open))await b.page.locator('#online-entry summary').click();
  await b.page.locator('#room-controls').waitFor({state:'visible',timeout:90000});
  assert(await b.page.locator('#room-controls').isVisible(),'joined room controls expanded');
  assert(await b.page.locator('#apply-room').isDisabled(),'non-host settings disabled');
  await a.page.locator('#ready-room').click();
  await until(()=>a.lobby.humans.find(h=>h.id===a.identity.participant).ready,'host ready');
  await b.page.locator('#ready-room').click();
  await a.page.locator('#ready-room').click();
  await until(()=>a.lobbies.some(l=>l.phase.Countdown),'authoritative countdown observed');
  await until(()=>a.lobby.phase==='Lobby'&&!a.lobby.humans.find(h=>h.id===a.identity.participant).ready,'unready cancels countdown');
  await a.page.waitForFunction(()=>document.getElementById('room-info').textContent.includes('· Lobby')&&document.getElementById('ready-room').textContent==='Ready',null,{timeout:60000});
  await a.page.screenshot({path:path.join(out,'lobby-cancelled.png'),timeout:180000});
  await a.page.locator('#target-room').selectOption('1');await a.page.locator('#apply-room').click();
  await until(()=>a.lobby.settings.wins_target===1,'select genuine first-to-one lifecycle fixture');
  await until(()=>a.lobby.humans.every(h=>!h.ready)||a.lobby.humans.filter(h=>h.ready).every(h=>h.id===native.participant),'settings clear human readiness');
  const ensureRunning=async()=>{
    if(a.last?.winner!==null&&a.last?.winner!==undefined)await until(()=>a.lobby.phase==='Lobby','finished match returns before next input check');
    for(const c of [a,b]) {
      if(c.lobby?.phase!=='Running'&&!c.lobby?.humans.find(h=>h.id===c.identity.participant)?.ready) {
        if(!await c.page.locator('#online-entry').evaluate(e=>e.open))await c.page.locator('#online-entry summary').click();
        await c.page.waitForFunction(()=>{const s=JSON.parse(globalThis.ryggattackRoom??'{}');return s.lobby?.phase!=='Running'&&s.lobby?.humans.some(h=>h.id===s.participant&&!h.ready);},null,{timeout:60000});
        await c.page.locator('#ready-room').click();
      }
    }
    await until(()=>a.lobby?.phase==='Running'&&b.lobby?.phase==='Running'&&a.last?.winner===null&&b.last?.winner===null,'shared unfinished match running');
    for(const c of [a,b])if(await c.page.locator('#online-entry').evaluate(e=>e.open))await c.page.locator('#online-entry summary').click();
  };
  await ensureRunning();
  assert.equal(isolatedLobby.humans.length,1);assert.equal(isolatedLobby.settings.arena,'Forest');isolated.close();
  for(const c of [a,b]){
    await c.page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').lobby?.phase==='Running',null,{timeout:60000});
    await c.page.screenshot({path:path.join(out,`initial-playing-${c.name}.png`),timeout:180000});
  }
  async function held(c,key,ms=2500){await ensureRunning();await c.page.bringToFront();await c.page.locator('canvas').focus();const start=c.inputs.length;await c.page.keyboard.down(key);await sleep(ms);await c.page.keyboard.up(key);return c.inputs.slice(start);}
  assert((await held(a,'d')).some(i=>i.horizontal===1),'browser A real keyboard steering');
  assert((await held(b,'w')).some(i=>i.vertical===1),'browser B real keyboard steering');
  assert((await held(a,'Space',3500)).some(i=>i.fire),'browser A real keyboard firing');
  assert((await held(b,'Space',3500)).some(i=>i.fire),'browser B real keyboard firing');
  // Escape suppresses input, never the authoritative simulation.
  await ensureRunning();await a.page.bringToFront();await a.page.locator('canvas').focus(); await a.page.keyboard.press('Escape',{delay:500}); await sleep(1800);
  let start=a.inputs.length, tick=a.last.tick;
  await held(a,'Space',3000);
  assert(a.inputs.slice(start).length>0 && a.inputs.slice(start).every(i=>!i.fire&&i.horizontal===0&&i.vertical===0),'menu neutral intent');
  assert(a.last.tick>tick+64,'online menu does not pause server');
  await a.page.keyboard.press('Escape',{delay:500}); await sleep(1800);
  // Editing the HTML form must keep focus and suppress game keys.
  await ensureRunning();
  await a.page.locator('#online-entry summary').click();
  const name=a.page.getByRole('textbox',{name:'Player name',exact:true}); await name.fill('Typing');
  await sleep(1800); start=a.inputs.length;
  await name.press('Space',{delay:1500}); await sleep(1800);
  assert.equal(await a.page.evaluate(()=>document.activeElement.getAttribute('aria-label')),'Player name');
  assert(a.inputs.slice(start).length>0 && a.inputs.slice(start).every(i=>!i.fire&&i.horizontal===0&&i.vertical===0),'form neutral intent');
  // Playwright forces every headless page focused; bringToFront alone is not
  // a blur. Exercise a real DOM canvas blur while a fire key is held instead.
  await ensureRunning();
  await a.page.locator('canvas').focus(); await a.page.keyboard.down('Space'); await sleep(2000);
  await a.page.locator('canvas').evaluate(el=>el.blur());
  assert.notEqual(await a.page.evaluate(()=>document.activeElement?.id),'ryggattack-canvas');
  await b.page.bringToFront(); await sleep(1800); start=a.inputs.length; tick=a.last.tick; await sleep(3000);
  assert(a.inputs.slice(start).length>0 && a.inputs.slice(start).every(i=>!i.fire),'canvas blur clears held fire');
  assert(a.last.tick>tick+64,'blur does not pause');
  await a.page.keyboard.up('Space');
  console.log('Input/menu/form/canvas-blur checks passed');
  // Continue real simultaneous fire through network delay, exercising elimination/rounds.
  for(const c of [a,b]){await c.page.bringToFront();await c.page.locator('canvas').focus();await c.page.keyboard.down('Space');}
  await sleep(22000);
  for(const c of [a,b])await c.page.keyboard.up('Space');
  for(const c of [a,b]) {if(await c.page.locator('#online-entry').evaluate(e=>e.open))await c.page.locator('#online-entry summary').click();await c.page.screenshot({path:path.join(out,`${c.name}.png`),timeout:180000}); assert(await c.page.evaluate(()=>globalThis.ryggattackOnline),'Bevy online app running');}
  // Finish naturally, then verify the authoritative SAME lobby, not a new match.
  await ensureRunning();
  for(const c of [a,b]){await c.page.locator('canvas').focus();await c.page.keyboard.down('Space');}
  const completionEnd=performance.now()+180000, completionHeld=new Map();evidence.completion_controls=[];
  while(!(a.lobbies.some(l=>l.phase==='Running')&&a.lobby.phase==='Lobby'&&a.last?.winner!==null)){
    assert(performance.now()<completionEnd,'natural victory returns to lobby');
    // A dead player cannot steer: move the first surviving BROWSER driver off
    // the neutral outer loop. This is real held Left/Fire input, never state.
    const driver=[a,b].find(c=>c.last?.carts[c.seat]?.alive);
    for(const c of [a,b]){const left=c===driver;if(completionHeld.get(c)!==left){if(left)await c.page.keyboard.down('a');else await c.page.keyboard.up('a');completionHeld.set(c,left);evidence.completion_controls.push({name:c.name,left,tick:c.last?.tick,alive:c.last?.carts.map(p=>p.alive)});}}
    await sleep(750);
  }
  for(const c of [a,b])await c.page.keyboard.up('Space');
  for(const c of [a,b])await c.page.keyboard.up('a');
  await until(()=>b.lobby.phase==='Lobby','both clients see lobby return');
  assert.equal(a.lobby.id,roomID);assert.equal(a.lobby.humans.length,3);assert.equal(a.lobby.settings.arena,'Volcano');
  assert(a.lobby.humans.filter(h=>h.id!==native.participant).every(h=>!h.ready));
  evidence.returned_lobby=a.lobby;
  const returnedClear=a.lobbies.slice(a.lobbies.findIndex(l=>l.phase==='Running')+1).find(l=>l.phase==='Lobby'&&l.humans.length===3&&l.humans.every(h=>!h.ready));
  assert(returnedClear,'server clears ALL readiness before the native probe readies again');
  evidence.returned_lobby_cleared=returnedClear;
  if(!await a.page.locator('#online-entry').evaluate(e=>e.open))await a.page.locator('#online-entry summary').click();
  await a.page.waitForFunction(()=>document.getElementById('room-info').textContent.includes('· Lobby')&&document.getElementById('ready-room').textContent==='Ready',null,{timeout:60000});
  await a.page.screenshot({path:path.join(out,'returned-lobby.png'),timeout:180000});
  probeStopped=true;probe.kill('SIGINT');await nativeDone;
  const sortedRTT=native.rtts.sort((a,b)=>a-b);evidence.native_summary={seat:native.seat,median_ack_rtt_ms:sortedRTT[Math.floor(sortedRTT.length/2)]};
  assert(evidence.native_summary.median_ack_rtt_ms>=140,'native RTT reflects injected two-way delay');
  assert.equal(new Set(clients.map(c=>c.seat)).size,3,'three distinct seats');
  for(const c of clients){assert(c.frames>20);assert(c.moved);assert(c.turns);assert(c.shots>0);}
  assert(clients.every(c=>c.eliminated>0 && c.rounds>0 && c.last.wins.some(w=>w>0)),'authoritative eliminations/scoring/round reset observed');
  const common=[...a.hashes.keys()].filter(t=>b.hashes.has(t)&&native.hashes.has(t));
  assert(common.length>20,'shared ticks across browser/browser/native');
  for(const t of common){assert.equal(a.hashes.get(t),b.hashes.get(t));assert.equal(a.hashes.get(t),native.hashes.get(t));}
  evidence.shared_identical_ticks=common.length;
  evidence.shared_tick_range=[common[0],common.at(-1)];
  if(!await a.page.locator('#online-entry').evaluate(e=>e.open))await a.page.locator('#online-entry summary').click();
  await a.page.locator('#leave-room').click();
  await until(()=>b.lobby.humans.length===1&&b.lobby.host===b.identity.participant,'deliberate leave elects earliest remaining human');
  await a.page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom).status.includes('Left room'),null,{timeout:60000});
  await a.page.close();a.page=null;
  // Reuse the real share link after freeing a renderer. It prefills the code
  // and admits a NEW participant, never taking over the disconnected owner.
  const linkPage=await (await browser.newContext({viewport:{width:640,height:480}})).newPage();
  linkPage.on('pageerror',e=>evidence.errors.push(`share-link: ${e.message}`));
  let linkedIdentity;
  linkPage.on('websocket',ws=>ws.on('framereceived',({payload})=>{const m=JSON.parse(payload);if(m.Welcome)linkedIdentity=m.Welcome;}));
  await linkPage.goto(share+'&name=LinkGuest');
  await linkPage.waitForFunction(()=>globalThis.ryggattackRoom&&JSON.parse(globalThis.ryggattackRoom).lobby?.humans.some(h=>h.name==='LinkGuest'),null,{timeout:90000});
  assert.equal(await linkPage.locator('#room-code').inputValue(),roomCode);
  assert.equal(linkedIdentity.room,roomID);assert.notEqual(linkedIdentity.participant,a.identity.participant);
  await linkPage.screenshot({path:path.join(out,'share-link-joined.png'),timeout:180000});
  evidence.share_link_identity=linkedIdentity;
  await linkPage.close();
  // The networking gate above requires two concurrent rendered clients. The
  // unrelated offline gate does not require a third concurrent software GPU
  // workload: diagnostics showed its canvas present, but focus queries blocked
  // >30s with all three rendering, then completed in 4ms after closing these.
  for(const c of [a,b]) {if(c.page)await c.page.close();c.page=null;}
  console.log(`Crossplay passed: ${common.length} identical shared ticks; online pages closed before offline smoke`);
  // Preserve ordinary offline launch/lobby/play/pause on this exact artifact.
  offline=await (await browser.newContext({viewport:{width:640,height:480}})).newPage();
  offline.on('pageerror',e=>evidence.errors.push(`offline: ${e.message}`));
  offline.on('crash',()=>evidence.errors.push('offline renderer crashed'));
  offline.on('console',m=>{if(m.type()==='error')evidence.errors.push(`offline console: ${m.text()}`);});
  let offlineSockets=0;offline.on('websocket',()=>offlineSockets++);
  await offline.goto(`http://127.0.0.1:${staticServer.address().port}/?online=${encodeURIComponent(address)}&room=bad`);
  await offline.waitForFunction(()=>globalThis.ryggattackRoom&&JSON.parse(globalThis.ryggattackRoom).status.includes('invalid room code'),null,{timeout:90000});
  await offline.screenshot({path:path.join(out,'invalid-invite.png'),timeout:180000});
  assert.equal(offlineSockets,0,'malformed invite is visible without opening a socket');
  await offline.goto(`http://127.0.0.1:${staticServer.address().port}/?quality=0`);
  await offline.waitForFunction(()=>!document.getElementById('status'),null,{timeout:240000});
  evidence.offline_lifecycle=await offline.evaluate(()=>({url:location.href,title:document.title,ready:document.readyState,canvases:document.querySelectorAll('canvas').length,iframes:document.querySelectorAll('iframe').length}));
  assert.equal(evidence.offline_lifecycle.canvases,1);
  await offline.locator('#ryggattack-canvas').waitFor({state:'visible',timeout:90000});
  await offline.screenshot({path:path.join(out,'offline-menu.png'),timeout:180000});
  await offline.locator('canvas').focus();
  for(const key of ['Enter','w','Enter']) {await offline.keyboard.down(key);await sleep(1800);await offline.keyboard.up(key);await sleep(2500);}
  await offline.screenshot({path:path.join(out,'offline-playing.png'),timeout:180000});
  await offline.keyboard.press('Escape',{delay:600});await sleep(3000);
  await offline.screenshot({path:path.join(out,'offline-pause.png'),timeout:180000});
  assert.equal(offlineSockets,0,'offline never connects');
  evidence.checks=['private create/join through actual HTML forms','host settings and disabled non-host controls','unready cancels authoritative countdown','settings clear readiness','isolated second room unaffected','natural victory returns same room/settings/connected humans','deliberate leave and earliest-human host election','share link prefill/join uses new participant ID','visible malformed invite error','two actual Bevy WASM clients','native shared adapter crossplay','75ms each-way TCP delay','authoritative snapshots identical at same tick','keyboard steering and fire','menu neutral input/server advances','form focus neutral input','canvas blur clears held fire','offline launch/lobby/game/pause screenshots','raw traversal/malformed-path rejection'];
  evidence.server_log=serverLog;
  assert.equal(evidence.errors.length,0,JSON.stringify(evidence.errors));
} catch(error) {
  evidence.failure=String(error.stack ?? error);
  evidence.failure_state=clients.map(c=>({name:c.name,identity:c.identity,lobby:c.lobby,last_frame:c.last,last_inputs:c.inputs.slice(-16)}));
  if(offline){
    evidence.offline_target={url:offline.url(),closed:offline.isClosed(),frames:offline.frames().map(f=>f.url())};
    try{await offline.screenshot({path:path.join(out,'offline-failure.png'),timeout:30000});}catch(e){evidence.offline_screenshot_error=String(e);}
  }
  for(const c of clients) if(c.page) {
    try {await c.page.screenshot({path:path.join(out,`${c.name}-failure.png`),timeout:30000});} catch {}
  }
  throw error;
} finally {
  evidence.server_log=serverLog;
  evidence.server_overruns=(serverLog.match(/tick overrun/g)||[]).length;
  for(const c of clients){const sorted=c.rtts.sort((a,b)=>a-b);const hz=c.last_observed&&c.first_observed?(c.last_observed.tick-c.first_observed.tick)/((c.last_observed.at_ms-c.first_observed.at_ms)/1000):null;evidence.clients.push({name:c.name,events:c.events,seat:c.seat,frames:c.frames,first_observed:c.first_observed,last_observed:c.last_observed,observed_server_hz:hz,simulated_to_wall_ratio:hz/64,projectile_observations:c.shots,explosion_observations:c.explosions,eliminated_frames:c.eliminated,round_finished_frames:c.rounds,moved:c.moved,turns:c.turns,input_count:c.inputs.length,wire_ack_rtt_median_ms:sorted[Math.floor(sorted.length/2)],wire_ack_rtt_p95_ms:sorted[Math.floor(sorted.length*.95)],last_tick:c.last?.tick,wins:c.last?.wins});}
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(evidence,null,2));
  await browser?.close();probe?.kill();server?.kill();for(const s of sockets)s.destroy();proxy?.close();staticServer.close();
}
console.log(JSON.stringify(evidence,null,2));
