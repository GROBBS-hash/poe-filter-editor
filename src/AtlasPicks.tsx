import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import type { AtlasStore } from "./AtlasTreeView";
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
];
const ITEM: Record<string, string> = Object.fromEntries(MECHS.map(([k, , item]) => [k, item]));
const LABEL: Record<string, string> = Object.fromEntries(MECHS.map(([k, label]) => [k, label]));
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

  const fit = (p: Pick) => sel.filter((m) => p.mechanics.includes(m)).length;
  const shown = (sel.length === 0 ? picks.filter((p) => p.group === "maxroll") : picks.filter((p) => fit(p) > 0))
    .slice().sort((a, b) => fit(b) - fit(a) || (a.group === b.group ? 0 : a.group === "maxroll" ? -1 : 1) || b.updated.localeCompare(a.updated));
  const list = all ? shown : shown.slice(0, 5);
  const saved = (id: string) => store.list.find((s) => s.link.includes(id));
  const toggle = (m: string) => setSel((s) => s.includes(m) ? s.filter((x) => x !== m) : [...s, m]);
  const names = sel.map((m) => LABEL[m] ?? m);

  const use = async (p: Pick) => {
    setBusy(p.id); setErr("");
    try {
      const s = await invoke<AtlasStore>("atlas_import", { link: link(p.id), name: "" });
      onImported(s, `Saved your copy of “${p.name}” by ${p.author}. It's the one shown in game now.`);
    } catch (e) { setErr(String(e)); }
    setBusy(null);
  };

  return (
    <div className="card ap">
      <h2>Build an atlas from strategies</h2>
      <p className="muted small">Pick what you want to farm. Trees from maxroll creators for this league{built ? ` (list from ${new Date(built + "T12:00:00").toLocaleDateString(undefined, { day: "numeric", month: "short" })})` : ""}.</p>
      <div className="ap-step">1 · What do you want to farm?</div>
      <div className="ap-tiles">
        {MECHS.map(([m, label]) => {
          const n = count[m] ?? 0;
          return (
            <button key={m} className={`ap-tile ${sel.includes(m) ? "on" : ""}`} disabled={n === 0 && !sel.includes(m)}
              onClick={() => toggle(m)} title={n ? `${n} tree${n > 1 ? "s" : ""}` : "No creator tree for this yet"}>
              <ItemIcon name={ITEM[m]} size={40} />
              <span>{label}</span>
            </button>
          );
        })}
      </div>
      <div className="ap-step">
        {sel.length === 0 ? "2 · Maxroll team picks (choose above to narrow it down)" : `2 · Creator trees for ${names.join(" + ")}`}
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
            {sel.length > 1 && <span className={`ap-fit ${n === sel.length ? "" : "part"}`}>{n === sel.length ? (sel.length === 2 ? "✓ Both" : "✓ All") : `${n} of ${sel.length}`}</span>}
            <button className="link small" onClick={() => invoke("open_url", { url: link(p.id) })}>maxroll</button>
            {mine
              ? <button onClick={() => onShow(mine.id)} title="You already saved a copy">In My strategies</button>
              : <button className="primary" disabled={busy !== null} onClick={() => use(p)}>{busy === p.id ? "Reading…" : "Use this tree"}</button>}
          </div>
        );
      })}
      {shown.length > list.length && <button className="link small" onClick={() => setAll(true)}>Show {shown.length - list.length} more</button>}
      {err && <div className="small at-msg">{err}</div>}
    </div>
  );
}
