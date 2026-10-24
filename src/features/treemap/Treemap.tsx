import { memo, useEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';
import { Check, KeyRound, Lock } from 'lucide-react';
import { t } from '@/i18n';
import { formatBytes } from '@/lib/format';
import { invoke } from '@/lib/ipc';
import type { LevelView, NodeView } from '@/lib/types';
import { useNested } from '@/features/settings/nested';
import { useScan } from '@/stores/scan';
import { squarify, type Rect } from './squarify';
import { buildTiles, innerTiles, MAX_NESTED, MIN_SIDE, minShare, nestable, nodeLabel, UNEXPLAINED, type Tile } from './tiles';

type Placed = { r: Rect; tile: Tile };
/** Slivers are not drawn: they would fill the gap between neighbours. */
const visible = ({ r }: Placed) => r.w >= MIN_SIDE && r.h >= MIN_SIDE;
const NONE = new Map<number, NodeView[]>();

/** Rule name of a rule-matched node (not of an overview remainder), else null. */
export const ruleName = (n: NodeView) => (n.ruleId && !n.remainder ? t(`rule.${n.ruleId}.name`) : null);
const ruleKeyName = (id: string) => t(`rule.${id}.name`);

/** Remainder tiles are "the rest of a folder", not a deletable unit; ⚫ is never deletable. */
export const selectable = (n: NodeView) =>
  n.safety !== 'system' && !n.remainder && !n.toolChild && !n.toolMissing;

export function Treemap() {
  const level = useScan((s) => s.level);
  const focused = useScan((s) => s.focused);
  const selected = useScan((s) => s.selected);
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });

  useEffect(() => {
    const el = ref.current!;
    const ro = new ResizeObserver(([e]) => {
      if (e) setSize({ w: Math.floor(e.contentRect.width), h: Math.floor(e.contentRect.height) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Layout only when the level or the container size changes.
  const rects = useMemo(() => {
    const tiles = level ? buildTiles(level, minShare(size.w * size.h)) : [];
    const byId = new Map(tiles.map((x) => [x.id, x]));
    return squarify(tiles, { x: 0, y: 0, ...size }).map((r) => ({ r, tile: byId.get(r.id)! }));
  }, [level, size]);

  // Children of big tiles, fetched in one batch per level/layout and cached for this level.
  const nestedOn = useNested((s) => s.on);
  const [kids, setKids] = useState<{ level: LevelView | null; map: Map<number, NodeView[]> }>({
    level: null,
    map: NONE,
  });
  const cached = kids.level === level ? kids.map : NONE;
  const seq = useRef(0);
  const missing = nestedOn
    ? rects
        .filter((x) => nestable(x.tile, x.r))
        .slice(0, MAX_NESTED)
        .map((x) => x.tile.id)
        .filter((id) => !cached.has(id))
        .join(',')
    : '';
  useEffect(() => {
    if (!level || !missing) return;
    const my = ++seq.current;
    invoke<[number, NodeView[]][]>('children_get', {
      ids: missing.split(',').map(Number),
      overview: level.node.id === 0,
    })
      .then((res) => {
        if (my !== seq.current) return; // a newer request (or level) superseded this one
        setKids((k) => ({ level, map: new Map([...(k.level === level ? k.map : []), ...res]) }));
      })
      .catch(() => {}); // e.g. a node deleted meanwhile: the level reloads and asks again
  }, [level, missing]);
  const nested = useMemo(() => {
    const out = new Map<number, Placed[]>();
    if (!nestedOn) return out;
    for (const { r, tile } of rects) {
      const c = cached.get(tile.id);
      const inner = c && nestable(tile, r) ? innerTiles(tile, c, r) : [];
      if (inner.length) out.set(tile.id, inner);
    }
    return out;
  }, [rects, cached, nestedOn]);
  const friendly = level?.node.id === 0;

  return (
    <div
      ref={ref}
      role="group"
      aria-label={t('treemap.label')}
      className="relative min-w-0 flex-1 overflow-hidden"
    >
      {!level && <p className="p-6 text-muted">{t('treemap.empty')}</p>}
      {rects.filter(visible).map(({ r, tile }) => (
        <TileView
          key={tile.id}
          r={r}
          tile={tile}
          focused={focused?.id === tile.id}
          selected={selected.has(tile.id)}
          inner={nested.get(tile.id)}
          friendly={friendly}
        />
      ))}
    </div>
  );
}

/**
 * Tiles drawn inside a big tile; own subscriptions keep the outer tiles' memo intact. Real names
 * only: siblings often share one rule (e.g. every ~/Documents/* child).
 */
function InnerTiles({ items }: { items: Placed[] }) {
  const focused = useScan((s) => s.focused);
  const selected = useScan((s) => s.selected);
  return items.filter(visible).map(({ r, tile }) => (
    <TileView
      key={tile.id}
      r={r}
      tile={tile}
      focused={focused?.id === tile.id}
      selected={selected.has(tile.id)}
      friendly={false}
    />
  ));
}

const TileView = memo(function TileView({
  r,
  tile,
  focused,
  selected,
  inner,
  friendly,
}: {
  r: Rect;
  tile: Tile;
  focused: boolean;
  selected: boolean;
  inner?: Placed[] | undefined;
  friendly: boolean;
}) {
  const { focus, toggleSelect, open } = useScan.getState();
  const n = tile.node;
  const label = n
    ? nodeLabel(n, friendly, ruleKeyName)
    : tile.id === UNEXPLAINED
      ? t('treemap.unexplained')
      : t('treemap.other', { n: tile.count.toLocaleString() });
  const safety = n?.safety ?? 'unknown';
  const sizeText = formatBytes(tile.size);
  const rule = n && ruleName(n);
  const desc = `${label}, ${sizeText}, ${t(`safety.${safety}`)}${rule && rule !== label ? `, ${rule}` : ''}${n && n.name !== label ? `, ${n.name}` : ''}${n?.unreadable ? `, ${t('treemap.unreadable')}` : ''}`;
  const showText = r.w >= 56 && r.h >= 22;
  const canSelect = !!n && selectable(n);
  const showCheck = canSelect && r.w >= 44 && r.h >= 44;
  const pos = { left: r.x, top: r.y, width: r.w, height: r.h };

  const onClick = (e: MouseEvent) => {
    if (!n) return;
    if (e.metaKey && canSelect) toggleSelect(n);
    else focus(n);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (!n) return;
    if (e.key === ' ') {
      e.preventDefault();
      if (canSelect) toggleSelect(n);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      focus(n);
    }
  };

  return (
    <div className="absolute p-px" style={pos}>
      <button
        type="button"
        data-safety={safety}
        data-kind={n ? 'node' : tile.id === UNEXPLAINED ? 'unexplained' : 'other'}
        title={desc}
        aria-label={desc}
        aria-pressed={n ? selected : undefined}
        onClick={onClick}
        onDoubleClick={() => n?.hasChildren && void open(n.id)}
        onKeyDown={onKeyDown}
        className={`tile flex h-full w-full flex-col items-start overflow-hidden rounded-[3px] p-1.5 text-left text-xs leading-tight outline-none ${showCheck ? 'pr-6' : ''} focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-inset ${
          n?.unreadable ? 'border-2 border-dashed border-current' : ''
        } ${focused ? 'ring-2 ring-fg ring-inset' : ''} ${selected ? 'outline-2 -outline-offset-2 outline-accent' : ''}`}
      >
        {showText && (
          <>
            <span className="flex max-w-full items-center gap-1 font-medium">
              {safety === 'system' && <Lock size={12} className="shrink-0" aria-hidden />}
              {n?.needsAdmin && <KeyRound size={12} className="shrink-0" aria-hidden />}
              <span className="truncate">{label}</span>
            </span>
            {r.h >= 38 && <span>{sizeText}</span>}
            {n?.unreadable && r.h >= 70 && <span className="mt-1">{t('treemap.unreadable')}</span>}
          </>
        )}
      </button>
      {inner && <InnerTiles items={inner} />}
      {showCheck && (
        <button
          type="button"
          role="checkbox"
          aria-checked={selected}
          aria-label={t('treemap.select', { name: label })}
          onClick={() => toggleSelect(n)}
          className={`absolute top-1.5 right-1.5 flex size-4 items-center justify-center rounded-sm border border-current text-[var(--tile-fg)] ${
            selected ? 'bg-accent text-white' : 'bg-transparent'
          }`}
        >
          {selected && <Check size={12} aria-hidden />}
        </button>
      )}
      {n?.unreadable && r.w >= 90 && r.h >= 100 && (
        <button
          type="button"
          onClick={() => void invoke('open_fda_settings')}
          className="absolute bottom-2 left-2 rounded border border-current px-1.5 py-0.5 text-xs text-[var(--tile-fg)]"
        >
          {t('treemap.openSettings')}
        </button>
      )}
    </div>
  );
});
