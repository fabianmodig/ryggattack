// Socket abuse checks against the real local development binary (Node 22+).
import assert from 'node:assert/strict';
import net from 'node:net';
import path from 'node:path';
import {spawn} from 'node:child_process';
const server=spawn(path.resolve('target/debug/ryggattack-server'),['127.0.0.1:0']);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const results=[];
const sockets=[];
const rawSockets=[];
function maskedFrame(opcode, fin, payload=Buffer.alloc(0)) {
  const header=payload.length<126 ? Buffer.from([(fin?128:0)|opcode,128|payload.length,0,0,0,0]) : Buffer.from([(fin?128:0)|opcode,254,payload.length>>8,payload.length&255,0,0,0,0]);
  return Buffer.concat([header,payload]);
}
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
  const last=await open();await last.until(m=>m.Welcome);await last.until(m=>m.State?.tick>0);results.push('healthy client after abuse receives advancing state');last.ws.close();await sleep(100);
  // Raw masked RFC6455 frames, not a completed-message WebSocket API: empty
  // continuation floods must not hide inside tungstenite.read indefinitely.
  async function raw(origin) {
    const socket=net.connect(port,'127.0.0.1');rawSockets.push(socket);
    const state={socket,closed:false,header:''};
    socket.on('error',()=>{});socket.on('close',()=>state.closed=true);
    let header=Buffer.alloc(0), done=false;
    const response=new Promise((resolve,reject)=>{
      socket.on('data',data=>{if(done)return;header=Buffer.concat([header,data]);const end=header.indexOf('\r\n\r\n');if(end>=0){done=true;state.header=header.subarray(0,end).toString();resolve();}});
      socket.once('error',reject);setTimeout(()=>reject(Error('raw upgrade timeout')),2000).unref();
    });
    socket.write(`GET / HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n${origin===undefined?'':`Origin: ${origin}\r\n`}\r\n`);
    await response;return state;
  }
  async function untilClosed(c) {const end=performance.now()+2500;while(!c.closed&&performance.now()<end)await sleep(5);assert(c.closed,'raw attacker not disconnected by absolute fragment deadline');}
  const denied=await raw('https://evil.example');assert(denied.header.startsWith('HTTP/1.1 403'));results.push('raw foreign Origin rejected before upgrade');
  const healthy=await open();await healthy.until(m=>m.Welcome);
  async function admitted(origin) {
    const c=await raw(origin);assert(c.header.startsWith('HTTP/1.1 101'));
    const count=healthy.messages.length;
    c.socket.write(maskedFrame(1,true,Buffer.from(JSON.stringify({Hello:{version:1,name:'Raw attacker'}}))));
    const end=performance.now()+2500;
    while(!healthy.messages.slice(count).some(m=>m.State?.names[1]==='Raw attacker')){assert(performance.now()<end,'raw attacker not admitted');await sleep(5);}
    return c;
  }
  const allowed=await admitted('http://127.0.0.1:8000');allowed.socket.destroy();await sleep(100);results.push('raw exact allowed Origin admitted alongside native no-Origin client');
  const aggregate=await admitted();
  aggregate.socket.write(Buffer.concat([maskedFrame(1,false,Buffer.alloc(700,120)),maskedFrame(0,true,Buffer.alloc(700,120))]));
  await untilClosed(aggregate);await sleep(100);results.push('raw fragmented aggregate payload over 1 KiB rejected');
  for(const [name,chunk] of [['trickle',maskedFrame(0,false)],['flood',Buffer.concat(Array.from({length:4000},()=>maskedFrame(0,false)))]]) {
    const c=await admitted();const before=healthy.messages.length;
    c.socket.write(maskedFrame(1,false));
    const timer=setInterval(()=>{if(!c.closed&&c.socket.writableLength<128*1024)c.socket.write(chunk);},name==='flood'?1:100);
    try {await untilClosed(c);} finally {clearInterval(timer);}
    const frames=healthy.messages.slice(before).filter(m=>m.State);
    assert(frames.length>5,'raw fragments starved healthy replication');
    assert(frames.at(-1).State.tick-frames[0].State.tick>=32,'authoritative ticks did not advance during raw fragments');
    await sleep(100);results.push(`raw empty non-FIN ${name} bounded; healthy ticks advance`);
  }
  const replacement=await admitted();replacement.socket.destroy();healthy.ws.close();results.push('raw attacker seat reusable after fragment abuse');
  console.log(JSON.stringify({passed:results.length,checks:results},null,2));
} finally {for(const ws of sockets)ws.close();for(const socket of rawSockets)socket.destroy();server.kill();}
