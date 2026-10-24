import { Select as S } from 'radix-ui';
import { Check, ChevronDown, FolderOpen, RefreshCw, X } from 'lucide-react';
import { t } from '@/i18n';
import { formatBytes } from '@/lib/format';
import { pickFolder } from '@/lib/ipc';
import { useScan } from '@/stores/scan';
import { HistoryView } from '@/features/history/HistoryView';
import { SettingsDialog } from '@/features/settings/SettingsDialog';
import { Breadcrumbs } from '@/features/treemap/Breadcrumbs';

const btn =
  'inline-flex h-7 shrink-0 items-center gap-1.5 rounded border border-border bg-panel px-2 text-sm hover:border-accent disabled:opacity-50';

export function TopBar() {
  const { volumes, root, scanning, progress, error, startScan, cancelScan } = useScan();
  // Volume holding the scanned root: longest matching mount path.
  const vol = root
    ? volumes
        .filter((v) => root === v.path || root.startsWith(v.path.endsWith('/') ? v.path : `${v.path}/`))
        .sort((a, b) => b.path.length - a.path.length)[0]
    : undefined;
  const rootLabel = vol && vol.path === root ? vol.name : (root?.split('/').filter(Boolean).pop() ?? '/');

  return (
    <header className="flex flex-col gap-1.5 border-b border-border px-3 py-2">
      <div className="flex items-center gap-2">
        <span className="font-semibold">{t('app.name')}</span>
        <S.Root
          value={vol && vol.path === root ? vol.path : ''}
          onValueChange={(p) => void startScan(p)}
          disabled={scanning}
        >
          <S.Trigger aria-label={t('topbar.volume')} className={btn}>
            <S.Value placeholder={t('topbar.volume')} />
            <S.Icon>
              <ChevronDown size={14} className="text-muted" />
            </S.Icon>
          </S.Trigger>
          <S.Portal>
            <S.Content
              position="popper"
              sideOffset={4}
              className="z-50 rounded-md border border-border bg-panel p-1 shadow-lg"
            >
              <S.Viewport>
                {volumes.map((v) => (
                  <S.Item
                    key={v.path}
                    value={v.path}
                    className="relative flex h-7 items-center rounded-sm pr-2 pl-6 text-sm outline-none data-[highlighted]:bg-accent data-[highlighted]:text-white"
                  >
                    <S.ItemIndicator className="absolute left-1.5">
                      <Check size={13} />
                    </S.ItemIndicator>
                    <S.ItemText>{v.name}</S.ItemText>
                  </S.Item>
                ))}
              </S.Viewport>
            </S.Content>
          </S.Portal>
        </S.Root>
        <button
          type="button"
          className={btn}
          disabled={scanning}
          onClick={async () => {
            const dir = await pickFolder();
            if (dir) void startScan(dir);
          }}
        >
          <FolderOpen size={14} aria-hidden />
          {t('topbar.pickFolder')}
        </button>
        <div className="flex-1" />
        {scanning ? (
          <>
            <span role="status" className="text-sm text-muted tabular-nums">
              {t('topbar.scanning', {
                files: (progress?.files ?? 0).toLocaleString(),
                bytes: formatBytes(progress?.bytes ?? 0),
              })}
            </span>
            <button type="button" className={btn} onClick={() => void cancelScan()}>
              <X size={14} aria-hidden />
              {t('common.cancel')}
            </button>
          </>
        ) : (
          <button type="button" className={btn} disabled={!root} onClick={() => root && void startScan(root)}>
            <RefreshCw size={14} aria-hidden />
            {t('topbar.rescan')}
          </button>
        )}
        <HistoryView triggerClass={btn} />
        <SettingsDialog triggerClass={btn} />
      </div>
      <div className="flex min-w-0 items-center gap-2 text-sm">
        <Breadcrumbs rootLabel={rootLabel} />
        {vol && (
          <span className="shrink-0 text-muted tabular-nums">
            ·{' '}
            {t('topbar.usage', {
              used: formatBytes(vol.total - vol.free),
              total: formatBytes(vol.total),
              free: formatBytes(vol.free),
            })}
          </span>
        )}
        {error && (
          <span role="alert" className="truncate text-danger">
            {t('common.error')}: {error}
          </span>
        )}
      </div>
    </header>
  );
}
