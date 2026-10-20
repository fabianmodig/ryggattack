import assert from 'node:assert/strict';
import {joinUrl, settingsCommand, installRoomControls} from '../web/rooms.mjs';
const code='0123456789abcdef0123';
const link=new URL(joinUrl('https://game.example/play/?name=Old&quality=0','ws://127.0.0.1:8787',code));
assert.equal(link.searchParams.get('room'),code);
assert.equal(link.searchParams.get('online'),'ws://127.0.0.1:8787');
assert.equal(link.searchParams.has('name'),false,'share does not impersonate host name');
assert.equal(link.searchParams.get('quality'),'0');
assert.deepEqual(settingsCommand('Volcano','1','2'),{Configure:{arena:'Volcano',wins_target:1,bots:2}});
for(const args of [['Forest','2','0'],['bad','1','0'],['Forest','1','4']])assert.throws(()=>settingsCommand(...args));
// Exercise the installed handler at its DOM boundary; invalid input must be
// visible, not an uncaught page exception. No socket/gameplay is mocked here.
const elements=new Map();
globalThis.document={getElementById:id=>{if(!elements.has(id))elements.set(id,{});return elements.get(id);}};
globalThis.location={search:'',href:'https://game.example/'};
const interval=globalThis.setInterval;globalThis.setInterval=()=>{};
installRoomControls();globalThis.setInterval=interval;
document.getElementById('arena-room').value='Forest';
document.getElementById('target-room').value='1';
document.getElementById('bots-room').value='4';
assert.doesNotThrow(()=>document.getElementById('apply-room').onclick());
assert.equal(document.getElementById('room-notice').textContent,'Invalid settings');
console.log('rooms UI helpers and visible validation passed');
