import { useEffect, useMemo, useRef, useState } from "react";
import AtlasTreeView, { type AtlasData, type AtlasStore, loadAtlas } from "./AtlasTreeView";
import AtlasTotals from "./AtlasTotals";

/** A saved tree: strategy id and which of its trees ("Early", "Late"…). */
type Ref = { id: number; v: number };
const KEY = "atlas-compare";
const refKey = (r: Ref) => `${r.id}:${r.v}`;
const parse = (s: string): Ref => { const [id, v] = s.split(":").map(Number); return { id, v: v || 0 }; };

function read(): [Ref, Ref] | null {
  try { const x = JSON.parse(localStorage.getItem(KEY) ?? "null"); return Array.isArray(x) && x.length === 2 ? x as [Ref, Ref] : null; } catch { return null; }
}

/** One tree of the pair, with what only it has coloured. */
function Side({ tag, title, nodes, choices, only, kind, find }: {
  tag: "A" | "B"; title: string; nodes: number[]; choices: [number, number][]; only: number[]; kind: "gain" | "lost";
  find: { id: number; n: number } | null;
}) {
  return (
    <div className="acmp-side">
      <div className="acmp-title"><span className={`acmp-chip ${kind}`}>{tag}</span><b>{title}</b><span className="muted small">{Math.max(0, nodes.length - 1)} points</span></div>
      <AtlasTreeView nodes={nodes} choices={choices} height={430} hint={kind === "gain" ? { add: only, cut: [] } : { add: [], cut: only }} find={find} />
      <div className="small muted">{kind === "lost" ? "Red: only in A" : "Green: only in B"}</div>
    </div>
  );
}

/**
 * Atlas page: two saved trees side by side (he picked mock-up B, 26 Nov, with the Compare / with dropdowns): what only A has
 * in red, what only B has in green, the notables that differ, and the totals A → B.
 */
export default function AtlasCompare({ store, shown, pick }: { store: AtlasStore; shown: number | null; pick?: { id: number; n: number } | null }) {
  const all = useMemo(() => store.list.flatMap((s) => s.variants.map((v, i) => ({
    ref: { id: s.id, v: i }, label: s.variants.length > 1 ? `${s.name} · ${v.name}` : s.name, nodes: v.nodes, choices: v.choices,
  }))), [store]);
  const [pair, setPair] = useState<[Ref, Ref] | null>(read);
  const [tree, setTree] = useState<AtlasData | null>(null);
  const [find, setFind] = useState<{ side: "A" | "B"; id: number; n: number } | null>(null);
  useEffect(() => { loadAtlas().then(setTree).catch(() => setTree(null)); }, []);

  // Trees that are gone (deleted) fall back to the one shown and the one in game, or the first two.
  const has = (r: Ref | undefined) => !!r && all.some((x) => refKey(x.ref) === refKey(r));
  const firstOther = (not: Ref) => all.find((x) => refKey(x.ref) !== refKey(not))?.ref;
  let a = pair?.[0], b = pair?.[1];
  if (!has(a)) a = all.find((x) => x.ref.id === shown)?.ref ?? all[0]?.ref;
  if (!has(b) || (a && b && refKey(a) === refKey(b))) b = (store.active != null && store.active !== a?.id ? all.find((x) => x.ref.id === store.active)?.ref : undefined) ?? (a ? firstOther(a) : undefined);
  const A = all.find((x) => a && refKey(x.ref) === refKey(a));
  const B = all.find((x) => b && refKey(x.ref) === refKey(b));

  const save = (p: [Ref, Ref]) => { setPair(p); try { localStorage.setItem(KEY, JSON.stringify(p)); } catch { /* fine */ } };
  const card = useRef<HTMLDivElement | null>(null);
  // "Compare" on a strategy: it becomes A (B stays, unless it's the same tree), and the page goes there.
  useEffect(() => {
    if (!pick || !a || !b) return;
    const na: Ref = { id: pick.id, v: 0 };
    save([na, refKey(b) === refKey(na) ? (refKey(a) !== refKey(na) ? a : firstOther(na) ?? b) : b]);
    window.setTimeout(() => card.current?.scrollIntoView({ behavior: "smooth", block: "start" }), 60);
  }, [pick?.n]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!A || !B) return null;

  const inA = new Set(A.nodes), inB = new Set(B.nodes);
  const onlyA = A.nodes.filter((n) => !inB.has(n)), onlyB = B.nodes.filter((n) => !inA.has(n));
  const same = A.nodes.filter((n) => inB.has(n)).length;
  const notable = (id: number) => { const n = tree?.nodes[String(id)]; return !!n && (n[2] === 1 || n[2] === 2); };
  const name = (id: number) => tree?.nodes[String(id)]?.[3] ?? String(id);
  const chips = (ids: number[], kind: "gain" | "lost", side: "A" | "B") => {
    const list = ids.filter(notable).sort((x, y) => name(x).localeCompare(name(y)));
    if (list.length === 0) return <span className="muted small">No notables, only small points.</span>;
    return list.map((id) => (
      <button key={id} className={`acmp-note ${kind}`} title={`${(tree?.nodes[String(id)]?.[4] ?? []).join("\n")}\n\nClick: show it on tree ${side}`}
        onClick={() => setFind((f) => ({ side, id, n: (f?.n ?? 0) + 1 }))}>{kind === "gain" ? "+" : "−"} {name(id)}</button>
    ));
  };
  const select = (r: Ref, set: (r: Ref) => void) => (
    <select value={refKey(r)} onChange={(e) => set(parse(e.target.value))}>
      {all.map((x) => <option key={refKey(x.ref)} value={refKey(x.ref)}>{x.label}</option>)}
    </select>
  );

  return (
    <div className="card acmp" ref={card}>
      <h2>Compare two trees</h2>
      <div className="row acmp-pick">
        <span className="muted">Compare</span>{select(A.ref, (r) => save([r, B.ref]))}
        <button className="link" title="Swap A and B" onClick={() => save([B.ref, A.ref])}>⇄</button>
        <span className="muted">with</span>{select(B.ref, (r) => save([A.ref, r]))}
      </div>
      {refKey(A.ref) === refKey(B.ref) ? <p className="muted small">Pick two different trees.</p> : <>
        <div className="acmp-sum">
          <span><b className="ff-up">+{onlyB.length}</b> points only in B</span>
          <span><b className="ff-dn">−{onlyA.length}</b> only in A</span>
          <span><b>{Math.max(0, same - 1)}</b> in both</span>
        </div>
        <div className="acmp-two">
          <Side tag="A" title={A.label} nodes={A.nodes} choices={A.choices} only={onlyA} kind="lost" find={find?.side === "A" ? find : null} />
          <Side tag="B" title={B.label} nodes={B.nodes} choices={B.choices} only={onlyB} kind="gain" find={find?.side === "B" ? find : null} />
        </div>
        <div className="acmp-two">
          <div><div className="acmp-h">Notables only in A</div><div className="acmp-notes">{chips(onlyA, "lost", "A")}</div></div>
          <div><div className="acmp-h">Notables only in B</div><div className="acmp-notes">{chips(onlyB, "gain", "B")}</div></div>
        </div>
        <AtlasTotals nodes={B.nodes} choices={B.choices} base={{ nodes: A.nodes, choices: A.choices }} label="What changes going from A to B" />
      </>}
    </div>
  );
}
