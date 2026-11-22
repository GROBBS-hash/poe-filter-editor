import { friendly } from "./errors";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { CurrencyIcon, ItemIcon } from "./icons";
import BaseUniques from "./BaseUniques";
import RelatedItems from "./RelatedItems";
import { Item, RARITY_COLOUR, itemTitle } from "./types";
import { CRAFT_ITEM_KEY } from "./Craft";

type StatFilter = { id: string; label: string; value: number; min: number | null; enabled: boolean; kind: string; tier: number | null; junk?: boolean; roll?: number | null; range?: string | null; affix?: string | null;
  /** An upper limit too: a cluster jewel's passive count is searched exactly. */
  max?: number | null;
  /** Where the mod came from when special: eldritch, essence, delve, influence… (from the advanced copy). */
  source?: string | null };

/** The coloured badge for a mod that needs something special (like Awakened's tags): [label, picture, what it means]. */
const SOURCE: Record<string, [string, string | null, string]> = {
  exarch: ["Exarch", null, "Searing Exarch implicit (eldritch altar)"],
  eater: ["Eater", null, "Eater of Worlds implicit (eldritch altar)"],
  essence: ["Essence", "essence", "Only from an essence"],
  delve: ["Delve", "delve", "Only from a fossil (Delve)"],
  veiled: ["Veiled", "veiled", "Unveiled with Jun (Betrayal)"],
  incursion: ["Incursion", "incursion", "Only from the Temple of Atzoatl (Incursion)"],
  shaper: ["Shaper", "influence-Shaper", "Shaper influence"],
  elder: ["Elder", "influence-Elder", "Elder influence"],
  crusader: ["Crusader", "influence-Crusader", "Crusader influence"],
  hunter: ["Hunter", "influence-Hunter", "Hunter influence"],
  redeemer: ["Redeemer", "influence-Redeemer", "Redeemer influence"],
  warlord: ["Warlord", "influence-Warlord", "Warlord influence"],
  infamous: ["Infamous", "mercenary", "From a mercenary (Infamous)"],
  rareonly: ["Rare only", "rareonly", "Can only roll on rare items"],
  synthesised: ["Synthesised", null, "Synthesised implicit"],
  corrupted: ["Corrupted", null, "Corruption implicit (Vaal Orb)"],
};
function SourceTag({ s }: { s: string }) {
  const x = SOURCE[s];
  if (!x) return null;
  return <span className={`tw-src tw-src-${s}`} title={x[2]}>{x[1] && <img src={`/mod-icons/${x[1]}.png`} alt="" width={16} height={16} />}{x[0]}</span>;
}
type Setup = { buyout?: boolean | null; item: Item; filters: StatFilter[]; unknown_kinds?: string[]; base_only: boolean; ilvl_min: number | null; ilvl_max?: number | null; links_min: number | null;
  gem_level: number | null; unknown: string[]; category: string | null; raw?: string; misc?: string[]; base?: string; affix_max?: [number, number] | null;
  /** Awakened-style "search as a base": why it's a crafting base, which mods to keep, item level, influences. */
  base_item?: { why: string[]; ids: string[]; ilvl_min: number | null; misc: string[] } | null;
  /** Corrupted only (true), not corrupted only (false), or either (null): set for uniques. */
  corrupted?: boolean | null };
type MapInfo = { mods: { text: string; warn: string | null }[]; unidentified: boolean; tier: number | null; stats: string[] };
type Check = { setup: Setup | null; ninja: [number, string] | null; league: string; from_ground: boolean; error: string | null; bulk?: string | null; map?: MapInfo | null; divine?: number | null; compare?: Comparison[]; instant_all?: boolean;
  /** poe.ninja's last 7 days: % change each day from a week ago (oldest first), and the total. */
  trend?: { points: number[]; change: number } | null; sold?: number | null };
type Comparison = { slot: string; current: string; rows: { stat: string; now: number; this: number }[]; gain: string[]; lose: string[] };
type Offer = { pay: number; pay_currency: string; get: number; get_currency: string; each: number; stock: number; account: string; whisper: string };
type Bulk = { offers: Offer[]; url: string; divine: number | null };
type Listing = { amount: number; currency: string; chaos: number | null; account: string; indexed: string; whisper: string; instant?: boolean; ilvl: number | null; mods?: string[] };
type Result = { listings: Listing[]; summary: [number, number] | null; total: number; url: string; divine: number | null };

const UNIT: Record<string, string> = { chaos: "c", exalted: "ex", divine: "div" };
const KIND: Record<string, string> = { pseudo: "total", implicit: "implicit", crafted: "crafted", enchant: "enchant", fractured: "fractured", mutated: "foulborn", property: "" };

/** "3h ago" from the time it was listed. */
function ago(iso: string) {
  const t = Date.parse(iso);
  if (!t) return "";
  const m = Math.max(0, Math.round((Date.now() - t) / 60000));
  return m < 60 ? `${m}m` : m < 60 * 48 ? `${Math.round(m / 60)}h` : `${Math.round(m / 1440)}d`;
}

/** Chaos, or divines when it's worth more than one. */
function money(c: number, divine: number | null) {
  if (divine && c >= divine) return `${(c / divine).toFixed(c / divine < 10 ? 1 : 0)} div`;
  return `${c < 1 ? +c.toFixed(2) : c < 10 ? +c.toFixed(1) : Math.round(c)}c`;
}

/** A price with its orb picture, e.g. "180 [chaos]". */
function Price({ n, cur }: { n: number; cur: string }) {
  const v = n < 1 ? +n.toFixed(3) : n < 10 ? +n.toFixed(2) : n < 100 ? +n.toFixed(1) : Math.round(n);
  return <span className="tw-pr">{v}{["chaos", "divine", "exalted"].includes(cur) ? <CurrencyIcon id={cur} /> : <> {UNIT[cur] ?? cur}</>}</span>;
}

/** A number that counts up to its value (a quarter second) when it first shows or changes: the big price arriving. */
function useCountUp(target: number, ms = 280) {
  const [v, setV] = useState(target);
  const from = useRef(0);
  useEffect(() => {
    const start = performance.now(), a = from.current;
    let raf = 0;
    const step = (t: number) => {
      const k = Math.min(1, (t - start) / ms);
      const e = 1 - Math.pow(1 - k, 3);
      setV(a + (target - a) * e);
      if (k < 1) raf = requestAnimationFrame(step); else from.current = target;
    };
    raf = requestAnimationFrame(step);
    return () => { cancelAnimationFrame(raf); from.current = target; };
  }, [target, ms]);
  return v;
}

function CountPrice({ n, cur }: { n: number; cur: string }) {
  return <Price n={useCountUp(n)} cur={cur} />;
}

/** A small turning ring while something loads. */
export function Spinner() { return <span className="spin" aria-hidden="true" />; }

/** The same price in the other currency: chaos -> divine, divine -> chaos. */
function Other({ n, cur, divine }: { n: number; cur: string; divine: number | null }) {
  if (!divine) return null;
  if (cur === "chaos") return n / divine < 0.01 ? null : <span className="tw-other">= <Price n={n / divine} cur="divine" /></span>;
  if (cur === "divine") return <span className="tw-other">= <Price n={n * divine} cur="chaos" /></span>;
  return null;
}

/** Every Ctrl+D searches the trade site straight away (bulk items use the exchange instead). */
/** Like Awakened: search straight away when the filters barely matter (uniques, unidentified items, gems,
 *  corpses and other items without a gear category, items read off the ground); rares, gear, jewels and maps wait
 *  for Search so you can pick the stats first. Bulk items use the exchange. Settings can make every item instant. */
const needsTrade = (c: Check) => {
  if (!c.setup || c.bulk) return false;
  const it = c.setup.item;
  return !!c.instant_all || it.rarity === "Unique" || it.unidentified || c.from_ground
    || (!c.setup.category && !c.map && it.item_class !== "Maps");
};
/** What a search depends on: when this changes after a search, the old prices no longer match the filters. */
const searchKey = (s: Setup) => JSON.stringify([s.filters.map((f) => [f.enabled, f.min, f.max ?? null]), s.base_only, s.ilvl_min, s.ilvl_max ?? null, s.links_min, s.misc ?? [], s.corrupted ?? null, s.buyout ?? null]);

/** Item-level band where base prices jump (75, 82, 84, 86 unlock better mod tiers): [low, high or null]. */
const ilvlBand = (l: number): [number, number | null] => l <= 67 ? [1, 67] : l <= 74 ? [68, 74] : l <= 81 ? [75, 81] : l <= 83 ? [82, 83] : l <= 85 ? [84, 85] : [86, null];
/** "73–74" or "86+": this item's level up to the top of its band. */
const ilvlText = (l: number) => { const top = ilvlBand(l)[1]; return top == null ? `${l}+` : top === l ? `${l}` : `${l}–${top}`; };

type Saved = { id: number; name: string; live: boolean; setup: Setup };

