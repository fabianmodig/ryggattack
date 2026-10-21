import assert from 'node:assert/strict';
const port=n=>assert(Number.isInteger(n)&&n>0&&n<=65535,'invalid smoke port');
const namespace=id=>assert(typeof id==='string'&&/^[a-f0-9]{12,64}$/.test(id),'expected the calling CI container ID');
export function smokeNetworkArgs(ports,container) {
 ports.forEach(port);
 if(container!==undefined){namespace(container);return ['--network','container:'+container];}
 return ports.flatMap(n=>['-p',`127.0.0.1::${n}`]);
}
export function smokeHttpUrl(internalPort,published,container) {
 port(internalPort);
 if(container!==undefined){namespace(container);return `http://127.0.0.1:${internalPort}`;}
 assert(typeof published==='string'&&/^127\.0\.0\.1:\d+$/.test(published),'container did not expose an expected loopback port');
 port(Number(published.split(':')[1]));return 'http://'+published;
}
