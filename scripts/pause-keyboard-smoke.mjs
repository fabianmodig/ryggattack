// Focused real WASM/DOM pause regression; no game/selection-state injection.
import assert from 'node:assert/strict';
import {createServer,get} from 'node:http';
import {readFileSync,writeFileSync,mkdirSync,readdirSync} from 'node:fs';
import {resolve,sep,extname,relative} from 'node:path';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
const root=resolve(process.env.PAUSE_BUNDLE??'dist/web'),out=resolve(process.env.PAUSE_EVIDENCE??'verification/pause-keyboard');mkdirSync(out,{recursive:true});
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const files=dir=>readdirSync(dir,{withFileTypes:true}).flatMap(e=>e.isDirectory()?files(resolve(dir,e.name)):[resolve(dir,e.name)]);
const result={status:'running',started:new Date().toISOString(),verifier:hash(new URL(import.meta.url)),artifact:Object.fromEntries(files(root).map(p=>[relative(root,p),hash(p)])),steps:[],errors:[]};
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
 await context.addInitScript(()=>{
  Date.now=()=>3000;
  // Emulated hardware API only: selection/action still cross gilrs and ECS.
  globalThis.__qaPad=null;Object.defineProperty(navigator,'getGamepads',{value:()=>[globalThis.__qaPad]});
 });
 page=await context.newPage();page.setDefaultTimeout(90000);page.on('pageerror',e=>result.errors.push(String(e)));
 await page.goto(`http://127.0.0.1:${port}/?quality=0`,{waitUntil:'domcontentloaded'});
 await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='MainMenu');
 await page.locator('#play-local').tap();await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Lobby');
 await page.locator('#touch-join').tap();await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackLocalStatus??'null')?.joined);
 await page.locator('#touch-start').tap();await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='Playing');
 const observe=async(label)=>{result.steps.push(await page.evaluate(label=>({label,focus:document.activeElement?.id,selection:globalThis.ryggattackPauseSelection,domFocus:globalThis.ryggattackPauseDomFocus,local:JSON.parse(globalThis.ryggattackLocalStatus??'null')}),label));save();};
 const selected=async(action)=>{
  await page.waitForFunction(action=>globalThis.ryggattackPauseSelection===action&&globalThis.ryggattackPauseDomFocus==null&&document.activeElement?.id===`help-${action}`,action,{timeout:12000});
  // ECS publication leads the 100ms DOM presentation poll. Let that poll consume
  // this selection before issuing the next key, not an artificial same-frame burst.
  await page.waitForTimeout(250);
  assert.deepEqual(await page.evaluate(()=>[globalThis.ryggattackPauseSelection,document.activeElement?.id]),[action,`help-${action}`]);
 };
 const pause=async()=>{await page.waitForFunction(()=>!JSON.parse(globalThis.ryggattackLocalStatus??'null')?.finished);await page.locator('#touch-pause').tap();await page.locator('#offline-pause').waitFor({state:'visible'});await selected('resume');};
 const resumed=async()=>{await page.waitForFunction(()=>!JSON.parse(globalThis.ryggattackLocalStatus??'null')?.paused);await page.locator('#offline-pause').waitFor({state:'hidden'});assert.equal(await page.evaluate(()=>document.activeElement?.id),'ryggattack-canvas');};
 await pause();await observe('pause initially focuses resume');
 for(const [key,action] of [['s','back'],['w','resume'],['S','back'],['W','resume'],['ArrowDown','back'],['ArrowUp','resume'],['Tab','back'],['Shift+Tab','resume']]){
  await page.keyboard.press(key);await selected(action);await observe(key);
 }
 await page.screenshot({path:resolve(out,'ws-selected-resume.png'),timeout:90000});
 await page.keyboard.press('Enter');await resumed();await observe('Enter resumes');
 await pause();await page.keyboard.press('s');await selected('back');await page.keyboard.press('w');await selected('resume');
 await page.keyboard.press('Space');await resumed();await observe('Space resumes after S/W');
 await pause();await page.keyboard.press('s');await selected('back');await page.keyboard.press('Escape');await resumed();await observe('Escape resumes from back');
 await pause();
 await page.evaluate(()=>{globalThis.__qaPad={id:'QA standard gamepad',index:0,connected:true,mapping:'standard',axes:[0,0,0,0],buttons:Array.from({length:17},()=>({pressed:false,touched:false,value:0})),timestamp:performance.now()};});
 await page.waitForTimeout(1000);
 const pad=async(index,condition)=>{
  await page.evaluate(i=>{const p=globalThis.__qaPad;p.buttons[i]={pressed:true,touched:true,value:1};p.timestamp=performance.now();},index);
  await condition();
  await page.evaluate(i=>{const p=globalThis.__qaPad;p.buttons[i]={pressed:false,touched:false,value:0};p.timestamp=performance.now();},index);
  await page.waitForTimeout(400);
 };
 await pad(13,()=>selected('back'));await observe('gamepad D-pad down selects back in ECS and DOM');
 await pad(12,()=>selected('resume'));await observe('gamepad D-pad up selects resume in ECS and DOM');
 await pad(0,resumed);await observe('gamepad south resumes');
 await pad(9,async()=>{await page.locator('#offline-pause').waitFor({state:'visible'});await selected('resume');});await observe('gamepad Start pauses');
 await pad(1,resumed);await observe('gamepad east resumes');
 await pause();await page.keyboard.press('s');await selected('back');
 await page.screenshot({path:resolve(out,'ws-selected-back.png'),timeout:90000});
 await page.keyboard.press('Enter');await page.waitForFunction(()=>globalThis.ryggattackLocalScreen==='MainMenu');await page.locator('#offline-pause').waitFor({state:'hidden'});await observe('S then Enter returns to main menu');
 assert.deepEqual(result.errors,[]);result.status='passed';
}catch(error){result.status='failed';result.failure=String(error.stack??error);if(page){await observeFailure();}console.error(error);process.exitCode=1;}
finally{result.finished=new Date().toISOString();save();if(browser)await browser.close();await new Promise(r=>server.close(r));}
async function observeFailure(){try{result.failureState=await page.evaluate(()=>({focus:document.activeElement?.id,selection:globalThis.ryggattackPauseSelection,domFocus:globalThis.ryggattackPauseDomFocus,local:globalThis.ryggattackLocalStatus}));await page.screenshot({path:resolve(out,'failure.png'),timeout:90000});}catch(e){result.captureFailure=String(e);}}