/** Saved searches: run again, or turn on live (pops up new listings while you play). */
function SavedPanel({ list, setList, onRun }: { list: Saved[]; setList: (l: Saved[]) => void; onRun: (s: Saved) => void }) {
  const [err, setErr] = useState("");
  const call = (cmd: string, args: Record<string, unknown>) => invoke<Saved[]>(cmd, args).then((l) => { setList(l); setErr(""); }).catch((e) => setErr(friendly(e)));
  if (list.length === 0) return <div className="muted small tw-saved">No saved searches yet: price check an item, then press Save search.</div>;
  return (
    <div className="tw-saved">
      <div className="muted small">Live searches check the trade site every 90 seconds and pop up new listings.</div>
      <ul className="tw-list">
        {list.map((x) => (
          <li key={x.id}>
            <span className="tw-saved-name" title={x.setup.filters.filter((f) => f.enabled).map((f) => f.label).join("\n")}>{x.name}</span>
            <button className="link small" onClick={() => onRun(x)}>Run</button>
            <button className={`link small ${x.live ? "tw-live-on" : ""}`} onClick={() => call("saved_live", { id: x.id, on: !x.live })}
              title={x.live ? "Live: on (click to stop)" : "Pop up new listings while you play"}>{x.live ? "● Live" : "Live"}</button>
            <button className="link small" onClick={() => call("saved_delete", { id: x.id })} title="Delete">×</button>
          </li>
        ))}
      </ul>
      {err && <div className="qm-warn">{err}</div>}
    </div>
  );
}

/** "Is this an upgrade?": against the gear in the same slot of your PoB build. */
function CompareBlock({ list }: { list: Comparison[] }) {
  const [open, setOpen] = useState(() => { try { return localStorage.getItem("tw-compare") !== "0"; } catch { return true; } });
  const toggle = () => { setOpen(!open); try { localStorage.setItem("tw-compare", open ? "0" : "1"); } catch { /* ignore */ } };
  const score = (c: Comparison) => [c.rows.filter((r) => r.this > r.now).length, c.rows.filter((r) => r.this < r.now).length];
  return (
    <div className="tw-cmp">
      <button className="tw-cmp-toggle" onClick={toggle}>
        <span className="tw-sec">{open ? "▾" : "▸"} Against your build</span>
        {!open && list.map((c) => { const [u, d] = score(c);
          return <Verdict key={c.slot} up={u} down={d} />; })}
      </button>
      {open && list.map((c) => {
        const [up, down] = score(c);
        return (
          <div key={c.slot} className="tw-cmp-slot">
            <div className="tw-cmp-head">
              <span className="small"><span className="muted">{c.slot}:</span> {c.current}</span>
              <Verdict up={up} down={down} />
            </div>
            {c.rows.length > 0 && (
              <table className="tw-cmp-rows">
                <thead><tr><th></th><th>yours</th><th>this</th><th></th></tr></thead>
                <tbody>
                  {c.rows.map((r) => {
                    const d = r.this - r.now;
                    return <tr key={r.stat}><td>{r.stat}</td><td className="muted">{r.now}</td><td>{r.this}</td>
                      <td className={d > 0 ? "up" : "down"}>{d > 0 ? "▲ " : "▼ "}{Math.abs(d)}</td></tr>;
                  })}
                </tbody>
              </table>
            )}
            <ModList title="Only on this item" lines={c.gain} cls="up" mark="+" />
            <ModList title="You'd lose" lines={c.lose} cls="down" mark="−" />
          </div>
        );
      })}
    </div>
  );
}

/** How high a mod rolled: a small bar from its lowest to its highest roll, and the range itself. */
function Roll({ r, range, tier }: { r: number; range?: string | null; tier: number | null }) {
  const pct = Math.round(r * 100);
  const cls = pct >= 80 ? "hi" : pct >= 40 ? "mid" : "lo";
  const word = pct >= 80 ? "high roll" : pct >= 40 ? "mid roll" : "low roll";
  return (
    <span className={`tw-roll ${cls}`} title={`${word}: ${pct}% of the way from the lowest to the highest roll${tier != null ? ` for tier ${tier}` : ""}${range ? ` (${range})` : ""}`}>
      <span className="tw-roll-bar"><span style={{ width: `${Math.max(4, pct)}%` }} /></span>
      {range && <span className="tw-roll-range">{range}</span>}
    </span>
  );
}

/** Tier 1 is the best roll a mod can have: green for top tiers through to red for low ones. */
const tierClass = (t: number) => (t <= 1 ? "t1" : t === 2 ? "t2" : t === 3 ? "t3" : t <= 5 ? "t45" : "t6");

/** The going rate for a bulk item, like Awakened: price fixers list a handful at a silly price to drag the
 *  number down, so weigh every offer by how many it has in stock and take the middle (a stock-weighted median).
 *  Offers well under that with little stock are marked as likely fixers. */
function bulkTypical(offers: Offer[], divine: number | null, market: number | null = null) {
  const chaos = (o: Offer) => (o.pay_currency === "divine" && divine ? o.each * divine : o.each);
  const rows = offers.map((o) => ({ c: chaos(o), stock: Math.max(1, o.stock || 1) })).sort((a, b) => a.c - b.c);
  const total = rows.reduce((s, r) => s + r.stock, 0);
  let acc = 0, typical = rows[0]?.c ?? 0;
  for (const r of rows) { acc += r.stock; if (acc >= total / 2) { typical = r.c; break; } }
  const medianStock = [...rows].map((r) => r.stock).sort((a, b) => a - b)[Math.floor(rows.length / 2)] ?? 1;
  // Judged against the market price when poe.ninja has one (fixers can fill the whole cheap end of the list).
  const ref = market ?? typical;
  const fixer = (o: Offer) => chaos(o) < ref * 0.75 && (o.stock || 1) < Math.max(5, medianStock / 2);
  return { typical, cheapest: rows[0]?.c ?? 0, fixer };
}

/** Fewer listings than this: the price box says "low confidence" instead of "sells for about". */
const LOW_CONFIDENCE = 5;

/** The price window's two layouts (Settings, or "Layout" under More): headings, or one flat list with a tag on each row. */
export const TW_LAYOUTS: [string, string, string][] = [
  ["grouped", "Grouped", "Headings: Totals, Base mods, Prefixes 2 of 3 used, Suffixes…"],
  ["flat", "Flat", "One list like Awakened, a small tag on each row; mods already counted in a total are tucked away"],
];
export const TW_LAYOUT_KEY = "tw.layout";

const isOpenSlot = (f: StatFilter) => f.id.includes("empty_prefix") || f.id.includes("empty_suffix");

/** How many prefixes / suffixes the item has, from the open-slot rows (none: it's full). */
function affixUsed(fs: StatFilter[], max: [number, number]): [number, number] {
  const free = (side: string) => fs.find((f) => f.id.includes(`empty_${side}`))?.value ?? 0;
  return [Math.max(0, max[0] - free("prefix")), Math.max(0, max[1] - free("suffix"))];
}

/** Rows in the order shown: the open prefix / suffix rows move to the end of their own group. */
function displayOrder(idx: number[], fs: StatFilter[]): number[] {
  const out = idx.filter((i) => !isOpenSlot(fs[i]));
  for (const side of ["prefix", "suffix"]) {
    const o = idx.find((i) => fs[i].id.includes(`empty_${side}`));
    if (o == null) continue;
    let at = -1;
    out.forEach((i, k) => { if (fs[i].affix === side) at = k; });
    if (at < 0) {
      // None of that side yet: a lone open prefix goes before the suffixes, a lone open suffix after the prefixes.
      if (side === "prefix") { const sx = out.findIndex((i) => fs[i].affix === "suffix"); at = sx >= 0 ? sx - 1 : out.length - 1; }
      else out.forEach((i, k) => { if (fs[i].affix === "prefix" || isOpenSlot(fs[i])) at = k; });
      if (at < 0) at = out.length - 1;
    }
    out.splice(at + 1, 0, o);
  }
  return out;
}

/** Flat layout: the small tag on the right of a row. */
function rowTag(f: StatFilter): string {
  if (isOpenSlot(f)) return "open slot";
  if (f.kind === "pseudo") return "total";
  if (f.kind === "property") return f.id.startsWith("prop.map_") ? "map" : f.id === "prop.quality" ? "" : f.id.endsWith("_influence") ? "influence" : "20% quality";
  if (f.kind === "implicit") return "base mod";
  if (f.kind === "enchant") return "enchant";
  const how = f.kind === "crafted" ? "crafted " : f.kind === "fractured" ? "fractured " : f.kind === "mutated" ? "foulborn " : "";
  return f.affix ? how + f.affix : how.trim();
}

/** Flat layout: a mod already counted in one of the totals (fire resistance inside "total Elemental Resistance",
 *  life inside "total maximum Life"), tucked away unless ticked. */
function inATotal(f: StatFilter, fs: StatFilter[]): boolean {
  if (f.kind === "pseudo" || f.kind === "property" || f.enabled) return false;
  const totals = fs.filter((x) => x.kind === "pseudo" && !isOpenSlot(x)).map((x) => x.label);
  return [/Resistance/, /maximum Life/, /maximum Mana/, /Strength/, /Dexterity/, /Intelligence/]
    .some((k) => k.test(f.label) && totals.some((t) => k.test(t)));
}

/** The small heading over a row, and its hint: [title, hint]. `used`/`max` = prefixes and suffixes (Cogwork Ring holds 2 / 4). */
function rowGroup(f: StatFilter, max: [number, number], used: [number, number] = max): [string, string] {
  if (f.id.startsWith("prop.map_")) return ["Map", "at least this much"];
  if (f.id === "prop.area_level" || f.id.startsWith("prop.heist_")) return ["Heist", "at least this much"];
  if (f.id.endsWith("_influence")) return ["Influence", "tick to only find items with it"];
  if (f.id === "prop.quality") return ["Quality", "above the usual 20%"];
  if (f.id.startsWith("prop.")) return [f.id.includes("dps") ? "Damage" : "Defences", "at 20% quality"];
  if (f.id.includes("empty_prefix")) return ["Prefixes", `${used[0]} of ${max[0]} used`];
  if (f.id.includes("empty_suffix")) return ["Suffixes", `${used[1]} of ${max[1]} used`];
  if (f.kind === "pseudo") return ["Totals", "all mods added up"];
  if (f.kind === "enchant") return ["Enchant", ""];
  if (f.kind === "implicit") return ["Base mods", "implicit, come with the item"];
  if (f.affix === "prefix") return ["Prefixes", `${used[0]} of ${max[0]} used`];
  if (f.affix === "suffix") return ["Suffixes", `${used[1]} of ${max[1]} used`];
  return ["Other mods", ""];
}

