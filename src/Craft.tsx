// Crafting page: pick a base (or bring an item from Ctrl+D), see every mod that can roll on it (influence-only ones
// marked), how often a Chaos Orb gives it, and click a mod for the ways to get it with odds and cost. Add several mods
// to the plan to get the odds of hitting them all at once. Everything fits on one screen: three columns that scroll.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState, type CSSProperties } from "react";
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
type Pool = { base: string; class: string; ilvl: number; influences: string[]; families: Family[]; item_mods: ItemMod[] };
type Way = { group: string; method: string; chance: number; cost_each: number | null; average: number | null; note: string };
type Start = { base: string; ilvl: number; influences: string[]; have: string[]; name: string; rarity: string };
type Target = { key: string; tier: number };

const INFLUENCES = ["Shaper", "Elder", "Crusader", "Redeemer", "Hunter", "Warlord"];
const INF_COLOUR: Record<string, string> = { Shaper: "#7fb2ff", Elder: "#c79bff", Crusader: "#ffb15e", Redeemer: "#6fdcd2", Hunter: "#9be07a", Warlord: "#ff7a6b" };
const RARITY_COLOUR: Record<string, string> = { Normal: "#c8c8c8", Magic: "#8888ff", Rare: "#ffff77", Unique: "#af6025" };
/** Shared with the Ctrl+D window: the item its "Craft this" button sends here. */
export const CRAFT_ITEM_KEY = "craft.item";

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
  const parts = method.startsWith("Harvest") ? ["Wild Crystallised Lifeforce"] : method.startsWith("Chaos Orb on") ? ["Chaos Orb"] : method.split(" + ");
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

  useEffect(() => { invoke<string[]>("craft_bases").then(setBases).catch(() => {}); }, []);

  const fromText = async (text: string) => {
    const s = await invoke<Start | null>("craft_start", { text }).catch(() => null);
    if (!s) { setErr("That doesn't look like gear you can craft on (armour, weapons, jewellery, quivers, shields)."); return; }
    setErr(""); setBase(s.base); setIlvl(s.ilvl); setInfl(s.influences); setHave(s.have); setRarity(s.rarity); setItemName(s.name); setItemText(text);
    setPick(null); setPaste(false); setPlan([]);
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
    return (pool?.families ?? []).filter((f) => words.every((w) => (f.text + " " + f.tags.join(" ") + " " + (f.needs ?? "")).toLowerCase().includes(w)));
  }, [pool, q]);
  const fam = (k: string | null) => pool?.families.find((f) => f.key === k) ?? null;
  const picked = fam(pick);

  const clearItem = () => { setHave([]); setItemName(""); setItemText(""); setRarity(""); };
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
        <div className="cr-field">Influence
          <div className="cr-infl">{INFLUENCES.map((i) => (
            <button key={i} className={`cr-inf ${infl.includes(i) ? "on" : ""}`} style={{ "--ic": INF_COLOUR[i] } as CSSProperties} onClick={() => toggleInfl(i)} title={`${i} influence`}>
              <InfIcon name={i} size={18} />{i}</button>))}</div>
        </div>
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
                    <ItemIcon base={base} size={44} />
                    <div><div className="cr-tip-name">{itemName}</div>
                    {itemName !== base && <div className="cr-tip-name">{base}</div>}</div>
                    {syms.length > 0 && sym(syms[1] ?? syms[0])}
                  </div>
                );
              })()}
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
              <div className="cr-tip-foot"><span>{pool.item_mods.filter((m) => m.affix === "prefix").length}/3 prefixes · {pool.item_mods.filter((m) => m.affix === "suffix").length}/3 suffixes</span>
                <button className="link small" onClick={clearItem}>forget item</button></div>
              <button className="primary cr-planall" onClick={() => {
                // Every mod on the item that can be crafted, at its own tier or better, into the plan.
                const seen = new Set<string>();
                setPlan(pool.item_mods.filter((m) => m.key && m.kind !== "fractured" && !seen.has(m.key) && seen.add(m.key))
                  .map((m) => { const f = fam(m.key); return { key: m.key!, tier: m.tier ?? Math.min(f?.tiers.length || 1, 2) }; }));
              }} title="Puts every mod on this item into the plan and shows a step-by-step route to make it">
                Plan this whole item</button>
            </div>
          ) : (
            <div className="card cr-empty">
              <b>Bring an item</b>
              <p className="muted small">In game, hover an item and press Ctrl+C, then click "Use a copied item". Or press Ctrl+D on it and click "Craft this". Its mods show here, and the odds use its open slots.</p>
            </div>
          )}
          <PlanBox plan={plan} setPlan={setPlan} fam={fam} base={base} ilvl={ilvl} infl={infl} onPick={setPick} />
          <RecipeBox plan={plan.filter((t) => fam(t.key))} base={base} ilvl={ilvl} infl={infl} />
        </div>

        {/* Every mod */}
        <div className="cr-col cr-list">
          <div className="cr-list-top">
            <input className="text cr-search" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Find a mod: life, cold res, spell, shaper…" />
            {busy && <span className="muted small">Rolling…</span>}
          </div>
          <div className="cr-legend small">
            {pool && <span className="muted">{pool.class} · item level {pool.ilvl}</span>}
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
                {f.essences.length > 0 && <span className="cr-mini" title="An essence can give it">E</span>}
                {f.bench.length > 0 && <span className="cr-mini" title="The crafting bench can add it">B</span>}
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
        <InfIcon name={fam.needs!} size={22} /><div><b>Only on a {fam.needs} item.</b> Your base has no {fam.needs} influence: add it with a {fam.needs}'s Exalted Orb,
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
          <li><b>Awakener's Orb</b>: merges two influenced items, keeping one influence mod from each.</li>
          <li><b>Eldritch Ember / Ichor</b> and <b>Eldritch Exalted / Annulment / Chaos</b>: Exarch and Eater implicits, and changing a mod while the item keeps them.</li>
          <li><b>Recombinator</b>: combines two items of the same kind, keeping a random mix of their mods.</li>
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
type Recipe = { steps: Step[]; total: number | null; ilvl_needed: number; note: string };

