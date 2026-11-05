// Real packaged WASM, local backend only. One rendered + one legal neutral human.
import assert from 'node:assert/strict';
import {createServer,get} from 'node:http';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,sep,extname} from 'node:path';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
const root=resolve(process.env.LAYOUT_BUNDLE??'dist/web'),out=resolve(process.env.LAYOUT_EVIDENCE??'verification/shared-play-layout');mkdirSync(out,{recursive:true});
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const baseline=process.env.LAYOUT_BASELINE==='1';
const result={status:'running',baseline,started:new Date().toISOString(),verifier:hash(new URL(import.meta.url)),artifact:{},views:[],failures:[],errors:[],fixture:{renderedHumans:1,neutralTransportHumans:1,bots:0}};
for(const name of ['index.html','touch-controls.mjs','refresh-rate.mjs','rooms.mjs','online-hud.mjs','online-hud-bridge.mjs','ryggattack.js','ryggattack_bg.wasm'])result.artifact[name]=hash(resolve(root,name));
const save=()=>writeFileSync(resolve(out,'results.json'),JSON.stringify(result,null,2));save();
const server=createServer((req,res)=>{let path;try{path=decodeURIComponent((req.url??'/').split('?')[0]);if(path.includes('\0'))throw Error();}catch{res.writeHead(400).end();return;}
 const file=resolve(root,'.'+(path==='/'?'/index.html':path));if(file!==root&&!file.startsWith(root+sep)){res.writeHead(403).end();return;}
 try{res.writeHead(200,{'Content-Type':({'.html':'text/html','.mjs':'text/javascript','.js':'text/javascript','.wasm':'application/wasm','.png':'image/png'})[extname(file)]??'application/octet-stream'});res.end(readFileSync(file));}catch{res.writeHead(404).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;
const raw=path=>new Promise((resolve,reject)=>get({hostname:'127.0.0.1',port,path},r=>{r.resume();r.on('end',()=>resolve(r.statusCode));}).on('error',reject));
assert.equal(await raw('/%2e%2e/README.md'),403);assert.equal(await raw('/%00'),400);assert.equal(await raw('/%ZZ'),400);
let browser,backend,peer,peerTimer,page;
try{
 backend=spawn(resolve(process.env.LAYOUT_SERVER??'target/debug/ryggattack-server'),['127.0.0.1:0'],{env:{...process.env,RYGGATTACK_MODE:'development',RYGGATTACK_ALLOWED_ORIGINS:`http://127.0.0.1:${port}`,RYGGATTACK_MANAGEMENT_BIND:'127.0.0.1:0'},stdio:['ignore','pipe','pipe']});
 const address=await new Promise((resolve,reject)=>{let text='';const timer=setTimeout(()=>reject(Error('backend startup timed out')),10000);backend.once('error',reject);backend.once('exit',c=>reject(Error(`backend exit ${c}`)));backend.stdout.on('data',d=>{text+=d;for(const l of text.split('\n'))try{const m=JSON.parse(l);if(m.event==='listening'){clearTimeout(timer);resolve(m.websocket);}}catch{}});});backend.stderr.on('data',()=>{});
 browser=await chromium.launch({headless:true,args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
 for(const coarse of [true,false]){
  const pointer=coarse?'coarse':'fine';
  const context=await browser.newContext({viewport:{width:375,height:740},hasTouch:coarse,isMobile:coarse,deviceScaleFactor:1});
  await context.addInitScript(()=>{Date.now=()=>3000;globalThis.ryggattackTouchDebug=true;globalThis.__inputs=[];const send=WebSocket.prototype.send;WebSocket.prototype.send=function(v){try{const m=JSON.parse(v);if(m.Input)globalThis.__inputs.push(m.Input);}catch{}return send.call(this,v);};});
  page=await context.newPage();page.setDefaultTimeout(90000);page.on('pageerror',e=>result.errors.push(String(e)));
  let authorityShot=false;
  page.on('websocket',socket=>socket.on('framereceived',({payload})=>{try{const f=JSON.parse(typeof payload==='string'?payload:payload.toString()).State;if(f?.carts?.some(c=>c.id===0&&c.alive&&c.shot_cooldown>0))authorityShot=true;}catch{}}));
  await page.goto(`http://127.0.0.1:${port}/?quality=0`,{waitUntil:'domcontentloaded'});await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='MainMenu');
  assert.equal(await page.evaluate(()=>matchMedia('(pointer:coarse)').matches),coarse);
  const activate=async id=>{if(coarse)await page.locator(id).tap();else await page.locator(id).click();};
  const key=async name=>{await page.keyboard.down(name);await page.waitForTimeout(600);await page.keyboard.up(name);await page.waitForTimeout(400);};
  const startLocal=async()=>{
   await activate('#play-local');await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Lobby');
   if(coarse){await activate('#touch-join');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackLocalStatus).joined);if(await page.evaluate(()=>JSON.parse(globalThis.ryggattackLocalStatus).bots!==0))await activate('#touch-bots');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackLocalStatus).bots===0);await activate('#touch-start');}
   else{await page.locator('canvas').focus();await page.waitForTimeout(800);await key('w');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackLocalStatus).joined);if(await page.evaluate(()=>JSON.parse(globalThis.ryggattackLocalStatus).bots!==0)){await key('ArrowRight');await key('Enter');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackLocalStatus).bots===0);await key('ArrowLeft');}await key('Enter');}
   await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Playing'&&document.querySelector('canvas').classList.contains('touch-playing'));
  };
  const record=async(mode,name,width,height)=>{
   await page.setViewportSize({width,height});await page.waitForFunction(()=>{const c=document.querySelector('canvas'),r=c.getBoundingClientRect();return c.width===Math.round(r.width*devicePixelRatio)&&c.height===Math.round(r.height*devicePixelRatio);});await page.waitForTimeout(1600);
   const view=await page.evaluate(()=>{const rect=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height};};const c=document.querySelector('canvas'),g=document.getElementById('game'),p=document.getElementById(globalThis.ryggattackLaunch?'open-match-menu':'touch-pause'),s=getComputedStyle(g);return {canvas:rect(c),backing:[c.width,c.height],game:rect(g),css:{top:s.top,bottom:s.bottom,left:s.left,right:s.right},pause:rect(p),buttons:[...document.querySelectorAll('button')].filter(e=>e.getClientRects().length).map(e=>e.id),offlineHud:!!document.getElementById('offline-hud').getClientRects().length,onlineHud:!!document.querySelector('.rygg-online-hud')?.getClientRects().length};});
   result.views.push({pointer,mode,name,width,height,...view});save();
   await page.screenshot({path:resolve(out,`${pointer}-${mode}-${name}.png`),timeout:90000});
   try{assert.deepEqual(view.canvas,{x:0,y:0,width,height},'game must fill viewport regardless of mode/pointer');assert.deepEqual(view.backing,[width,height],'actual Winit backing surface');assert(!view.offlineHud&&!view.onlineHud,'scores belong in pause');assert.deepEqual(view.buttons,[mode==='online'?'open-match-menu':'touch-pause']);assert.equal(view.pause.width,48);assert.equal(view.pause.height,48);assert(view.pause.x>=0&&view.pause.y>=0&&view.pause.x+48<=width&&view.pause.y+48<=height,'floating pause fits');}catch(e){result.failures.push({pointer,mode,name,error:String(e)});save();if(!baseline)throw e;}
  };
  await startLocal();
  for(const [name,w,h] of [['portrait',375,740],['narrow',320,640],['landscape',740,375],['desktop',1280,720],['rotated-back',375,740]])await record('offline',name,w,h);
  if(coarse){await page.touchscreen.tap(180,360);await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.cooldown>0));result.offlineTouchFire=true;}
  else{await page.locator('canvas').focus();await page.keyboard.down('ArrowLeft');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===-1));await page.keyboard.up('ArrowLeft');result.keyboardSteering=true;}
  if(coarse||!baseline)await activate('#touch-pause');else await key('Escape');
  await page.locator('#offline-pause').waitFor({state:'visible'});assert.equal(await page.locator('#pause-scores').innerText(),await page.evaluate(()=>globalThis.ryggattackOfflineHud));await page.screenshot({path:resolve(out,`${pointer}-offline-paused.png`),timeout:90000});
  await activate('#help-resume');await page.waitForFunction(()=>!JSON.parse(globalThis.ryggattackLocalStatus).paused&&document.getElementById('offline-pause').hidden);
  if(coarse||!baseline)await activate('#touch-pause');else await key('Escape');await page.locator('#offline-pause').waitFor({state:'visible'});
  await activate('#help-back');await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='MainMenu');
  await activate('#play-online');await page.locator('#player-name').fill('LayoutQA');if(!await page.locator('#online-setup details').evaluate(e=>e.open))await activate('#online-setup summary');await page.locator('#backend').fill(address);await activate('#create-room');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'null')?.lobby?.phase==='Lobby');
  await page.locator('#bots-room').fill('0');await page.locator('#target-room').selectOption('7');await activate('#apply-room');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom).lobby.settings.bots===0);
  const code=await page.evaluate(()=>JSON.parse(globalThis.ryggattackRoom).lobby.code);let phase,sequence=0;
  peer=new WebSocket(address);await new Promise((resolve,reject)=>{const t=setTimeout(()=>reject(Error('peer admission timeout')),10000);peer.addEventListener('error',()=>reject(Error('peer error')));peer.addEventListener('open',()=>peer.send(JSON.stringify({Hello:{version:4,name:'LayoutPeer',room:code}})));peer.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Lobby){phase=m.Lobby.phase;if(m.Lobby.humans.length===2){clearTimeout(t);resolve();}}});});
  peer.send(JSON.stringify({Ready:true}));peerTimer=setInterval(()=>{if(phase==='Running'&&peer.readyState===WebSocket.OPEN)peer.send(JSON.stringify({Input:{sequence:++sequence,horizontal:0,vertical:0,fire:false}}));},100);
  await activate('#ready-room');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackHud??'null')?.lobby?.phase==='Running'&&document.getElementById('online-entry').hidden);
  for(const [name,w,h] of [['portrait',375,740],['narrow',320,640],['landscape',740,375],['desktop',1280,720],['rotated-back',375,740]])await record('online',name,w,h);
  if(coarse){
   const cdp=await context.newCDPSession(page);
   await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:130,y:360,id:1}]});await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:210,y:360,id:1}]});await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
   await page.waitForFunction(()=>globalThis.__inputs.some(i=>i.horizontal===1&&!i.fire));assert.equal(authorityShot,false,'swipe must not fire');
   await page.touchscreen.tap(180,360);await page.waitForFunction(()=>globalThis.__inputs.some(i=>i.horizontal===1&&i.fire));
   const deadline=Date.now()+15000;while(!authorityShot&&Date.now()<deadline)await page.waitForTimeout(100);assert(authorityShot,'real authoritative cooldown after touch fire');result.onlineTouchSwipeFire=true;
  }
  await activate('#open-match-menu');await page.locator('#match-menu').waitFor({state:'visible'});
  if(!baseline)await page.locator('#online-pause-details').waitFor({state:'visible'});
  await page.screenshot({path:resolve(out,`${pointer}-online-paused.png`),timeout:90000});await activate('#resume-match');await page.waitForFunction(()=>document.getElementById('online-entry').hidden);
  await activate('#open-match-menu');await activate('#leave-match');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'null')?.status==='Left room; create or join another');
  clearInterval(peerTimer);peerTimer=null;if(peer.readyState===WebSocket.OPEN)peer.send(JSON.stringify('Leave'));peer.close();peer=null;
  await activate('#setup-back');await startLocal();await record('offline','after-online',375,740);
  result[pointer+'Transitions']='local → pause/menu → online → pause/resume/leave → local';save();await context.close();
 }
 assert.deepEqual(result.errors,[]);assert.equal(result.failures.length,0,'shared gameplay layout regressions');result.status='passed';
}catch(e){result.status='failed';result.failure=String(e.stack??e);console.error(e);process.exitCode=1;}
finally{result.finished=new Date().toISOString();save();if(peerTimer)clearInterval(peerTimer);if(peer)peer.close();if(browser)await browser.close();if(backend&&backend.exitCode===null){backend.kill('SIGTERM');await new Promise(r=>backend.once('exit',r));}await new Promise(r=>server.close(r));}