/** A search found nothing: offers to try it without each ticked stat, then lists what each would find. */
function UntickHelp({ state, onAsk, onPick }: { state: null | "busy" | string | { key: string; label: string; total: number }[];
  onAsk: () => void; onPick: (key: string) => void }) {
  if (state === null) return (
    <button className="tw-untick-ask" onClick={onAsk}
      title="Searches again once without each ticked stat (one trade search each) and shows how many each would find">
      Which stat to untick?</button>
  );
  if (state === "busy") return <div className="tw-untick muted small">Trying the search without each stat…</div>;
  if (typeof state === "string") return <div className="tw-untick qm-warn">{state}</div>;
  const found = state.filter((t) => t.total > 0);
  return (
    <div className="tw-untick">
      {found.length === 0 && <div className="muted small">Still nothing with any one stat unticked: untick two or more.</div>}
      {found.map((t) => (
        <button key={t.key} onClick={() => onPick(t.key)} title="Untick it and search">
          Untick <b>{t.label}</b> <span className="tw-untick-n">{t.total} for sale</span></button>
      ))}
    </div>
  );
}

/** Awakened's "market ratio" row: poe.ninja's price, placed among the offers so you can see who's under or over it. */
function MarketRatio({ c, divine }: { c: number; divine: number | null }) {
  return (
    <li className="tw-ratio-row" title="poe.ninja's price, from real trades on the in-game Currency Exchange">
      <span className="tw-price"><Price n={c} cur="chaos" /></span>
      <span className="small">{divine && c / divine >= 0.01 ? <Other n={c} cur="chaos" divine={divine} /> : null}</span>
      <span className="small">market price (poe.ninja)</span>
      <span />
    </li>
  );
}

/** The answer first, big and clear: what this sells for. */
/** poe.ninja's price over the last 7 days: a small line and the week's change (green up, red down). */
function TrendLine({ t }: { t: { points: number[]; change: number } }) {
  const pts = t.points;
  const lo = Math.min(...pts), hi = Math.max(...pts), span = hi - lo || 1;
  const W = 120, H = 26;
  const xy = pts.map((v, i) => [(i / (pts.length - 1)) * W, H - 3 - ((v - lo) / span) * (H - 6)]);
  const d = xy.map(([x, y], i) => (i ? "L" : "M") + x.toFixed(1) + " " + y.toFixed(1)).join(" ");
  const up = t.change > 0.5, down = t.change < -0.5;
  const word = up ? "up" : down ? "down" : "steady";
  return (
    <div className={`tw-trend ${up ? "up" : down ? "down" : "flat"}`} title={"poe.ninja's price over the last 7 days, day by day"}>
      <span className="tw-trend-label">This week</span>
      <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
        <path d={d} fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" strokeLinecap="round" />
        <circle cx={xy[xy.length - 1][0]} cy={xy[xy.length - 1][1]} r="2.6" fill="currentColor" />
      </svg>
      <span className="tw-trend-pct">{up ? "▲" : down ? "▼" : ""} {Math.abs(t.change).toFixed(0)}% {word}</span>
    </div>
  );
}

function PriceHero({ res, bulk, busy, err, divine, ninja, ninjaNote, waiting, stack, name }: { res: Result | null; bulk: Bulk | null; busy: boolean; err: string; waiting?: string | null; stack?: number; name?: string;
  divine: number | null; ninja: [number, string] | null; ninjaNote: string }) {
  const ninjaChaos = ninja ? (ninja[1] === "divine" && divine ? ninja[0] * divine : ninja[1] === "chaos" ? ninja[0] : null) : null;
  let main: number | null = null, label = "", sub: React.ReactNode = null, source = "", low = false;
  if (res?.summary) {
    main = res.summary[1];
    const ps = res.listings.map((l) => l.chaos).filter((x): x is number => x != null);
    const top = ps.length ? Math.max(...ps) : null;
    if (res.total < LOW_CONFIDENCE) {
      // A handful of listings is a few people's asking prices, not a price (his Dragon Turn: one at 45 div).
      label = "low confidence"; low = true;
      sub = <>only {res.total} listed{res.total > 1 ? <> · {money(res.summary[0], divine)}{top != null && top > res.summary[0] ? <> to {money(top, divine)}</> : null}</> : null}</>;
    } else {
      label = "sells for about";
      sub = <>{money(res.summary[0], divine)}{top != null && top > res.summary[0] ? <> to {money(top, divine)}</> : null} · {res.total.toLocaleString()} listed</>;
    }
  } else if (bulk?.offers[0]) {
    // Like Awakened: the headline is poe.ninja's price (from real in-game Currency Exchange trades, which price
    // fixers can't move); our stock-weighted going rate only when poe.ninja doesn't have the item.
    const t = bulkTypical(bulk.offers, divine, ninjaChaos);
    const o = bulk.offers[0];
    main = ninjaChaos ?? t.typical; label = "each";
    source = ninjaChaos != null ? "poe.ninja's price, from real trades on the in-game Currency Exchange" : "Going rate on the trade site's exchange, weighted by stock";
    sub = <>cheapest listed {money(t.cheapest, divine)}{o.stock ? ` (${o.stock} in stock)` : ""}
      {ninjaChaos == null && <> · going rate {money(t.typical, divine)}</>} · {bulk.offers.length} sellers</>;
  } else if (ninjaChaos != null) {
    main = ninjaChaos; label = `poe.ninja${ninjaNote}`;
  }
  // With a stack (e.g. 732 lifeforce): two tiles, "your 732" (bright) and "each". The ratio line ("11,880 Wild
  // Crystallised Lifeforce = 1 [divine]") is for bulk items: how they're traded.
  const many = stack != null && stack > 1 && main != null;
  const big = main != null ? (many ? main * stack! : main) : null;
  const showDiv = big != null && divine && big >= divine;
  // Chaos, or divines once it's worth one, always with the orb picture.
  const amount = (c: number) => (divine && c >= divine ? <Price n={c / divine} cur="divine" /> : <Price n={c} cur="chaos" />);
  const perDiv = bulk && main && divine ? divine / main : null;
  return (
    <div className={`tw-hero ${main == null ? "empty" : ""} ${low ? "low" : ""}`}>
      {main != null && big != null && many ? (
        <div className="tw-tiles2" title={source || undefined}>
          <div className="tw-tile2 hi"><div className="tw-tile2-lab">YOUR {stack!.toLocaleString()}</div><div className="tw-tile2-val">{amount(big)}</div>
            {divine && big < divine && big / divine >= 0.01 && <div className="tw-tile2-sub">= <Price n={big / divine} cur="divine" /></div>}</div>
          <div className="tw-tile2"><div className="tw-tile2-lab">EACH</div><div className="tw-tile2-val">{amount(main)}</div></div>
        </div>
      ) : main != null && big != null ? (
        <div className="tw-hero-main">
          <span className="tw-hero-price" title={source || undefined}>{showDiv ? <CountPrice n={big / divine!} cur="divine" /> : <CountPrice n={big} cur="chaos" />}</span>
          <span className="tw-hero-side">
            <span className={low ? "tw-hero-label tw-low-tag" : "tw-hero-label"}>{label}</span>
            {divine && <span className="muted">{showDiv ? <>= <Price n={big} cur="chaos" /></> : big / divine >= 0.01 ? <>= <Price n={big / divine} cur="divine" /></> : null}</span>}
          </span>
        </div>
      ) : (
        <div className="tw-hero-main"><span className="tw-hero-label">
          {busy ? <><Spinner /> Checking the trade site…</> : res && res.total === 0 ? "None listed like this: untick a stat or two" : res ? `${res.total} listed, none priced` : err ? "No trade price" : waiting ?? "No price yet"}</span></div>
      )}
      {perDiv != null && name && (
        <div className="tw-perdiv">{perDiv >= 1
          ? <><b>{perDiv >= 10 ? Math.round(perDiv).toLocaleString() : +perDiv.toFixed(1)}</b> {name} = 1 <CurrencyIcon id="divine" /></>
          : <>1 {name} = <b>{+(1 / perDiv).toFixed(1)}</b> <CurrencyIcon id="divine" /></>}</div>
      )}
      <div className="tw-hero-foot muted small">
        {sub}
        {busy && main != null && <span className="tw-hero-busy"> · <Spinner /> checking trade…</span>}
        {res?.summary && ninjaChaos != null && <span> · poe.ninja {money(ninjaChaos, divine)}</span>}
      </div>
    </div>
  );
}

/** "4 better · 1 worse" as a coloured badge. */
function Verdict({ up, down }: { up: number; down: number }) {
  const cls = up > down ? "up" : down > up ? "down" : "";
  return <span className={`tw-verdict ${cls}`}>{up > 0 && `${up} better`}{up > 0 && down > 0 && " · "}{down > 0 && `${down} worse`}{!up && !down && "same stats"}</span>;
}