/** "How to make it": a step-by-step route for everything in the plan (base, roll the hardest mods, add the rest,
 *  finish at the bench), each step with its cheapest ways. */
function RecipeBox({ plan, base, ilvl, infl }: { plan: Target[]; base: string; ilvl: number; infl: string[] }) {
  const [r, setR] = useState<Recipe | null>(null);
  const [err, setErr] = useState("");
  const [open, setOpen] = useState<number | null>(null);
  useEffect(() => {
    if (plan.length < 2) { setR(null); return; }
    let live = true;
    setR(null); setErr("");
    invoke<Recipe | null>("craft_recipe", { base, ilvl, influences: infl, targets: plan.map((t) => [t.key, t.tier]) })
      .then((x) => live && setR(x)).catch((e) => live && setErr(friendly(e)));
    return () => { live = false; };
  }, [JSON.stringify(plan), base, ilvl, infl.join(",")]);
  if (plan.length < 2) return null;
  return (
    <div className="card rc">
      <h2>How to make it <span className="muted small">a suggested route</span></h2>
      {err && <p className="danger-text small">{err}</p>}
      {!r && !err && <p className="muted small">Working out a route…</p>}
      {r && <>
        <ol className="rc-steps">
          {r.steps.map((s, i) => {
            const best = s.ways.find((w) => w.average != null) ?? s.ways[0];
            return (
              <li key={i} className="rc-step">
                <button className="rc-head" onClick={() => setOpen(open === i ? null : i)} aria-expanded={open === i}>
                  <span className="rc-num">{i + 1}</span>
                  <span className="rc-title">{s.title}</span>
                  {best && <span className="rc-best">{best.method}{best.chance < 0.995 ? ` · ${odds(best.chance)}` : ""}{best.average != null ? <> · ≈ <Chaos c={best.average} /></> : null}</span>}
                </button>
                {(open === i || s.ways.length === 0) && <div className="rc-body">
                  <div className="muted small">{s.detail}</div>
                  {s.ways.map((w, j) => <WayCard key={w.method} w={w} best={j === 0 && w.average != null} />)}
                </div>}
              </li>
            );
          })}
        </ol>
        <div className="rc-total">Roughly <b>{r.total != null ? <Chaos c={r.total} /> : "unknown (some prices missing)"}</b> in total, on average. Click a step for its options.</div>
        {r.note && <p className="muted small">{r.note}</p>}
      </>}
    </div>
  );
}