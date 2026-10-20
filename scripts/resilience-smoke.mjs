// Two real rendered WASM clients; TCP outages, never synthetic game state.
// PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json node scripts/resilience-smoke.mjs [out]
import assert from 'node:assert/strict';
import {roomStatusHas} from './resilience-helpers.mjs';
import http from 'node:http';
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawn} from 'node:child_process';
import {createRequire} from 'node:module';
const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
const root=path.resolve('dist/web'),out=path.resolve(process.argv[2]??'verification/resilience-browser');fs.mkdirSync(out,{recursive:true});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const until=async(p,label,ms=90000)=>{const end=performance.now()+ms;while(!p()){assert(performance.now()<end,label);await sleep(25);}};
const listen=s=>new Promise(r=>s.listen(0,'127.0.0.1',r));
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const secrets=new Set(),clients=[],gates=[],nodeSockets=[];
const evidence={artifact:Object.fromEntries(['index.html','rooms.mjs','ryggattack.js','ryggattack_bg.wasm'].map(f=>[f,sha(fs.readFileSync(path.join(root,f)))])),checks:[],errors:[],transport_console:[],server_events:[]};
let browser,server,port,serverLog='',outage=false;
const staticServer=http.createServer((req,res)=>{let file;try{let p=decodeURIComponent(req.url.split('?')[0]);if(p.includes('\0'))throw Error();if(p.endsWith('/'))p+='index.html';file=path.resolve(root,'.'+p);}catch{res.writeHead(400).end();return;}if(!file.startsWith(root+path.sep)){res.writeHead(403).end();return;}fs.readFile(file,(e,d)=>{if(e){res.writeHead(404).end();return;}res.writeHead(200,{'content-type':({'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.html':'text/html'})[path.extname(file)]??'application/octet-stream'}).end(d);});});
async function startServer(bind=0){
 server=spawn(path.resolve('target/debug/ryggattack-server'),[`127.0.0.1:${bind}`],{env:{...process.env,RYGGATTACK_ALLOWED_ORIGINS:`http://127.0.0.1:${staticServer.address().port}`}});
 server.stderr.on('data',b=>{serverLog+=b;evidence.server_events.push({at:performance.now(),text:String(b)});});
 port=await new Promise((r,j)=>{let text='';server.stdout.on('data',b=>{text+=b;const m=text.match(/ws:\/\/127\.0\.0\.1:(\d+)/);if(m)r(+m[1]);});server.once('error',j);setTimeout(()=>j(Error('server startup')),10000).unref();});
}
async function gate(){
 const g={offline:false,sockets:new Set()};
 g.listener=net.createServer(down=>{if(g.offline){down.destroy();return;}const up=net.connect(port,'127.0.0.1');for(const s of [down,up]){g.sockets.add(s);s.setNoDelay(true);s.on('error',()=>{down.destroy();up.destroy();});s.on('close',()=>g.sockets.delete(s));}down.pipe(up);up.pipe(down);});
 await listen(g.listener);g.address=`ws://127.0.0.1:${g.listener.address().port}`;g.cut=()=>{g.offline=true;for(const s of g.sockets)s.destroy();};g.restore=()=>g.offline=false;gates.push(g);return g;
}
function publicIdentity(w){secrets.add(w.credential);const {credential,...safe}=w;return safe;}
async function pageFor(name,g,code){
 const page=await (await browser.newContext({viewport:{width:640,height:480}})).newPage();
 const c={name,page,g,welcomes:[],frames:[],inputs:[],handshakes:[],statuses:[]};clients.push(c);
 page.on('pageerror',e=>evidence.errors.push(`${name}: ${e.message}`));
 page.on('console',m=>{if(m.type()==='error'){const text=m.text();if(outage&&text.includes('WebSocket'))evidence.transport_console.push(text);else evidence.errors.push(`${name}: ${text}`);}});
 page.on('websocket',ws=>{
  ws.on('framereceived',({payload})=>{const m=JSON.parse(payload);if(m.Welcome){c.identity=publicIdentity(m.Welcome);c.welcomes.push({identity:c.identity,at:performance.now()});}if(m.Lobby)c.lobby=m.Lobby;if(m.State){c.last=m.State;c.frames.push({frame:m.State,at:performance.now()});}if(m.Error)c.error=m.Error;});
  ws.on('framesent',({payload})=>{const m=JSON.parse(payload);if(m.Input)c.inputs.push(m.Input);if(m.Hello)c.handshakes.push({kind:'Hello',room:m.Hello.room});if(m.Reconnect)c.handshakes.push({kind:'Reconnect',room:m.Reconnect.room,participant:m.Reconnect.participant});});
 });
 await page.goto(`http://127.0.0.1:${staticServer.address().port}/?quality=0&online=${encodeURIComponent(g.address)}&name=${name}${code?'&room='+code:''}`);
 await until(()=>c.identity&&c.lobby,`${name} admitted`);
 await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').lobby,null,{timeout:90000});
 return c;
}
async function expand(c){if(!await c.page.locator('#online-entry').evaluate(e=>e.open))await c.page.locator('#online-entry summary').click();}
async function rendered(c,label){
 await c.page.waitForFunction(()=>{const [seat,tick]=(globalThis.ryggattackOnline??'99|0').split('|').map(Number);return seat<4&&tick>0&&JSON.parse(globalThis.ryggattackRoom??'{}').status?.startsWith('Online');},null,{timeout:90000});
 const presented=await c.page.evaluate(()=>globalThis.ryggattackOnline);
 await c.page.waitForFunction(previous=>globalThis.ryggattackOnline!==previous,presented,{timeout:90000});
 assert.equal(await c.page.locator('canvas').count(),1);
 const p=path.join(out,label+'.png');await c.page.screenshot({path:p,timeout:180000});return sha(fs.readFileSync(p));
}
async function rawJoin(code=null){
 const ws=new WebSocket(`ws://127.0.0.1:${port}`);nodeSockets.push(ws);let identity,lobby;
 ws.addEventListener('open',()=>ws.send(JSON.stringify({Hello:{version:3,name:'Capacity',room:code}})));
 ws.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Welcome)identity=publicIdentity(m.Welcome);if(m.Lobby)lobby=m.Lobby;});
 await until(()=>identity&&lobby,'capacity fixture admits');return {ws,identity,lobby};
}
try{
 await listen(staticServer);await startServer();
 browser=await chromium.launch({args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding']});
 const a=await pageFor('ResilienceA',await gate());const code=a.lobby.code;
 const b=await pageFor('ResilienceB',await gate(),code);
 const original={a:a.identity,b:b.identity};
 await expand(a);await a.page.locator('#bots-room').fill('0');await a.page.locator('#target-room').selectOption('7');await a.page.locator('#apply-room').click();await until(()=>a.lobby.settings.bots===0,'two humans only');
 await a.page.locator('#ready-room').click();await expand(b);await b.page.locator('#ready-room').click();
 await until(()=>a.last&&b.last,'both running');
 const beforeRender=await rendered(a,'before-disconnect');
 await a.page.locator('canvas').focus();
 await a.page.keyboard.down('d');await a.page.keyboard.down('Space');
 const heldBefore=a.inputs.length;
 await until(()=>a.inputs.slice(heldBefore).some(i=>i.horizontal===1&&i.fire),'real held steering/fire before outage');
 outage=true;a.g.cut();
 await until(()=>b.lobby.humans.some(h=>h.id===original.a.participant&&!h.connected),'real bot takeover');
 await b.page.waitForFunction(()=>document.getElementById('room-info').textContent.includes('Bot takeover'),null,{timeout:90000});
 const botStart=b.last;await sleep(2200);const botEnd=b.last;
 assert(botEnd.simulation_tick>botStart.simulation_tick+64,'other human keeps real simulation running');
 assert.equal(b.lobby.host,original.b.participant);
 evidence.bot_takeover={human:original.a.participant,seat:original.a.seat,simulation_ticks:botEnd.simulation_tick-botStart.simulation_tick,reservation:b.lobby.humans.find(h=>h.id===original.a.participant).reconnect_seconds};
 await b.page.screenshot({path:path.join(out,'bot-reservation.png'),timeout:180000});
 a.g.restore();
 await until(()=>a.welcomes.length>=2,'automatic reconnect within reservation');
 assert.equal(a.identity.participant,original.a.participant);assert.equal(a.identity.seat,original.a.seat);assert.equal(a.identity.room,original.a.room);
 await until(()=>a.lobby.host===original.b.participant&&a.lobby.humans.every(h=>h.connected),'former host returns ordinary');
 const afterRender=await rendered(a,'reconnected-same-seat');assert.notEqual(beforeRender,afterRender);
 const heldAfter=a.inputs.length;
 await until(()=>a.inputs.length>=heldAfter+3,'post-reconnect held-key packets observed');
 assert(a.inputs.slice(heldAfter).every(i=>i.horizontal===0&&i.vertical===0&&!i.fire),'held steering/fire remains neutral after reconnect until key release');
 evidence.held_input_neutralization={packets:a.inputs.length-heldAfter,held_steering_and_fire:true,all_neutral:true};
 await a.page.keyboard.up('d');await a.page.keyboard.up('Space');
 // At software-rendering speed a timed sleep need not contain an input sample.
 // Observe two actual rendered updates with released keys before a new press.
 for(let i=0;i<2;i++){const released=await a.page.evaluate(()=>globalThis.ryggattackOnline);await a.page.waitForFunction(previous=>globalThis.ryggattackOnline!==previous,released,{timeout:90000});}
 const inputStart=a.inputs.length;await a.page.locator('canvas').focus();await a.page.keyboard.down('d');
 await until(()=>a.inputs.slice(inputStart).some(i=>i.horizontal===1),'rendered client accepts fresh keyboard after reconnect');
 await a.page.keyboard.up('d');
 assert(a.inputs.slice(inputStart).some(i=>i.horizontal===1),'rendered client accepts fresh keyboard after reconnect');
 const restoredAt=a.welcomes.at(-1).at;
 const bFrames=new Map(b.frames.map(f=>[f.frame.tick,f.frame]));
 const common=a.frames.filter(f=>f.at>=restoredAt&&bFrames.has(f.frame.tick));
 assert(common.length>=4,'both rendered transports receive common post-reconnect ticks');
 for(const f of common)assert.equal(sha(JSON.stringify(f.frame)),sha(JSON.stringify(bFrames.get(f.frame.tick))),'same authoritative state after reconnect');
 evidence.reconnect_character={participant:a.identity.participant,seat:a.identity.seat,while_bot:botEnd.carts[a.identity.seat],first_restored:common[0].frame.carts[a.identity.seat],shared_identical_ticks:common.length};
 outage=false;
 evidence.checks.push('actual two WASM clients; TCP cut -> bot takeover and visible reservation -> same participant/seat, ordinary former host; rendered loop and real input resume');
 // Both real transports drop together. First post-return simulation clock may
 // advance a few ticks to the 16Hz snapshot boundary, never the whole wall gap.
 const before=a.last,cutAt=performance.now(),welcomeCount=a.welcomes.length;
 outage=true;a.g.cut();b.g.cut();await sleep(3200);a.g.restore();
 await until(()=>a.welcomes.length>welcomeCount,'empty paused room resumes');
 const welcomeAt=a.welcomes.at(-1).at;
 await until(()=>a.frames.some(f=>f.at>=welcomeAt),'post-resume frame');
 const first=a.frames.find(f=>f.at>=welcomeAt).frame;
 assert(first.simulation_tick>=before.simulation_tick&&first.simulation_tick-before.simulation_tick<=32,'all-offline simulation clock froze');
 evidence.empty_pause={wall_ms:welcomeAt-cutAt,before,first_after:first,simulation_delta:first.simulation_tick-before.simulation_tick};
 b.g.restore();await until(()=>b.welcomes.length>=2&&a.lobby.humans.every(h=>h.connected),'both resume');
 await rendered(a,'empty-pause-resumed');outage=false;
 evidence.checks.push('last-human transport loss freezes simulation across real wall gap; eligible credential resumes same room and character');
 // Two rendered clients above establish concurrent recovery. Close the second
 // SwiftShader renderer before repeated full-page starts: otherwise shader
 // startup can consume the real 60-second reservation while testing storage.
 await b.page.close();b.page=null;
 // Session reload exercises private storage, not a new public invitation join.
 const reloadId=a.identity.participant,reloadWelcomes=a.welcomes.length;
 await a.page.reload();await until(()=>a.welcomes.length>reloadWelcomes,'saved credential reload');assert.equal(a.identity.participant,reloadId);
 await rendered(a,'saved-seat-reload');
 const storage=await a.page.evaluate(()=>({keys:Object.keys(sessionStorage).filter(k=>k.startsWith('ryggattack.claim.')),url:location.href,publicState:globalThis.ryggattackRoom}));
 assert.equal(storage.keys.length,1);for(const token of secrets){assert(!storage.url.includes(token));assert(!storage.publicState.includes(token));}
 evidence.storage={claim_keys:storage.keys.length,private_url_and_public_state_scan:true};
 evidence.checks.push('saved seat reload; no credential in URL/public UI state');
 // The same tab must not send a saved claim to another endpoint or room.
 const scopedBase=`http://127.0.0.1:${staticServer.address().port}/?quality=0`,scopeStarted=performance.now();
 evidence.storage.scope_steps=[];
 for(const [address,invite,error] of [[b.g.address,code,'match running'],[a.g.address,'0'.repeat(20),'room not found']]){
  const handshakesBefore=a.handshakes.length;
  await a.page.goto(`${scopedBase}&online=${encodeURIComponent(address)}&room=${invite}`);
  await a.page.waitForFunction(roomStatusHas,error,{timeout:90000});
  const actual=a.handshakes.slice(handshakesBefore);
  assert(actual.length>0&&actual.every(m=>m.kind==='Hello'),'unrelated endpoint/room must not receive a private reconnect claim');
  const originalClaimRetained=await a.page.evaluate(key=>Boolean(sessionStorage.getItem(key)),storage.keys[0]);
  assert(originalClaimRetained,'unrelated admission error removed original scoped claim');
  evidence.storage.scope_steps.push({address,invite,error,elapsed_ms:performance.now()-scopeStarted,original_claim_retained:true,handshakes:actual});
 }
 const scopeWelcomes=a.welcomes.length;
 await a.page.goto(`${scopedBase}&online=${encodeURIComponent(a.g.address)}&room=${code}`);
 await until(()=>a.welcomes.length>scopeWelcomes,'original scoped claim survives unrelated endpoint/room errors');
 assert.equal(a.identity.participant,original.a.participant);
 await rendered(a,'scope-restored-original-seat');
 evidence.storage.endpoint_and_room_isolation=true;
 evidence.checks.push('same browser tab never sends another endpoint/room claim; original private identity still reclaims');
 // A real process restart destroys all memory, so retry fails visibly, never
 // silently creates a new room and never advertises persistence.
 outage=true;const fixedPort=port;const dead=server;dead.kill();await new Promise(r=>dead.once('exit',r));await startServer(fixedPort);
 await until(()=>a.error?.includes('backend restarted'),'restart clear terminal feedback');
 await a.page.waitForFunction(roomStatusHas,'backend restarted',{timeout:90000});
 await a.page.screenshot({path:path.join(out,'backend-restarted.png'),timeout:180000});
 evidence.restart={a:a.error,second_renderer_closed_after_concurrent_recovery:true};
 assert.equal(await a.page.evaluate(()=>Object.keys(sessionStorage).filter(k=>k.startsWith('ryggattack.claim.')).length),0,'failed old claim cleared');
 evidence.checks.push('real backend process restart: terminal expired/restarted feedback, no fake persistence, saved claim cleared');
 // Only one renderer remains for terminal admission errors.
 const base=`http://127.0.0.1:${staticServer.address().port}/?quality=0&online=${encodeURIComponent(a.g.address)}`;
 await a.page.goto(base+'&room='+'0'.repeat(20));
 await a.page.waitForFunction(roomStatusHas,'room not found',{timeout:90000});
 await a.page.screenshot({path:path.join(out,'missing-invite.png'),timeout:180000});
 // Adversarial old-version wire handshake, not a synthetic server response.
 await a.page.addInitScript(()=>{if(new URLSearchParams(location.search).has('wire-version')){const Original=globalThis.WebSocket;globalThis.WebSocket=class extends Original{send(data){let m;try{m=JSON.parse(data);}catch{}if(m?.Hello){m.Hello.version=2;data=JSON.stringify(m);}return super.send(data);}};}});
 await a.page.goto(base+'&wire-version=2');
 await a.page.waitForFunction(roomStatusHas,'incompatible',{timeout:90000});
 await a.page.screenshot({path:path.join(out,'incompatible-client.png'),timeout:180000});
 evidence.checks.push('real incompatible-version WebSocket handshake is shown as a terminal error in rendered UI');
 const cap=await rawJoin();for(let i=0;i<3;i++)await rawJoin(cap.lobby.code);
 await a.page.goto(base+'&room='+cap.lobby.code);
 await a.page.waitForFunction(roomStatusHas,'room full',{timeout:90000});
 await a.page.screenshot({path:path.join(out,'room-full.png'),timeout:180000});
 const freshWelcomeCount=a.welcomes.length;
 await a.page.goto(base);
 await until(()=>a.welcomes.length>freshWelcomeCount&&a.identity.code!==cap.lobby.code,'fresh room create');
 await a.page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').lobby?.humans.some(h=>h.name==='Player'),null,{timeout:90000});
 await expand(a);await a.page.locator('#leave-room').click();
 await a.page.waitForFunction(roomStatusHas,'Left room',{timeout:90000});
 assert.equal(await a.page.evaluate(()=>Object.keys(sessionStorage).filter(k=>k.startsWith('ryggattack.claim.')).length),0);
 const count=a.welcomes.length;await sleep(2000);assert.equal(a.welcomes.length,count,'deliberate leave disables retry');
 a.g.cut();await a.page.goto(base);
 await a.page.waitForFunction(roomStatusHas,'unavailable',{timeout:90000});
 await a.page.screenshot({path:path.join(out,'backend-unavailable.png'),timeout:180000});
 evidence.checks.push('actual missing invite, four-seat full rejection, unavailable backend visible; deliberate Leave clears session claim and disables retries');
 assert.equal(evidence.errors.length,0,JSON.stringify(evidence.errors));
}catch(e){evidence.failure=String(e.stack??e);evidence.failure_clients=[];for(const c of clients)if(c.page)try{evidence.failure_clients.push({name:c.name,input_tail:c.inputs.slice(-12),handshakes:c.handshakes,dom:await c.page.evaluate(()=>({active:document.activeElement?.id,focused:document.hasFocus(),state:globalThis.ryggattackRoom,presented:globalThis.ryggattackOnline}))});await c.page.screenshot({path:path.join(out,c.name+'-failure.png'),timeout:30000});}catch{}throw e;
}finally{
 evidence.server_log=serverLog;evidence.server_overruns=serverLog.split('tick overrun').length-1;
 evidence.clients=clients.map(c=>({name:c.name,welcomes:c.welcomes,latest_lobby:c.lobby,latest_frame:c.last,frames:c.frames.length,inputs:c.inputs.length}));
 const text=JSON.stringify(evidence,null,2);assert([...secrets].every(t=>!text.includes(t)),'credential leaked into evidence');
 evidence.credential_negative_scan=true;fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(evidence,null,2));
 console.log(JSON.stringify({checks:evidence.checks,error_count:evidence.errors.length,expected_transport_errors:evidence.transport_console.length,overruns:evidence.server_overruns,failure:evidence.failure},null,2));
 for(const s of nodeSockets)s.close();if(browser)await browser.close();for(const g of gates){for(const s of g.sockets)s.destroy();g.listener.close();}if(server)server.kill();staticServer.close();
}