/** Other mods, one per line; long lists fold after four. */
function ModList({ title, lines, cls, mark }: { title: string; lines: string[]; cls: string; mark: string }) {
  const [all, setAll] = useState(false);
  if (lines.length === 0) return null;
  const shown = all ? lines : lines.slice(0, 4);
  return (
    <div className="tw-cmp-mods">
      <div className={`tw-cmp-mods-title ${cls}`}>{title}</div>
      <ul>{shown.map((l, i) => <li key={i}><span className={cls}>{mark}</span> {l}</li>)}</ul>
      {lines.length > 4 && <button className="link small" onClick={() => setAll(!all)}>{all ? "show fewer" : `+ ${lines.length - 4} more`}</button>}
    </div>
  );
}

/** Ctrl+D on a map: its mods, the ones you can't run in red. */
function MapCheck({ map, rarity, corrupted }: { map: MapInfo; rarity: string; corrupted: boolean }) {
  const bad = map.mods.filter((m) => m.warn);
  const [copied, setCopied] = useState("");
  // The one-word answer first (22 Nov): run it or not, and what to do if not.
  const fix = corrupted ? "It's corrupted: put it aside (or sell it)."
    : rarity === "Rare" ? "Reroll it with a Chaos Orb (or Scour + Alchemy)."
    : rarity === "Magic" ? "Reroll it with an Orb of Alteration."
    : "";
  return (
    <div className="tw-map">
      {!map.unidentified && <div className={`tw-map-verdict ${bad.length ? "bad" : "ok"}`}>
        <b>{bad.length ? "Don't run" : "Safe to run"}</b>{bad.length > 0 && fix && <span className="small"> · {fix}</span>}
      </div>}
      <div className={`tw-map-head ${bad.length ? "bad" : map.unidentified ? "" : "ok"}`}>
        {map.unidentified ? "Unidentified: its mods are hidden until you identify it."
          : bad.length ? `⚠ ${bad.length} mod${bad.length > 1 ? "s" : ""} you can't run` : "✓ Nothing on your can't-run list"}
        {map.tier != null && <span className="muted small"> · tier {map.tier}</span>}
      </div>
      {map.stats.length > 0 && <div className="muted small tw-map-stats">{map.stats.join(" · ")}</div>}
      <ul className="tw-map-mods">
        {map.mods.map((m, i) => (
          <li key={i} className={m.warn ? "bad" : ""}>{m.text}{m.warn && <span className="tw-map-tag">{m.warn}</span>}</li>
        ))}
      </ul>
      <div className="row">
        <button className="small" onClick={() => invoke<string>("map_regex_copy").then((r) => setCopied(r ? "Copied: paste it in your map stash search" : "Nothing to copy yet: set it up on the Map mods page")).catch((e) => setCopied(String(e)))}>
          Copy map regex</button>
        {copied && <span className="muted small">{copied}</span>}
      </div>
      <div className="muted small">Change which mods you can't run on the Map mods page: this warning and the stash search use the same list.</div>
    </div>
  );
}

/** Looks you can switch between (Settings, or the Style button at the bottom of the window). Shared via localStorage. */
export const TW_STYLES: [string, string, string][] = [
  ["classic", "Classic", "The original layout"],
  ["tooltip", "Tooltip", "Your item drawn like the game tooltip, the price at the bottom"],
  ["split", "Split", "Your item on the left, the market beside it"],
  ["compare", "Compare", "Each listing's mods next to yours: green is better, red is worse"],
  ["price", "Price first", "The price big at the top, with how listings spread"],
  // 22 Nov 2026: Awakened PoE Trade's layout made nicer (GROBBS: everything you need one click away, game icons).
  ["awakened", "Awakened", "Awakened's layout: item, filter chips, stats, listings; every button on show"],
  ["awakened-price", "Awakened + price", "Awakened's layout with a price bar under the item: cheapest, typical, how many listed"],
  ["awakened-tooltip", "Awakened tooltip", "Awakened's layout inside the game's tooltip frame"],
];

/** Small line pictures for the Awakened looks' bottom bar. */
const FOOT_ICO: Record<string, string> = {
  site: "M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
  craft: "M14 4l6 6-3 3-6-6zM11 7L4 14l2 2 7-7",
  saved: "M6 4h12v16l-6-4-6 4z",
  save: "M12 5v14M5 12h14",
  style: "M12 3a9 9 0 1 0 0 18c1 0 1.5-.8 1.2-1.6-.4-1 .3-2 1.4-2H17a4 4 0 0 0 4-4c0-5.5-4-10.4-9-10.4z",
  layout: "M4 6h16M4 12h16M4 18h16",
  report: "M5 21V4M5 4h11l-2 4 2 4H5",
};
function FootIco({ k }: { k: string }) {
  return <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d={FOOT_ICO[k]} /></svg>;
}
export const TW_STYLE_KEY = "tw.style";

/** "+52 to Strength" -> ["+# to strength", 52]. */
function modShape(t: string): [string, number | null] {
  const m = t.match(/-?\d+(\.\d+)?/);
  return [t.replace(/\d+(\.\d+)?/g, "#").replace(/^[+-]/, "").toLowerCase(), m ? +m[0] : null];
}

/** A listing's mods against yours: better (green), worse (red), same, or not on yours (grey). */
function ListingMods({ mods, yours }: { mods: string[]; yours: StatFilter[] }) {
  const mine = new Map(yours.filter((f) => f.kind !== "pseudo" && !f.id.startsWith("prop.")).map((f) => { const [s] = modShape(f.label); return [s, f.value] as const; }));
  if (!mods.length) return null;
  // Two rows: the mods you also have (compared), then the rest of theirs.
  const rated = mods.map((m) => {
    const [s, v] = modShape(m);
    const y = mine.get(s);
    return { m, y, cls: y == null || v == null ? "other" : v > y ? "better" : v < y ? "worse" : "same" };
  });
  const shared = rated.filter((r) => r.cls !== "other");
  const rest = rated.filter((r) => r.cls === "other");
  const row = (label: string, list: typeof rated) => list.length > 0 && (
    <div className="tw-lmods-row"><span className="tw-lmods-lab">{label}</span>
      <span className="tw-lmods-chips">{list.map((r, i) => <span key={i} className={`tw-lmod ${r.cls}`} title={r.y == null ? "Not on your item" : `Yours: ${r.y}`}>{r.m}</span>)}</span></div>
  );
  return (
    <div className="tw-lmods">
      {row("Like yours", shared)}
      {row("Also has", rest)}
    </div>
  );
}

/** Where the listings' prices sit (a small bar chart), with the typical price marked. */
function PriceSpread({ res, divine }: { res: Result | null; divine: number | null }) {
  const ps = (res?.listings ?? []).map((l) => l.chaos).filter((x): x is number => x != null).sort((a, b) => a - b);
  if (ps.length < 3) return null;
  const lo = ps[0], hi = ps[ps.length - 1];
  const bins = 10;
  const counts = Array.from({ length: bins }, () => 0);
  ps.forEach((p) => { counts[Math.min(bins - 1, Math.floor(((p - lo) / Math.max(1e-9, hi - lo)) * bins))] += 1; });
  const top = Math.max(...counts);
  const typ = res?.summary?.[1];
  const mark = typ != null ? Math.min(bins - 1, Math.floor(((typ - lo) / Math.max(1e-9, hi - lo)) * bins)) : -1;
  return (
    <div className="tw-spread" title="How the cheapest listings' prices spread out; the gold bar holds the typical price">
      <div className="tw-spread-bars">{counts.map((n, i) => <i key={i} className={i === mark ? "me" : n === 0 ? "z" : ""} style={{ height: `${Math.max(8, (n / top) * 100)}%` }} />)}</div>
      <div className="tw-spread-ends"><span>{money(lo, divine)}</span><span>listing prices</span><span>{money(hi, divine)}</span></div>
    </div>
  );
}

/** The market at a glance, for the Split look's side panel. */
function MarketStats({ res, divine }: { res: Result; divine: number | null }) {
  const ps = res.listings.map((l) => l.chaos).filter((x): x is number => x != null);
  // With the orb pictures: divines once it's worth one, chaos below that.
  const m = (c: number) => (divine && c >= divine ? <Price n={c / divine} cur="divine" /> : <Price n={c} cur="chaos" />);
  return (
    <div className="tw-mstats">
      {res.summary && <div><span>Cheapest</span><b>{m(res.summary[0])}</b></div>}
      {res.summary && <div><span>Typical</span><b>{m(res.summary[1])}</b></div>}
      {ps.length > 0 && <div><span>Dearest shown</span><b>{m(Math.max(...ps))}</b></div>}
      <div><span>Listed</span><b>{res.total.toLocaleString()}</b></div>
      <div><span>Online now</span><b>{res.listings.filter((l) => l.instant || l.whisper).length}</b></div>
    </div>
  );
}

