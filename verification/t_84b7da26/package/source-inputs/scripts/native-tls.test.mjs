// The native adapter must implement WSS and reject an untrusted certificate,
// not silently downgrade or fail merely because TLS support was omitted.
import assert from 'node:assert/strict';
import https from 'node:https';
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import {execFileSync,spawn} from 'node:child_process';
const out='verification/native-tls';fs.mkdirSync(out,{recursive:true});
execFileSync('openssl',['req','-x509','-newkey','rsa:2048','-nodes','-keyout',out+'/key.pem','-out',out+'/cert.pem','-days','1','-subj','/CN=localhost','-addext','subjectAltName=DNS:localhost,IP:127.0.0.1'],{stdio:'ignore'});
const server=https.createServer({maxVersion:'TLSv1.2',key:fs.readFileSync(out+'/key.pem'),cert:fs.readFileSync(out+'/cert.pem')});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const sockets=new Set();let first=Buffer.alloc(0);const serverBytes=[];
const proxy=net.createServer(down=>{const up=net.connect(server.address().port,'127.0.0.1');for(const s of [down,up]){sockets.add(s);s.on('error',()=>{up.destroy();down.destroy();});s.on('close',()=>sockets.delete(s));}down.on('data',b=>{if(!first.length)first=b.subarray(0,8);});up.on('data',b=>serverBytes.push(b));down.pipe(up);up.pipe(down);});
await new Promise(r=>proxy.listen(0,'127.0.0.1',r));
let connected=false;const tlsErrors=[];server.on('secureConnection',()=>connected=true);server.on('tlsClientError',e=>tlsErrors.push(e.message));
try{
 const child=spawn(path.resolve(process.env.NATIVE_PROBE_BIN??'target/debug/ryggattack-net-probe'),[`wss://127.0.0.1:${proxy.address().port}/ws`,'2']);let stderr='';child.stderr.on('data',b=>stderr+=b);
 const result=await new Promise(r=>child.once('exit',r));
 assert.notEqual(result,0);if(!first.length)console.error({stderr,tlsErrors});assert.equal(first[0],22,'native WSS must send a real TLS ClientHello');assert.equal(first[1],3);const cert=Buffer.from(fs.readFileSync(out+'/cert.pem','utf8').replace(/-----[^-]+-----/g,'').replace(/\s/g,''),'base64');assert(Buffer.concat(serverBytes).includes(cert),'TLS server actually supplied the untrusted certificate before native rejection');assert(!connected,'untrusted TLS server cannot finish application handshake');
 console.log('Native shared adapter supports WSS and fails closed on an untrusted local TLS certificate');
}finally{for(const s of sockets)s.destroy();proxy.close();server.closeAllConnections();server.close();}
