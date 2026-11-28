import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import AtlasTreeView, { type AtlasData, type AtlasStep, type AtlasStore, loadAtlas } from "./AtlasTreeView";
import AtlasTotals from "./AtlasTotals";
import { ItemIcon } from "./icons";
import { MechArt, hasMechArt } from "./mechArt";

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
  ["divination cards", "Div cards", "Stacked Deck"], ["mirage", "Mirage", "Coin of Restoration"],
];
/** Only on "Build my own" (no creator tree for them, but the optimiser knows their nodes). */
const OWN_ONLY = ["divination cards", "mirage"];
/** "Also spend points on": all-round travel nodes for Build my own (he asked 23 Nov), keys as in core atlas_opt TRAVEL. */
const TRAVEL: [string, string, string][] = [
  ["pack size", "Pack size", "Cartographer's Chisel"], ["explicit effect", "Map mod effect", "Orb of Alchemy"],
  ["threads", "Threads found", "Frayed Thread"], ["maps found", "Maps found", "Map (Tier 16)"], ["rarity", "Item rarity", "Exalted Orb"],
];
/** Goals that aren't a league mechanic (core atlas_model.json), with the picture on their tile. */
const GOALS: [string, string, string, string][] = [
  ["currency", "Raw currency", "Divine Orb", "Rare monsters, altars and map bonuses: currency off the floor"],
  ["rush", "Boss rush", "The Maven's Writ", "Run to the boss, kill it, leave"],
  ["terror", "Many bosses: Terror Shrines", "Fragment of the Minotaur", "Terror Shrine maps: every shrine is a boss, about 15 a map"],
  ["destructive", "Many bosses: Destructive Play", "Fragment of the Hydra", "The Maven adds bosses when most of the map is dead: boss maps and chisels"],
  ["maps", "Map drops", "Map (Tier 16)", "Clear everything, sell the maps"],
  ["threads", "Threads", "Legion Thread", "Find and sell Threads"],
];
type Setup = { map: string; build: string; altar: string; has: string[]; threads: [string, number][] };
const SETUP: Setup = { map: "rolled", build: "normal", altar: "auto", has: [], threads: [] };
const MAPS: [string, string, string][] = [["plain", "Plain", "White or quickly rolled maps"], ["rolled", "Rolled", "6 mods, about 90% Atlas Drops"], ["juiced", "Juiced", "8 mods, 120%+ Atlas Drops and more currency"]];
const BUILDS: [string, string, string][] = [["weak", "Fragile", "Leaves out points that make maps deadly"], ["normal", "Normal", ""], ["strong", "Strong", "Dangerous points cost you nothing"]];
const ALTARS: [string, string, string][] = [["auto", "Best", "Whichever pays more"], ["eater", "Eater", "Eater of Worlds altars"], ["exarch", "Exarch", "Searing Exarch altars"], ["maven", "Maven", "No altars: the Maven watches"], ["none", "None", "No influence"]];
const HAS: [string, string][] = [["mirage", "Mirage"], ["delirium", "Delirium mirror"], ["wildwood", "Wildwood"]];
const PRIMARY = ["abyss", "bestiary", "betrayal", "blight", "breach", "expedition", "incursion", "legion", "ritual", "ultimatum"];
const SURE: Record<string, [string, string]> = {
  both: ["Solid", "The maths and what players take agree"],
  maths: ["Maths", "Pays by the maths, fewer players take it: a good one to test"],
  players: ["Players", "Most players take it; the maths has no number for it yet"],
};
const TRAVEL_KEYS = TRAVEL.map(([k]) => k);
const ITEM: Record<string, string> = Object.fromEntries([...MECHS, ...TRAVEL].map(([k, , item]) => [k, item]));
const LABEL: Record<string, string> = Object.fromEntries([...MECHS, ...TRAVEL].map(([k, label]) => [k, label]));
const link = (id: string) => `https://maxroll.gg/poe/farming-strategy-planner/${id}`;

function ago(day: string) {
  const n = Math.round((Date.now() - new Date(day + "T12:00:00").getTime()) / 864e5);
  return n <= 0 ? "today" : n === 1 ? "yesterday" : n < 60 ? `${n} days ago` : `on ${day}`;
}

function MechIcon({ m, size }: { m: string; size: number }) {
  return <span className="ap-mi" title={LABEL[m] ?? m}>{hasMechArt(m) ? <MechArt m={m} size={size} /> : <ItemIcon name={ITEM[m]} size={size} />}</span>;
}