/** The Ctrl+D price window. */
export default function TradeWindow() {
  const [check, setCheck] = useState<Check | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [res, setRes] = useState<Result | null>(null);
  // "Which stat to untick?": the search tried once without each ticked stat (only when you ask).
  const [untick, setUntick] = useState<null | "busy" | string | { key: string; label: string; total: number }[]>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [copied, setCopied] = useState<string | null>(null);
  const [bulk, setBulk] = useState<Bulk | null>(null);
  const [pay, setPay] = useState<"chaos" | "divine">("chaos");
  // Awakened's stock filter: only sellers with at least as many as your stack (off by default).
  const [minStock, setMinStock] = useState(false);
  // Likely price fixers are hidden until you ask to see them.
  const [showFixers, setShowFixers] = useState(false);
  const [saved, setSaved] = useState<Saved[]>([]);
  const [showSaved, setShowSaved] = useState(false);
  /** The footer's "More": saved searches, Style and Something's wrong. */
  const [more, setMore] = useState(false);
  const [layout, setLayout] = useState<string>(() => { try { return localStorage.getItem(TW_LAYOUT_KEY) || "grouped"; } catch { return "grouped"; } });
  useEffect(() => {
    const on = (e: StorageEvent) => { if (e.key === TW_LAYOUT_KEY && e.newValue) setLayout(e.newValue); };
    window.addEventListener("storage", on);
    return () => window.removeEventListener("storage", on);
  }, []);
  const nextLayout = () => { const n = layout === "flat" ? "grouped" : "flat"; setLayout(n); try { localStorage.setItem(TW_LAYOUT_KEY, n); } catch { /* not kept */ } };
  /** Flat layout: show the mods that are already counted in a total. */
  const [showInTotals, setShowInTotals] = useState(false);
  /** Base mods that are the same on every item of this base are tucked away (like Awakened's "Show hidden"). */
  const [showFixed, setShowFixed] = useState(false);
  const [saving, setSaving] = useState<string | null>(null);
  const [preset, setPreset] = useState<number | null>(0);
  // Awakened-style tabs: price this item, or price it as a crafting base. Your own search is kept while you look.
  const [asBase, setAsBase] = useState(false);
  const [ownSetup, setOwnSetup] = useState<Setup | null>(null);
  const [mapMods, setMapMods] = useState(false);
  const [reporting, setReporting] = useState<string | null>(null);
  const [reported, setReported] = useState("");
  const [hasSide, setHasSide] = useState(false);
  const [searched, setSearched] = useState<string | null>(null);
  // "Also searching for": what the search asks besides the ticked rows (Awakened's hidden filters), in plain words.
  const [hidden, setHidden] = useState<string[]>([]);
  const [showHidden, setShowHidden] = useState<boolean>(() => { try { return localStorage.getItem("tw-hidden") === "1"; } catch { return false; } });
  const hiddenKey = setup ? searchKey(setup) : "";
  useEffect(() => {
    if (!setup || check?.bulk) { setHidden([]); return; }
    invoke<string[]>("trade_hidden", { setup }).then(setHidden).catch(() => setHidden([]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hiddenKey, check?.bulk]);
  const toggleHidden = () => { const n = !showHidden; setShowHidden(n); try { localStorage.setItem("tw-hidden", n ? "1" : "0"); } catch { /* not kept */ } };
  // Like Awakened: change a filter after a search and the old prices go, until you search again.
  useEffect(() => { if (res && setup && searched && searchKey(setup) !== searched) setRes(null); }, [setup, res, searched]);
  const box = useRef<HTMLDivElement | null>(null);
  const [style, setStyle] = useState<string>(() => { try { return localStorage.getItem(TW_STYLE_KEY) || "classic"; } catch { return "classic"; } });
  useEffect(() => {
    const on = (e: StorageEvent) => { if (e.key === TW_STYLE_KEY && e.newValue) setStyle(e.newValue); };
    window.addEventListener("storage", on);
    return () => window.removeEventListener("storage", on);
  }, []);
  const nextStyle = () => {
    const i = TW_STYLES.findIndex(([k]) => k === style);
    const n = TW_STYLES[(i + 1) % TW_STYLES.length][0];
    setStyle(n);
    try { localStorage.setItem(TW_STYLE_KEY, n); } catch { /* not kept */ }
  };

  const close = useCallback(() => { setCheck(null); setRes(null); setBulk(null); setErr(""); invoke("hide_trade_window"); }, []);

  const runBulk = useCallback(async (want: string, p: "chaos" | "divine", minimum: number | null = null) => {
    setBusy(true); setErr(""); setBulk(null);
    try {
      let b = await invoke<Bulk>("trade_bulk", { want, pay: p, minimum });
      // Pricier items are usually only sold for divines: when chaos finds nobody, try divines.
      if (b.offers.length === 0 && p === "chaos") {
        const d = await invoke<Bulk>("trade_bulk", { want, pay: "divine", minimum }).catch(() => null);
        if (d && d.offers.length) { b = d; setPay("divine"); }
      }
      setBulk(b);
    } catch (e) { setErr(friendly(e)); }
    setBusy(false);
  }, []);

  const run = useCallback(async (s: Setup) => {
    setBusy(true); setErr(""); setRes(null); setUntick(null); setSearched(searchKey(s));
    try { setRes(await invoke<Result>("trade_search", { setup: s })); } catch (e) { setErr(friendly(e)); }
    setBusy(false);
  }, []);

  useEffect(() => {
    const a = listen<Check>("price-check", (e) => {
      const c = e.payload;
      setCheck(c); setSetup(c.setup); setRes(null); setSearched(null); setBulk(null); setErr(c.error ? friendly(c.error) : ""); setCopied(null); setPay("chaos"); setMinStock(false); setShowFixers(false);
      setSaving(null); setShowSaved(!c.setup); setPreset(null); setMapMods(false); setReporting(null); setReported(""); setAsBase(false); setOwnSetup(null); setUntick(null); setShowFixed(false); window.scrollTo(0, 0);
      invoke<Saved[]>("saved_list").then(setSaved).catch(() => {});
      if (c.bulk) {
        // Worth half a divine or more: sellers mostly take divines, so ask for those first.
        const nin = c.ninja ? (c.ninja[1] === "divine" && c.divine ? c.ninja[0] * c.divine : c.ninja[1] === "chaos" ? c.ninja[0] : 0) : 0;
        const p = c.divine && nin >= c.divine * 0.5 ? "divine" : "chaos";
        setPay(p); runBulk(c.bulk, p);
      }
      else if (c.setup && needsTrade(c)) run(c.setup);
    });
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
    const onBlur = () => close();
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    return () => { a.then((f) => f()); window.removeEventListener("keydown", onKey); window.removeEventListener("blur", onBlur); };
  }, [close, run, runBulk]);

  // Fit the window to what's shown.
  useEffect(() => {
    if (!box.current) return;
    const ro = new ResizeObserver(() => {
      const h = Math.ceil(box.current?.getBoundingClientRect().height ?? 300) + 4;
      // As tall as the screen allows; past that the window scrolls.
      const cap = Math.max(400, (window.screen.availHeight || 900) - 40);
      // Wider when the related-items panel sits beside the prices.
      const w = Math.ceil(box.current?.getBoundingClientRect().width ?? 370) + 10;
      getCurrentWindow().setSize(new LogicalSize(Math.max(380, w), Math.max(100, Math.min(cap, h)))).then(() => invoke("keep_on_screen")).catch(() => {});
    });
    ro.observe(box.current);
    return () => ro.disconnect();
  });

  const runSaved = (x: Saved) => {
    setCheck({ setup: x.setup, ninja: null, league: check?.league ?? "", from_ground: false, error: null });
    setSetup(x.setup); setBulk(null); setShowSaved(false); run(x.setup);
  };
  const savedToggle = (
    <button className="link small tw-saved-toggle" onClick={() => setShowSaved(!showSaved)}>
      {showSaved ? "Hide saved searches" : `Saved searches (${saved.length})`}{saved.some((x) => x.live) ? " · live on" : ""}</button>
  );
  const savedPanel = showSaved && <SavedPanel list={saved} setList={setSaved} onRun={runSaved} />;
  const savedBlock = <>{savedToggle}{savedPanel}</>;

  if (!check) return null;
  if (!setup) return <div className="qm tw" ref={box}><div className="qm-fail">{err || "Nothing to price."}</div>{savedBlock}</div>;

  const it = setup.item;
  // Awakened-style side panel: a fragment's set or parts, lifeforce types, what a boss drops (shown when there are any).
  const relName = (it.rarity === "Unique" ? it.name : it.base_type) || null;
  // Maps: priced by tier and rarity; their mods are behind a link.
  const showStats = !check.map || mapMods;
  // Maps: their drop bonuses (quantity, atlas drops, pack size…) always; the map mods behind a link.
  // "As a base": only the rows the base search uses (the rest come back on "This item").
  const shown = (x: StatFilter) => (!asBase || !!setup.base_item?.ids.includes(x.id)) && (showStats || x.kind === "property" || x.kind === "pseudo");
  const mapModCount = setup.filters.filter((x) => !(x.kind === "property" || x.kind === "pseudo")).length;
  const patch = (i: number, p: Partial<StatFilter>) => { if ("min" in p) setPreset(null); return setSetup({ ...setup, filters: setup.filters.map((f, j) => (j === i ? { ...f, ...p } : f)) }); };
  const divine = res?.divine ?? bulk?.divine ?? check?.divine ?? null;
  const stack = it.stack_size ?? 1;
  const ninjaC = check.ninja ? (check.ninja[1] === "divine" && divine ? check.ninja[0] * divine : check.ninja[1] === "chaos" ? check.ninja[0] : null) : null;
  const bt = bulk?.offers.length ? bulkTypical(bulk.offers, bulk.divine, ninjaC) : null;
  const fixers = bulk && bt ? bulk.offers.filter((o) => bt.fixer(o)).length : 0;
  // Where poe.ninja's price sits among the offers (Awakened's "market ratio" row).
  const ratioAt = bulk && ninjaC != null ? (() => { const i = bulk.offers.findIndex((o) => (o.pay_currency === "divine" && bulk.divine ? o.each * bulk.divine : o.each) >= ninjaC); return i === -1 ? bulk.offers.length : i; })() : -1;
  // Item listings: the same seller at the same price several times in a row shows once, with "× N" (like Awakened).
  // The item level column only when the listings' levels differ.
  const ilvls = new Set((res?.listings ?? []).map((l) => l.ilvl).filter(Boolean)).size > 1;
  const collapsed = (res?.listings ?? []).reduce<{ l: Listing; n: number; i: number }[]>((acc, l, i) => {
    const last = acc[acc.length - 1];
    if (last && last.l.account === l.account && last.l.amount === l.amount && last.l.currency === l.currency) last.n += 1; else acc.push({ l, n: 1, i });
    return acc;
  }, []);
  const copy = (text: string, key: string) => { invoke("copy_text", { text }); setCopied(key); };
  // Mouse wheel on a minimum: 1 at a time (Shift: 10).
  const wheel = (i: number, f: StatFilter, e: React.WheelEvent) => {
    const step = (e.shiftKey ? 10 : 1) * (e.deltaY < 0 ? 1 : -1);
    const v = Math.max(0, (f.min ?? f.value) + step);
    patch(i, f.max != null ? { min: v, max: v, enabled: true } : { min: v, enabled: true });
  };

  const hero = (
    <PriceHero res={res} bulk={check.bulk ? bulk : null} busy={busy} err={err} divine={divine}
      waiting={!res && !busy && !check.bulk ? (searched ? "Filters changed: move onto Search again" : "Pick the stats, then move your mouse onto Search") : null}
      ninja={check.ninja} ninjaNote={it.rarity === "Unique" && !it.name ? " (best ground unique on this base)" : ""} stack={it.stack_size ?? undefined} name={it.base_type} />
  );
  const trend = check.trend ? <TrendLine t={check.trend} /> : null;
  // Which look: where the price sits, and the extras each look adds.
  const heroTop = style === "classic" || style === "compare" || style === "awakened-price";
  // The Awakened looks: filter chips under the item, nothing tucked behind "More" or "Also searching for".
  const awk = style.startsWith("awakened");
  const saveBtn = <button className="link small" onClick={() => setSaving(saving == null ? itemTitle(it) : null)} title="Keep this search to run again or watch live">
    {awk && <FootIco k="save" />}Save search</button>;
  return (
    <div className={`tw-wrap tws-${style}`} ref={box}>
    <div className="qm tw">
      {style === "price" && <div className="tw-pf">{hero}<PriceSpread res={res} divine={divine} />{trend}</div>}
      <div className="qm-head with-icon">
        <ItemIcon name={it.rarity === "Unique" ? it.name : null} base={it.base_type || null} size={36} />
        <div className="qm-head-text">
          <div className="qm-name" style={{ color: RARITY_COLOUR[it.rarity] ?? "#ddd" }}>{itemTitle(it)}</div>
          <div className="qm-sub">{["Rare", "Magic"].includes(it.rarity) && it.base_type && it.name ? it.base_type : it.item_class}{it.item_level ? ` · ilvl ${it.item_level}` : ""}{setup.links_min ? ` · ${setup.links_min}-link` : ""}</div>
        </div>
      </div>

      {awk && !check.bulk && (setup.category || hidden.length > 0 || it.rarity === "Unique") && (
        <div className="tw-chips" title="What the search asks for besides the ticked stats. Buttons switch on (gold) and off; the grey labels come from the item itself.">
          {setup.category && it.rarity !== "Unique" && (
            <button className={`tw-chip ${setup.base_only ? "on" : ""}`} onClick={() => setSetup({ ...setup, base_only: !setup.base_only })}>
              <ItemIcon base={setup.base || it.base_type || null} size={18} />{setup.base || it.base_type} only</button>
          )}
          {it.item_level != null && it.rarity !== "Unique" && setup.category && (
            <button className={`tw-chip ${setup.ilvl_min != null ? "on" : ""}`} title="Bases in the same item-level band: better mod tiers unlock at 75, 82, 84 and 86"
              onClick={() => { const on = setup.ilvl_min == null; setSetup({ ...setup, ilvl_min: on ? it.item_level ?? null : null, ilvl_max: on && it.item_level ? ilvlBand(it.item_level)[1] : null }); }}>
              ilvl {ilvlText(it.item_level)}</button>
          )}
          {it.rarity === "Unique" && (
            <button className={`tw-chip ${setup.corrupted != null ? "on" : ""}`} title="A corrupted unique can't be changed any more, and its corruption implicit is part of its price"
              onClick={() => setSetup({ ...setup, corrupted: setup.corrupted != null ? null : it.corrupted })}>
              <CurrencyIcon id="vaal" size={18} />{it.corrupted ? "Corrupted only" : "Not corrupted only"}</button>
          )}
          {(() => {
            const on = hidden[0] === "Instant buyout only";
            return hidden.length > 0 && (
              <button className={`tw-chip ${on ? "on" : ""}`} title={on ? "Only sellers you can buy from straight away. Click to include whisper-and-wait sellers too (this search only)."
                : "Includes sellers you have to whisper. Click for instant buyout only (the usual)."}
                onClick={() => setSetup({ ...setup, buyout: !on })}>{on ? "Instant buyout only" : "Any seller"}</button>
            );
          })()}
          {hidden.slice(1).map((h) => <span key={h} className="tw-chip fixed">{/online/i.test(h) && <span className="tw-online-dot" />}{h}</span>)}
        </div>
      )}
      {heroTop && hero}
      {style === "awakened-price" && res && <MarketStats res={res} divine={divine} />}
      {heroTop && trend}
      {check.sold != null && check.sold < 5 && (
        <div className="qm-warn tw-thin" title="poe.ninja's price comes from the few that sold. The filter's worth line counts cards like this under 50c as worthless.">
          Hardly traded: about {Math.max(1, Math.round(check.sold))} sold recently on the exchange, so this price is a guess.</div>
      )}
      {err && <div className="qm-warn">{err}</div>}
      {res && res.total === 0 && searched === searchKey(setup) && (setup.filters.some((f) => f.enabled) || (setup.base_only && setup.category) || setup.ilvl_min) && (
        <UntickHelp state={untick} onAsk={() => { setUntick("busy");
            invoke<{ key: string; label: string; total: number }[]>("trade_untick_one", { setup }).then(setUntick).catch((e) => setUntick(String(e))); }}
          onPick={(key) => {
            const s = key === "base" ? { ...setup, base_only: false } : key === "ilvl" ? { ...setup, ilvl_min: null, ilvl_max: null }
              : { ...setup, filters: setup.filters.map((f) => f.id === key ? { ...f, enabled: false } : f) };
            setUntick(null); setSetup(s); run(s); }} />
      )}
      {it.rarity === "Unique" && !it.name && it.base_type && <BaseUniques base={it.base_type} divine={divine} />}

      {check.map && <MapCheck map={check.map} rarity={it.rarity} corrupted={/\nCorrupted/.test(setup?.raw ?? "")} />}
      {check.compare && check.compare.length > 0 && <CompareBlock list={check.compare} />}

      {check.map && mapModCount > 0 && (
        <button className="link small tw-mapmods" onClick={() => setMapMods(!mapMods)}>
          {mapMods ? "▾" : "▸"} Search by map mods too ({mapModCount})</button>
      )}
      {setup.base_item && (() => {
        const b = setup.base_item;
        const pick = (base: boolean) => {
          if (base === asBase) return;
          if (base) {
            setOwnSetup(setup);
            setSetup({ ...setup, base_only: true, ilvl_min: b.ilvl_min, ilvl_max: null, misc: b.misc,
              filters: setup.filters.map((f) => ({ ...f, enabled: b.ids.includes(f.id), min: b.ids.includes(f.id) ? f.value : f.min })) });
          } else if (ownSetup) setSetup(ownSetup);
          setPreset(null); setAsBase(base);
        };
        return (
          <div className="tw-tabs" title={"Awakened PoE Trade's two searches: this item with its stats, or what it's worth as a base to craft on (" + b.why.join(", ") + ")"}>
            <button className={asBase ? "" : "on"} onClick={() => pick(false)}>This item</button>
            <button className={asBase ? "on" : ""} onClick={() => pick(true)}>As a base</button>
          </div>
        );
      })()}
      {asBase && (
        <div className="tw-base-sum">
          <b>Searching for:</b> {[setup.base || it.base_type, setup.ilvl_min ? `item level ${setup.ilvl_min}+` : null,
            ...(setup.misc ?? []).map((m) => m.replace("_item", "").replace(/^./, (c) => c.toUpperCase())),
            ...setup.filters.filter((f) => f.enabled).map((f) => f.label)].filter(Boolean).join(" · ")}
          {!setup.filters.some((f) => f.enabled) && <span className="muted"> (no mods: just the base)</span>}
        </div>
      )}
      {setup.filters.some(shown) && (
        <div className="tw-range">
          <span className="tw-sec">Minimum rolls</span>
          <span className="tw-range-btns" title="Set every minimum relative to this item's rolls">
          {[0, 10, 20].map((p) => (
            <button key={p} className={preset === p ? "on" : ""} title={p ? `Every minimum ${p}% below this item's roll` : "Every minimum at this item's roll"}
              onClick={() => { setPreset(p); setSetup({ ...setup, filters: setup.filters.map((f) => {
                // Open-slot counts and timeless jewel seeds are exact: never lowered.
                if (f.min == null || f.max != null || f.id.includes("empty") || f.id.includes("timeless")) return f;
                if (p === 0 || Math.abs(f.value) <= 3) return { ...f, min: f.value };
                const v = f.value * (1 - p / 100);
                return { ...f, min: Math.abs(f.value) >= 10 ? Math.floor(v) : Math.floor(v * 10) / 10 };
              }) }); }}>
              {p === 0 ? "exact" : `−${p}%`}</button>
          ))}
          </span>
        </div>
      )}
      {setup.filters.some(shown) && (() => {
        const jewel = setup.category === "jewel.base" || setup.category === "jewel.abyss";
        const max = setup.affix_max ?? (jewel ? [2, 2] : [3, 3]);
        const used = affixUsed(setup.filters, max);
        const flat = layout === "flat";
        const order = displayOrder(setup.filters.map((_, i) => i).filter((i) => shown(setup.filters[i])), setup.filters);
        const tucked = flat ? order.filter((i) => inATotal(setup.filters[i], setup.filters)) : [];
        // Like Awakened: a base mod with no roll range (the same on every item of this base, e.g. the Cogwork Ring's
        // "-1 Prefix Modifier allowed") never changes the search, so it's hidden until asked for. Only when the copy
        // showed ranges at all (the advanced copy), and never a ticked one or a map's.
        const advanced = setup.filters.some((f) => f.range != null);
        const fixed = advanced && !check.map ? order.filter((i) => { const f = setup.filters[i]; return f.kind === "implicit" && f.range == null && !f.enabled && !f.source; }) : [];
        const rows = order.filter((i) => !(flat && !showInTotals && tucked.includes(i)) && (showFixed || !fixed.includes(i)));
        return (
        <div className={`tw-stats ${flat ? "flat" : ""}`}>
          {rows.map((i, k) => {
            const f = setup.filters[i];
            // Grouped: a small heading where the kind of row changes, with what it means ("all mods added up",
            // "2 of 3 used"). Flat: no headings, a tag on each row instead.
            const g = rowGroup(f, max, used);
            const prev = k > 0 ? setup.filters[rows[k - 1]] : null;
            const head = !flat && (!prev || rowGroup(prev, max, used)[0] !== g[0]);
            const tag = flat ? rowTag(f) : "";
            const open = isOpenSlot(f);
            const label = open ? `${f.value} open ${f.id.includes("prefix") ? "prefix" : "suffix"}${f.value > 1 ? "es" : ""}`
              : f.label.replace(/\s*\(at 20% quality\)$/i, "");
            return (
              <React.Fragment key={f.id + i}>
                {head && <div className="tw-group"><span>{g[0]}</span>{g[1] && <span className="tw-group-hint">{g[1]}</span>}</div>}
                <label className={`tw-stat ${f.enabled ? "on" : ""} ${f.junk ? "junk" : ""} ${open ? "open" : ""}`} title={f.junk ? "Rarely changes the price" : undefined}>
                  <input type="checkbox" checked={f.enabled} onChange={(e) => patch(i, { enabled: e.target.checked })} />
                  <span className={`tw-label mod-${f.kind}`}>{label}{open && <span className="tw-room"> room to craft</span>}
                    {f.source && <SourceTag s={f.source} />}
                    {!flat && KIND[f.kind] && f.kind !== "pseudo" && f.kind !== "implicit" && <span className={`tw-kind tw-kind-${f.kind}`}>{KIND[f.kind]}</span>}
                    {f.tier != null && <span className={`tw-tier ${tierClass(f.tier)}`} title={f.kind === "pseudo" ? "The best tier among the mods adding up to this total (tier 1 is the best a mod can roll)" : "Tier 1 is the best this mod can roll"}>T{f.tier}</span>}
                    {f.roll != null && <Roll r={f.roll} range={f.range} tier={f.tier} />}</span>
                  {flat && <span className={`tw-tag ${tag.split(" ").pop()} ${tag.includes("crafted") ? "crafted" : ""}`}>{tag}</span>}
                  <input className="tw-min" type="number" value={f.min ?? ""} placeholder="min" onWheel={(e) => wheel(i, f, e)}
                    title={f.max != null ? "Exactly this many" : undefined}
                    onChange={(e) => { const v = e.target.value === "" ? null : +e.target.value; patch(i, f.max != null ? { min: v, max: v, enabled: true } : { min: v, enabled: true }); }} />
                </label>
              </React.Fragment>
            );
          })}
          {fixed.length > 0 && (
            <button className="link small tw-tucked" onClick={() => setShowFixed(!showFixed)}
              title="Base mods that are the same on every item of this base: ticking them doesn't change the search">
              {showFixed ? `Hide the ${fixed.length} fixed base mod${fixed.length > 1 ? "s" : ""}`
                : `${fixed.length} base mod${fixed.length > 1 ? "s" : ""} the same on every ${setup.base || it.base_type || "item like this"} · show`}</button>
          )}
          {tucked.length > 0 && (
            <button className="link small tw-tucked" onClick={() => setShowInTotals(!showInTotals)}>
              {showInTotals ? `Hide the ${tucked.length} mods counted in the totals` : `${tucked.length} mod${tucked.length > 1 ? "s are" : " is"} inside the totals above · show`}</button>
          )}
        </div>
        );
      })()}
      {setup.unknown.length > 0 && showStats && !asBase && (
        <div className="tw-unknown">
          <div className="tw-group" title="The trade site has no filter for these"><span>Not searchable</span></div>
          {setup.unknown.map((u, i) => {
            const k = setup.unknown_kinds?.[i] ?? "explicit";
            return <div key={i} className={`tw-label mod-${k} small`} title="The trade site has no search for this mod">
              {u}{KIND[k] && <span className="tw-kind">{KIND[k]}</span>}</div>;
          })}
        </div>
      )}

      {style === "tooltip" && <div className="tw-tt-price">{hero}{trend}</div>}
      {!awk && !check.bulk && layout !== "flat" && setup.category && it.rarity !== "Unique" && <div className="tw-group tw-opts-head"><span>Base and item level</span></div>}
      {!check.bulk && <div className="tw-opts">
        {!awk && setup.category && it.rarity !== "Unique" && (
          <label><input type="checkbox" checked={setup.base_only} onChange={(e) => setSetup({ ...setup, base_only: e.target.checked })} /> {setup.base || it.base_type} only</label>
        )}
        {!awk && it.item_level != null && it.rarity !== "Unique" && setup.category && (
          <label title="Bases in the same item-level band: better mod tiers unlock at 75, 82, 84 and 86"><input type="checkbox" checked={setup.ilvl_min != null}
            onChange={(e) => setSetup({ ...setup, ilvl_min: e.target.checked ? it.item_level ?? null : null, ilvl_max: e.target.checked && it.item_level ? ilvlBand(it.item_level)[1] : null })} /> ilvl {ilvlText(it.item_level)}</label>
        )}
        <span className="tw-actions">
        {!awk && saveBtn}
        <button className="tw-search" disabled={busy} onClick={() => run(setup)}
          title="Move your mouse onto this button to search (or click)"
          onMouseEnter={(e) => {
            // Hover to search, like Awakened, but only when coming from inside the window (not when it opens under the mouse).
            const from = e.relatedTarget as Node | null;
            if (!busy && !res && from && box.current?.contains(from)) run(setup);
          }}>{busy ? <><Spinner /> Searching…</> : searched ? "Search again" : "Search trade"}</button>
        </span>
      </div>}
      {!awk && !check.bulk && hidden.length > 0 && (
        <div className="tw-hidden">
          <button className="link small" onClick={toggleHidden} title="What the search also asks for, besides the ticked stats">
            {showHidden ? "▾" : "▸"} Also searching for ({hidden.length})</button>
          {showHidden && (
            <div className="tw-hidden-list small">
              {hidden.map((h) => <span key={h} className="tw-hidden-chip">{h}</span>)}
              {it.rarity === "Unique" && (
                <label className="tw-hidden-opt" title="A corrupted unique can't be changed any more, and its corruption implicit is part of its price">
                  <input type="checkbox" checked={setup.corrupted != null}
                    onChange={(e) => setSetup({ ...setup, corrupted: e.target.checked ? it.corrupted : null })} />
                  {it.corrupted ? "Corrupted ones only" : "Not corrupted only"}</label>
              )}
            </div>
          )}
        </div>
      )}
      {saving != null && (
        <form className="tw-save" onSubmit={(e) => { e.preventDefault();
          invoke<Saved[]>("saved_add", { name: saving, setup }).then((l) => { setSaved(l); setSaving(null); setShowSaved(true); }).catch((er) => setErr(friendly(er))); }}>
          <input className="text" autoFocus value={saving} onChange={(e) => setSaving(e.target.value)} placeholder="Name this search" />
          <button type="submit">Save</button>
        </form>
      )}

      {awk && style !== "awakened-price" && <div className="tw-awk-price">{hero}{trend}</div>}
      {check.from_ground && it.rarity !== "Unique" && setup.filters.length === 0 && ["Rare", "Magic"].includes(it.rarity) && (
        <div className="muted small">Read off the ground, so no stats: pick it up and press Ctrl+D in your inventory for a proper price.</div>
      )}
      {check.bulk && (
        <div className="tw-res">
          <div className="tw-sum tw-bulkhead">
            <span>{bulk?.offers[0] && bt ? <>{ninjaC != null ? "Market price" : "Going rate"} <b><Price n={ninjaC ?? bt.typical} cur="chaos" /></b> each
              {" "}<Other n={ninjaC ?? bt.typical} cur="chaos" divine={bulk.divine} /></> : busy ? "checking…" : "no offers"}</span>
            <span className="tw-pay">
              {(["chaos", "divine"] as const).map((p) => (
                <button key={p} className={pay === p ? "on" : ""} onClick={() => { setPay(p); runBulk(check.bulk!, p, minStock ? stack : null); }}>
                  pay in <CurrencyIcon id={p} /></button>
              ))}
            </span>
          </div>
          {stack > 1 && (
            <label className="tw-minstock" title="Like Awakened's stock filter: hides sellers who can't take your whole stack">
              <input type="checkbox" checked={minStock} onChange={(e) => { setMinStock(e.target.checked); runBulk(check.bulk!, pay, e.target.checked ? stack : null); }} />
              Only sellers with at least {stack} in stock
            </label>
          )}
          <ul className="tw-list">
            {bulk?.offers.map((o, i) => {
              // Like Awakened's "fulfill": how many trades of this offer's size the seller can actually fill.
              const fills = o.get > 0 ? Math.floor(o.stock / o.get) : o.stock;
              return (
                <React.Fragment key={i}>
                  {i === ratioAt && ninjaC != null && <MarketRatio c={ninjaC} divine={bulk.divine} />}
                  {(!bt?.fixer(o) || showFixers) && <li className={bt?.fixer(o) ? "fixer" : ""} title={bt?.fixer(o) ? "Much cheaper than the market price with very little stock: often a price fixer" : undefined}>
                    <span className="tw-price"><Price n={o.each} cur={o.pay_currency} /></span>
                    <span className="muted small"><Other n={o.each} cur={o.pay_currency} divine={bulk.divine} />{o.get > 1 ? ` · ${o.pay} for ${o.get}` : ""}{bt?.fixer(o) && <span className="tw-fixer">low stock</span>}</span>
                    <span className="tw-seller muted small" title={`${o.stock} in stock: fills ${fills} trade${fills === 1 ? "" : "s"} of ${o.get}`}>
                      {o.stock < o.get && <span className="tw-warn-ico">⚠ </span>}{o.stock} in stock{o.get > 1 ? ` · fills ${fills}` : ""} · {o.account}</span>
                    {o.whisper && <button className="link small" onClick={() => copy(o.whisper, "b" + i)}>{copied === "b" + i ? "Copied" : "Whisper"}</button>}
                  </li>}
                </React.Fragment>
              );
            })}
            {bulk && ratioAt === bulk.offers.length && ninjaC != null && <MarketRatio c={ninjaC} divine={bulk.divine} />}
          </ul>
          {fixers > 0 && (
            <button className="link small tw-fixers-toggle" onClick={() => setShowFixers(!showFixers)}
              title="Offers far below the market price with only a few in stock: usually there to drag the price down">
              {showFixers ? `Hide the ${fixers} likely price fixer${fixers === 1 ? "" : "s"}` : `${fixers} likely price fixer${fixers === 1 ? "" : "s"} hidden · show`}</button>
          )}
          {bulk && <button className="link small" onClick={() => invoke("open_url", { url: bulk.url })}>Open on the trade site</button>}
        </div>
      )}
      {res && res.listings.length > 0 && (
        <div className="tw-res">
          <div className="tw-sec tw-res-sec">For sale now <span className="tw-group-hint">cheapest first</span></div>
          {style === "compare" && <PriceSpread res={res} divine={divine} />}
          <div className="tw-table">
            <div className="tw-th"><span>Price</span><span>{ilvls ? "ilvl" : ""}</span><span>Listed</span><span>Seller</span><span /></div>
            {collapsed.map(({ l, n, i }) => (
              <div key={i} className="tw-tr">
                <span className="tw-price"><Price n={l.amount} cur={l.currency} />
                  {l.chaos != null && !["chaos", "divine"].includes(l.currency) && <span className="tw-eq">{money(l.chaos, divine)}</span>}
                  {n > 1 && <span className="tw-times" title={`The same seller lists ${n} of these at this price`}>× {n}</span>}</span>
                <span className="muted">{ilvls ? l.ilvl || "" : ""}</span>
                <span className="muted">{ago(l.indexed)}</span>
                <span className="tw-seller muted">{l.account}</span>
                {l.instant ? <span />
                  : l.whisper ? <button className="link small" onClick={() => copy(l.whisper, "l" + i)}>
                  {copied === "l" + i ? "Copied" : "Whisper"}</button> : <span />}
                {style === "compare" && l.mods && <ListingMods mods={l.mods} yours={setup.filters} />}
              </div>
            ))}
          </div>
        </div>
      )}
      {awk ? (
      <div className="tw-foot tw-awk-foot">
        {(res?.url || bulk?.url) && <button className="link small" onClick={() => invoke("open_url", { url: res?.url || bulk?.url })}><FootIco k="site" />Trade site</button>}
        {check.setup?.raw && check.setup.item.rarity !== "Unique" && /^(armour|weapon|accessory)\./.test(check.setup.category ?? "") && (
          <button className="link small" title="See what else can roll on it and the cheapest way to get a mod"
            onClick={() => { try { localStorage.setItem(CRAFT_ITEM_KEY, check.setup?.raw ?? ""); } catch { /* not passed */ } invoke("open_editor", { tab: "craft" }); }}>
            <FootIco k="craft" />Craft</button>)}
        {!check.bulk && saveBtn}
        {!check.bulk && <button className={`link small ${showSaved ? "on" : ""}`} onClick={() => setShowSaved(!showSaved)} title="Your saved searches">
          <FootIco k="saved" />Saved ({saved.length}){saved.some((x) => x.live) ? " · live" : ""}</button>}
        <button className="link small" onClick={nextStyle} title={"Switch the look of this window: " + TW_STYLES.map(([, n]) => n).join(", ")}>
          <FootIco k="style" />{TW_STYLES.find(([k]) => k === style)?.[1]}</button>
        <button className="link small" onClick={nextLayout} title="Grouped: headings. Flat: one list with a tag on each row, like Awakened.">
          <FootIco k="layout" />{TW_LAYOUTS.find(([k]) => k === layout)?.[1]}</button>
        {reporting == null && <button className="link small" onClick={() => { setReporting(""); setReported(""); }}
          title="Something's wrong? Saves this price check with your note, so Claude can see exactly what went wrong">
          <FootIco k="report" />{reported ? "✓ Saved" : "Report"}</button>}
      </div>
      ) : (
      <div className="tw-foot">
      {res && res.url && <button className="link small tw-open-site" onClick={() => invoke("open_url", { url: res.url })}>Open on the trade site</button>}
      {/* Gear you can craft on: open the Crafting page with this item (shared localStorage, see Craft.tsx). */}
      {check.setup?.raw && check.setup.item.rarity !== "Unique" && /^(armour|weapon|accessory)\./.test(check.setup.category ?? "") && (
        <button className="link small tw-open-site" title="See what else can roll on it and the cheapest way to get a mod"
          onClick={() => { try { localStorage.setItem(CRAFT_ITEM_KEY, check.setup?.raw ?? ""); } catch { /* not passed */ } invoke("open_editor", { tab: "craft" }); }}>
          Craft this</button>)}
      <button className={`link small tw-more-btn ${more ? "on" : ""}`} onClick={() => setMore(!more)}>More {more ? "▴" : "▾"}</button>
      {more && <div className="tw-more">
      {!check.bulk && savedToggle}
      <button className="link small tw-style-btn" onClick={nextStyle} title={"Switch the look of this window: " + TW_STYLES.map(([, n]) => n).join(", ")}>
        Style: {TW_STYLES.find(([k]) => k === style)?.[1] ?? "Classic"} ›</button>
      <button className="link small tw-layout-btn" onClick={nextLayout} title="Grouped: headings. Flat: one list with a tag on each row, like Awakened.">
        Layout: {TW_LAYOUTS.find(([k]) => k === layout)?.[1] ?? "Grouped"} ›</button>
      {reporting == null && (
        <button className="link small tw-report-toggle" onClick={() => { setReporting(""); setReported(""); }}
          title="Saves this price check with your note, so Claude can see exactly what went wrong">
          {reported ? "✓ Report saved" : "Something's wrong?"}</button>
      )}
      </div>}
      </div>
      )}
      {!check.bulk && savedPanel}
      {reporting != null && (
        <form className="tw-report" onSubmit={(e) => { e.preventDefault();
          invoke<string>("report_problem", { note: reporting, data: { check, setup, res, bulk, err } })
            .then(() => { setReporting(null); setReported("ok"); }).catch((er) => setErr(friendly(er))); }}>
          <textarea className="text" autoFocus rows={2} value={reporting} onChange={(e) => setReporting(e.target.value)}
            placeholder="What looks wrong? e.g. price too high, wrong mod ticked" />
          <div className="row"><button type="submit">Save report</button>
            <button type="button" className="link small" onClick={() => setReporting(null)}>Cancel</button></div>
        </form>
      )}
    </div>
    {style === "split" && (
      <aside className="qm tw-market">
        <div className="tw-sec">Market</div>
        {hero}
        {trend}
        {res && <MarketStats res={res} divine={divine} />}
        <PriceSpread res={res} divine={divine} />
      </aside>
    )}
    {relName && (
      <aside className="qm tw-side" hidden={!hasSide}>
        <RelatedItems name={relName} divine={divine} onData={setHasSide} />
      </aside>
    )}
    </div>
  );
}
