import { Dialog as D } from 'radix-ui';
import { t } from '@/i18n';
import { formatBytes } from '@/lib/format';
import { btn, dialogClass, type DeleteDone } from './DeleteDialog';

/** Plain-language reason for an `ItemResult.error` / `HistoryEntry.error` code; unknown codes stay raw. */
export function reasonText(code: string): string {
  if (!code) return t('delete.failed');
  if (code.startsWith('quit:')) return t('delete.err.quit', { app: code.slice(5) });
  if (code.startsWith('covered_by:')) return t('delete.err.covered_by', { path: code.slice(11) });
  if (code.startsWith('tool_missing:')) return t('delete.err.tool_missing', { tool: code.slice(13) });
  if (code.startsWith('tool_failed:')) return t('delete.err.tool_failed', { detail: code.slice(12) });
  if (code.startsWith('trash_failed')) return `${t('delete.err.trash_failed')} (${code})`;
  const key = `delete.err.${code}`;
  const s = t(key);
  return s === key ? code : s;
}

/** After `delete_run`: measured free space before/after (statfs), never a sum of file sizes. */
export function ResultDialog({ done, onClose }: { done: DeleteDone; onClose: () => void }) {
  const { report: r, trashBytes } = done;
  const freed = r.freeAfter - r.freeBefore;
  const failed = r.results.filter((x) => !x.ok);
  const okCount = r.results.length - failed.length;
  const vars = {
    before: formatBytes(r.freeBefore),
    after: formatBytes(r.freeAfter),
    freed: formatBytes(freed),
  };
  return (
    <D.Root open onOpenChange={(o) => !o && onClose()}>
      <D.Portal>
        <D.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <D.Content className={dialogClass} aria-describedby={undefined}>
          <D.Title className="text-base font-semibold">{t('delete.result.title')}</D.Title>
          <p className="tabular-nums">{t(freed > 0 ? 'delete.result.freed' : 'delete.result.none', vars)}</p>
          {r.expected > 0 && freed < 0.9 * r.expected && (
            <p className="text-muted">{t('delete.result.apfs', { expected: formatBytes(r.expected) })}</p>
          )}
          {trashBytes > 0 && <p>{t('delete.result.emptyTrash', { size: formatBytes(trashBytes) })}</p>}
          {okCount > 0 && <p className="text-muted">{t('delete.result.ok', { n: okCount })}</p>}
          {failed.length > 0 && (
            <div role="alert" className="flex min-h-0 flex-col gap-1 overflow-y-auto">
              <p className="font-medium text-danger">{t('delete.result.errors', { n: failed.length })}</p>
              <ul className="flex flex-col gap-1.5">
                {failed.map((x) => (
                  <li key={x.path}>
                    <span className="block truncate font-mono text-xs text-muted" title={x.path}>
                      {x.path}
                    </span>
                    {reasonText(x.error ?? '')}
                  </li>
                ))}
              </ul>
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
