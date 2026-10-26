import { report } from './agent.mjs';

let bytes = 0;
const chunks = [];
let key = '';
const parent = process.ppid;
const watchdog = setInterval(() => {
  try { process.kill(parent, 0); }
  catch (error) { if (error.code === 'ESRCH') process.exit(1); }
}, 250);
watchdog.unref();
const emit = event => {
  const encoded = JSON.stringify(event);
  process.stdout.write((key ? encoded.replaceAll(key, '[redacted]') : encoded) + '\n');
};
try {
  for await (const chunk of process.stdin) {
    bytes += chunk.length;
    if (bytes > 2 * 1024 * 1024) throw new Error('Butler input exceeds 2 MiB');
    chunks.push(chunk);
  }
  const request = JSON.parse(Buffer.concat(chunks).toString('utf8'));
  key = typeof request.apiKey === 'string' ? request.apiKey : '';
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(), 110_000);
  try { await report(request, emit, abort.signal); }
  finally { clearTimeout(timer); }
} catch (error) {
  emit({ type: 'error', text: String(error.message ?? error).slice(0, 1500) });
  process.exitCode = 1;
} finally {
  clearInterval(watchdog);
}
