// Socket abuse checks against the real local development binary (Node 22+).
import assert from 'node:assert/strict';
import net from 'node:net';
import path from 'node:path';
import {spawn} from 'node:child_process';
const server=spawn(path.resolve('target/debug/ryggattack-server'),['127.0.0.1:0']);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const results=[];
const sockets=[];
try {
  const port=await new Promise((resolve,reject)=>{let s='';server.stdout.on('data',b=>{s+=b;const m=s.match(/ws:\/\/127.0.0.1:(\d+)/);if(m)resolve(+m[1]);});server.on('error',reject);setTimeout(()=>reject(Error('startup timeout')),5000).unref();});
  async function open(version=1) {
    const ws=new WebSocket(`ws://127.0.0.1:${port}`);sockets.push(ws);
    const messages=[];let closed=false;
    ws.addEventListener('message',e=>messages.push(JSON.parse(e.data)));
    ws.addEventListener('close',()=>closed=true);
    ws.addEventListener('error',()=>{});
    await new Promise((r,j)=>{ws.addEventListener('open',r,{once:true});setTimeout(()=>j(Error('connect timeout')),3000).unref();});
    ws.send(JSON.stringify({Hello:{version,name:'Socket test'}}));
    async function until(predicate){const deadline=performance.now()+4000;while(performance.now()<deadline){const value=messages.find(predicate);if(value)return value;if(closed&&predicate({closed:true}))return {closed:true};await sleep(5);}throw Error('message timeout');}
    return {ws,messages,until};
  }
  const half=net.connect(port,'127.0.0.1');
  const a=await open();const welcome=await a.until(m=>m.Welcome);assert.equal(welcome.Welcome.seat,0);half.destroy();results.push('half-open upgrade does not stall admission');
  a.ws.send(JSON.stringify({Input:{sequence:9,horizontal:0,vertical:1,fire:true}}));
  await a.until(m=>m.State?.ack[0]===9);
  const count=a.messages.length;
  a.ws.send(JSON.stringify({Input:{sequence:8,horizontal:-1,vertical:0,fire:false}}));
  await sleep(250);
  const newer=a.messages.slice(count).filter(m=>m.State);assert(newer.length>1);assert(newer.every(m=>m.State.ack[0]===9));results.push('stale input discarded');a.ws.close();await sleep(100);
  for(const [name,payload] of [
    ['seat spoof',JSON.stringify({Input:{sequence:1,horizontal:0,vertical:0,fire:false,seat:3}})],
    ['position spoof',JSON.stringify({Input:{sequence:1,horizontal:0,vertical:0,fire:false,position:[999,0,999]}})],
    ['axis bounds',JSON.stringify({Input:{sequence:1,horizontal:2,vertical:0,fire:false}})],
    ['oversize','x'.repeat(2048)],
    ['binary',new Uint8Array([1,2,3])],
  ]) {
    const c=await open();await c.until(m=>m.Welcome);c.ws.send(payload);await c.until(m=>m.Error||m.closed);c.ws.close();await sleep(80);results.push(`${name} rejected`);
  }
  const bad=await open(2);await bad.until(m=>m.Error);bad.ws.close();results.push('incompatible version rejected');
  const flood=await open();await flood.until(m=>m.Welcome);
  for(let sequence=1;sequence<=100;sequence++)flood.ws.send(JSON.stringify({Input:{sequence,horizontal:0,vertical:0,fire:false}}));
  await flood.until(m=>m.Error||m.closed);results.push('input flood disconnected');flood.ws.close();await sleep(100);
  const last=await open();await last.until(m=>m.Welcome);await last.until(m=>m.State?.tick>0);results.push('healthy client after abuse receives advancing state');last.ws.close();
  console.log(JSON.stringify({passed:results.length,checks:results},null,2));
} finally {for(const ws of sockets)ws.close();server.kill();}
