import { create } from 'zustand';
import { t } from '@/i18n';
import { errorMessage, invoke, listen } from '@/lib/ipc';
import type { LevelView, NodeView, ScanDone, ScanProgress, Volume } from '@/lib/types';

interface ScanState {
  volumes: Volume[];
  root: string | null;
  scanning: boolean;
  progress: ScanProgress | null;
  level: LevelView | null;
  focused: NodeView | null;
  selected: Map<number, NodeView>;
  error: string | null;
  /** Load volumes, subscribe to scan events, scan the boot volume. Idempotent. */
  init(): Promise<void>;
  startScan(root: string): Promise<void>;
  cancelScan(): Promise<void>;
  open(id: number): Promise<void>;
  focus(n: NodeView | null): void;
  toggleSelect(n: NodeView): void;
  clearSelection(): void;
}

let initialized = false;
/** Latest `open()` call; an older `level_get` that resolves late is dropped. */
let openSeq = 0;
// Real backend sends AppError::Cancelled's text, the mock sends 'cancelled'.
const isCancelled = (e?: string) => e === 'cancelled' || e === 'operation cancelled';

export const useScan = create<ScanState>()((set, get) => ({
  volumes: [],
  root: null,
  scanning: false,
  progress: null,
  level: null,
  focused: null,
  selected: new Map(),
  error: null,

  async init() {
    if (initialized) return;
    initialized = true;
    await listen<ScanProgress>('scan://progress', (p) => {
      if (get().scanning) set({ progress: p });
    });
    await listen<ScanDone>('scan://done', (d) => {
      // A cancelled scan's late "done" must not touch a newer scan's state.
      if (!d.ok && isCancelled(d.error)) return;
      set({ scanning: false, progress: null });
      if (d.ok) {
        set({ selected: new Map() });
        void get().open(0);
      } else set({ error: d.error ?? t('topbar.scanFailed') });
    });
    try {
      const volumes = await invoke<Volume[]>('volumes_list');
      set({ volumes });
      const boot = volumes.find((v) => v.isBoot) ?? volumes[0];
      if (boot) await get().startScan(boot.path);
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  async startScan(root) {
    set({ root, scanning: true, progress: { files: 0, bytes: 0 }, error: null });
    try {
      await invoke('scan_start', { root });
    } catch (e) {
      set({ scanning: false, progress: null, error: errorMessage(e) });
    }
  },

  async cancelScan() {
    set({ scanning: false, progress: null });
    try {
      await invoke('scan_cancel');
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  async open(id) {
    const my = ++openSeq;
    try {
      const level = await invoke<LevelView>('level_get', { id });
      if (my === openSeq) set({ level, focused: null, error: null });
    } catch (e) {
      if (my === openSeq) set({ error: errorMessage(e) });
    }
  },

  focus: (focused) => set({ focused }),

  toggleSelect(n) {
    const selected = new Map(get().selected);
    if (selected.has(n.id)) selected.delete(n.id);
    else selected.set(n.id, n);
    set({ selected });
  },

  clearSelection: () => set({ selected: new Map() }),
}));

// Lets Playwright drive the store (perf measurement of a 2,000-child level).
if (__MOCK__) (window as unknown as { __scan: typeof useScan }).__scan = useScan;
