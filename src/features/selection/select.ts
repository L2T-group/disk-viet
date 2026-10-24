import type { LevelView, NodeView } from '../../lib/types.ts';

const under = (child: string, parent: string) =>
  child.startsWith(parent.endsWith('/') ? parent : `${parent}/`);

/** `path` is `root` or inside it. */
export const within = (path: string, root: string) => path === root || under(path, root);

/** Level to reload after a delete: the current one, or the deepest crumb not inside a deleted path. */
export function reopenId(level: LevelView, deleted: string[]): number {
  const gone = (n: NodeView) => deleted.some((d) => within(n.path, d));
  return [...level.crumbs, level.node].reverse().find((n) => !gone(n))?.id ?? 0;
}

/** Drop nodes under another selected node, and duplicate paths. */
export function topLevelOnly(nodes: NodeView[]): NodeView[] {
  return nodes.filter(
    (n, i) =>
      !nodes.some((o, j) => (o.path === n.path ? j < i : under(n.path, o.path))),
  );
}
