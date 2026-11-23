import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import AtlasTreeView, { type AtlasData, type AtlasStep, type AtlasStore, loadAtlas } from "./AtlasTreeView";
import AtlasTotals from "./AtlasTotals";
import { ItemIcon } from "./icons";

/** One creator tree from public/atlas_picks.json (built by tools/build_atlas_picks.mjs). */
type Pick = { id: string; group: "maxroll" | "community"; name: string; author: string; mechanics: string[];
  category: string; season: string; updated: string; maps: string[]; points: number };

/** Mechanics in the order shown, each with the game item pictured on its tile: the mechanic's thread where it
 *  has one (scarabs are gone since 3.30, it's all threads), else its best-known item. */
const MECHS: [string, string, string][] = [
  ["ritual", "Ritual", "Ritual Thread"], ["harvest", "Harvest", "Wild Crystallised Lifeforce"], ["breach", "Breach", "Breach Thread"],
  ["abyss", "Abyss", "Abyss Thread"], ["legion", "Legion", "Legion Thread"], ["blight", "Blight", "Blight Thread"],
  ["essence", "Essence", "Deafening Essence of Greed"], ["betrayal", "Betrayal", "Betrayal Thread"], ["incursion", "Incursion", "Incursion Thread"],
  ["expedition", "Expedition", "Expedition Thread"], ["bestiary", "Bestiary", "Bestiary Thread"], ["ultimatum", "Ultimatum", "Ultimatum Thread"],
  ["delirium", "Delirium", "Simulacrum Splinter"], ["shrines", "Shrines", "Shrine Map"], ["bosses", "Bosses", "Fragment of the Minotaur"],
  ["rare monsters", "Rare monsters", "Orb of Chance"], ["magic monsters", "Magic monsters", "Orb of Augmentation"],
  ["memories", "Memories", "Alva's Memory"], ["settlers", "Settlers", "Kalguuran Scarab"], ["mercenaries", "Mercenaries", "Gold"],
  ["divination cards", "Div cards", "Stacked Deck"],
];
/** Only on "Build my own" (no creator tree for them, but the optimiser knows their nodes). */
const OWN_ONLY = ["divination cards"];
/** "Also spend points on": all-round travel nodes for Build my own (he asked 23 Nov), keys as in core atlas_opt TRAVEL. */
const TRAVEL: [string, string, string][] = [
  ["pack size", "Pack size", "Cartographer's Chisel"], ["explicit effect", "Map mod effect", "Orb of Alchemy"],
  ["threads", "Threads found", "Frayed Thread"], ["maps found", "Maps found", "Map (Tier 16)"], ["rarity", "Item rarity", "Exalted Orb"],
];
const TRAVEL_KEYS = TRAVEL.map(([k]) => k);
const ITEM: Record<string, string> = Object.fromEntries([...MECHS, ...TRAVEL].map(([k, , item]) => [k, item]));
const LABEL: Record<string, string> = Object.fromEntries([...MECHS, ...TRAVEL].map(([k, label]) => [k, label]));
const link = (id: string) => `https://maxroll.gg/poe/farming-strategy-planner/${id}`;

function ago(day: string) {
  const n = Math.round((Date.now() - new Date(day + "T12:00:00").getTime()) / 864e5);
  return n <= 0 ? "today" : n === 1 ? "yesterday" : n < 60 ? `${n} days ago` : `on ${day}`;
}

function MechIcon({ m, size }: { m: string; size: number }) {
  return <span className="ap-mi" title={LABEL[m] ?? m}><ItemIcon name={ITEM[m]} size={size} /></span>;
}

function readSel(): string[] {
  try { return JSON.parse(localStorage.getItem("atlas-picks-sel") ?? "[]"); } catch { return []; }
}

/** Atlas page, top card: pick what you want to farm, see creators' trees that fit (best matches first),
 *  and "Use this tree" saves your own copy under My strategies (the creator's original stays on maxroll). */
