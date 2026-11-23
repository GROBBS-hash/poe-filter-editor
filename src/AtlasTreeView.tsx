import { useEffect, useMemo, useRef, useState } from "react";

// Saved atlas strategies (core/src/atlas.rs).
export type NoteLine = { kind: "h" | "p" | "li"; depth: number; text: string };
export type AtlasNotes = { title: string; lines: NoteLine[] };
/** One step of a tree (like maxroll's "Leveling Path"): the points to take, ending at `node`. */
export type AtlasStep = { name: string; node: number; nodes: number[]; total: number };
export type AtlasVariant = { name: string; nodes: number[]; choices: [number, number][]; notes: AtlasNotes[]; steps?: AtlasStep[] };
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

/**
 * Which step you're on, remembered per strategy tree (so the F7 window opens where you left off).
 * Starts on the last step (the whole tree).
 */
export function useStep(key: string, count: number): [number, (i: number) => void] {
  const read = () => {
    try { const v = localStorage.getItem(`atlas-step:${key}`); if (v != null) return Math.min(Math.max(0, Number(v)), Math.max(0, count - 1)); } catch { /* fine */ }
    return Math.max(0, count - 1);
  };
  const [step, setStep] = useState(read);
  useEffect(() => { setStep(read()); }, [key, count]); // eslint-disable-line react-hooks/exhaustive-deps
  const set = (i: number) => {
    const v = Math.min(Math.max(0, i), Math.max(0, count - 1));
    setStep(v);
    try { localStorage.setItem(`atlas-step:${key}`, String(v)); } catch { /* fine */ }
  };
  // Other windows (page ↔ F7 window) see the change too.
  useEffect(() => {
    const on = (e: StorageEvent) => { if (e.key === `atlas-step:${key}`) setStep(read()); };
    window.addEventListener("storage", on);
    return () => window.removeEventListener("storage", on);
  }); // eslint-disable-line react-hooks/exhaustive-deps
  return [step, set];
}

// Dot sizes in tree units: taken notables and keystones stand out.
const R_OFF: Record<number, number> = { 0: 20, 1: 38, 2: 52, 4: 30, 5: 60 };
const R_ON: Record<number, number> = { 0: 44, 1: 80, 2: 100, 4: 50, 5: 70 };
type Box = { x: number; y: number; w: number; h: number };

/** "‹ 2 · Shaping the Mountains · 8 points   13/105 ›" */
export function StepBar({ steps, step, onStep }: { steps: AtlasStep[]; step: number; onStep: (i: number) => void }) {
  const s = steps[step];
  if (!s) return null;
  const all = steps[steps.length - 1].total;
  return (
    <div className="atv-stepbar">
      <button className="atv-arrow" disabled={step <= 0} onClick={() => onStep(step - 1)} title="Previous step">‹</button>
      <div className="atv-stepname">
        <span className="atv-stepno">{step + 1}</span>
        <span className="atv-steptext"><b title={s.name}>{s.name}</b><span className="small">{s.nodes.length} point{s.nodes.length === 1 ? "" : "s"} this step</span></span>
      </div>
      <span className="atv-count">{s.total}/{all}</span>
      <button className="atv-arrow" disabled={step >= steps.length - 1} onClick={() => onStep(step + 1)} title="Next step">›</button>
    </div>
  );
}

