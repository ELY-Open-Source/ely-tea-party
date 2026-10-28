import { cp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.dirname(fileURLToPath(import.meta.url));
const dist = path.join(root, 'dist');
const repo = path.dirname(root);
const release = JSON.parse(await readFile(path.join(root, 'release.json'), 'utf8'));
if (!/^\d+\.\d+\.\d+$/.test(release.version)) throw new Error('website: invalid release version');
const values = { VERSION: release.version, RELEASE_URL: release.page };
for (const platform of ['mac', 'windows', 'linux']) {
  if (!release[platform]?.includes(release.version)) throw new Error(`website: invalid ${platform} asset`);
  values[`${platform.toUpperCase()}_URL`] = `${release.base}/${release[platform]}`;
}
values.CHECKSUM_URL = `${release.base}/SHA256SUMS`;
for (const [key, value] of Object.entries(values)) {
  if (key !== 'VERSION' && (!value.startsWith('https://') || /["<>]/.test(value))) {
    throw new Error(`website: invalid URL in ${key}`);
  }
}
await rm(dist, { recursive: true, force: true });
await mkdir(path.join(dist, 'assets', 'agents'), { recursive: true });
await mkdir(path.join(dist, 'assets', 'mascot'), { recursive: true });
for (const file of await readdir(path.join(root, 'src'))) {
  let content = await readFile(path.join(root, 'src', file), 'utf8');
  content = content.replace(/@@([A-Z_]+)@@/g, (_, key) => {
    if (!(key in values)) throw new Error(`website: missing template value ${key}`);
    return values[key];
  });
  await writeFile(path.join(dist, file), content);
}
for (const file of await readdir(path.join(repo, 'assets', 'agents'))) {
  if (/-(working|thinking|needs_input|sleeping)\.webp$/.test(file)) {
    await cp(path.join(repo, 'assets', 'agents', file), path.join(dist, 'assets', 'agents', file));
  }
}
await cp(path.join(repo, 'assets/mascot/whale-idle.webp'), path.join(dist, 'assets/mascot/whale-idle.webp'));
await cp(path.join(repo, 'docs/screenshot.png'), path.join(dist, 'assets/screenshot.png'));
await cp(path.join(repo, 'docs/icon.png'), path.join(dist, 'assets/icon.png'));
await cp(path.join(repo, 'LICENSE'), path.join(dist, 'LICENSE.txt'));
console.log(`website: built Ely Tea Party ${release.version} into ${dist}`);
