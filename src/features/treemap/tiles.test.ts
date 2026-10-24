import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { NodeView } from '../../lib/types.ts';
import { buildTiles, HEADER, innerTiles, minShare, nestable, nodeLabel, OTHER, UNEXPLAINED, type Tile } from './tiles.ts';

const n = (id: number, size: number, extra: Partial<NodeView> = {}): NodeView => ({
  id,
  name: `n${id}`,
  path: `/n${id}`,
  size,
  isDir: true,
  ruleId: null,
  safety: 'unknown',
  needsAdmin: false,
  tool: null,
  toolCommand: null,
  toolMissing: false,
  toolChild: false,
  unreadable: false,
  smallBytes: 0,
  smallCount: 0,
  hasChildren: false,
  remainder: false,
  ...extra,
});

test('small children merge into "other" with smallBytes; tiles sum to folder size', () => {
  // 1000 = 900 + 4 + 3 (< 0.5% = 5) + 93 small bytes
  const level = {
    node: n(0, 1000, { smallBytes: 93, smallCount: 7 }),
    children: [n(1, 900), n(2, 4), n(3, 3)],
    crumbs: [],
    unexplained: null,
  };
  const tiles = buildTiles(level);
  assert.deepEqual(
    tiles.map((t) => [t.id, t.size, t.count]),
    [
      [1, 900, 1],
      [OTHER, 100, 9],
    ],
  );
  assert.equal(
    tiles.reduce((s, t) => s + t.size, 0),
    1000,
  );
});

test('unexplained is its own tile, no "other" when nothing is small', () => {
  const level = { node: n(0, 600), children: [n(1, 600)], crumbs: [], unexplained: 400 };
  assert.deepEqual(
    buildTiles(level).map((t) => [t.id, t.size]),
    [
      [1, 600],
      [UNEXPLAINED, 400],
    ],
  );
});

test('nestable: big tiles with folders only; never system, unreadable or synthetic', () => {
  const big = { x: 0, y: 0, w: 160, h: 120 };
  const tile = (extra: Partial<NodeView>): Tile => ({ id: 1, size: 1, node: n(1, 1, { hasChildren: true, ...extra }), count: 1 });
  assert.equal(nestable(tile({}), big), true);
  assert.equal(nestable(tile({}), { ...big, w: 159 }), false);
  assert.equal(nestable(tile({}), { ...big, h: 119 }), false);
  assert.equal(nestable(tile({ hasChildren: false }), big), false);
  assert.equal(nestable(tile({ safety: 'system' }), big), false);
  assert.equal(nestable(tile({ unreadable: true }), big), false);
  assert.equal(nestable({ id: OTHER, size: 1, node: null, count: 3 }, big), false);
});

test('innerTiles: fill the area under the header; tiny children merge into "other"', () => {
  // Remainder tile shown as 100 (its node is bigger): 60 + 37 + 1 (< 0.5) ... + 2 small bytes
  const parent = n(5, 500, { smallBytes: 2, smallCount: 4 });
  const tile: Tile = { id: 5, size: 100, node: parent, count: 1 };
  const r = { x: 0, y: 0, w: 206, h: 141 };
  const inner = innerTiles(tile, [n(6, 60), n(7, 37.6), n(8, 0.4)], r);
  assert.deepEqual(
    inner.map((x) => [x.tile.id, x.tile.size]),
    [
      [6, 60],
      [7, 37.6],
      [OTHER, 2.4],
    ],
  );
  const area = inner.reduce((s, x) => s + x.r.w * x.r.h, 0);
  assert.ok(Math.abs(area - 200 * 100) < 1e-6, String(area));
  assert.ok(inner.every((x) => x.r.y >= HEADER && x.r.x >= 3 && x.r.x + x.r.w <= 203 + 1e-9));
});

test('nodeLabel: rule name only where friendly, never for remainders', () => {
  const rn = (id: string) => `R:${id}`;
  const avd = n(1, 1, { name: 'Pixel_8.avd', ruleId: 'android_avd' });
  assert.equal(nodeLabel(avd, true, rn), 'R:android_avd');
  assert.equal(nodeLabel(avd, false, rn), 'Pixel_8.avd');
  assert.equal(nodeLabel({ ...avd, remainder: true }, true, rn), 'Pixel_8.avd');
  assert.equal(nodeLabel(n(2, 1, { name: 'Desktop' }), true, rn), 'Desktop');
});

test('innerTiles: nothing when the only child would repeat the parent', () => {
  const tile: Tile = { id: 5, size: 100, node: n(5, 100), count: 1 };
  assert.deepEqual(innerTiles(tile, [n(6, 100, { name: 'n5' })], { x: 0, y: 0, w: 300, h: 200 }), []);
});

test('innerTiles: children too small to draw (< MIN_AREA px²) merge into "other"', () => {
  // Area 200×100 = 20,000 px² → share 1024/20000 ≈ 5.1%: the 4% child joins "other".
  const tile: Tile = { id: 5, size: 100, node: n(5, 100), count: 1 };
  const inner = innerTiles(tile, [n(6, 60), n(7, 36), n(8, 4)], { x: 0, y: 0, w: 206, h: 141 });
  assert.deepEqual(
    inner.map((x) => [x.tile.id, x.tile.size]),
    [
      [6, 60],
      [7, 36],
      [OTHER, 4],
    ],
  );
  assert.equal(minShare(0), 0.005);
  assert.equal(minShare(10_000_000), 0.005);
});
