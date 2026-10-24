export type Rect = { x: number; y: number; w: number; h: number };

// Worst aspect ratio of a row with area sum s, min/max item area, laid along side length l.
const worst = (s: number, min: number, max: number, l: number) =>
  Math.max((l * l * max) / (s * s), (s * s) / (l * l * min));

/** Squarified treemap (Bruls, Huizing, van Wijk). Input sorted descending; size <= 0 dropped. */
export function squarify(
  items: { id: number; size: number }[],
  rect: Rect,
): (Rect & { id: number })[] {
  const list = items.filter((i) => i.size > 0);
  const total = list.reduce((s, i) => s + i.size, 0);
  const out: (Rect & { id: number })[] = [];
  if (total === 0 || rect.w <= 0 || rect.h <= 0) return out;

  const k = (rect.w * rect.h) / total;
  let { x, y, w, h } = rect;
  let i = 0;
  while (i < list.length) {
    const horiz = w >= h; // row is a column along the shorter side
    // shortcut: size ratios > ~1e16 give Infinity lengths, unreachable for file sizes
    const side = horiz ? h : w; // can reach 0 after float drift
    let j = i;
    let sum = 0;
    let min = Infinity;
    let max = 0;
    let cur = Infinity;
    while (j < list.length) {
      const a = list[j]!.size * k;
      const nsum = sum + a;
      const nw = worst(nsum, Math.min(min, a), Math.max(max, a), side);
      if (nw > cur) break;
      cur = nw;
      sum = nsum;
      min = Math.min(min, a);
      max = Math.max(max, a);
      j++;
    }
    const last = j >= list.length;
    const thick = last ? (horiz ? w : h) : sum / side; // last row absorbs rounding drift
    let p = horiz ? y : x;
    for (let m = i; m < j; m++) {
      const len = (list[m]!.size * k) / thick;
      out.push(
        horiz ? { id: list[m]!.id, x, y: p, w: thick, h: len } : { id: list[m]!.id, x: p, y, w: len, h: thick },
      );
      p += len;
    }
    if (horiz) {
      x += thick;
      w -= thick;
    } else {
      y += thick;
      h -= thick;
    }
    i = j;
  }
  return out;
}
