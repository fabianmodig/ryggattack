import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {checkCopiedEntryArtifacts} from './online-hud-artifacts.mjs';

function fixture(t) {
  fs.mkdirSync(os.tmpdir(), {recursive: true});
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'online-hud-artifacts-'));
  t.after(() => fs.rmSync(root, {recursive: true, force: true}));
  const source = path.join(root, 'web'), artifacts = path.join(root, 'dist');
  const put = (dir, name, bytes) => {
    const file = path.join(dir, name);
    fs.mkdirSync(path.dirname(file), {recursive: true}); fs.writeFileSync(file, bytes);
  };
  const copied = (name, bytes) => { put(source, name, bytes); put(artifacts, name, bytes); };
  copied('index.html', '<script type="module">import init from "./ryggattack.js"; import "./app.mjs";</script>');
  copied('app.mjs', 'export const ready = true;');
  put(artifacts, 'ryggattack.js', '// generated bindgen JS, not a copied source module');
  put(artifacts, 'ryggattack_bg.wasm', Buffer.from([0, 97, 115, 109]));
  return {source, artifacts, put, copied};
}

test('fixtures create a missing configured scratch parent', t => {
  const outer = fixture(t);
  const original = process.env.TMPDIR;
  const scratch = path.join(path.dirname(outer.source), 'missing', 'scratch');
  try {
    process.env.TMPDIR = scratch;
    const nested = fixture(t);
    assert(nested.source.startsWith(scratch + path.sep));
  } finally {
    if (original === undefined) delete process.env.TMPDIR;
    else process.env.TMPDIR = original;
  }
});

test('unreferenced retired HTML is not a game artifact requirement', t => {
  const f = fixture(t);
  f.put(f.source, 'pages-redirect.html', '<meta http-equiv="refresh" content="0;url=https://retired.invalid/">');
  assert.doesNotThrow(() => checkCopiedEntryArtifacts(f.source, f.artifacts));
});

test('mismatched reachable copied module bytes fail', t => {
  const f = fixture(t);
  f.put(f.artifacts, 'app.mjs', 'export const ready = false;');
  assert.throws(() => checkCopiedEntryArtifacts(f.source, f.artifacts), /stale dist\/web\/app\.mjs/);
});

test('module script src, nested re-exports, cycles and side-effect imports are traversed', t => {
  const f = fixture(t);
  f.copied('index.html', '<script type="module" src="./app.mjs"></script>');
  f.copied('app.mjs', 'export {ready} from "./nested/start.mjs";');
  f.copied('nested/start.mjs', 'import "../app.mjs"; import "../ryggattack.js"; export const ready = true;');
  assert.deepEqual(checkCopiedEntryArtifacts(f.source, f.artifacts), {
    copied: ['app.mjs', 'index.html', 'nested/start.mjs'],
    generated: ['ryggattack.js', 'ryggattack_bg.wasm']
  });
});

test('missing reachable nested dist module fails', t => {
  const f = fixture(t);
  f.copied('app.mjs', 'import "./nested/required.mjs";');
  f.copied('nested/required.mjs', 'export {};');
  fs.unlinkSync(path.join(f.artifacts, 'nested/required.mjs'));
  assert.throws(() => checkCopiedEntryArtifacts(f.source, f.artifacts), /required\.mjs/);
});

test('mismatched nested mjs bytes fail', t => {
  const f = fixture(t);
  f.copied('app.mjs', 'export * from "./nested/required.mjs";');
  f.copied('nested/required.mjs', 'export const ready = true;');
  f.put(f.artifacts, 'nested/required.mjs', 'export const ready = false;');
  assert.throws(() => checkCopiedEntryArtifacts(f.source, f.artifacts), /stale dist\/web\/nested\/required\.mjs/);
});

test('dynamic imports cannot silently escape the static graph', t => {
  const f = fixture(t);
  f.copied('app.mjs', 'export const ready = () => import // dependency\n ("./missing.mjs");');
  assert.throws(() => checkCopiedEntryArtifacts(f.source, f.artifacts), /dynamic imports are outside the static entry contract/);
});

test('missing reachable source module fails rather than silently skipping it', t => {
  const f = fixture(t);
  fs.unlinkSync(path.join(f.source, 'app.mjs'));
  assert.throws(() => checkCopiedEntryArtifacts(f.source, f.artifacts), /app\.mjs/);
});
