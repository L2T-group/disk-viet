import { ChevronRight } from 'lucide-react';
import { Fragment } from 'react';
import { t } from '@/i18n';
import { useScan } from '@/stores/scan';

/** `<root> › … › <current>`; id 0 is the overview, labelled `rootLabel`. */
export function Breadcrumbs({ rootLabel }: { rootLabel: string }) {
  const level = useScan((s) => s.level);
  const open = useScan((s) => s.open);
  if (!level) return null;
  const items = [...level.crumbs, level.node];
  return (
    <nav aria-label={t('treemap.breadcrumb')} className="flex min-w-0 items-center gap-0.5">
      {items.map((c, i) => (
        <Fragment key={c.id}>
          {i > 0 && <ChevronRight size={14} className="shrink-0 text-muted" aria-hidden />}
          <button
            type="button"
            onClick={() => void open(c.id)}
            aria-current={i === items.length - 1 ? 'page' : undefined}
            className="truncate rounded px-1 font-medium hover:bg-panel aria-[current=page]:font-semibold"
          >
            {c.id === 0 ? rootLabel : c.name}
          </button>
        </Fragment>
      ))}
    </nav>
  );
}
