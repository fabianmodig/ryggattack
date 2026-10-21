// Trusted-public, ONE disposable room, ONE real rendered human. No app/server changes.
// PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json node scripts/solo-bots-smoke.mjs --live
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
import {redact} from './live-deployment-helpers.mjs';

const usage='Usage: node scripts/solo-bots-smoke.mjs --live\nExplicitly authorizes ONE disposable private room at https://ryggattack.modig.online. No network/browser actions without --live.';
const args=process.argv.slice(2);
if(args.length===1 && ['--help','-h'].includes(args[0])) {console.log(usage);process.exit(0);}
if(args.length!==1 || args[0]!=='--live') {console.error(usage);process.exit(2);}
assert.notEqual(process.env.NODE_TLS_REJECT_UNAUTHORIZED,'0','TLS verification must remain enabled');
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const origin='https://ryggattack.modig.online',backend='wss://ryggattack.modig.online/ws';
const out=path.join(repo,'verification/solo-bots',new Date().toISOString().replace(/[:.]/g,'-')+'-'+crypto.randomBytes(3).toString('hex'));
fs.mkdirSync(path.dirname(out),{recursive:true});fs.mkdirSync(out);
const sha=x=>crypto.createHash('sha256').update(x).digest('hex');
const start=performance.now(), elapsed=()=>Math.round(performance.now()-start);
const secrets=new Set(),sleep=ms=>new Promise(r=>setTimeout(r,ms));
const evidence={status:'running',started_at:new Date().toISOString(),origin,backend,source_sha256:sha(fs.readFileSync(fileURLToPath(import.meta.url))),scope:'one human, three bots, one disposable private room; no restarts/admin/state injection',assets:{},timings_ms:{},protocol:[],lobbies:[],state_summary:{frames:0,round_finished_frames:0,elimination_frames:0,max_dead_active:0},inputs:{count:0,left_fire_count:0},errors:[],console:[],screenshots:[],cleanup:{leave_packet:false,left_room_ui:false}};
let browser,page,identity,lobby,last,winner,leaveSent=false,interrupted=false;
const interrupt=()=>{interrupted=true;};
process.on('SIGTERM',interrupt);process.on('SIGINT',interrupt);
async function until(fn,label,ms=90000,cleanup=false) {
  const end=performance.now()+ms;
  while(!fn()) {assert(cleanup||!interrupted,'interrupted');assert(performance.now()<end,label);await sleep(25);}
}
function fault(source,message) {evidence.errors.push({source,message:String(message),at_ms:elapsed()});}
function safeLobby(l) {
  return {id:l.id,code:l.code,host:l.host,settings:l.settings,phase:l.phase,humans:l.humans.map(h=>({id:h.id,name:h.name,seat:h.seat,ready:h.ready,connected:h.connected}))};
}
async function expand() {if(!await page.locator('#online-entry').evaluate(e=>e.open))await page.locator('#online-entry summary').click();}
async function screenshot(name) {
  const before=elapsed();await page.screenshot({path:path.join(out,name+'.png'),timeout:60000});
  evidence.screenshots.push({file:name+'.png',started_ms:before,duration_ms:elapsed()-before,observed_phase:lobby?.phase,sha256:sha(fs.readFileSync(path.join(out,name+'.png')))});
}
try {
  const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
  // Hash exactly the trusted public artifacts, without TLS bypass or host mapping.
  for(const [file,mime] of [['index.html',/^text\/html\b/],['rooms.mjs',/^(text|application)\/javascript\b/],['ryggattack.js',/^(text|application)\/javascript\b/],['ryggattack_bg.wasm',/^application\/wasm\b/]]) {
    const r=await fetch(`${origin}/${file}`,{redirect:'error',signal:AbortSignal.timeout(30000)});
    const b=Buffer.from(await r.arrayBuffer());
    evidence.assets[file]={status:r.status,mime:r.headers.get('content-type'),bytes:b.length,sha256:sha(b)};
    assert.equal(r.status,200);assert(mime.test(r.headers.get('content-type')??''));assert(b.length);
    if(file.endsWith('.wasm'))assert(b.subarray(0,4).equals(Buffer.from([0,97,115,109])));
  }
  evidence.timings_ms.assets=elapsed();
  browser=await chromium.launch({args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding']});
  const context=await browser.newContext({viewport:{width:640,height:480},ignoreHTTPSErrors:false});
  page=await context.newPage();page.setDefaultTimeout(60000);page.setDefaultNavigationTimeout(45000);
  evidence.browser_version=browser.version();
  const session=await context.newCDPSession(page),urls=new Map();
  await session.send('Network.enable');
  session.on('Network.webSocketCreated',e=>urls.set(e.requestId,e.url));
  session.on('Network.webSocketWillSendHandshakeRequest',e=>{
    const observedOrigin=Object.entries(e.request.headers).find(([k])=>k.toLowerCase()==='origin')?.[1];
    evidence.protocol.push({kind:'handshake',url:urls.get(e.requestId),origin:observedOrigin,at_ms:elapsed()});
  });
  page.on('crash',()=>fault('page','renderer crash'));
  page.on('pageerror',e=>fault('page',e.message));
  page.on('console',m=>{if(['warning','error'].includes(m.type()))evidence.console.push({type:m.type(),text:m.text()});if(m.type()==='error')fault('console',m.text());});
  page.on('requestfailed',r=>fault('request',`${r.url()} ${r.failure()?.errorText}`));
  page.on('response',r=>{if(r.status()>=400)fault('http',`${r.status()} ${r.url()}`);});
  page.on('websocket',ws=>{
    if(ws.url()!==backend)fault('socket','unexpected URL');
    ws.on('socketerror',e=>fault('socket',e));
    ws.on('framereceived',({payload})=>{
      try {
        const m=JSON.parse(String(payload));redact(m,secrets); // Discover secrets, never retain raw Welcome.
        if(m.Welcome) {
          identity=Object.fromEntries(['version','seat','room','participant','code'].map(k=>[k,m.Welcome[k]]));
          evidence.identity=identity;evidence.protocol.push({kind:'Welcome',...identity,at_ms:elapsed()});
        }
        if(m.Lobby) {
          lobby=safeLobby(m.Lobby);evidence.lobbies.push({...lobby,at_ms:elapsed()});
          if(lobby.humans.length!==1)fault('scope','expected exactly ONE human');
          if(typeof lobby.phase==='object'&&lobby.phase.Countdown)evidence.timings_ms.countdown??=elapsed();
          if(lobby.phase==='Running')evidence.timings_ms.running??=elapsed();
        }
        if(m.State) {
          last=m.State;const s=evidence.state_summary;s.frames++;
          s.first_tick??=last.tick;s.last_tick=last.tick;s.last_simulation_tick=last.simulation_tick;
          const active=last.carts.filter(c=>c.active),dead=active.filter(c=>!c.alive).length;
          if(active.length!==4)fault('state','expected FOUR active carts');
          if(dead)s.elimination_frames++;s.max_dead_active=Math.max(s.max_dead_active,dead);
          if(last.round_finished)s.round_finished_frames++;
          if(!evidence.first_state)evidence.first_state=last;
          if(last.winner!==null&&last.winner!==undefined&&!winner) {
            winner=last;evidence.winner_state=last;evidence.timings_ms.winner=elapsed();
          }
        }
        if(m.Error||m.Notice)fault('protocol',m.Error??m.Notice);
      }catch {fault('protocol','invalid received frame');}
    });
    ws.on('framesent',({payload})=>{
      try {
        const m=JSON.parse(String(payload));redact(m,secrets);
        const kind=typeof m==='string'?m:Object.keys(m)[0];
        if(!['Hello','Ready','Configure','Input','Leave'].includes(kind))fault('protocol','unexpected command (reconnect not part of this gate)');
        if(m.Hello) {
          assert.equal(m.Hello.name,'AcceptanceSolo');assert.equal(m.Hello.room??null,null);
          evidence.protocol.push({kind:'Hello',name:m.Hello.name,at_ms:elapsed()});
        }
        if(m.Configure||m.Ready!==undefined)evidence.protocol.push({kind,value:m[kind],at_ms:elapsed()});
        if(m.Input) {evidence.inputs.count++;if(m.Input.horizontal===-1&&m.Input.fire)evidence.inputs.left_fire_count++;}
        if(kind==='Leave') {leaveSent=true;evidence.protocol.push({kind:'Leave',at_ms:elapsed()});}
      }catch {fault('protocol','invalid sent frame or scope');}
    });
  });
  await page.goto(`${origin}/?quality=0`,{waitUntil:'domcontentloaded'});
  await page.waitForFunction(expected=>document.getElementById('backend')?.value===expected,backend);
  await expand();await page.locator('#player-name').fill('AcceptanceSolo');await page.locator('#create-room').click();
  await until(()=>identity&&lobby,'real form admission');
  evidence.timings_ms.admitted=elapsed();evidence.created_lobby=lobby;
  assert.equal(identity.room,lobby.id);assert.equal(lobby.humans.length,1);
  await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').lobby);
  await page.locator('#bots-room').fill('3');await page.locator('#target-room').selectOption('1');await page.locator('#apply-room').click();
  await until(()=>lobby.settings.bots===3&&lobby.settings.wins_target===1,'actual bots=3 wins=1 configuration');
  const settings={...lobby.settings};evidence.configured_lobby=lobby;
  await page.locator('#ready-room').click();
  await until(()=>evidence.timings_ms.countdown!==undefined&&evidence.timings_ms.running!==undefined&&last,'Ready -> Countdown -> Running');
  const completionStart=performance.now();evidence.timings_ms.completion_budget_started=elapsed();
  if(await page.locator('#online-entry').evaluate(e=>e.open))await page.locator('#online-entry summary').click();
  await page.locator('canvas').focus();await page.keyboard.down('a');await page.keyboard.down('Space');
  // Ordinary keyboard input only; bot or human victory both satisfy acceptance.
  await screenshot('actual-game');
  await until(()=>winner&&lobby.phase==='Lobby','natural winner and lobby return',Math.max(1,180000-(performance.now()-completionStart)));
  evidence.timings_ms.natural_completion=elapsed();evidence.timings_ms.completion_duration=Math.round(performance.now()-completionStart);
  await page.keyboard.up('a');await page.keyboard.up('Space');
  assert.equal(lobby.id,identity.room);assert.equal(lobby.code,identity.code);
  assert.equal(lobby.humans.length,1);assert.equal(lobby.humans[0].id,identity.participant);
  assert(lobby.humans.every(h=>!h.ready&&h.connected));assert.deepEqual(lobby.settings,settings);
  assert.equal(winner.carts.filter(c=>c.active).length,4);assert(winner.round_finished);
  assert(winner.wins[winner.winner]>=1);assert(evidence.state_summary.elimination_frames>0);assert(evidence.state_summary.round_finished_frames>0);
  evidence.returned_lobby=lobby;
  await expand();await page.waitForFunction(()=>document.getElementById('room-info').textContent.includes('· Lobby')&&document.getElementById('ready-room').textContent==='Ready');
  await screenshot('returned-lobby');
  const handshakes=evidence.protocol.filter(e=>e.kind==='handshake');
  assert.equal(handshakes.length,1);assert(handshakes.every(e=>e.url===backend&&e.origin===origin));
  assert.equal(evidence.errors.length,0,'unexpected browser/protocol errors');
  evidence.status='passed';
} catch(e) {
  evidence.status='failed';evidence.failure=String(e.message??e);
  if(page&&!page.isClosed())try{await screenshot('failure');}catch{fault('evidence','failure screenshot unavailable');}
} finally {
  if(page&&identity&&!page.isClosed()) {
    try {
      await page.keyboard.up('a');await page.keyboard.up('Space');await expand();await page.locator('#leave-room').click();
      await until(()=>leaveSent,'own real Leave packet',10000,true);evidence.cleanup.leave_packet=true;
      await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').status?.includes('Left room'),null,{timeout:10000});
      evidence.cleanup.left_room_ui=true;evidence.cleanup.at_ms=elapsed();
    }catch {fault('cleanup','own Leave/UI verification failed');evidence.status='failed';}
  }
  if(browser)try{await browser.close();evidence.cleanup.browser_closed=true;}catch{evidence.status='failed';}
  if(evidence.errors.length)evidence.status='failed';
  evidence.finished_at=new Date().toISOString();evidence.timings_ms.total=elapsed();
  const safe=redact(evidence,secrets),text=JSON.stringify(safe,null,2);
  if([...secrets].some(s=>text.includes(s))) {
    fs.writeFileSync(path.join(out,'results.json'),JSON.stringify({status:'failed',failure:'secret scan failed; evidence withheld'}));process.exitCode=1;
  } else {
    safe.private_material_negative_scan={passed:true,known_values_checked:secrets.size};
    fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(safe,null,2)+'\n');
    const files=fs.readdirSync(out).map(file=>({file,sha256:sha(fs.readFileSync(path.join(out,file)))}));
    fs.writeFileSync(path.join(out,'sha256.json'),JSON.stringify(files,null,2)+'\n');
    console.log(JSON.stringify({status:safe.status,evidence_directory:out,timings_ms:safe.timings_ms,failure:safe.failure}));
    if(safe.status!=='passed')process.exitCode=1;
  }
  process.removeListener('SIGTERM',interrupt);process.removeListener('SIGINT',interrupt);
}
