import { useState } from 'react';
import { Dialog as D } from 'radix-ui';
import { History } from 'lucide-react';
import { t } from '@/i18n';
import { formatBytes } from '@/lib/format';
import { errorMessage, invoke } from '@/lib/ipc';
import type { HistoryEntry } from '@/lib/types';
import { btn, dialogClass } from '@/features/delete/DeleteDialog';
import { reasonText } from '@/features/delete/ResultDialog';

/** "History" button + dialog listing `history_list` (newest first). */
export function HistoryView({ triggerClass }: { triggerClass: string }) {
  const [rows, setRows] = useState<HistoryEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function load(open: boolean) {
    if (!open) return;
    setRows(null);
    setError(null);
    try {
      setRows(await invoke<HistoryEntry[]>('history_list'));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <D.Root onOpenChange={(o) => void load(o)}>
      <D.Trigger className={triggerClass}>
        <History size={14} aria-hidden />
        {t('history.open')}
      </D.Trigger>
      <D.Portal>
        <D.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <D.Content className={dialogClass} aria-describedby={undefined}>
          <D.Title className="text-base font-semibold">{t('history.title')}</D.Title>
          {error ? (
            <p role="alert" className="text-danger">
              {t('common.error')}: {error}
            </p>
          ) : !rows ? (
            <p className="text-muted">{t('common.loading')}</p>
          ) : rows.length === 0 ? (
            <p className="text-muted">{t('history.empty')}</p>
          ) : (
            <div className="min-h-0 overflow-y-auto">
              <table className="w-full table-fixed text-left text-xs">
                <thead className="text-muted">
                  <tr>
                    <th className="w-32 py-1 font-normal">{t('history.time')}</th>
                    <th className="py-1 font-normal">{t('history.path')}</th>
                    <th className="w-20 py-1 font-normal">{t('history.size')}</th>
                    <th className="w-24 py-1 font-normal">{t('history.mode')}</th>
                    <th className="w-40 py-1 font-normal">{t('history.status')}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {rows.map((h) => (
                    <tr key={`${h.time}:${h.path}:${h.mode}`} className="align-top">
                      <td className="py-1 tabular-nums">{new Date(h.time * 1000).toLocaleString()}</td>
                      <td className="truncate py-1 font-mono" title={h.path}>
                        {h.path}
                      </td>
                      <td className="py-1 tabular-nums">{h.tool ? '—' : formatBytes(h.size)}</td>
                      <td className="py-1">
                        {h.tool ? t('delete.byTool', { tool: h.tool }) : t(`delete.${h.mode}`)}
                      </td>
                      <td className={`py-1 ${h.ok ? '' : 'text-danger'}`}>
                        {h.ok ? t('history.ok') : reasonText(h.error ?? '')}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <div className="flex justify-end">
            <D.Close className={btn}>{t('common.close')}</D.Close>
          </div>
        </D.Content>
      </D.Portal>
    </D.Root>
  );
}
