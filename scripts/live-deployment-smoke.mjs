// PUBLIC acceptance only: never starts servers, builds, maps DNS, or bypasses TLS.
// After rollout: node scripts/live-deployment-smoke.mjs --live
// Optional: --url https://public-host --out verification/live-deployment/<run>
// Requires existing target/debug/ryggattack-net-probe and Playwright Chromium.
// PLAYWRIGHT_NODE_MODULES may point to a package.json resolving playwright.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {isIP} from 'node:net';
import {spawn} from 'node:child_process';
import {createRequire} from 'node:module';
import {canonical,redact} from './live-deployment-helpers.mjs';

const args = process.argv.slice(2);
const usage = 'Usage: node scripts/live-deployment-smoke.mjs --live [--url https://ryggattack.modig.online] [--out verification/live-deployment/<new-run>]\n--live explicitly authorizes creation of ONE disposable private room on the deployed public endpoint. Without it, no network or browser work occurs.';
if (args.length === 1 && ['--help','-h'].includes(args[0])) { console.log(usage); process.exit(0); }
let authorized = false, endpoint = 'https://ryggattack.modig.online', output;
for (let i=0;i<args.length;i++) {
  if (args[i] === '--live' && !authorized) authorized = true;
  else if (['--url','--out'].includes(args[i]) && args[i+1] && !args[i+1].startsWith('--')) {
    const key=args[i++]; if(key==='--url')endpoint=args[i];else output=args[i];
  } else { console.error(usage); process.exit(2); }
}
if (!authorized) { console.error('Refusing live actions: --live is required.\n'+usage); process.exit(2); }
const site = new URL(endpoint);
assert(site.protocol==='https:' && !site.username && !site.password && !site.search && !site.hash && site.pathname==='/', 'public HTTPS origin only');
assert(!isIP(site.hostname.replace(/^\[|\]$/g,'')) && !/(^localhost$|\.localhost$|\.local$)/i.test(site.hostname) && site.hostname.includes('.'), 'public DNS hostname required');
assert(!site.port || site.port==='443','standard trusted HTTPS port required');
assert.notEqual(process.env.NODE_TLS_REJECT_UNAUTHORIZED,'0','TLS verification must remain enabled');
const origin=site.origin, backend=`wss://${site.host}/ws`;
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const evidenceRoot=path.join(repo,'verification/live-deployment');
const out=path.resolve(repo,output??path.join(evidenceRoot,new Date().toISOString().replace(/[:.]/g,'-')+'-'+crypto.randomBytes(3).toString('hex')));
assert(out.startsWith(evidenceRoot+path.sep),'evidence must be a fresh child of verification/live-deployment');
// Never overwrite historical evidence or write any private-key material.
fs.mkdirSync(evidenceRoot,{recursive:true}); fs.mkdirSync(out,{recursive:false});
const sha = data => crypto.createHash('sha256').update(data).digest('hex');
const secrets=new Set(), clients=[];
const evidence={status:'running',started_at:new Date().toISOString(),origin,backend,artifacts:{},errors:[],expected_outage_errors:[],console:[],socket_handshakes:[],checks:[],cleanup:{scope:'only the newly created private room; no administration, deletion, enumeration, or writes to other rooms',rendered:[]}};
let browser,probe,probeDone,probeStopped=false,stopping=false,interrupted=false,outage=false;
let native,stderrBytes=0,stderrHash=crypto.createHash('sha256');
const deadline=performance.now()+420000;
const interrupt=()=>{interrupted=true;};
process.on('SIGINT',interrupt);process.on('SIGTERM',interrupt);
const pause = ms => new Promise(resolve=>setTimeout(resolve,ms));
async function until(predicate,label,ms=90000,cleanup=false) {
  const end=performance.now()+ms;
  while(!predicate()) {
    if(!cleanup) { assert(!interrupted,'run interrupted'); assert(performance.now()<deadline,'overall acceptance deadline exceeded'); }
    assert(performance.now()<end,label); await pause(25);
  }
}
function fault(source,error,expected=false) {
  (expected?evidence.expected_outage_errors:evidence.errors).push({source,at_ms:performance.now(),message:String(error)});
}
function client(name) {
  const c={name,hashes:new Map(),inputs:[],welcomes:[],events:[],rtts:[],frames:0}; clients.push(c);return c;
}
function frame(c,f) {
  assert(Number.isSafeInteger(f.tick),'valid authoritative tick');
  const observation={tick:f.tick,simulation_tick:f.simulation_tick,at_ms:performance.now()};
  c.first??=observation;c.observed=observation;c.last=f;c.frames++;
  c.hashes.set(f.tick,sha(JSON.stringify(canonical(f))));
}
const safeIdentity = w => Object.fromEntries(['version','seat','room','participant','code'].map(key=>[key,w[key]]));
async function expand(c) { if(!await c.page.locator('#online-entry').evaluate(el=>el.open))await c.page.locator('#online-entry summary').click(); }
async function collapse(c) { if(await c.page.locator('#online-entry').evaluate(el=>el.open))await c.page.locator('#online-entry summary').click(); }
async function screenshot(c,name) { await c.page.screenshot({path:path.join(out,name+'.png'),timeout:15000}); }
async function pageFor(name,code) {
  const c=client(name);
  c.context=await browser.newContext({viewport:{width:800,height:600},ignoreHTTPSErrors:false});
  c.page=await c.context.newPage(); c.page.setDefaultTimeout(30000);c.page.setDefaultNavigationTimeout(45000);
  await c.page.addInitScript(()=>{
    // Capture actual runtime-created instances BEFORE the WASM adapter starts.
    globalThis.__acceptanceSockets=[];
    const Original=globalThis.WebSocket;
    globalThis.WebSocket=class extends Original {
      constructor(...args) { super(...args);globalThis.__acceptanceSockets.push(this); }
    };
  });
  const session=await c.context.newCDPSession(c.page), socketURLs=new Map();
  await session.send('Network.enable');
  session.on('Network.webSocketCreated',e=>socketURLs.set(e.requestId,e.url));
  session.on('Network.webSocketWillSendHandshakeRequest',e=>{
    const headers=e.request.headers;
    const observedOrigin=Object.entries(headers).find(([key])=>key.toLowerCase()==='origin')?.[1];
    evidence.socket_handshakes.push({client:name,url:socketURLs.get(e.requestId),origin:observedOrigin});
  });
  c.page.on('crash',()=>fault(name,'renderer crashed'));
  c.page.on('pageerror',e=>fault(name,e.message));
  c.page.on('console',m=>{
    if(!['error','warning'].includes(m.type()))return;
    const expected=outage && c.name==='LiveA' && /WebSocket|ERR_INTERNET_DISCONNECTED|ERR_NETWORK_CHANGED/i.test(m.text());
    evidence.console.push({client:name,type:m.type(),expected_outage:expected,text:m.text()});
    if(m.type()==='error')fault(name,m.text(),expected);
  });
  c.page.on('requestfailed',r=>fault(name,`${r.method()} ${r.url()} ${r.failure()?.errorText}`,outage&&name==='LiveA'&&/ERR_INTERNET_DISCONNECTED|ERR_NETWORK_CHANGED/i.test(r.failure()?.errorText??'')));
  c.page.on('response',r=>{if(r.status()>=400)fault(name,`HTTP ${r.status()} ${r.url()}`);});
  c.page.on('websocket',ws=>{
    if(ws.url()!==backend)fault(name,'unexpected WebSocket URL');
    c.events.push({event:'socket',url:ws.url(),at_ms:performance.now()});
    ws.on('close',()=>c.events.push({event:'close',at_ms:performance.now()}));
    ws.on('socketerror',e=>fault(name,e,outage&&name==='LiveA'));
    ws.on('framereceived',({payload})=>{
      try {
        const m=JSON.parse(String(payload));redact(m,secrets);
        if(m.Welcome) {c.identity=safeIdentity(m.Welcome);c.welcomes.push(c.identity);}
        if(m.Lobby)c.lobby=m.Lobby;
        if(m.State)frame(c,m.State);
        if(m.Error || m.Notice)fault(name,m.Error??m.Notice);
      }catch{fault(name,'received packet failed JSON/state validation');}
    });
    ws.on('framesent',({payload})=>{
      try {
        const m=JSON.parse(String(payload));redact(m,secrets);
        const kind=typeof m==='string'?m:Object.keys(m)[0];
        if(!['Hello','Reconnect','Input','Configure','Ready','Leave'].includes(kind))fault(name,'unexpected outbound command');
        if(m.Hello) {
          assert.equal(m.Hello.room??null,code??null,'admission is scoped to the disposable room');
          c.events.push({event:'Hello',at_ms:performance.now()});
        }
        if(m.Reconnect)assert.equal(m.Reconnect.room,c.identity.code);
        if(m.Input)c.inputs.push({...m.Input,at_ms:performance.now()});
        if(kind==='Leave')c.leaveSent=true;
      }catch{fault(name,'sent packet failed JSON/scope validation');}
    });
  });
  await c.page.goto(`${origin}/?quality=0${code?'&room='+encodeURIComponent(code):''}`,{waitUntil:'domcontentloaded'});
  await c.page.waitForFunction(expected=>document.getElementById('backend')?.value===expected,backend);
  await expand(c);await c.page.locator('#player-name').fill(name);
  if(code) { assert.equal(await c.page.locator('#room-code').inputValue(),code); }
  await c.page.locator(code?'#join-room':'#create-room').click();
  await until(()=>c.identity&&c.lobby,`${name} real HTML form admission`);
  await c.page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').lobby);
  c.dom=await c.page.evaluate(()=>({origin:location.origin,backend:document.getElementById('backend').value,canvas_count:document.querySelectorAll('canvas').length}));
  assert.deepEqual(c.dom,{origin,backend,canvas_count:1});
  return c;
}
async function leave(c,observers) {
  await c.context.setOffline(false);await expand(c);await c.page.locator('#leave-room').click();
  await until(()=>c.leaveSent,`${c.name} real Leave packet`,10000,true);
  await c.page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').status?.includes('Left room'),null,{timeout:10000});
  await until(()=>observers.every(o=>o.lobby?.id===c.identity.room&&!o.lobby.humans.some(h=>h.id===c.identity.participant)),`${c.name} removed only from test lobby`,10000,true);
  evidence.cleanup.rendered.push({client:c.name,participant:c.identity.participant,room:c.identity.room,leave_packet:true,observed_removed_by:observers.map(o=>o.name)});
  c.left=true;
}

