import { create } from 'zustand';

const KEY = 'diskviet.nested';

function load(): boolean {
  try {
    return localStorage.getItem(KEY) !== '0';
  } catch {
    return true;
  }
}

/** "Show folders inside large tiles"; default on. Read by the treemap, no re-mount. */
export const useNested = create<{ on: boolean; set: (on: boolean) => void }>((set) => ({
  on: load(),
  set: (on) => {
    try {
      localStorage.setItem(KEY, on ? '1' : '0');
    } catch {
      /* works without storage */
    }
    set({ on });
  },
}));
