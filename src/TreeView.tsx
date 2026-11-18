import { useEffect, useMemo, useRef, useState } from "react";

type NodeData = [number, number, number, string, string[], string]; // x, y, kind, name, stats, ascendancy
type TreeData = { bounds: [number, number, number, number]; nodes: Record<string, NodeData>; edges: number[][] };

let cache: Promise<TreeData> | null = null;
const loadTree = () => (cache ??= fetch("/tree_poe1.json").then((r) => r.json()));

const RADIUS = [26, 40, 58, 30, 38, 60, 40]; // by kind: small, notable, keystone, mastery, jewel, start, asc start
type Box = { x: number; y: number; w: number; h: number };

/** The passive tree for one stage: your points gold, new ones this stage green, respecced-away ones red. */
export default function TreeView({ nodes, previous, height = 520 }: { nodes: number[]; previous?: number[]; height?: number }) {
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
            return <circle key={id} cx={n[0]} cy={n[1]} r={RADIUS[n[2]] ?? 26} className={cls}
              onMouseEnter={(e) => setHover({ id, x: e.clientX, y: e.clientY })} onMouseLeave={() => setHover(null)} />;
          })}
        </g>
      </svg>
      {hovered && hover && (
        <div className="tv-tip" style={{ left: hover.x + 14, top: hover.y + 14 }}>
          <b>{hovered[3] || "Passive"}</b>
          {hovered[4].map((s, i) => <div key={i}>{s}</div>)}
          {added.has(hover.id) && <div className="tv-tag new">New this stage</div>}
          {removed.has(hover.id) && <div className="tv-tag gone">Take this back this stage</div>}
        </div>
      )}
      {asc.length > 0 && (
        <div className="tv-asc small"><span className="muted">Ascendancy:</span> {asc.map((n) => n[3]).filter((x, i, a) => x && a.indexOf(x) === i).join(" · ")}</div>
      )}
    </div>
  );
}