/** The numbered list of steps; click one to jump to it. */
export function StepList({ steps, step, onStep }: { steps: AtlasStep[]; step: number; onStep: (i: number) => void }) {
  const cur = useRef<HTMLButtonElement | null>(null);
  // Keep the current step in view by scrolling only the list (scrollIntoView also scrolled the whole page, so
  // opening the Atlas page jumped down to the list).
  useEffect(() => {
    const b = cur.current;
    if (!b) return;
    // The nearest box that scrolls on its own (the list here, its panel in the F7 window), never the page itself.
    let box = b.parentElement;
    while (box && box !== document.body && !(/(auto|scroll)/.test(getComputedStyle(box).overflowY) && box.scrollHeight > box.clientHeight)) box = box.parentElement;
    if (!box || box === document.body) return;
    const r = b.getBoundingClientRect(), br = box.getBoundingClientRect();
    if (r.top < br.top) box.scrollTop -= br.top - r.top;
    else if (r.bottom > br.bottom) box.scrollTop += r.bottom - br.bottom;
  }, [step]);
  return (
    <div className="atv-steps">
      {steps.map((s, i) => (
        <button key={i} ref={i === step ? cur : undefined} className={`atv-step ${i === step ? "on" : i < step ? "done" : ""}`} onClick={() => onStep(i)}>
          <span className="atv-stepno">{i + 1}</span>
          <span className="atv-steptext"><b>{s.name}</b><span className="small">{s.nodes.length} point{s.nodes.length === 1 ? "" : "s"}, {s.total} total</span></span>
        </button>
      ))}
    </div>
  );
}

/**
 * The atlas passive tree: what you take in gold, the current step bright green with its name,
 * the rest of the atlas faint. Hover any point for its name. Scroll to zoom, drag to move.
 */
