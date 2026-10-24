// Canonical HTTPS/WSS origin, resolved ONLY to loopback by Chromium. Self-signed
// fixture TLS is ignored locally; this is not public certificate/deployment proof.
import assert from 'node:assert/strict';
import https from 'node:https';import net from 'node:net';import fs from 'node:fs';import path from 'node:path';import crypto from 'node:crypto';
import {spawn,execFileSync} from 'node:child_process';import {createRequire} from 'node:module';
const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
const root=path.resolve('dist/web'),out=path.resolve(process.argv[2]??'verification/production-browser');fs.mkdirSync(out,{recursive:true});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const until=async(p,label,ms=120000)=>{const end=performance.now()+ms;while(!p()){assert(performance.now()<end,label);await sleep(25);}};
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const secrets=new Set(),clients=[],pipes=new Set(),blocked=new Set();
const evidence={artifact:Object.fromEntries(['index.html','rooms.mjs','ryggattack.js','ryggattack_bg.wasm'].map(f=>[f,sha(fs.readFileSync(path.join(root,f)))])),checks:[],errors:[],expected_outage_errors:[],origins:[]};
let server,backend,log='',outage=false;const browsers=[];let browserOptions;
execFileSync('openssl',['req','-x509','-newkey','rsa:2048','-nodes','-keyout',out+'/key.pem','-out',out+'/cert.pem','-days','1','-subj','/CN=ryggattack.modig.online','-addext','subjectAltName=DNS:ryggattack.modig.online'],{stdio:'ignore'});
const front=https.createServer({key:fs.readFileSync(out+'/key.pem'),cert:fs.readFileSync(out+'/cert.pem')},(req,res)=>{
 let file;try{let p=decodeURIComponent(req.url.split('?')[0]);if(p.includes('\0'))throw Error();if(p.endsWith('/'))p+='index.html';file=path.resolve(root,'.'+p);}catch{res.writeHead(400).end();return;}
 if(!file.startsWith(root+path.sep)){res.writeHead(403).end();return;}
 fs.readFile(file,(e,d)=>{if(e){res.writeHead(404).end();return;}res.writeHead(200,{'content-type':({'.html':'text/html','.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm'})[path.extname(file)]??'application/octet-stream'}).end(d);});
});
front.on('upgrade',(req,down,head)=>{
 const name=req.headers['user-agent'];evidence.origins.push({origin:req.headers.origin,path:req.url});
 if(req.url!=='/ws'||blocked.has(name)){down.destroy();return;}
 const up=net.connect(backend,'127.0.0.1');const p={name,up,down};pipes.add(p);
 const proxyEvent={client:name,at_ms:performance.now()};(evidence.proxy??=[]).push(proxyEvent);up.once('connect',()=>proxyEvent.connected_at_ms=performance.now());up.once('data',b=>{proxyEvent.response_line=/^HTTP\/1\.[01] [0-9]{3} [A-Za-z ]+\r\n/.exec(String(b))?.[0].trim()??'unrecognized HTTP status line';proxyEvent.response_at_ms=performance.now();});up.once('close',()=>proxyEvent.closed_at_ms=performance.now());up.once('error',e=>proxyEvent.error=e.message);
 for(const s of [up,down]){s.on('error',()=>{up.destroy();down.destroy();});s.on('close',()=>pipes.delete(p));}
 up.write(`${req.method} ${req.url} HTTP/${req.httpVersion}\r\n${Object.entries(req.headers).map(([k,v])=>`${k}: ${v}`).join('\r\n')}\r\n\r\n`);if(head.length)up.write(head);down.pipe(up);up.pipe(down);
});
async function expand(c){if(!await c.page.locator('#online-entry').evaluate(e=>e.open))await c.page.locator('#online-entry summary').click();}
async function pageFor(name,code){
 // Separate real clients must not share a SwiftShader GPU process: another
 // context's cold GPU initialization can starve open/Hello after HTTP 101.
 // Keep both renderers running together for every gameplay/recovery assertion.
 const browser=await chromium.launch(browserOptions);browsers.push(browser);
 const page=await (await browser.newContext({viewport:{width:640,height:480},ignoreHTTPSErrors:true,userAgent:name})).newPage();const c={name,page,welcomes:[],frames:[],inputs:[],events:[]};clients.push(c);
 await page.addInitScript(()=>{globalThis.productionSocketTrace=[];const Original=globalThis.WebSocket;globalThis.WebSocket=class extends Original{constructor(...args){super(...args);productionSocketTrace.push(['construct',performance.now()]);for(const event of ['open','close','error'])this.addEventListener(event,()=>productionSocketTrace.push([event,performance.now()]));this.addEventListener('message',e=>{try{productionSocketTrace.push(['receive',performance.now(),Object.keys(JSON.parse(e.data))[0]]);}catch{}});}send(data){try{productionSocketTrace.push(['send',performance.now(),Object.keys(JSON.parse(data))[0]]);}catch{}return super.send(data);}};});
 page.on('pageerror',e=>evidence.errors.push(`${name}: ${e.message}`));page.on('console',m=>{if(m.type()==='error'){if(outage&&m.text().includes('WebSocket'))evidence.expected_outage_errors.push(m.text());else evidence.errors.push(`${name}: ${m.text()}`);}});
 page.on('websocket',ws=>{assert.equal(ws.url(),'wss://ryggattack.modig.online/ws');ws.on('framereceived',({payload})=>{const m=JSON.parse(payload);if(m.Welcome){secrets.add(m.Welcome.credential);const {credential,...safe}=m.Welcome;c.identity=safe;c.welcomes.push(safe);}if(m.Lobby)c.lobby=m.Lobby;if(m.State){c.last=m.State;c.frames.push(m.State);}});ws.on('framesent',({payload})=>{const m=JSON.parse(payload);if(m.Input)c.inputs.push(m.Input);});});
 await page.goto(`https://ryggattack.modig.online/?quality=0${code?'&room='+code:''}`);
 await page.waitForFunction(()=>document.getElementById('backend').value==='wss://ryggattack.modig.online/ws');await expand(c);await page.locator('#player-name').fill(name);
 await page.locator(code?'#join-room':'#create-room').click();await until(()=>c.identity&&c.lobby,`${name} actual form admission`);
 await page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').lobby,null,{timeout:120000});return c;
}
try{
 server=spawn(path.resolve('target/debug/ryggattack-server'),['127.0.0.1:0'],{env:{...process.env,RYGGATTACK_MODE:'production',RYGGATTACK_ALLOWED_ORIGINS:'https://ryggattack.modig.online',RYGGATTACK_MANAGEMENT_BIND:'127.0.0.1:0'}});server.stderr.on('data',b=>log+=b);
 backend=await new Promise((r,j)=>{server.stdout.on('data',b=>{const m=String(b).match(/ws:\/\/127.0.0.1:(\d+)/);if(m)r(+m[1]);});server.once('error',j);setTimeout(()=>j(Error('backend startup')),10000).unref();});
 await new Promise(r=>front.listen(0,'127.0.0.1',r));
 browserOptions={args:[`--host-resolver-rules=MAP ryggattack.modig.online 127.0.0.1:${front.address().port}`,'--no-proxy-server','--ignore-certificate-errors','--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding']};
 const a=await pageFor('ProductionA'),b=await pageFor('ProductionB',a.lobby.code);
 assert.equal(a.identity.room,b.identity.room);assert.notEqual(a.identity.seat,b.identity.seat);
 await expand(a);await a.page.locator('#bots-room').fill('0');await a.page.locator('#target-room').selectOption('7');await a.page.locator('#apply-room').click();await until(()=>a.lobby.settings.bots===0,'host settings');
 await a.page.locator('#ready-room').click();await expand(b);await b.page.locator('#ready-room').click();await until(()=>a.last&&b.last,'production authoritative start');
 await a.page.waitForFunction(()=>globalThis.ryggattackOnline?.split('|')[1]>0,null,{timeout:120000});
 await a.page.screenshot({path:path.join(out,'same-origin-playing.png'),timeout:180000});
 await a.page.locator('canvas').focus();const beforeInputs=a.inputs.length;await a.page.keyboard.down('d');await a.page.keyboard.down('Space');await until(()=>a.inputs.slice(beforeInputs).some(i=>i.horizontal===1&&i.fire),'actual rendered keyboard intent');
 const identity=a.identity,previous=b.last.tick;outage=true;blocked.add(a.name);for(const p of pipes)if(p.name===a.name){p.up.destroy();p.down.destroy();}
 await until(()=>b.lobby.humans.some(h=>h.id===identity.participant&&!h.connected),'production bot takeover');await sleep(1500);assert(b.last.tick>previous+32,'healthy client advances during outage');
 blocked.delete(a.name);await until(()=>a.welcomes.length>=2,'production WSS secure reconnect');assert.equal(a.identity.participant,identity.participant);assert.equal(a.identity.seat,identity.seat);
 await a.page.waitForFunction(()=>JSON.parse(globalThis.ryggattackRoom??'{}').status?.startsWith('Online'),null,{timeout:120000});
 const restored=a.inputs.length;await until(()=>a.inputs.length>=restored+3,'post-reconnect input');assert(a.inputs.slice(restored).every(i=>i.horizontal===0&&i.vertical===0&&!i.fire));await a.page.keyboard.up('d');await a.page.keyboard.up('Space');
 await a.page.screenshot({path:path.join(out,'same-origin-reconnected.png'),timeout:180000});outage=false;
 const other=new Map(b.frames.map(f=>[f.tick,f]));const common=a.frames.filter(f=>other.has(f.tick));assert(common.length>8);for(const f of common)assert.deepEqual(f,other.get(f.tick));
 assert(evidence.origins.length>=3&&evidence.origins.every(r=>r.origin==='https://ryggattack.modig.online'&&r.path==='/ws'));
 evidence.identities={a:a.identity,b:b.identity};evidence.shared_identical_ticks=common.length;evidence.held_input_neutralization=true;evidence.checks=['actual canonical HTTPS location with local-only DNS map/self-signed fixture','default same-origin WSS /ws from real HTML create/join forms','two real rendered WASM clients configure/ready/start and send keyboard intent','authoritative common snapshots identical','real WSS disconnect -> bot takeover -> same private identity/seat secure reconnect; held controls neutralized'];
 evidence.browser_isolation='one Chromium/SwiftShader GPU process per rendered participant';
 evidence.lifecycle=[];for(const c of clients)evidence.lifecycle.push({name:c.name,...await c.page.evaluate(()=>({time_origin_ms:performance.timeOrigin,trace:productionSocketTrace.filter(([event,,kind])=>!['send','receive'].includes(event)||['Hello','Reconnect','Welcome','Error'].includes(kind))}))});
 // Release both software renderers before the offline game, just as the shared harness.
 for(const c of clients){await c.page.close();c.page=null;}
 assert.equal(evidence.errors.length,0,JSON.stringify(evidence.errors));
}catch(e){evidence.failure=String(e.stack??e);evidence.failure_state=[];for(const c of clients)if(c.page){try{evidence.failure_state.push({name:c.name,identity:c.identity,lobby:c.lobby,welcomes:c.welcomes,frames:c.frames.length,trace:await c.page.evaluate(()=>globalThis.productionSocketTrace??[]),status:await c.page.locator('#room-notice').innerText()});await c.page.screenshot({path:path.join(out,c.name+'-failure.png'),timeout:30000});}catch{}}throw e;
}finally{
 evidence.server_log=log;const text=JSON.stringify(evidence,null,2);assert([...secrets].every(t=>!text.includes(t)),'credential leaked');evidence.credential_negative_scan=true;fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(evidence,null,2));
 await Promise.all(browsers.map(b=>b.close()));for(const p of pipes){p.up.destroy();p.down.destroy();}await new Promise(r=>front.close(r));server?.kill('SIGTERM');
}
