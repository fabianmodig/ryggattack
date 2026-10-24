// Rail-distance corroboration of actual two-observer browser streams.
// Geometry follows src/tracks/path.rs and map.rs, never modifies gameplay.
import fs from 'node:fs';
import {gunzipSync} from 'node:zlib';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0),sub=(a,b)=>a.map((x,i)=>x-b[i]);
const len=a=>Math.hypot(...a),add=(a,b)=>a.map((x,i)=>x+b[i]),mul=(a,s)=>a.map(x=>x*s);
const pos=n=>[(n%5-2)*3.5,(Math.floor(n/5)-2)*3.5];
const neighbors=n=>[n%5>0?n-1:null,n%5<4?n+1:null,n>=5?n-5:null,n<20?n+5:null].filter(x=>x!==null);
const segments=[];
function line(id,start,end){const length=len(sub(end,start)),forward=mul(sub(end,start),1/length);segments.push({id,length,start,end,first:forward,last:forward,locate(p,d){const along=dot(sub(p,start),forward);return along>=-1e-4&&along<=length+1e-4&&len(sub(p,add(start,mul(forward,along))))<1e-4&&dot(d,forward)>.99999?Math.min(length,Math.max(0,along)):null;}});}
for(let n=0;n<25;n++)for(const next of neighbors(n)){const p=pos(n),q=pos(next),d=mul(sub(q,p),1/3.5);line(`edge:${n}:${next}`,add(p,mul(d,.9)),add(q,mul(d,-.9)));}
for(let n=0;n<25;n++)for(const from of neighbors(n))for(const to of neighbors(n))if(from!==to){const p=pos(n),a=mul(sub(pos(to),p),1/3.5),b=mul(sub(pos(from),p),1/3.5),start=add(p,mul(b,.9)),end=add(p,mul(a,.9));if(dot(a,b)<-.99){line(`junction:${from}:${n}:${to}`,start,end);continue;}
const center=add(p,mul(add(a,b),.9));segments.push({id:`curve:${from}:${n}:${to}`,length:Math.PI*.45,start,end,first:mul(b,-1),last:a,locate(v,d){const r=mul(sub(v,center),1/.9);if(Math.abs(len(r)-1)>1e-4)return null;const theta=Math.atan2(-dot(r,b),-dot(r,a));if(theta< -1e-4||theta>Math.PI/2+1e-4)return null;const tangent=sub(mul(a,Math.sin(theta)),mul(b,Math.cos(theta)));return dot(tangent,d)>.99999?Math.min(Math.PI/2,Math.max(0,theta))*.9:null;}});}
for(const s of segments)s.next=segments.filter(t=>len(sub(s.end,t.start))<1e-5&&dot(s.last,t.first)>.99999);
function locations(c){const p=[c.position[0],c.position[2]],d=[c.forward[0],c.forward[2]];return segments.flatMap(s=>{const along=s.locate(p,d);return along===null?[]:[{s,along}];});}
function travel(a,b,max=1.6){const found=[];for(const x of locations(a))for(const y of locations(b)){if(x.s===y.s&&y.along>=x.along-1e-5)found.push(Math.max(0,y.along-x.along));if(x.s.next.includes(y.s))found.push(x.s.length-x.along+y.along);}const values=found.filter(x=>x<=max+1e-4);if(!values.length)return null;assert(Math.max(...values)-Math.min(...values)<1e-4,'ambiguous rail displacement');return values[0];}
const cart=(x,z,fx,fz)=>({position:[x,.62,z],forward:[fx,0,fz]});
assert(Math.abs(travel(cart(-5,-7,1,0),cart(-4.6,-7,1,0))-.4)<1e-8);
assert.equal(travel(cart(-5,-7,1,0),cart(-4.6,-7,-1,0)),null,'reversal cannot pass');
assert.equal(travel(cart(-5,-7,1,0),cart(-4.6,-6.9,1,0)),null,'off-rail cannot pass');
// Corner at node24: from west to north; radius0.9, forward tangent evolves.
const before=cart(6.1,7,1,0),angle=.4/.9,after=cart(6.1+.9*Math.sin(angle),6.1+.9*Math.cos(angle),Math.cos(angle),-Math.sin(angle));
assert(Math.abs(travel(before,after)-.4)<1e-7,'curve rail length, not chord');
const root=process.argv[2];assert(root,'supply preserved browser attempt directory');
const bytes=fs.existsSync(root+'/states.json')?fs.readFileSync(root+'/states.json'):gunzipSync(fs.readFileSync(root+'/states.json.gz')),streams=JSON.parse(bytes),a=streams[0].frames,b=new Map(streams[1].frames.map(x=>[`${x.match}:${x.frame.tick}`,x.frame])),events=[];
let shared=0;
for(let i=1;i<a.length;i++){const p=a[i-1],n=a[i],dt=n.frame.simulation_tick-p.frame.simulation_tick;if(p.match!==n.match||dt!==4||n.frame.tick-p.frame.tick!==4||p.frame.round_finished||n.frame.round_finished)continue;const pb=b.get(`${p.match}:${p.frame.tick}`),nb=b.get(`${n.match}:${n.frame.tick}`);if(!pb||!nb)continue;assert.deepEqual(p.frame,pb);assert.deepEqual(n.frame,nb);shared++;
const carts=p.frame.carts.filter(c=>c.active&&c.alive);if(carts.length!==2)continue;
for(let front=0;front<2;front++){let rear=1-front,gap=travel(carts[rear],carts[front],1.5);if(gap===null||gap<1e-4)continue;const measurements=[];
for(const x of carts){const y=n.frame.carts.find(c=>c.id===x.id);if(!y?.active||!y.alive||(x.speed_seconds>0)!==(y.speed_seconds>0))break;const distance=travel(x,y,1);if(distance===null)break;const ordinary=4.2*(x.speed_seconds>0?1.25:1)*dt/64;measurements.push({seat:x.id,rail_distance:distance,ordinary,extra:distance-ordinary,before:x,after:y});}
if(measurements.length===2&&Math.abs(measurements[front].extra-.4)<.002&&Math.abs(measurements[rear].extra)<.002)events.push({match:p.match,from_tick:p.frame.tick,to_tick:n.frame.tick,initial_directed_gap:gap,front_seat:carts[front].id,rear_seat:carts[rear].id,measurements});
}}
const report={status:events.length?'passed-browser-wire-rail-contact':'no-qualifying-rail-contact',scope:'Independent read-only rail-distance analysis of equal adjacent actual browser snapshots. Shared directed segment or continuous seam, positive forward travel for both, leader ordinary+0.4 rail unit and rear ordinary only. Curves measured by arc length, not chord. Geometry uses full grid superset only to locate observed poses; does not fabricate topology or input, nor claim exact pixels/causal isolation. CPU paired-entry fixture remains causal oracle.',states_sha256:sha(bytes),shared_adjacent_pairs:shared,event_count:events.length,events,geometry_self_tests:4};
console.log(JSON.stringify(report,null,2));
