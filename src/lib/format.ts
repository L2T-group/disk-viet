const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

/** Decimal units like Finder (1 GB = 10⁹ B); one decimal from GB up: `12.0 GB`, `850 MB`. */
export function formatBytes(n: number): string {
  let v = n;
  for (let i = 0; ; i++) {
    const s = v.toFixed(i >= 3 ? 1 : 0);
    if (Number(s) < 1000 || i === UNITS.length - 1) return `${s} ${UNITS[i]}`;
    v /= 1000;
  }
}
