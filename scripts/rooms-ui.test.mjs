import assert from 'node:assert/strict';
import fs from 'node:fs';
import {joinUrl,settingsCommand,installRoomControls,participantStatus,validLaunch,savedLaunch,navigateControls} from '../web/rooms.mjs';
const html=fs.readFileSync(new URL('../web/index.html',import.meta.url),'utf8');
assert(!html.includes('Private online rooms'));
assert.match(html,/PLAY LOCAL/);assert.match(html,/PLAY ONLINE/);
assert.match(html,/60.*wall.*second/i);
assert.match(html,/server keeps running/i);
assert.equal(participantStatus({connected:false,reconnect_seconds:59},'Running'),'Bot takeover · reserved 59s');
assert.equal(participantStatus({connected:false,reconnect_seconds:12},'Lobby'),'Disconnected · reserved 12s');
assert.equal(participantStatus({connected:true,ready:false},'Running'),'playing');
assert.equal(participantStatus({connected:true,ready:true},'Lobby'),'ready');
const code='0123456789abcdef0123';
const link=new URL(joinUrl('https://game.example/play/?name=Old&quality=0','ws://127.0.0.1:8787',code));
assert.equal(link.searchParams.get('room'),code);assert.equal(link.searchParams.has('name'),false);assert.equal(link.searchParams.get('quality'),'0');
assert.deepEqual(settingsCommand('Volcano','1','2'),{Configure:{arena:'Volcano',wins_target:1,bots:2}});
for(const args of [['Forest','2','0'],['bad','1','0'],['Forest','1','4']])assert.throws(()=>settingsCommand(...args));
const launch={address:'wss://game.example/ws',name:'Ada',code:null};
assert(validLaunch(launch));assert(validLaunch({...launch,code}));
for(const change of [{name:''},{name:' '.repeat(10)},{name:'a'.repeat(25)},{address:'https://game.example'},{address:'ws://user:password@host/'},{address:'bad'},{code:''},{code:code.toUpperCase()}])assert(!validLaunch({...launch,...change}));

