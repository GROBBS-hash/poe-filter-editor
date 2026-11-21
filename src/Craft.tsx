// Crafting page: pick a base (or bring an item from Ctrl+D), see every mod that can roll on it with how often a
// Chaos Orb gives it, and click a mod for the ways to get it (essence, bench, fossils, Chaos Orbs) with odds and cost.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { friendly } from "./errors";
import { usePersist } from "./usePersist";
import { CurrencyIcon } from "./icons";

type Tier = { tier: number; name: string; lvl: number; text: string; weight: number; ok: boolean };
type EssenceWay = { name: string; text: string; like_tier: number | null; price: number | null; corrupt: boolean };
type BenchWay = { text: string; cost: [string, number][] };
type Family = { key: string; affix: "prefix" | "suffix"; text: string; tags: string[]; tiers: Tier[]; chaos: number; have: number | null;
  essences: EssenceWay[]; bench: BenchWay[]; fossils_more: string[]; fossils_none: string[] };
type Pool = { base: string; class: string; ilvl: number; influences: string[]; families: Family[] };
type Way = { method: string; chance: number; cost_each: number | null; average: number | null; note: string };
type Start = { base: string; ilvl: number; influences: string[]; have: string[]; name: string };

const INFLUENCES = ["Shaper", "Elder", "Crusader", "Redeemer", "Hunter", "Warlord"];
/** Shared with the Ctrl+D window: the item its "Craft" button sends here. */
export const CRAFT_ITEM_KEY = "craft.item";

/** "1 in 12" / "always" / "never". */
function odds(p: number) {
  if (p >= 0.995) return "always";
  if (p <= 0) return "can't roll";
  const n = 1 / p;
  return n < 1.5 ? `${Math.round(p * 100)}%` : `1 in ${n >= 1000 ? Math.round(n).toLocaleString() : +n.toFixed(n < 10 ? 1 : 0)}`;
}
function chaos(c: number | null) {
  if (c == null) return <span className="muted">price unknown</span>;
  return <>{c >= 100 ? Math.round(c).toLocaleString() : +c.toFixed(1)}<CurrencyIcon id="chaos" /></>;
}

