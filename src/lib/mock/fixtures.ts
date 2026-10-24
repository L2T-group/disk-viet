import type { NodeView, Safety, Volume } from '../types';

const GB = 1e9;
const H = '/Users/demo';

export const volumes: Volume[] = [
  { path: '/', name: 'Macintosh HD', total: 228 * GB, free: 13 * GB, isBoot: true },
  { path: '/Volumes/Data', name: 'Data', total: 1000 * GB, free: 640 * GB, isBoot: false },
];

type Spec = [
  id: number,
  parent: number | null,
  name: string,
  gb: number,
  safety?: Safety,
  ruleId?: string,
  extra?: Partial<NodeView>,
];

// Sizes of parents are the sum of children (+ a little small-file mass).
const specs: Spec[] = [
  [0, null, 'demo', 144],
  [1, 0, 'Library', 62],
  [2, 1, 'Caches', 24],
  [3, 2, 'ru.keepcoder.Telegram', 12, 'safe', 'telegram_cache'],
  [4, 2, 'JetBrains', 4, 'safe', 'jetbrains_cache'],
  [5, 2, 'Homebrew', 3, 'safe', 'homebrew_cache', { tool: 'brew', toolCommand: 'brew cleanup -s' }],
  [6, 2, 'Google', 2, 'safe', 'chrome_cache'],
  [7, 1, 'Developer', 30],
  [8, 7, 'CoreSimulator', 22],
  [9, 8, 'Caches', 11, 'safe', 'simulator_caches'],
  [10, 8, 'Devices', 11, 'caution', 'simulator_devices'],
  [11, 7, 'Xcode', 8],
  [12, 11, 'DerivedData', 4, 'safe', 'xcode_derived_data'],
  [13, 11, 'Archives', 4, 'caution', 'xcode_archives'],
  [14, 1, 'Application Support', 8],
  [15, 14, 'Cursor', 8],
  [16, 0, '.gradle', 6],
  [17, 0, '.android', 6],
  [18, 17, 'avd', 6],
  [19, 0, 'Documents', 35],
  [20, 19, 'Projects', 20, 'important', 'user_documents'],
  [21, 19, 'Photos', 15, 'important', 'user_documents'],
  [22, 0, 'Downloads', 5, 'caution', 'downloads'],
  [23, 22, 'installer.dmg', 3, 'caution', 'downloads'],
  [24, 22, 'archive.zip', 2, 'caution', 'downloads'],
  [25, 0, 'Desktop', 1],
  [26, 0, '.Trash', 2, 'caution', 'trash', { unreadable: true }],
  [27, 0, 'Applications', 27, 'system', 'applications'],
  [28, 27, 'Xcode.app', 15, 'system', 'applications'],
  [29, 27, 'Cursor.app', 12, 'system', 'applications'],
  [30, 15, 'CachedData', 3, 'safe', 'cursor_cache'],
  [31, 15, 'User', 5],
  [32, 31, 'globalStorage', 5, 'caution', 'cursor_global_storage'],
  [33, 16, 'caches', 6, 'caution', 'gradle_cache'],
  [34, 18, 'Pixel_8.avd', 6, 'caution', 'android_avd'],
  [35, 21, 'Vacation', 9, 'important'],
  [36, 21, 'Family', 6, 'important'],
];

const byId = new Map<number, NodeView>();
const kids = new Map<number, number[]>();

function pathOf(id: number): string {
  const s = specs[id]!;
  if (id === 0) return H;
  // Applications lives at /Applications, not under the home folder.
  if (id === 27) return '/Applications';
  return `${pathOf(s[1]!)}/${s[2]}`;
}

for (const [id, parent, name, gb, safety = 'unknown', ruleId, extra] of specs) {
  if (parent !== null) kids.set(parent, [...(kids.get(parent) ?? []), id]);
  byId.set(id, {
    id,
    name,
    path: pathOf(id),
    size: gb * GB,
    isDir: !/\.(dmg|zip)$/.test(name),
    ruleId: ruleId ?? null,
    safety,
    needsAdmin: safety === 'system',
    tool: null,
    toolCommand: null,
    toolMissing: false,
    toolChild: false,
    unreadable: false,
    smallBytes: 0.1 * GB,
    smallCount: 1200,
    hasChildren: false,
    remainder: false,
    ...extra,
  });
}
for (const [id, c] of kids) byId.get(id)!.hasChildren = c.length > 0;

export const unexplained = 18 * GB;