// Small DOM boundary fixture. It does not emulate network/server authority.
function fixture(search='',store=new Map()) {
 const elements=new Map(), calls=[];let interval;
 const doc={activeElement:null,getElementById:id=>elements.get(id),createElement:()=>make('li')};
 function make(id,tag='button') {return {id,tag,hidden:false,disabled:false,value:'',textContent:'',children:[],ownerDocument:doc,
  focus(){doc.activeElement=this;},blur(){doc.activeElement=null;},select(){},click(){this.onclick?.();},
  getClientRects(){return this.hidden?[]:[{}];},matches(s){return s.split(',').includes(this.tag);},closest(s){return this.parent?.id===s.slice(1)?this.parent:null;},
  querySelector(){return this.children.find(e=>!e.disabled)??null;},querySelectorAll(selector){return this.children.filter(e=>!e.hidden&&selector.split(',').includes(e.tag));},
  replaceChildren(...children){this.children=children;},addEventListener(type,fn){this[type]=fn;}};}
 for(const m of html.matchAll(/<(\w+)\b[^>]*\bid="([^"]+)"/g))elements.set(m[2],make(m[2],m[1]));
 for(const [id,children] of Object.entries({'main-controls':['play-local','play-online'],'online-setup':['player-name','create-room','room-code','join-room','setup-back'],'room-controls':['copy-room','ready-room','leave-room'],'match-menu':['resume-match','leave-match'],'online-entry':['play-local','play-online']}))elements.get(id).children=children.map(id=>elements.get(id));
 const url=new URL('https://game.example/'+search);
 const env={document:doc,location:{search:url.search,href:url.href,assign:url=>calls.push(url)},navigator:{clipboard:{writeText:async()=>{}}},
  sessionStorage:{getItem:key=>store.get(key)??null,setItem:(key,v)=>store.set(key,v),removeItem:key=>store.delete(key)},
  setInterval:fn=>{interval=fn;return 1;},clearInterval(){}};
 const menu=installRoomControls(env), $=id=>elements.get(id);
 return {env,menu,$,calls,store,tick:()=>interval(),publish:state=>{env.ryggattackRoom=JSON.stringify(state);interval();}};
}
let f=fixture();
assert.equal(f.menu.screen,'main');assert.equal(f.env.ryggattackLaunch,null);assert.equal(f.$('backend').value,'wss://game.example/ws');
f.$('play-online').click();assert.equal(f.menu.screen,'setup');assert.equal(f.calls.length,0,'mode change does not join');
f.$('room-code').value=code;f.tick();assert.equal(f.calls.length,0,'typing does not join');
f.$('setup-back').click();assert.equal(f.menu.screen,'main');assert.equal(f.env.document.activeElement.id,'play-online');
f.$('play-local').click();assert.equal(f.env.ryggattackLocalAction,'local');assert.equal(f.menu.screen,'local');
f.env.ryggattackLocalScreen='Lobby';f.tick();f.env.ryggattackLocalScreen='MainMenu';f.tick();assert.equal(f.menu.screen,'main');
for(const value of ['0','2','6','7','-1','auto','','06']){const x=fixture('?quality='+value);assert.equal(x.$('quality-override').disabled,!['0','2','6'].includes(value));assert.equal(x.$('quality-override').value,value);}
f=fixture('?online=wss%3A%2F%2Fgame.example%2Fws&room='+code);
assert.equal(f.menu.screen,'setup');assert.equal(f.env.ryggattackLaunch,null,'invitation prefills without admission');assert.equal(f.$('room-code').value,code);
f.$('player-name').value='';f.$('join-room').click();assert.match(f.$('room-notice').textContent,/Enter a player name/);assert.equal(f.calls.length,0);
f.$('player-name').value='Ada';f.$('join-room').click();f.$('join-room').click();assert.equal(f.calls.length,1,'duplicate submit blocked');assert(f.$('join-room').disabled);
const pending=JSON.parse(f.store.get('ryggattack.launch'));assert.equal(pending.code,code);
const restored=fixture('?online=wss%3A%2F%2Fgame.example%2Fws&room='+code,f.store);assert.deepEqual(JSON.parse(restored.env.ryggattackLaunch),pending);assert(!f.store.has('ryggattack.launch'),'one-shot consumed');
restored.publish({status:'Backend unavailable',notice:'',lobby:null});assert.equal(restored.$('join-room').disabled,false);assert.match(restored.$('room-notice').textContent,/Backend unavailable/);
restored.$('setup-back').click();assert(!new URL(restored.calls[0]).searchParams.has('online'));
const key=`ryggattack.claim.${JSON.stringify([launch.address,code])}`;
const claims=new Map([[key,JSON.stringify({room:1,participant:2,seat:0,credential:'a'.repeat(64)})]]);
const storage={getItem:k=>claims.get(k)};
assert(savedLaunch(storage,launch.address,'Ada',code));assert.equal(savedLaunch(storage,'wss://other/ws','Ada',code),null);
claims.set(key,'bad');assert.equal(savedLaunch(storage,launch.address,'Ada',code),null);
const humans=[{id:1,seat:0,name:'Host',ready:false,connected:true},{id:2,seat:1,name:'Guest',ready:false,connected:true}];
const lobby={id:1,code,host:1,phase:'Lobby',humans,settings:{arena:'Forest',wins_target:3,bots:0}};
f=fixture();f.publish({lobby,participant:2,status:'Online',notice:''});assert.equal(f.menu.screen,'lobby');assert(f.$('apply-room').disabled);assert.equal(f.$('room-roster').children.length,2);
f.$('ready-room').click();assert.deepEqual(JSON.parse(f.env.ryggattackCommand),{Ready:true});f.$('ready-room').click();assert.match(f.$('room-notice').textContent,/previous command/);
f.env.ryggattackCommand=null;f.publish({lobby:{...lobby,phase:{Countdown:{ticks_left:128}},humans:[humans[0],{...humans[1],ready:true}]},participant:2,status:'Online',notice:''});assert.match(f.$('room-info').textContent,/Starting in 2.0s/);assert.match(f.$('ready-room').textContent,/CANCEL/);
f.publish({lobby:{...lobby,phase:'Running'},participant:2,status:'Online',notice:''});assert.equal(f.menu.screen,'playing');assert(f.$('ready-room').disabled);
f.env.ryggattackHud=JSON.stringify({lobby:{phase:'Running'},menu:true});f.tick();assert.equal(f.menu.screen,'match');f.$('resume-match').click();assert.equal(f.env.ryggattackResume,true);assert.equal(f.menu.screen,'playing');
f.publish({lobby,participant:1,status:'Online again',notice:''});assert.equal(f.menu.screen,'lobby');assert.equal(f.$('apply-room').disabled,false);
f.$('leave-room').click();assert.equal(f.env.ryggattackCommand,'"Leave"');assert.equal(f.menu.screen,'lobby','do not hide a still-connected seat');
f.env.ryggattackCommand=null;f.publish({lobby:null,participant:null,status:'Left room; create or join another',notice:''});assert.equal(f.menu.screen,'setup');assert.equal(f.$('join-room').disabled,false);
// A field's Escape only exits editing, then Back is a separate intent.
f=fixture();f.$('play-online').click();f.$('player-name').focus();let backs=0;
const root=f.$('online-entry');root.children=[f.$('play-local'),f.$('play-online')];
navigateControls(root,{accept:true},()=>backs++);assert.equal(backs,0);assert.equal(f.calls.length,0);
navigateControls(root,{cancel:true},()=>backs++);assert.equal(backs,0);assert.equal(f.env.document.activeElement.id,'play-local');
navigateControls(root,{down:true},()=>backs++);assert.equal(f.env.document.activeElement.id,'play-online');
navigateControls(root,{start:true},()=>backs++);assert.equal(f.menu.screen,'setup');
navigateControls(root,{cancel:true},()=>backs++);assert.equal(backs,1);
const cancel=fixture('?online=wss%3A%2F%2Fgame.example%2Fws',new Map([['ryggattack.launch',JSON.stringify(launch)]]));
assert.equal(cancel.$('setup-back').disabled,false);cancel.$('setup-back').click();assert.equal(cancel.env.ryggattackCommand,'"Leave"');
cancel.publish({lobby:null,status:'Left room; create or join another',notice:''});assert.equal(cancel.menu.screen,'setup');assert.equal(cancel.$('create-room').disabled,false);
for(const status of ['room full','room not found','reconnect expired; join lobby again']){const failure=fixture('?room='+code);failure.publish({lobby:null,status,notice:''});assert.equal(failure.menu.screen,'setup');assert.equal(failure.$('room-notice').textContent,status);assert.equal(failure.$('join-room').disabled,false);}
// Handshake is still in flight after Hello; cancel must send Leave, not reload.
const waiting=fixture('?online=wss%3A%2F%2Fgame.example%2Fws',new Map([['ryggattack.launch',JSON.stringify(launch)]]));
waiting.publish({lobby:null,status:'Waiting for seat…',notice:''});
assert(waiting.$('create-room').disabled,'Waiting for seat keeps duplicate admission disabled');
waiting.$('create-room').click();assert.equal(waiting.calls.length,0);
waiting.$('setup-back').click();assert.equal(waiting.env.ryggattackCommand,'"Leave"');assert.equal(waiting.calls.length,0);
// Model visibility correctly: a hidden main-menu button cannot receive focus.
const editing=fixture();editing.$('play-online').click();const field=editing.$('player-name'),hidden=editing.$('play-local'),action=editing.$('create-room');
hidden.getClientRects=()=>[];const visibleRoot=editing.$('online-entry');visibleRoot.children=[hidden,field,action];field.focus();
navigateControls(visibleRoot,{cancel:true},()=>assert.fail('field cancel navigated'));
assert.equal(editing.env.document.activeElement,action,'field cancel focuses a visible action');
field.focus();navigateControls(visibleRoot,{controller:true},()=>assert.fail('idle navigated'));
assert.equal(editing.env.document.activeElement,field,'idle bridge frames preserve editing focus');
const selector=editing.$('target-room');selector.options=[{value:'1'},{value:'3'},{value:'5',disabled:true}];selector.value='1';selector.dispatchEvent=()=>{};selector.focus();
navigateControls(visibleRoot,{controller:true,right:true},()=>assert.fail('setting navigated'));assert.equal(selector.value,'3');assert.equal(editing.env.document.activeElement,selector);
navigateControls(visibleRoot,{controller:true,left:true},()=>assert.fail('setting navigated'));assert.equal(selector.value,'1');
// Clipboard success and unavailable fallbacks remain visible and actionable.
const copied=fixture();copied.publish({lobby,participant:1,status:'Online',notice:''});
let clipboardText;copied.env.navigator.clipboard.writeText=async text=>{clipboardText=text;};
await copied.$('copy-room').onclick();assert.equal(clipboardText,copied.$('share-room').value);assert.equal(copied.$('room-notice').textContent,'Link copied');
copied.env.navigator.clipboard.writeText=async()=>{throw Error('permission denied');};
await copied.$('copy-room').onclick();assert.match(copied.$('room-notice').textContent,/Clipboard unavailable/);assert.equal(copied.env.document.activeElement.id,'share-room');
copied.publish({lobby,participant:2,status:'Online',notice:'Only the host can configure this room'});assert.match(copied.$('room-notice').textContent,/Only the host/);assert(copied.$('apply-room').disabled);
// The in-match opener is not available while its menu is already open.
copied.publish({lobby:{...lobby,phase:'Running'},participant:2,status:'Online',notice:''});assert(!copied.$('open-match-menu').hidden);
copied.$('open-match-menu').click();copied.tick();assert(copied.$('open-match-menu').hidden);assert.equal(copied.menu.screen,'match');
console.log('unified menu: validation, explicit admission, duplicate guard, invitations, scoped resume, lifecycle, authority, focus, clipboard and shared navigation passed');
