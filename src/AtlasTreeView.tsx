import { useEffect, useMemo, useRef, useState } from "react";

// Saved atlas strategies (core/src/atlas.rs).
export type NoteLine = { kind: "h" | "p" | "li"; depth: number; text: string };
export type AtlasNotes = { title: string; lines: NoteLine[] };
export type AtlasVariant = { name: string; nodes: number[]; choices: [number, number][]; notes: AtlasNotes[] };
export type AtlasDevice = { maps: string[]; threads: string[]; rolls: string[]; text: string };
export type AtlasStrategy = { id: number; name: string; site: string; link: string; author: string; imported: number;
  device: AtlasDevice | null; notes: AtlasNotes[]; variants: AtlasVariant[]; own_notes: string };
export type AtlasStore = { list: AtlasStrategy[]; active: number | null; variant: number };

/** The strategy shown in game (or the first one). */
export function currentStrategy(s: AtlasStore | null) {
  if (!s) return null;
  return s.list.find((x) => x.id === s.active) ?? s.list[0] ?? null;
}

// public/atlas_poe1.json (tools/build_atlas_data.mjs).
type ANode = [number, number, number, string, string[]]; // x, y, kind (0 small, 1 notable, 2 keystone, 4 gateway, 5 start), name, stats
export type AtlasData = { version: string; bounds: [number, number, number, number]; start: number; nodes: Record<string, ANode>;
  edges: number[][]; labels: [number, number, string][]; choices: Record<string, Record<string, string>> };

let cache: Promise<AtlasData> | null = null;
export const loadAtlas = () => (cache ??= fetch("/atlas_poe1.json").then((r) => r.json()));

const RADIUS: Record<number, number> = { 0: 34, 1: 56, 2: 76, 4: 46, 5: 80 };
type Box = { x: number; y: number; w: number; h: number };

