// Supported UI languages. To add one: add an entry here, then add its
// dictionary (`<code>: { ... }`) to src/i18n/core.ts and every
// src/features/*/i18n.ts — missing keys fall back to English.
export const LANGUAGES = [
  { code: 'en', name: 'English' },
  { code: 'vi', name: 'Tiếng Việt' },
] as const;

export type Lang = (typeof LANGUAGES)[number]['code'];
/** Setting value: a language, or follow the OS. */
export type LangSetting = Lang | 'system';

export const FALLBACK_LANG: Lang = 'en';

const isLang = (c: string): c is Lang => LANGUAGES.some((l) => l.code === c);

/** First OS-preferred language we support (by primary subtag), else English. */
export function systemLang(prefs: readonly string[] = navigator.languages ?? [navigator.language]): Lang {
  for (const p of prefs) {
    const primary = p.toLowerCase().split(/[-_]/)[0] ?? '';
    if (isLang(primary)) return primary;
  }
  return FALLBACK_LANG;
}

export function resolveLang(setting: LangSetting | undefined): Lang {
  return setting && setting !== 'system' && isLang(setting) ? setting : systemLang();
}

/** Options for a language picker: "System (<detected>)" + every language in its own name. */
export function languageOptions(systemLabel: string): { value: LangSetting; label: string }[] {
  const detected = LANGUAGES.find((l) => l.code === systemLang())!.name;
  return [{ value: 'system', label: `${systemLabel} (${detected})` }, ...LANGUAGES.map((l) => ({ value: l.code, label: l.name }))];
}
