import test from 'node:test';import assert from 'node:assert/strict';
import {installOnlineHud} from '../web/online-hud-bridge.mjs';
function fixture(payload) {
 const game={},updates=[],timers=new Map();let destroyed=0;
 const hud={element:{hidden:false},update:value=>updates.push(value),destroy:()=>{destroyed++;}};
 const env={ryggattackHud:payload,ryggattackCommand:'unchanged',document:{getElementById:id=>{assert.equal(id,'game');return game;}},
  setInterval(fn,ms){assert.equal(ms,100);timers.set(1,fn);return 1;},clearInterval(id){timers.delete(id);}};
 const mount=container=>{assert.equal(container,game);return hud;};
 return {env,hud,updates,timers,mount,get destroyed(){return destroyed;}};
}
test('bootstrap mounts and atomically forwards parsed HUD data without room commands',()=>{
 const input={lobby:{id:1,phase:'Running'},frame:{tick:1},seat:0,status:'Online',notice:'',menu:false,rtt_ms:12};
 const f=fixture(JSON.stringify(input));const control=installOnlineHud(f.env,f.mount);
 assert.equal(f.env.ryggattackHudMounted,true);assert.equal(f.hud.element.hidden,false);assert.deepEqual(f.updates,[input]);assert.equal(f.timers.size,1);
 assert.equal(f.env.ryggattackCommand,'unchanged');control.destroy();assert.equal(f.destroyed,1);assert.equal(f.timers.size,0);assert.equal(f.env.ryggattackHudMounted,false);
 control.update();assert.equal(f.updates.length,1);control.destroy();assert.equal(f.destroyed,1);
});
test('missing offline or malformed bridge data clears HUD and enables canvas fallback',()=>{
 const f=fixture(undefined);const control=installOnlineHud(f.env,f.mount);
 assert.equal(f.env.ryggattackHudMounted,false);assert.equal(f.hud.element.hidden,true);assert.deepEqual(f.updates,[{}]);
 for(const value of ['{invalid',null,[],12,'null']) {
  f.env.ryggattackHud=value;control.update();assert.equal(f.env.ryggattackHudMounted,false);assert.equal(f.hud.element.hidden,true);assert.deepEqual(f.updates.at(-1),{});
 }
 f.env.ryggattackHud=JSON.stringify({lobby:null,frame:null,seat:null,status:'Backend unavailable',notice:''});control.update();
 assert.equal(f.env.ryggattackHudMounted,true);assert.equal(f.hud.element.hidden,false);assert.equal(f.updates.at(-1).status,'Backend unavailable');control.destroy();
});
test('fresh coherent payload replaces old outcome and leave clears the roster',()=>{
 const f=fixture(JSON.stringify({status:'Online',lobby:{id:1,phase:'Running'},frame:{tick:4,winner:0},seat:0}));const c=installOnlineHud(f.env,f.mount);
 f.env.ryggattackHud=JSON.stringify({status:'Left room',lobby:null,frame:null,seat:null});f.timers.get(1)();assert.equal(f.updates.at(-1).frame,null);
 f.env.ryggattackHud=JSON.stringify({status:'Online',lobby:{id:2,phase:'Running'},frame:{tick:5,winner:null},seat:1});c.update();assert.equal(f.updates.at(-1).lobby.id,2);assert.equal(f.updates.at(-1).frame.winner,null);c.destroy();
});
