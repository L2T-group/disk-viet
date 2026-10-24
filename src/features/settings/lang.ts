import { create } from 'zustand';
import { resolveLang, setLang, type LangSetting } from '@/i18n';

const KEY = 'diskviet.lang';

function load(): LangSetting {
  try {
    const v = localStorage.getItem(KEY);
    return v === 'vi' || v === 'en' ? v : 'system';
  } catch {
    return 'system';
  }
}

const initial = load();
setLang(resolveLang(initial)); // before first render

/** `epoch` keys the app root so every t() call re-renders after a change. */
export const useLangSetting = create<{ setting: LangSetting; epoch: number; choose: (s: LangSetting) => void }>((set) => ({
  setting: initial,
  epoch: 0,
  choose: (s) => {
    try {
      localStorage.setItem(KEY, s);
    } catch {
      /* works without storage */
    }
    setLang(resolveLang(s));
    set((st) => ({ setting: s, epoch: st.epoch + 1 }));
  },
}));
