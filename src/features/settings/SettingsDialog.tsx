import { Dialog as D, RadioGroup as R, Switch } from 'radix-ui';
import { Settings, X } from 'lucide-react';
import { languageOptions, t, type LangSetting } from '@/i18n';
import { useLangSetting } from './lang';
import { useNested } from './nested';

export function SettingsDialog({ triggerClass }: { triggerClass: string }) {
  const { setting, choose } = useLangSetting();
  const nested = useNested();
  return (
    <D.Root>
      <D.Trigger aria-label={t('settings.open')} className={triggerClass}>
        <Settings size={14} aria-hidden />
      </D.Trigger>
      <D.Portal>
        <D.Overlay className="fixed inset-0 z-40 bg-black/40" />
        <D.Content
          aria-describedby={undefined}
          className="fixed top-1/2 left-1/2 z-50 w-80 -translate-x-1/2 -translate-y-1/2 rounded-md border border-border bg-panel p-4 shadow-lg"
        >
          <div className="mb-3 flex items-center justify-between">
            <D.Title className="font-semibold">{t('settings.title')}</D.Title>
            <D.Close aria-label={t('settings.close')} className="text-muted hover:text-current">
              <X size={14} aria-hidden />
            </D.Close>
          </div>
          <div id="settings-lang" className="mb-1 text-sm text-muted">
            {t('settings.language')}
          </div>
          <R.Root
            aria-labelledby="settings-lang"
            value={setting}
            onValueChange={(v) => choose(v as LangSetting)}
            className="flex flex-col gap-1.5"
          >
            {languageOptions(t('settings.system')).map((o) => (
              <label key={o.value} className="flex items-center gap-2 text-sm">
                <R.Item
                  value={o.value}
                  className="flex size-4 items-center justify-center rounded-full border border-border data-[state=checked]:border-accent"
                >
                  <R.Indicator className="size-2 rounded-full bg-accent" />
                </R.Item>
                {o.label}
              </label>
            ))}
          </R.Root>
          <label className="mt-4 flex items-center justify-between gap-2 text-sm">
            {t('settings.nested')}
            <Switch.Root
              checked={nested.on}
              onCheckedChange={nested.set}
              className="relative h-5 w-9 shrink-0 rounded-full border border-border bg-bg outline-none focus-visible:ring-2 focus-visible:ring-accent data-[state=checked]:border-accent data-[state=checked]:bg-accent"
            >
              <Switch.Thumb className="block size-4 translate-x-0.5 rounded-full bg-fg transition-transform data-[state=checked]:translate-x-4 data-[state=checked]:bg-white" />
            </Switch.Root>
          </label>
        </D.Content>
      </D.Portal>
    </D.Root>
  );
}
