import assert from 'node:assert/strict';
import {test} from 'node:test';
import {roomStatusHas} from './resilience-helpers.mjs';
test('status waits tolerate only unpublished initialization and require the actual expected status',()=>{
 delete globalThis.ryggattackRoom;assert.equal(roomStatusHas('room full'),false);
 globalThis.ryggattackRoom='{}';assert.equal(roomStatusHas('room full'),false);
 globalThis.ryggattackRoom=JSON.stringify({status:'Connecting…'});assert.equal(roomStatusHas('room full'),false);
 globalThis.ryggattackRoom=JSON.stringify({status:'room full'});assert.equal(roomStatusHas('room full'),true);
 globalThis.ryggattackRoom='broken JSON';assert.throws(()=>roomStatusHas('room full'));
 delete globalThis.ryggattackRoom;
});