function readSel(): string[] {
  try { return JSON.parse(localStorage.getItem("atlas-picks-sel") ?? "[]"); } catch { return []; }
}

/** Atlas page, top card: pick what you want to farm, see creators' trees that fit (best matches first),
 *  and "Use this tree" saves your own copy under My trees (the creator's original stays on maxroll). */
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
  const [goals, setGoals] = useState<string[]>(() => { try { return JSON.parse(localStorage.getItem("atlas-opt-goals") ?? "[]"); } catch { return []; } });
  useEffect(() => { try { localStorage.setItem("atlas-opt-goals", JSON.stringify(goals)); } catch { /* fine */ } }, [goals]);
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
        ? "Pick a goal, something to farm, or both. The editor works out what every point does for it, how points multiply each other, and what experts take."
        : <>Pick what you want to farm. Trees from maxroll creators for this league{built ? ` (list from ${new Date(built + "T12:00:00").toLocaleDateString(undefined, { day: "numeric", month: "short" })})` : ""}.</>}</p>
      {own && <>
        <div className="ap-step">1 · What's your goal? <span className="ap-step-note">(optional)</span></div>
        <div className="ap-tiles ap-goals">
          {GOALS.map(([k, label, item, what]) => (
            <button key={k} className={`ap-tile ${goals.includes(k) ? "on" : ""}`} aria-pressed={goals.includes(k)} title={what}
              onClick={() => setGoals((g) => g.includes(k) ? [] : [k])}>
              {k === "terror" ? <MechArt m="shrines" size={34} /> : <ItemIcon name={item} size={34} />}<span>{label}</span>
            </button>
          ))}
        </div>
      </>}
      <div className="ap-step">{own ? "2 · What do you want to farm?" : "1 · What do you want to farm?"}</div>
      <div className="ap-tiles">
        {MECHS.filter(([m]) => own || !OWN_ONLY.includes(m)).map(([m, label]) => {
          const n = count[m] ?? 0;
          return (
            <button key={m} className={`ap-tile ${sel.includes(m) ? "on" : ""}`} disabled={!own && n === 0 && !sel.includes(m)}
              onClick={() => toggle(m)} title={own ? label : n ? `${n} tree${n > 1 ? "s" : ""}` : "No creator tree for this yet"}>
              {hasMechArt(m) ? <MechArt m={m} size={44} /> : <ItemIcon name={ITEM[m]} size={40} />}
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
      {own ? <BuildOwn sel={sel} goals={goals} onClear={() => { setSel([]); setGoals([]); }} onSaved={onImported} /> : <>
      <div className="ap-step">
        {ms.length === 0 ? "2 · Maxroll team picks (choose above to narrow it down)" : `2 · Creator trees for ${names.join(" + ")}`}
        {sel.length > 0 && <button className="link small" onClick={() => setSel([])}>Clear</button>}
      </div>
      {list.length === 0 && <div className="muted small ap-none">No creator tree has these yet. Try fewer, or paste a link below.</div>}
      {list.map((p, i) => {
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
              ? <button onClick={() => onShow(mine.id)} title="You already saved a copy">In My trees</button>
              : <button className={i === 0 ? "primary" : ""} disabled={busy !== null} onClick={() => use(p)}>{busy === p.id ? "Reading…" : "Use this tree"}</button>}
          </div>
        );
      })}
      {shown.length > list.length && <button className="link small" onClick={() => setAll(true)}>Show {shown.length - list.length} more</button>}
      {err && <div className="small at-msg">{err}</div>}
      </>}
    </div>
  );
}

type NodeWork = { node: number; name: string; value: number; why: string; gain?: number; share?: number; sure?: string; kept?: number };
type Plan = { nodes: number[]; steps: AtlasStep[]; choices: [number, number][]; points: number; budget: number;
  top: NodeWork[]; weak: NodeWork[]; trees_used: number; notes?: string[]; left_out?: [string, string][]; tips?: string[];
  threads?: [string, number][]; altar?: string; parts?: { key: string; label: string; share: number }[] };
type Priority = "fewest" | "balanced" | "profit";
const PRIORITIES: [Priority, string][] = [["fewest", "Fewest points"], ["balanced", "Balanced"], ["profit", "Most profit"]];

