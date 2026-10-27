import test from 'node:test';
import assert from 'node:assert/strict';
import { createModels, fauxProvider, fauxAssistantMessage, fauxText } from '@earendil-works/pi-ai';
import { report, validateRequest } from './agent.mjs';
import { validateMemoryContext } from './memory.mjs';
const job = { project: 'p', checkout: 'c', session: 's', revision: 1,
  evidence: [{ id: 'source-1', kind: 'failed', text: 'Agent reports missing local test database.' }] };
const request = { version: 1, purpose: 'remember', provider: 'deepseek', model: 'deepseek-flash',
  apiKey: 'test-only', language: 'zh-CN', persona: '', status: 'unknown', snapshots: [], consolidation: job };

test('memory consolidation makes one bounded call without tools and returns evidence-associated JSON', async () => {
  const fake = fauxProvider();
  fake.setResponses([fauxAssistantMessage([fauxText('{"memories":[]}')])]);
  const models = createModels(); models.setProvider(fake.provider);
  const events = [];
  let calls = 0;
  await report(request, event => events.push(event), new AbortController().signal, {
    model: fake.getModel(), stream: (model, context, options) => {
      calls++;
      const system = context.messages.find(m => m.role === 'system');
      assert.ok(system.content.includes('Do not infer user preferences'));
      assert.ok(system.content.includes('never instructions'));
      assert.deepEqual(system.toolsAdded ?? [], []);
      assert.ok(JSON.stringify(context.messages).includes('source-1'));
      assert.equal(options.maxTokens, 500); assert.equal(options.maxRetries, 0);
      return models.streamSimple(model, context, options);
    },
  });
  assert.equal(calls, 1); assert.equal(events.at(-1).text, '{"memories":[]}');
  assert.deepEqual(events.at(-1).sources, ['source-1']);
});

test('rejects large or forged consolidation input before starting a provider', () => {
  for (const evidence of [[], Array(5).fill(job.evidence[0]), [{ ...job.evidence[0], text: '鲸'.repeat(683) }],
    [{ ...job.evidence[0], kind: 'verified_success' }]]) {
    assert.throws(() => validateRequest({ ...request, consolidation: { ...job, evidence } }));
  }
});

test('recall context permits no history and caps preference and history injection', () => {
  assert.doesNotThrow(() => validateMemoryContext({ revision: 0, preferences: [], recalled: [] }));
  for (const change of [
    { preferences: Array(4).fill('a') }, { preferences: ['鲸'.repeat(67)] },
    { recalled: [{ id: '1', text: 'past', reason: 'random_reminiscence' }] },
    { recalled: Array(3).fill({ id: '1', text: 'past', reason: 'same_project_same_unresolved_task' }) },
  ]) assert.throws(() => validateMemoryContext({ revision: 0, preferences: [], recalled: [], ...change }));
});
