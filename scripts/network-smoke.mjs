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
const evidence = {artifact:Object.fromEntries(['index.html','ryggattack.js','ryggattack_bg.wasm'].map(f=>[f,crypto.createHash('sha256').update(fs.readFileSync(path.join(root,f))).digest('hex')])),clients:[],errors:[],checks:[],delay_ms_each_direction:75};
const clients = [];
function client(name) {
  const c={name,seat:null,frames:0,hashes:new Map(),inputs:[],rtts:[],sent:new Map(),shots:0,explosions:0,eliminated:0,rounds:0,moved:false,turns:false,initial:null,last:null}; clients.push(c); return c;
}
function frame(c,f) {
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
  server=spawn(path.join(bin,'ryggattack-server'),['127.0.0.1:0'],{stdio:['ignore','pipe','pipe']});
  server.stderr.on('data',b=>{serverLog+=b;});
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
  async function pageFor(name) {
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
      ws.on('framereceived',({payload})=>{try{const m=JSON.parse(payload);if(m.Welcome)c.seat=m.Welcome.seat;if(m.State)frame(c,m.State);if(m.Error)evidence.errors.push(`${name}: ${m.Error}`);}catch(e){evidence.errors.push(`${name} parse: ${e.message}`);}});
    });
    await page.bringToFront();
    await page.goto(`http://127.0.0.1:${staticServer.address().port}/?online=${encodeURIComponent(address)}&name=${name}`);
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
  const b=await pageFor('BrowserB');
  const native=client('native'); let text='',stderr='';
  probe=spawn(path.join(bin,'ryggattack-net-probe'),[address,'110'],{stdio:['ignore','pipe','pipe']});
  probe.stdout.on('data',chunk=>{text+=chunk;let i;while((i=text.indexOf('\n'))>=0){const line=text.slice(0,i);text=text.slice(i+1);try{const m=JSON.parse(line);native.seat=m.seat;frame(native,m.frame);}catch(e){evidence.errors.push(`native: ${e.message}`);}}});
  probe.stderr.on('data',b=>stderr+=b);
  const nativeDone=new Promise(r=>probe.on('exit',r));
  async function held(c,key,ms=2500){await c.page.bringToFront();await c.page.locator('canvas').focus();const start=c.inputs.length;await c.page.keyboard.down(key);await sleep(ms);await c.page.keyboard.up(key);return c.inputs.slice(start);}
  assert((await held(a,'d')).some(i=>i.horizontal===1),'browser A real keyboard steering');
  assert((await held(b,'w')).some(i=>i.vertical===1),'browser B real keyboard steering');
  assert((await held(a,'Space',3500)).some(i=>i.fire),'browser A real keyboard firing');
  assert((await held(b,'Space',3500)).some(i=>i.fire),'browser B real keyboard firing');
  // Escape suppresses input, never the authoritative simulation.
  await a.page.bringToFront(); await a.page.keyboard.press('Escape',{delay:500}); await sleep(1800);
  let start=a.inputs.length, tick=a.last.tick;
  await held(a,'Space',3000);
  assert(a.inputs.slice(start).length>0 && a.inputs.slice(start).every(i=>!i.fire&&i.horizontal===0&&i.vertical===0),'menu neutral intent');
  assert(a.last.tick>tick+64,'online menu does not pause server');
  await a.page.keyboard.press('Escape',{delay:500}); await sleep(1800);
  // Editing the HTML form must keep focus and suppress game keys.
  await a.page.locator('#online-entry summary').click();
  const name=a.page.getByRole('textbox',{name:'Player name',exact:true}); await name.fill('Typing');
  await sleep(1800); start=a.inputs.length;
  await name.press('Space',{delay:1500}); await sleep(1800);
  assert.equal(await a.page.evaluate(()=>document.activeElement.getAttribute('aria-label')),'Player name');
  assert(a.inputs.slice(start).length>0 && a.inputs.slice(start).every(i=>!i.fire&&i.horizontal===0&&i.vertical===0),'form neutral intent');
  // Playwright forces every headless page focused; bringToFront alone is not
  // a blur. Exercise a real DOM canvas blur while a fire key is held instead.
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
  for(const c of [a,b]) {await c.page.screenshot({path:path.join(out,`${c.name}.png`),timeout:180000}); assert(await c.page.evaluate(()=>globalThis.ryggattackOnline),'Bevy online app running');}
  const code=await nativeDone; assert.equal(code,0,stderr); evidence.native_summary=JSON.parse(stderr.trim().split('\n').at(-1));
  assert(evidence.native_summary.median_ack_rtt_ms>=140,'native RTT reflects injected two-way delay');
  assert.equal(new Set(clients.map(c=>c.seat)).size,3,'three distinct seats');
  for(const c of clients){assert(c.frames>20);assert(c.moved);assert(c.turns);assert(c.shots>0);}
  assert(clients.every(c=>c.eliminated>0 && c.rounds>0 && c.last.wins.some(w=>w>0)),'authoritative eliminations/scoring/round reset observed');
  const common=[...a.hashes.keys()].filter(t=>b.hashes.has(t)&&native.hashes.has(t));
  assert(common.length>20,'shared ticks across browser/browser/native');
  for(const t of common){assert.equal(a.hashes.get(t),b.hashes.get(t));assert.equal(a.hashes.get(t),native.hashes.get(t));}
  evidence.shared_identical_ticks=common.length;
  evidence.shared_tick_range=[common[0],common.at(-1)];
  // The networking gate above requires two concurrent rendered clients. The
  // unrelated offline gate does not require a third concurrent software GPU
  // workload: diagnostics showed its canvas present, but focus queries blocked
  // >30s with all three rendering, then completed in 4ms after closing these.
  for(const c of [a,b]) {await c.page.close();c.page=null;}
  console.log(`Crossplay passed: ${common.length} identical shared ticks; online pages closed before offline smoke`);
  // Preserve ordinary offline launch/lobby/play/pause on this exact artifact.
  offline=await (await browser.newContext({viewport:{width:640,height:480}})).newPage();
  offline.on('pageerror',e=>evidence.errors.push(`offline: ${e.message}`));
  offline.on('crash',()=>evidence.errors.push('offline renderer crashed'));
  offline.on('console',m=>{if(m.type()==='error')evidence.errors.push(`offline console: ${m.text()}`);});
  let offlineSockets=0;offline.on('websocket',()=>offlineSockets++);
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
  evidence.checks=['two actual Bevy WASM clients','native shared adapter crossplay','75ms each-way TCP delay','authoritative snapshots identical at same tick','keyboard steering and fire','menu neutral input/server advances','form focus neutral input','canvas blur clears held fire','offline launch/lobby/game/pause screenshots','raw traversal/malformed-path rejection'];
  evidence.server_log=serverLog;
  assert.equal(evidence.errors.length,0,JSON.stringify(evidence.errors));
} catch(error) {
  evidence.failure=String(error.stack ?? error);
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
  for(const c of clients){const sorted=c.rtts.sort((a,b)=>a-b);evidence.clients.push({name:c.name,events:c.events,seat:c.seat,frames:c.frames,projectile_observations:c.shots,explosion_observations:c.explosions,eliminated_frames:c.eliminated,round_finished_frames:c.rounds,moved:c.moved,turns:c.turns,input_count:c.inputs.length,wire_ack_rtt_median_ms:sorted[Math.floor(sorted.length/2)],wire_ack_rtt_p95_ms:sorted[Math.floor(sorted.length*.95)],last_tick:c.last?.tick,wins:c.last?.wins});}
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(evidence,null,2));
  await browser?.close();probe?.kill();server?.kill();for(const s of sockets)s.destroy();proxy?.close();staticServer.close();
}
console.log(JSON.stringify(evidence,null,2));
