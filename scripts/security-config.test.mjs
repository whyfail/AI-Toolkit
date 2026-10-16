import assert from 'node:assert/strict';
import test from 'node:test';
import { resolveConfig } from 'vite';

test('native signing secrets are excluded from the frontend environment', async () => {
  const keys = ['TAURI_SIGNING_PRIVATE_KEY', 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD'];
  const previous = keys.map((key) => process.env[key]);
  try {
    keys.forEach((key) => { process.env[key] = `synthetic-${key}`; });
    const config = await resolveConfig({}, 'build', 'production');
    for (const key of keys) assert.equal(config.env[key], undefined);
    assert.deepEqual(config.envPrefix, ['VITE_']);
  } finally {
    keys.forEach((key, index) => {
      if (previous[index] === undefined) delete process.env[key];
      else process.env[key] = previous[index];
    });
  }
});
