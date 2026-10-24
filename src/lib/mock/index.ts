// Fake IPC for `--mode mock`.
import type { DeleteItem, DeleteReport, HistoryEntry, ScanDone, ScanProgress } from '../types';
import { children, level, removeNode, volumes } from './fixtures';

type Cb = (payload: never) => void;
const listeners = new Map<string, Set<Cb>>();

function emit(event: string, payload: unknown) {
  for (const cb of listeners.get(event) ?? []) (cb as (p: unknown) => void)(payload);
}

let timers: ReturnType<typeof setTimeout>[] = [];
let scanned = false;
const history: HistoryEntry[] = [];

export async function mockInvoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  switch (cmd) {
    case 'volumes_list':
      return volumes as T;
    case 'scan_start': {
      timers.forEach(clearTimeout);
      const steps: [number, string, ScanProgress | ScanDone][] = [
        [50, 'scan://progress', { files: 12_000, bytes: 30e9 }],
        [150, 'scan://progress', { files: 48_000, bytes: 95e9 }],
        [250, 'scan://done', { ok: true }],
      ];
      timers = steps.map(([ms, ev, p]) =>
        setTimeout(() => {
          if (ev === 'scan://done') scanned = true;
          emit(ev, p);
        }, ms),
      );
      return undefined as T;
    }
    case 'scan_cancel':
      timers.forEach(clearTimeout);
      timers = [];
      emit('scan://done', { ok: false, error: 'cancelled' } satisfies ScanDone);
      return undefined as T;
    case 'reveal_path':
    case 'open_fda_settings':
      return undefined as T;
    case 'level_get': {
      // Test hook: `window.__mockDelay = { [id]: ms }` makes that level answer late.
      const ms = (globalThis as { __mockDelay?: Record<number, number> }).__mockDelay?.[Number(args.id)];
      if (ms) await new Promise((r) => setTimeout(r, ms));
      if (!scanned) throw { kind: 'notScanned', message: 'nothing has been scanned yet' };
      return level(Number(args.id)) as T;
    }
    case 'children_get':
      if (!scanned) throw { kind: 'notScanned', message: 'nothing has been scanned yet' };
      return children(args.ids as number[], Boolean(args.overview)) as T;
    case 'delete_run': {
      const freeBefore = volumes[0]!.free;
      let expected = 0;
      const results = (args.items as DeleteItem[]).map((it) => {
        // Fixture: Telegram is "running", so its cache is refused like the real process check.
        const busy = it.path.endsWith('/ru.keepcoder.Telegram') ? 'quit:Telegram' : null;
        // Like the real cleaner: a tool row runs its tool, counts 0 bytes and stays in the tree.
        const tool = it.path.endsWith('/Caches/Homebrew') ? 'brew' : null;
        const size = busy ? null : tool ? 0 : removeNode(it.path);
        const ok = size !== null;
        const error = ok ? null : (busy ?? 'not_found');
        if (it.mode === 'permanent') expected += size ?? 0; // Trash frees nothing until emptied
        history.unshift({
          time: Math.floor(Date.now() / 1000),
          path: it.path,
          size: size ?? 0,
          mode: it.mode,
          ok,
          ...(error ? { error } : {}),
          ...(tool ? { tool } : {}),
        });
        return { path: it.path, ok, error };
      });
      return { freeBefore, freeAfter: freeBefore + expected, expected, results } satisfies DeleteReport as T;
    }
    case 'history_list':
      return history as T;
    default:
      throw { kind: 'invalid', message: `mock: unknown command ${cmd}` };
  }
}

export async function mockListen<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  const set = listeners.get(event) ?? new Set<Cb>();
  listeners.set(event, set);
  set.add(cb as Cb);
  return () => set.delete(cb as Cb);
}