export default function AtlasPicks({ store, onImported, onShow }: {
  store: AtlasStore; onImported: (s: AtlasStore, msg: string) => void; onShow: (id: number) => void;
}) {
  const [picks, setPicks] = useState<Pick[] | null>(null);
  const [built, setBuilt] = useState("");
  const [sel, setSel] = useState<string[]>(readSel);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState("");
  const [all, setAll] = useState(false);
  const [tab, setTabState] = useState<"picks" | "own">(() => { try { return localStorage.getItem("atlas-picks-tab") === "own" ? "own" : "picks"; } catch { return "picks"; } });
  const setTab = (t: "picks" | "own") => { setTabState(t); try { localStorage.setItem("atlas-picks-tab", t); } catch { /* fine */ } };

  useEffect(() => {
    fetch("/atlas_picks.json").then((r) => r.json()).then((d) => { setPicks(d.picks ?? []); setBuilt(d.built ?? ""); })
      .catch(() => setPicks([]));
  }, []);
  useEffect(() => { try { localStorage.setItem("atlas-picks-sel", JSON.stringify(sel)); } catch { /* fine */ } setAll(false); }, [sel]);

  const count = useMemo(() => {
    const c: Record<string, number> = {};
    for (const p of picks ?? []) for (const m of p.mechanics) c[m] = (c[m] ?? 0) + 1;
    return c;
  }, [picks]);

  if (!picks || picks.length === 0) return null;

  // Creator trees only know mechanics: travel options and own-only picks don't narrow them.
  const ms = sel.filter((m) => !TRAVEL_KEYS.includes(m) && !OWN_ONLY.includes(m));
  const fit = (p: Pick) => ms.filter((m) => p.mechanics.includes(m)).length;
  const shown = (ms.length === 0 ? picks.filter((p) => p.group === "maxroll") : picks.filter((p) => fit(p) > 0))
    .slice().sort((a, b) => fit(b) - fit(a) || (a.group === b.group ? 0 : a.group === "maxroll" ? -1 : 1) || b.updated.localeCompare(a.updated));
  const list = all ? shown : shown.slice(0, 5);
  const saved = (id: string) => store.list.find((s) => s.link.includes(id));
  const toggle = (m: string) => setSel((s) => s.includes(m) ? s.filter((x) => x !== m) : [...s, m]);
  const names = ms.map((m) => LABEL[m] ?? m);

  const use = async (p: Pick) => {
    setBusy(p.id); setErr("");
    try {
      const s = await invoke<AtlasStore>("atlas_import", { link: link(p.id), name: "" });
      onImported(s, `Saved your copy of “${p.name}” by ${p.author}. It's the one shown in game now.`);
    } catch (e) { setErr(String(e)); }
    setBusy(null);
  };

  const own = tab === "own";
  return (
    <div className="card ap">
      <h2>Build an atlas from strategies</h2>
      <div className="ap-tabs" role="tablist">
        <button role="tab" aria-selected={!own} className={!own ? "on" : ""} onClick={() => setTab("picks")}>Creator trees</button>
        <button role="tab" aria-selected={own} className={own ? "on" : ""} onClick={() => setTab("own")}>Build my own</button>
      </div>
      <p className="muted small">{own
        ? "Pick what you want to farm and the editor builds a tree for it, spending points where they do the most."
        : <>Pick what you want to farm. Trees from maxroll creators for this league{built ? ` (list from ${new Date(built + "T12:00:00").toLocaleDateString(undefined, { day: "numeric", month: "short" })})` : ""}.</>}</p>
      <div className="ap-step">1 · What do you want to farm?</div>
      <div className="ap-tiles">
        {MECHS.filter(([m]) => own || !OWN_ONLY.includes(m)).map(([m, label]) => {
          const n = count[m] ?? 0;
          return (
            <button key={m} className={`ap-tile ${sel.includes(m) ? "on" : ""}`} disabled={!own && n === 0 && !sel.includes(m)}
              onClick={() => toggle(m)} title={own ? label : n ? `${n} tree${n > 1 ? "s" : ""}` : "No creator tree for this yet"}>
              <ItemIcon name={ITEM[m]} size={40} />
              <span>{label}</span>
            </button>
          );
        })}
      </div>
      {own && <>
        <div className="ap-step">Also spend points on <span className="ap-step-note">(the nodes you travel through)</span></div>
        <div className="ap-tiles ap-travel">
          {TRAVEL.map(([m, label]) => (
            <button key={m} className={`ap-tile ${sel.includes(m) ? "on" : ""}`} onClick={() => toggle(m)} title={label}>
              <ItemIcon name={ITEM[m]} size={34} />
              <span>{label}</span>
            </button>
          ))}
        </div>
      </>}
      {own ? <BuildOwn sel={sel} onClear={() => setSel([])} onSaved={onImported} /> : <>
      <div className="ap-step">
        {ms.length === 0 ? "2 · Maxroll team picks (choose above to narrow it down)" : `2 · Creator trees for ${names.join(" + ")}`}
        {sel.length > 0 && <button className="link small" onClick={() => setSel([])}>Clear</button>}
      </div>
      {list.length === 0 && <div className="muted small ap-none">No creator tree has these yet. Try fewer, or paste a link below.</div>}
      {list.map((p) => {
        const n = fit(p);
        const mine = saved(p.id);
        return (
          <div key={p.id} className="ap-match">
            <span className="ap-icons">{p.mechanics.filter((m) => ITEM[m]).map((m) => <MechIcon key={m} m={m} size={28} />)}</span>
            <div className="ap-what">
              <div><b>{p.name.replace(/^"[^"]*":\s*/, "")}</b> <span className={`ap-badge ${p.group}`}>{p.group === "maxroll" ? "Maxroll team" : "Community"}</span></div>
              <div className="muted small">by {p.author} · {p.points} points · updated {ago(p.updated)}{p.maps[0] ? ` · runs ${p.maps[0]}` : ""}</div>
            </div>
            {ms.length > 1 && <span className={`ap-fit ${n === ms.length ? "" : "part"}`}>{n === ms.length ? (ms.length === 2 ? "✓ Both" : "✓ All") : `${n} of ${ms.length}`}</span>}
            <button className="link small" onClick={() => invoke("open_url", { url: link(p.id) })}>maxroll</button>
            {mine
              ? <button onClick={() => onShow(mine.id)} title="You already saved a copy">In My strategies</button>
              : <button className="primary" disabled={busy !== null} onClick={() => use(p)}>{busy === p.id ? "Reading…" : "Use this tree"}</button>}
          </div>
        );
      })}
      {shown.length > list.length && <button className="link small" onClick={() => setAll(true)}>Show {shown.length - list.length} more</button>}
      {err && <div className="small at-msg">{err}</div>}
      </>}
    </div>
  );
}

type NodeWork = { node: number; name: string; value: number; why: string };
type Plan = { nodes: number[]; steps: AtlasStep[]; choices: [number, number][]; points: number; budget: number;
  top: NodeWork[]; weak: NodeWork[]; trees_used: number };
type Priority = "fewest" | "balanced" | "profit";
const PRIORITIES: [Priority, string][] = [["fewest", "Fewest points"], ["balanced", "Balanced"], ["profit", "Most profit"]];

function readList(key: string): number[] {
  try { const v = JSON.parse(localStorage.getItem(key) ?? "[]"); return Array.isArray(v) ? v : []; } catch { return []; }
}
function saveList(key: string, v: unknown) { try { localStorage.setItem(key, JSON.stringify(v)); } catch { /* fine */ } }

/** "Build my own": the optimiser (core atlas_opt.rs) for the picked mechanics. Always / Never on any node it
 *  shows; the tree updates straight away. Save adds it to My strategies like a creator tree. */
function BuildOwn({ sel, onClear, onSaved }: { sel: string[]; onClear: () => void; onSaved: (s: AtlasStore, msg: string) => void }) {
  const [prio, setPrio] = useState<Priority>(() => { try { return (localStorage.getItem("atlas-opt-prio") as Priority) || "balanced"; } catch { return "balanced"; } });
  const [lock, setLock] = useState<number[]>(() => readList("atlas-opt-lock"));
  const [block, setBlock] = useState<number[]>(() => readList("atlas-opt-block"));
  const [plans, setPlans] = useState<Partial<Record<Priority, Plan>>>({});
  const [err, setErr] = useState("");
  const [saving, setSaving] = useState(false);
  const [tree, setTree] = useState<AtlasData | null>(null);
  const names = useMemo(() => {
    const m: Record<number, string> = {};
    for (const [id, n] of Object.entries(tree?.nodes ?? {})) m[Number(id)] = n[3];
    return m;
  }, [tree]);
  // The row opened in "Doing the most" (shows what the node does), and the tree zooming to it.
  const [openNode, setOpenNode] = useState<number | null>(null);
  const [find, setFind] = useState<{ id: number; n: number } | null>(null);

  useEffect(() => { try { localStorage.setItem("atlas-opt-prio", prio); } catch { /* fine */ } }, [prio]);
  useEffect(() => { saveList("atlas-opt-lock", lock); saveList("atlas-opt-block", block); }, [lock, block]);
  // The atlas data: names for the Always / Never chips (a blocked node isn't in the plan any more) and what each node does.
  useEffect(() => { loadAtlas().then(setTree).catch(() => setTree(null)); }, []);
  useEffect(() => {
    if (sel.length === 0) { setPlans({}); return; }
    let live = true;
    const t = window.setTimeout(async () => {
      try {
        const out: Partial<Record<Priority, Plan>> = {};
        for (const [p] of PRIORITIES) out[p] = await invoke<Plan>("atlas_optimise", { ask: { mechanics: sel, priority: p, budget: 0, lock, block } });
        if (live) { setPlans(out); setErr(""); }
      } catch (e) { if (live) setErr(String(e)); }
    }, 120);
    return () => { live = false; window.clearTimeout(t); };
  }, [sel.join(","), lock.join(","), block.join(",")]);

  if (sel.length === 0) return <div className="muted small ap-none">Pick one or more above to build a tree.</div>;
  const plan = plans[prio];
  const label = sel.map((m) => LABEL[m] ?? m).join(" + ");
  const always = (id: number) => { setBlock((b) => b.filter((x) => x !== id)); setLock((l) => l.includes(id) ? l.filter((x) => x !== id) : [...l, id]); };
  const never = (id: number) => { setLock((l) => l.filter((x) => x !== id)); setBlock((b) => b.includes(id) ? b.filter((x) => x !== id) : [...b, id]); };
  const save = async () => {
    setSaving(true);
    try {
      const s = await invoke<AtlasStore>("atlas_save_plan", { ask: { mechanics: sel, priority: prio, budget: 0, lock, block }, name: "" });
      onSaved(s, `Saved “${label}” (${plan?.points ?? ""} points) to My strategies. It's the one shown in game now.`);
    } catch (e) { setErr(String(e)); }
    setSaving(false);
  };
  /** What a node does: the option picked for a multiple-choice notable, else its own lines. */
  const does = (id: number): string[] => {
    const n = tree?.nodes[String(id)];
    if (!n) return [];
    const eff = plan?.choices.find(([c]) => c === id)?.[1];
    const pick = eff != null ? tree?.choices[String(id)]?.[String(eff)] : undefined;
    return (pick ? [pick] : n[4] ?? []).map((s) => s.replace(/\s*\n\s*/g, " "));
  };
  const toggleOpen = (id: number) => {
    setOpenNode((o) => (o === id ? null : id));
    setFind((f) => ({ id, n: (f?.n ?? 0) + 1 }));
  };
  const row = (w: NodeWork, weak = false) => (
    <div key={w.node} className={`ap-work ${weak ? "weak" : ""} ${openNode === w.node ? "open" : ""}`}>
      <button className="ap-work-name" onClick={() => toggleOpen(w.node)} aria-expanded={openNode === w.node} title="What it does (and find it on the tree)">
        <b>{w.name}</b><span className="ap-caret" aria-hidden="true">{openNode === w.node ? "▾" : "▸"}</span>
      </button>
      <span className="ap-acts">
        <button className={`ap-mini lock ${lock.includes(w.node) ? "on" : ""}`} onClick={() => always(w.node)} title="Always keep this in">Always</button>
        <button className={`ap-mini block ${block.includes(w.node) ? "on" : ""}`} onClick={() => never(w.node)} title="Leave this out and build around it">Never</button>
      </span>
      <span className="ap-why">{w.why}</span>
      {openNode === w.node && (
        <div className="ap-does">
          {does(w.node).map((l, i) => <div key={i}>{l}</div>)}
        </div>
      )}
    </div>
  );

  return (
    <>
      <div className="ap-step">2 · How <button className="link small" onClick={onClear}>Clear</button></div>
      <div className="row ap-how">
        <div className="ap-seg">
          {PRIORITIES.map(([p, l]) => (
            <button key={p} className={prio === p ? "on" : ""} onClick={() => setPrio(p)}>{l}{plans[p] ? ` · ${plans[p]!.points}` : ""}</button>
          ))}
        </div>
        {plan && <span className="muted small">{plan.trees_used ? `Learned from ${plan.trees_used} creator tree${plan.trees_used > 1 ? "s" : ""} for ${label}` : "No creator trees for these: built from what the nodes say"}</span>}
        <span className="grow" />
        <button className="primary" disabled={!plan || saving} onClick={save}>{saving ? "Saving…" : "Save as my strategy"}</button>
      </div>
      {(lock.length > 0 || block.length > 0) && (
        <div className="ap-pins">
          {lock.map((id) => <button key={`l${id}`} className="ap-pin lock" onClick={() => always(id)} title="Remove">✓ {names[id] || id} ✕</button>)}
          {block.map((id) => <button key={`b${id}`} className="ap-pin block" onClick={() => never(id)} title="Remove">⦸ {names[id] || id} ✕</button>)}
        </div>
      )}
      {err && <div className="small at-msg">{err}</div>}
      {plan && <>
        <div className="ap-step">3 · Your tree · {plan.points} of {plan.budget} points</div>
        <div className="ap-own">
          <div className="ap-own-tree"><AtlasTreeView key={plan.nodes.length + plan.nodes.slice(-1).join()} nodes={plan.nodes} choices={plan.choices} height={620} find={find} /></div>
          <div className="ap-own-side">
            <b>Doing the most</b>
            <div className="muted small">Always = keep it in. Never = leave it out and build around it.</div>
            {plan.top.map((w) => row(w))}
            {plan.weak.length > 0 && <>
              <b className="ap-weak-h">Doing little</b>
              <div className="muted small">Only taken because a path runs through them.</div>
              {plan.weak.map((w) => row(w, true))}
            </>}
          </div>
        </div>
        <AtlasTotals nodes={plan.nodes} choices={plan.choices} />
      </>}
    </>
  );
}