/** Mock delete: detach the node at `path` and shrink its ancestors. Returns its size, or null. */
export function removeNode(path: string): number | null {
  const node = [...byId.values()].find((n) => n.path === path);
  const parent = node ? specs[node.id]![1] : null;
  if (!node || parent === null) return null;
  const sib = kids.get(parent)?.filter((k) => k !== node.id) ?? [];
  if (sib.length === kids.get(parent)?.length) return null; // already removed
  kids.set(parent, sib);
  byId.get(parent)!.hasChildren = sib.length > 0;
  for (let p: number | null = parent; p !== null; p = specs[p]![1]) byId.get(p)!.size -= node.size;
  return node.size;
}

// Outermost direct Safe/Caution rule hits under `id` (into `out`); returns the bytes they cover.
// A rule hit is "direct" when its parent does not carry the same rule.
function ruleTiles(id: number, out: NodeView[]): number {
  const n = byId.get(id)!;
  const p = specs[id]![1];
  const direct = n.ruleId !== null && (p === null || byId.get(p)!.ruleId !== n.ruleId);
  if (direct && (n.safety === 'safe' || n.safety === 'caution')) {
    out.push(n);
    return n.size;
  }
  return (kids.get(id) ?? []).reduce((a, k) => a + ruleTiles(k, out), 0);
}
const isRuleTile = (id: number) => {
  const out: NodeView[] = [];
  ruleTiles(id, out);
  return out[0]?.id === id;
};

// Overview (id 0): rule tiles + one remainder tile per root child.
function overview() {
  const tiles: NodeView[] = [];
  const rest: NodeView[] = [];
  for (const c of kids.get(0) ?? []) {
    const n = byId.get(c)!;
    const left = n.size - ruleTiles(c, tiles);
    if (left > 0) rest.push({ ...n, size: left, remainder: true });
  }
  const children = [...tiles, ...rest].sort((a, b) => b.size - a.size);
  return { node: byId.get(0)!, children, crumbs: [], unexplained };
}

/** Mirrors Rust `children_view`: on the overview, remainder tiles' children lose their rule tiles. */
export function children(ids: number[], overview: boolean): [number, NodeView[]][] {
  return ids.map((id) => {
    if (id > PERF_ID && id <= PERF_ID + PERF_BIG) return [id, perfInner(id)];
    if (!byId.has(id)) throw { kind: 'invalid', message: `unknown node ${id}` };
    const cut = overview && !isRuleTile(id);
    const list = (kids.get(id) ?? []).flatMap((k) => {
      const n = byId.get(k)!;
      const covered = cut ? ruleTiles(k, []) : 0;
      return n.size - covered > 0 ? [{ ...n, size: n.size - covered, remainder: covered > 0 }] : [];
    });
    return [id, list.sort((a, b) => b.size - a.size)];
  });
}

// Perf fixture (not reachable from the UI): 2,000 children, 150 of them just above the 0.5%
// merge threshold, plus PERF_BIG large folders with 200 children each (20 big ones
// draw inside the tile, the rest merge into "other").
const PERF_ID = 9000;
const PERF_BIG = 2;
function perfInner(id: number): NodeView[] {
  return Array.from({ length: 200 }, (_, i) => ({
    ...byId.get(23)!,
    id: id * 1000 + i,
    name: `inner-${i}.bin`,
    path: `${H}/many/dir-${id}/inner-${i}.bin`,
    size: i < 20 ? (8 * GB) / 10 : (4 * GB) / 180, // 20 drawable folders, the rest "other"
    safety: (['safe', 'caution', 'important', 'unknown'] as const)[i % 4]!,
  }));
}
function perfLevel() {
  const node = { ...byId.get(19)!, id: PERF_ID, name: 'many', path: `${H}/many`, hasChildren: true };
  const children = Array.from({ length: 2000 }, (_, i) => ({
    ...byId.get(23)!,
    id: PERF_ID + 1 + i,
    name: `file-${i}.bin`,
    path: `${H}/many/file-${i}.bin`,
    size: i < PERF_BIG ? 20 * GB : i < 150 + PERF_BIG ? GB : GB / 1000,
    safety: (['safe', 'caution', 'important', 'unknown'] as const)[i % 4]!,
    hasChildren: i < PERF_BIG,
    isDir: i < PERF_BIG,
  }));
  node.size = children.reduce((s, c) => s + c.size, 0) + node.smallBytes;
  return { node, children, crumbs: [byId.get(0)!], unexplained: null };
}

export function level(id: number) {
  if (id === 0) return overview();
  if (id === PERF_ID) return perfLevel();
  const node = byId.get(id);
  if (!node) throw { kind: 'invalid', message: `unknown node ${id}` };
  const crumbs: NodeView[] = [];
  for (let p = specs[id]![1]; p !== null; p = specs[p]![1]) crumbs.unshift(byId.get(p)!);
  const children = (kids.get(id) ?? []).map((k) => byId.get(k)!).sort((a, b) => b.size - a.size);
  return { node, children, crumbs, unexplained: id === 0 ? unexplained : null };
}