export default function Craft() {
  const [bases, setBases] = useState<string[]>([]);
  const [base, setBase] = usePersist("craft.base", "Eternal Burgonet");
  const [ilvl, setIlvl] = usePersist("craft.ilvl", 84);
  const [infl, setInfl] = usePersist<string[]>("craft.infl", []);
  const [have, setHave] = usePersist<string[]>("craft.have", []);
  const [itemName, setItemName] = usePersist("craft.name", "");
  const [itemText, setItemText] = usePersist("craft.text", "");
  const [pool, setPool] = useState<Pool | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [q, setQ] = useState("");
  const [pick, setPick] = useState<string | null>(null);
  const [paste, setPaste] = useState(false);
  const [extras, setExtras] = usePersist("craft.extras", false);

  useEffect(() => { invoke<string[]>("craft_bases").then(setBases).catch(() => {}); }, []);

  /** Loads a copied item: its base, item level, influences and mods. */
  const fromText = async (text: string) => {
    const s = await invoke<Start | null>("craft_start", { text }).catch(() => null);
    if (!s) { setErr("That doesn't look like gear you can craft on (armour, weapons, jewellery, quivers, shields)."); return; }
    setErr(""); setBase(s.base); setIlvl(s.ilvl); setInfl(s.influences); setHave(s.have); setItemName(s.name); setItemText(text); setPick(null); setPaste(false);
  };
  // An item sent from the Ctrl+D window (now, or while this page was closed).
  useEffect(() => {
    const take = () => {
      let t: string | null = null;
      try { t = localStorage.getItem(CRAFT_ITEM_KEY); localStorage.removeItem(CRAFT_ITEM_KEY); } catch { /* none */ }
      if (t) fromText(t);
    };
    take();
    const on = (e: StorageEvent) => { if (e.key === CRAFT_ITEM_KEY && e.newValue) take(); };
    window.addEventListener("storage", on);
    window.addEventListener("focus", take);
    return () => { window.removeEventListener("storage", on); window.removeEventListener("focus", take); };
  }, []);

  useEffect(() => {
    if (!bases.length || !bases.includes(base)) { setPool(null); return; }
    let live = true;
    setBusy(true);
    invoke<Pool | null>("craft_pool", { base, ilvl: Math.max(1, Math.min(100, ilvl || 1)), influences: infl, have })
      .then((p) => { if (live) { setPool(p); setErr(""); } })
      .catch((e) => live && setErr(friendly(e)))
      .finally(() => live && setBusy(false));
    return () => { live = false; };
  }, [base, ilvl, infl.join(","), have.join("|"), bases.length]);

  const shown = useMemo(() => {
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    return (pool?.families ?? []).filter((f) => words.every((w) => (f.text + " " + f.tags.join(" ")).toLowerCase().includes(w)));
  }, [pool, q]);
  const picked = pool?.families.find((f) => f.key === pick) ?? null;

  const clearItem = () => { setHave([]); setItemName(""); setItemText(""); };
  const toggleInfl = (i: string) => setInfl(infl.includes(i) ? infl.filter((x) => x !== i) : [...infl, i].slice(-2));

  return (
    <div className="page cr">
      <div className="card">
        <div className="row cr-top">
          <label className="cr-field">Base
            <input list="cr-bases" value={base} onChange={(e) => { setBase(e.target.value); clearItem(); setPick(null); }} placeholder="Eternal Burgonet" />
            <datalist id="cr-bases">{bases.map((b) => <option key={b} value={b} />)}</datalist>
          </label>
          <label className="cr-field cr-ilvl">Item level
            <input type="number" min={1} max={100} value={ilvl} onChange={(e) => setIlvl(+e.target.value)} />
          </label>
          <div className="cr-field">Influence
            <div className="row cr-infl">{INFLUENCES.map((i) => <button key={i} className={`chip ${infl.includes(i) ? "on" : ""}`} onClick={() => toggleInfl(i)}>{i}</button>)}</div>
          </div>
        </div>
        <div className="row">
          <button onClick={async () => { try { const t = await navigator.clipboard.readText(); if (t.includes("Item Class:")) { fromText(t); return; } } catch { /* fall back */ } setPaste(true); }}>
            Use a copied item</button>
          <span className="muted small">In game: hover an item and press Ctrl+C, or press Ctrl+D and click "Craft this".</span>
          <button className="link small cr-coe" onClick={async () => {
            if (itemText) { try { await navigator.clipboard.writeText(itemText); } catch { /* still open it */ } }
            invoke("open_url", { url: "https://www.craftofexile.com/" });
          }} title="For step-by-step plans: Craft of Exile's simulator">Open in Craft of Exile{itemText ? " (item copied: click Import item, then paste)" : ""}</button>
        </div>
        {paste && <textarea className="text cr-paste" autoFocus placeholder="Paste the item here (Ctrl+V)" onChange={(e) => e.target.value.includes("Item Class:") && fromText(e.target.value)} />}
        {itemName && <div className="cr-item small">Crafting on <b>{itemName}</b> · {have.length} mods read from it <button className="link small" onClick={clearItem}>forget the item</button></div>}
        {err && <p className="danger-text small">{err}</p>}
      </div>

      <div className="cr-body">
        <div className="card cr-list">
          <div className="row">
            <input className="text cr-search" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Find a mod: life, cold res, spell…" />
            {busy && <span className="muted small">Rolling…</span>}
          </div>
          {pool && <button className="link small" onClick={() => setExtras(!extras)}>{extras ? "Hide" : "Show"} bench and essence only mods</button>}
          {pool && <p className="muted small">{pool.class} · item level {pool.ilvl}{pool.influences.length ? ` · ${pool.influences.join(" + ")}` : ""}. The number is how often one Chaos Orb gives that mod (any tier).</p>}
          {(["prefix", "suffix"] as const).map((a) => {
            const all = shown.filter((f) => f.affix === a);
            const natural = all.filter((f) => f.tiers.length > 0);
            // Bench- and essence-only mods are folded away unless asked for (or searched for).
            const list = extras || q ? all : natural;
            return (
              <div key={a}>
                <h3>{a === "prefix" ? "Prefixes" : "Suffixes"} <span className="muted small">({natural.length} can roll{all.length > natural.length ? `, ${all.length - natural.length} only from the bench or an essence` : ""})</span></h3>
                {list.map((f) => (
                  <button key={f.key + f.text} className={`cr-mod ${pick === f.key ? "on" : ""} ${f.tiers.length ? "" : "cr-special"}`} onClick={() => setPick(f.key)}>
                    <span className="cr-text">{f.text.split("\n").map((l, i) => <span key={i}>{l}</span>)}</span>
                    <span className="cr-meta">
                      {f.have != null && <span className="tag cr-have">yours{f.have ? ` T${f.have}` : ""}</span>}
                      {f.essences.length > 0 && <span className="cr-pill">essence</span>}
                      {f.bench.length > 0 && <span className="cr-pill">bench</span>}
                      <span className="cr-odds">{f.tiers.length ? odds(f.chaos) : f.bench.length ? "bench only" : "essence only"}</span>
                    </span>
                  </button>
                ))}
              </div>
            );
          })}
        </div>
        <div className="cr-side">
          {picked ? <Detail key={picked.key} fam={picked} base={base} ilvl={ilvl} infl={infl} /> :
            <div className="card muted">Click a mod to see its tiers and the cheapest ways to get it.</div>}
        </div>
      </div>
    </div>
  );
}

