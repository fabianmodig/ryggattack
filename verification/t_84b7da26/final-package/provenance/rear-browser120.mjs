// Local verification-only controller; no runtime or artifact edits.
import fs from 'node:fs';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const old='verification/t_84b7da26/rear-replay-provenance/executed.mjs';
let source=fs.readFileSync(old,'utf8');
assert.equal(sha(source),'33da12d04e2a1e59eb1f4a9da4b3a1d1fe09fe9755c38e329d44cc2591f935ca');
source=source.replace("'verification/rear-browser'","'verification/rear-browser120'");
source=source.replace('gameplay_wall_ms: 180000','gameplay_wall_ms: 480000');
source=source.replace(/report.replay=\{[^\n]+\};/,`report.replay={launcher_sha256:${JSON.stringify(sha(fs.readFileSync(import.meta.filename)))},previous_executed_sha256:${JSON.stringify(sha(source))},policy:'A neutral for first30s then cardinal every10s. B pursues point1.0 unit behind A using real keyboard diagonal/cardinal inputs, updated at most every750ms; no fire, no state injection. Independent480s contact-search budget; no production deadlines changed.'};`);
const start=source.indexOf('  let held=null,index=0,next=now();');
const end=source.indexOf('  report.rear_contacts=contacts();assert',start);
assert(start>0&&end>start);
source=source.slice(0,start)+`  const held=[new Set(),new Set()]; let next=now();
  async function steer(c,seat,keys){
    const wanted=new Set(keys);
    for(const key of held[seat])if(!wanted.has(key))await c.page.keyboard.up(key);
    for(const key of wanted)if(!held[seat].has(key))await c.page.keyboard.down(key);
    held[seat]=wanted;
  }
  const directions=['ArrowUp','ArrowRight','ArrowDown','ArrowLeft'];
  while(now()-gameplayStarted<480000){
    assert.equal(report.errors.length,0,'protocol/browser errors');
    if(contacts().length)break;
    if(now()>=next){
      const frame=b.latest,leader=frame.carts.find(c=>c.id===a.identity.seat),chaser=frame.carts.find(c=>c.id===b.identity.seat);
      const delta=leader.position.map((x,i)=>x-leader.forward[i]*1.0-chaser.position[i]);
      const keys=[];if(Math.abs(delta[0])>.4)keys.push(delta[0]>0?'ArrowRight':'ArrowLeft');if(Math.abs(delta[2])>.4)keys.push(delta[2]>0?'ArrowDown':'ArrowUp');
      await steer(b,1,keys);
      const elapsed=now()-gameplayStarted;
      await steer(a,0,elapsed<30000?[]:[directions[Math.floor((elapsed-30000)/10000)%4]]);
      next=now()+750;
    }
    await sleep(100);
  }
  for(let i=0;i<clients.length;i++)await steer(clients[i],i,[]);
`+source.slice(end);
const out='verification/t_84b7da26/rear120-provenance';
fs.mkdirSync(out,{recursive:true});
const executed=path.join(out,'executed.mjs');assert(!fs.existsSync(executed),'refuse overwriting executed harness');
fs.writeFileSync(executed,source);
fs.writeFileSync(path.join(out,'receipt.json'),JSON.stringify({launcher_sha256:sha(fs.readFileSync(import.meta.filename)),executed_sha256:sha(source),previous_executed_sha256:sha(fs.readFileSync(old)),base_harness_sha256:sha(fs.readFileSync('scripts/online-explosion-smoke.mjs'))},null,2)+'\n');
await import(pathToFileURL(path.resolve(executed)).href);
