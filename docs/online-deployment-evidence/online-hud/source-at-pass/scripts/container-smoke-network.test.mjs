import test from 'node:test';import assert from 'node:assert/strict';import {smokeNetworkArgs,smokeHttpUrl} from './container-smoke-network.mjs';
const id='0123456789abcdef';
test('local smoke publishes only random loopback ports',()=>{
 assert.deepEqual(smokeNetworkArgs([8080]),['-p','127.0.0.1::8080']);
 assert.equal(smokeHttpUrl(8080,'127.0.0.1:45678'),'http://127.0.0.1:45678');
});
test('remote-daemon CI smoke shares the calling job network without host exposure',()=>{
 assert.deepEqual(smokeNetworkArgs([8787,8788],id),['--network','container:'+id]);
 assert.equal(smokeHttpUrl(8788,undefined,id),'http://127.0.0.1:8788');
});
test('network helper rejects malformed namespaces, ports and non-loopback publication',()=>{
 for(const bad of ['', 'host','other:name','../../x'])assert.throws(()=>smokeNetworkArgs([8080],bad));
 for(const bad of [0,65536,'8080'])assert.throws(()=>smokeNetworkArgs([bad]));
 for(const bad of ['', '0.0.0.0:1234','evil.invalid:1234'])assert.throws(()=>smokeHttpUrl(8080,bad));
});
