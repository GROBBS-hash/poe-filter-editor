import type { AtlasData } from "./AtlasTreeView";

/** Who each atlas point joins (gateway pairs included, as in core atlas.rs). */
const adjCache = new WeakMap<AtlasData, Map<number, number[]>>();
export function neighbours(tree: AtlasData): Map<number, number[]> {
  let m = adjCache.get(tree);
  if (m) return m;
  m = new Map();
  for (const e of tree.edges as unknown as number[][]) {
    const [a, b] = [Number(e[0]), Number(e[1])];
    if (!m.has(a)) m.set(a, []);
    if (!m.has(b)) m.set(b, []);
    m.get(a)!.push(b);
    m.get(b)!.push(a);
  }
  adjCache.set(tree, m);
  return m;
}

/** Clicking a point you don't have: it and the shortest way to it from your tree (in the order you'd take them). */
export function pathTo(tree: AtlasData, taken: Set<number>, target: number): number[] {
  if (taken.has(target)) return [];
  const adj = neighbours(tree);
  const prev = new Map<number, number>();
  const seen = new Set(taken);
  const queue = [...taken];
  for (let i = 0; i < queue.length; i++) {
    const n = queue[i];
    if (n === target) break;
    for (const m of adj.get(n) ?? []) {
      if (seen.has(m)) continue;
      seen.add(m); prev.set(m, n); queue.push(m);
    }
  }
  if (!seen.has(target)) return [];
  const path: number[] = [];
  for (let c: number | undefined = target; c !== undefined && !taken.has(c); c = prev.get(c)) path.push(c);
  return path.reverse();
}

/** Clicking a point you have: it and every point that only joins the start through it. Never the start. */
export function cutOff(tree: AtlasData, taken: Set<number>, id: number): number[] {
  if (!taken.has(id) || id === tree.start) return [];
  const adj = neighbours(tree);
  const ok = new Set([tree.start]);
  const queue = [tree.start];
  for (let i = 0; i < queue.length; i++) {
    for (const m of adj.get(queue[i]) ?? []) {
      if (m === id || ok.has(m) || !taken.has(m)) continue;
      ok.add(m); queue.push(m);
    }
  }
  return [...taken].filter((n) => !ok.has(n));
}