export default function AtlasTreeView({ nodes, choices = [], steps, step, height = 520, compact, find, hint, onPick, onHoverNode }: {
  nodes: number[]; choices?: [number, number][]; steps?: AtlasStep[]; step?: number; height?: number; compact?: boolean;
  /** Editing: points a click would add (green) or take off (red). */
  hint?: { add: number[]; cut: number[] };
  /** Editing: a point was clicked (not dragged), and the point under the mouse. */
  onPick?: (id: number) => void;
  onHoverNode?: (id: number | null) => void;
  /** Centre the view on this node (`n` changes on every click, so the same node can be found again). */
  find?: { id: number; n: number } | null;
}) {
  const [tree, setTree] = useState<AtlasData | null>(null);
  const [box, setBox] = useState<Box | null>(null);
  const [px, setPx] = useState(600); // the drawing's width on screen, for text sizes
  const [hover, setHover] = useState<{ id: string; x: number; y: number } | null>(null);
  const [areas, setAreas] = useState(false);
  // "Find on the atlas" (he asked 23 Nov): name or what it does; Enter goes to the next one.
  const [q, setQ] = useState("");
  const [hit, setHit] = useState(-1);
  const svg = useRef<SVGSVGElement | null>(null);
  const drag = useRef<{ x: number; y: number; box: Box } | null>(null);
  const moved = useRef(false);
  // The view right now, and the glide to a new view when the step changes.
  const boxNow = useRef<Box | null>(null);
  boxNow.current = box;
  const anim = useRef<number | null>(null);
  const stopGlide = () => { if (anim.current != null) { cancelAnimationFrame(anim.current); anim.current = null; } };
  useEffect(() => stopGlide, []);
  /** Moves the view smoothly to `to` (straight there when Windows asks for less motion). */
  const glide = (to: Box) => {
    stopGlide();
    const from = boxNow.current;
    const still = typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (!from || still) { setBox(to); return; }
    const t0 = performance.now(), ms = 420;
    const tick = (now: number) => {
      const k = Math.min(1, (now - t0) / ms), e = 1 - Math.pow(1 - k, 3); // ease out
      const mix = (a: number, b: number) => a + (b - a) * e;
      setBox({ x: mix(from.x, to.x), y: mix(from.y, to.y), w: mix(from.w, to.w), h: mix(from.h, to.h) });
      anim.current = k < 1 ? requestAnimationFrame(tick) : null;
    };
    anim.current = requestAnimationFrame(tick);
  };

  useEffect(() => { loadAtlas().then(setTree).catch(() => setTree(null)); }, []);
  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setPx(el.getBoundingClientRect().width || 600));
    ro.observe(el);
    // Scrolling over the tree zooms it and nothing else: the page must not scroll too (he asked 23 Nov). React's own
    // wheel handler can't stop that (it's passive), so a plain listener does.
    const stop = (e: WheelEvent) => e.preventDefault();
    el.addEventListener("wheel", stop, { passive: false });
    return () => { ro.disconnect(); el.removeEventListener("wheel", stop); };
  }, [tree, box != null]);

  const stepping = !!steps && steps.length > 0 && step != null;
  // Taken so far (up to this step) and this step's points.
  const have = useMemo(() => {
    if (!stepping) return new Set(nodes.map(String));
    const s = new Set<string>(tree ? [String(tree.start)] : []);
    steps!.slice(0, step! + 1).forEach((x) => x.nodes.forEach((n) => s.add(String(n))));
    return s;
  }, [nodes, steps, step, stepping, tree]);
  const now = useMemo(() => new Set(stepping ? steps![step!].nodes.map(String) : []), [steps, step, stepping]);
  const picked = useMemo(() => new Map(choices.map(([n, e]) => [String(n), String(e)])), [choices]);
  const needle = q.trim().toLowerCase();
  // Notables and keystones first, so Enter visits the big ones before the small path points.
  const matches = useMemo(() => {
    if (!tree || needle.length < 3) return [] as string[];
    const out = Object.entries(tree.nodes).filter(([id, n]) =>
      [n[3], ...(n[4] ?? []), ...Object.values(tree.choices[id] ?? {})].join(" ").toLowerCase().includes(needle));
    out.sort((a, b) => Number(b[1][2] === 1 || b[1][2] === 2) - Number(a[1][2] === 1 || a[1][2] === 2));
    return out.map(([id]) => id);
  }, [tree, needle]);
  const matchSet = useMemo(() => new Set(matches), [matches]);
  const hAdd = useMemo(() => new Set((hint?.add ?? []).map(String)), [hint?.add.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps
  const hCut = useMemo(() => new Set((hint?.cut ?? []).map(String)), [hint?.cut.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  /** The view around these nodes, with room round the edge (never zoomed in too far). */
  const around = (ids: Iterable<string>): Box | null => {
    if (!tree) return null;
    const pts = [...ids].map((id) => tree.nodes[id]).filter(Boolean);
    if (pts.length === 0) return null;
    const xs = pts.map((n) => n[0]), ys = pts.map((n) => n[1]);
    const pad = 700, min = 3200;
    let x0 = Math.min(...xs) - pad, x1 = Math.max(...xs) + pad, y0 = Math.min(...ys) - pad, y1 = Math.max(...ys) + pad;
    if (x1 - x0 < min) { const c = (x0 + x1) / 2; x0 = c - min / 2; x1 = c + min / 2; }
    if (y1 - y0 < min) { const c = (y0 + y1) / 2; y0 = c - min / 2; y1 = c + min / 2; }
    return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  };
  const fit = (all = false) => {
    stopGlide();
    if (!tree) return;
    const pts = all ? [] : nodes.map((id) => tree.nodes[String(id)]).filter(Boolean);
    if (pts.length < 2) {
      const [x0, y0, x1, y1] = tree.bounds;
      setBox({ x: x0 - 400, y: y0 - 400, w: x1 - x0 + 800, h: y1 - y0 + 800 });
      return;
    }
    const xs = pts.map((n) => n[0]), ys = pts.map((n) => n[1]);
    const pad = 600;
    setBox({ x: Math.min(...xs) - pad, y: Math.min(...ys) - pad, w: Math.max(...xs) - Math.min(...xs) + pad * 2, h: Math.max(...ys) - Math.min(...ys) + pad * 2 });
  };
  useEffect(() => {
    // Editing: a click adds or takes off points; keep the view where he zoomed to (he asked 23 Nov).
    if (onPick && boxNow.current) return;
    const b = stepping ? around(have) : null;
    if (b) setBox(b); else fit();
  }, [tree, nodes.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps
  // A new step: glide to show everything taken so far, the new step's points included.
  const firstStep = useRef(true);
  useEffect(() => {
    if (firstStep.current) { firstStep.current = false; return; }
    if (!stepping || !tree) return;
    const b = around(have);
    if (b) glide(b);
  }, [step]); // eslint-disable-line react-hooks/exhaustive-deps
  // "Find it on the tree": zoom in around that node.
  useEffect(() => {
    const n = find && tree?.nodes[String(find.id)];
    if (!n) return;
    const b = boxNow.current, w = 3600, h = b ? 3600 * (b.h / b.w) : 3600;
    glide({ x: n[0] - w / 2, y: n[1] - h / 2, w, h });
  }, [find?.n, tree]); // eslint-disable-line react-hooks/exhaustive-deps
  // New search words: show all the matches at once.
  useEffect(() => {
    setHit(-1);
    const b = matches.length ? around(matches) : null;
    if (b) glide(b);
  }, [matches]); // eslint-disable-line react-hooks/exhaustive-deps
  /** Zooms in on match i (wraps round). */
  const goHit = (i: number) => {
    if (!tree || matches.length === 0) return;
    const k = ((i % matches.length) + matches.length) % matches.length;
    const n = tree.nodes[matches[k]];
    const b = boxNow.current, w = 3600, h = b ? 3600 * (b.h / b.w) : 3600;
    setHit(k);
    glide({ x: n[0] - w / 2, y: n[1] - h / 2, w, h });
  };

  if (!tree || !box) return <div className="empty" style={{ height }}>Loading the atlas…</div>;

  const toTree = (cx: number, cy: number) => {
    const r = svg.current!.getBoundingClientRect();
    const scale = Math.max(box.w / r.width, box.h / r.height);
    const ox = box.x + (box.w - r.width * scale) / 2, oy = box.y + (box.h - r.height * scale) / 2;
    return { x: ox + (cx - r.left) * scale, y: oy + (cy - r.top) * scale };
  };
  const zoom = (f: number, cx?: number, cy?: number) => {
    stopGlide();
    const r = svg.current!.getBoundingClientRect();
    const p = toTree(cx ?? r.left + r.width / 2, cy ?? r.top + r.height / 2);
    const w = Math.min(Math.max(box.w * f, 900), 26000), h = box.h * (w / box.w);
    setBox({ x: p.x - (p.x - box.x) * (w / box.w), y: p.y - (p.y - box.y) * (h / box.h), w, h });
  };
  const on = (id: string) => have.has(id);
  const hovered = hover && tree.nodes[hover.id];
  const choiceText = (id: string) => { const e = picked.get(id); return e ? tree.choices[id]?.[e] : undefined; };
  // Tree units per screen pixel (the drawing keeps its shape, so the tighter side decides).
  const upp = Math.max(box.w / px, box.h / height);
  // Names: only this step's notables/keystones (and its last point), so the tree stays readable.
  const named = stepping ? [...now].filter((id) => { const n = tree.nodes[id]; return n && (n[2] === 1 || n[2] === 2 || Number(id) === steps![step!].node); }) : [];
  // Searching: name the notables found (all of them when there are few, else just the one Enter is on).
  if (matches.length > 0) {
    const big = matches.filter((id) => tree.nodes[id][2] === 1 || tree.nodes[id][2] === 2);
    for (const id of big.length <= 12 ? big : hit >= 0 ? [matches[hit]] : []) if (!named.includes(id)) named.push(id);
  }
  const edgeClass = (a: string, b: string) => {
    if ((hAdd.has(a) || hAdd.has(b)) && (on(a) || hAdd.has(a)) && (on(b) || hAdd.has(b))) return "e-add";
    if ((hCut.has(a) || hCut.has(b)) && on(a) && on(b)) return "e-cut";
    return on(a) && on(b) ? (now.has(a) || now.has(b) ? "e-now" : "e-on") : "e-off";
  };

  return (
    <div className={`treeview atlas-tv ${compact ? "compact" : ""}`}>
      <div className="tv-tools">
        {!stepping && <span className="muted small">{Math.max(0, have.size - 1)} points</span>}
        <input className="atv-find" placeholder="Find on the atlas (e.g. pack size, Exarch)…" value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") { e.preventDefault(); goHit(hit + (e.shiftKey ? -1 : 1)); }
            else if (e.key === "Escape") setQ("");
          }} />
        {needle.length >= 3 && <span className="muted small atv-find-n" title="Enter = next one, Shift+Enter = back, Esc = clear">
          {matches.length === 0 ? "none found" : hit >= 0 ? `${hit + 1} of ${matches.length}` : `${matches.length} found · Enter`}
        </span>}
        {needle.length >= 3 && matches.length > 0 && hit >= 0 && tree.nodes[matches[hit]] && (
          <b className="small atv-find-name">{tree.nodes[matches[hit]][3]}</b>
        )}
        <span className="grow" />
        <button className={`link small ${areas ? "atv-on" : ""}`} onClick={() => setAreas(!areas)} title="Show the names of the atlas areas (Breach, Delirium…)">Area names</button>
        <button className="link small" onClick={() => zoom(0.7)} title="Zoom in">＋</button>
        <button className="link small" onClick={() => zoom(1.4)} title="Zoom out">－</button>
        <button className="link small" onClick={() => fit()}>My points</button>
        <button className="link small" onClick={() => fit(true)}>Whole atlas</button>
      </div>
      <svg ref={svg} className="tv-svg atv-svg" style={{ height }} viewBox={`${box.x} ${box.y} ${box.w} ${box.h}`}
        onWheel={(e) => zoom(e.deltaY > 0 ? 1.18 : 0.85, e.clientX, e.clientY)}
        onMouseDown={(e) => { stopGlide(); moved.current = false; drag.current = { x: e.clientX, y: e.clientY, box }; }}
        onMouseMove={(e) => {
          if (!drag.current) return;
          if (Math.abs(e.clientX - drag.current.x) + Math.abs(e.clientY - drag.current.y) > 4) moved.current = true;
          const r = svg.current!.getBoundingClientRect();
          const scale = Math.max(drag.current.box.w / r.width, drag.current.box.h / r.height);
          setBox({ ...drag.current.box, x: drag.current.box.x - (e.clientX - drag.current.x) * scale, y: drag.current.box.y - (e.clientY - drag.current.y) * scale });
        }}
        onMouseUp={() => { drag.current = null; }} onMouseLeave={() => { drag.current = null; setHover(null); }}>
        {areas && (
          <g className="atv-labels">
            {tree.labels.map(([x, y, name], i) => <text key={i} x={x} y={y} textAnchor="middle" style={{ fontSize: 11 * upp }}>{name}</text>)}
          </g>
        )}
        <g className="atv-edges" style={{ ["--w" as string]: `${Math.max(6, 1.3 * upp)}px` }}>
          {tree.edges.map((e, i) => {
            if (e.length === 3) return null; // gateway pairs: no line
            const a = String(e[0]), b = String(e[1]);
            const na = tree.nodes[a], nb = tree.nodes[b];
            if (!na || !nb) return null;
            const cls = edgeClass(a, b);
            if (e.length === 5) {
              const [, , cx, cy, r] = e;
              const a1 = Math.atan2(na[1] - cy, na[0] - cx), a2 = Math.atan2(nb[1] - cy, nb[0] - cx);
              let d = a2 - a1; while (d > Math.PI) d -= 2 * Math.PI; while (d < -Math.PI) d += 2 * Math.PI;
              return <path key={i} className={cls} d={`M${na[0]} ${na[1]} A${r} ${r} 0 0 ${d > 0 ? 1 : 0} ${nb[0]} ${nb[1]}`} />;
            }
            return <line key={i} className={cls} x1={na[0]} y1={na[1]} x2={nb[0]} y2={nb[1]} />;
          })}
        </g>
        <g className="atv-nodes">
          {Object.entries(tree.nodes).map(([id, n]) => {
            const taken = on(id);
            const r = (taken ? R_ON : R_OFF)[n[2]] ?? 24;
            const cls = `k${n[2]} ${taken ? (now.has(id) ? "now" : "on") : ""} ${hAdd.has(id) ? "add" : ""} ${hCut.has(id) ? "cut" : ""} ${(find && String(find.id) === id) || matchSet.has(id) ? "found" : ""} ${hit >= 0 && matches[hit] === id ? "found-now" : ""}`;
            return (
              <g key={id}>
                <circle cx={n[0]} cy={n[1]} r={Math.max(r, taken || hAdd.has(id) ? (n[2] === 1 || n[2] === 2 ? 5 : 3) * upp : 0)} className={cls} />
                <circle cx={n[0]} cy={n[1]} r={Math.max(r, 7 * upp)} className="hit"
                  onMouseEnter={(e) => { setHover({ id, x: e.clientX, y: e.clientY }); onHoverNode?.(Number(id)); }}
                  onMouseLeave={() => { setHover(null); onHoverNode?.(null); }}
                  onClick={() => { if (onPick && !moved.current) onPick(Number(id)); }} />
              </g>
            );
          })}
        </g>
        <g className="atv-names" style={{ fontSize: 13 * upp, strokeWidth: 4 * upp }}>
          {named.map((id) => {
            const n = tree.nodes[id];
            return <text key={id} x={n[0]} y={n[1] - (R_ON[n[2]] ?? 34) - 5 * upp} textAnchor="middle">{n[3]}</text>;
          })}
        </g>
      </svg>
      {hovered && hover && (
        <div className="tv-tip" style={{ left: Math.min(hover.x + 14, window.innerWidth - 290), top: hover.y + 14 }}>
          <b>{hovered[3] || "Passive"}</b>
          {choiceText(hover.id) ? <div className="tv-pick">Pick: {choiceText(hover.id)}</div> : hovered[4].map((s, i) => <div key={i}>{s}</div>)}
          {onPick && hAdd.size > 0 ? <div className="tv-tag add">Click to add · +{hAdd.size} point{hAdd.size > 1 ? "s" : ""}</div>
            : onPick && hCut.size > 0 ? <div className="tv-tag cut">Click to take off · −{hCut.size} point{hCut.size > 1 ? "s" : ""}</div>
            : now.has(hover.id) ? <div className="tv-tag new">This step</div> : on(hover.id) && <div className="tv-tag on">Taken</div>}
        </div>
      )}
    </div>
  );
}

/**
 * Nodes with several options (e.g. Blended Realities) taken up to this step, with the option the strategy
 * picks; this step's ones highlighted. Click one to find it on the tree (like the build window's Masteries).
 */
export function ChoiceList({ nodes, choices, steps, step, onFind }: {
  nodes: number[]; choices: [number, number][]; steps?: AtlasStep[]; step?: number; onFind: (id: number) => void;
}) {
  const [tree, setTree] = useState<AtlasData | null>(null);
  useEffect(() => { loadAtlas().then(setTree).catch(() => setTree(null)); }, []);
  if (!tree || choices.length === 0) return null;
  const stepping = !!steps && steps.length > 0 && step != null;
  const taken = new Set(stepping ? steps!.slice(0, step! + 1).flatMap((s) => s.nodes) : nodes);
  const now = new Set(stepping ? steps![step!].nodes : []);
  // In the order they're taken.
  const order = stepping ? steps!.flatMap((s) => s.nodes) : nodes;
  const list = choices.filter(([n]) => taken.has(n)).sort((a, b) => order.indexOf(a[0]) - order.indexOf(b[0]));
  if (list.length === 0) return null;
  return (
    <div className="atv-choices">
      <div className="atv-choices-head">Options picked <span className="muted small">(click one to find it on the tree)</span></div>
      {list.map(([node, eff]) => (
        <button key={node} className={`tv-mastery ${now.has(node) ? "new" : ""}`} onClick={() => onFind(node)}>
          <span className="tv-m-name">{tree.nodes[String(node)]?.[3] ?? "Option"}{now.has(node) && <span className="tv-tag new"> this step</span>}</span>
          <span className="tv-m-eff">{tree.choices[String(node)]?.[String(eff)] ?? "?"}</span>
        </button>
      ))}
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
      {sections.length === 0 && !s.device && !s.own_notes.trim() && <div className="muted small">No notes from the site for this one (add your own on the Atlas tree page).</div>}
    </div>
  );
}
