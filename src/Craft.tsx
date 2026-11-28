// Crafting page: pick a base (or bring an item from Ctrl+D), see every mod that can roll on it (influence-only ones
// marked), how often a Chaos Orb gives it, and click a mod for the ways to get it with odds and cost. Add several mods
// to the plan to get the odds of hitting them all at once. Everything fits on one screen: three columns that scroll.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { friendly } from "./errors";
import { usePersist } from "./usePersist";
import { CurrencyIcon, ItemIcon } from "./icons";
import "./craft-recipe.css";

type Tier = { tier: number; name: string; lvl: number; text: string; weight: number; ok: boolean };
type EssenceWay = { name: string; text: string; like_tier: number | null; price: number | null; corrupt: boolean };
type BenchWay = { text: string; cost: [string, number][] };
type Family = { key: string; affix: "prefix" | "suffix"; text: string; tags: string[]; tiers: Tier[]; chaos: number; have: number | null;
  essences: EssenceWay[]; bench: BenchWay[]; fossils_more: string[]; fossils_none: string[]; needs: string | null };
type ItemMod = { text: string; affix: string | null; tier: number | null; key: string | null; needs: string | null; kind: string; how: string | null };
type Cluster = { size: string; enchant: string[]; min: number; max: number };
type Pool = { base: string; class: string; ilvl: number; influences: string[]; families: Family[]; item_mods: ItemMod[]; side: number; jewel: boolean; cluster: Cluster | null };
type Way = { group: string; method: string; chance: number; cost_each: number | null; average: number | null; note: string };
type Start = { base: string; ilvl: number; influences: string[]; have: string[]; name: string; rarity: string; passives: number | null };
type Target = { key: string; tier: number };

const INFLUENCES = ["Shaper", "Elder", "Crusader", "Redeemer", "Hunter", "Warlord"];
const INF_COLOUR: Record<string, string> = { Shaper: "#7fb2ff", Elder: "#c79bff", Crusader: "#ffb15e", Redeemer: "#6fdcd2", Hunter: "#9be07a", Warlord: "#ff7a6b" };
const RARITY_COLOUR: Record<string, string> = { Normal: "#c8c8c8", Magic: "#8888ff", Rare: "#ffff77", Unique: "#af6025" };
/** Shared with the Ctrl+D window: the item its "Craft this" button sends here. */
export const CRAFT_ITEM_KEY = "craft.item";
/** Put in front of the item text when it's an item to make (from your build), not one you own: the page plans every
 *  mod on it and starts from a fresh base. */
export const CRAFT_PLAN_MARK = "[plan]\n";

/** "1 in 12" / "always" / "can't roll". */
function odds(p: number) {
  if (p >= 0.995) return "always";
  if (p <= 0) return "can't roll";
  const n = 1 / p;
  return n < 1.5 ? `${Math.round(p * 100)}%` : `1 in ${n >= 1000 ? Math.round(n).toLocaleString() : +n.toFixed(n < 10 ? 1 : 0)}`;
}
function Chaos({ c }: { c: number | null }) {
  if (c == null) return <span className="muted">price unknown</span>;
  return <span className="cr-price">{c >= 100 ? Math.round(c).toLocaleString() : +c.toFixed(1)}<CurrencyIcon id="chaos" size={14} /></span>;
}
function InfIcon({ name, size = 16 }: { name: string; size?: number }) {
  return <span className="cr-inf-ico" title={`${name} influence`}><ItemIcon name={`${name}'s Exalted Orb`} size={size} /></span>;
}
/** Pictures for a method: each fossil of a pair, the essence, the orb. */
function MethodIcons({ method }: { method: string }) {
  if (method === "Crafting bench") return <span className="cr-bench-ico" aria-hidden>⚒</span>;
  // Route steps: bench locks and meta-crafts cost Divines; "Bench filler + Exalted Orb" shows the orb.
  if (method.startsWith("Bench: lock") || method.startsWith("Bench: Prefix") || method.startsWith("Bench: Suffix") || method === "Bench: Can have up to 3 Crafted Modifiers")
    return <span className="cr-icons"><ItemIcon name="Divine Orb" size={22} /></span>;
  const cleaned = method.replace(/^Bench: Cannot roll \w+ Modifiers \+ /, "Divine Orb + ").replace(/^Bench filler \+ /, "").replace(/^Lock the \w+ \+ /, "Divine Orb + ")
    .replace(" (both at once)", "");
  const parts = method.startsWith("Harvest") ? ["Wild Crystallised Lifeforce"] : method.startsWith("Chaos Orb on") ? ["Chaos Orb"] : cleaned.split(" + ");
  return <span className="cr-icons">{parts.map((p) => <ItemIcon key={p} name={p} size={22} />)}</span>;
}
const lines = (t: string) => t.split("\n").join(" / ");

