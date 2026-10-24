import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {execFileSync} from 'node:child_process';
import {capturePrivateScreenshot,publishSoloEvidence} from '../../scripts/solo-bots-helpers.mjs';
const {chromium}=createRequire('/home/hermes/.npm/_npx/e41f203b7505f1fb/node_modules/playwright/package.json')('playwright');
const root=path.resolve('dist/web'),out=path.resolve('verification/t_84b7da26/offline');
assert(!fs.existsSync(out));fs.mkdirSync(out);
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const report={status:'running',scope:'One isolated actual WASM client; real keyboard menu/lobby/play/pause/resume; no backend, no injected state or clocks',source_revision:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),errors:[],served:{},screenshots:[],keys:[],sockets:0};
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
let browser,page;
const server=http.createServer((req,res)=>{
 let file;try{let p=decodeURIComponent(req.url.split('?')[0]);if(p.includes('\0'))throw Error();if(p.endsWith('/'))p+='index.html';file=path.resolve(root,'.'+p);}catch{res.writeHead(400).end();return;}
 if(!file.startsWith(root+path.sep)){res.writeHead(403).end();return;}
 fs.realpath(file,(err,real)=>{if(err){res.writeHead(404).end();return;}if(!real.startsWith(root+path.sep)){res.writeHead(403).end();return;}
 fs.readFile(real,(err,b)=>{if(err){res.writeHead(404).end();return;}const type={'.html':'text/html','.mjs':'text/javascript','.js':'text/javascript','.wasm':'application/wasm'}[path.extname(real)]??'application/octet-stream';report.served[path.relative(root,real)]={sha256:sha(b),bytes:b.length,mime:type};res.writeHead(200,{'content-type':type,'content-security-policy':"connect-src 'self'; object-src 'none'; base-uri 'self'"}).end(b);});});
});
try{
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const port=server.address().port;
 for(const [p,expected] of [['/%2e%2e/Cargo.toml',403],['/%00',400],['/%ZZ',400]]){
 const status=await new Promise((resolve,reject)=>http.get({host:'127.0.0.1',port,path:p},res=>{res.resume();resolve(res.statusCode);}).on('error',reject));assert.equal(status,expected);}
 browser=await chromium.launch({args:['--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist']});
 const context=await browser.newContext({viewport:{width:640,height:480}});
 await context.route('**/*',route=>{const u=new URL(route.request().url());if(u.origin===`http://127.0.0.1:${port}`||['data:','blob:'].includes(u.protocol))return route.continue();report.errors.push('blocked nonlocal request');return route.abort();});
 page=await context.newPage();page.on('pageerror',()=>report.errors.push('pageerror'));page.on('crash',()=>report.errors.push('crash'));page.on('console',m=>{if(m.type()==='error')report.errors.push('console error');});page.on('websocket',()=>report.sockets++);
 await page.goto(`http://127.0.0.1:${port}/?quality=0`,{waitUntil:'domcontentloaded'});
 await page.waitForFunction(()=>!document.getElementById('status'),null,{timeout:180000});await page.locator('canvas').focus();
 async function shot(label){const file=label+'.png';await capturePrivateScreenshot(page,new Set(),()=>page.screenshot({path:path.join(out,file),timeout:60000}));report.screenshots.push({file,sha256:sha(fs.readFileSync(path.join(out,file)))});}
 async function hold(key,ms=1800){report.keys.push({key,hold_ms:ms});await page.keyboard.down(key);await sleep(ms);await page.keyboard.up(key);await sleep(2500);}
 await shot('menu');await hold('Enter');await shot('lobby');await hold('w');await hold('Enter');await sleep(6000);await shot('playing');await hold('Space',3000);await shot('firing');await hold('Escape');await shot('pause');await hold('Escape');await shot('resumed');
 report.public_state=await page.evaluate(()=>{const p=JSON.parse(globalThis.ryggattackHud??'null');return{canvases:document.querySelectorAll('canvas').length,hud_phase:p?.lobby?.phase??null,hud_frame_present:!!p?.frame,hud_visible:[...document.querySelectorAll('.rygg-online-hud')].some(e=>e.getClientRects().length>0)};});
 assert.equal(report.sockets,0);assert.equal(report.public_state.canvases,1);assert.equal(report.public_state.hud_frame_present,false);assert.equal(report.errors.length,0);
 report.status='captured-review-required';
}catch(e){report.status='failed';report.failure=String(e.stack).replace(/https?:\/\/[^\s]+/g,'[endpoint omitted]');process.exitCode=1;}
finally{
 if(browser){await browser.close();report.browser_closed=true;}
 await new Promise(r=>server.close(r));report.http_closed=!server.listening;
 report.visual_claim='Screenshots require manual image review; key events alone do not establish game state.';
 if(!publishSoloEvidence(out,report,new Set()))process.exitCode=1;
 console.log(JSON.stringify({status:report.status,out}));
}
