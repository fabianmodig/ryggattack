import fs from 'node:fs';
import assert from 'node:assert/strict';
const root='verification/online-explosion/2026-10-24T15-30-37-066Z-fe8307a389d1';
const streams=JSON.parse(fs.readFileSync(root+'/states.json'));
const a=streams[0].frames,b=new Map(streams[1].frames.map(x=>[x.frame.tick,x]));
const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0),sub=(a,b)=>a.map((x,i)=>x-b[i]);
const norm=a=>Math.hypot(...a);
const found=[];
for(let i=1;i<a.length;i++){
 const p=a[i-1],n=a[i];if(p.match!==n.match||n.frame.tick-p.frame.tick!==4||n.frame.simulation_tick-p.frame.simulation_tick!==4||p.frame.round_finished||n.frame.round_finished)continue;
 if(!b.has(p.frame.tick)||!b.has(n.frame.tick))continue;
 assert.deepEqual(p.frame,b.get(p.frame.tick).frame);assert.deepEqual(n.frame,b.get(n.frame.tick).frame);
 const before=p.frame.carts.filter(c=>c.active&&c.alive),after=n.frame.carts.filter(c=>c.active&&c.alive);if(before.length!==2||after.length!==2)continue;
 const d=before[0].forward;if(dot(d,before[1].forward)<.9999||before.some(c=>Math.abs(c.forward[1])>.001))continue;
 const along=dot(sub(before[1].position,before[0].position),d);
 const front=along>0?1:0,rear=1-front;
 const offset=sub(before[front].position,before[rear].position);
 if(norm(offset.map((x,i)=>x-Math.abs(along)*d[i]))>.001||Math.abs(along)>1.5)continue;
 const measurements=[];
 for(let j=0;j<2;j++){
 const x=before[j],y=after.find(c=>c.id===x.id);if(dot(x.forward,y.forward)<.9999||((x.speed_seconds>0)!==(y.speed_seconds>0)))break;
 const v=sub(y.position,x.position),travel=dot(v,d),off=norm(v.map((x,i)=>x-travel*d[i]));
 const ordinary=4.2*(x.speed_seconds>0?1.25:1)*4/64;
 measurements.push({seat:x.id,travel,ordinary,extra:travel-ordinary,off_axis:off,forward_dot:dot(x.forward,y.forward)});
 }
 if(measurements.length===2&&measurements.every(m=>m.off_axis<.001)&&Math.abs(measurements[front].extra-.4)<.002&&Math.abs(measurements[rear].extra)<.002){
 found.push({match:n.match,from_tick:p.frame.tick,to_tick:n.frame.tick,initial_gap:Math.abs(along),front_seat:before[front].id,rear_seat:before[rear].id,measurements});}
}
const r={status:found.length?'observed-shared-wire-rear-contact-corroboration':'no-qualifying-browser-wire-contact',scope:'Read-only analysis of two actual browser wire streams at shared adjacent sampled ticks; straight aligned live carts, unchanged speed buff, four simulation ticks at64Hz and4.2 rail units/sec; both observers equal; CPU paired-entry fixture is the exact causal oracle. Not pixel measurement or injected contact.',event_count:found.length,events:found};
fs.writeFileSync('verification/t_84b7da26/rear-wire.json',JSON.stringify(r,null,2)+'\n');console.log(JSON.stringify({status:r.status,event_count:r.event_count}));
