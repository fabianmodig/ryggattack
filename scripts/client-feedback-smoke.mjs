// LOCAL ONLY: actual already-pulled published containers, one Chromium, sequential contexts.
// PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json node scripts/client-feedback-smoke.mjs --local
// No browser/build/deployment occurs for --help, missing authorization, or malformed arguments.
import assert from 'node:assert/strict';
import {openRoomMenu} from './menu-harness.mjs';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
import {parseLocalArgs,noticeMatches,requirePrivateScan,localPort,requireZeroCapabilities,localEngineEndpoint,requireLocalDockerEndpoint,localEngineArgs,usage} from './client-feedback-helpers.mjs';

const args=process.argv.slice(2);
if(args.length===1&&['--help','-h'].includes(args[0])) {console.log(usage);process.exit(0);}
let config;try {config=parseLocalArgs(args);}catch {console.error(usage);process.exit(2);}
assert.notEqual(process.env.NODE_TLS_REJECT_UNAUTHORIZED,'0','global TLS bypass forbidden');
const exec=promisify(execFile);
let engineEndpoint;
try {
  engineEndpoint=localEngineEndpoint(config.engine,process.env);
  if(config.engine==='docker'&&!engineEndpoint) {
    // Local config discovery only: this command never contacts a Docker daemon.
    // All daemon operations below instead use the validated, explicit --host.
    const result=await exec('docker',['context','inspect','--format','{{json .Endpoints.docker.Host}}'],{timeout:45000,maxBuffer:4*1024*1024});
    engineEndpoint=requireLocalDockerEndpoint(JSON.parse(result.stdout.trim()));
  }
} catch {
  // Never print inherited values or discovery output: either can contain secrets.
  console.error('Local engine authorization failed: Docker context override forbidden or invalid local endpoint/environment');process.exit(2);
}
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const origin='https://ryggattack.modig.online',backend=origin.replace('https:','wss:')+'/ws';
const run=new Date().toISOString().replace(/[:.]/g,'-')+'-'+crypto.randomBytes(4).toString('hex');
const out=path.join(repo,'verification/client-feedback',run);
fs.mkdirSync(out,{recursive:true});
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const cli=async(...argv)=>(await exec(config.engine,localEngineArgs(config.engine,engineEndpoint,argv),{timeout:45000,maxBuffer:4*1024*1024})).stdout.trim();
const start=performance.now(),elapsed=()=>Math.round(performance.now()-start),sleep=ms=>new Promise(r=>setTimeout(r,ms));
const names={web:`ryggattack-feedback-web-${process.pid}-${crypto.randomBytes(4).toString('hex')}`,server:`ryggattack-feedback-server-${process.pid}-${crypto.randomBytes(4).toString('hex')}`};
const owned=new Set(),secrets=new Set(),peers=new Set(),pipes=new Set(),frontSockets=new Set();
const evidence={status:'running',started_at:new Date().toISOString(),scope:'isolated rootless actual published images; canonical origin mapped only to loopback; no public traffic, deployment, builds, state/packet injection',budgets_ms:{assertion:config.timeoutMs,startup:config.startupMs,screenshot:60000},source_sha256:sha(fs.readFileSync(fileURLToPath(import.meta.url))),images:{},assets:{},checks:[],screenshots:[],proxy:[],page_errors:[],cleanup:{}};
let browser,context,page,client,front,fixtureDir,webPort,serverPort,managementPort,blockBackend=false,interrupted=false;
let scenario='setup';
const interrupt=()=>{interrupted=true;};process.on('SIGINT',interrupt);process.on('SIGTERM',interrupt);
async function until(fn,label,ms=config.timeoutMs) {
  const end=performance.now()+ms;while(!await fn()) {assert(!interrupted,'interrupted');assert(performance.now()<end,label);await sleep(40);}
}
async function ready(port) {
  await until(async()=>{try{return (await fetch(`http://127.0.0.1:${port}/readyz`,{signal:AbortSignal.timeout(1500)})).status===200;}catch{return false;}},'isolated backend readiness',config.startupMs);
}
const inspect=async name=>JSON.parse(await cli('inspect',name))[0];
async function publication(name,port) {return localPort(await cli('port',name,`${port}/tcp`));}
const hardened=['--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--pids-limit=128','--memory=512m','--cpus=2'];
async function verifyContainer(name,expectedImage) {
  const c=await inspect(name);assert.equal(c.Image.replace(/^sha256:/,''),expectedImage.replace(/^sha256:/,''));
  assert(c.Config.User&&!['0','root'].includes(c.Config.User.split(':')[0]),'nonroot image required');
  assert(c.HostConfig.ReadonlyRootfs);assert(c.HostConfig.CapDrop.length>0);
  // Podman expands ALL into individual capability names; verify kernel state,
  // rather than weakening the assertion to one daemon's inspect spelling.
  const capabilities=requireZeroCapabilities(await cli('exec',name,'cat','/proc/1/status'));
  assert(c.HostConfig.SecurityOpt.some(x=>x.startsWith('no-new-privileges')));
  return {image_id:c.Image,user:c.Config.User,readonly:true,cap_drop_all:true,capabilities,no_new_privileges:true};
}
async function startBackend() {
  owned.add(names.server);
  await cli('run','--pull=never','-d','--name',names.server,...hardened,
    '-e','RYGGATTACK_MODE=production','-e',`RYGGATTACK_ALLOWED_ORIGINS=${origin}`,'-e','RYGGATTACK_MANAGEMENT_BIND=0.0.0.0:8788',
    '-p','127.0.0.1::8787','-p','127.0.0.1::8788',evidence.images.server.id);
  serverPort=await publication(names.server,8787);managementPort=await publication(names.server,8788);
  await ready(managementPort);return verifyContainer(names.server,evidence.images.server.id);
}
function discover(m) {
  for(const v of [m?.Welcome?.credential,m?.Reconnect?.credential])if(typeof v==='string'&&v)secrets.add(v);
}
function received(c,m) {
  discover(m);
  if(m.Welcome) {c.welcomes++;c.identity=Object.fromEntries(['version','room','code','seat','participant'].map(k=>[k,m.Welcome[k]]));}
  if(m.Lobby)c.lobby=m.Lobby;
  if(m.Error)c.errors.push({text:String(m.Error),at_ms:elapsed()});
  if(m.Notice)c.notices.push({text:String(m.Notice),at_ms:elapsed()});
}
async function rawJoin(code=null) {
  const ws=new WebSocket(`ws://127.0.0.1:${serverPort}/ws`);peers.add(ws);
  const c={ws,welcomes:0,errors:[],notices:[],closed:false};
  ws.addEventListener('open',()=>ws.send(JSON.stringify({Hello:{version:4,name:'FeedbackCapacity',room:code}})));
  ws.addEventListener('message',e=>{try{received(c,JSON.parse(String(e.data)));}catch{c.invalid=true;}});
  ws.addEventListener('error',()=>{c.transportError=true;});ws.addEventListener('close',()=>c.closed=true);
  await until(()=>c.identity&&c.lobby,'real raw peer admission');assert(!c.invalid&&!c.transportError);
  if(code)assert.equal(c.lobby.code,code);return c;
}
async function closePeers() {
  for(const ws of peers)if(ws.readyState===WebSocket.OPEN){ws.send(JSON.stringify('Leave'));ws.close();}else if(ws.readyState===WebSocket.CONNECTING)ws.close();
  peers.clear();
}
async function expand() {
  await openRoomMenu(page);
  assert(await page.locator('#online-entry').isVisible(),'integrated menu is visible');
}
async function claims() {return page.evaluate(()=>Object.keys(sessionStorage).filter(k=>k.startsWith('ryggattack.claim.')).length);}
async function screenshot(label) {
  const filename=label+'.png',before=elapsed();
  // Only public DOM text is captured; never storage, console dumps, or raw Welcome.
  const publicText=await page.locator('body').innerText();requirePrivateScan(publicText,secrets);
  await page.screenshot({path:path.join(out,filename),timeout:60000});
  const b=fs.readFileSync(path.join(out,filename));assert(b.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])));
  assert(b.readUInt32BE(16)>0&&b.readUInt32BE(20)>0);requirePrivateScan(b.toString('latin1'),secrets);
  const record={file:filename,sha256:sha(b),bytes:b.length,width:b.readUInt32BE(16),height:b.readUInt32BE(20),at_ms:before,duration_ms:elapsed()-before};
  evidence.screenshots.push(record);return record;
}
async function visibleFeedback(label,pattern,{packet=null,recovery=true}={}) {
  await expand();
  // First wait for actual DOM text, then scroll the real notice into the viewport.
  // A status-only diagnostic or an unrelated s.notice cannot satisfy this assertion.
  const expression={source:pattern.source,flags:pattern.flags};
  await page.waitForFunction(({source,flags})=>new RegExp(source,flags).test(document.getElementById('room-notice')?.textContent??''),expression,{timeout:config.timeoutMs});
  await page.locator('#room-notice').scrollIntoViewIfNeeded();
  await page.waitForFunction(noticeMatches,expression,{timeout:config.timeoutMs});
  assert(await page.locator('#room-notice').isVisible());
  const text=(await page.locator('#room-notice').innerText()).trim();assert.match(text,pattern);
  if(packet!==null)assert.equal(text,packet,'terminal server Error must not be masked by a stale Notice');
  const actions=[];
  if(recovery)for(const [selector,caption] of [['#create-room','CREATE PRIVATE ROOM'],['#join-room','JOIN ROOM']]) {
    const button=page.locator(selector);assert(await button.isVisible());assert(await button.isEnabled());assert.equal((await button.innerText()).trim(),caption);actions.push(caption);
  }
  // Also require backend/name/code controls remain usable; the buttons are real recovery actions.
  if(recovery)for(const id of ['backend','player-name','room-code'])assert(await page.locator('#'+id).isEnabled());
  await page.locator('#room-notice').scrollIntoViewIfNeeded();
  await page.waitForFunction(noticeMatches,expression,{timeout:config.timeoutMs});
  const shot=await screenshot(label);
  const proof={scenario:label,notice:text,visible:true,details_open:true,actions,at_ms:elapsed(),screenshot:shot.file};
  evidence.checks.push(proof);return proof;
}
async function newRendered(label,code=null) {
  // Exactly one browser process and at most one rendered context/page, sequentially.
  if(context)await context.close();context=await browser.newContext({viewport:{width:800,height:700},ignoreHTTPSErrors:true});
  page=await context.newPage();page.setDefaultTimeout(config.timeoutMs);page.setDefaultNavigationTimeout(config.timeoutMs);
  client={label,welcomes:0,errors:[],notices:[],closes:[],handshakes:[],socketAttempts:0};const c=client;
  // This route guard is defense in depth: no external request can reach public DNS/network.
  await context.route('**/*',async route=>{
    const u=new URL(route.request().url());if(u.origin!==origin){evidence.page_errors.push({scenario:label,kind:'external-request-blocked'});await route.abort();return;}await route.continue();
  });
  page.on('pageerror',e=>evidence.page_errors.push({scenario:label,message:e.message}));
  page.on('crash',()=>evidence.page_errors.push({scenario:label,message:'renderer crash'}));
  page.on('websocket',ws=>{
    c.socketAttempts++;if(ws.url()!==backend){evidence.page_errors.push({scenario:label,message:'unexpected WebSocket URL'});return;}
    ws.on('framereceived',({payload})=>{try{received(c,JSON.parse(String(payload)));}catch{evidence.page_errors.push({scenario:label,message:'invalid frame'});}});
    ws.on('framesent',({payload})=>{try{const m=JSON.parse(String(payload));discover(m);if(m.Hello)c.handshakes.push({kind:'Hello',room:m.Hello.room??null,at_ms:elapsed()});if(m.Reconnect)c.handshakes.push({kind:'Reconnect',room:m.Reconnect.room,at_ms:elapsed()});}catch{evidence.page_errors.push({scenario:label,message:'invalid sent frame'});}});
    ws.on('close',()=>c.closes.push(elapsed()));
  });
  const navigation=await page.goto(origin+'/?quality=0',{waitUntil:'domcontentloaded'});
  assert.equal(navigation.headers()['x-ryggattack-local-fixture'],run,'prove this page came from our isolated loopback fixture before sending any game command');
  await page.waitForFunction(expected=>document.getElementById('backend')?.value===expected,backend,{timeout:config.timeoutMs});
  assert.equal(await claims(),0,'fresh context starts with no private claims');await expand();
  await page.locator('#player-name').fill('Feedback'+label);
  if(code)await page.locator('#room-code').fill(code);
  await page.locator(code?'#join-room':'#create-room').click();
  await until(()=>c.handshakes.some(h=>h.kind==='Hello'),'actual rendered HTML form sends Hello');
  assert(c.handshakes.every(h=>h.kind==='Hello'&&h.room===(code??null)),'fresh admission must not reclaim another context');
  return c;
}
async function admitted(c) {
  await until(()=>c.identity&&c.lobby,'actual rendered admission');
  await visibleFeedback(c.label+'-online',/^Online\b/i,{recovery:false});
  await page.waitForFunction(code=>document.getElementById('room-info')?.textContent.includes(code),c.identity.code,{timeout:config.timeoutMs});
  assert.equal(await page.locator('canvas').count(),1);await until(async()=>await claims()===1,'real private session claim saved');
}
function closePipes() {for(const p of pipes){p.up.destroy();p.down.destroy();}}
async function setupFront() {
  const privateRoot=process.env.TMPDIR??path.join(repo,'verification','.private-fixtures');fs.mkdirSync(privateRoot,{recursive:true,mode:0o700});
  if(path.resolve(privateRoot).startsWith(repo+path.sep))await exec('git',['check-ignore','-q',privateRoot+'/key.pem'],{cwd:repo});
  fixtureDir=fs.mkdtempSync(path.join(privateRoot,'ryggattack-feedback-tls-'));fs.chmodSync(fixtureDir,0o700);
  await exec('openssl',['req','-x509','-newkey','rsa:2048','-nodes','-keyout',fixtureDir+'/key.pem','-out',fixtureDir+'/cert.pem','-days','1','-subj','/CN=ryggattack.modig.online','-addext','subjectAltName=DNS:ryggattack.modig.online'],{timeout:15000});
  fs.chmodSync(fixtureDir+'/key.pem',0o600);
  front=https.createServer({key:fs.readFileSync(fixtureDir+'/key.pem'),cert:fs.readFileSync(fixtureDir+'/cert.pem')},(req,res)=>{
    if(req.url==='/ws'){res.writeHead(426).end();return;}
    const up=http.request({host:'127.0.0.1',port:webPort,path:req.url,method:req.method,headers:{...req.headers,host:`127.0.0.1:${webPort}`}},r=>{res.writeHead(r.statusCode,{...r.headers,'x-ryggattack-local-fixture':run});r.pipe(res);});
    up.on('error',()=>{if(!res.headersSent)res.writeHead(502);res.end();});req.on('aborted',()=>up.destroy());res.on('close',()=>up.destroy());req.pipe(up);
  });
  front.on('connection',s=>{frontSockets.add(s);s.on('close',()=>frontSockets.delete(s));});
  front.on('upgrade',(req,down,head)=>{
    const event={scenario,path:req.url,origin:req.headers.origin,blocked:blockBackend,at_ms:elapsed()};evidence.proxy.push(event);
    if(req.url!=='/ws'||req.headers.origin!==origin){evidence.page_errors.push({scenario,message:'noncanonical upgrade rejected'});down.destroy();return;}
    if(blockBackend){down.destroy();return;}
    const up=net.connect(serverPort,'127.0.0.1'),p={up,down};pipes.add(p);
    for(const s of [up,down])s.on('error',()=>{up.destroy();down.destroy();});
    // Let upstream EOF flush pending Error/Close bytes through pipe's normal end.
    // Destroying downstream immediately on upstream close can truncate graceful feedback.
    down.on('close',()=>{up.destroy();pipes.delete(p);});
    up.once('connect',()=>{
      // Preserve exact original /ws upgrade and canonical Origin, no synthetic frames.
      up.write(`${req.method} ${req.url} HTTP/${req.httpVersion}\r\n${req.rawHeaders.reduce((a,v,i)=>i%2?a:a+`${v}: ${req.rawHeaders[i+1]}\r\n`,'')}\r\n`);
      if(head.length)up.write(head);down.pipe(up);up.pipe(down);
    });
  });
  await new Promise((resolve,reject)=>{front.once('error',reject);front.listen(0,'127.0.0.1',resolve);});
}

