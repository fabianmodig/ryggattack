// Real production binary contract. Also reusable against a container via URLs.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import net from 'node:net';
import path from 'node:path';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const external=process.env.PRODUCTION_WS;
const child=external?null:spawn(path.resolve('target/debug/ryggattack-server'),['0.0.0.0:0'],{env:{...process.env,RYGGATTACK_MODE:'production',RYGGATTACK_ALLOWED_ORIGINS:'https://ryggattack.modig.online',RYGGATTACK_MANAGEMENT_BIND:'127.0.0.1:0'}});
let log=''; child?.stderr.on('data',b=>log+=b);
const sockets=[];
try {
 const urls=external?{ws:external,management:process.env.PRODUCTION_MANAGEMENT}:await new Promise((resolve,reject)=>{let text='';child.stdout.on('data',b=>{text+=b;const ws=text.match(/ws:\/\/0.0.0.0:(\d+)/),m=text.match(/management.*127.0.0.1:(\d+)/);if(ws&&m)resolve({ws:`ws://127.0.0.1:${ws[1]}/ws`,management:`http://127.0.0.1:${m[1]}`});});child.once('exit',code=>reject(Error(`production startup exited ${code}: ${log}`)));setTimeout(()=>reject(Error(`production startup timeout: ${log}`)),10000).unref();});
 async function upgrade(origin,route='/ws',extra='') {
  const u=new URL(urls.ws);const socket=net.connect(+u.port,u.hostname);sockets.push(socket);
  return await new Promise((r,j)=>{let bytes='';socket.on('error',j);socket.on('data',b=>{bytes+=b.toString();if(bytes.includes('\r\n\r\n')){r(bytes.split('\r\n')[0]);socket.destroy();}});socket.write(`GET ${route} HTTP/1.1\r\nHost: ${u.host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n${origin===undefined?'':`Origin: ${origin}\r\n`}${extra}\r\n`);setTimeout(()=>j(Error('upgrade timeout')),3000).unref();});
 }
 assert.match(await upgrade('https://ryggattack.modig.online'),/101/);
 assert.match(await upgrade(undefined),/101/,'native no-Origin allowed');
 for(const origin of ['null','https://evil.example','http://ryggattack.modig.online','https://ryggattack.modig.online:443','https://ryggattack.modig.online/','https://ryggattack.modig.online.evil'])assert.match(await upgrade(origin),/403/,origin);
 assert.match(await upgrade('https://ryggattack.modig.online','/ws','Origin: https://ryggattack.modig.online\r\n'),/403/);
 for(const route of ['/','/ws/','/ws?x=1','/metrics','/healthz'])assert.match(await upgrade(undefined,route),/404/);
 const ws=new WebSocket(urls.ws);sockets.push(ws);const messages=[];
 ws.addEventListener('message',e=>messages.push(JSON.parse(e.data)));await new Promise((r,j)=>{ws.addEventListener('open',r);ws.addEventListener('error',j);});
 ws.send(JSON.stringify({Hello:{version:3,name:'Production probe'}}));
 for(let i=0;i<100&&!messages.some(m=>m.Welcome);i++)await sleep(20);
 assert(messages.some(m=>m.Welcome),'production admits room');
 ws.send(JSON.stringify({Ready:true}));
 for(let i=0;i<200&&!messages.some(m=>m.State);i++)await sleep(20);
 assert(messages.some(m=>m.State),'real production room starts shared simulation before health/shutdown tests');
 const health=await fetch(urls.management+'/healthz');assert.equal(health.status,200);
 assert.equal((await fetch(urls.management+'/readyz')).status,200);
 const metrics=await (await fetch(urls.management+'/metrics')).text();
 assert.match(metrics,/ryggattack_rooms 1\n/);assert.match(metrics,/ryggattack_players 1\n/);assert.match(metrics,/ryggattack_ticks_total [1-9]/);assert.match(metrics,/ryggattack_connections 1\n/);
 assert.equal((await fetch(urls.management+'/')).status,404);
 const guesses=[];
 for(let i=0;i<65;i++){
  const attempt=new WebSocket(urls.ws);sockets.push(attempt);
  guesses.push(await new Promise((resolve,reject)=>{attempt.addEventListener('open',()=>attempt.send(JSON.stringify({Hello:{version:3,name:'Guess',room:'00000000000000000000'}})));attempt.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Error){resolve(m.Error);attempt.close();}});attempt.addEventListener('error',reject);setTimeout(()=>reject(Error('guess timeout')),3000).unref();}));
 }
 assert(guesses.some(s=>s.includes('rate limit')),'anonymous room-code guessing is globally rate limited');
 // Real spoofed proxy headers must not create a new admission identity/budget.
 for(const ip of ['192.0.2.1','192.0.2.2']){
  const u=new URL(urls.ws),s=net.connect(+u.port,u.hostname);sockets.push(s);
  await new Promise((resolve,reject)=>{let bytes='',sent=false;s.on('error',reject);s.on('data',b=>{bytes=(bytes+b.toString()).slice(-8192);if(!sent&&bytes.includes('\r\n\r\n')){assert.match(bytes,/101/);sent=true;const payload=Buffer.from(JSON.stringify({Hello:{version:3,name:'Proxy guess',room:'00000000000000000000'}})),mask=[1,2,3,4];assert(payload.length<126);s.write(Buffer.concat([Buffer.from([129,128|payload.length,...mask]),Buffer.from(payload.map((v,i)=>v^mask[i%4]))]));}if(bytes.includes('global admission rate limit')){s.destroy();resolve();}});s.write(`GET /ws HTTP/1.1\r\nHost: ${u.host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nX-Forwarded-For: ${ip}\r\nForwarded: for=${ip};proto=https\r\n\r\n`);setTimeout(()=>reject(Error('spoofed forwarded headers bypassed global admission limit')),3000).unref();});
 }
 const creations=[];
 for(let i=0;i<16;i++){
  const attempt=new WebSocket(urls.ws);sockets.push(attempt);
  creations.push(await new Promise((resolve,reject)=>{attempt.addEventListener('open',()=>attempt.send(JSON.stringify({Hello:{version:3,name:'Create'}})));attempt.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Error||m.Welcome){resolve(m.Error??'created');attempt.close();}});attempt.addEventListener('error',reject);setTimeout(()=>reject(Error('creation timeout')),3000).unref();}));
 }
 assert(creations.at(-1).includes('rate limit'),'anonymous room creation is globally rate limited');
 assert.equal((await fetch(urls.management+'/readyz')).status,200,'abuse does not stall authoritative tick');
 if(child){const start=performance.now();const exit=new Promise(r=>child.once('exit',(code,signal)=>r({code,signal})));child.kill('SIGTERM');const result=await exit;assert.equal(result.code,0);assert(performance.now()-start<2000,'bounded shutdown');await sleep(50);assert(messages.some(m=>m.Error?.includes('backend restarting')),'clear restart message');assert(!log.includes(messages.find(m=>m.Welcome).Welcome.credential),'safe logs');}
 console.log(JSON.stringify({checks:['explicit production bind','exact HTTPS Origin and native policy','only /ws upgrades','private health/readiness and aggregate metrics','anonymous creation/code guessing globally rate limited without stalling tick','spoofed Forwarded/X-Forwarded-For cannot evade global limits',...(child?['SIGTERM notifies connected clients and exits cleanly within 2s']:[])],logs:log},null,2));
} finally {for(const s of sockets){if(s instanceof net.Socket)s.destroy();else s.close();}child?.kill('SIGKILL');}