/** One question with a few answers side by side. */
function Pickers({ label, list, value, onPick }: { label: string; list: [string, string, string][]; value: string; onPick: (v: string) => void }) {
  return (
    <div className="ap-set">
      <span className="ap-set-l">{label}</span>
      <div className="ap-seg">{list.map(([k, l, what]) => <button key={k} className={value === k ? "on" : ""} aria-pressed={value === k} title={what || l} onClick={() => onPick(k)}>{l}</button>)}</div>
    </div>
  );
}

function readList(key: string): number[] {
  try { const v = JSON.parse(localStorage.getItem(key) ?? "[]"); return Array.isArray(v) ? v : []; } catch { return []; }
}
function saveList(key: string, v: unknown) { try { localStorage.setItem(key, JSON.stringify(v)); } catch { /* fine */ } }

/** "Build my own": the optimiser (core atlas_opt.rs) for the picked mechanics. Always / Never on any node it
 *  shows; the tree updates straight away. Save adds it to My trees like a creator tree. */
function BuildOwn({ sel, goals, onClear, onSaved }: { sel: string[]; goals: string[]; onClear: () => void; onSaved: (s: AtlasStore, msg: string) => void }) {
  const [prio, setPrio] = useState<Priority>(() => { try { return (localStorage.getItem("atlas-opt-prio") as Priority) || "balanced"; } catch { return "balanced"; } });
  const [lock, setLock] = useState<number[]>(() => readList("atlas-opt-lock"));
  const [block, setBlock] = useState<number[]>(() => readList("atlas-opt-block"));
  const [setup, setSetup] = useState<Setup>(() => { try { return { ...SETUP, ...JSON.parse(localStorage.getItem("atlas-opt-setup") ?? "{}") }; } catch { return SETUP; } });
  useEffect(() => { saveList("atlas-opt-setup", setup); }, [setup]);
  const change = (p: Partial<Setup>) => setSetup((x) => ({ ...x, ...p }));
  const ask = (p: Priority) => ({ mechanics: sel, goals, priority: p, budget: 0, lock, block, threads: setup.threads, altar: setup.altar, map_has: setup.has, map: setup.map, build: setup.build });
  const [plans, setPlans] = useState<Partial<Record<Priority, Plan>>>({});
  const [err, setErr] = useState("");
  const [saving, setSaving] = useState(false);
  const [tree, setTree] = useState<AtlasData | null>(null);
  const names = useMemo(() => {
    const m: Record<number, string> = {};
    for (const [id, n] of Object.entries(tree?.nodes ?? {})) m[Number(id)] = n[3];
    return m;
  }, [tree]);
  // The row opened in "Most important points" (shows what the node does), and the tree zooming to it.
  const [openNode, setOpenNode] = useState<number | null>(null);
  const [find, setFind] = useState<{ id: number; n: number } | null>(null);
  // A point clicked on the tree: pinned at the top of the side panel with Always / Never (he asked 24 Nov).
  const [clicked, setClicked] = useState<number | null>(null);

  useEffect(() => { try { localStorage.setItem("atlas-opt-prio", prio); } catch { /* fine */ } }, [prio]);
  useEffect(() => { saveList("atlas-opt-lock", lock); saveList("atlas-opt-block", block); }, [lock, block]);
  // The atlas data: names for the Always / Never chips (a blocked node isn't in the plan any more) and what each node does.
  useEffect(() => { loadAtlas().then(setTree).catch(() => setTree(null)); }, []);
  useEffect(() => {
    if (sel.length === 0 && goals.length === 0) { setPlans({}); return; }
    let live = true;
    const t = window.setTimeout(async () => {
      try {
        const out: Partial<Record<Priority, Plan>> = {};
        await Promise.all(PRIORITIES.map(async ([p]) => { out[p] = await invoke<Plan>("atlas_optimise", { ask: ask(p) }); }));
        if (live) { setPlans(out); setErr(""); }
      } catch (e) { if (live) setErr(String(e)); }
    }, 120);
    return () => { live = false; window.clearTimeout(t); };
  }, [sel.join(","), goals.join(","), lock.join(","), block.join(","), JSON.stringify(setup)]);

  // Threads picked for another farm don't follow him to the next one.
  const first = useRef(true);
  useEffect(() => { if (first.current) { first.current = false; return; } setSetup((x) => x.threads.length ? { ...x, threads: [] } : x); }, [sel.join(","), goals.join(",")]);

  if (sel.length === 0 && goals.length === 0) return <div className="muted small ap-none">Pick a goal or something to farm above to build a tree.</div>;
  const plan = plans[prio];
  const label = [...goals.map((g) => GOALS.find((x) => x[0] === g)?.[1] ?? g), ...sel.map((m) => LABEL[m] ?? m)].join(" + ");
  const always = (id: number) => { setBlock((b) => b.filter((x) => x !== id)); setLock((l) => l.includes(id) ? l.filter((x) => x !== id) : [...l, id]); };
  const never = (id: number) => { setLock((l) => l.filter((x) => x !== id)); setBlock((b) => b.includes(id) ? b.filter((x) => x !== id) : [...b, id]); };
  const save = async () => {
    setSaving(true);
    try {
      const s = await invoke<AtlasStore>("atlas_save_plan", { ask: ask(prio), name: "" });
      onSaved(s, `Saved “${label}” (${plan?.points ?? ""} points) to My trees. It's the one shown in game now.`);
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
  const pickOnTree = (id: number) => {
    setClicked(id);
    const listed = plan?.top.some((w) => w.node === id) || plan?.weak.some((w) => w.node === id);
    setOpenNode(listed ? id : null);
    requestAnimationFrame(() => {
      document.querySelector(".ap-own-side")?.scrollTo({ top: 0, behavior: "smooth" });
    });
  };
  const row = (w: NodeWork, weak = false) => (
    <div key={w.node} id={`ap-row-${w.node}`} className={`ap-work ${weak ? "weak" : ""} ${openNode === w.node ? "open" : ""}`}>
      <button className="ap-work-name" onClick={() => toggleOpen(w.node)} aria-expanded={openNode === w.node} title="What it does (and find it on the tree)">
        <b>{w.name}</b>{(w.gain ?? 0) >= 0.004 && <span className="ap-gain">+{(w.gain! * 100).toFixed(w.gain! < 0.1 ? 1 : 0)}%</span>}
        {w.sure && SURE[w.sure] && <span className={`ap-sure ${w.sure}`} title={SURE[w.sure][1]}>{SURE[w.sure][0]}</span>}
        {w.kept != null && w.kept < 0.67 && <span className="ap-sure close" title={`A close call: with the model's guessed numbers nudged up or down a little, it stayed in ${Math.round(w.kept * 6)} of 6 trees. Worth testing in your own maps.`}>Close call</span>}<span className="ap-caret" aria-hidden="true">{openNode === w.node ? "▾" : "▸"}</span>
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
      <div className="ap-step">3 · Your maps</div>
      <div className="ap-setup">
        <Pickers label="Maps" list={MAPS} value={setup.map} onPick={(map) => change({ map })} />
        <Pickers label="Your build" list={BUILDS} value={setup.build} onPick={(build) => change({ build })} />
        <Pickers label="Altars" list={ALTARS} value={setup.altar} onPick={(altar) => change({ altar })} />
        <div className="ap-set">
          <span className="ap-set-l" title="Bought or rolled that way: points for its chance are worth nothing then">Every map has</span>
          <span className="ap-chips">{HAS.map(([k, l]) => (
            <button key={k} className={`ap-chip ${setup.has.includes(k) ? "on" : ""}`} aria-pressed={setup.has.includes(k)}
              onClick={() => change({ has: setup.has.includes(k) ? setup.has.filter((x) => x !== k) : [...setup.has, k] })}>{hasMechArt(k) && <MechArt m={k} size={20} />}{l}</button>
          ))}</span>
        </div>
        <div className="ap-set">
          <span className="ap-set-l" title="A map takes three kinds. Only 1, 3, 6 and 8 of a kind pay">Threads</span>
          <span className="ap-chips">
            {(plan?.threads ?? []).map(([t, n], i) => (
              <span key={i} className="ap-chip on">
                <MechArt m={t} size={20} />
                <select aria-label="Thread" value={t} onChange={(e) => change({ threads: (plan!.threads!).map((x, j) => j === i ? [e.target.value, x[1]] : x) })}>
                  {PRIMARY.filter((p) => p === t || !plan!.threads!.some((x) => x[0] === p)).map((p) => <option key={p} value={p}>{LABEL[p] ?? p}</option>)}
                </select>
                <select aria-label="How many" value={n} onChange={(e) => change({ threads: (plan!.threads!).map((x, j) => j === i ? [x[0], Number(e.target.value)] : x) })}>
                  {[...new Set([0, 1, 3, 6, 8, n])].sort((a, b) => a - b).map((k) => <option key={k} value={k}>{k || "none"}</option>)}
                </select>
              </span>
            ))}
            {setup.threads.length > 0 && <button className="link small" onClick={() => change({ threads: [] })}>Choose for me</button>}
          </span>
        </div>
      </div>
      <div className="ap-step">4 · How <button className="link small" onClick={onClear}>Clear</button></div>
      <div className="row ap-how">
        <div className="ap-seg">
          {PRIORITIES.map(([p, l]) => (
            <button key={p} className={prio === p ? "on" : ""} onClick={() => setPrio(p)}>{l}{plans[p] ? ` · ${plans[p]!.points}` : ""}</button>
          ))}
        </div>
        {plan && <span className="muted small">{plan.trees_used ? `Checked against about ${plan.trees_used} different tree ideas for these` : "Built from what the points do and what all trees take"}</span>}
        <span className="grow" />
        <button className="primary" disabled={!plan || saving} onClick={save}>{saving ? "Saving…" : "Save to My trees"}</button>
      </div>
      {(lock.length > 0 || block.length > 0) && (
        <div className="ap-pins">
          {lock.map((id) => <button key={`l${id}`} className="ap-pin lock" onClick={() => always(id)} title="Remove">✓ {names[id] || id} ✕</button>)}
          {block.map((id) => <button key={`b${id}`} className="ap-pin block" onClick={() => never(id)} title="Remove">⦸ {names[id] || id} ✕</button>)}
        </div>
      )}
      {err && <div className="small at-msg">{err}</div>}
      {plan && <>
        <div className="ap-step">5 · Your tree · {plan.points} of {plan.budget} points</div>
        {(plan.parts ?? []).length > 0 && (
          <div className="ap-parts" title="Where this plan's income comes from, by the maths">
            <div className="ap-parts-bar">{plan.parts!.map((x, i) => <i key={x.key} className={`c${i % 8}`} style={{ flexGrow: x.share }} />)}</div>
            <div className="ap-parts-key">{plan.parts!.map((x, i) => <span key={x.key}><i className={`c${i % 8}`} />{x.label} {Math.round(x.share * 100)}%</span>)}</div>
          </div>
        )}
        {(plan.notes ?? []).length > 0 && <div className="small ap-notes">{plan.notes!.join(" · ")}.</div>}
        {(plan.tips ?? []).length > 0 && <div className="small ap-tips"><b>How to play it:</b> {plan.tips!.join(" ")}</div>}
        {(plan.left_out ?? []).length > 0 && <div className="small ap-left"><b>Left out on purpose:</b> {plan.left_out!.map(([n, w]) => `${n} (${w})`).join(" · ")}</div>}
        <div className="ap-own">
          <div className="ap-own-tree"><AtlasTreeView key={[...goals, ...sel].join(",")} nodes={plan.nodes} choices={plan.choices} height={620} find={find} onPick={pickOnTree} /></div>
          <div className="ap-own-side">
            {clicked != null && names[clicked] !== undefined && (
              <div className="ap-clicked">
                <div className="row-between">
                  <b>{names[clicked] || "Passive"}</b>
                  <button className="link small" onClick={() => setClicked(null)} aria-label="Close">✕</button>
                </div>
                <div className="muted small">{plan.nodes.includes(clicked) ? "In your tree" : "Not in your tree"}
                  {lock.includes(clicked) ? " · Always" : block.includes(clicked) ? " · Never" : ""}</div>
                <div className="ap-does">{does(clicked).map((l, i) => <div key={i}>{l}</div>)}</div>
                <span className="ap-acts">
                  <button className={`ap-mini lock ${lock.includes(clicked) ? "on" : ""}`} onClick={() => always(clicked)} title="Always keep this in">Always</button>
                  <button className={`ap-mini block ${block.includes(clicked) ? "on" : ""}`} onClick={() => never(clicked)} title="Leave this out and build around it">Never</button>
                </span>
              </div>
            )}
            <b>Most important points</b>
            <div className="muted small">The % is roughly how much worse the plan gets without that point. Click a point, then <b>Always</b> to keep it or <b>Never</b> to plan without it.</div>
            {plan.top.map((w) => row(w))}
            {plan.weak.length > 0 && <>
              <b className="ap-weak-h">Points just for the path</b>
              <div className="muted small">They do little on their own: the plan takes them to reach better points.</div>
              {plan.weak.map((w) => row(w, true))}
            </>}
          </div>
        </div>
        <AtlasTotals nodes={plan.nodes} choices={plan.choices} />
      </>}
    </>
  );
}
