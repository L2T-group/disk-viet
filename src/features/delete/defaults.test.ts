import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { NodeView } from '../../lib/types.ts';
import { defaultMode, lockedMode, totals } from './defaults.ts';

const n = (o: Partial<NodeView>) =>
  ({ safety: 'unknown', needsAdmin: false, ruleId: null, size: 0, tool: null, ...o }) as NodeView;

test('safety levels pick the default mode', () => {
  assert.equal(defaultMode(n({ safety: 'safe' })), 'permanent');
  assert.equal(defaultMode(n({ safety: 'caution' })), 'trash');
  assert.equal(defaultMode(n({ safety: 'important' })), 'trash');
  assert.equal(defaultMode(n({ safety: 'unknown' })), 'trash');
});

test('needsAdmin and the Trash rule are locked to permanent', () => {
  assert.equal(defaultMode(n({ safety: 'caution', needsAdmin: true })), 'permanent');
  assert.equal(lockedMode(n({ safety: 'caution', needsAdmin: true })), 'permanent');
  assert.equal(defaultMode(n({ safety: 'caution', ruleId: 'trash' })), 'permanent');
  assert.equal(lockedMode(n({ safety: 'caution', ruleId: 'trash' })), 'permanent');
  assert.equal(lockedMode(n({ safety: 'safe' })), null);
});

test('totals split bytes by mode and count risky permanent rows', () => {
  const t = totals([
    { node: n({ safety: 'safe', size: 5 }), mode: 'permanent' },
    { node: n({ safety: 'important', size: 7 }), mode: 'permanent' },
    { node: n({ safety: 'unknown', size: 11 }), mode: 'trash' },
    { node: n({ safety: 'caution', size: 13 }), mode: 'permanent' },
  ]);
  assert.deepEqual(t, { permanent: 25, trash: 11, tool: 0, risky: 1 });
});

test('tool rows are counted apart, never as Trash or risky', () => {
  const t = totals([
    { node: n({ safety: 'safe', size: 3, tool: 'brew' }), mode: 'trash' },
    { node: n({ safety: 'unknown', size: 2, tool: 'docker' }), mode: 'permanent' },
    { node: n({ safety: 'safe', size: 5 }), mode: 'permanent' },
  ]);
  assert.deepEqual(t, { permanent: 5, trash: 0, tool: 5, risky: 0 });
});
