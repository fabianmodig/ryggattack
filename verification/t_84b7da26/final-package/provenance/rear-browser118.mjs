// Controlled local rear-contact replay. Reuses reviewed harness transport/privacy/cleanup.
import fs from 'node:fs';
import crypto from 'node:crypto';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const repo=process.cwd();
const base=fs.readFileSync('scripts/online-explosion-smoke.mjs','utf8');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
let source=base.replaceAll(/from '(\.\/[^']+)'/g,(_,p)=>`from '${pathToFileURL(path.resolve('scripts',p)).href}'`);
source=source.replace("const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');",`const repo = ${JSON.stringify(repo)};`);
source=source.replace("'verification/online-explosion'","'verification/rear-browser'");
source=source.replace("assert(input.horizontal === 0 && input.vertical === 0, 'neutral steering throughout');","assert([-1,0,1].includes(input.horizontal) && [-1,0,1].includes(input.vertical) && !input.fire, 'legal directional keys and no fire');");
const begin=source.indexOf('async function run() {'),end=source.indexOf('\nasync function leave(c)',begin);
if(begin<0||end<0)throw Error('missing reviewed boundaries');
const run=String.raw`async function run() {
  report.scope='LOCAL two actual rendered WASM humans; legal UI and directional keyboard only; no fire or state injection';
  report.replay={launcher_sha256:${JSON.stringify(sha(fs.readFileSync('verification/t_84b7da26/rear-browser.mjs')))},base_harness_sha256:${JSON.stringify(sha(base))},policy:'A neutral; B cycles cardinal intent every 6 seconds. Contact sought in full shared wire; no event injected.'};
  const {chromium}=createRequire(process.env.PLAYWRIGHT_NODE_MODULES)('playwright');
  const a=await openClient(chromium,'RearA'),b=await openClient(chromium,'RearB',a.code);
  assert.notEqual(a.pid,b.pid);assert.equal(a.identity.room,b.identity.room);
  await until(()=>clients.every(c=>c.lobby.humans.length===2),'two humans');
  await expand(a);await a.page.locator('#arena-room').selectOption('Forest');
  await a.page.locator('#target-room').selectOption('7');await a.page.locator('#bots-room').fill('0');await a.page.locator('#apply-room').click();
  await until(()=>clients.every(c=>c.lobby.settings.bots===0&&c.lobby.settings.wins_target===7),'configured');
  for(const c of clients){await c.page.locator('#ready-room').click();await expand(c,false);await c.page.locator('canvas').focus();}
  await until(()=>clients.every(c=>c.latest&&c.lobby.phase==='Running'),'running');
  const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0),sub=(a,b)=>a.map((x,i)=>x-b[i]);
  function contacts(){
    const frames=a.frames,other=new Map(b.frames.map(x=>[x.frame.tick,x.frame])),events=[];
    for(let i=1;i<frames.length;i++){
      const p=frames[i-1].frame,n=frames[i].frame,dt=n.simulation_tick-p.simulation_tick;
      if(dt!==4||n.tick-p.tick!==4||p.round_finished||n.round_finished||!other.has(p.tick)||!other.has(n.tick))continue;
      assert.deepEqual(p,other.get(p.tick));assert.deepEqual(n,other.get(n.tick));
      const carts=p.carts.filter(c=>c.active&&c.alive);if(carts.length!==2)continue;
      const d=carts[0].forward;if(dot(d,carts[1].forward)<.9999)continue;
      const gap=dot(sub(carts[1].position,carts[0].position),d),front=gap>0?1:0,rear=1-front;
      const offset=sub(carts[front].position,carts[rear].position);
      if(Math.hypot(...offset.map((x,i)=>x-Math.abs(gap)*d[i]))>.001||Math.abs(gap)>1.5)continue;
      const ms=[];
      for(const x of carts){const y=n.carts.find(c=>c.id===x.id);if(!y?.alive||dot(x.forward,y.forward)<.9999||((x.speed_seconds>0)!==(y.speed_seconds>0)))break;
        const v=sub(y.position,x.position),travel=dot(v,d),ordinary=4.2*(x.speed_seconds>0?1.25:1)*dt/64;
        ms.push({seat:x.id,travel,ordinary,extra:travel-ordinary,off_axis:Math.hypot(...v.map((x,i)=>x-travel*d[i])),forward_dot:dot(x.forward,y.forward)});
      }
      if(ms.length===2&&ms.every(m=>m.off_axis<.001)&&Math.abs(ms[front].extra-.4)<.002&&Math.abs(ms[rear].extra)<.002)events.push({from_tick:p.tick,to_tick:n.tick,initial_gap:Math.abs(gap),front_seat:carts[front].id,rear_seat:carts[rear].id,measurements:ms});
    }return events;
  }
  let held=null,index=0,next=now();const directions=['ArrowUp','ArrowRight','ArrowDown','ArrowLeft'];
  while(now()-gameplayStarted<170000){
    assert.equal(report.errors.length,0,'protocol/browser errors');
    const events=contacts();if(events.length){report.rear_contacts=events;break;}
    if(now()>=next){if(held)await b.page.keyboard.up(held);held=directions[index++%4];await b.page.keyboard.down(held);next=now()+6000;}
    await sleep(100);
  }
  if(held)await b.page.keyboard.up(held);
  report.rear_contacts=contacts();assert(report.rear_contacts.length>0,'no qualifying rear contact within bounded legal-input replay');
  report.checks.push({rear_contacts:report.rear_contacts.length,scope:'Both actual browser streams equal at before/after ticks; straight live aligned pair, ordinary travel plus0.4 only leader, unchanged heading for both. CPU paired-entry fixture remains causal oracle; no pixel displacement claim.'});
  for(const c of clients){await capturePrivateScreenshot(c.page,secrets,()=>c.page.screenshot({path:path.join(out,c.name+'.png'),timeout:60000}));report.screenshots.push({file:c.name+'.png',sha256:sha(fs.readFileSync(path.join(out,c.name+'.png'))),claim:'Actual post-event gameplay; not exact contact-tick pixels.'});}
}
`;
source=source.slice(0,begin)+run+source.slice(end);
fs.mkdirSync('verification/t_84b7da26/rear-replay-provenance',{recursive:true});
fs.writeFileSync('verification/t_84b7da26/rear-replay-provenance/executed.mjs',source);
console.log(JSON.stringify({launcher_sha256:sha(fs.readFileSync('verification/t_84b7da26/rear-browser.mjs')),executed_sha256:sha(source),base_harness_sha256:sha(base)}));
await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
