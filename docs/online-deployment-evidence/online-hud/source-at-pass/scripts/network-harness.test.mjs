import test from 'node:test';
import assert from 'node:assert/strict';
import {neutralWindow} from './network-harness-helpers.mjs';
const neutral={horizontal:0,vertical:0,fire:false};
test('a genuine round boundary retries rather than claiming an empty input window passed or failed neutral intent',()=>{
 assert.equal(neutralWindow([],'Lobby'),'retry-round');
 assert.equal(neutralWindow([neutral],'Lobby'),'retry-round');
 assert.equal(neutralWindow([],'Running'),'wait');
 assert.equal(neutralWindow([neutral,neutral],'Running'),'wait');
 assert.equal(neutralWindow([neutral,neutral,neutral],'Running'),'passed');
});
test('non-neutral form input always fails, including across a natural match boundary',()=>{
 for(const phase of ['Running','Lobby'])for(const bad of [{...neutral,fire:true},{...neutral,horizontal:1},{...neutral,vertical:-1}])assert.throws(()=>neutralWindow([bad],phase),/non-neutral/);
});
