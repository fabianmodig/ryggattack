// Real rendered offline game. No state/pickup/score injection; no remote services.
import assert from 'node:assert/strict';
import {createServer,get} from 'node:http';
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,sep,extname} from 'node:path';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
const root=resolve(process.env.HUD_BUNDLE??'dist/web'),out=resolve(process.env.HUD_EVIDENCE??'verification/mobile-hud-fix');mkdirSync(out,{recursive:true});
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const result={status:'running',started:new Date().toISOString(),verifier:hash(new URL(import.meta.url)),artifact:{},views:[],errors:[]};
for(const name of ['index.html','touch-controls.mjs','ryggattack.js','ryggattack_bg.wasm'])result.artifact[name]=hash(resolve(root,name));
const save=()=>writeFileSync(resolve(out,'results.json'),JSON.stringify(result,null,2));save();
const server=createServer((req,res)=>{let path;try{path=decodeURIComponent((req.url??'/').split('?')[0]);if(path.includes('\0'))throw Error();}catch{res.writeHead(400).end();return;}
 const file=resolve(root,'.'+(path==='/'?'/index.html':path));if(file!==root&&!file.startsWith(root+sep)){res.writeHead(403).end();return;}
 try{res.writeHead(200,{'Content-Type':({'.html':'text/html','.mjs':'text/javascript','.js':'text/javascript','.wasm':'application/wasm','.png':'image/png'})[extname(file)]??'application/octet-stream'});res.end(readFileSync(file));}catch{res.writeHead(404).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;
const raw=path=>new Promise((resolve,reject)=>get({hostname:'127.0.0.1',port,path},r=>{r.resume();r.on('end',()=>resolve(r.statusCode));}).on('error',reject));
assert.equal(await raw('/%2e%2e/README.md'),403);assert.equal(await raw('/%00'),400);
let browser,page;
try {
 browser=await chromium.launch({headless:true,args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
 const context=await browser.newContext({viewport:{width:375,height:740},hasTouch:true,isMobile:true,deviceScaleFactor:1});
 await context.addInitScript(()=>{Date.now=()=>3000;globalThis.ryggattackTouchDebug=true;});
 page=await context.newPage();page.setDefaultTimeout(90000);page.on('pageerror',e=>result.errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${port}/?quality=0`,{waitUntil:'domcontentloaded'});
 await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='MainMenu');
 await page.locator('#play-local').tap();await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Lobby');
 await page.locator('#touch-join').tap();await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.joined);
 await page.locator('#touch-start').tap();await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Playing');
 const shot=async(name)=>{await page.screenshot({path:resolve(out,name+'.png'),timeout:90000});};
 for(const [name,width,height] of [['phone',375,740],['landscape',740,375],['narrow',320,640],['rotated-back',375,740]]){
  await page.setViewportSize({width,height});
  await page.waitForTimeout(1600); // allow real Winit resize + projection frames, not just DOM reflow
  await page.waitForFunction(()=>document.querySelector('canvas').classList.contains('touch-playing')||JSON.parse(globalThis.ryggattackLocalStatus??'null')?.finished);
  await shot(name+'-playing');
  result.views.push(await page.evaluate(name=>{const rect=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,bottom:r.bottom,right:r.right};};const hud=document.getElementById('offline-hud'),canvas=document.querySelector('canvas');return {name,canvas:rect(canvas),canvasPixels:[canvas.width,canvas.height],hudHidden:hud.hidden,text:hud.textContent,pause:rect(document.getElementById('touch-pause')),visibleButtons:[...document.querySelectorAll('button')].filter(e=>e.getClientRects().length).map(e=>e.id),ecs:globalThis.ryggattackOfflineHud,local:globalThis.ryggattackLocalStatus};},name));save();
  {
   const view=result.views.at(-1);
   assert.equal(view.canvas.y,0,'mobile must not reserve a top info band');
   assert.equal(view.canvas.x,0);
   assert.equal(view.canvas.width,width);assert.equal(view.canvas.height,height,'mobile canvas must fill the viewport, without a bottom control row');
   assert.equal(view.hudHidden,true,'scores belong in pause, not active gameplay');
   assert.deepEqual(view.visibleButtons,['touch-pause'],'only one discreet active-play pause affordance');
   assert(view.pause.width>=44&&view.pause.width<=56&&view.pause.height>=44&&view.pause.height<=56,'pause must be accessible but discreet');
   assert(Math.abs(view.canvasPixels[0]-view.canvas.width)<=1&&Math.abs(view.canvasPixels[1]-view.canvas.height)<=1,'Bevy backing surface did not resize');
   assert.equal(view.text,view.ecs,'retained scores must be actual ECS publication');
   // A trusted tap and real ECS cooldown prove this is the running Bevy game.
   if(name==='phone'){
    await page.waitForFunction(()=>!JSON.parse(globalThis.ryggattackLocalStatus??'null')?.finished&&/^P1:.* IN/m.test(globalThis.ryggattackOfflineHud??''));
    await page.touchscreen.tap(width/2,view.canvas.y+view.canvas.height/2);
    await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackTouchObservation??'[]').some(p=>p.seat===0&&p.cooldown>0));result.fired=true;
   }
   {
    // Round transitions are legal; wait for real play rather than silently
    // skipping the required phone/landscape pause-help acceptance captures.
    await page.waitForFunction(()=>!JSON.parse(globalThis.ryggattackLocalStatus??'null')?.finished);
    await page.locator('#touch-pause').tap();await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.paused);
    await page.locator('#offline-pause').waitFor({state:'visible'});await shot(name+'-pause-help');
    assert.equal(await page.locator('#pause-scores').innerText(),await page.evaluate(()=>globalThis.ryggattackOfflineHud));
    const help=await page.locator('#offline-pause').innerText();for(const term of ['rear','yellow','pink','SPD','HIT','swipe','gamepad'])assert(help.toLowerCase().includes(term.toLowerCase()),`missing pause help ${term}`);
    if(name==='landscape'){
     await page.locator('#offline-pause').evaluate(e=>{e.scrollTop=e.scrollHeight;});await shot(name+'-pause-help-details');
    }
    if(name==='phone'){
     await page.keyboard.press('Tab');await page.waitForFunction(()=>globalThis.ryggattackPauseSelection==='back'&&document.activeElement?.id==='help-back');
     await page.keyboard.press('Tab');await page.waitForFunction(()=>globalThis.ryggattackPauseSelection==='resume'&&document.activeElement?.id==='help-resume');
     await page.keyboard.press('Escape');result.keyboardPauseFocus=true;
    }else await page.locator('#help-resume').tap();
    await page.waitForFunction(()=>!JSON.parse(globalThis.ryggattackLocalStatus??'null')?.paused);await page.locator('#offline-pause').waitFor({state:'hidden'});
   }
  }
 }

 await context.close();
 const desktopContext=await browser.newContext({viewport:{width:1280,height:720},deviceScaleFactor:1});
 page=await desktopContext.newPage();page.setDefaultTimeout(90000);page.on('pageerror',e=>result.errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${port}/?quality=0`,{waitUntil:'domcontentloaded'});
 await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='MainMenu');await page.locator('#play-local').click();
 await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Lobby');await page.waitForTimeout(500);
 await page.keyboard.down('w');await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.joined);await page.keyboard.up('w');
 await page.keyboard.press('Enter');await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Playing');
 await page.waitForFunction(()=>document.getElementById('game').classList.contains('gameplay'));await page.waitForTimeout(1600);
 result.desktop=await page.evaluate(()=>{const c=document.querySelector('canvas'),r=c.getBoundingClientRect();return {canvas:{x:r.x,y:r.y,width:r.width,height:r.height},backing:[c.width,c.height],hudVisible:!!document.getElementById('offline-hud').getClientRects().length,menuVisible:!!document.getElementById('touch-back').getClientRects().length};});
 assert.deepEqual(result.desktop.canvas,{x:0,y:0,width:1280,height:720});assert.deepEqual(result.desktop.backing,[1280,720]);assert(!result.desktop.hudVisible&&!result.desktop.menuVisible,'desktop shares game-only layout; scores and menu live in pause');
 await shot('desktop-playing');
 assert.deepEqual(result.errors,[]);result.status='passed';
}catch(error){result.status='failed';result.failure=String(error.stack??error);console.error(error);process.exitCode=1;}
finally{result.finished=new Date().toISOString();save();if(browser)await browser.close();await new Promise(r=>server.close(r));}