try {
  const binary=path.join(repo,'target/debug/ryggattack-net-probe');fs.accessSync(binary,fs.constants.X_OK);
  const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
  for(const [file,mime] of [['index.html',/^text\/html\b/],['rooms.mjs',/^(text|application)\/javascript\b/],['ryggattack.js',/^(text|application)\/javascript\b/],['ryggattack_bg.wasm',/^application\/wasm\b/]]) {
    const url=`${origin}/${file}`;
    const response=await fetch(url,{redirect:'error',signal:AbortSignal.timeout(30000)});
    const bytes=Buffer.from(await response.arrayBuffer());
    const contentType=response.headers.get('content-type')??'';
    evidence.artifacts[file]={url:response.url,status:response.status,mime:contentType,bytes:bytes.length,sha256:sha(bytes)};
    assert.equal(response.status,200,`${file} HTTP status`);assert(mime.test(contentType),`${file} MIME`);assert(bytes.length>0,`${file} nonempty`);
    if(file.endsWith('.wasm'))assert(bytes.subarray(0,4).equals(Buffer.from([0,97,115,109])),'WASM magic');
  }
  evidence.checks.push('trusted public HTTPS asset statuses, MIME types and SHA-256 hashes');
  browser=await chromium.launch({args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding']});
  const a=await pageFor('LiveA'),b=await pageFor('LiveB',a.lobby.code);
  evidence.room={id:a.lobby.id,code:a.lobby.code,created_by_this_run:true};
  assert.equal(a.identity.room,b.identity.room);assert.equal(a.lobby.humans.length,2);
  await expand(a);await a.page.locator('#bots-room').fill('0');await a.page.locator('#target-room').selectOption('7');await a.page.locator('#apply-room').click();
  await until(()=>a.lobby.settings.bots===0&&a.lobby.settings.wins_target===7,'host bots=0 wins=7 applied');
  native=client('Native');let buffer='';
  probe=spawn(binary,[backend,'600',a.lobby.code],{stdio:['ignore','pipe','pipe']});
  probe.on('error',()=>fault('Native','probe could not start'));
  probe.stdout.on('data',chunk=>{
    buffer+=chunk;
    if(buffer.length>8*1024*1024) {buffer='';fault('Native','oversize probe line');return;}
    let end;
    while((end=buffer.indexOf('\n'))>=0) {
      const line=buffer.slice(0,end);buffer=buffer.slice(end+1);
      try {
        const m=JSON.parse(line);redact(m,secrets);
        if(m.lobby) {native.lobby=m.lobby;native.participant=m.participant;}
        if(m.frame) {native.seat=m.seat;frame(native,m.frame);if(Number.isFinite(m.rtt_ms))native.rtts.push(m.rtt_ms);}
      }catch{fault('Native','probe output failed JSON/state validation');}
    }
  });
  // Never dump raw native stderr (including on failure). Safe digest/byte count only.
  probe.stderr.on('data',chunk=>{stderrBytes+=chunk.length;stderrHash.update(chunk);});
  probeDone=new Promise(resolve=>probe.once('close',(code,signal)=>{
    evidence.native_exit={code,signal,controlled:probeStopped,at_ms:performance.now()};
    if(!probeStopped)fault('Native','probe exited before controlled teardown');resolve();
  }));
  await until(()=>native.lobby&&a.lobby.humans.length===3,'shared native adapter admitted');
  assert.equal(native.lobby.id,a.lobby.id);assert.equal(native.lobby.code,a.lobby.code);
  assert.equal(new Set([a.identity.participant,b.identity.participant,native.participant]).size,3);
  await expand(b);assert(await b.page.locator('#apply-room').isDisabled(),'non-host settings are disabled');
  for(const c of [a,b]) {await expand(c);await c.page.locator('#ready-room').click();}
  await until(()=>clients.every(c=>c.last)&&a.lobby.phase==='Running'&&b.lobby.phase==='Running','actual ready/countdown/start and all three authoritative streams');
  assert.equal(new Set([a.identity.seat,b.identity.seat,native.seat]).size,3);
  for(const c of [a,b]) {
    await c.page.waitForFunction(()=>Number(globalThis.ryggattackOnline?.split('|')[1])>0);
    await collapse(c);await c.page.locator('canvas').focus();
    const start=c.inputs.length;
    await c.page.keyboard.down('d');await c.page.keyboard.down('Space');
    await until(()=>c.inputs.slice(start).some(i=>i.horizontal===1&&i.fire),`${c.name} real steering/fire input packets`);
    await c.page.keyboard.up('d');await c.page.keyboard.up('Space');
  }
  await screenshot(a,'live-game');
  const progressStart={...b.observed};
  await until(()=>b.last.tick>=progressStart.tick+192&&b.last.simulation_tick>=progressStart.simulation_tick+128&&performance.now()-progressStart.at_ms>=2000,'meaningful server/simulation clock progression');
  const progressEnd={...b.observed},hz=(progressEnd.tick-progressStart.tick)/((progressEnd.at_ms-progressStart.at_ms)/1000);
  assert(hz>8&&hz<128,'server clock advances relative to wall time, not just a nonzero tick');
  evidence.clock={start:progressStart,end:progressEnd,observed_hz:hz};
  evidence.checks.push('two real rendered WASM clients and shared native adapter in one private room','real host settings, native automatic ready, browser Ready and authoritative Running','actual keyboard steering/fire packets from both WASM clients','meaningful authoritative/simulation clock progression');

  await a.page.locator('canvas').focus();const heldStart=a.inputs.length;
  await a.page.keyboard.down('d');await a.page.keyboard.down('Space');
  await until(()=>a.inputs.slice(heldStart).some(i=>i.horizontal===1&&i.fire),'held controls before disconnect');
  const identity={...a.identity},welcomes=a.welcomes.length,closes=a.events.filter(e=>e.event==='close').length;
  const selected=await a.page.evaluate(()=>{
    globalThis.__acceptanceActive=globalThis.__acceptanceSockets.findLast(s=>s.readyState===WebSocket.OPEN);
    return Boolean(globalThis.__acceptanceActive);
  });assert(selected,'actual page-created OPEN WebSocket captured');
  outage=true;await a.context.setOffline(true);
  await a.page.evaluate(()=>globalThis.__acceptanceActive.close(4000,'acceptance outage'));
  await until(()=>a.events.filter(e=>e.event==='close').length>closes,'actual transport close observed');
  const healthyStart={...b.observed};
  await until(()=>b.lobby.phase==='Running'&&b.lobby.humans.some(h=>h.id===identity.participant&&!h.connected)&&b.last.carts[identity.seat].active,'other human observes disconnected occupied seat during Running');
  await expand(b);
  await b.page.waitForFunction(()=>document.getElementById('room-info').textContent.includes('LiveA: Bot takeover'));
  await until(()=>b.last.tick>=healthyStart.tick+128&&b.last.simulation_tick>=healthyStart.simulation_tick+64,'healthy client continues meaningful simulation during outage');
  assert.equal(a.welcomes.length,welcomes,'offline context holds reconnection');
  evidence.outage={closed_runtime_socket:true,offline_context_held_reconnect:true,observed_by:b.name,disconnected_participant:identity.participant,seat:identity.seat,phase:b.lobby.phase,healthy_start:healthyStart,healthy_end:{...b.observed},bot_takeover_ui:true};
  const restoredStart=a.inputs.length;
  await a.context.setOffline(false);
  await until(()=>a.welcomes.length>welcomes,'trusted WSS reconnect Welcome');
  assert.deepEqual(a.identity,identity,'same public participant/seat identity after private reconnect');
  await until(()=>b.lobby.humans.some(h=>h.id===identity.participant&&h.connected),'healthy peer sees restored human');
  await a.page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').status?.startsWith('Online'));
  await until(()=>a.inputs.length>=restoredStart+3,'three actual post-reconnect packets');
  const restored=a.inputs.slice(restoredStart);
  assert(restored.every(i=>i.horizontal===0&&i.vertical===0&&!i.fire),'held controls neutralized by reconnect before key release');
  evidence.held_input_neutralization={packets:restored.length,all_neutral:true};
  await a.page.keyboard.up('d');await a.page.keyboard.up('Space');
  await expand(a);await screenshot(a,'reclaimed-identity');outage=false;
  evidence.identity_before_disconnect=identity;evidence.identity_after_reconnect=a.identity;
  evidence.checks.push('real runtime WebSocket close; offline context holds retries; other human sees bot takeover and continuing ticks','same participant/seat Welcome on trusted WSS recovery; held-input neutralization before key release');
  const recoveredTick=b.last.tick;
  await until(()=>[...a.hashes.keys()].filter(t=>t>recoveredTick&&b.hashes.has(t)&&native.hashes.has(t)).length>=8,'post-reconnect common browser/browser/native snapshots');
  const common=[...a.hashes.keys()].filter(t=>b.hashes.has(t)&&native.hashes.has(t)).sort((x,y)=>x-y);
  evidence.post_reconnect_common_snapshots={after_tick:recoveredTick,count:common.filter(t=>t>recoveredTick).length};
  assert(common.length>=32,'at least 32 common browser/browser/native ticks');
  for(const tick of common) {assert.equal(a.hashes.get(tick),b.hashes.get(tick),`WASM equality at tick ${tick}`);assert.equal(a.hashes.get(tick),native.hashes.get(tick),`native f32 equality at tick ${tick}`);}
  assert(common.at(-1)-common[0]>=128,'shared equality spans meaningful ticks');
  evidence.common_snapshots={count:common.length,tick_range:[common[0],common.at(-1)],canonicalization:'sorted JSON keys; noninteger Rust f32 values via Math.fround; integer IDs/ticks/ack preserved',samples:common.filter((_,i)=>i<8||i>=common.length-8).map(tick=>({tick,sha256:a.hashes.get(tick)}))};
  assert(native.rtts.length>0,'native acknowledgement RTT samples required');
  evidence.native_rtt_ms=native.rtts;
  assert(evidence.socket_handshakes.length>=3,'initial and reconnect browser handshakes captured');
  assert(evidence.socket_handshakes.every(h=>h.url===backend&&h.origin===origin),'real browser WSS handshake URL/Origin');
  evidence.checks.push('identical f32-canonical authoritative snapshots at common ticks across all three clients','native real acknowledgement RTT samples; real browser HTTPS/WSS backend and Origin');
  await leave(a,[b,native]);await leave(b,[native]);
  assert.equal(evidence.errors.length,0,'unexpected console/page/socket/protocol/native errors; see scrubbed evidence');
  evidence.status='passed';
} catch(error) {
  evidence.status='failed';evidence.failure=String(error.message??error);
  for(const c of clients)if(c.page&&!c.page.isClosed())try{await screenshot(c,c.name+'-failure');}catch{fault(c.name,'failure screenshot unavailable');}
} finally {
  stopping=true;outage=false;
  // Explicit Leave even after partial failure. No leave/reconnect credential values are persisted.
  for(const c of clients.filter(c=>c.page&&c.identity&&!c.left)) {
    try {
      await c.page.keyboard.up('d');await c.page.keyboard.up('Space');
      const observers=clients.filter(other=>other!==c&&!other.left&&other.lobby&&(!other.page||!other.page.isClosed()));
      await leave(c,observers);
    }catch{evidence.cleanup.rendered.push({client:c.name,explicit_leave_verified:false});evidence.status='failed';}
  }
  if(probe) {
    probeStopped=true;
    if(probe.exitCode===null&&probe.signalCode===null)probe.kill('SIGTERM');
    try{await Promise.race([probeDone,pause(5000).then(()=>{if(probe.exitCode===null&&probe.signalCode===null)probe.kill('SIGKILL');})]);await Promise.race([probeDone,pause(2000)]);}catch{evidence.status='failed';}
    evidence.cleanup.native={controlled_termination:probeStopped,closed:probe.exitCode!==null||probe.signalCode!==null,explicit_leave:false,server_side_removal_verified:false,reservation_note:'Transport closure reserves the native seat; last-human empty-room cleanup expires it after 60 seconds. Local process termination is not proof of server-side expiry.'};
    if(!evidence.cleanup.native.closed)evidence.status='failed';
  }
  try{await browser?.close();}catch{evidence.status='failed';}
  evidence.native_stderr={bytes:stderrBytes,sha256:stderrHash.digest('hex'),raw_text_excluded:true};
  evidence.clients=clients.map(c=>({name:c.name,identity:c.identity,participant:c.participant,seat:c.seat,dom:c.dom,frames:c.frames,first:c.first,last:c.observed,welcomes:c.welcomes,events:c.events,input_count:c.inputs.length,last_inputs:c.inputs.slice(-8),last_lobby:c.lobby}));
  if(evidence.errors.length)evidence.status='failed';
  evidence.finished_at=new Date().toISOString();
  const safe=redact(evidence,secrets),serialized=JSON.stringify(safe,null,2);
  const leaked=[...secrets].some(secret=>serialized.includes(secret));
  // Never persist or print unsafe evidence, even if the scrubber itself regresses.
  if(leaked) {
    fs.writeFileSync(path.join(out,'results.json'),JSON.stringify({status:'failed',failure:'private material negative scan failed; unsafe evidence withheld'}));
    console.error('Evidence withheld: private material scan failed.');process.exitCode=1;
  } else {
    safe.private_material_negative_scan={passed:true,known_values_checked:secrets.size};
    fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(safe,null,2)+'\n');
    console.log(JSON.stringify({status:safe.status,evidence_directory:out,results:path.join(out,'results.json')}));
    if(safe.status!=='passed')process.exitCode=1;
  }
  process.removeListener('SIGINT',interrupt);process.removeListener('SIGTERM',interrupt);
}
