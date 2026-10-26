import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
const runtime = new URL('../target/butler/', import.meta.url);
const node = fileURLToPath(new URL(process.platform === 'win32' ? 'node.exe' : 'node', runtime));
const script = fileURLToPath(new URL('main.mjs', runtime));
const run = spawnSync(node, [script], { input: '{}', encoding: 'utf8', timeout: 15_000,
  env: { ...process.env, NODE_OPTIONS: '', NODE_PATH: '' } });
assert.equal(run.error, undefined);
assert.equal(run.status, 1);
assert.deepEqual(JSON.parse(run.stdout), { type: 'error', text: 'Invalid butler request' });
console.log('Bundled Node and Pi load and validate the host protocol without a system Node runtime.');