export default function Craft() {
  const [bases, setBases] = useState<string[]>([]);
  const [base, setBase] = usePersist("craft.base", "Eternal Burgonet");
  const [ilvl, setIlvl] = usePersist("craft.ilvl", 84);
  const [infl, setInfl] = usePersist<string[]>("craft.infl", []);
  const [have, setHave] = usePersist<string[]>("craft.have", []);
  const [itemName, setItemName] = usePersist("craft.name", "");
  const [itemText, setItemText] = usePersist("craft.text", "");
  const [rarity, setRarity] = usePersist("craft.rarity", "");
  const [plan, setPlan] = usePersist<Target[]>("craft.plan", []);
  const [extras, setExtras] = usePersist("craft.extras", false);
  const [pool, setPool] = useState<Pool | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [q, setQ] = useState("");
  const [pick, setPick] = useState<string | null>(null);
  const [paste, setPaste] = useState(false);
  /** Which non-clickable mod of your item has its "how to get it" note open. */
  const [how, setHow] = useState<number | null>(null);
  /** A copied cluster jewel: how many passives it adds. */
  const [passives, setPassives] = useState<number | null>(null);
  const [showRoutes, setShowRoutes] = useState(false);
  /** An item from your build arrived: plan all its mods once its mods are read, then forget it (you don't own it). */
  const planWanted = useRef(false);

  useEffect(() => { invoke<string[]>("craft_bases").then(setBases).catch(() => {}); }, []);

  const fromText = async (text: string) => {
    const s = await invoke<Start | null>("craft_start", { text }).catch(() => null);
    if (!s) { setErr("That doesn't look like something you can craft on (armour, weapons, jewellery, quivers, shields, jewels, cluster jewels)."); return; }
    // PoB doesn't keep item levels (its copies say 1): an item to make from your build is planned at 86, where every tier rolls.
    setErr(""); setBase(s.base); setIlvl(planWanted.current && s.ilvl < 68 ? 86 : s.ilvl); setInfl(s.influences); setHave(s.have); setRarity(s.rarity); setItemName(s.name); setItemText(text); setPassives(s.passives);
    setPick(null); setPaste(false); setPlan([]);
  };
  // An item sent from the Ctrl+D window (now, or while this page was closed).
  useEffect(() => {
    const take = () => {
      let t: string | null = null;
      try { t = localStorage.getItem(CRAFT_ITEM_KEY); localStorage.removeItem(CRAFT_ITEM_KEY); } catch { /* none */ }
      if (t && t.startsWith(CRAFT_PLAN_MARK)) { planWanted.current = true; t = t.slice(CRAFT_PLAN_MARK.length); }
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
    return (pool?.families ?? []).filter((f) => words.every((w) => (f.text + " " + f.tags.join(" ") + " " + (f.needs ?? "")).toLowerCase().includes(w)));
  }, [pool, q]);
  const fam = (k: string | null) => pool?.families.find((f) => f.key === k) ?? null;
  /** Every mod on the item that can be crafted, at its own tier or better, into the plan. */
  const planAll = (p: Pool) => {
    const seen = new Set<string>();
    setPlan(p.item_mods.filter((m) => m.key && m.kind !== "fractured" && !seen.has(m.key) && seen.add(m.key))
      .map((m) => { const f = p.families.find((x) => x.key === m.key); return { key: m.key!, tier: m.tier ?? Math.min(f?.tiers.length || 1, 2) }; }));
  };
  // From your build: plan it, then start from a fresh base (the plan stays; the item's mods aren't yours yet).
  useEffect(() => {
    if (!planWanted.current || !pool || pool.item_mods.length === 0) return;
    planWanted.current = false;
    planAll(pool);
    clearItem();
  }, [pool]); // eslint-disable-line react-hooks/exhaustive-deps
  const picked = fam(pick);
  const routePlan = useMemo(() => plan.filter((t) => fam(t.key)), [plan, pool]);
  const routes = useRoutes(routePlan, base, ilvl, infl, have);

  const clearItem = () => { setHave([]); setItemName(""); setItemText(""); setRarity(""); setPassives(null); };
  /** The picture and name the game uses ("Large Cluster Jewel", not the page's "Large Cluster Jewel: Lightning Damage"). */
  const realBase = pool?.cluster?.size ?? base;
  const toggleInfl = (i: string) => setInfl(infl.includes(i) ? infl.filter((x) => x !== i) : [...infl, i].slice(-2));
  const inPlan = (k: string) => plan.some((t) => t.key === k);
  const addPlan = (f: Family) => { if (!inPlan(f.key)) setPlan([...plan, { key: f.key, tier: Math.min(f.tiers.length || 1, 2) }]); };
  const lockedOut = (f: Family) => !!f.needs && !infl.includes(f.needs);

  return (
    <div className="cr">
      <div className="cr-bar">
        <label className="cr-field">Base
          <input className="text" list="cr-bases" value={base} onChange={(e) => { setBase(e.target.value); clearItem(); setPick(null); setPlan([]); }} placeholder="Eternal Burgonet" />
          <datalist id="cr-bases">{bases.map((b) => <option key={b} value={b} />)}</datalist>
        </label>
        <label className="cr-field cr-ilvl">Item level
          <input type="number" min={1} max={100} value={ilvl} onChange={(e) => setIlvl(+e.target.value)} />
        </label>
        {!pool?.jewel && <div className="cr-field">Influence
          <div className="cr-infl">{INFLUENCES.map((i) => (
            <button key={i} className={`cr-inf ${infl.includes(i) ? "on" : ""}`} style={{ "--ic": INF_COLOUR[i] } as CSSProperties} onClick={() => toggleInfl(i)} title={`${i} influence`}>
              <InfIcon name={i} size={18} />{i}</button>))}</div>
        </div>}
        <div className="cr-bar-end">
          <button onClick={async () => { try { const t = await navigator.clipboard.readText(); if (t.includes("Item Class:")) { fromText(t); return; } } catch { /* fall back */ } setPaste(true); }}>
            Use a copied item</button>
          <button className="link small" onClick={async () => {
            if (itemText) { try { await navigator.clipboard.writeText(itemText); } catch { /* still open it */ } }
            invoke("open_url", { url: "https://www.craftofexile.com/" });
          }} title={itemText ? "Your item is copied: click Import item there, then paste" : "Craft of Exile's simulator"}>Open in Craft of Exile ↗</button>
        </div>
      </div>
      {paste && <textarea className="text cr-paste" autoFocus placeholder="Paste the item here (Ctrl+V). In game: hover it and press Ctrl+C." onChange={(e) => e.target.value.includes("Item Class:") && fromText(e.target.value)} />}
      {err && <p className="danger-text small">{err}</p>}
      {pool?.cluster && <ClusterBase c={pool.cluster} passives={passives} need={routes.r?.ilvl_needed ?? null} />}

      <div className="cr-cols">
        {/* Your item + plan */}
        <div className="cr-col">
          {itemName && pool ? (
            <div className="cr-tip">
              {(() => {
                // Like the game's tooltip: a symbol at each end of the title for a fractured item or its influences
                // (one symbol: both ends; two influences: one each).
                const syms = pool.item_mods.some((m) => m.kind === "fractured") ? ["Fracturing Orb"] : infl.map((i) => `${i}'s Exalted Orb`);
                const sym = (n?: string) => n ? <span className="cr-tip-sym"><ItemIcon name={n} size={30} /></span> : <span className="cr-tip-sym" />;
                return (
                  <div className={`cr-tip-head ${syms.length ? "has-sym" : ""}`} style={{ color: RARITY_COLOUR[rarity] ?? "#ddd" }}>
                    {syms.length > 0 && sym(syms[0])}
                    <ItemIcon base={realBase} size={44} />
                    <div><div className="cr-tip-name">{itemName}</div>
                    {itemName !== realBase && <div className="cr-tip-name">{realBase}</div>}</div>
                    {syms.length > 0 && sym(syms[1] ?? syms[0])}
                  </div>
                );
              })()}
              {pool.cluster && <div className="cr-tip-meta">{passives ? "Adds " + passives + " Passive Skills · " : ""}{pool.cluster.enchant.join(" / ")}</div>}
              <div className="cr-tip-meta">Item level {ilvl}{infl.length ? <> · {infl.map((i) => <InfIcon key={i} name={i} />)} {infl.join(" + ")}</> : null}</div>
              {pool.item_mods.length === 0 && <div className="cr-tip-meta">No mods read: copy it with Ctrl+Alt+C for tiers.</div>}
              {pool.item_mods.map((m, i) => (
                <div key={i} className={`cr-tip-row ${m.kind}`}>
                  <button className={`cr-tip-mod ${m.key && m.key === pick ? "on" : ""} ${m.key ? "" : "nokey"}`} onClick={() => m.key ? setPick(m.key) : setHow(how === i ? null : i)}
                    title={m.key ? "See how to get this mod" : "How do I get this?"}>
                    <span className="cr-aff">{m.affix === "prefix" ? "P" : m.affix === "suffix" ? "S" : "·"}</span>
                    {m.kind === "fractured" && <span className="cr-kind-ico" title="Fractured: locked in place"><ItemIcon name="Fracturing Orb" size={16} /></span>}
                    {m.kind === "crafted" && <span className="cr-kind-ico crafted" title="Crafted at the bench">⚒</span>}
                    <span className="cr-tip-text" style={m.needs ? { color: INF_COLOUR[m.needs] } : undefined}>{m.text}</span>
                    {m.tier ? <span className={`cr-tier-chip t${Math.min(m.tier, 4)}`}>T{m.tier}</span> : null}
                    {!m.key && <span className="cr-q" aria-hidden>?</span>}
                  </button>
                  {m.how && (how === i || m.kind === "fractured") && <div className="cr-how">{m.how}</div>}
                </div>
              ))}
              <div className="cr-tip-foot"><span>{pool.item_mods.filter((m) => m.affix === "prefix").length}/{pool.side} prefixes · {pool.item_mods.filter((m) => m.affix === "suffix").length}/{pool.side} suffixes</span>
                <button className="link small" onClick={clearItem}>forget item</button></div>
              <button className="primary cr-planall" onClick={() => planAll(pool)} title="Puts every mod on this item into the plan and shows a step-by-step route to make it">
                Plan this whole item</button>
            </div>
          ) : (
            <div className="card cr-empty">
              <b>Bring an item</b>
              <p className="muted small">Press Ctrl+D on an item in game and click "Craft this", or Ctrl+C it and click "Use a copied item".</p>
            </div>
          )}
          {itemText && pool && routePlan.length > 0 && <WhatNowCard text={itemText} div={routes.r?.divine ?? null}
            plan={{ base, ilvl, infl, targets: routePlan.map((t) => [t.key, t.tier] as [string, number]) }}
            onRecheck={async () => { try { const t = await navigator.clipboard.readText(); if (t.includes("Item Class:")) { fromText(t); return; } } catch { /* fall back */ } setPaste(true); }} />}
          <PlanBox plan={plan} setPlan={setPlan} fam={fam} base={base} ilvl={ilvl} infl={infl} onPick={setPick} />
          <RouteSummary plan={routePlan} r={routes.r} err={routes.err} open={showRoutes} onOpen={() => setShowRoutes(!showRoutes)} />
        </div>

        {/* How to make it: every route, wide (over the mod list and the detail) */}
        {showRoutes && routePlan.length >= 2 && <div className="cr-col rc-wide">
          <RoutesPanel r={routes.r} err={routes.err} onClose={() => setShowRoutes(false)} itemText={itemText}
            plan={{ base, ilvl, infl, targets: routePlan.map((t) => [t.key, t.tier] as [string, number]) }} />
        </div>}

        {/* Every mod */}
        {!(showRoutes && routePlan.length >= 2) && <>
        <div className="cr-col cr-list">
          <div className="cr-list-top">
            <input className="text cr-search" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Find a mod: life, cold res, spell, shaper…" />
            {busy && <span className="muted small">Rolling…</span>}
          </div>
          <div className="cr-legend small">
            {pool && <span className="muted">{pool.cluster ? pool.cluster.size : pool.class === "AbyssJewel" ? "Abyss jewel" : pool.class} · item level {pool.ilvl}{pool.jewel ? " · up to " + pool.side + " prefixes + " + pool.side + " suffixes" : ""}</span>}
            <span className="muted cr-key" title="Badges after a mod"><span className="cr-mini" aria-hidden>E</span> an essence can give it <span className="cr-mini" aria-hidden>B</span> the bench can add it</span>
            <button className="link small" onClick={() => setExtras(!extras)}>{extras ? "Hide" : "Show"} bench / essence only</button>
          </div>
          {(["prefix", "suffix"] as const).map((a) => {
            const all = shown.filter((f) => f.affix === a);
            const natural = all.filter((f) => f.tiers.length > 0 && !lockedOut(f));
            const influenced = all.filter((f) => f.tiers.length > 0 && lockedOut(f));
            const special = all.filter((f) => f.tiers.length === 0);
            const row = (f: Family) => (
              <div key={f.key + f.text} className={`cr-mod ${pick === f.key ? "on" : ""} ${f.tiers.length ? "" : "cr-special"}`}
                style={f.needs ? { "--ic": INF_COLOUR[f.needs] } as CSSProperties : undefined} onClick={() => setPick(f.key)}>
                {f.needs && <InfIcon name={f.needs} />}
                <span className={`cr-text ${f.needs ? "inf" : ""}`}>{lines(f.text)}</span>
                {f.have != null && <span className="cr-yours" title="On your item">{f.have ? `T${f.have}` : "yours"}</span>}
                {f.essences.length > 0 && <span className="cr-mini" title="An essence can give it" aria-label="An essence can give it">E</span>}
                {f.bench.length > 0 && <span className="cr-mini" title="The crafting bench can add it" aria-label="The crafting bench can add it">B</span>}
                <span className="cr-odds">{f.tiers.length ? (lockedOut(f) ? `needs ${f.needs}` : odds(f.chaos)) : f.bench.length ? "bench" : "essence"}</span>
                <button className={`cr-add ${inPlan(f.key) ? "on" : ""}`} title={inPlan(f.key) ? "In your plan" : "Add to the plan (odds for several mods at once)"}
                  onClick={(e) => { e.stopPropagation(); inPlan(f.key) ? setPlan(plan.filter((t) => t.key !== f.key)) : addPlan(f); }}>{inPlan(f.key) ? "✓" : "+"}</button>
              </div>
            );
            return (
              <div key={a}>
                <h3 className="cr-h"><span>{a === "prefix" ? "Prefixes" : "Suffixes"} <span className="muted small">{natural.length} can roll</span></span>
                  <span className="cr-h-col" title="How often one Chaos Orb gives this mod">per Chaos Orb</span></h3>
                {natural.map(row)}
                {influenced.length > 0 && <div className="cr-sub">Needs an influenced item ({influenced.length})</div>}
                {influenced.map(row)}
                {(extras || q) && special.length > 0 && <div className="cr-sub">Only from the bench or an essence ({special.length})</div>}
                {(extras || q) && special.map(row)}
              </div>
            );
          })}
        </div>

        {/* One mod */}
        <div className="cr-col">
          {picked ? <Detail key={picked.key + infl.join()} fam={picked} base={base} ilvl={ilvl} infl={infl} have={have} rarity={rarity}
            inPlan={inPlan(picked.key)} onPlan={() => addPlan(picked)} /> :
            <div className="card muted cr-empty">Click a mod to see its tiers and every way to get it. Use + to add mods to a plan and see the odds of hitting them together.</div>}
        </div>
        </>}
      </div>
    </div>
  );
}

function WayCard({ w, best }: { w: Way; best: boolean }) {
  return (
    <div className={`cr-way ${best ? "best" : ""}`}>
      <MethodIcons method={w.method} />
      <div className="cr-way-main">
        <div className="cr-way-top"><b>{w.method}</b><span className="cr-way-group">{w.group}{best ? " · cheapest" : ""}</span></div>
        <div className="muted small">{w.note}</div>
      </div>
      <div className="cr-way-nums">
        <div className="cr-odds">{odds(w.chance)}</div>
        <div className="small">{w.chance < 0.995 ? (w.average != null ? <>≈ <Chaos c={w.average} /></> : <Chaos c={null} />) : <Chaos c={w.cost_each} />}</div>
        {w.chance < 0.995 && w.cost_each != null && <div className="muted small">each <Chaos c={w.cost_each} /></div>}
      </div>
    </div>
  );
}

/** A cluster jewel: which base to start from (the enchant decides the notables, fewer passives is better). */
function ClusterBase({ c, passives, need }: { c: Cluster; passives: number | null; need: number | null }) {
  return (
    <div className="card cr-cluster small">
      <ItemIcon base={c.size} size={30} />
      <div>
        <b>Start from a {c.size}</b> with the enchant <b>"{c.enchant.join(" / ")}"</b>
        {" "}and {passives ? passives + " passives" : c.min + " passives (the fewest: " + c.min + "–" + c.max + ")"}
        {need && need > 1 ? <>, item level <b>{need}+</b> for the notables in your plan</> : null}.
        <div className="muted">The enchant decides which notables can roll; the item level decides which of them. "Buy base" in the build window finds one.
          Jewels take no essences or bench crafts: use Alteration, Augmentation, Regal, Exalted Orbs, fossils and Harvest reforges.</div>
      </div>
    </div>
  );
}

function PlanBox({ plan, setPlan, fam, base, ilvl, infl, onPick }: { plan: Target[]; setPlan: (p: Target[]) => void; fam: (k: string) => Family | null;
  base: string; ilvl: number; infl: string[]; onPick: (k: string) => void }) {
  const [ways, setWays] = useState<Way[] | null>(null);
  const [err, setErr] = useState("");
  const targets = plan.filter((t) => fam(t.key));
  useEffect(() => {
    if (targets.length < 2) { setWays(null); return; }
    let live = true;
    setWays(null); setErr("");
    invoke<Way[]>("craft_plan", { base, ilvl, influences: infl, targets: targets.map((t) => [t.key, t.tier]) })
      .then((w) => live && setWays(w)).catch((e) => live && setErr(friendly(e)));
    return () => { live = false; };
  }, [JSON.stringify(targets), base, ilvl, infl.join(",")]);
  return (
    <div className="card cr-plan">
      <h2>Plan <span className="muted small">all of these on one item</span></h2>
      {targets.length === 0 && <p className="muted small">Press + next to mods you want together (e.g. life + two resistances) to see the odds of hitting them all at once.</p>}
      {targets.map((t) => { const f = fam(t.key)!; return (
        <div key={t.key} className="cr-plan-row">
          <button className="link cr-plan-text" onClick={() => onPick(t.key)} style={f.needs ? { color: INF_COLOUR[f.needs] } : undefined}>{lines(f.text)}</button>
          {f.tiers.length > 0 && <select value={t.tier} onChange={(e) => setPlan(plan.map((x) => x.key === t.key ? { ...x, tier: +e.target.value } : x))} title="This tier or better">
            {f.tiers.map((x) => <option key={x.tier} value={x.tier}>T{x.tier}+</option>)}</select>}
          <button className="link small" onClick={() => setPlan(plan.filter((x) => x.key !== t.key))} aria-label="Remove">✕</button>
        </div>); })}
      {targets.length === 1 && <p className="muted small">Add one more mod to see combined odds.</p>}
      {err && <p className="danger-text small">{err}</p>}
      {targets.length >= 2 && !ways && !err && <p className="muted small">Rolling it thousands of times…</p>}
      {ways && ways.map((w, i) => <WayCard key={w.method} w={w} best={i === 0 && w.average != null} />)}
      {targets.length > 0 && <CraftSearchBox base={base} ilvl={ilvl} infl={infl} targets={targets} />}
      {targets.length > 0 && <button className="link small" onClick={() => setPlan([])}>Clear the plan</button>}
    </div>
  );
}

function Detail({ fam, base, ilvl, infl, have, rarity, inPlan, onPlan }: { fam: Family; base: string; ilvl: number; infl: string[]; have: string[]; rarity: string;
  inPlan: boolean; onPlan: () => void }) {
  const best = fam.tiers.find((t) => t.ok)?.tier ?? 1;
  const [tier, setTier] = useState(Math.min(fam.tiers.length || 1, best + 1));
  const [ways, setWays] = useState<Way[] | null>(null);
  const [err, setErr] = useState("");
  useEffect(() => {
    let live = true;
    setWays(null);
    invoke<Way[]>("craft_ways", { base, ilvl, influences: infl, key: fam.key, minTier: tier, have, rarity })
      .then((w) => live && setWays(w)).catch((e) => live && setErr(friendly(e)));
    return () => { live = false; };
  }, [fam.key, tier, base, ilvl, infl.join(","), have.join("|"), rarity]);
  const locked = !!fam.needs && !infl.includes(fam.needs);
  return (
    <div className="card cr-detail">
      <div className="cr-detail-head">
        <h2 style={fam.needs ? { color: INF_COLOUR[fam.needs] } : undefined}>{fam.needs && <InfIcon name={fam.needs} size={20} />}{fam.text.split("\n").map((l, i) => <div key={i}>{l}</div>)}</h2>
        <button className="small" onClick={onPlan} disabled={inPlan}>{inPlan ? "✓ In plan" : "+ Add to plan"}</button>
      </div>
      <div className="muted small">{fam.affix === "prefix" ? "Prefix" : "Suffix"}{fam.tags.length ? ` · ${fam.tags.join(", ")}` : ""}</div>
      {locked && <div className="cr-needs" style={{ "--ic": INF_COLOUR[fam.needs!] } as CSSProperties}>
        <InfIcon name={fam.needs!} size={22} /><div><b>Only on a {fam.needs} item.</b> Your base has no {fam.needs} influence: {["Shaper", "Elder"].includes(fam.needs!) ? <>buy one with {fam.needs} influence (there's no {fam.needs} Exalted Orb),</> : <>add it with a {fam.needs}'s Exalted Orb,</>}
        or tick {fam.needs} above to see the odds on an item that already has it.</div></div>}

      <h3>Ways to get it</h3>
      {err && <p className="danger-text small">{err}</p>}
      {!ways && !err && <p className="muted small">Rolling the item a few thousand times…</p>}
      {ways && ways.length === 0 && <p className="muted small">No simple way found: see "More ways" below.</p>}
      {ways && ways.length > 0 && <p className="muted small">Cheapest first, for tier {tier} or better. {rarity ? `Worked out on your ${rarity.toLowerCase()} item.` : "Bring your item for exact Exalted / Regal odds."}</p>}
      {ways && ways.map((w, i) => <WayCard key={w.method} w={w} best={i === 0 && w.average != null} />)}

      {fam.tiers.length > 0 && <>
        <h3>Tiers <span className="muted small">click to aim for that tier or better</span></h3>
        <div className="cr-tiers">
          {fam.tiers.map((t) => (
            <button key={t.name} className={`cr-tier ${t.tier <= tier ? "want" : ""} ${t.ok ? "" : "locked"}`} onClick={() => setTier(t.tier)}
              title={t.ok ? `Aim for tier ${t.tier} or better` : `Needs item level ${t.lvl}`}>
              <b>T{t.tier}</b><span>{lines(t.text)}</span><span className="muted">{t.name}</span><span className="muted">{t.ok ? `ilvl ${t.lvl}` : `needs ${t.lvl}`}</span>
            </button>
          ))}
        </div>
      </>}

      {fam.essences.length > 0 && <>
        <h3>Essences</h3>
        <div className="cr-table">
          {fam.essences.map((e) => (
            <div key={e.name} className="cr-trow">
              <ItemIcon name={e.name} size={24} />
              <span className="cr-tname">{e.name.replace(/ Essence of .*/, "")}<span className="muted"> {e.name.match(/Essence of .*/)?.[0]}</span></span>
              <span className="cr-ttext">{lines(e.text)}</span>
              {e.like_tier ? <span className={`cr-tier-chip t${Math.min(e.like_tier, 4)}`}>≈T{e.like_tier}</span> : <span />}
              <Chaos c={e.price} />
            </div>
          ))}
        </div>
      </>}
      {fam.bench.length > 0 && <>
        <h3>Crafting bench</h3>
        <div className="cr-table">
          {fam.bench.slice(0, 5).map((b) => (
            <div key={b.text} className="cr-trow cr-bench">
              <span className="cr-ttext">{lines(b.text)}</span>
              <span className="cr-cost">{b.cost.map(([c, n]) => <span key={c} title={c}>{n}×<ItemIcon name={c} size={20} /></span>)}</span>
            </div>
          ))}
        </div>
      </>}
      {(fam.fossils_more.length > 0 || fam.fossils_none.length > 0) && <>
        <h3>Fossils</h3>
        <div className="cr-fossils">
          {fam.fossils_more.map((f) => <span key={f} className="cr-fossil good"><ItemIcon name={f} size={20} />{f}<em>more likely</em></span>)}
          {fam.fossils_none.map((f) => <span key={f} className="cr-fossil bad"><ItemIcon name={f} size={20} />{f}<em>never</em></span>)}
        </div>
      </>}
      <details className="cr-more">
        <summary>More ways (not worked out here)</summary>
        <ul className="small">
          <li><b>Veiled Chaos Orb / Aisling</b>: adds a veiled mod you unveil from Jun (a choice of 3); "veiled" bench mods like "of the Order" come only from this.</li>
          <li><b>Awakener's Orb</b>: merges two influenced items, keeping one influence mod from each (listed above for influence mods).</li>
          <li><b>Eldritch Ember / Ichor</b> and <b>Eldritch Exalted / Annulment / Chaos</b>: Exarch and Eater implicits, and changing a mod while the item keeps them.</li>
          <li><b>Recombinator</b> (only in leagues that have them): combines two items of the same kind, keeping a random mix of their mods.</li>
          <li><b>Fracturing Orb</b>: locks one mod on a rare with 4+ mods, so you can reroll the rest freely.</li>
          <li><b>Metamods on the bench</b>: "Prefixes Cannot Be Changed" + Orb of Scouring removes only the suffixes (or the other way round); "Cannot roll Attack/Caster Modifiers" blocks those for Exalts.</li>
          <li><b>Orb of Annulment</b> removes one random mod; <b>Divine Orb</b> rerolls the numbers; <b>Blessed Orb</b> rerolls the implicit.</li>
          <li><b>Beastcrafting</b>: Imprint a magic item to try again; add a prefix or suffix and more.</li>
        </ul>
      </details>
      <p className="muted small">Odds come from rolling the item here thousands of times: a good guide, not exact.</p>
    </div>
  );
}


type Step = { title: string; detail: string; ways: Way[] };
type Route = { id: string; name: string; blurb: string; steps: Step[]; cost: number | null; clicks: number; risk: number; checks: string[]; badges: string[] };
type Routes = { routes: Route[]; ilvl_needed: number; note: string; divine: number | null };
type Listing = { price: number | null };
type BuyResult = { summary: [number, number] | null; total: number; url: string; divine: number | null; listings: Listing[] };
type Sort = "cost" | "clicks" | "risk";

const RISK = ["Safe", "Can fail a step", "Can lose mods"];
/** Chaos, with the Divine equivalent once it's big. */
function Cost({ c, div }: { c: number | null; div: number | null }) {
  if (c == null) return <span className="muted">price unknown</span>;
  return <span className="rc-cost"><Chaos c={c} />{div && c >= div * 0.8 ? <span className="muted"> ≈ {+(c / div).toFixed(c / div < 10 ? 1 : 0)}<CurrencyIcon id="divine" size={14} /></span> : null}</span>;
}

/** The routes for the plan, worked out once and shared by the summary card and the wide panel. */
function useRoutes(plan: Target[], base: string, ilvl: number, infl: string[], have: string[]) {
  const [r, setR] = useState<Routes | null>(null);
  const [err, setErr] = useState("");
  useEffect(() => {
    if (plan.length < 2) { setR(null); setErr(""); return; }
    let live = true;
    setR(null); setErr("");
    invoke<Routes | null>("craft_routes", { base, ilvl, influences: infl, targets: plan.map((t) => [t.key, t.tier]), have })
      .then((x) => live && setR(x ?? { routes: [], ilvl_needed: 1, note: "", divine: null })).catch((e) => live && setErr(friendly(e)));
    return () => { live = false; };
  }, [JSON.stringify(plan), base, ilvl, infl.join(","), have.join("|")]);
  return { r, err };
}

/** Left column: the best route in one line, and the button to compare them all. */
function RouteSummary({ plan, r, err, onOpen, open }: { plan: Target[]; r: Routes | null; err: string; onOpen: () => void; open: boolean }) {
  if (plan.length < 2) return null;
  const best = r?.routes[0];
  return (
    <div className="card rc-sum">
      <h2>How to make it</h2>
      {err && <p className="danger-text small">{err}</p>}
      {!r && !err && <p className="muted small">Comparing ways to craft it…</p>}
      {r && !best && <p className="muted small">{r.note || "No route found for these mods."}</p>}
      {best && <>
        <div className="rc-sum-best"><span className="rc-sum-label">Cheapest</span><b>{best.name}</b><Cost c={best.cost} div={r.divine} /></div>
        <Verdict best={best} div={r.divine} />
        <button className={open ? "" : "primary"} onClick={onOpen}>{open ? "Back to the mods" : `Compare all ${r.routes.length} routes →`}</button>
      </>}
    </div>
  );
}

/** The wide panel: every route side by side, sortable, each with its steps and the options per step. */
type PlanRef = { base: string; ilvl: number; infl: string[]; targets: [string, number][] };
function RoutesPanel({ r, err, onClose, itemText, plan }: { r: Routes | null; err: string; onClose: () => void; itemText: string; plan: PlanRef }) {
  const [sort, setSort] = usePersist<Sort>("craft.routeSort", "cost");
  const [open, setOpen] = useState<string | null>(null);
  const routes = useMemo(() => {
    const v = [...(r?.routes ?? [])];
    const cost = (x: Route) => x.cost ?? Number.MAX_VALUE;
    if (sort === "clicks") v.sort((a, b) => a.clicks - b.clicks || cost(a) - cost(b));
    if (sort === "risk") v.sort((a, b) => a.risk - b.risk || cost(a) - cost(b));
    return v;
  }, [r, sort]);
  const shown = open ?? routes[0]?.id ?? null;
  return (
    <div className="card rc-panel">
      <div className="rc-panel-head">
        <div><h2>How to make it</h2>
          <p className="muted small">{r ? `${routes.length} ways to craft it, worked out with today's prices. Costs are averages: luck swings them.` : "Comparing ways to craft it…"}</p></div>
        <button className="link small" onClick={onClose}>← Back to the mods</button>
      </div>
      {err && <p className="danger-text small">{err}</p>}
      {r && <div className="rc-sort" role="tablist" aria-label="Sort routes">
        {([["cost", "Cheapest"], ["clicks", "Fewest clicks"], ["risk", "Safest"]] as [Sort, string][]).map(([k, label]) => (
          <button key={k} role="tab" aria-selected={sort === k} className={`chip ${sort === k ? "on" : ""}`} onClick={() => setSort(k)}>{label}</button>))}
      </div>}
      <BuyCheck text={itemText} plan={plan} best={r?.routes[0]?.cost ?? null} div={r?.divine ?? null} />
      {r?.note && <p className="muted small rc-note">{r.note}</p>}
      <div className="rc-list">
        {routes.map((x) => <RouteCard key={x.id} x={x} div={r?.divine ?? null} open={shown === x.id} onToggle={() => setOpen(shown === x.id ? "" : x.id)} />)}
      </div>
      <p className="muted small">Odds come from rolling the item here thousands of times, with the 3.30 rules. Each step only counts once it leaves room for the steps after it.</p>
    </div>
  );
}

function RouteCard({ x, div, open, onToggle }: { x: Route; div: number | null; open: boolean; onToggle: () => void }) {
  const [more, setMore] = useState<number | null>(null);
  return (
    <section className={`rc-route ${open ? "open" : ""}`}>
      <button className="rc-route-head" onClick={onToggle} aria-expanded={open}>
        <div className="rc-route-name">
          <b>{x.name}</b>
          {x.badges.map((b) => <span key={b} className="rc-badge">{b}</span>)}
          {x.checks.length > 0 && <span className="rc-check-pill" title={x.checks.join("\n")}>check in game</span>}
        </div>
        <div className="rc-route-blurb muted small">{x.blurb}</div>
        <div className="rc-route-nums">
          <span className="rc-big"><Cost c={x.cost} div={div} /></span>
          <span className="muted small">{Math.max(1, Math.round(x.clicks)).toLocaleString()} clicks</span>
          <span className={`rc-risk r${x.risk}`}>{RISK[x.risk] ?? ""}</span>
        </div>
      </button>
      {open && <div className="rc-route-body">
        <ol className="rc-steps">
          {x.steps.map((s, i) => {
            const best = s.ways[0];
            return (
              <li key={i} className="rc-step">
                <span className="rc-num">{i + 1}</span>
                <div className="rc-step-main">
                  <div className="rc-title">{s.title}</div>
                  <div className="muted small">{s.detail}</div>
                  {best && <div className="rc-best-way">
                    <MethodIcons method={best.method} />
                    <div className="rc-best-text"><b>{best.method}</b><span className="muted small">{best.note}</span></div>
                    <div className="rc-best-nums">
                      {best.chance < 0.995 && <span className="cr-odds">{odds(best.chance)}</span>}
                      <span className="small">{best.average != null ? <>≈ <Chaos c={best.average} /></> : <Chaos c={null} />}</span>
                    </div>
                  </div>}
                  {s.ways.length > 1 && <button className="link small" onClick={() => setMore(more === i ? null : i)}>
                    {more === i ? "Hide the other ways" : `${s.ways.length - 1} other way${s.ways.length > 2 ? "s" : ""}`}</button>}
                  {more === i && <div className="rc-others">{s.ways.slice(1).map((w) => <WayCard key={w.method} w={w} best={false} />)}</div>}
                </div>
              </li>
            );
          })}
        </ol>
        {x.checks.length > 0 && <div className="rc-checks"><b>Check in game:</b><ul>{x.checks.map((c) => <li key={c}>{c}</li>)}</ul></div>}
      </div>}
    </section>
  );
}

type NowOption = { title: string; why: string; icon: string; chance: number; cost_each: number | null; average: number | null; risk: number; check: string | null };
type WhatNow = { name: string; mods: [string, string][]; missing: string[]; options: NowOption[]; left: number | null; unlucky: number | null;
  done: boolean; warnings: string[]; note: string };
const nowOdds = (p: number) => p >= 0.995 ? "Sure thing" : p >= 0.5 ? `${Math.round(p * 100)}%` : `1 in ${Math.round(1 / p)}`;

/** Craft: what now? (26 Nov, look A): the copied item against the plan: the best next click, two other choices, what's
 *  left to spend against buying one, and selling it as it is. Same maths as the routes; `core/src/craft/now.rs`. */
function WhatNowCard({ text, plan, div, onRecheck }: { text: string; plan: PlanRef; div: number | null; onRecheck: () => void }) {
  const [n, setN] = useState<WhatNow | null>(null);
  const [err, setErr] = useState("");
  const [buy, setBuy] = useState<BuyResult | null>(null);
  const [sell, setSell] = useState<BuyResult | null>(null);
  const [busy, setBusy] = useState("");
  useEffect(() => {
    let live = true;
    setErr("");
    invoke<WhatNow | null>("craft_now", { text, targets: plan.targets }).then((x) => live && setN(x)).catch((e) => live && setErr(friendly(e)));
    return () => { live = false; };
  }, [text, JSON.stringify(plan.targets)]);
  useEffect(() => { setBuy(null); }, [JSON.stringify(plan)]);
  useEffect(() => { setSell(null); }, [text]);
  const look = async (what: "buy" | "sell") => {
    setBusy(what);
    try {
      if (what === "buy") setBuy(await invoke<BuyResult>("craft_buy_plan", { base: plan.base, ilvl: plan.ilvl, influences: plan.infl, targets: plan.targets }));
      else setSell(await invoke<BuyResult>("craft_buy", { text }));
    } catch (e) { setErr(friendly(e)); } finally { setBusy(""); }
  };
  if (err) return <div className="cn card"><b>What now?</b><p className="danger-text small">{err}</p></div>;
  if (!n) return null;
  const [next, ...alts] = n.options;
  const buyAt = buy?.summary?.[1] ?? null;
  const sellAt = sell?.summary?.[1] ?? null;
  const verdict = n.done ? ["good", "Done"] : n.left == null ? null
    : buyAt != null && buyAt < n.left * 0.8 ? ["bad", "Buy one instead"]
    : buyAt != null && buyAt < n.left ? ["warn", "Close call"]
    : ["good", "Keep going"];
  return (
    <div className="cn">
      <div className="cn-head"><b>What now?</b>
        <button className="link small" onClick={onRecheck} title="Copy the item again in game (Ctrl+C), then click">I clicked it, check again</button></div>
      {n.warnings.map((w) => <div key={w} className="cn-warn small">{w}</div>)}
      {n.done ? <div className="cn-next done"><b>It's done</b><div className="cn-why">{n.note}</div></div>
        : next ? <div className="cn-next">
          <div className="muted small">Next click</div>
          <div className="cn-title"><ItemIcon name={next.icon} size={26} /><b>{next.title}</b></div>
          <div className="cn-why">{next.why}</div>
          <div className="cn-tags">
            <span className={`cn-tag ${next.chance >= 0.2 ? "g" : ""}`}>{nowOdds(next.chance)}</span>
            {next.cost_each != null && <span className="cn-tag">about <Cost c={next.cost_each} div={div} /> a try</span>}
            {next.risk === 2 && <span className="cn-tag r">can take a mod you want</span>}
            {next.check && <span className="cn-tag y" title={next.check}>check in game</span>}
          </div>
        </div>
        : <div className="cn-next"><div className="cn-why">{n.note}</div></div>}
      {alts.slice(0, 2).map((a) => (
        <div key={a.title} className="cn-alt small"><b>Or:</b> {a.title} <span className="muted">({nowOdds(a.chance)}{a.cost_each != null ? <>, <Cost c={a.cost_each} div={div} /> a try</> : null})</span></div>
      ))}
      {!n.done && n.note && next && <div className="muted small">{n.note}</div>}
      <div className="cn-stats">
        <div className="cn-stat"><span className="muted small">Left to spend</span>
          <b className="gold">{n.done ? "Nothing" : n.left != null ? <Cost c={n.left} div={div} /> : "—"}</b>
          {!n.done && n.unlucky != null && <span className="muted small">unlucky: <Cost c={n.unlucky} div={div} /></span>}</div>
        <div className="cn-stat"><span className="muted small">Buy a finished one</span>
          {buy ? <><b>{buyAt != null ? <Cost c={buyAt} div={div} /> : "none listed"}</b>
            <button className="link small" onClick={() => invoke("open_url", { url: buy.url })}>{buy.total} listed ↗</button></>
            : <button className="small" onClick={() => look("buy")} disabled={busy === "buy"}>{busy === "buy" ? "Searching…" : "Check"}</button>}</div>
        <div className="cn-stat"><span className="muted small">Sell it as it is</span>
          {sell ? <><b>{sellAt != null ? <Cost c={sellAt} div={div} /> : "none like it"}</b>
            <button className="link small" onClick={() => invoke("open_url", { url: sell.url })}>{sell.total} listed ↗</button></>
            : <button className="small" onClick={() => look("sell")} disabled={busy === "sell"}>{busy === "sell" ? "Searching…" : "Price it"}</button>}</div>
        {verdict && <div className={`cn-stat cn-verdict ${verdict[0]}`}><span className="muted small">Verdict</span><b>{verdict[1]}</b></div>}
      </div>
    </div>
  );
}

/** "Or just buy one": the same trade search as Ctrl+D, on a click. */
function BuyCheck({ text, plan, best, div }: { text: string; plan: PlanRef; best: number | null; div: number | null }) {
  const [res, setRes] = useState<BuyResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  useEffect(() => { setRes(null); setErr(""); }, [text, JSON.stringify(plan)]);
  const go = async () => {
    setBusy(true); setErr("");
    try {
      // A copied item: items like it. Planned from a blank base: an item with the planned mods at their tiers or better.
      setRes(text ? await invoke<BuyResult>("craft_buy", { text })
        : await invoke<BuyResult>("craft_buy_plan", { base: plan.base, ilvl: plan.ilvl, influences: plan.infl, targets: plan.targets }));
    } catch (e) { setErr(friendly(e)); } finally { setBusy(false); }
  };
  const typical = res?.summary?.[1] ?? null;
  return (
    <div className="rc-buy">
      <ItemIcon name="Chaos Orb" size={22} />
      <div className="rc-buy-main">
        <b>Or just buy one</b>
        {!res && !err && <span className="muted small">{text ? "Searches the trade site for items like your copied one." : "Searches the trade site for this base with every planned mod at its tier or better."}</span>}
        {err && <span className="danger-text small">{err}</span>}
        {res && (res.summary ? <span className="small">{res.total.toLocaleString()} listed · lowest <Cost c={res.summary[0]} div={div} /> · typical <Cost c={res.summary[1]} div={div} />
          {best != null && typical != null && <b className={typical < best ? "rc-buy-win" : ""}> · {typical < best * 0.8 ? "buy it instead" : typical < best ? "buying is a bit cheaper" : "crafting is cheaper"}</b>}</span>
          : <span className="muted small">None listed like it right now.</span>)}
      </div>
      {res ? <button className="link small" onClick={() => invoke("open_url", { url: res.url })}>Open on trade ↗</button>
        : <button className="small" onClick={go} disabled={busy}>{busy ? "Searching…" : "Check the price"}</button>}
    </div>
  );
}
type CraftSearch = { text: string; parts: [string, string][]; too_long: boolean; notes: string[] };

/** Crafting search (22 Nov): a stash search that lights the item up when a planned mod rolls. Put the item in your
 *  stash tab, paste this into the stash search, spam orbs, stop when it glows. No Ctrl+C after every orb. */
function CraftSearchBox({ base, ilvl, infl, targets }: { base: string; ilvl: number; infl: string[]; targets: Target[] }) {
  const [all, setAll] = usePersist<boolean>("craft.search.all", false);
  const [s, setS] = useState<CraftSearch | null>(null);
  const [note, setNote] = useState("");
  useEffect(() => {
    let live = true;
    invoke<CraftSearch | null>("craft_search", { base, ilvl, influences: infl, targets: targets.map((t) => [t.key, t.tier]), all })
      .then((r) => live && setS(r)).catch(() => live && setS(null));
    return () => { live = false; };
  }, [JSON.stringify(targets), base, ilvl, infl.join(","), all]);
  if (!s || !s.text) return null;
  const flash = (t: string) => { setNote(t); setTimeout(() => setNote(""), 2500); };
  const keep = async () => {
    const v = await invoke<{ saved: { name: string; text: string }[] }>("searches_get");
    const name = `Craft: ${base}`;
    await invoke("searches_save", { list: [...v.saved.filter((x) => x.name !== name), { name, text: s.text }] });
    flash("Added to your searches (F10)");
  };
  return (
    <div className="cr-search">
      <div className="row between" style={{ margin: "4px 0" }}>
        <b>Crafting search</b>
        <span className="row" style={{ margin: 0 }}>
          <label className="check small" style={{ margin: 0 }}><input type="radio" checked={!all} onChange={() => setAll(false)} /> any of these</label>
          <label className="check small" style={{ margin: 0 }}><input type="radio" checked={all} onChange={() => setAll(true)} /> all of them</label>
        </span>
      </div>
      <p className="muted small" style={{ margin: "2px 0 6px" }}>Put the item in your stash tab, paste this into the stash search, then spam your orbs:
        the item lights up the moment {all ? "it has all of these" : "one of these rolls"}.</p>
      <code className="cr-search-text" title={s.text}>{s.text}</code>
      <div className="row" style={{ margin: "6px 0 0" }}>
        <button className="small" onClick={() => { invoke("copy_text", { text: s.text }); flash("Copied"); }}>Copy</button>
        <button className="small" onClick={() => invoke("search_type", { text: s.text }).then(() => flash("Put in the game's search box")).catch((e) => flash(friendly(e)))}
          title="Open your stash in game first">Put it in game</button>
        <button className="small" onClick={keep}>Add to my searches</button>
        <span className={`small ${s.too_long ? "danger-text" : "muted"}`}>{s.text.length}/250</span>
        {note && <span className="muted small">{note}</span>}
      </div>
      {s.notes.map((n, i) => <p key={i} className="muted small" style={{ margin: "4px 0 0" }}>{n}</p>)}
    </div>
  );
}

/** One plain verdict on the cheapest route: easy, worth a try, or usually not worth crafting (then check the buy price). */
function Verdict({ best, div }: { best: Route; div: number | null }) {
  if (best.cost == null) return null;
  const d = div ? best.cost / div : null;
  const [cls, title, text] = d != null && d < 0.5 && best.risk === 0 ? ["good", "Easy craft", "Cheap and safe: go for it."]
    : d != null && d > 30 ? ["bad", "Usually not worth crafting", `Expect about ${Math.round(d)} divines on average (luck swings it a lot). Compare with buying one first.`]
    : d != null && d > 8 ? ["warn", "Pricey craft", `Expect about ${Math.round(d)} divines on average. Check the buy price before you start.`]
    : ["ok", "Worth a try", best.risk === 2 ? "A step can lose mods: have spare currency." : best.risk === 1 ? "A step can fail and need redoing." : "Reasonable cost and risk."];
  return <div className={`rc-verdict ${cls}`}><b>{title}</b> <span className="small">{text}</span></div>;
}
