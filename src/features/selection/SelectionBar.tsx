import { t } from '@/i18n';
import { formatBytes } from '@/lib/format';
import type { NodeView } from '@/lib/types';
import { useScan } from '@/stores/scan';
import { topLevelOnly } from './select';

const btn =
  'h-7 rounded border border-border bg-panel px-3 text-sm hover:border-accent aria-disabled:cursor-not-allowed aria-disabled:opacity-50';

export function SelectionBar({ onDelete }: { onDelete?: (nodes: NodeView[]) => void }) {
  const selected = useScan((s) => s.selected);
  const clearSelection = useScan((s) => s.clearSelection);
  // A rescan started before the delete would bring the deleted items back.
  const scanning = useScan((s) => s.scanning);
  const nodes = topLevelOnly([...selected.values()]);
  if (nodes.length === 0) return null;
  const size = nodes.reduce((a, n) => a + n.size, 0);
  return (
    <footer className="flex items-center gap-3 border-t border-border px-3 py-2">
      <span aria-live="polite" className="flex-1 text-sm">
        {t('selection.summary', { n: nodes.length, size: formatBytes(size) })}
      </span>
      <button type="button" className={btn} onClick={clearSelection}>
        {t('selection.clear')}
      </button>
      <button
        type="button"
        className={`${btn} border-accent`}
        aria-disabled={scanning}
        title={scanning ? t('selection.waitScan') : undefined}
        onClick={() => !scanning && onDelete?.(nodes)}
      >
        {t('selection.delete')}
      </button>
    </footer>
  );
}
