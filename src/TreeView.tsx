import { useEffect, useMemo, useRef, useState } from "react";

type NodeData = [number, number, number, string, string[], string]; // x, y, kind, name, stats, ascendancy
type TreeData = { bounds: [number, number, number, number]; nodes: Record<string, NodeData>; edges: number[][]; effects?: Record<string, string> };

let cache: Promise<TreeData> | null = null;
const loadTree = () => (cache ??= fetch("/tree_poe1.json").then((r) => r.json()));

const RADIUS = [26, 40, 58, 30, 38, 60, 40]; // by kind: small, notable, keystone, mastery, jewel, start, asc start
type Box = { x: number; y: number; w: number; h: number };

/** The passive tree for one stage: your points gold, new ones this stage green, respecced-away ones red. */
/** Your ascendancy on its own: notables named, taken ones gold (new this stage green). Small passives are just dots. */
function Ascendancy({ tree, name, have, added }: { tree: TreeData; name: string; have: Set<string>; added: Set<string> }) {
  const [tip, setTip] = useState<{ id: string; x: number; y: number } | null>(null);
  const list = Object.entries(tree.nodes).filter(([, n]) => n[5] === name);
  if (list.length === 0) return null;
  const ids = new Set(list.map(([id]) => id));
  const xs = list.map(([, n]) => n[0]), ys = list.map(([, n]) => n[1]);
  const pad = 260;
  const vb = `${Math.min(...xs) - pad} ${Math.min(...ys) - pad} ${Math.max(...xs) - Math.min(...xs) + pad * 2} ${Math.max(...ys) - Math.min(...ys) + pad * 2}`;
  const majors = list.filter(([id, n]) => n[2] === 1 && have.has(id));
  const centre = list.find(([, n]) => n[2] === 6)?.[1];
  const t = tip && tree.nodes[tip.id];
  return (
    <div className="tv-ascbox">
      <div className="tv-asc-head">{name}{majors.length > 0 && <span className="muted small"> · {majors.map(([, n]) => n[3]).join(" · ")}</span>}</div>
      <svg className="tv-asc-svg" viewBox={vb}>
        {tree.edges.filter((e) => ids.has(String(e[0])) && ids.has(String(e[1]))).map((e, i) => {
          const a = tree.nodes[String(e[0])], b = tree.nodes[String(e[1])];
          const on = have.has(String(e[0])) && have.has(String(e[1]));
          if (e.length === 5) {
            const [, , cx, cy, r] = e;
            const a1 = Math.atan2(a[1] - cy, a[0] - cx), a2 = Math.atan2(b[1] - cy, b[0] - cx);
            let d = a2 - a1; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI;
            return <path key={i} className={on ? "e-on" : "e-off"} d={`M${a[0]} ${a[1]} A${r} ${r} 0 0 ${d > 0 ? 1 : 0} ${b[0]} ${b[1]}`} />;
          }
          return <line key={i} className={on ? "e-on" : "e-off"} x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} />;
        })}
        {list.map(([id, n]) => {
          const major = n[2] === 1, start = n[2] === 6;
          const state = have.has(id) ? (added.has(id) ? "new" : "on") : "";
          return (
            <g key={id} onMouseEnter={(e) => setTip({ id, x: e.clientX, y: e.clientY })} onMouseLeave={() => setTip(null)}>
              <circle cx={n[0]} cy={n[1]} r={start ? 50 : major ? 54 : 22} className={`${major ? "k1" : start ? "k5" : "k0"} ${state}`} />
              {major && state && (() => {
                // Beside the node, pointing away from the ascendancy's centre, so neighbours don't overlap.
                const dx = centre ? n[0] - centre[0] : 0;
                const anchor = dx < -40 ? "end" : dx > 40 ? "start" : "middle";
                const x = n[0] + (anchor === "end" ? -80 : anchor === "start" ? 80 : 0);
                const y = n[1] + (anchor === "middle" ? 120 : 18);
                return <text x={x} y={y} textAnchor={anchor} className={`tv-asc-label ${state}`}>{n[3]}</text>;
              })()}
            </g>
          );
        })}
      </svg>
      {t && tip && (
        <div className="tv-tip" style={{ left: tip.x + 14, top: tip.y + 14 }}>
          <b>{t[3]}</b>{t[4].map((x, i) => <div key={i}>{x}</div>)}
        </div>
      )}
    </div>
  );
}

