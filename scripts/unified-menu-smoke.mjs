// Isolated real-rendered acceptance for the integrated menu. No public endpoint.
import assert from 'node:assert/strict';
import fs from 'node:fs';import path from 'node:path';import http from 'node:http';import crypto from 'node:crypto';
import {spawn,execFileSync} from 'node:child_process';import {createRequire} from 'node:module';
import {openRoomMenu} from './menu-harness.mjs';
const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
const root=path.resolve('dist/web'),out=path.resolve(process.argv[2]??'verification/unified-menu');fs.mkdirSync(out,{recursive:true});
for(const file of ['index.html','ryggattack.js','ryggattack_bg.wasm','rooms.mjs'])assert(fs.existsSync(path.join(root,file)),`incomplete web artifact: ${file}`);
const sleep=ms=>new Promise(r=>setTimeout(r,ms)),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const evidence={checks:[],errors:[],expected_errors:[],screenshots:[],artifact:{},source:{},clients:[]};
for(const file of fs.readdirSync(root).filter(f=>/\.(html|mjs|js|wasm)$/.test(f)))evidence.artifact[file]=sha(fs.readFileSync(path.join(root,file)));
evidence.source_head=execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim();
for(const file of execFileSync('git',['ls-files','src','web','assets','Cargo.toml','Cargo.lock','scripts/build-web.sh'],{encoding:'utf8'}).trim().split('\n'))evidence.source[file]=sha(fs.readFileSync(file));
evidence.harness_sha256=sha(fs.readFileSync(new URL(import.meta.url)));
for(const file of ['unified-menu-smoke.mjs','menu-harness.mjs'])fs.copyFileSync(new URL(file,import.meta.url),path.join(out,file+'.txt'));
const browsers=[],clients=[],secrets=new Set();let server,backendLog='',expectedError=false;
const front=http.createServer((req,res)=>{
 let file;try{let p=decodeURIComponent(req.url.split('?')[0]);if(p.includes('\0'))throw Error();if(p.endsWith('/'))p+='index.html';file=path.resolve(root,'.'+p);}catch{res.writeHead(400).end();return;}
 if(!file.startsWith(root+path.sep)){res.writeHead(403).end();return;}
 fs.readFile(file,(e,d)=>{if(e){res.writeHead(404).end();return;}res.writeHead(200,{'content-type':({'.html':'text/html','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm'})[path.extname(file)]??'application/octet-stream'}).end(d);});
});
const until=async(p,label,ms=120000)=>{const end=performance.now()+ms;while(!p()){assert(performance.now()<end,label);await sleep(40);}};
async function shot(c,name){const start=performance.now();try{await c.page.screenshot({path:path.join(out,name+'.png'),timeout:60000,fullPage:true});evidence.screenshots.push(name+'.png');}finally{fs.appendFileSync(path.join(out,'capture-timings.jsonl'),JSON.stringify({name,seconds:(performance.now()-start)/1000})+'\n');}}
async function visible(c,selector){await c.page.locator(selector).waitFor({state:'visible',timeout:120000});}
async function ready(c){await visible(c,'#ready-room');await c.page.waitForFunction(()=>!document.getElementById('ready-room').disabled,null,{timeout:30000,polling:20});await c.page.locator('#ready-room').click({force:true});}
async function hold(c,key,ms=900){
 const before=await c.page.evaluate(()=>globalThis.__testPadPolls??0);await c.page.keyboard.down(key);
 await c.page.waitForFunction(before=>(globalThis.__testPadPolls??0)>before+1,before,{timeout:60000,polling:20});await sleep(ms);await c.page.keyboard.up(key);
 const released=await c.page.evaluate(()=>globalThis.__testPadPolls??0);await c.page.waitForFunction(before=>(globalThis.__testPadPolls??0)>before+1,released,{timeout:60000,polling:20});
}
async function pad(c,index){
 const before=await c.page.evaluate(index=>{if(!globalThis.__testNavWrapped){const original=globalThis.ryggattackNavigate;globalThis.__testNavWrapped=true;globalThis.__testNavEvents=[];globalThis.ryggattackNavigate=input=>{if(Object.entries(input).some(([k,v])=>k!=='controller'&&v))globalThis.__testNavEvents.push({input,active:document.activeElement?.id});return original(input);};}globalThis.__testPadEnabled=true;globalThis.__testPadButtons[index]=true;return globalThis.__testPadPolls??0;},index);
 await c.page.waitForFunction(before=>(globalThis.__testPadPolls??0)>before,before,{timeout:60000}).catch(error=>{throw Error(`pad ${index} was not sampled: ${error.message}`);});
 await sleep(1800);
 const pressed=await c.page.evaluate(index=>{const polls=globalThis.__testPadPolls??0;const state={polls,active:document.activeElement?.id,value:document.activeElement?.value,events:globalThis.__testNavEvents};globalThis.__testPadButtons[index]=false;return state;},index);
 await c.page.waitForFunction(before=>(globalThis.__testPadPolls??0)>before,pressed.polls,{timeout:60000});await sleep(1400);
 evidence.pad_samples??=[];evidence.pad_samples.push({client:c.name,index,before,...pressed});
}
async function running(a,b){
 const active=()=>a.lobby?.phase==='Running'&&b.lobby?.phase==='Running'&&a.last?.winner===null&&b.last?.winner===null;
 if(!active()){
  await until(()=>active()||(a.lobby?.phase==='Lobby'&&b.lobby?.phase==='Lobby'),'active match or same lobby');
  if(!active()){
   for(const c of [a,b])if(!c.lobby.humans.find(h=>h.id===c.identity.participant).ready)await ready(c);
   await until(active,'fresh authoritative running match');
  }
 }
 for(const c of [a,b])await c.page.waitForFunction(()=>{try{const h=JSON.parse(globalThis.ryggattackHud);return h?.lobby?.phase==='Running'&&h?.frame?.winner===null;}catch{return false;}},null,{timeout:120000,polling:20});
}
let origin,address;const rawPeers=[];
async function startBackend(bind='127.0.0.1:0') {
 server=spawn(path.resolve('target/debug/ryggattack-server'),[bind],{env:{...process.env,RYGGATTACK_ALLOWED_ORIGINS:origin},stdio:['ignore','pipe','pipe']});server.stderr.on('data',b=>backendLog+=b);
 return new Promise((r,j)=>{server.stdout.on('data',b=>{const m=String(b).match(/ws:\/\/127\.0\.0\.1:\d+/);if(m)r(m[0]);});server.once('error',j);server.once('exit',code=>j(Error('backend exit '+code)));setTimeout(()=>j(Error('backend startup timeout')),10000).unref();});
}
async function stopBackend(){const child=server;await new Promise((r,j)=>{child.once('exit',code=>code===0?r():j(Error('isolated backend exit '+code)));child.kill('SIGTERM');});server=null;}
async function rawJoin(code=null){
 const version=Number(fs.readFileSync('src/network.rs','utf8').match(/pub const VERSION: u32 = (\d+)/)[1]);
 const ws=new WebSocket(address),peer={ws};rawPeers.push(peer);
 ws.addEventListener('open',()=>ws.send(JSON.stringify({Hello:{version,name:'Capacity',room:code}})));
 ws.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Welcome){secrets.add(m.Welcome.credential);const {credential,...safe}=m.Welcome;peer.identity=safe;}if(m.Lobby)peer.lobby=m.Lobby;});
 await until(()=>peer.identity&&peer.lobby,'real capacity peer');return peer;
}
async function browserClient(name,url=origin+'/?quality=0',viewport={width:960,height:720}){
 const browser=await chromium.launch({args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding']});browsers.push(browser);
 const page=await browser.newPage({viewport}),c={name,page,browser,welcomes:[],lobbies:[],inputs:[],sockets:0,states:0,commands:[],transitions:[]};clients.push(c);
 // Synthetic browser device only: inputs still traverse gilrs and Bevy's real
 // gamepad/menu collector. This is not physical-controller coverage.
 await page.addInitScript(()=>{globalThis.__testPadEnabled=false;globalThis.__testPadButtons=Array(17).fill(false);Object.defineProperty(navigator,'getGamepads',{value:()=>{globalThis.__testPadPolls=(globalThis.__testPadPolls??0)+1;return globalThis.__testPadEnabled?[{id:'Menu acceptance standard pad',index:0,connected:true,mapping:'standard',timestamp:performance.now(),axes:[0,0,0,0],buttons:globalThis.__testPadButtons.map(pressed=>({pressed,touched:pressed,value:pressed?1:0}))}]:[];}});});
 page.on('pageerror',e=>evidence.errors.push(name+': '+e.message));
 page.on('console',msg=>{if(msg.type()==='error')(expectedError?evidence.expected_errors:evidence.errors).push(name+': '+msg.text());});
 page.on('websocket',ws=>{c.sockets++;if(new URL(ws.url()).href!==new URL(address).href){evidence.errors.push('unexpected WebSocket endpoint');return;}ws.on('framesent',({payload})=>{const m=JSON.parse(payload);if(m.Ready!==undefined)c.transitions.push({at:performance.now(),send:m});if(m.Input)c.inputs.push(m.Input);else c.commands.push(typeof m==='string'?m:Object.keys(m)[0]);});ws.on('framereceived',({payload})=>{const m=JSON.parse(payload);if(m.Welcome){secrets.add(m.Welcome.credential);const {credential,...safe}=m.Welcome;c.identity=safe;c.welcomes.push(safe);}if(m.Lobby){const phase=typeof m.Lobby.phase==='string'?m.Lobby.phase:'Countdown';if(phase!==c.phase){c.transitions.push({at:performance.now(),phase});c.phase=phase;}c.lobby=m.Lobby;c.lobbies.push(m.Lobby);}if(m.State){c.last=m.State;c.states++;if(m.State.winner!==null)c.outcome={tick:m.State.tick,winner:m.State.winner,wins:m.State.wins};}if(m.Error)c.error=m.Error;});});
 await page.goto(url,{waitUntil:'domcontentloaded'});return c;
}
async function setup(c){await openRoomMenu(c.page);}
async function enter(c,code=null){await setup(c);await c.page.locator('#backend').fill(address);await c.page.locator('#player-name').fill(c.name);if(code)await c.page.locator('#room-code').fill(code);await c.page.locator(code?'#join-room':'#create-room').click({noWaitAfter:true});await until(()=>c.identity&&c.lobby,c.name+' admission');await visible(c,'#room-controls');}
try{
 await new Promise(r=>front.listen(0,'127.0.0.1',r));origin=`http://127.0.0.1:${front.address().port}`;
 for(const [p,status] of [['/%2e%2e/Cargo.toml',403],['/%ZZ',400],['/%00',400]]){const code=await new Promise((r,j)=>http.get({host:'127.0.0.1',port:front.address().port,path:p},s=>{s.resume();r(s.statusCode);}).on('error',j));assert.equal(code,status);}
 address=await startBackend();
 const a=await browserClient('MenuHost');await visible(a,'#play-local');await shot(a,'main-menu');assert.equal(a.sockets,0);
 await a.page.locator('#play-local').click();await a.page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Lobby',null,{timeout:180000});await shot(a,'local-setup');await a.page.setViewportSize({width:390,height:844});await sleep(1500);await shot(a,'narrow-local-setup');await a.page.setViewportSize({width:960,height:720});
 await hold(a,'Escape');await visible(a,'#play-online');await a.page.evaluate(()=>{globalThis.__testPadEnabled=true;});await sleep(1600);await pad(a,13);assert.equal(await a.page.evaluate(()=>document.activeElement.id),'play-online');await pad(a,0);await visible(a,'#online-setup');await pad(a,1);assert.equal(await a.page.evaluate(()=>document.activeElement.id),'create-room');await pad(a,1);await visible(a,'#play-online');await a.page.locator('#play-online').focus();await a.page.keyboard.press('Enter');await visible(a,'#online-setup');await shot(a,'online-entry');
 await a.page.locator('#room-code').fill('invalid');await a.page.locator('#join-room').click();assert.match(await a.page.locator('#room-notice').innerText(),/20-character/);assert.equal(a.sockets,0);
 await a.page.locator('#player-name').focus();await a.page.keyboard.press('Escape');assert(await a.page.locator('#online-setup').isVisible(),'field Escape does not navigate');await sleep(1200);assert.equal(await a.page.evaluate(()=>document.activeElement.id),'create-room','idle Bevy bridge does not refocus field');await shot(a,'validation-feedback');
 await enter(a);assert.equal(a.sockets,1);await a.page.evaluate(()=>{globalThis.__testPadEnabled=true;});await shot(a,'host-lobby');
 const invite=await a.page.locator('#share-room').inputValue();const b=await browserClient('MenuGuest',invite,{width:390,height:844});
 await visible(b,'#online-setup');assert.equal(await b.page.locator('#room-code').inputValue(),a.lobby.code);await sleep(700);assert.equal(b.sockets,0,'fresh invite is not automatic admission');
 await shot(b,'narrow-invitation');assert(await b.page.evaluate(()=>document.documentElement.scrollWidth===innerWidth));
 await enter(b,a.lobby.code);assert.equal(b.sockets,1);await until(()=>a.lobby.humans.length===2,'authoritative roster');
 assert(await b.page.locator('#apply-room').isDisabled());await shot(b,'narrow-guest-lobby');
 assert(await b.page.evaluate(()=>document.documentElement.scrollWidth===innerWidth));
 await b.page.locator('#leave-room').scrollIntoViewIfNeeded();await shot(b,'narrow-guest-lobby-actions');
 await a.page.locator('#bots-room').fill('0');await a.page.locator('#target-room').focus();await pad(a,14);assert.equal(await a.page.locator('#target-room').inputValue(),'1','controller adjusts host settings');await a.page.locator('#apply-room').click();await until(()=>a.lobby.settings.bots===0&&a.lobby.settings.wins_target===1,'host settings');
 // The host is already ready before the guest starts Countdown. Its enabled
 // NOT READY control needs no fresh DOM update in the two-second critical path.
 await ready(a);
 await a.page.waitForFunction(()=>{const e=document.getElementById('ready-room');return !e.disabled&&e.textContent.includes('CANCEL');},null,{timeout:30000,polling:20});
 await a.page.locator('#ready-room').scrollIntoViewIfNeeded();
 const cancelBox=await a.page.locator('#ready-room').boundingBox();assert(cancelBox);
 await ready(b);await until(()=>typeof a.lobby.phase==='object','authoritative countdown');
 await a.page.mouse.click(cancelBox.x+cancelBox.width/2,cancelBox.y+cancelBox.height/2);await until(()=>a.lobby.phase==='Lobby','countdown cancellation');
 await shot(a,'countdown-canceled');await ready(a);await until(()=>a.last&&b.last,'two actual rendered matches started');
 // No bots and no held fire while first-render shaders settle. Release/sample
 // neutral input before a fresh press, including any automatic saved-seat retry.
 await running(a,b);
 for(const c of [a,b]){await c.page.locator('canvas').focus();await c.page.keyboard.up('d');await c.page.keyboard.up('Space');const n=c.inputs.length;await until(()=>c.inputs.length>n+2,'processed neutral input after presentation warmup');assert(c.inputs.slice(n).every(i=>i.horizontal===0&&i.vertical===0&&!i.fire));}
 await a.page.locator('canvas').focus();await pad(a,9);await visible(a,'#match-menu');assert(!(await a.page.locator('#room-form').isVisible()));
 // Space on the initially focused Resume button would activate it on keyup.
 // Keep the actual menu open, focus canvas and verify fresh neutral wire input.
 await a.page.locator('canvas').focus();const before=a.inputs.length;await a.page.keyboard.down('Space');await until(()=>a.inputs.length>before+2,'fresh input during match menu');assert(a.inputs.slice(before).every(i=>i.horizontal===0&&i.vertical===0&&!i.fire),'menu suppresses gameplay');await a.page.keyboard.up('Space');
 assert(await a.page.locator('#match-menu').isVisible());await shot(a,'online-context-menu');await a.page.locator('#resume-match').click();
 // Seed1, two humans, zero bots, target1: ordinary real-server
 // characterization reaches winner1 with host Right+fire and guest fire.
 // Capture first, with nobody firing, so screenshot time cannot alter the case.
 await running(a,b);await shot(a,'online-playing');
 await a.page.locator('canvas').focus();await b.page.locator('canvas').focus();const inputStarts=[a.inputs.length,b.inputs.length];await a.page.keyboard.down('d');await a.page.keyboard.down('Space');await b.page.keyboard.down('Space');
 await until(()=>a.inputs.slice(inputStarts[0]).some(i=>i.horizontal===1&&i.fire)&&b.inputs.slice(inputStarts[1]).some(i=>i.fire),'real outgoing keyboard intent');
 await until(()=>a.lobby.phase==='Lobby'&&b.lobby.phase==='Lobby','natural return to same lobby',240000);
 assert(a.outcome&&b.outcome,'both rendered peers observed a natural winner');assert.equal(a.outcome.winner,b.outcome.winner);evidence.natural_outcomes=[a.outcome,b.outcome];
 await a.page.keyboard.up('d');await a.page.keyboard.up('Space');await b.page.keyboard.up('Space');await visible(a,'#room-controls');await shot(a,'returned-lobby');
 assert.equal(a.lobby.id,b.lobby.id);assert.equal(a.lobby.humans.length,2);
 const previous=b.identity,welcomes=b.welcomes.length;await b.page.reload();await until(()=>b.welcomes.length>welcomes,'saved-seat reload');assert.equal(b.identity.participant,previous.participant);assert.equal(b.identity.seat,previous.seat);await visible(b,'#room-controls');
 await b.page.locator('#leave-room').click();await visible(b,'#online-setup');await until(()=>a.lobby.humans.length===1,'peer seat released');assert(!a.lobby.humans.some(h=>h.id===previous.participant));
 await b.browser.close();await a.page.setViewportSize({width:390,height:844});await shot(a,'narrow-host-lobby');assert(await a.page.evaluate(()=>document.documentElement.scrollWidth===innerWidth));await a.page.locator('#leave-room').scrollIntoViewIfNeeded();await shot(a,'narrow-host-lobby-actions');await a.page.setViewportSize({width:960,height:720});await a.page.locator('#leave-room').click();await visible(a,'#online-setup');await a.page.locator('#setup-back').click();await visible(a,'#play-local');
 const sockets=a.sockets;await a.page.locator('#play-local').click();await a.page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Lobby',null,{timeout:180000,polling:20});await a.page.locator('canvas').focus();await a.page.mouse.move(0,0);const neutral=await a.page.evaluate(()=>globalThis.__testPadPolls??0);await a.page.waitForFunction(n=>(globalThis.__testPadPolls??0)>n+2,neutral,{timeout:60000,polling:20});await hold(a,'w');await hold(a,'Enter');await a.page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Playing',null,{timeout:60000});await shot(a,'local-after-online');assert.equal(a.sockets,sockets,'no hidden online transport in local game');
 await hold(a,'Escape');await hold(a,'ArrowDown');await hold(a,'Enter');await visible(a,'#play-online');
 await setup(a);await a.page.locator('#backend').fill(address);await a.page.locator('#room-code').fill('0'.repeat(20));expectedError=true;await a.page.locator('#join-room').click();await a.page.waitForFunction(()=>document.getElementById('room-notice').textContent.includes('room not found'),null,{timeout:120000});await shot(a,'invalid-room-feedback');assert(await a.page.locator('#join-room').isEnabled());expectedError=false;
 const cap=await rawJoin();for(let i=0;i<3;i++)await rawJoin(cap.lobby.code);await until(()=>cap.lobby.humans.length===4,'full room has four real peers');
 await a.page.locator('#room-code').fill(cap.lobby.code);expectedError=true;await a.page.locator('#join-room').click();await a.page.waitForFunction(()=>document.getElementById('room-notice').textContent.includes('room full'),null,{timeout:120000});await shot(a,'full-room-feedback');
 evidence.formerly_valid_invite={room:cap.identity.room,code:cap.lobby.code,connected_before_restart:4};
 for(const peer of rawPeers){peer.ws.send(JSON.stringify('Leave'));peer.ws.close();}
 const bind=new URL(address).host;await stopBackend();assert.equal(await startBackend(bind),address);
 await a.page.locator('#join-room').click();await a.page.waitForFunction(()=>document.getElementById('room-notice').textContent.includes('room not found'),null,{timeout:120000});await shot(a,'expired-invitation-feedback');evidence.formerly_valid_invite.rejected_after_isolated_restart=true;
 await stopBackend();await a.page.locator('#create-room').click();await a.page.waitForFunction(()=>document.getElementById('room-notice').textContent.includes('Backend unavailable'),null,{timeout:120000});await shot(a,'backend-failure-feedback');assert(await a.page.locator('#join-room').isEnabled());expectedError=false;
 evidence.checks=['normal main local/online choice','local setup and later real local match after online Leave','keyboard accept and field Escape','visible validation without sockets','one socket per explicit fresh admission','invitation prefill without autojoin','390px invitation has no horizontal overflow','two separate rendered WASM clients','authoritative host/guest roster and permissions','host settings','countdown start/cancel','real keyboard network gameplay','no creation form in match menu; neutral input while menu focused','natural match return to same lobby','saved-seat reload keeps participant and seat','explicit peer Leave removes roster seat','online transport absent during later local play','real missing-room feedback and enabled recovery'];
 evidence.checks.push('real four-peer full-room rejection','formerly valid invitation rejected after isolated backend reset','backend unavailable feedback and enabled recovery');
 assert.equal(evidence.errors.length,0,JSON.stringify(evidence.errors));evidence.status='passed';
}catch(error){evidence.status='failed';evidence.failure=String(error.stack??error);for(const c of clients){if(c.page.isClosed())continue;try{evidence.failure_state??=[];evidence.failure_state.push({name:c.name,identity:c.identity,lobby:c.lobby,states:c.states,commands:c.commands,input_tail:c.inputs.slice(-20),page_state:await c.page.evaluate(()=>({active:document.activeElement?.id,focused:document.hasFocus(),hud:JSON.parse(globalThis.ryggattackHud??'null')})),notice:await c.page.locator('#room-notice').textContent(),local:await c.page.evaluate(()=>globalThis.ryggattackLocalScreen)});await shot(c,c.name+'-failure');}catch{}}process.exitCode=1;
}finally{
 evidence.clients=clients.map(c=>({name:c.name,identity:c.identity,sockets:c.sockets,welcomes:c.welcomes,states:c.states,input_count:c.inputs.length,commands:c.commands,transitions:c.transitions}));evidence.backend_log=backendLog;evidence.native_graphics=false;evidence.physical_controller=false;
 const text=JSON.stringify(evidence,null,2);assert([...secrets].every(secret=>!text.includes(secret)),'private reconnect credential in evidence');fs.writeFileSync(path.join(out,'results.json'),text);console.log(evidence.status,evidence.failure??evidence.checks);
 for(const peer of rawPeers)peer.ws.close();await Promise.all(browsers.map(b=>b.close()));server?.kill('SIGTERM');await new Promise(r=>front.close(r));
}
