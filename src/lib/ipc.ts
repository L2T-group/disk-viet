// Typed IPC wrapper. Every feature calls Rust via `invoke()`; never import
// `@tauri-apps/api/core` directly so that `--mode mock` (browser + Playwright)
// keeps working.
import type { AppError } from './types';

type Unlisten = () => void;

let mockMod: typeof import('./mock') | null = null;
async function mock() {
  if (!mockMod) mockMod = await import('./mock');
  return mockMod;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === 'object' && e !== null && 'kind' in e && 'message' in e;
}

export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Invoke a Rust command. Arguments use camelCase keys (Tauri converts). */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (__MOCK__) return (await mock()).mockInvoke<T>(cmd, args ?? {});
  const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (e) {
    throw isAppError(e) ? e : ({ kind: 'unknown', message: String(e) } satisfies AppError);
  }
}

/** Subscribe to a backend event. Returns an unlisten function. */
export async function listen<T>(event: string, cb: (payload: T) => void): Promise<Unlisten> {
  if (__MOCK__) return (await mock()).mockListen<T>(event, cb);
  const { listen: tauriListen } = await import('@tauri-apps/api/event');
  return tauriListen<T>(event, (e) => cb(e.payload));
}

/** Native folder picker; `null` when cancelled. Mock mode returns the fixture home folder. */
export async function pickFolder(): Promise<string | null> {
  if (__MOCK__) return '/Users/demo';
  const { open } = await import('@tauri-apps/plugin-dialog');
  const r = await open({ directory: true });
  return typeof r === 'string' ? r : null;
}