export default function TreeView({ nodes, previous, height = 520, ascendancy, masteries = [], prevMasteries }: {
  nodes: number[]; previous?: number[]; height?: number; ascendancy?: string;
  /** (mastery node, effect picked) for this stage and the one before. */
  masteries?: [number, number][]; prevMasteries?: [number, number][];
}) {
  const [tree, setTree] = useState<TreeData | null>(null);
  const [box, setBox] = useState<Box | null>(null);
  const [hover, setHover] = useState<{ id: string; x: number; y: number } | null>(null);
  const [q, setQ] = useState("");
  const svg = useRef<SVGSVGElement | null>(null);
  const drag = useRef<{ x: number; y: number; box: Box } | null>(null);

  useEffect(() => { loadTree().then(setTree).catch(() => setTree(null)); }, []);

  const have = useMemo(() => new Set(nodes.map(String)), [nodes]);
  const before = useMemo(() => new Set((previous ?? []).map(String)), [previous]);
  const added = useMemo(() => new Set([...have].filter((n) => previous && !before.has(n))), [have, before, previous]);
  const removed = useMemo(() => new Set([...before].filter((n) => !have.has(n))), [have, before]);
  const picked = useMemo(() => new Map(masteries.map(([n, e]) => [String(n), String(e)])), [masteries]);
  const pickedBefore = useMemo(() => new Map((prevMasteries ?? []).map(([n, e]) => [String(n), String(e)])), [prevMasteries]);
  const needle = q.trim().toLowerCase();
  const found = useMemo(() => {
    if (!tree || needle.length < 3) return new Set<string>();
    return new Set(Object.entries(tree.nodes).filter(([, n]) => !n[5] && (n[3].toLowerCase().includes(needle) || n[4].some((s) => s.toLowerCase().includes(needle)))).map(([id]) => id));
  }, [tree, needle]);

  // Fit to your points (or the whole tree) when the stage changes.
  const fit = (all = false) => {
    if (!tree) return;
    const pts = all ? [] : [...have].map((id) => tree.nodes[id]).filter((n) => n && !n[5]);
    if (pts.length < 2) {
      const [x0, y0, x1, y1] = tree.bounds;
      setBox({ x: x0 - 300, y: y0 - 300, w: x1 - x0 + 600, h: y1 - y0 + 600 });
      return;
    }
    const xs = pts.map((n) => n[0]), ys = pts.map((n) => n[1]);
    const pad = 900;
    setBox({ x: Math.min(...xs) - pad, y: Math.min(...ys) - pad, w: Math.max(...xs) - Math.min(...xs) + pad * 2, h: Math.max(...ys) - Math.min(...ys) + pad * 2 });
  };
  useEffect(() => { fit(); }, [tree, nodes.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!tree || !box) return <div className="empty">Loading the tree…</div>;

  const toTree = (cx: number, cy: number) => {
    const r = svg.current!.getBoundingClientRect();
    const scale = Math.max(box.w / r.width, box.h / r.height);
    const ox = box.x + (box.w - r.width * scale) / 2, oy = box.y + (box.h - r.height * scale) / 2;
    return { x: ox + (cx - r.left) * scale, y: oy + (cy - r.top) * scale, scale };
  };
  const zoom = (f: number, cx?: number, cy?: number) => {
    const r = svg.current!.getBoundingClientRect();
    const p = toTree(cx ?? r.left + r.width / 2, cy ?? r.top + r.height / 2);
    const w = Math.min(Math.max(box.w * f, 800), 30000), h = box.h * (w / box.w);
    setBox({ x: p.x - (p.x - box.x) * (w / box.w), y: p.y - (p.y - box.y) * (h / box.h), w, h });
  };

  const on = (id: string) => have.has(id);
  const effectText = (e?: string) => (e && tree.effects?.[e]) || "";
  /** Centre the view on one node (from the masteries list). */
  const focusNode = (id: string) => {
    const n = tree.nodes[id];
    if (n) setBox({ x: n[0] - 1600, y: n[1] - 1000, w: 3200, h: 2000 });
    setHover(null);
  };
  const edgeClass = (a: string, b: string) => (on(a) && on(b) ? (added.has(a) || added.has(b) ? "e-new" : "e-on") : "e-off");
  const hovered = hover && tree.nodes[hover.id];
  const asc = [...have].map((id) => tree.nodes[id]).filter((n) => n && n[5] && n[2] !== 6);

  return (
    <div className="treeview">
      <div className="tv-tools">
        <input placeholder="Find on the tree (e.g. life, cast speed)…" value={q} onChange={(e) => setQ(e.target.value)} />
        {needle.length >= 3 && <span className="muted small">{found.size} found</span>}
        <span className="grow" />
        <button className="link small" onClick={() => zoom(0.7)}>＋</button>
        <button className="link small" onClick={() => zoom(1.4)}>－</button>
        <button className="link small" onClick={() => fit()}>My points</button>
        <button className="link small" onClick={() => fit(true)}>Whole tree</button>
      </div>
      <div className="tv-legend small">
        <span><i className="dot on" /> taken</span>
        {previous && <span><i className="dot new" /> new this stage ({added.size})</span>}
        {previous && removed.size > 0 && <span><i className="dot gone" /> take back ({removed.size})</span>}
        <span className="muted">scroll to zoom · drag to move</span>
      </div>
      <svg ref={svg} className="tv-svg" style={{ height }} viewBox={`${box.x} ${box.y} ${box.w} ${box.h}`}
        onWheel={(e) => zoom(e.deltaY > 0 ? 1.18 : 0.85, e.clientX, e.clientY)}
        onMouseDown={(e) => { drag.current = { x: e.clientX, y: e.clientY, box }; }}
        onMouseMove={(e) => {
          if (!drag.current) return;
          const r = svg.current!.getBoundingClientRect();
          const scale = Math.max(drag.current.box.w / r.width, drag.current.box.h / r.height);
          setBox({ ...drag.current.box, x: drag.current.box.x - (e.clientX - drag.current.x) * scale, y: drag.current.box.y - (e.clientY - drag.current.y) * scale });
        }}
        onMouseUp={() => { drag.current = null; }} onMouseLeave={() => { drag.current = null; setHover(null); }}>
        <g className="tv-edges">
          {tree.edges.map((e, i) => {
            const a = String(e[0]), b = String(e[1]);
            const na = tree.nodes[a], nb = tree.nodes[b];
            if (!na || !nb || na[5] || nb[5]) return null;
            const cls = edgeClass(a, b);
            if (e.length === 5) {
              // Arc around the group's centre, the short way round.
              const [, , cx, cy, r] = e;
              const a1 = Math.atan2(na[1] - cy, na[0] - cx), a2 = Math.atan2(nb[1] - cy, nb[0] - cx);
              let d = a2 - a1; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI;
              return <path key={i} className={cls} d={`M${na[0]} ${na[1]} A${r} ${r} 0 0 ${d > 0 ? 1 : 0} ${nb[0]} ${nb[1]}`} />;
            }
            return <line key={i} className={cls} x1={na[0]} y1={na[1]} x2={nb[0]} y2={nb[1]} />;
          })}
        </g>
        <g className="tv-nodes">
          {Object.entries(tree.nodes).map(([id, n]) => {
            if (n[5]) return null;
            const cls = `k${n[2]} ${on(id) ? (added.has(id) ? "new" : "on") : removed.has(id) ? "gone" : ""} ${found.has(id) ? "found" : ""}`;
            const enter = (e: React.MouseEvent) => setHover({ id, x: e.clientX, y: e.clientY });
            return (
              <g key={id}>
                <circle cx={n[0]} cy={n[1]} r={RADIUS[n[2]] ?? 26} className={cls} />
                {/* Masteries are hard to hit: a big invisible target around them. */}
                <circle cx={n[0]} cy={n[1]} r={n[2] === 3 ? 110 : Math.max(RADIUS[n[2]] ?? 26, 45)} className="hit"
                  onMouseEnter={enter} onMouseLeave={() => setHover(null)} />
              </g>
            );
          })}
        </g>
      </svg>
      {hovered && hover && (
        <div className="tv-tip" style={{ left: hover.x + 14, top: hover.y + 14 }}>
          <b>{hovered[3] || "Passive"}</b>
          {hovered[2] === 3 && picked.has(hover.id) ? (
            <>
              <div className="tv-pick">Take: {effectText(picked.get(hover.id))}</div>
              <div className="muted small">{hovered[4].length - 1} other choices</div>
            </>
          ) : hovered[4].map((s, i) => <div key={i}>{s}</div>)}
          {added.has(hover.id) && <div className="tv-tag new">New this stage</div>}
          {removed.has(hover.id) && <div className="tv-tag gone">Take this back this stage</div>}
        </div>
      )}
      {picked.size > 0 && (
        <div className="tv-masteries">
          <div className="tv-asc-head">Masteries <span className="muted small">(click one to find it on the tree)</span></div>
          {[...picked.entries()].map(([node, eff]) => {
            const n = tree.nodes[node];
            const isNew = previous && pickedBefore.get(node) !== eff;
            return (
              <button key={node} className={`tv-mastery ${isNew ? "new" : ""}`} onClick={() => focusNode(node)}>
                <span className="tv-m-name">{n?.[3] ?? "Mastery"}{isNew && <span className="tv-tag new"> new</span>}</span>
                <span className="tv-m-eff">{effectText(eff)}</span>
              </button>
            );
          })}
        </div>
      )}
      {ascendancy ? <Ascendancy tree={tree} name={ascendancy} have={have} added={added} />
        : asc.length > 0 && <div className="tv-asc small"><span className="muted">Ascendancy:</span> {asc.filter((n) => n[2] === 1).map((n) => n[3]).join(" · ")}</div>}
    </div>
  );
}
