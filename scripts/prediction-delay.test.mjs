import {test} from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {delayPipe} from './prediction-delay.mjs';
function fixture(delays, maxBytes=1024){
 let now=0,timers=[],received=[],errors=[];
 const a=new EventEmitter(),b=new EventEmitter();
 Object.assign(a,{pause(){},resume(){},destroy(e){errors.push(e?.message);a.destroyed=true;}});
 Object.assign(b,{write(data){received.push({at:now,data:String(data)});return true;},end(){received.push({at:now,end:true});},destroy(e){errors.push(e?.message);b.destroyed=true;}});
 let i=0;
 const stats=delayPipe(a,b,()=>delays[i++%delays.length],{maxBytes,now:()=>now,setTimer:(fn,ms)=>{const t={fn,at:now+ms};timers.push(t);return t;},clearTimer:t=>{timers=timers.filter(x=>x!==t);}});
 const advance=at=>{now=at;for(let guard=0;guard<100;guard++){const ready=timers.filter(t=>t.at<=now);timers=timers.filter(t=>t.at>now);if(!ready.length)return;for(const t of ready)t.fn();}throw Error('unbounded timer loop');};
 return {a,b,received,errors,stats,advance};
}
test('many chunks pay latency concurrently, not once per packet',()=>{
 const f=fixture([100]);for(let i=0;i<20;i++)f.a.emit('data',Buffer.from(String(i)+','));
 f.advance(99);assert.equal(f.received.length,0);f.advance(100);assert.equal(f.received.length,20);assert.equal(f.stats.queued,0);
});
test('jitter cannot reorder TCP bytes or EOF',()=>{
 const f=fixture([100,50,75]);for(const s of ['a','b','c'])f.a.emit('data',Buffer.from(s));f.a.emit('end');
 f.advance(99);assert.equal(f.received.length,0);f.advance(100);assert.deepEqual(f.received.map(x=>x.data??'EOF'),['a','b','c','EOF']);
});
test('buffer exhaustion closes both owned streams rather than growing',()=>{
 const f=fixture([100],4);f.a.emit('data',Buffer.from('12345'));assert(f.a.destroyed&&f.b.destroyed);assert.equal(f.stats.queued,0);
});