function Detail({ fam, base, ilvl, infl }: { fam: Family; base: string; ilvl: number; infl: string[] }) {
  const best = fam.tiers.find((t) => t.ok)?.tier ?? 1;
  const [tier, setTier] = useState(Math.min(fam.tiers.length || 1, best + 1));
  const [ways, setWays] = useState<Way[] | null>(null);
  const [err, setErr] = useState("");
  useEffect(() => {
    let live = true;
    setWays(null);
    invoke<Way[]>("craft_ways", { base, ilvl, influences: infl, key: fam.key, minTier: tier })
      .then((w) => live && setWays(w)).catch((e) => live && setErr(friendly(e)));
    return () => { live = false; };
  }, [fam.key, tier, base, ilvl, infl.join(",")]);
  return (
    <div className="card cr-detail">
      <h2>{fam.text.split("\n").map((l, i) => <div key={i}>{l}</div>)}</h2>
      <div className="muted small">{fam.affix === "prefix" ? "Prefix" : "Suffix"}{fam.tags.length ? ` · ${fam.tags.join(", ")}` : ""}</div>

      {fam.tiers.length > 0 && <>
        <h3>Tiers on this base</h3>
        <div className="cr-tiers">
          {fam.tiers.map((t) => (
            <button key={t.name} className={`cr-tier ${t.tier <= tier ? "want" : ""} ${t.ok ? "" : "locked"}`} onClick={() => setTier(t.tier)}
              title={t.ok ? `Click: aim for tier ${t.tier} or better` : `Needs item level ${t.lvl}`}>
              <b>T{t.tier}</b><span>{t.text.split("\n").join(" / ")}</span><span className="muted">{t.ok ? `ilvl ${t.lvl}` : `needs ilvl ${t.lvl}`}</span>
            </button>
          ))}
        </div>
        <p className="muted small">Aiming for tier {tier} or better (click a tier to change).</p>
      </>}

      <h3>Ways to get it</h3>
      {err && <p className="danger-text small">{err}</p>}
      {!ways && !err && <p className="muted small">Rolling the item a few thousand times…</p>}
      {ways && ways.length === 0 && <p className="muted small">No simple way found: try Craft of Exile for harvest, veiled or influenced crafts.</p>}
      {ways && ways.map((w) => (
        <div key={w.method} className="cr-way">
          <div className="cr-way-top"><b>{w.method}</b><span className="cr-odds">{odds(w.chance)}</span></div>
          <div className="small">
            {w.chance < 0.995 && <>Each try {chaos(w.cost_each)} · on average {chaos(w.average)}</>}
            {w.chance >= 0.995 && <>Costs {chaos(w.cost_each)}</>}
          </div>
          <div className="muted small">{w.note}</div>
        </div>
      ))}

      {fam.essences.length > 0 && <>
        <h3>Essences</h3>
        {fam.essences.map((e) => <div key={e.name} className="small cr-line"><b>{e.name}</b> → {e.text}{e.like_tier ? <span className="muted"> (like T{e.like_tier})</span> : null} · {chaos(e.price)}</div>)}
      </>}
      {fam.bench.length > 0 && <>
        <h3>Crafting bench</h3>
        {fam.bench.slice(0, 4).map((b) => <div key={b.text} className="small cr-line">{b.text.split("\n").join(" / ")} <span className="muted">· {b.cost.map(([c, n]) => `${n} × ${c}`).join(", ")}</span></div>)}
      </>}
      {(fam.fossils_more.length > 0 || fam.fossils_none.length > 0) && <>
        <h3>Fossils</h3>
        {fam.fossils_more.length > 0 && <div className="small cr-line">More likely with: <b>{fam.fossils_more.join(", ")}</b></div>}
        {fam.fossils_none.length > 0 && <div className="small cr-line">Never with: {fam.fossils_none.join(", ")}</div>}
      </>}
      <p className="muted small">Odds are worked out by rolling the item here thousands of times, a rough guide. For multi-step plans use Craft of Exile.</p>
    </div>
  );
}
