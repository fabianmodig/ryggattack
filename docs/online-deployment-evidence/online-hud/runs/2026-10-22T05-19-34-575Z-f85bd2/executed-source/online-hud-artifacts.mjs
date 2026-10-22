import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';

// Use Node's ESM parser, not a regexp that can overlook side-effect imports or
// re-exports. Parsing happens in an isolated child; project code is never run.
const parseImports = `
  import fs from 'node:fs';
  import {SourceTextModule} from 'node:vm';
  const source = fs.readFileSync(0, 'utf8');
  if (/\\bimport(?:\\s|\\/\\*[\\s\\S]*?\\*\\/|\\/\\/[^\\r\\n\\u2028\\u2029]*)*\\(/.test(source))
    throw Error('dynamic imports are outside the static entry contract');
  console.log(JSON.stringify(new SourceTextModule(source).dependencySpecifiers));
`;
const generated = new Set(['ryggattack.js', 'ryggattack_bg.wasm']);

export function checkCopiedEntryArtifacts(sourceDir, artifactDir) {
  const seen = new Set();
  function visit(name) {
    assert(!name.startsWith('../') && !path.isAbsolute(name), `entry dependency escapes web/: ${name}`);
    if (generated.has(name) || seen.has(name)) return;
    seen.add(name);
    const bytes = fs.readFileSync(path.join(sourceDir, name));
    assert.deepEqual(fs.readFileSync(path.join(artifactDir, name)), bytes, `stale dist/web/${name}`);
    function dependency(specifier) {
      assert(specifier.startsWith('./') || specifier.startsWith('../'), `nonlocal entry import: ${specifier}`);
      assert(!/[?#%\\\\]/.test(specifier), `unsupported entry import URL: ${specifier}`);
      visit(path.posix.normalize(path.posix.join(path.posix.dirname(name), specifier)));
    }
    function module(source) {
      const parsed = spawnSync(process.execPath, ['--no-warnings', '--experimental-vm-modules', '--input-type=module', '-e', parseImports], {input: source, encoding: 'utf8', timeout: 10000});
      assert.equal(parsed.status, 0, `cannot parse static entry module ${name}: ${parsed.error?.message ?? parsed.stderr}`);
      for (const specifier of JSON.parse(parsed.stdout)) dependency(specifier);
    }
    if (name === 'index.html') {
      const scripts = [...bytes.toString().matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script\s*>/gi)];
      let modules = 0;
      for (const [, attrs, body] of scripts) {
        if (!/\btype\s*=\s*(["'])module\1/i.test(attrs)) continue;
        modules++;
        const src = attrs.match(/\bsrc\s*=\s*(["'])(.*?)\1/i);
        if (src) dependency(src[2]); else module(body);
      }
      assert(modules > 0, 'index.html must contain a module entry');
    } else {
      assert(/\.(?:mjs|js)$/.test(name), `unsupported source entry module: ${name}`);
      module(bytes.toString());
    }
  }
  visit('index.html');
  return {copied: [...seen].sort(), generated: [...generated].sort()};
}
