import { useState } from 'react';
import { Dialog as D, RadioGroup as R } from 'radix-ui';
import { KeyRound, LoaderCircle, TriangleAlert, Wrench } from 'lucide-react';
import { t } from '@/i18n';
import { formatBytes } from '@/lib/format';
import { errorMessage, invoke } from '@/lib/ipc';
import type { DeleteItem, DeleteMode, DeleteReport, NodeView } from '@/lib/types';
import { reopenId, topLevelOnly, within } from '@/features/selection/select';
import { useScan } from '@/stores/scan';
import { ruleName } from '@/features/treemap/Treemap';
import { defaultMode, isRisky, lockedMode, totals } from './defaults';

export const dialogClass =
  'fixed top-1/2 left-1/2 z-50 flex max-h-[85vh] w-[min(640px,92vw)] -translate-x-1/2 -translate-y-1/2 flex-col gap-3 rounded-md border border-border bg-panel p-4 text-sm shadow-lg';
export const btn =
  'inline-flex h-8 items-center gap-1.5 rounded border border-border px-3 text-sm hover:border-accent disabled:opacity-50';

export interface DeleteDone {
  report: DeleteReport;
  /** Bytes of rows that went to the Trash successfully: freed only when the Trash is emptied. */
  trashBytes: number;
}

/** Confirm dialog: one row per item with its own Trash / Delete permanently choice (spec §4.3). */
export function DeleteDialog(props: {
  nodes: NodeView[];
  onClose: () => void;
  onDone: (d: DeleteDone) => void;
}) {
  // Defensive: the focused node plus a selection could nest; send only the outermost.
  const [nodes] = useState(() => topLevelOnly(props.nodes));
  const merged = props.nodes.length - nodes.length;
  const [modes, setModes] = useState(() => new Map(nodes.map((n) => [n.path, defaultMode(n)])));
  const [ack, setAck] = useState(false);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const rows = nodes.map((node) => ({ node, mode: modes.get(node.path)! }));
  const sum = totals(rows);
  const needsAck = sum.risky > 0;
  const acked = needsAck && ack;

  async function run() {
    setRunning(true);
    setError(null);
    const items: DeleteItem[] = rows.map((r) => ({
      path: r.node.path,
      mode: r.mode,
      ackIrreversible: acked,
    }));
    try {
      const report = await invoke<DeleteReport>('delete_run', { items });
      // ~/Downloads comes back as its children (the folder is emptied, not removed).
      const done = (r: (typeof rows)[number]) => {
        const res = report.results.filter((x) => within(x.path, r.node.path));
        return res.length > 0 && res.every((x) => x.ok);
      };
      const trashBytes = rows
        .filter((r) => r.mode === 'trash' && done(r))
        .reduce((a, r) => a + r.node.size, 0);
      props.onDone({ report, trashBytes });
    } catch (e) {
      setError(errorMessage(e));
      setRunning(false);
      // Some items may already be gone: drop the selection and reload what is on screen.
      const s = useScan.getState();
      s.clearSelection();
      if (s.level) void s.open(reopenId(s.level, items.map((x) => x.path)));
    }
  }

  return (
    <D.Root open onOpenChange={(o) => !o && !running && props.onClose()}>
      <D.Portal>
        <D.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <D.Content className={dialogClass} onInteractOutside={(e) => e.preventDefault()}>
          <D.Title className="text-base font-semibold">{t('delete.title', { n: nodes.length })}</D.Title>
          <D.Description className="text-muted">{t('delete.intro')}</D.Description>
          {merged > 0 && <p className="text-muted">{t('delete.merged', { n: merged })}</p>}

          <ul className="-mx-1 flex min-h-0 flex-col divide-y divide-border overflow-y-auto px-1">
            {rows.map(({ node, mode }) => (
              <Row
                key={node.path}
                node={node}
                mode={mode}
                disabled={running}
                onMode={(m) => {
                  setModes(new Map(modes).set(node.path, m));
                  setAck(false); // consent covers the rows as they were when ticked
                }}
              />
            ))}
          </ul>

          {needsAck && (
            <div className="flex flex-col gap-1.5 rounded border border-danger p-2">
              <p className="flex items-center gap-1.5 text-danger">
                <TriangleAlert size={14} aria-hidden />
                {t('delete.riskyNote')}
              </p>
              <label className="flex items-center gap-2 font-medium">
                <input
                  type="checkbox"
                  checked={ack}
                  disabled={running}
                  onChange={(e) => setAck(e.target.checked)}
                  className="size-4 accent-[var(--danger)]"
                />
                {t('delete.ack')}
              </label>
            </div>
          )}

          <p className="tabular-nums">
            {t('delete.summary', { permanent: formatBytes(sum.permanent), trash: formatBytes(sum.trash) })}
            {sum.tool > 0 && ` · ${t('delete.summaryTool', { size: formatBytes(sum.tool) })}`}
          </p>
          {running && (
            <p role="status" className="flex items-center gap-2 text-muted">
              <LoaderCircle size={14} className="animate-spin" aria-hidden />
              {t('delete.running')}
            </p>
          )}
          {error && (
            <p role="alert" className="text-danger">
              {t('delete.failed')}: {error}
            </p>
          )}

          <div className="flex justify-end gap-2">
            <D.Close className={btn} disabled={running}>
              {t('common.cancel')}
            </D.Close>
            <button
              type="button"
              disabled={running || (needsAck && !ack)}
              onClick={() => void run()}
              className={`${btn} ${needsAck ? 'border-danger bg-danger text-white' : 'border-accent bg-accent text-white'}`}
            >
              {needsAck ? t('delete.runRisky', { n: sum.risky }) : t('delete.run')}
            </button>
          </div>
        </D.Content>
      </D.Portal>
    </D.Root>
  );
}

