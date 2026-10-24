import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {requirePrivateScan} from './client-feedback-helpers.mjs';

export class PrivacyViolation extends Error {
 constructor(){super('private material found; artifacts withheld');this.name='PrivacyViolation';}
}
export async function capturePrivateScreenshot(page,secrets,capture) {
 // Compressed image bytes cannot prove text privacy. Inspect rendered DOM text
 // against observed credentials before asking the browser to capture pixels.
 const text=await page.locator('body').innerText();
 try {requirePrivateScan(text,secrets);}catch {throw new PrivacyViolation();}
 return capture();
}
export function publishSoloEvidence(out,evidence,secrets,{privacyFailed=false}={}) {
 const scan=(directory=out,prefix='')=>fs.readdirSync(directory,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name)).flatMap(entry=>{
  const file=prefix+entry.name,target=path.join(directory,entry.name);
  requirePrivateScan(file,secrets);
  if(entry.isDirectory())return scan(target,file+'/');
  assert(entry.isFile(),'nonregular evidence artifact forbidden');
  const bytes=fs.readFileSync(target);
  // Exact-value checks over every artifact are supplemental, not image OCR.
  requirePrivateScan(bytes.toString('utf8'),secrets);requirePrivateScan(bytes.toString('latin1'),secrets);
  return [{file,sha256:crypto.createHash('sha256').update(bytes).digest('hex')}];
 });
 try {
  assert(!privacyFailed,'earlier screenshot privacy failure');
  requirePrivateScan(JSON.stringify(evidence),secrets);scan();
  evidence.private_material_negative_scan={passed:true,known_values_checked:secrets.size,all_artifacts_exact_value_scan:true,screenshot_text_check:'visible DOM before capture; binary scans are not OCR'};
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(evidence,null,2)+'\n');
  fs.writeFileSync(path.join(out,'sha256.json'),JSON.stringify(scan(),null,2)+'\n');
  scan(); // Includes the final results AND manifest, not just earlier artifacts.
  return true;
 } catch {
  // This directory belongs only to this run. Withhold prior images as well as
  // metadata even if the credential only became visible in a later screenshot.
  for(const entry of fs.readdirSync(out))fs.rmSync(path.join(out,entry),{recursive:true,force:true});
  fs.writeFileSync(path.join(out,'results.json'),JSON.stringify({status:'failed',failure:'private-material scan failed; artifacts withheld'})+'\n');
  return false;
 }
}

export function recordSoloInput(inputs,input) {
 inputs.count++;
 if(input.fire===true) {
  inputs.firing_count++;
  if(input.horizontal===-1)inputs.left_fire_count++;
  if(input.horizontal===0&&input.vertical===0)inputs.neutral_fire_count++;
 }
}
export function hasSoloPolicyInput(bots,inputs) {
 assert([1,2,3].includes(bots),'unsupported bot input policy');
 return (bots===1?inputs.neutral_fire_count:inputs.left_fire_count)>0;
}

export const usage='Usage: node scripts/solo-bots-smoke.mjs --live [--bots 1|2|3]\nExplicitly authorizes ONE disposable private room at https://ryggattack.modig.online. No network/browser actions without --live.';
export function parseSoloArgs(args) {
 let bots=3,live=false;const seen=new Set();
 for(let i=0;i<args.length;i++) {
  const flag=args[i];assert(!seen.has(flag),usage);seen.add(flag);
  if(flag==='--live'){live=true;continue;}
  assert(flag==='--bots',usage);const value=args[++i];assert(/^[123]$/.test(value??''),usage);bots=Number(value);
 }
 assert(live,usage);return {bots};
}
