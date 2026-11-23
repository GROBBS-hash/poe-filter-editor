import { useEffect, useMemo, useState } from "react";
import AtlasTreeView, { type AtlasData, type AtlasStrategy, loadAtlas } from "./AtlasTreeView";
import AtlasTotals from "./AtlasTotals";
import { cutOff, pathTo } from "./atlasEdit";

type Change = { kind: "add" | "cut"; target: number; nodes: number[] };
const MAX_POINTS = 150;

/**
 * Editing a saved atlas tree (layout B he chose 23 Nov): click a grey point to add it (with the shortest way
 * there), a gold one to take it off (with anything that only hangs off it); hover shows the change first. Each
 * change is listed with its points and can be undone on its own. Totals underneath show what the changes do.
 * Saves over the strategy or as a new one (`atlas_save_tree`); the creator's tree on maxroll is never touched.
 */
export default function AtlasEditor({ s, vi, run, onDone }: {
  s: AtlasStrategy; vi: number; run: (cmd: string, args: Record<string, unknown>) => Promise<void>; onDone: (id: number | null) => void;
}) {
  const v = s.variants[vi];
  const [tree, setTree] = useState<AtlasData | null>(null);
  const [changes, setChanges] = useState<Change[]>([]);
  const [hoverId, setHoverId] = useState<number | null>(null);
  const [asNew, setAsNew] = useState(false);
  const [name, setName] = useState(`${s.name} (my edit)`);
  const [busy, setBusy] = useState(false);
  useEffect(() => { loadAtlas().then(setTree).catch(() => setTree(null)); }, []);

  // Changes applied in order; undoing one can change what later ones do, so they're worked out again each time.
  const { set, done } = useMemo(() => {
    const set = new Set(v.nodes);
    const done: (Change & { i: number })[] = [];
    if (!tree) return { set, done };
    changes.forEach((c, i) => {
      const nodes = c.kind === "add" ? pathTo(tree, set, c.target) : cutOff(tree, set, c.target);
      if (nodes.length === 0) return;
      if (c.kind === "add") nodes.forEach((n) => set.add(n)); else nodes.forEach((n) => set.delete(n));
      done.push({ ...c, nodes, i });
    });
    return { set, done };
  }, [tree, v.nodes, changes]);
  // The original order with removed points left out, then added paths in the order they were added.
  const current = useMemo(() => {
    const out = v.nodes.filter((n) => set.has(n));
    const seen = new Set(out);
    for (const c of done) if (c.kind === "add") for (const n of c.nodes) if (set.has(n) && !seen.has(n)) { out.push(n); seen.add(n); }
    return out;
  }, [set, done, v.nodes]);
  const choices = v.choices.filter(([n]) => set.has(n));
  const hint = useMemo(() => {
    if (!tree || hoverId == null || hoverId === tree.start) return undefined;
    return set.has(hoverId) ? { add: [], cut: cutOff(tree, set, hoverId) } : { add: pathTo(tree, set, hoverId), cut: [] };
  }, [tree, hoverId, set]);

  if (!tree) return <div className="empty">Loading the atlas…</div>;
  const pick = (id: number) => {
    if (id === tree.start) return;
    setChanges((c) => [...c, { kind: set.has(id) ? "cut" : "add", target: id, nodes: [] }]);
  };
  const points = set.size - 1;
  const plus = done.filter((c) => c.kind === "add").reduce((a, c) => a + c.nodes.length, 0);
  const minus = done.filter((c) => c.kind === "cut").reduce((a, c) => a + c.nodes.length, 0);
  const nameOf = (id: number) => tree.nodes[String(id)]?.[3] || "A point";
  const over = points > MAX_POINTS;
  const save = async () => {
    setBusy(true);
    await run("atlas_save_tree", { id: s.id, variant: vi, nodes: current, choices, asNew: asNew ? name : "" });
    setBusy(false);
    onDone(asNew ? -1 : s.id); // -1: show the new copy (it's the active one now)
  };
  const site = s.site === "maxroll" ? "maxroll" : s.site === "official" ? "pathofexile.com" : "";

  return (
    <>
      <div className="ate">
        <div className="ate-tree">
          <AtlasTreeView nodes={current} choices={choices} height={600} hint={hint} onPick={pick} onHoverNode={setHoverId} />
        </div>
        <div className="ate-side">
          <div className="ate-points">
            <b className={over ? "over" : ""}>{points}</b><span className="muted"> / {MAX_POINTS} points</span>
            <span className="grow" />
            {plus > 0 && <span className="ate-pill plus">+{plus}</span>}
            {minus > 0 && <span className="ate-pill minus">−{minus}</span>}
          </div>
          <div className="ate-bar"><i className={over ? "over" : ""} style={{ width: `${Math.min(100, (points / MAX_POINTS) * 100)}%` }} /></div>
          {over && <div className="small ate-warn">That's {points - MAX_POINTS} more than you can spend: take some off.</div>}
          <div className="ate-h">Your changes</div>
          {done.length === 0 && <div className="muted small ate-empty">Click a grey point to add it, or a gold one to take it off. Hover first to see what changes.</div>}
          {done.map((c) => (
            <div key={c.i} className="ate-chg">
              <span className={`ate-pill ${c.kind === "add" ? "plus" : "minus"}`}>{c.kind === "add" ? "+" : "−"}{c.nodes.length}</span>
              <b>{nameOf(c.target)}</b>
              <button className="link small" onClick={() => setChanges((list) => list.filter((_, j) => j !== c.i))}>Undo</button>
            </div>
          ))}
          <label className="ate-new">
            <input type="checkbox" checked={asNew} onChange={(e) => setAsNew(e.target.checked)} /> Save as a new strategy (keep this one as it is)
          </label>
          {asNew && <input className="text" value={name} onChange={(e) => setName(e.target.value)} placeholder="Name" />}
          <div className="row">
            <button className="primary" disabled={busy || done.length === 0 || (asNew && !name.trim())} onClick={save}>{busy ? "Saving…" : asNew ? "Save as new" : "Save"}</button>
            <button disabled={changes.length === 0} onClick={() => setChanges((c) => c.slice(0, -1))}>Undo last</button>
            <button className="link small" onClick={() => onDone(null)}>Cancel</button>
          </div>
          {site && <div className="muted small">Saving changes your copy here. The creator's tree on {site} stays as it is.</div>}
        </div>
      </div>
      <AtlasTotals nodes={current} choices={choices} base={{ nodes: v.nodes, choices: v.choices }} />
    </>
  );
}
