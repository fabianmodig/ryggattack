import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync,readFileSync,mkdtempSync,writeFileSync,rmSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
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

function fixture(coarse=false){
  const listeners=new Map(), elements=new Map();
  const document={hidden:false,activeElement:null,addEventListener:(n,f)=>listeners.set('doc:'+n,f),removeEventListener(){},getElementById(id){
    if(!elements.has(id))elements.set(id,{hidden:true,textContent:'',dataset:{},handlers:new Map(),
      addEventListener(n,f){this.handlers.set(n,f);},removeEventListener(){},
      focus(){document.activeElement=this;this.handlers.get("focus")?.();},setPointerCapture(){},classList:{values:new Set(),toggle(n,on){if(on)this.values.add(n);else this.values.delete(n);},contains(n){return this.values.has(n);}},
      querySelectorAll(){return [];}});
    return elements.get(id);
  }};
  let tick;
  const env={document,ryggattackLocalStatus:JSON.stringify({screen:'Playing',paused:false,winner:false}),ryggattackBrowserMenu:false,
    addEventListener:(n,f)=>listeners.set(n,f),removeEventListener(){},setInterval:f=>{tick=f;return 1;},clearInterval(){},matchMedia:()=>({matches:coarse})};
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
test('multitouch invalidates an already sampled tap and blocks replacements until every finger lifts',()=>{
  const f=fixture();
  f.event('pointerdown');f.event('pointerup',{timeStamp:100});
  const sampled=f.sample();assert.equal(sampled.fire,true); // already owned by Rust now
  f.event('pointerdown',{pointerId:2,timeStamp:200});
  f.event('pointerdown',{pointerId:3,timeStamp:210});
  const cancelled=f.sample();
  assert.equal(cancelled.epoch,sampled.epoch+1,'cancellation must invalidate Rust pending fire, not only the JS tap');
  assert.deepEqual(cancelled,{horizontal:0,vertical:0,fire:false,epoch:cancelled.epoch});
  f.event('pointerup',{pointerId:2,timeStamp:220});
  f.event('pointerdown',{pointerId:4,timeStamp:230}); // finger 3 still held
  f.event('pointermove',{pointerId:4,clientX:180,timeStamp:240});
  f.event('pointerup',{pointerId:4,timeStamp:250});
  assert.equal(f.sample().horizontal,0);
  f.event('pointerup',{pointerId:3,timeStamp:260});
  assert.equal(f.sample().fire,false);
  f.event('pointerdown',{pointerId:5,timeStamp:300});
  f.event('pointerup',{pointerId:5,timeStamp:350});
  assert.equal(f.sample().fire,true,'fresh single-finger gesture works after all lifted');
  f.control.destroy();
});

test('offline play is game-only for fine pointers too; help and scores remain paused-only',()=>{
  const f=fixture(),$=id=>f.env.document.getElementById(id);
  f.env.ryggattackOfflineHud='FIRST TO 3\nP1: 2 IN SPD10s HIT10s\nSOLO - no scoring';f.tick();
  assert.equal($('offline-hud').hidden,true,'fine pointers must not retain local-only score bands');
  assert.equal($('touch-controls').classList.contains('immersive-match'),true);
  assert.equal($('touch-back').hidden,true);
  assert.equal($('offline-hud').textContent,f.env.ryggattackOfflineHud);
  assert.equal($('game').classList.contains('gameplay'),true);
  assert.equal($('offline-pause').hidden,true);
  assert.equal($('touch-hint').textContent,'');
  f.env.ryggattackLocalStatus=JSON.stringify({screen:'Playing',paused:true});f.tick();
  assert.equal($('offline-pause').hidden,false);
  assert.equal(f.env.document.activeElement,$('help-resume'));
  f.env.ryggattackPauseSelection='back';f.tick();
  assert.equal(f.env.document.activeElement,$('help-back'),'actual Bevy gamepad menu focus must be visible in the DOM');
  f.env.ryggattackPauseSelection='resume';f.tick();
  assert.equal(f.env.document.activeElement,$('help-resume'));
  const keydown=$('offline-pause').handlers.get('keydown');
  keydown({key:'Tab',preventDefault(){}});assert.equal(f.env.document.activeElement,$('help-back'));
  assert.equal(f.env.ryggattackPauseDomFocus,'back','DOM selection is returned to the actual Bevy menu');
  keydown({key:'Tab',preventDefault(){}});assert.equal(f.env.document.activeElement,$('help-resume'));
  keydown({key:'Escape',preventDefault(){}});assert.equal(f.env.ryggattackTouchAction,'resume');
  assert.equal($('touch-controls').hidden,true);
  $('help-resume').onclick();assert.equal(f.env.ryggattackTouchAction,'resume');
  $('help-back').onclick();assert.equal(f.env.ryggattackTouchAction,'back');
  f.env.ryggattackLaunch='online';f.tick();
  assert.equal($('offline-hud').hidden,true);assert.equal($('offline-pause').hidden,true);
  assert.equal($('game').classList.contains('gameplay'),false);
  f.env.ryggattackHud=JSON.stringify({status:'Connected',lobby:{phase:'Running'},menu:false});f.tick();
  assert.equal($('game').classList.contains('gameplay'),true,'online uses the same gameplay state as local');
  f.env.ryggattackHud=JSON.stringify({status:'Connected',lobby:{phase:'Running'},menu:true});f.tick();
  assert.equal($('online-pause-details').hidden,false,'fine-pointer players still have scores and help');
  f.env.ryggattackLaunch=null;f.env.ryggattackLocalStatus=JSON.stringify({screen:'MainMenu'});f.tick();
  assert.equal($('offline-hud').hidden,true);assert.equal($('game').classList.contains('gameplay'),false);
  f.env.ryggattackLocalStatus=JSON.stringify({screen:'Playing'});f.tick();
  assert.equal($('game').classList.contains('gameplay'),true,'returning from online reuses the local viewport');
  f.control.destroy();
});

test('mobile offline play exposes only floating pause; actual scores move into pause',()=>{
  const f=fixture(true),$=id=>f.env.document.getElementById(id);
  f.env.ryggattackOfflineHud='FIRST TO 3\nP1: 2 IN SPD10s HIT10s\nROUND LIVE';f.tick();
  assert.equal($('offline-hud').hidden,true,'mobile active scores must not reserve or overlay gameplay');
  assert.equal($('touch-back').hidden,true,'menu is reachable through pause, not an active control row');
  assert.equal($('touch-pause').hidden,false);
  assert.equal($('touch-controls').classList.contains('immersive-match'),true);
  f.env.ryggattackLocalStatus=JSON.stringify({screen:'Playing',paused:true});f.tick();
  assert.equal($('pause-scores').textContent,f.env.ryggattackOfflineHud,'pause uses actual ECS scores/effects');
  assert.equal($('offline-pause').hidden,false);
  $('help-resume').onclick();assert.equal(f.env.ryggattackTouchAction,'resume');
  f.env.ryggattackLocalStatus=JSON.stringify({screen:'Playing',paused:false});f.tick();
  assert.equal($('offline-pause').hidden,true);assert.equal($('offline-hud').hidden,true);
  f.control.destroy();
});

test('mobile online scores are available only in the input-pause menu from authoritative publication',()=>{
  const f=fixture(true),$=id=>f.env.document.getElementById(id);
  f.env.ryggattackLaunch='online';
  const hud={status:'Connected',seat:0,lobby:{phase:'Running'},menu:false,frame:{names:['TouchQA'],wins:[2],carts:[{id:0,active:true,alive:true,speed_seconds:4}]}};
  f.env.ryggattackHud=JSON.stringify(hud);f.tick();
  assert.equal($('online-pause-details').hidden,true);
  hud.menu=true;f.env.ryggattackHud=JSON.stringify(hud);f.tick();
  assert.equal($('online-pause-details').hidden,false,'authoritative scores and rules must remain available while input is suspended');
  assert.match($('online-pause-scores').textContent,/TouchQA.*2 wins.*IN.*Speed 4s/);
  hud.menu=false;f.env.ryggattackHud=JSON.stringify(hud);f.tick();
  assert.equal($('online-pause-details').hidden,true);
  f.control.destroy();
});

test('paused DOM W/S physical codes and key fallbacks publish focus without changing activation',()=>{
  const f=fixture(),$=id=>f.env.document.getElementById(id);
  f.env.ryggattackLocalStatus=JSON.stringify({screen:'Playing',paused:true});f.tick();
  const keydown=$('offline-pause').handlers.get('keydown');
  for(const event of [
    {code:'KeyS',key:'ы'}, {code:'KeyW',key:'ц'}, // physical positions on a non-Latin layout
    {key:'s'}, {key:'w'}, {key:'S'}, {key:'W'},
    {key:'ArrowDown'}, {key:'ArrowUp'}, {key:'Tab'}, {key:'Tab',shiftKey:true},
  ]){
    const expected=f.env.document.activeElement===$('help-resume')?'back':'resume';let prevented=false;
    keydown({...event,preventDefault(){prevented=true;}});
    assert.equal(f.env.document.activeElement,$(`help-${expected}`),JSON.stringify(event));
    assert.equal(f.env.ryggattackPauseDomFocus,expected,'publish DOM selection to the real Bevy menu');
    assert.equal(prevented,true,'navigation must not scroll pause help');
    assert.equal(f.env.ryggattackTouchAction,undefined,'navigation alone must not activate');
  }
  // Unhandled activation stays native to the focused button (real browser replay).
  for(const key of ['Enter',' ']){let prevented=false;keydown({key,preventDefault(){prevented=true;}});assert.equal(prevented,false);}
  f.env.ryggattackPauseSelection='back';f.tick();assert.equal(f.env.document.activeElement,$('help-back'));
  f.tick();assert.equal(f.env.document.activeElement,$('help-back'),'no repeated DOM write fights the gamepad selection');
  f.env.ryggattackPauseSelection='resume';f.tick();assert.equal(f.env.document.activeElement,$('help-resume'));
  f.control.destroy();
});

// Compile the checked-in Rust transition blocks, not a JS imitation of latches.
// Neutral device/transport fixtures isolate the interval before fixed tick/send.
// The native ECS regression separately exercises apply_inputs on real players.
test('sampled tap cancellation reaches source-real local pending-fire and online pending-send adapters',()=>{
  const f=fixture();
  f.event('pointerdown');f.event('pointerup',{timeStamp:100});const tap=f.sample();
  f.event('pointerdown',{pointerId:2,timeStamp:200});
  f.event('pointerdown',{pointerId:3,timeStamp:210});const cancel=f.sample();
  f.control.destroy();
  const source=path=>readFileSync(new URL(`../src/${path}`,import.meta.url),'utf8');
  const between=(text,start,end)=>{const a=text.indexOf(start),b=text.indexOf(end,a);assert(a>=0&&b>a,`missing Rust boundary: ${start}`);return text.slice(a,b);};
  const input=source('input.rs'),players=source('players/mod.rs'),online=source('online.rs');
  const sampleBlock=between(input,'            touch.reset = *epoch != sample.epoch;','        } else {');
  const localBlock=between(players,'        let intent = if touch_seat == Some(player.id) {','\n    }\n}',);
  const onlineBlock=between(online,'    if touch.reset {','    sync_visual_boundary(&client');
  const merge=between(input,'pub(crate) fn merge_touch','#[derive(Resource, Default)]');
  const literal=s=>`Sample { horizontal: ${s.horizontal}.0, vertical: ${s.vertical}.0, fire: ${s.fire}, epoch: ${s.epoch} }`;
  const rust=`
#![allow(dead_code)]
#[derive(Clone,Copy,Default)] struct ControlIntent { horizontal:f32, vertical:f32, fire:bool }
#[derive(Default)] struct TouchInput { intent:ControlIntent, reset:bool }
struct Sample { horizontal:f32, vertical:f32, fire:bool, epoch:u32 }
fn default<T:Default>()->T { T::default() }
mod input { use super::*; ${merge} }
fn sample(touch:&mut TouchInput,epoch:&mut u32,sample:Sample) { ${sampleBlock} }
#[derive(Default)] struct Inputs { local_fire_pending:[bool;4], intents:[ControlIntent;4] }
struct Player { id:usize }
fn local(inputs:&mut Inputs,touch:Option<&TouchInput>) {
 let player=Player{id:0};let touch_seat=Some(0);let device=default(); ${localBlock}
}
enum KeyCode { KeyD,ArrowRight,KeyA,ArrowLeft,KeyW,ArrowUp,KeyS,ArrowDown,Space }
struct Keys; impl Keys { fn pressed(&self,_:KeyCode)->bool{false} fn just_pressed(&self,_:KeyCode)->bool{false} }
enum GamepadButton { South }
struct Stick {x:f32,y:f32} struct Pad; impl Pad {fn left_stick(&self)->Stick{Stick{x:0.,y:0.}} fn pressed(&self,_:GamepadButton)->bool{false}}
#[derive(Default)] struct Client { send:bool, sent:Vec<ControlIntent> }
impl Client {fn pump(&mut self,intent:ControlIntent)->bool {if self.send {self.sent.push(intent);} self.send}}
fn online(fire:&mut bool,touch:&TouchInput,client:&mut Client) {
 let active=true;let keys=Keys;let pads:Vec<Pad>=vec![]; ${onlineBlock}
}
fn main() {
 let mut touch=TouchInput::default();let mut epoch=0;
 let mut inputs=Inputs::default();let mut fire=false;let mut client=Client::default();
 sample(&mut touch,&mut epoch,${literal(tap)});
 local(&mut inputs,Some(&touch));online(&mut fire,&touch,&mut client);
 assert!(inputs.local_fire_pending[0],"local tap must already be latched");
 assert!(fire,"online tap must already be latched while send is throttled");
 assert!(client.sent.is_empty());
 sample(&mut touch,&mut epoch,${literal(cancel)});
 local(&mut inputs,Some(&touch));online(&mut fire,&touch,&mut client);
 assert!(!inputs.local_fire_pending[0],"multitouch epoch must clear local fixed-tick pending fire");
 assert!(!inputs.intents[0].fire);
 assert!(!fire,"multitouch epoch must clear online pending send");
 client.send=true;online(&mut fire,&touch,&mut client);
 assert_eq!(client.sent.len(),1);assert!(!client.sent[0].fire,"next online send must be neutral");
 println!("source-real sample -> local pending=false; online pending=false; next send fire=false");
}
`;
  const dir=mkdtempSync(join(tmpdir(),'touch-adapters-'));
  try{
    const file=join(dir,'adapters.rs'),binary=join(dir,'adapters');writeFileSync(file,rust);
    const build=spawnSync(process.env.RUSTC??'rustc',['--edition=2024',file,'-o',binary],{encoding:'utf8'});
    assert.equal(build.status,0,`source-real Rust fixture compile failed: ${build.error??''}\n${build.stderr}`);
    const run=spawnSync(binary,[],{encoding:'utf8'});assert.equal(run.status,0,run.stderr);
    console.log(run.stdout.trim());
  }finally{rmSync(dir,{recursive:true,force:true});}
});

test('entry scopes touch-action to gestures, retains form scrolling and packages safe-area controls',()=>{
  const html=readFileSync(new URL('../web/index.html',import.meta.url),'utf8');
  assert.match(html,/viewport-fit=cover/);
  assert.match(html,/#game\s*\{[^}]*inset:\s*0/);
  assert.doesNotMatch(html,/#game\.offline-playing|124px|80px/,'no local-only viewport reservations');
  assert.match(html,/#game\.gameplay\s*>\s*\.rygg-online-hud/,'shared gameplay hides active info bars');
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
