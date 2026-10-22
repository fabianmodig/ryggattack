import {mountOnlineHud} from './online-hud.mjs';

// Presentation bridge only. No gameplay commands, frame mutation or rAF work:
// refresh calibration must remain independent of game/render throughput.
export function installOnlineHud(env=globalThis,mount=mountOnlineHud) {
 const hud=mount(env.document.getElementById('game'));
 let destroyed=false;
 const fallback=()=>{
  env.ryggattackHudMounted=false;hud.element.hidden=true;
  hud.update({});
 };
 function update() {
  if(destroyed)return;
  try {
   const raw=env.ryggattackHud;
   const value=typeof raw==='string'?JSON.parse(raw):null;
   if(!value||typeof value!=='object'||Array.isArray(value)||typeof value.status!=='string'){fallback();return;}
   hud.update(value);hud.element.hidden=false;env.ryggattackHudMounted=true;
  } catch {fallback();}
 }
 update();
 const timer=env.setInterval(update,100);
 return {element:hud.element,update,destroy(){
  if(destroyed)return;destroyed=true;env.clearInterval(timer);env.ryggattackHudMounted=false;hud.destroy();
 }};
}