/** The atlas passive tree with a strategy's points lit up. Scroll to zoom, drag to move. */
export default function AtlasTreeView({ nodes, choices = [], height = 520, compact }: {
  nodes: number[]; choices?: [number, number][]; height?: number; compact?: boolean;
}) {
  const [tree, setTree] = useState<AtlasData | null>(null);
  const [box, setBox] = useState<Box | null>(null);
  const [hover, setHover] = useState<{ id: string; x: number; y: number } | null>(null);
  const [q, setQ] = useState("");
  const svg = useRef<SVGSVGElement | null>(null);
  const drag = useRef<{ x: number; y: number; box: Box; moved: boolean } | null>(null);

  useEffect(() => { loadAtlas().then(setTree).catch(() => setTree(null)); }, []);
  const have = useMemo(() => new Set(nodes.map(String)), [nodes]);
  const picked = useMemo(() => new Map(choices.map(([n, e]) => [String(n), String(e)])), [choices]);
  const needle = q.trim().toLowerCase();
  const found = useMemo(() => {
    if (!tree || needle.length < 3) return new Set<string>();
    return new Set(Object.entries(tree.nodes).filter(([, n]) => n[3].toLowerCase().includes(needle) || n[4].some((s) => s.toLowerCase().includes(needle))).map(([id]) => id));
  }, [tree, needle]);

  const fit = (all = false) => {
    if (!tree) return;
    const pts = all ? [] : [...have].map((id) => tree.nodes[id]).filter(Boolean);
    if (pts.length < 2) {
      const [x0, y0, x1, y1] = tree.bounds;
      setBox({ x: x0 - 400, y: y0 - 400, w: x1 - x0 + 800, h: y1 - y0 + 800 });
      return;
    }
    const xs = pts.map((n) => n[0]), ys = pts.map((n) => n[1]);
    const pad = 700;
    setBox({ x: Math.min(...xs) - pad, y: Math.min(...ys) - pad, w: Math.max(...xs) - Math.min(...xs) + pad * 2, h: Math.max(...ys) - Math.min(...ys) + pad * 2 });
  };
  useEffect(() => { fit(); }, [tree, nodes.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!tree || !box) return <div className="empty" style={{ height }}>Loading the atlas…</div>;

  const toTree = (cx: number, cy: number) => {
    const r = svg.current!.getBoundingClientRect();
    const scale = Math.max(box.w / r.width, box.h / r.height);
    const ox = box.x + (box.w - r.width * scale) / 2, oy = box.y + (box.h - r.height * scale) / 2;
    return { x: ox + (cx - r.left) * scale, y: oy + (cy - r.top) * scale };
  };
  const zoom = (f: number, cx?: number, cy?: number) => {
    const r = svg.current!.getBoundingClientRect();
    const p = toTree(cx ?? r.left + r.width / 2, cy ?? r.top + r.height / 2);
    const w = Math.min(Math.max(box.w * f, 900), 26000), h = box.h * (w / box.w);
    setBox({ x: p.x - (p.x - box.x) * (w / box.w), y: p.y - (p.y - box.y) * (h / box.h), w, h });
  };
  const on = (id: string) => have.has(id);
  const hovered = hover && tree.nodes[hover.id];
  // The big passives you take, for the list under the tree.
  const majors = [...have].map((id) => [id, tree.nodes[id]] as const).filter(([, n]) => n && (n[2] === 1 || n[2] === 2))
    .sort((a, b) => b[1][2] - a[1][2] || a[1][3].localeCompare(b[1][3]));
  const choiceText = (id: string) => { const e = picked.get(id); return e ? tree.choices[id]?.[e] : undefined; };
  // Text is sized for the view, so labels stay readable when zoomed out.
  const labelSize = Math.max(150, box.w / 38) * (compact ? 1.6 : 1);

  return (
    <div className={`treeview atlas-tv ${compact ? "compact" : ""}`}>
      <div className="tv-tools">
        {!compact && <input placeholder="Find on the atlas (e.g. scarab, essence)…" value={q} onChange={(e) => setQ(e.target.value)} />}
        {!compact && needle.length >= 3 && <span className="muted small">{found.size} found</span>}
        <span className="muted small">{have.size > 0 ? have.size - 1 : 0} points</span>
        <span className="grow" />
        <button className="link small" onClick={() => zoom(0.7)} title="Zoom in">＋</button>
        <button className="link small" onClick={() => zoom(1.4)} title="Zoom out">－</button>
        <button className="link small" onClick={() => fit()}>My points</button>
        <button className="link small" onClick={() => fit(true)}>Whole atlas</button>
      </div>
      <svg ref={svg} className="tv-svg" style={{ height }} viewBox={`${box.x} ${box.y} ${box.w} ${box.h}`}
        onWheel={(e) => zoom(e.deltaY > 0 ? 1.18 : 0.85, e.clientX, e.clientY)}
        onMouseDown={(e) => { drag.current = { x: e.clientX, y: e.clientY, box, moved: false }; }}
        onMouseMove={(e) => {
          if (!drag.current) return;
          const r = svg.current!.getBoundingClientRect();
          const scale = Math.max(drag.current.box.w / r.width, drag.current.box.h / r.height);
          setBox({ ...drag.current.box, x: drag.current.box.x - (e.clientX - drag.current.x) * scale, y: drag.current.box.y - (e.clientY - drag.current.y) * scale });
        }}
        onMouseUp={() => { drag.current = null; }} onMouseLeave={() => { drag.current = null; setHover(null); }}>
        <g className="atv-labels">
          {tree.labels.map(([x, y, name], i) => <text key={i} x={x} y={y} textAnchor="middle" style={{ fontSize: labelSize * 0.55 }}>{name}</text>)}
        </g>
        <g className="tv-edges">
          {tree.edges.map((e, i) => {
            if (e.length === 3) return null; // gateway pairs: no line
            const a = String(e[0]), b = String(e[1]);
            const na = tree.nodes[a], nb = tree.nodes[b];
            if (!na || !nb) return null;
            const cls = on(a) && on(b) ? "e-on" : "e-off";
            if (e.length === 5) {
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
            const cls = `k${n[2]} ${on(id) ? "on" : ""} ${found.has(id) ? "found" : ""}`;
            const r = RADIUS[n[2]] ?? 34;
            return (
              <g key={id}>
                <circle cx={n[0]} cy={n[1]} r={r} className={cls} />
                <circle cx={n[0]} cy={n[1]} r={Math.max(r, 60)} className="hit"
                  onMouseEnter={(e) => setHover({ id, x: e.clientX, y: e.clientY })} onMouseLeave={() => setHover(null)} />
              </g>
            );
          })}
        </g>
        <g className="atv-names">
          {majors.map(([id, n]) => (
            <text key={id} x={n[0]} y={n[1] + RADIUS[n[2]] + labelSize * 0.75} textAnchor="middle" className={n[2] === 2 ? "k2" : ""}
              style={{ fontSize: labelSize * 0.62 }}>{n[3]}</text>
          ))}
        </g>
      </svg>
      {hovered && hover && (
        <div className="tv-tip" style={{ left: Math.min(hover.x + 14, window.innerWidth - 290), top: hover.y + 14 }}>
          <b>{hovered[3] || "Passive"}</b>
          {choiceText(hover.id) ? <div className="tv-pick">Pick: {choiceText(hover.id)}</div> : hovered[4].map((s, i) => <div key={i}>{s}</div>)}
          {on(hover.id) && <div className="tv-tag new">Taken</div>}
        </div>
      )}
      {!compact && majors.length > 0 && (
        <div className="atv-majors">
          {majors.map(([id, n]) => (
            <span key={id} className={`atv-major k${n[2]}`} title={(choiceText(id) ?? n[4].join("\n"))}>
              {n[3]}{choiceText(id) && <span className="atv-pick"> · {choiceText(id)}</span>}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

/** A strategy's notes: device setup, then each heading as a fold-out section. */
export function StrategyNotes({ s, v, open }: { s: AtlasStrategy; v: AtlasVariant | undefined; open?: string[] }) {
  const sections = [...(v?.notes ?? []), ...s.notes];
  return (
    <div className="atn">
      {s.device && (
        <div className="atn-device">
          {s.device.maps.length > 0 && <div><span className="atn-k">Map</span>{s.device.maps.join(", ")}</div>}
          {s.device.threads.length > 0 && <div><span className="atn-k">Threads</span>{s.device.threads.join(" · ")}</div>}
          {s.device.rolls.length > 0 && <div><span className="atn-k">Map rolls</span>{s.device.rolls.join(" · ")}</div>}
          {s.device.text && <div className="atn-tip">{s.device.text}</div>}
        </div>
      )}
      {s.own_notes.trim() && (
        <details className="atn-sec" open>
          <summary>My notes</summary>
          <div className="atn-own">{s.own_notes}</div>
        </details>
      )}
      {sections.map((n, i) => (
        <details key={i} className="atn-sec" open={open ? open.includes(n.title) : i === 0}>
          <summary>{n.title}</summary>
          {n.lines.map((l, j) => l.kind === "h" ? <div key={j} className="atn-h">{l.text}</div>
            : l.kind === "li" ? <div key={j} className="atn-li" style={{ marginLeft: l.depth * 14 }}>{l.text}</div>
            : <p key={j}>{l.text}</p>)}
        </details>
      ))}
      {sections.length === 0 && !s.device && !s.own_notes.trim() && <div className="muted small">No notes from the site for this one (add your own on the Atlas page).</div>}
    </div>
  );
}
