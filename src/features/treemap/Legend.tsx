import { Lock } from 'lucide-react';
import type { ReactNode } from 'react';
import { t } from '@/i18n';
import type { Safety } from '@/lib/types';

const SAFETIES: Safety[] = ['safe', 'caution', 'important', 'unknown', 'system'];

function Item({ swatch, label, hint }: { swatch: ReactNode; label: string; hint: string }) {
  return (
    <li className="flex items-center gap-1.5" title={hint}>
      {swatch}
      <span>{label}</span>
    </li>
  );
}

const box = 'tile flex size-3.5 shrink-0 items-center justify-center rounded-[3px]';

/** Color key for the treemap; swatches reuse the tile classes so colors never drift. */
export function Legend() {
  return (
    <ul aria-label={t('legend.label')} className="flex flex-wrap items-center gap-x-4 gap-y-1 px-4 py-1.5 text-xs text-muted">
      {SAFETIES.map((s) => (
        <Item
          key={s}
          swatch={
            <span className={box} data-safety={s} aria-hidden>
              {s === 'system' && <Lock size={9} />}
            </span>
          }
          label={t(`safety.${s}`)}
          hint={t(`legend.${s}`)}
        />
      ))}
      <Item
        swatch={<span className={box} data-kind="unexplained" aria-hidden />}
        label={t('treemap.unexplained')}
        hint={t('legend.unexplained')}
      />
    </ul>
  );
}
