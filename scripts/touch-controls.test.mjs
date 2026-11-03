import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync,readFileSync} from 'node:fs';
const url=new URL('../web/touch-controls.mjs',import.meta.url);
const module=existsSync(url)?await import(url):{};
test('swipe chooses a cardinal direction retained for the next junction, without firing',()=>{
  assert.equal(typeof module.createGesture,'function','touch gesture adapter is missing');
  const g=module.createGesture();
  g.down(1,100,100,0); g.move(1,150,110); g.up(1,150,110,100);
  assert.deepEqual(g.sample(),{horizontal:1,vertical:0,fire:false});
  assert.deepEqual(g.sample(),{horizontal:1,vertical:0,fire:false});
  g.down(2,50,100,200);g.up(2,48,30,300);
  assert.deepEqual(g.sample(),{horizontal:0,vertical:1,fire:false});
});
test('short taps latch exactly once; long holds and out-and-back swipes never tap-fire',()=>{
  const g=module.createGesture();
  g.down(1,0,0,0);g.up(1,1,1,150);
  assert.equal(g.sample().fire,true);
  assert.equal(g.sample().fire,false);
  g.down(2,0,0,200);g.up(2,0,0,800);assert.equal(g.sample().fire,false);
  g.down(3,0,0,1000);g.move(3,-60,0);g.up(3,0,0,1100);
  assert.deepEqual(g.sample(),{horizontal:-1,vertical:0,fire:false});
});
test('multitouch cancels until every finger lifts; reset drops direction and pending tap',()=>{
  const g=module.createGesture();
  g.down(1,0,0,0);g.move(1,70,0);g.down(2,0,0,10);
  g.up(1,70,0,100);g.up(2,0,0,110);
  assert.deepEqual(g.sample(),{horizontal:0,vertical:0,fire:false});
  g.down(3,0,0,200);g.up(3,0,0,250);g.reset();
  assert.deepEqual(g.sample(),{horizontal:0,vertical:0,fire:false});
  g.down(4,0,0,300);g.move(4,0,80);g.reset();g.up(4,0,80,350);
  assert.deepEqual(g.sample(),{horizontal:0,vertical:0,fire:false});
});

function fixture(){
  const listeners=new Map(), elements=new Map();
  const document={hidden:false,activeElement:null,addEventListener:(n,f)=>listeners.set('doc:'+n,f),removeEventListener(){},getElementById(id){
    if(!elements.has(id))elements.set(id,{hidden:true,textContent:'',dataset:{},handlers:new Map(),
      addEventListener(n,f){this.handlers.set(n,f);},removeEventListener(){},
      focus(){document.activeElement=this;},setPointerCapture(){},classList:{toggle(){}},
      querySelectorAll(){return [];}});
    return elements.get(id);
  }};
  let tick;
  const env={document,ryggattackLocalStatus:JSON.stringify({screen:'Playing',paused:false,winner:false}),ryggattackBrowserMenu:false,
    addEventListener:(n,f)=>listeners.set(n,f),removeEventListener(){},setInterval:f=>{tick=f;return 1;},clearInterval(){},matchMedia:()=>({matches:true})};
  const canvas=document.getElementById('ryggattack-canvas');canvas.focus();
  const control=module.installTouchControls(env);
  const event=(type,values={})=>canvas.handlers.get(type)?.({pointerId:1,pointerType:'touch',clientX:100,clientY:100,timeStamp:0,preventDefault(){},...values});
  return {env,canvas,control,event,listeners,tick:()=>tick(),sample:()=>JSON.parse(env.ryggattackSampleTouch())};
}
test('real bridge consumes taps and resets on cancel/blur/visibility/menu without hijacking mouse or forms',()=>{
  assert.equal(typeof module.installTouchControls,'function','browser touch bridge missing');
  const f=fixture();
  f.event('pointerdown');f.event('pointerup',{timeStamp:100});assert.equal(f.sample().fire,true);assert.equal(f.sample().fire,false);
  for(const reset of [()=>f.event('pointercancel'),()=>{f.event('pointerdown');f.event('lostpointercapture');},()=>f.listeners.get('doc:pointerdown')({pointerType:'touch',target:{}}),()=>f.listeners.get('blur')(),()=>{f.env.document.hidden=true;f.listeners.get('doc:visibilitychange')();},()=>{f.env.ryggattackBrowserMenu=true;f.tick();}]){
    f.env.document.hidden=false;f.env.ryggattackBrowserMenu=false;
    f.event('pointerdown');f.event('pointermove',{clientX:160});f.event('pointerup',{clientX:160,timeStamp:100});assert.equal(f.sample().horizontal,1);
    reset();const cleared=f.sample();assert.deepEqual(cleared,{horizontal:0,vertical:0,fire:false,epoch:cleared.epoch});
  }
  f.env.ryggattackBrowserMenu=false;
  f.event('pointerdown',{pointerType:'mouse'});f.event('pointerup',{pointerType:'mouse',timeStamp:100});assert.equal(f.sample().fire,false);
  f.env.document.activeElement={};f.event('pointerup',{timeStamp:100});assert.equal(f.sample().fire,false);
  f.control.destroy();assert.equal(f.env.ryggattackSampleTouch,undefined);
});
test('entry scopes touch-action to gestures, retains form scrolling and packages safe-area controls',()=>{
  const html=readFileSync(new URL('../web/index.html',import.meta.url),'utf8');
  assert.match(html,/viewport-fit=cover/);
  assert.match(html,/#game\s*\{[^}]*inset:\s*env\(safe-area-inset-top/);
  assert.doesNotMatch(html,/user-scalable=no/);
  assert.match(html,/#ryggattack-canvas\.touch-playing\s*\{[^}]*touch-action:\s*none/);
  assert.match(html,/env\(safe-area-inset-bottom/);
  const hud=readFileSync(new URL('../web/online-hud.mjs',import.meta.url),'utf8');
  assert.match(hud,/env\(safe-area-inset-top\)/);
  assert.match(hud,/env\(safe-area-inset-left\)/);
  for(const id of ['touch-join','touch-start','touch-pause','touch-resume','touch-restart'])assert(html.includes(`id="${id}"`),id);
});

test('online bridge only samples live Running gameplay, resets across round/menu and retains normal pointer-up',()=>{
  const f=fixture();f.env.ryggattackLaunch='online';
  f.env.ryggattackHud=JSON.stringify({lobby:{phase:'Running'},menu:false,frame:{round_finished:false}});
  f.event('pointerdown');f.event('pointermove',{clientY:30});f.event('pointerup',{clientY:30,timeStamp:100});
  f.event('lostpointercapture');assert.equal(f.sample().vertical,1);
  f.env.ryggattackHud=JSON.stringify({lobby:{phase:'Running'},menu:false,frame:{round_finished:true}});
  f.tick();assert.equal(f.sample().vertical,0);
  f.env.ryggattackHud=JSON.stringify({lobby:{phase:'Running'},menu:true,frame:{round_finished:false}});
  f.event('pointerdown');f.event('pointerup',{timeStamp:100});assert.equal(f.sample().fire,false);
  f.env.ryggattackHud=JSON.stringify({lobby:{phase:'Lobby'},menu:false});
  f.event('pointerdown');f.event('pointerup',{timeStamp:100});assert.equal(f.sample().fire,false);
  f.control.destroy();
});
