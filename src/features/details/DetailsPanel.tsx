import { FolderOpen, KeyRound, Lock, ShieldAlert, Trash2, Wrench } from 'lucide-react';
import { t } from '@/i18n';
import { formatBytes } from '@/lib/format';
import { invoke } from '@/lib/ipc';
import type { NodeView } from '@/lib/types';
import { useScan } from '@/stores/scan';
import { selectable } from '@/features/treemap/Treemap';

const btn =
  'inline-flex h-7 items-center gap-1.5 rounded border border-border px-2 text-sm hover:border-accent';

/** Right-hand panel for the focused tile. `onDelete` opens the delete dialog for it. */
export function DetailsPanel({ onDelete }: { onDelete?: (n: NodeView) => void }) {
  const n = useScan((s) => s.focused);
  const open = useScan((s) => s.open);
  const scanning = useScan((s) => s.scanning);
  const rule = n?.ruleId && !n.remainder ? n.ruleId : null;

  return (
    <aside
      aria-label={t('details.label')}
      className="flex w-72 shrink-0 flex-col gap-3 overflow-y-auto border-l border-border bg-panel p-4 text-sm"
    >
      {!n ? (
        <p className="text-muted">{t('details.hint')}</p>
      ) : (
        <>
          <h2 className="flex items-center gap-1.5 text-base font-semibold break-words">
            {n.safety === 'system' && <Lock size={14} aria-hidden />}
            {n.name}
          </h2>
          {rule && <p className="-mt-2 text-muted">{t(`rule.${rule}.name`)}</p>}
          <p className="flex items-center gap-2">
            <span className="font-medium tabular-nums">{formatBytes(n.size)}</span>
            <span aria-hidden>·</span>
            <span className="tile inline-block size-3 rounded-sm" data-safety={n.safety} aria-hidden />
            {t(`safety.${n.safety}`)}
          </p>
          {rule && (
            <dl className="flex flex-col gap-2">
              <div>
                <dt className="text-muted">{t('details.why')}</dt>
                <dd>{t(`rule.${rule}.why`)}</dd>
              </div>
              <div>
                <dt className="text-muted">{t('details.effect')}</dt>
                <dd>{t(`rule.${rule}.effect`)}</dd>
              </div>
            </dl>
          )}
          {n.remainder && <p className="text-muted">{t('details.remainder')}</p>}
          <p className="font-mono text-xs break-all text-muted">{n.path}</p>
          {n.needsAdmin && (
            <p className="flex items-center gap-1.5">
              <KeyRound size={14} aria-hidden />
              {t('details.admin')}
            </p>
          )}
          {n.tool && (
            <p className="flex items-center gap-1.5">
              <Wrench size={14} aria-hidden />
              {t(n.toolMissing ? 'details.toolMissing' : 'details.tool', { tool: n.tool })}
            </p>
          )}
          {n.toolChild && <p className="text-muted">{t('delete.err.tool_root_only')}</p>}
          {n.unreadable && (
            <div className="flex flex-col items-start gap-1.5 rounded border border-dashed border-border p-2">
              <span className="flex items-center gap-1.5">
                <ShieldAlert size={14} aria-hidden />
                {t('treemap.unreadable')}
              </span>
              <button type="button" className={btn} onClick={() => void invoke('open_fda_settings')}>
                {t('treemap.openSettings')}
              </button>
            </div>
          )}
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className={btn}
              onClick={() => void invoke('reveal_path', { path: n.path })}
            >
              {t('details.reveal')}
            </button>
            {n.hasChildren && (
              <button type="button" className={btn} onClick={() => void open(n.id)}>
                <FolderOpen size={14} aria-hidden />
                {t('details.open')}
              </button>
            )}
            {selectable(n) && (
              <button
                type="button"
                className={`${btn} text-danger aria-disabled:cursor-not-allowed aria-disabled:opacity-50`}
                aria-disabled={scanning}
                title={scanning ? t('selection.waitScan') : undefined}
                onClick={() => !scanning && onDelete?.(n)}
              >
                <Trash2 size={14} aria-hidden />
                {t('details.delete')}
              </button>
            )}
          </div>
        </>
      )}
    </aside>
  );
}
