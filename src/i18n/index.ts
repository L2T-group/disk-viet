// Minimal i18n (no library). Dictionaries are merged from:
//   - src/i18n/core.ts             (shared strings)
//   - src/features/*/i18n.ts       (each feature owns its keys, prefixed "<feature>.")
// Each exports `dict: { vi: Record<string,string>, en: Record<string,string> }`.
// Changing language re-mounts the app (see App.tsx), so plain `t()` is enough.
import { dict as core } from './core';
import { FALLBACK_LANG, systemLang, type Lang } from './languages';

export type { Lang, LangSetting } from './languages';
export { LANGUAGES, languageOptions, resolveLang, systemLang } from './languages';
type Dict = Partial<Record<Lang, Record<string, string>>>;

const modules = import.meta.glob<{ dict: Dict }>('../features/*/i18n.ts', { eager: true });
const all: Record<string, Record<string, string>> = {};
for (const d of [core as Dict, ...Object.values(modules).map((m) => m.dict)]) {
  for (const [code, entries] of Object.entries(d)) Object.assign((all[code] ??= {}), entries);
}

let lang: Lang = systemLang();
export const setLang = (l: Lang) => {
  lang = l;
  document.documentElement.lang = l;
};
export const getLang = () => lang;

/** Translate `key`, replacing `{name}` placeholders from `vars`. Falls back to English, then the key. */
export function t(key: string, vars?: Record<string, string | number>): string {
  let s = all[lang]?.[key] ?? all[FALLBACK_LANG]?.[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.split(`{${k}}`).join(String(v));
  return s;
}
