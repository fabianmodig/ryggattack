// Real loopback service load without Chromium/GPU/compiler contention.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {spawn} from 'node:child_process';
const out=process.argv[2]??'verification/rooms-load.json';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const server=spawn(path.resolve('target/debug/ryggattack-server'),['127.0.0.1:0']);
const clients=[],results=[];let log='';server.stderr.on('data',b=>log+=b);
const until=async(p,label)=>{const end=performance.now()+15000;while(!p()){assert(performance.now()<end,label);await sleep(10);}};
try {
 const port=await new Promise((resolve,reject)=>{let text='';server.stdout.on('data',b=>{text+=b;const m=text.match(/ws:\/\/127\.0\.0\.1:(\d+)/);if(m)resolve(m[1]);});server.on('error',reject);});
 async function add(){
  const c={ws:new WebSocket(`ws://127.0.0.1:${port}`),pending:false,seq:0};clients.push(c);
  c.ws.addEventListener('open',()=>c.ws.send(JSON.stringify({Hello:{version:4,name:'Load human'}})));
  c.ws.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Welcome)c.identity=m.Welcome;
   if(m.Lobby){c.lobby=m.Lobby;const me=m.Lobby.humans.find(h=>h.id===c.identity.participant);if(me?.ready)c.pending=false;if(m.Lobby.phase==='Lobby'&&!me?.ready&&!c.pending){c.pending=true;c.ws.send(JSON.stringify({Ready:true}));}}
   if(m.State){c.last={tick:m.State.tick,at:performance.now()};c.first??=c.last;c.ws.send(JSON.stringify({Input:{sequence:++c.seq,horizontal:0,vertical:0,fire:true}}));}
   assert(!m.Error,JSON.stringify(m));
  });
  await until(()=>c.last,'room starts with real bots');return c;
 }
 await add();
 for(const count of [1,16]){
  while(clients.length<count)await add();
  if(count===16){
   const extra=new WebSocket(`ws://127.0.0.1:${port}`);let error;
   extra.addEventListener('open',()=>extra.send(JSON.stringify({Hello:{version:4,name:'Over capacity'}})));
   extra.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Error)error=m.Error;});
   await until(()=>error,'room capacity rejection');assert(error.includes('capacity'));extra.close();
  }
  // Measure only a steady window after admission/simulation construction.
  await sleep(1000);const starts=clients.map(c=>c.last),before=log.split('tick_overrun').length-1;
  const cpu=fs.readFileSync(`/proc/${server.pid}/stat`,'utf8').split(' ');
  const begin=performance.now();await sleep(20000);const elapsed=(performance.now()-begin)/1000;
  const cpuEnd=fs.readFileSync(`/proc/${server.pid}/stat`,'utf8').split(' ');
  const hz=clients.map((c,i)=>(c.last.tick-starts[i].tick)/((c.last.at-starts[i].at)/1000));
  assert(clients.every(c=>c.ws.readyState===WebSocket.OPEN),'load sockets remain admitted');
  assert(hz.every(v=>Number.isFinite(v)&&v>0),'all room clocks advance');
  results.push({rooms:count,humans:count,bots:count*3,wall_seconds:elapsed,observed_hz:hz,minimum_hz:Math.min(...hz),minimum_simulated_to_wall_ratio:Math.min(...hz)/64,overruns_during_window:log.split('tick_overrun').length-1-before,cpu_ticks:Number(cpuEnd[13])+Number(cpuEnd[14])-Number(cpu[13])-Number(cpu[14])});
 }
 console.log(JSON.stringify({results,server_log:log},null,2));
 fs.mkdirSync(path.dirname(out),{recursive:true});fs.writeFileSync(out,JSON.stringify({results,server_log:log},null,2));
}finally{for(const c of clients)c.ws.close();server.kill();}
