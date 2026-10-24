import type { LevelView, NodeView } from '../../lib/types.ts';
import { squarify, type Rect } from './squarify.ts';

export const OTHER = -1;
export const UNEXPLAINED = -2;

/** A treemap tile: a real node, or the synthetic "other" / "unexplained" tile (`node: null`). */
export interface Tile {
  id: number;
  size: number;
  node: NodeView | null;
  /** Items merged into the "other" tile. */
  count: number;
}

/** Smallest tile area (px²) worth drawing on its own; smaller children merge into "other". */
export const MIN_AREA = 32 * 32;
/** Tiles thinner than this (px) are not drawn: a sliver would fill the gap between tiles. */
export const MIN_SIDE = 8;

/** Share of the total below which a child merges into "other", for a drawing area of `px` px². */
export const minShare = (px: number) => (px > 0 ? Math.max(0.005, MIN_AREA / px) : 0.005);

/**
 * Tiles of a level, sorted by size desc. Children under `share` of the level total merge with the
 * folder's small files (`smallBytes`) into "other"; `unexplained` is its own tile.
 */
export function buildTiles(level: LevelView, share = 0.005): Tile[] {
  const min = (level.node.size + (level.unexplained ?? 0)) * share;
  const tiles: Tile[] = [];
  let otherSize = level.node.smallBytes;
  let otherCount = level.node.smallCount;
  for (const c of level.children) {
    if (c.size >= min) tiles.push({ id: c.id, size: c.size, node: c, count: 1 });
    else {
      otherSize += c.size;
      otherCount++;
    }
  }
  if (otherSize > 0) tiles.push({ id: OTHER, size: otherSize, node: null, count: otherCount });
  if (level.unexplained) tiles.push({ id: UNEXPLAINED, size: level.unexplained, node: null, count: 0 });
  return tiles.sort((a, b) => b.size - a.size);
}

/**
 * Rule name where tiles are "what can I clean" aggregates (the overview and the tiles inside its
 * tiles); the real name elsewhere, so siblings sharing a rule stay distinguishable.
 */
export const nodeLabel = (n: NodeView, friendly: boolean, ruleName: (ruleId: string) => string) =>
  friendly && n.ruleId && !n.remainder ? ruleName(n.ruleId) : n.name;

/** Height kept for a nesting tile's own name + size. */
export const HEADER = 38;
/** Most tiles asked for their children per layout. */
export const MAX_NESTED = 40;

/** Big enough, has folders, and not ⚫ / unreadable / a synthetic tile. */
export const nestable = (tile: Tile, r: Rect) =>
  !!tile.node &&
  tile.node.hasChildren &&
  tile.node.safety !== 'system' &&
  !tile.node.unreadable &&
  r.w >= 160 &&
  r.h >= 120;

/**
 * Children laid out inside `r` below the header, positioned relative to the tile. Empty when that
 * would be a single tile: it would only repeat the parent.
 */
export function innerTiles(tile: Tile, children: NodeView[], r: Rect): { r: Rect; tile: Tile }[] {
  const area = { x: 3, y: HEADER, w: r.w - 6, h: r.h - HEADER - 3 };
  const node = { ...tile.node!, size: tile.size };
  const tiles = buildTiles({ node, children, crumbs: [], unexplained: null }, minShare(area.w * area.h));
  if (tiles.length < 2) return [];
  const byId = new Map(tiles.map((x) => [x.id, x]));
  return squarify(tiles, area).map((ir) => ({
    r: ir,
    tile: byId.get(ir.id)!,
  }));
}
