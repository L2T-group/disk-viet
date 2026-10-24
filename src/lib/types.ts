/** Mirrors Rust `AppError` (serialized as `{kind, message}`). */
export interface AppError {
  kind: string;
  message: string;
}

/** Mirrors Rust `disk::Volume`. */
export interface Volume {
  path: string;
  name: string;
  total: number;
  free: number;
  isBoot: boolean;
}

export type Safety = 'safe' | 'caution' | 'important' | 'system' | 'unknown';

/** Mirrors Rust `commands::NodeView`. */
export interface NodeView {
  id: number;
  name: string;
  path: string;
  size: number;
  isDir: boolean;
  ruleId: string | null;
  safety: Safety;
  needsAdmin: boolean;
  /** Tool that cleans this node (`brew`, `xcrun`, `docker`); only on the rule-matched node. */
  tool: string | null;
  /** Exact command shown with `tool`. */
  toolCommand: string | null;
  /** `tool` is not installed: cannot be cleaned. */
  toolMissing: boolean;
  /** Inside a tool-cleaned folder: only that folder can be cleaned. */
  toolChild: boolean;
  unreadable: boolean;
  smallBytes: number;
  smallCount: number;
  hasChildren: boolean;
  /** Overview only: root child minus its rule tiles. */
  remainder: boolean;
}

/** Mirrors Rust `commands::LevelView`. */
export interface LevelView {
  node: NodeView;
  children: NodeView[];
  crumbs: NodeView[];
  unexplained: number | null;
}

export interface ScanProgress {
  files: number;
  bytes: number;
}

export interface ScanDone {
  ok: boolean;
  error?: string;
}

export type DeleteMode = 'trash' | 'permanent';

/** Mirrors Rust `cleaner::DeleteItem`. */
export interface DeleteItem {
  path: string;
  mode: DeleteMode;
  ackIrreversible: boolean;
}

/**
 * Mirrors Rust `cleaner::ItemResult`. `error` is a code: a guard reason (`not_found`,
 * `system_safety`, `tool_root_only`, …), `quit:<app>`, `no_trash_on_volume`, `mount_inside`,
 * `covered_by:<outer path>` (a selected parent failed), `trash_failed: …`, `trash_denied`, `admin_cancelled`,
 * `admin_failed`, `not_admin_item`, `tool_missing:<tool>`, `tool_failed:<stderr>`, or an OS message.
 */
export interface ItemResult {
  path: string;
  ok: boolean;
  error: string | null;
}

/** Mirrors Rust `cleaner::DeleteReport`. */
export interface DeleteReport {
  freeBefore: number;
  freeAfter: number;
  /** Tree bytes of items deleted for good (Trash frees nothing until emptied). */
  expected: number;
  results: ItemResult[];
}

/** Mirrors Rust `history::HistoryEntry`; `time` is unix seconds. */
export interface HistoryEntry {
  time: number;
  path: string;
  size: number;
  mode: DeleteMode;
  ok: boolean;
  error?: string;
  /** Cleaned with this tool: `size` is 0 (the tool decides what goes). */
  tool?: string;
}
