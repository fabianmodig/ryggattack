// Real image smoke; no mock HTTP/socket endpoints. ENGINE=podman locally, docker in CI.
import assert from 'node:assert/strict';
import {execFile,spawn} from 'node:child_process';
import {promisify} from 'node:util';
import fs from 'node:fs';
import crypto from 'node:crypto';
const exec=promisify(execFile),engine=process.env.ENGINE??'docker';
const [web,server]=process.argv.slice(2);assert(web&&server,'web and backend image required');
const names={web:`ryggattack-web-${process.pid}`,server:`ryggattack-server-${process.pid}`};
const cli=async(...args)=>(await exec(engine,args,{maxBuffer:1024*1024})).stdout.trim();
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const until=async(p,label)=>{for(let i=0;i<100;i++){try{if(await p())return;}catch{}await sleep(100);}throw Error(label);};
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const startBackend=async()=>await cli('run','-d','--name',names.server,'--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','-e','RYGGATTACK_MODE=production','-e','RYGGATTACK_ALLOWED_ORIGINS=https://ryggattack.modig.online','-e','RYGGATTACK_MANAGEMENT_BIND=0.0.0.0:8788','-p','127.0.0.1::8787','-p','127.0.0.1::8788',server);
try {
 await cli('run','-d','--name',names.web,'--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--tmpfs','/tmp:rw,noexec,nosuid,size=16m','-p','127.0.0.1::8080',web);
 await startBackend();
 const url=async(name,port)=>{const address=(await cli('port',name,`${port}/tcp`)).split('\n')[0];assert.match(address,/^127\.0\.0\.1:\d+$/,'container did not expose its expected loopback port');return 'http://'+address;};
 const webURL=await url(names.web,8080),wsURL=(await url(names.server,8787)).replace('http:','ws:')+'/ws',management=await url(names.server,8788);
 await until(async()=> (await fetch(webURL)).status===200,'web readiness');
 await until(async()=> (await fetch(management+'/readyz')).status===200,'backend readiness');
 assert((await (await fetch(webURL)).text()).includes('ryggattack-canvas'));
 for(const file of ['index.html','rooms.mjs','ryggattack.js','ryggattack_bg.wasm']){
  const r=await fetch(webURL+'/'+file);assert.equal(r.status,200);
  const type=r.headers.get('content-type');if(file.endsWith('.wasm'))assert.match(type,/application\/wasm/);if(file.endsWith('.mjs'))assert.match(type,/javascript/);
  assert.equal(sha(Buffer.from(await r.arrayBuffer())),sha(fs.readFileSync('dist/web/'+file)),`${file} exact image artifact`);
 }
 const privateQuery='phase5-query-must-not-log';await fetch(webURL+'/?name='+privateQuery+'&room=0123456789abcdef0123');await sleep(100);
 assert(!(await cli('logs',names.web)).includes(privateQuery),'web access logs must not disclose room/name queries');
 for(const route of ['/metrics','/healthz','/readyz','/ws'])assert.equal((await fetch(webURL+route)).status,404,'web must not expose backend/management');
 const result=await new Promise(r=>{const c=spawn(process.execPath,['scripts/production-smoke.mjs'],{env:{...process.env,PRODUCTION_WS:wsURL,PRODUCTION_MANAGEMENT:management},stdio:'inherit'});c.once('exit',r);});assert.equal(result,0);
 // Exercise the chart's actual Recreate contract. A fresh network namespace
 // avoids Podman/pasta's rapid same-port restart race without hiding app errors.
 await cli('stop','--time','10',names.server);await cli('rm',names.server);
 names.server=`ryggattack-server-recreated-${process.pid}`;await startBackend();
 const resetManagement=await url(names.server,8788),resetWS=(await url(names.server,8787)).replace('http:','ws:')+'/ws';
 await until(async()=> (await fetch(resetManagement+'/readyz')).status===200,'recreated backend ready');
 assert.match(await (await fetch(resetManagement+'/metrics')).text(),/ryggattack_rooms 0\n/);
 const ws=new WebSocket(resetWS),messages=[];let closed=false;
 ws.addEventListener('message',e=>messages.push(JSON.parse(e.data)));ws.addEventListener('close',()=>closed=true);
 await new Promise((r,j)=>{ws.addEventListener('open',r);ws.addEventListener('error',j);});ws.send(JSON.stringify({Hello:{version:3,name:'Shutdown probe'}}));
 await until(()=>messages.some(m=>m.Welcome),'shutdown client admitted');
 ws.send(JSON.stringify({Ready:true}));await until(()=>messages.some(m=>m.State),'shutdown client receives actual running simulation');
 const start=performance.now();await cli('stop','--time','10',names.server);assert(performance.now()-start<2000,'container bounded SIGTERM shutdown');
 await until(()=>closed,'shutdown closes client');assert(messages.some(m=>m.Error?.includes('backend restarting')));
 const inspect=JSON.parse(await cli('inspect',names.server))[0];assert.equal(inspect.State.ExitCode,0);
 const configs={};for(const [component,name] of Object.entries(names)){const i=JSON.parse(await cli('inspect',name))[0];assert(i.Config.User&&i.Config.User!=='0');assert(i.HostConfig.ReadonlyRootfs);configs[component]={image:i.Image,user:i.Config.User,readonly:i.HostConfig.ReadonlyRootfs,capDrop:i.HostConfig.CapDrop};}
 console.log(JSON.stringify({checks:['actual nonroot read-only cap-drop images','exact JS/WASM/HTML/module bytes and MIME','no public management routes','production socket/abuse contract','same-image Recreate resets all in-memory rooms','container SIGTERM notifies and closes real client with exit 0'],images:configs},null,2));
} finally {for(const name of Object.values(names))try{await cli('rm','-f',name);}catch{}}