function Row(p: { node: NodeView; mode: DeleteMode; disabled: boolean; onMode: (m: DeleteMode) => void }) {
  const { node: n } = p;
  const locked = lockedMode(n);
  const rule = ruleName(n);
  return (
    <li className="flex items-center gap-3 py-2">
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="flex items-center gap-2">
          <span className="truncate font-medium">{n.name}</span>
          <span className="shrink-0 tabular-nums">{formatBytes(n.size)}</span>
          <span className="tile shrink-0 rounded-sm px-1.5 text-xs" data-safety={n.safety}>
            {t(`safety.${n.safety}`)}
          </span>
        </span>
        {rule && <span className="truncate text-xs text-muted">{rule}</span>}
        <span className="truncate font-mono text-xs text-muted" title={n.path}>
          {n.path}
        </span>
        {locked && (
          <span className="flex items-center gap-1 text-xs text-muted">
            {n.needsAdmin && <KeyRound size={12} aria-hidden />}
            {t(n.needsAdmin ? 'delete.lockedAdmin' : 'delete.lockedTrash')}
          </span>
        )}
      </div>
      {n.tool ? (
        <span className="flex shrink-0 flex-col items-end gap-0.5 text-xs">
          <span className="flex items-center gap-1">
            <Wrench size={12} aria-hidden />
            {t('delete.byTool', { tool: n.tool })}
          </span>
          <code className="font-mono text-muted">
            {n.toolCommand}
            {n.ruleId === 'simulator_runtimes' && ` ${t('delete.selectedRuntime')}`}
          </code>
        </span>
      ) : (
        <R.Root
          aria-label={t('delete.modeFor', { name: n.name })}
          value={p.mode}
          disabled={p.disabled || !!locked}
          onValueChange={(v) => p.onMode(v as DeleteMode)}
          orientation="horizontal"
          className="flex shrink-0 overflow-hidden rounded border border-border"
        >
          {(['trash', 'permanent'] as const).map((m) => (
            <R.Item
              key={m}
              value={m}
              className={`h-7 px-2.5 text-xs disabled:opacity-60 data-[state=checked]:text-white ${
                isRisky(n, m) ? 'data-[state=checked]:bg-danger' : 'data-[state=checked]:bg-accent'
              }`}
            >
              {t(`delete.${m}`)}
            </R.Item>
          ))}
        </R.Root>
      )}
    </li>
  );
}
