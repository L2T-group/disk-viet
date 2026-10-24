import { test } from 'node:test';
import assert from 'node:assert/strict';
import { squarify, type Rect } from './squarify.ts';

const rect: Rect = { x: 10, y: 20, w: 600, h: 400 };
const mk = (sizes: number[]) => sizes.map((size, id) => ({ id, size }));
const area = (r: Rect) => r.w * r.h;

test('empty input gives []', () => {
  assert.deepEqual(squarify([], rect), []);
});

test('drops size <= 0', () => {
  const out = squarify(mk([5, 0, -1]), rect);
  assert.deepEqual(out.map((r) => r.id), [0]);
});

test('areas sum to rect area and are proportional to size', () => {
  const sizes = [500, 300, 200, 90, 40, 40, 10, 5, 1];
  const out = squarify(mk(sizes), rect);
  const total = sizes.reduce((a, b) => a + b, 0);
  assert.ok(Math.abs(out.reduce((s, r) => s + area(r), 0) - area(rect)) < 1e-6);
  out.forEach((r, i) => {
    assert.ok(Math.abs(area(r) - (sizes[i]! / total) * area(rect)) < 1e-6);
  });
});

test('no cell leaves the rect', () => {
  const e = 1e-9;
  for (const r of squarify(mk([50, 40, 30, 20, 10, 5, 3, 1]), rect)) {
    assert.ok(r.x >= rect.x - e && r.y >= rect.y - e);
    assert.ok(r.x + r.w <= rect.x + rect.w + e && r.y + r.h <= rect.y + rect.h + e);
  }
});

test('paper example: worst aspect ratio <= 3', () => {
  const out = squarify(mk([6, 6, 4, 3, 2, 2, 1]), { x: 0, y: 0, w: 6, h: 4 });
  const worst = Math.max(...out.map((r) => Math.max(r.w / r.h, r.h / r.w)));
  assert.ok(worst <= 3, `worst=${worst}`);
});

test('2000 items finish under 50 ms', () => {
  const sizes = Array.from({ length: 2000 }, (_, i) => 2000 - i);
  const t = performance.now();
  const out = squarify(mk(sizes), { x: 0, y: 0, w: 1920, h: 1080 });
  assert.equal(out.length, 2000);
  assert.ok(performance.now() - t < 50);
});

const inside = (r: Rect, o: Rect) =>
  [r.x, r.y, r.w, r.h].every(Number.isFinite) &&
  r.x >= o.x - 1e-9 && r.y >= o.y - 1e-9 &&
  r.x + r.w <= o.x + o.w + 1e-9 && r.y + r.h <= o.y + o.h + 1e-9;

test('degenerate rect (w=0 or h=0): no tiles', () => {
  for (const o of [{ x: 5, y: 5, w: 0, h: 100 }, { x: 5, y: 5, w: 100, h: 0 }]) {
    assert.equal(squarify(mk([3, 2, 1]), o).length, 0);
  }
});

test('one huge + 500 tiny: area preserved, inside', () => {
  const out = squarify(mk([1e9, ...Array<number>(500).fill(1)]), rect);
  assert.ok(Math.abs(out.reduce((s, r) => s + area(r), 0) - area(rect)) < 1e-6);
  assert.ok(out.every((r) => inside(r, rect)));
});

test('all-equal sizes: equal areas', () => {
  const out = squarify(mk(Array<number>(10).fill(7)), rect);
  assert.equal(out.length, 10);
  for (const r of out) assert.ok(Math.abs(area(r) - area(rect) / 10) < 1e-6);
});

test('single item fills the rect', () => {
  const [r] = squarify(mk([42]), rect);
  for (const key of ["x", "y", "w", "h"] as const) assert.ok(Math.abs(r![key] - rect[key]) < 1e-9);
});
