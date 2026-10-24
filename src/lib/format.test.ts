import { test } from 'node:test';
import assert from 'node:assert/strict';
import { formatBytes } from './format.ts';

test('formatBytes uses decimal units', () => {
  assert.equal(formatBytes(0), '0 B');
  assert.equal(formatBytes(999), '999 B');
  assert.equal(formatBytes(850e6), '850 MB');
  assert.equal(formatBytes(12e9), '12.0 GB');
  assert.equal(formatBytes(999.96e6), '1.0 GB'); // rounding bumps the unit
  assert.equal(formatBytes(2.5e12), '2.5 TB');
});
