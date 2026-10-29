// Isolated loopback authority measurement; never prints Welcome/credentials.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {writeFile} from 'node:fs/promises';
const child=spawn('target/debug/ryggattack-server',['127.0.0.1:0']);
const peers=[];
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function waitFor(fn,ms=8000){const start=performance.now();while(performance.now()-start<ms){const result=fn();if(result)return result;await sleep(5);}throw Error('bounded baseline wait timed out');}
try {
 let address='';child.stdout.on('data',data=>{address+=(data.toString().match(/ws:\/\/127\.0\.0\.1:\d+/)||[''])[0];});
 child.stderr.resume();
 await waitFor(()=>address);
 async function peer(room){const ws=new WebSocket(address);const p={ws,frames:[],lobby:null,welcome:null};peers.push(p);
 ws.addEventListener('message',e=>{const m=JSON.parse(e.data);if(m.Welcome)p.welcome=m.Welcome;if(m.Lobby)p.lobby=m.Lobby;if(m.State)p.frames.push({received:performance.now(),frame:m.State});});
 await new Promise((resolve,reject)=>{ws.addEventListener('open',resolve,{once:true});ws.addEventListener('error',reject,{once:true});});
 ws.send(JSON.stringify({Hello:{version:4,name:'Baseline',room}}));await waitFor(()=>p.welcome);return p;}
 const host=await peer(null);const guest=await peer(host.welcome.code);
 host.ws.send(JSON.stringify({Configure:{bots:0,wins_target:7,arena:'Forest'}}));
 host.ws.send(JSON.stringify({Ready:true}));guest.ws.send(JSON.stringify({Ready:true}));
 await waitFor(()=>host.frames.length);
 const samples=[];const start=performance.now();
 for(let sequence=1;sequence<=12;sequence++){
  const sent=performance.now();host.ws.send(JSON.stringify({Input:{sequence,horizontal:sequence%2?1:0,vertical:0,fire:false}}));
  const ack=await waitFor(()=>host.frames.find(s=>s.received>=sent&&s.frame.ack[0]>=sequence));
  samples.push(ack.received-sent);await sleep(150);
 }
 const frames=host.frames.filter(s=>s.received>=start);assert(frames.length>15);
 const gaps=frames.slice(1).map((s,i)=>s.received-frames[i].received);
 const mean=a=>a.reduce((s,v)=>s+v,0)/a.length;
 const span=(frames.at(-1).received-frames[0].received)/1000;
 const result={base:'a48fbf5',scope:'isolated-loopback-transport-only; no renderer',frames:frames.length,observedServerHz:(frames.at(-1).frame.tick-frames[0].frame.tick)/span,snapshotGapMs:{min:Math.min(...gaps),mean:mean(gaps),max:Math.max(...gaps)},tickGaps:[...new Set(frames.slice(1).map((s,i)=>s.frame.tick-frames[i].frame.tick))],inputToSnapshotAckMs:{count:samples.length,min:Math.min(...samples),mean:mean(samples),max:Math.max(...samples)},notes:'ack is latest received held intent, not consumed client-step count; timing includes server batching and Node scheduling'};
 await writeFile('verification/prediction-baseline.json',JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));
} finally {for(const p of peers){p.ws.close();p.welcome=null;}child.kill();}
