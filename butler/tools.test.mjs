import test from 'node:test';
import assert from 'node:assert/strict';
import { terminalTools } from './tools.mjs';

const snapshot = { id: 'a', title: 'Session A', url: 'ely://session/12345678-1234-4234-8234-123456789abc', folder: '/tmp', capturedAt: '2026-10-25T10:00:00Z',
  alternateScreen: true, truncated: true, lines: ['old output', 'FAILED build', 'need input'] };
const tools = terminalTools([snapshot]);
const call = async (name, args) => JSON.parse((await tools.find(t => t.name === name).execute('id', args)).content[0].text);

test('session listing excludes terminal contents', async () => {
  const [listed] = await call('list_sessions', {});
  assert.equal(listed.lines, 3);
  assert.equal(listed.url, snapshot.url);
  assert.equal(listed.link, `[Session A](${snapshot.url})`);
  assert.equal(listed.capturedAt, snapshot.capturedAt);
  assert.ok(!JSON.stringify(listed).includes('FAILED'));
});
test('reading preserves source and snapshot limits', async () => {
  const r = await call('read_terminal', { sessionId: 'a', count: 2 });
  assert.deepEqual(r.lines, [{ line: 2, text: 'FAILED build' }, { line: 3, text: 'need input' }]);
  assert.equal(r.truncated, true);
  assert.equal(r.alternateScreen, true);
  await assert.rejects(call('read_terminal', { sessionId: 'missing' }), /Unknown terminal/);
});
test('search is literal and case-insensitive', async () => {
  assert.equal((await call('search_terminal', { sessionId: 'a', query: 'failed' })).matches[0].line, 2);
  assert.deepEqual((await call('search_terminal', { sessionId: 'a', query: '.*' })).matches, []);
});

test('repeated reads cannot grow the provider context without bound', async () => {
  const tools = terminalTools([{ ...snapshot, lines: ['x'.repeat(30_000)] }]);
  const read = tools.find(t => t.name === 'read_terminal');
  await read.execute('1', { sessionId: 'a' });
  await read.execute('2', { sessionId: 'a' });
  await assert.rejects(read.execute('3', { sessionId: 'a' }), /budget exceeded/);
});


test('same-name sessions carry distinct host addresses and escaped link labels', async () => {
  const a = { ...snapshot, title: 'Agent [same] \\ tab' };
  const b = { ...a, id: 'b', url: 'ely://session/87654321-1234-4234-8234-123456789abc' };
  const tools = terminalTools([a, b]);
  const listed = JSON.parse((await tools[0].execute('list', {})).content[0].text);
  assert.notEqual(listed[0].url, listed[1].url);
  assert.equal(listed[0].link, String.raw`[Agent \[same\] \\ tab](${a.url})`);
  const read = JSON.parse((await tools[1].execute('read', { sessionId: 'b' })).content[0].text);
  assert.equal(read.url, b.url);
  assert.equal(read.link, listed[1].link);
});
