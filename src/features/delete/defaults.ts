import type { DeleteMode, NodeView } from '../../lib/types.ts';

/** Rows that cannot go to the Trash: admin items (Trash refuses them) and the Trash itself. */
export const lockedMode = (n: NodeView): DeleteMode | null =>
  n.needsAdmin || n.ruleId === 'trash' ? 'permanent' : null;

/** Spec §4.3: 🟢 → permanent; 🟡/🔴/⚪ → Trash; locked rows → permanent. */
export const defaultMode = (n: NodeView): DeleteMode =>
  lockedMode(n) ?? (n.safety === 'safe' ? 'permanent' : 'trash');

/** 🔴/⚪ deleted for good need the "cannot be recovered" acknowledgement. */
export const isRisky = (n: NodeView, mode: DeleteMode) =>
  mode === 'permanent' && (n.safety === 'important' || n.safety === 'unknown');

/** Tool rows (brew, simctl, docker) are cleaned by the tool, whatever their mode. */
export function totals(rows: { node: NodeView; mode: DeleteMode }[]) {
  const t = { permanent: 0, trash: 0, tool: 0, risky: 0 };
  for (const { node, mode } of rows) {
    if (node.tool) {
      t.tool += node.size;
      continue;
    }
    t[mode] += node.size;
    if (isRisky(node, mode)) t.risky++;
  }
  return t;
}
