// Test-only ordered latency/jitter fixture. Each chunk gets an absolute due
// time; delays overlap instead of applying a packet-size-dependent bandwidth cap.
export function delayPipe(source,target,delay,{maxBytes=1024*1024,now=()=>performance.now(),setTimer=setTimeout,clearTimer=clearTimeout}={}){
 const stats={queued:0,peak:0,chunks:0,delivered:0};
 let queue=[],timer=null,lastDue=0,blocked=false,closed=false;
 const close=()=>{closed=true;if(timer)clearTimer(timer);timer=null;queue=[];stats.queued=0;};
 const fail=()=>{close();source.destroy(Error('bounded delay fixture overflow'));target.destroy(Error('bounded delay fixture overflow'));};
 const arm=()=>{if(closed||blocked||timer||!queue.length)return;timer=setTimer(flush,Math.max(0,queue[0].due-now()));};
 const flush=()=>{timer=null;if(closed||blocked)return;
  while(queue.length&&queue[0].due<=now()){
   const item=queue.shift();if(item.end){target.end();close();return;}
   stats.queued-=item.data.length;stats.delivered++;
   if(!target.write(item.data)){blocked=true;source.pause();target.once('drain',()=>{blocked=false;source.resume();arm();});break;}
  }arm();
 };
 const enqueue=item=>{const ms=delay();if(!Number.isFinite(ms)||ms<0||ms>1000){fail();return;}lastDue=Math.max(lastDue,now()+ms);queue.push({...item,due:lastDue});arm();};
 source.on('data',data=>{if(closed)return;if(stats.queued+data.length>maxBytes){fail();return;}stats.queued+=data.length;stats.peak=Math.max(stats.peak,stats.queued);stats.chunks++;enqueue({data});});
 source.on('end',()=>enqueue({end:true}));
 source.on('close',close);target.on('close',close);
 return stats;
}
