import { useEffect, useMemo, useState } from "react";
import { type AtlasData, loadAtlas } from "./AtlasTreeView";
import { ItemIcon } from "./icons";
import { GROUPS } from "./atlasGroups";

const NUM = /\d+(?:\.\d+)?/g;
type Total = { key: string; count: number; values: number[] };

/** Every stat line of the taken points, same lines added up ("+12%" ×5 → "+60%"). A multiple-choice notable
 *  counts the option picked. */
function totals(tree: AtlasData, nodes: number[], choices: [number, number][]): Map<string, Total> {
  const picked = new Map(choices.map(([n, e]) => [String(n), String(e)]));
  const out = new Map<string, Total>();
  for (const id of nodes) {
    const n = tree.nodes[String(id)];
    if (!n || id === tree.start) continue;
    const pick = picked.get(String(id));
    const lines = pick && tree.choices[String(id)]?.[pick] ? [tree.choices[String(id)][pick]] : n[4] ?? [];
    for (const raw of lines) {
      const line = raw.replace(/\s*\n\s*/g, " ").trim();
      if (!line || /^Select a type of content/.test(line)) continue;
      const nums = [...line.matchAll(NUM)].map((m) => Number(m[0]));
      const key = line.replace(NUM, "#");
      const t = out.get(key) ?? { key, count: 0, values: nums.map(() => 0) };
      t.count++;
      nums.forEach((x, i) => { t.values[i] = (t.values[i] ?? 0) + x; });
      out.set(key, t);
    }
  }
  return out;
}

const fmt = (x: number) => (Math.round(x * 10) / 10).toString();
const fill = (key: string, values: number[]) => { let i = 0; return key.replace(/#/g, () => fmt(values[i++] ?? 0)); };

/**
 * "Totals": what all your points add up to, grouped by mechanic with its picture (like poe atlas planners:
 * "+265% Surpassing Chance to contain Sacred Flora"). With `base`, each line shows how an edit changes it.
 */
export default function AtlasTotals({ nodes, choices, base }: {
  nodes: number[]; choices: [number, number][]; base?: { nodes: number[]; choices: [number, number][] };
}) {
  const [tree, setTree] = useState<AtlasData | null>(null);
  const [open, setOpen] = useState<Record<string, boolean>>({});
  useEffect(() => { loadAtlas().then(setTree).catch(() => setTree(null)); }, []);
  const groups = useMemo(() => {
    if (!tree) return [];
    const now = totals(tree, nodes, choices);
    const before = base ? totals(tree, base.nodes, base.choices) : null;
    const keys = new Set([...now.keys(), ...(before?.keys() ?? [])]);
    const byGroup = new Map<string, { t: Total | undefined; b: Total | undefined; key: string }[]>();
    for (const key of keys) {
      const g = GROUPS.find(([, , , re]) => re.test(key))![0];
      if (!byGroup.has(g)) byGroup.set(g, []);
      byGroup.get(g)!.push({ key, t: now.get(key), b: before?.get(key) });
    }
    return GROUPS.filter(([g]) => byGroup.has(g)).map(([g, label, item]) => {
      const differs = (r: { t?: Total; b?: Total }) => !!before && (r.t?.values.join() !== r.b?.values.join() || r.t?.count !== r.b?.count);
      // Changed lines first (so a removed one never hides under "more"), then the biggest.
      const rows = byGroup.get(g)!.sort((a, b) => Number(differs(b)) - Number(differs(a)) || (b.t?.count ?? 0) - (a.t?.count ?? 0) || a.key.localeCompare(b.key));
      const changed = rows.filter(differs).length;
      return { g, label, item, rows, points: rows.reduce((s, r) => s + (r.t?.count ?? 0), 0), changed };
    }).sort((a, b) => (a.g === "maps" ? 1 : b.g === "maps" ? -1 : b.points - a.points));
  }, [tree, nodes.join(","), choices.join(","), base?.nodes.join(","), base?.choices.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!tree || groups.length === 0) return null;
  return (
    <div className="att">
      <div className="att-head"><b>Totals</b><span className="muted small">What all your points add up to{base ? ", and what your changes do" : ""}.</span></div>
      <div className="att-grid">
        {groups.map(({ g, label, item, rows, changed }) => {
          const big = rows.length > 6 && !open[g];
          return (
            <section key={g} className={`att-group ${changed ? "changed" : ""}`}>
              <header>
                <span className="att-ic">{item ? <ItemIcon name={item} size={30} /> : null}</span>
                <b>{label}</b>
                {changed > 0 && <span className="att-chg">{changed} changed</span>}
              </header>
              {(big ? rows.slice(0, 6) : rows).map(({ key, t, b }) => {
                const gone = !t;
                const d = t && b ? (t.values[0] ?? 0) - (b.values[0] ?? 0) : 0;
                const isNew = !!t && !!base && !b;
                return (
                  <div key={key} className={`att-row ${gone ? "gone" : ""} ${isNew ? "new" : ""}`}>
                    <span className="att-txt">{fill(key, (t ?? b)!.values)}</span>
                    {gone ? <span className="att-d minus">removed</span>
                      : isNew ? <span className="att-d plus">new</span>
                      : d !== 0 ? <span className={`att-d ${d > 0 ? "plus" : "minus"}`}>{d > 0 ? "+" : "−"}{fmt(Math.abs(d))}</span>
                      : (t!.count > 1 && t!.values.length === 0 ? <span className="att-n">×{t!.count}</span> : null)}
                  </div>
                );
              })}
              {rows.length > 6 && <button className="link small" onClick={() => setOpen((o) => ({ ...o, [g]: !o[g] }))}>{big ? `${rows.length - 6} more` : "Less"}</button>}
            </section>
          );
        })}
      </div>
    </div>
  );
}
