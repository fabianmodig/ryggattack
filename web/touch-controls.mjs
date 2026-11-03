// CSS-pixel gestures feed device-independent intent, never synthetic keys.
export function createGesture(onCancel=()=>{}) {
  let pointer=null, horizontal=0, vertical=0, fire=false;
  function move(id,x,y) {
    if(pointer?.id!==id)return;
    const dx=x-pointer.x,dy=y-pointer.y;
    if(Math.hypot(dx,dy)<20)return;
    pointer.swiped=true;
    horizontal=Math.abs(dx)>=Math.abs(dy)?Math.sign(dx):0;
    vertical=horizontal?0:-Math.sign(dy);
  }
  const fingers=new Set();
  function reset(){pointer=null;horizontal=vertical=0;fire=false;fingers.clear();}
  return {
    reset,
    hasPointer(id){return fingers.has(id);},
    down(id,x,y,time){
      fingers.add(id);
      // Invalidate already sampled fire without forgetting fingers still held.
      if(fingers.size>1){pointer=null;horizontal=vertical=0;fire=false;onCancel();return;}
      pointer={id,x,y,time,swiped:false};
    },move,
    up(id,x,y,time){
      fingers.delete(id);
      move(id,x,y);
      if(pointer?.id!==id)return;
      fire ||= !pointer.swiped && time-pointer.time<=350;
      pointer=null;
    },
    sample(){const intent={horizontal,vertical,fire};fire=false;return intent;},
  };
}

// One canvas gesture surface; DOM menus keep native scrolling and editing.
export function installTouchControls(env=globalThis) {
  const {document}=env,$=id=>document.getElementById(id),canvas=$('ryggattack-canvas');
  let epoch=0,lastMode='',lastPauseSelection='',destroyed=false;
  const gesture=createGesture(()=>{epoch++;}),listeners=[];
  function reset(){gesture.reset();epoch++;}
  function status(){try{return JSON.parse(env.ryggattackLocalStatus??'null');}catch{return null;}}
  function playing(){
    if(document.hidden||env.ryggattackBrowserMenu)return false;
    const local=status();
    if(!env.ryggattackLaunch)return local?.screen==='Playing'&&!local.paused&&!local.finished;
    try{const h=JSON.parse(env.ryggattackHud??'null');return h?.lobby?.phase==='Running'&&!h.menu&&!h.frame?.round_finished;}catch{return false;}
  }
  function listen(target,name,fn){target.addEventListener(name,fn);listeners.push(()=>target.removeEventListener(name,fn));}
  function handle(kind,event){
    if(event.pointerType!=='touch'&&event.pointerType!=='pen')return;
    if(!playing()){reset();return;}
    if(kind==='down'){canvas.focus();canvas.setPointerCapture(event.pointerId);}
    gesture[kind](event.pointerId,event.clientX,event.clientY,event.timeStamp);
    event.preventDefault();
  }
  for(const [name,kind] of [['pointerdown','down'],['pointermove','move'],['pointerup','up']])listen(canvas,name,event=>handle(kind,event));
  listen(canvas,'pointercancel',reset);
  // Normal implicit capture loss follows up; only unexpected loss cancels intent.
  listen(canvas,'lostpointercapture',event=>{if(gesture.hasPointer(event.pointerId))reset();});
  listen(document,'pointerdown',event=>{if(event.pointerType==='touch'&&event.target!==canvas)reset();});
  listen(env,'blur',reset);listen(document,'visibilitychange',reset);
  listen(canvas,'blur',reset);
  env.ryggattackSampleTouch=()=>{
    if(!playing()||document.activeElement!==canvas)reset();
    return JSON.stringify({...gesture.sample(),epoch});
  };
  function update(){
    const local=status(),online=!!env.ryggattackLaunch;
    const offlineMatch=!online&&local?.screen==='Playing';
    const paused=offlineMatch&&!!local.paused;
    const wasPaused=!$('offline-pause').hidden;
    $('game').classList.toggle('offline-playing',offlineMatch);
    $('offline-hud').hidden=!offlineMatch;
    const text=env.ryggattackOfflineHud??'';
    if($('offline-hud').textContent!==text)$('offline-hud').textContent=text;
    $('offline-pause').hidden=!paused;
    if(paused&&!wasPaused){env.ryggattackPauseSelection='resume';$('help-resume').focus();}
    const selection=paused?env.ryggattackPauseSelection??'':'';
    if(paused&&selection&&selection!==lastPauseSelection)$(selection==='back'?'help-back':'help-resume').focus();
    lastPauseSelection=selection;
    if(!paused&&wasPaused&&offlineMatch)canvas.focus();
    $('touch-controls').classList.toggle('offline-match',offlineMatch);
    $('touch-back').textContent=offlineMatch?'MENU':'BACK TO MENU';
    const mode=online?'online':`${local?.screen}:${local?.paused}:${local?.finished}`;
    if(mode!==lastMode||!playing()){reset();lastMode=mode;}
    canvas.classList.toggle('touch-playing',playing());
    $('touch-controls').hidden=online||env.ryggattackBrowserMenu||!local||local.screen==='MainMenu'||paused;
    $('touch-lobby').hidden=local?.screen!=='Lobby';
    $('touch-playing-controls').hidden=local?.screen!=='Playing';
    $('touch-pause').hidden=!!local?.paused||!!local?.finished;
    $('touch-resume').hidden=!local?.paused;
    $('touch-restart').hidden=!local?.winner;
    $('touch-hint').textContent=''; // instructions live in pause help, never over play
    $('touch-join').textContent=local?.joined?'CART JOINED':'JOIN CART';
    $('touch-start').disabled=!local?.joined;
    if(local){$('touch-bots').textContent=`BOTS: ${local.bots??3}`;$('touch-target').textContent=`FIRST TO: ${local.target??3}`;$('touch-arena').textContent=local.arena??'Forest';}
  }
  for(const [id,action] of [['help-resume','resume'],['help-back','back'],['touch-join','join'],['touch-start','start'],['touch-pause','pause'],['touch-resume','resume'],['touch-restart','restart'],['touch-back','back'],['touch-bots','bots'],['touch-target','target'],['touch-arena','arena']]){
    $(id).onclick=()=>{reset();env.ryggattackTouchAction=action;canvas.focus();};
  }
  // Keep the existing Bevy keyboard/gamepad menu and DOM focus in agreement.
  for(const [id,action] of [['help-resume','resume'],['help-back','back']])listen($(id),'focus',()=>{env.ryggattackPauseDomFocus=action;});
  listen($('offline-pause'),'keydown',event=>{
    if(event.key==='Escape'){event.preventDefault();reset();env.ryggattackTouchAction='resume';}
    if(event.key==='Tab'||event.key==='ArrowUp'||event.key==='ArrowDown'||event.code==='KeyW'||event.code==='KeyS'||/^[ws]$/i.test(event.key??'')){event.preventDefault();(document.activeElement===$('help-resume')?$('help-back'):$('help-resume')).focus();}
  });
  const timer=env.setInterval(update,100);update();
  return {destroy(){if(destroyed)return;destroyed=true;reset();env.clearInterval(timer);for(const off of listeners)off();delete env.ryggattackSampleTouch;}};
}
