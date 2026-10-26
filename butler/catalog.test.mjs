import test from 'node:test';
import assert from 'node:assert/strict';
import { createModels } from '@earendil-works/pi-ai';
import providers from './models.json' with { type: 'json' };

test('every model offered by Settings resolves in the bundled Pi provider', async () => {
  assert.equal(new Set(providers.map(p => p.id)).size, providers.length);
  for (const provider of providers) {
    const factory = await import(`@earendil-works/pi-ai/providers/${provider.id}`);
    const models = createModels();
    models.setProvider(factory[`${provider.id}Provider`]());
    assert.ok(provider.models.length >= 2 && provider.models.length <= 4);
    assert.equal(new Set(provider.models.map(m => m.id)).size, provider.models.length);
    for (const choice of provider.models) {
      const model = models.getModel(provider.id, choice.id);
      assert.ok(model, `${provider.id}/${choice.id} must resolve before it is offered`);
      assert.equal(model.type, 'chat');
    }
  }
});
