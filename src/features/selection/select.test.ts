import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { LevelView, NodeView } from '../../lib/types.ts';
import { reopenId, topLevelOnly } from './select.ts';

const p = (path: string) => ({ path }) as NodeView;
const paths = (a: string[]) => topLevelOnly(a.map(p)).map((x) => x.path);

test('selection drops descendants', () => {
  assert.deepEqual(paths(['/a', '/a/b', '/ab']), ['/a', '/ab']);
  assert.deepEqual(paths(['/a/b', '/a']), ['/a']);
});

test('equal paths collapse to one', () => {
  assert.deepEqual(paths(['/a', '/a']), ['/a']);
});

test('root "/" swallows everything else', () => {
  assert.deepEqual(paths(['/a', '/', '/a/b']), ['/']);
});

test('reopen: deepest crumb outside every deleted path', () => {
  const n = (id: number, path: string) => ({ id, path }) as NodeView;
  const level = { node: n(3, '/r/a/x/y'), crumbs: [n(0, '/r'), n(1, '/r/a'), n(2, '/r/a/x')] } as LevelView;
  assert.equal(reopenId(level, []), 3);
  assert.equal(reopenId(level, ['/r/a/xy', '/r/b']), 3);
  assert.equal(reopenId(level, ['/r/a/x/y']), 2);
  assert.equal(reopenId(level, ['/r/a/x']), 1);
  assert.equal(reopenId(level, ['/r/a', '/r/a/x/y']), 0);
});
