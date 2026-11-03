// Local-only mobile Chromium acceptance. No production service is contacted.
import assert from 'node:assert/strict';
import {createServer,get} from 'node:http';
import {readFileSync,writeFileSync,mkdirSync,existsSync} from 'node:fs';
import {resolve,sep,extname} from 'node:path';
import {createRequire} from 'node:module';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
const require=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url);
const {chromium}=require('playwright');
const root=resolve('dist/web'),out=resolve(process.env.TOUCH_EVIDENCE??'verification/mobile-touch/browser');mkdirSync(out,{recursive:true});
const result={status:'running',steps:[],errors:[],artifact:{},started:new Date().toISOString(),verifier_sha256:createHash('sha256').update(readFileSync(new URL(import.meta.url))).digest('hex')};
const sourcePath=process.env.TOUCH_SOURCE_MANIFEST??'verification/mobile-touch/build-source.json';
if(existsSync(sourcePath)){const source=JSON.parse(readFileSync(sourcePath));for(const [path,hash] of Object.entries(source.files))assert.equal(createHash('sha256').update(readFileSync(path)).digest('hex'),hash,`source changed since build: ${path}`);result.source=source;}else result.source=null; // interaction coverage is not compiler provenance
for(const name of ['index.html','touch-controls.mjs','refresh-rate.mjs','rooms.mjs','online-hud.mjs','online-hud-bridge.mjs','ryggattack.js','ryggattack_bg.wasm'])result.artifact[name]=createHash('sha256').update(readFileSync(resolve(root,name))).digest('hex');
function save(){writeFileSync(resolve(out,'results.json'),JSON.stringify(result,null,2));}
function step(name,data={}){result.steps.push({name,...data,at:new Date().toISOString()});save();console.log(name,JSON.stringify(data));}
const server=createServer((req,res)=>{
  let path;try{path=decodeURIComponent((req.url??'/').split('?')[0]);if(path.includes('\0'))throw Error();}catch{res.writeHead(400).end();return;}
  const file=resolve(root,'.'+(path==='/'?'/index.html':path));
  if(file!==root&&!file.startsWith(root+sep)){res.writeHead(403).end();return;}
  try{const body=readFileSync(file);res.writeHead(200,{'Content-Type':({'.mjs':'text/javascript','.js':'text/javascript','.html':'text/html','.wasm':'application/wasm','.png':'image/png'})[extname(file)]??'application/octet-stream'});res.end(body);}catch{res.writeHead(404).end();}
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;
const raw=path=>new Promise((resolve,reject)=>get({hostname:'127.0.0.1',port,path},r=>{r.resume();r.on('end',()=>resolve(r.statusCode));}).on('error',reject));
assert.equal(await raw('/%2e%2e/README.md'),403);assert.equal(await raw('/%00'),400);assert.equal(await raw('/%ZZ'),400);
let browser,backend,lastPage;
try{
  browser=await chromium.launch({headless:true,args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
  const context=await browser.newContext({viewport:{width:375,height:740},deviceScaleFactor:1,isMobile:true,hasTouch:true});
  await context.addInitScript(()=>{globalThis.ryggattackTouchDebug=true;
    Date.now=()=>3000; // legal offline map seed 3; no winner/state injection
    globalThis.__touchInputs=[];const send=WebSocket.prototype.send;WebSocket.prototype.send=function(value){try{const m=JSON.parse(value);if(m.Input){globalThis.__touchInputs.push(m.Input);if(globalThis.__touchInputs.length>1000)globalThis.__touchInputs.shift();}}catch{}return send.call(this,value);};});
  const page=await context.newPage();lastPage=page;page.setDefaultTimeout(60000);
  page.on('pageerror',e=>result.errors.push(String(e)));
  const cdp=await context.newCDPSession(page);
  const touch=async(type,points)=>cdp.send('Input.dispatchTouchEvent',{type,touchPoints:points.map(([x,y,id=1])=>({x,y,id,radiusX:2,radiusY:2,force:1}))});
  const tap=async(x=180,y=360)=>page.touchscreen.tap(x,y); // trusted atomic tap, not a slow CDP hold
  const swipe=async(dx=85,dy=0)=>{await touch('touchStart',[[150,360]]);await touch('touchMove',[[150+dx,360+dy]]);await touch('touchEnd',[]);};
  const shot=async name=>{await page.screenshot({path:resolve(out,name+'.png'),timeout:90000});};
  const local=()=>page.evaluate(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null'));
  const observation=()=>page.evaluate(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').find(p=>p.seat===0));
  const waitLocal=async condition=>page.waitForFunction(condition,null,{timeout:90000});
  const traceSamples=async target=>target.evaluate(()=>{
    globalThis.__cancelSamples=[];
    const sample=globalThis.ryggattackSampleTouch;
    globalThis.ryggattackSampleTouch=()=>{const value=sample();globalThis.__cancelSamples.push(JSON.parse(value));if(globalThis.__cancelSamples.length>500)globalThis.__cancelSamples.shift();return value;};
  });
  const fireThenCancel=async(target,session,mode)=>{
    await target.evaluate(()=>{globalThis.__cancelSamples=[];});
    await target.touchscreen.tap(180,370);
    await target.waitForFunction(()=>globalThis.__cancelSamples.some(s=>s.fire));
    const sampled=await target.evaluate(()=>globalThis.__cancelSamples.find(s=>s.fire));
    await session.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:120,y:360,id:1},{x:240,y:360,id:2}]});
    await target.waitForFunction(epoch=>globalThis.__cancelSamples.some(s=>s.epoch>epoch&&!s.fire&&s.horizontal===0&&s.vertical===0),sampled.epoch);
    await session.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
    await target.waitForFunction(epoch=>globalThis.__cancelSamples.filter(s=>s.epoch>epoch).length>=3,sampled.epoch);
    const after=await target.evaluate(epoch=>globalThis.__cancelSamples.filter(s=>s.epoch>epoch),sampled.epoch);
    assert(after.every(s=>!s.fire&&s.horizontal===0&&s.vertical===0),'no gesture may survive multitouch cancellation');
    step(`${mode} real sampled fire then multitouch cancellation epoch`,{sampled,after});
  };
  await page.goto(`http://127.0.0.1:${port}/?quality=0`,{waitUntil:'domcontentloaded',timeout:90000});
  await waitLocal(()=>globalThis.ryggattackLocalScreen==='MainMenu');await shot('01-phone-main');
  await page.locator('#play-local').tap();await waitLocal(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.screen==='Lobby');
  await page.locator('#touch-join').tap();await waitLocal(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.joined);
  await page.locator('#touch-bots').tap();await waitLocal(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.bots===0);
  await page.waitForFunction(()=>document.getElementById('touch-bots').textContent==='BOTS: 0');await shot('02-phone-lobby');await page.locator('#touch-start').tap();await waitLocal(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.screen==='Playing');
  await page.waitForFunction(()=>document.querySelector('canvas').classList.contains('touch-playing'));
  assert.equal(await page.locator('canvas').evaluate(e=>getComputedStyle(e).touchAction),'none');
  await swipe();await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===1));
  await page.waitForTimeout(1600);assert.equal((await observation()).turn[0],1);assert.equal((await observation()).cooldown,0);
  step('offline real ECS retained swipe; no accidental fire',{observation:await observation()});
  await tap();await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.cooldown>0),null,{timeout:15000});
  step('offline tap fired through existing cooldown',{observation:await observation()});await shot('03-phone-playing');
  await traceSamples(page);await fireThenCancel(page,cdp,'offline');
  await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===0&&p.turn[1]===0&&p.cooldown===0));
  step('offline cancelled fire drains to zero cooldown and neutral real ECS');await shot('07-offline-cancelled');
  await swipe();await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===1));
  await page.locator('#touch-pause').tap();await waitLocal(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.paused);
  await page.locator('#help-resume').waitFor({state:'visible'});await shot('04-phone-paused');await page.locator('#help-resume').tap();await waitLocal(()=>!JSON.parse(globalThis.ryggattackLocalStatus??'null')?.paused);
  await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===0&&p.turn[1]===0));step('offline pause/resume clears retained steering');
  await swipe(0,-85);await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[1]===1));
  await touch('touchStart',[[120,360],[240,360,2]]);await touch('touchEnd',[]);
  await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[1]===0));step('real multitouch clears steering');
  await swipe();await touch('touchStart',[[150,360]]);await touch('touchCancel',[]);
  await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===0));step('real pointer cancellation clears steering');
  await swipe();await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===1));await page.locator('#touch-pause').focus();
  assert.notEqual(await page.evaluate(()=>document.activeElement?.id),'ryggattack-canvas');
  await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===0&&p.turn[1]===0));step('actual DOM canvas blur clears input');
  // Keyboard regression still enters the same ECS path on a touch viewport.
  await page.locator('canvas').focus();await page.keyboard.down('ArrowLeft');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.turn[0]===-1));await page.keyboard.up('ArrowLeft');step('desktop keyboard steering preserved');
  await page.locator('#touch-back').tap();await waitLocal(()=>globalThis.ryggattackLocalScreen==='MainMenu');
  await page.locator('#play-local').tap();await waitLocal(()=>globalThis.ryggattackLocalScreen==='Lobby');await page.locator('#touch-bots').tap();await waitLocal(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.bots===1);
  for(const target of [5,7,1]){await page.locator('#touch-target').tap();await waitLocal(new Function(`return JSON.parse(globalThis.ryggattackLocalStatus??'null')?.target===${target}`));}
  await page.locator('#touch-start').tap();await waitLocal(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.screen==='Playing');
  // Real bots/combat determine the winner; no ECS/winner injection.
  await waitLocal(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.winner);await page.locator('#touch-restart').waitFor({state:'visible'});await shot('05-phone-match-over');
  await page.locator('#touch-restart').tap();await waitLocal(()=>!JSON.parse(globalThis.ryggattackLocalStatus??'null')?.finished);step('touch restart of a naturally completed match');
  await page.locator('#touch-back').tap();await waitLocal(()=>globalThis.ryggattackLocalScreen==='MainMenu');
  await context.close();
  // Same built client, one local authoritative server, no competing renderer.
  const binary=resolve('target/debug/ryggattack-server');assert(existsSync(binary),'build the native server first');
  backend=spawn(binary,['127.0.0.1:0'],{env:{...process.env,RYGGATTACK_MODE:'development',RYGGATTACK_ALLOWED_ORIGINS:`http://127.0.0.1:${port}`,RYGGATTACK_MANAGEMENT_BIND:'127.0.0.1:0'},stdio:['ignore','pipe','pipe']});
  const backendAddress=await new Promise((resolve,reject)=>{let text='';const timer=setTimeout(()=>reject(Error('local backend startup timed out')),10000);backend.once('error',reject);backend.once('exit',code=>reject(Error(`backend exited ${code}`)));backend.stdout.on('data',data=>{text+=data;for(const line of text.split('\n'))try{const m=JSON.parse(line);if(m.event==='listening'){clearTimeout(timer);resolve(m.websocket);}}catch{}});});
  backend.stderr.on('data',()=>{});
  const onlineContext=await browser.newContext({viewport:{width:375,height:740},deviceScaleFactor:1,isMobile:true,hasTouch:true});
  await onlineContext.addInitScript(()=>{globalThis.__touchInputs=[];const send=WebSocket.prototype.send;WebSocket.prototype.send=function(v){try{const m=JSON.parse(v);if(m.Input)globalThis.__touchInputs.push(m.Input);}catch{}return send.call(this,v);};});
  const online=await onlineContext.newPage();lastPage=online;
  let serverShot=false;result.online_authority_shots=[];
  online.on('websocket',socket=>socket.on('framereceived',({payload})=>{try{const frame=JSON.parse(typeof payload==='string'?payload:payload.toString()).State;const cart=frame?.carts?.find(c=>c.id===0);if(cart?.alive&&cart.shot_cooldown>0){serverShot=true;if(result.online_authority_shots.length<12)result.online_authority_shots.push({tick:frame.tick,seat:cart.id,cooldown:cart.shot_cooldown,alive:cart.alive});}}catch{}}));online.setDefaultTimeout(60000);online.on('pageerror',e=>result.errors.push(String(e)));
  await online.goto(`http://127.0.0.1:${port}/?quality=0`,{waitUntil:'domcontentloaded'});
  await online.locator('#play-online').tap();await online.locator('#player-name').fill('TouchQA');
  await online.locator('#online-setup summary').tap();await online.locator('#backend').fill(backendAddress);
  assert.equal(await online.locator('#online-entry').evaluate(e=>getComputedStyle(e).touchAction),'auto');
  await online.locator('#create-room').tap();
  await online.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'null')?.lobby?.phase==='Lobby',null,{timeout:120000});
  await online.locator('#bots-room').fill('1');await online.locator('#target-room').selectOption('7');await online.locator('#apply-room').tap();
  await online.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'null')?.lobby?.settings?.bots===1);
  await online.locator('#ready-room').tap();await online.waitForFunction(()=>JSON.parse(globalThis.ryggattackHud??'null')?.lobby?.phase==='Running',null,{timeout:120000});
  await online.waitForFunction(()=>document.getElementById('online-entry').hidden&&document.getElementById('ryggattack-canvas').classList.contains('touch-playing')&&document.activeElement?.id==='ryggattack-canvas');
  const oc=await onlineContext.newCDPSession(online);
  await oc.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:140,y:370,id:1}]});await oc.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:220,y:370,id:1}]});await oc.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});
  await online.waitForFunction(()=>globalThis.__touchInputs?.some(i=>i.horizontal===1&&i.fire===false));
  assert.equal(serverShot,false,'online swipe must not accidentally fire');
  await online.touchscreen.tap(180,370);await online.waitForFunction(()=>globalThis.__touchInputs?.some(i=>i.horizontal===1&&i.fire));
  const swipeInput=await online.evaluate(()=>globalThis.__touchInputs.find(i=>i.horizontal===1&&!i.fire));const tapInput=await online.evaluate(()=>globalThis.__touchInputs.find(i=>i.horizontal===1&&i.fire));
  const shotDeadline=Date.now()+15000;while(!serverShot&&Date.now()<shotDeadline)await online.waitForTimeout(100);assert(serverShot,'touch tap must produce the authoritative local cart cooldown, not just outgoing wire fire');
  await online.screenshot({path:resolve(out,'06-phone-online.png'),timeout:90000});step('online swipe and tap reached real websocket intent',{swipe:swipeInput,tap:tapInput,authority:result.online_authority_shots});
  await traceSamples(online);await fireThenCancel(online,oc,'online');
  const cancelWireStart=await online.evaluate(()=>globalThis.__touchInputs.length);
  await online.waitForFunction(start=>globalThis.__touchInputs.length>=start+3,cancelWireStart);
  const cancelledWire=await online.evaluate(start=>globalThis.__touchInputs.slice(start),cancelWireStart);
  assert(cancelledWire.every(i=>!i.fire&&i.horizontal===0&&i.vertical===0));
  step('online cancelled fire sends neutral real wire intents',{inputs:cancelledWire});
  await online.screenshot({path:resolve(out,'08-online-cancelled.png'),timeout:90000});
  await online.locator('#open-match-menu').tap();await online.waitForFunction(()=>JSON.parse(globalThis.ryggattackHud??'null')?.menu);
  await online.waitForFunction(()=>{const i=globalThis.__touchInputs?.at(-1);return i&&i.horizontal===0&&i.vertical===0&&!i.fire;});
  await online.locator('#resume-match').tap();await online.waitForFunction(()=>!JSON.parse(globalThis.ryggattackHud??'null')?.menu);step('online accessible menu/resume neutralized input; server keeps running');
  await online.locator('#open-match-menu').tap();await online.locator('#leave-match').tap();
  await online.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'null')?.status==='Left room; create or join another');
  await onlineContext.close();assert.deepEqual(result.errors,[]);result.status='passed';save();
}catch(error){if(lastPage&&!lastPage.isClosed())result.diagnostic=await lastPage.evaluate(()=>({pointers:globalThis.__pointerTrace,samples:globalThis.__sampleTrace,focus:document.activeElement?.id,status:globalThis.ryggattackLocalStatus,room:globalThis.ryggattackRoom?JSON.parse(globalThis.ryggattackRoom).lobby?.phase:null,menuHidden:document.getElementById('online-entry')?.hidden,inputs:globalThis.__touchInputs?.slice(-20),hudPhase:globalThis.ryggattackHud?JSON.parse(globalThis.ryggattackHud).lobby?.phase:null}));if(lastPage&&!lastPage.isClosed())try{await lastPage.screenshot({path:resolve(out,'failure.png'),timeout:90000});}catch{}result.status='failed';result.failure=String(error.stack??error);save();console.error(error);process.exitCode=1;}
finally{if(browser)await browser.close();if(backend&&backend.exitCode===null){backend.kill('SIGTERM');await new Promise(r=>backend.once('exit',r));}await new Promise(r=>server.close(r));}
