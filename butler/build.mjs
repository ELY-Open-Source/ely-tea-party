import { build } from 'esbuild';
import { createHash } from 'node:crypto';
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile, chmod } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const root = path.dirname(fileURLToPath(import.meta.url));
const out = path.resolve(root, '../target/butler');
const version = '22.23.4';
const target = `${process.platform === 'win32' ? 'win' : process.platform}-${process.arch}`;
const hashes = {
  'darwin-arm64': 'c9e1dbc748b33ed2d14f42445191ecaf14c56259c7a9a26d0042c49ec2b66428',
  'darwin-x64': 'efcc2b0db6f94bf53eddb038fe2b370277a042bd29e4f19b8f8e23598795b819',
  'linux-arm64': 'f7a1e486dbc8b286847815b477d4fb5e9fb963ed4489c3e1588853d685f589f9',
  'linux-x64': 'b8492284c76a54baf58d61668dd0e41ee0fc98adabf9503d30f05d08f55b4406',
  'win-arm64': 'a8b8cd79cbbb3062435f449591d98dd63730bbbf30ce2d7cc3a6376c236cb111',
  'win-x64': 'cbc74b5e0982c9dbb41525fd8fb5dcd32fbee237863f7829072f4639a1ce5f20',
};
if (!hashes[target]) throw new Error(`Unsupported butler runtime target: ${target}`);
await mkdir(out, { recursive: true });
await build({
  entryPoints: [path.join(root, 'main.mjs')], outfile: path.join(out, 'main.mjs'),
  bundle: true, platform: 'node', target: 'node22', format: 'esm',
  banner: { js: 'import { createRequire as __elyRequire } from "node:module"; const require = __elyRequire(import.meta.url);' },
  legalComments: 'eof',
});
const archive = `node-v${version}-${target}.${process.platform === 'win32' ? 'zip' : 'tar.gz'}`;
const cache = path.resolve(root, '../target/butler-downloads');
await mkdir(cache, { recursive: true });
const archivePath = path.join(cache, archive);
let bytes;
try { bytes = await readFile(archivePath); } catch (error) { if (error.code !== 'ENOENT') throw error; }
if (!bytes) {
  const response = await fetch(`https://nodejs.org/dist/v${version}/${archive}`, { signal: AbortSignal.timeout(120_000) });
  if (!response.ok) throw new Error(`Node runtime download failed: ${response.status}`);
  bytes = Buffer.from(await response.arrayBuffer());
}
if (createHash('sha256').update(bytes).digest('hex') !== hashes[target]) throw new Error('Node runtime checksum mismatch');
await writeFile(archivePath, bytes);
const temp = await mkdtemp(path.join(tmpdir(), 'ely-butler-'));
try {
  execFileSync('tar', ['-xf', archivePath, '-C', temp]);
  const extracted = path.join(temp, `node-v${version}-${target}`);
  const binary = process.platform === 'win32' ? 'node.exe' : 'node';
  await cp(path.join(extracted, process.platform === 'win32' ? binary : `bin/${binary}`), path.join(out, binary));
  await chmod(path.join(out, binary), 0o755);
  await cp(path.join(extracted, 'LICENSE'), path.join(out, 'NODE-LICENSE'));
} finally { await rm(temp, { recursive: true, force: true }); }
const licenses = path.join(out, 'licenses');
await mkdir(licenses, { recursive: true });
async function collect(dir, prefix = '') {
  for (const item of await readdir(dir, { withFileTypes: true })) {
    if (item.name.startsWith('.')) continue;
    const file = path.join(dir, item.name);
    if (item.isDirectory()) {
      if (item.name.startsWith('@')) await collect(file, `${item.name}-`);
      else {
        for (const name of await readdir(file)) {
          if (/^(licen[sc]e|copying|notice)(\.|$)/i.test(name)) {
            await cp(path.join(file, name), path.join(licenses, `${prefix}${item.name}-${name}`), { recursive: true });
          }
        }
      }
    }
  }
}
await collect(path.join(root, 'node_modules'));
await cp(path.join(root, 'PI-LICENSE'), path.join(licenses, 'PI-LICENSE'));
await writeFile(path.join(out, 'versions.json'), JSON.stringify({ pi: '1.4.0', node: version, protocol: 1, target }) + '\n');
console.log(`butler: bundled Pi and Node for ${target} at ${out}`);
