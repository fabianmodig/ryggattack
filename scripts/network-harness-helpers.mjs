// A completed match stops input traffic. Never confuse that with wrong intent,
// and never let a round-boundary retry hide an actual non-neutral packet.
export function neutralWindow(samples,phase) {
  if(samples.some(i=>i.fire||i.horizontal!==0||i.vertical!==0))throw Error('non-neutral form input');
  if(samples.length>=3)return 'passed';
  return phase==='Running'?'wait':'retry-round';
}