try {
  // Resolve/verify immutable local image IDs before any container/browser action. Never pull.
  const info=JSON.parse(await cli('info','--format',config.engine==='podman'?'json':'{{json .}}'));
  const rootless=info.host?.security?.rootless??info.Host?.Security?.Rootless??info.SecurityOptions?.some(s=>s.includes('rootless'));
  assert.equal(rootless,true,'use a local rootless container engine');evidence.rootless=true;
  const paired=JSON.parse(fs.readFileSync(path.join(repo,'verification/paired-images.json'),'utf8'));
  for(const [role,image] of Object.entries({web:config.webImage,server:config.serverImage})) {
    const i=JSON.parse(await cli('image','inspect',image))[0];const id=i.Id??i.ID;assert(id,'image must already exist locally');
    evidence.images[role]={reference:image,id,repo_digests:i.RepoDigests??[]};
    if(image===paired[role].image){assert.equal(id.replace(/^sha256:/,''),paired[role].id);assert((i.RepoDigests??[]).some(d=>d.endsWith('@'+paired[role].manifest)),'published manifest differs from paired-image proof');evidence.images[role].paired_manifest=paired[role].manifest;}
  }
  owned.add(names.web);await cli('run','--pull=never','-d','--name',names.web,...hardened,'--tmpfs','/tmp:rw,noexec,nosuid,size=16m','-p','127.0.0.1::8080',evidence.images.web.id);
  webPort=await publication(names.web,8080);evidence.web_container=await verifyContainer(names.web,evidence.images.web.id);
  evidence.server_container=await startBackend();
  for(const file of ['index.html','rooms.mjs','ryggattack.js','ryggattack_bg.wasm']) {
    const r=await fetch(`http://127.0.0.1:${webPort}/${file}`,{redirect:'error',signal:AbortSignal.timeout(config.startupMs)});assert.equal(r.status,200);
    const b=Buffer.from(await r.arrayBuffer());assert(b.length);if(file.endsWith('.wasm'))assert(b.subarray(0,4).equals(Buffer.from([0,97,115,109])));
    evidence.assets[file]={sha256:sha(b),bytes:b.length,mime:r.headers.get('content-type')};
  }
  await setupFront();
  const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES??import.meta.url)('playwright');
  browser=await chromium.launch({args:[`--host-resolver-rules=MAP ryggattack.modig.online 127.0.0.1:${front.address().port}`,'--no-proxy-server','--ignore-certificate-errors','--use-angle=swiftshader','--enable-unsafe-swiftshader','--ignore-gpu-blocklist','--disable-backgrounding-occluded-windows','--disable-renderer-backgrounding']});
  evidence.browser_version=browser.version();evidence.browser_isolation='one fresh Chromium; sequential fresh contexts; locally ignored self-signed fixture certificate only';

  scenario='graceful-restart';const a=await newRendered('Restart');await admitted(a);const oldCode=a.identity.code;
  // Obtain a real server Notice through ordinary UI, so stale s.notice cannot fake a pass.
  await expand();await page.locator('#bots-room').fill('0');await page.locator('#apply-room').click();
  await until(()=>a.lobby.settings.bots===0,'actual UI settings applied');await page.locator('#ready-room').click();
  await until(()=>a.notices.some(n=>/solo practice is offline/.test(n.text)),'actual server nonterminal Notice');
  await visibleFeedback('pre-restart-notice',/solo practice is offline/i,{recovery:false});
  const validInvite=await rawJoin(oldCode);assert.equal(validInvite.identity.room,a.identity.room);
  evidence.formerly_valid_invite={code:oldCode,admitted_before_restart:true,room:a.identity.room};
  assert(await page.locator('#room-notice').isVisible(),'integrated room menu exposes notices directly');
  const stoppedAt=elapsed();await cli('stop','--time','10',names.server);
  const stopped=await inspect(names.server);assert.equal(stopped.State.ExitCode,0);assert.equal(stopped.State.Running,false);
  await until(()=>a.errors.some(e=>/backend restarting.*rooms reset.*join a lobby again/i.test(e.text))&&a.closes.length,'actual graceful Error and WebSocket close');
  const restartError=a.errors.find(e=>/backend restarting/i.test(e.text));
  assert(restartError.at_ms>=stoppedAt);await visibleFeedback('graceful-restart',/backend restarting.*rooms reset.*join a lobby again/i,{packet:restartError.text});
  await until(async()=>await claims()===0,'graceful restart clears saved private claims');
  evidence.restart={exit_code:0,error:restartError.text,transport_closed:true,claims_cleared:true,preexisting_notice_cleared:true,duration_ms:elapsed()-stoppedAt};
  await context.close();context=null;page=null;await closePeers();
  await cli('rm',names.server);owned.delete(names.server);
  evidence.recreated_server=await startBackend();assert.equal(evidence.recreated_server.image_id,evidence.server_container.image_id);
  const metrics=await (await fetch(`http://127.0.0.1:${managementPort}/metrics`,{signal:AbortSignal.timeout(config.startupMs)})).text();assert.match(metrics,/ryggattack_rooms 0\n/);

  scenario='formerly-valid-stale-invite';const stale=await newRendered('Stale',oldCode);
  await until(()=>stale.errors.some(e=>/room not found|room closed/i.test(e.text)),'formerly valid invitation rejected after same-image Recreate');
  assert.equal(stale.welcomes,0,'stale join must not silently create another room');
  await visibleFeedback('formerly-valid-stale-invite',/room not found|room closed/i,{packet:stale.errors.at(-1).text});assert.equal(await claims(),0);
  evidence.formerly_valid_invite.rejected_after_same_image_recreate=true;
  await context.close();context=null;page=null;

  scenario='full-room';const cap=await rawJoin();const capacity=[cap];for(let i=0;i<3;i++)capacity.push(await rawJoin(cap.lobby.code));
  await until(()=>cap.lobby.humans.length===4&&cap.lobby.humans.every(h=>h.connected),'four real raw clients occupy the actual room');
  assert.equal(new Set(capacity.map(c=>c.identity.seat)).size,4);
  const full=await newRendered('Full',cap.lobby.code);
  await until(()=>full.errors.some(e=>/room full/i.test(e.text)),'actual fifth client gets room full');assert.equal(full.welcomes,0);
  await visibleFeedback('full-room',/room full/i,{packet:full.errors.at(-1).text});assert.equal(await claims(),0);
  evidence.capacity={raw_connected_humans:4,distinct_seats:4,rendered_fifth_rejected:true};
  await context.close();context=null;page=null;await closePeers();

  scenario='connection-failure';const unavailable=await newRendered('Unavailable');await admitted(unavailable);
  const blockedAt=elapsed();blockBackend=true;closePipes();
  // Disconnect an admitted real client, preserving production retry deadlines and private claim.
  // No synthetic Error packet or modified browser WebSocket is used.
  await visibleFeedback('connection-retrying',/^Retrying\b/i,{recovery:false});
  await visibleFeedback('connection-unavailable',/Backend unavailable.*leave and join a lobby again/i);
  const attempts=evidence.proxy.filter(e=>e.scenario===scenario&&e.blocked&&e.at_ms>=blockedAt);
  assert(attempts.length>=2,'genuine bounded retry must make multiple blocked real upgrades');
  assert.equal(unavailable.welcomes,1,'unavailable backend cannot fake another Welcome');
  evidence.connection_failure={cause:'owned local proxy destroys real backend upgrades and established TCP sockets',blocked_upgrade_attempts:attempts.length,elapsed_ms:elapsed()-blockedAt,production_deadlines_unchanged:true};
  assert(evidence.proxy.length>0&&evidence.proxy.every(e=>e.path==='/ws'&&e.origin===origin));
  assert.equal(evidence.page_errors.length,0,'unexpected renderer/protocol/egress errors');
  assert(['graceful-restart','formerly-valid-stale-invite','full-room','connection-unavailable'].every(s=>evidence.checks.some(c=>c.scenario===s)));
  evidence.status='passed';
} catch(e) {
  evidence.status='failed';evidence.failure={scenario,message:String(e.message??e)};
  if(page&&!page.isClosed())try {
    await expand();evidence.failure.dom_notice=(await page.locator('#room-notice').innerText()).trim();
    evidence.failure.safe_protocol=client?{welcomes:client.welcomes,errors:client.errors,notices:client.notices,handshakes:client.handshakes,socket_attempts:client.socketAttempts}:null;
    await screenshot('failure');
  }catch {evidence.failure.screenshot_unavailable=true;}
} finally {
  // Cleanup is limited to the random container names and sockets created by this run.
  // Connection-failure intentionally cannot send Leave: disposal removes only our own room store.
  try {await closePeers();}catch {evidence.cleanup.raw_peers_failed=true;evidence.status='failed';}
  try {if(context)await context.close();if(browser)await browser.close();evidence.cleanup.browser_closed=true;}catch {evidence.status='failed';evidence.cleanup.browser_failed=true;}
  closePipes();for(const s of frontSockets)s.destroy();if(front)await new Promise(r=>front.close(r));
  for(const name of owned)try {await cli('rm','-f',name);await assert.rejects(()=>cli('inspect',name),/no such (?:container|object)|does not exist/i);evidence.cleanup[name]='removed-and-verified';}catch {evidence.cleanup[name]='failed';evidence.status='failed';}
  if(fixtureDir){fs.rmSync(fixtureDir,{recursive:true,force:true});evidence.cleanup.private_tls_fixture_removed=!fs.existsSync(fixtureDir);}
  evidence.finished_at=new Date().toISOString();evidence.total_ms=elapsed();
  // Scan all on-disk artifacts and the complete serialized evidence BEFORE publishing results.
  try {
    const text=JSON.stringify(evidence,null,2);requirePrivateScan(text,secrets);
    for(const f of fs.readdirSync(out))requirePrivateScan(fs.readFileSync(path.join(out,f)).toString('latin1'),secrets);
    evidence.private_material_negative_scan={passed:true,known_values_checked:secrets.size};
    fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(evidence,null,2)+'\n');
    const files=fs.readdirSync(out).map(file=>({file,sha256:sha(fs.readFileSync(path.join(out,file)))}));
    fs.writeFileSync(path.join(out,'sha256.json'),JSON.stringify(files,null,2)+'\n');
    console.log(JSON.stringify({status:evidence.status,evidence_directory:out,checks:evidence.checks.map(c=>c.scenario),failure:evidence.failure?.message}));
  }catch {
    for(const f of fs.readdirSync(out))fs.rmSync(path.join(out,f),{force:true});
    fs.writeFileSync(path.join(out,'results.json'),JSON.stringify({status:'failed',failure:'private-material scan failed; artifacts withheld'}));evidence.status='failed';
  }
  process.removeListener('SIGINT',interrupt);process.removeListener('SIGTERM',interrupt);
  if(evidence.status!=='passed')process.exitCode=1;
}
